# Roadmap

Status: accepted direction; items below are planned unless marked implemented

Reviewed: 2026-08-08 against current implementation revision
`56869ad0a61565012cc039bd6c94f01afb34f098`; the claim-ledger baseline remains
`f1048bc0399fad9801559ca7fcfd3163427b5832`.

This roadmap contains future product work only. Current behavior belongs in the
[README](../README.md), [architecture](architecture.md), generated rustdoc, and
[testing contract](testing.md).

## Non-negotiable constraints

Every roadmap item must preserve:

- codec-only scope: detection, inspection, decoding, and encoding;
- no public image-processing API;
- `bytemuck` as the only Cargo dependency;
- independently feature-gated formats;
- Rust-only default codecs;
- eventual full WASM functionality for every published codec feature;
- explicit output format selection;
- structured `ImageResult` failures;
- exact manifest-driven Pillow success and error parity; and
- 100% line, branch, function, and region coverage as a release target; the
  current accepted snapshot is recorded below and is not represented as 100%.

Native AVIF is migration debt, not an exception to the final WASM constraint.

## Implemented foundation

The following decisions are already implemented and are not roadmap work:

- canonical auto-detecting root APIs;
- signature-validated explicit-format still decode for trusted out-of-band
  format knowledge;
- private codec dispatchers;
- separate source `ImageFormat` and decoded `ImageMode`;
- exact palette, alpha, frame, timing, disposal, and background transfer
  models;
- structured errors rather than fallible `Option`;
- target-qualified typed encoder options with strict migration from legacy
  string pairs;
- one shared decode policy with pre-detection encoded-byte and inspected
  primary-canvas limits;
- immutable encoded snapshots and shared lazy decode results;
- checked zero-copy `DecodedImage` constructors and palette builders alongside
  explicitly unchecked compatibility builders;
- runtime capability discovery that distinguishes feature, target, and
  operation availability;
- stable `UnsupportedReason` values that align target-unavailable and
  not-implemented operation failures with capability discovery;
- one Cargo feature per format, with ICO forwarding PNG and BMP;
- exact fixture-backed errors and byte outputs;
- no general image-processing layer; and
- complete retained third-party provenance and legal texts.

## Revision-bound implementation audit

This section is the single follow-up ledger requested after the public API and
ecosystem comparison. It is intentionally kept in the roadmap instead of
creating another active document. Delete resolved rows as their behavior moves
into the README, architecture reference, rustdoc, or testing contract.

The correction evidence below is the committed state based on
`f1048bc0399fad9801559ca7fcfd3163427b5832`, identified by manifest SHA-256
`bffa47f55b0a4ef2d64979392410e7544617fcebdedcd4086cd76532a4c936e3`
and generated matrix SHA-256
`b087396b064ed216a03ed789d9a6171d1f97ec99491f2f90f0c134bce29bf510`.
Findings were produced from:

- the public types and dispatch in [`src/lib.rs`](../src/lib.rs),
  [`src/types/`](../src/types/), and
  [`src/codecs/mod.rs`](../src/codecs/mod.rs);
- all eight inspectors, decoders, and encoders;
- `Cargo.toml`, `build.rs`, the CI workflow, package file list, and feature
  matrix script;
- every assertion made by `tests/coverage_matrix_tests.rs` and
  `tests/feature_gate_tests.rs`;
- the active 1,417-row fixture manifest;
- direct probes against the repository's Pillow 12.2.0 virtual environment;
  and
- current primary documentation for comparable codec libraries.

Finding classes used below:

| Class | Meaning |
| --- | --- |
| Confirmed defect | Current code returned wrong data, panicked, or silently discarded represented state in a direct reproduction |
| Evidence gap | The implementation may be right, but the retained tests do not prove the stated claim |
| Missing capability | A useful encode/decode contract is explicitly unavailable |
| Contract ambiguity | Public types or documentation permit two reasonable interpretations |
| Build/release gap | Source behavior may work, but a target, artifact, or downstream path is unproved |

Confirmed defects and over-broad evidence claims require correction. Missing
capabilities remain research backlog until accepted one slice at a time; their
presence here does not authorize image processing, new dependencies, or a
change to the WASM requirement.

This is not a security audit. Caller-controlled limits remain a release blocker
because they are part of a robust decoder contract, but this audit does not add
filesystem policy, sandboxing, or a general hardening project to the codec-only
scope.

### Executive result

The common auto-detecting API, per-format feature dispatch, persistent
still-image lazy cache, exact encoded-byte comparisons, and exact decoded-pixel
comparisons are real. The broader statement "every codec is complete" is not
supported.

The correction sweep fixed the WebP indexed/bilevel panics and color errors,
strict WebP/JPEG option validation, silent one-frame sequence metadata loss,
unchecked public-mode dispatch, CUR hotspot loss, and lossy WebP's failure to
choose an uncompressed alpha chunk when it is smaller. It also disproved the
suspected TIFF numeric-endianness defect: Pillow's observable contract
preserves the source sample bytes rather than canonicalizing scalar byte
order. The decoded-sequence schema now independently proves retained source
geometry, exact rational timing, disposal, blend, interlace, default-image
state, loop/background data, pixel layout, and exact rendered bytes where the
Pillow and Rust layouts are comparable. AVIF detection now rejects generic
`mif1`/`msf1` HEIF containers unless their bounded `ftyp` brands include
`avif` or `avis`. The encoder input surface remains materially narrower than
Pillow 12.2.0 for JPEG, GIF, TIFF, WebP, ICO, and AVIF.

### Completed corrections after the audit

The COR-001–COR-072 correction records and TST-001–TST-010 test-system
records are closed and removed from the active roadmap queue. Their current
behavior and acceptance contracts live in the README, architecture reference,
testing contract, rustdoc, committed fixtures, and claim ledger. New confirmed
defects belong in the immediate correction queue below; future capability work
belongs in the API, codec, FTR, and QA backlog tables.

The claim-ledger baseline all-feature Coverage MCP run
`9bbe6760-7aa9-4ed8-8b31-bbf65444b85a`, snapshot
`f9a2fc69-ad68-493e-9c46-8837d0dd8d52`, passed 58 tests with zero failures
or skips and reports 47,943/47,943 lines, 6,578/6,578 branches,
2,686/2,686 functions, and 74,654/74,654 regions. The same committed
run includes the TIFF, PNG/BMP, one-frame BMP sequence, ICO still and
one-frame ICO sequence structural sink contracts, GIF still, WebP still,
native AVIF still, cancellation checkpoints, and the deterministic encode
work-budget contract, and passes the strict LLVM coverage verifier.
Strict Clippy, rustfmt, every isolated native feature lane, and every supported
WASM compile/rustdoc lane also pass. The WebP root-cause trace additionally
corrected VP8L histogram-map sampling/box references for small palettes and
VP8 terminal padding ownership. A retained defensive optimizer-state model
covers the box-chain state that cannot be independently selected by an image
fixture; it is explicitly labeled as defensive-model evidence rather than
Pillow parity.

### Immediate correction queue

The immediate correction queue is empty. New confirmed defects discovered
while implementing later rows belong here first; capability expansion must not
silently bypass the schema, limits, or target-evidence gates below.

### What the current manifest actually proves

The active fixture manifest contains 1,024 decode/inspect/verify rows and 393
encode rows (1,417 active rows, with no planned or unwired rows). The managed
Pillow parity command reports 1,445 checks because its 28 worker functions add
execution-level checks around those fixture rows. The separate
`feature_gate_tests` command contributes 45 non-Pillow contract assertions per
native and `wasm32-wasip1` runtime lane in the 991-check matrix. These counts
and origins are distinct; neither row count nor aggregate coverage expands the
Pillow assertion schema.

| Surface | What is asserted now | Missing from the oracle assertion |
| --- | --- | --- |
| Detection | Explicit operation success/error and expected common `ImageFormat`; Pillow registration predicates cover seven formats, while AVIF uses the bounded specification/libavif compatibility rule and retains Pillow's final open outcomes | Extension aliases, ICO-versus-CUR identity, and the separate headerless-DIB scope decision |
| Decode | Explicit still-operation success/error, exact width, height, mode, palette state and table bytes, decoded byte length, every decoded pixel/sample byte, and exact TIFF source byte order | Decoded auxiliary metadata and non-byte-order source descriptors |
| Inspect | Explicit operation success/error, format, width, height, mode, encoded bit depth, bit-depth evidence origin, exact palette state and table bytes for successful decode rows, animation flag, optional frame count, and exact TIFF source byte order | ICC/EXIF/XMP/text/orientation; independent palette bytes for inspect-success/decode-error rows; broader source descriptors |
| Sequence decode | Exact canvas, loop, background, frame count/order, source rectangle, rational duration, disposal, blend, interlace, default-image state, pixel layout, mode/size, exact TIFF per-page source byte order, and exact rendered frame bytes where Pillow exposes the same layout | Exact raw GIF source-rectangle bytes and auxiliary per-frame metadata |
| Encode success | Explicit still/sequence operation applicability, exact complete encoded bytes, container checks, and exact re-decoded reference pixels when applicable | Systematic coverage of every Pillow input mode × target format; metadata not represented by the source model |
| Encode/decode error | Explicit per-operation failure; exact Pillow exception type/message when an exception exists; separately asserted Rust kind, selected format, non-empty contextual diagnostic policy, and evidence origin | Pillow has no equivalent fields for operation stage, byte offset, chunk/marker/tag identity, typed limit reason, cancellation, or output-write cause; those are separate Rust contracts |
| Lazy source | Inspection before decode, one shared successful or failed still decode, separate lazy sequence materialization, concurrency, clone-visible cache state, and explicit not-attempted/succeeded/failed state per cache | Cache eviction; repeated verification cost |
| Coverage | Release target: 100% aggregate native all-feature line, branch, function, and region metrics across parity, defensive contracts, and permitted private coverage models; the current accepted snapshot at `295965ae-83c5-4fe2-a09b-396be34d020e` covers implementation, test, and runtime revision `56869ad0a61565012cc039bd6c94f01afb34f098`: 53,345/53,961 lines, 7,567/7,718 branches, 3,001/3,077 functions, and 82,549/83,927 regions. Compared with the preceding accepted snapshot `83634c29-ba52-4054-a695-7417262366ff`, covered/source totals changed by +7/+6 lines, +4/+4 branches, +0/+0 functions, and +10/+11 regions; the regular Cargo test profile remains `opt-level = 2`, while the warm feature-matrix run used 12 concurrent lanes, one test worker per lane, one build job per lane, debug 0, and verbose 0; explicit overrides remain available. Unknown-target compile-only lanes lint the library surface without rebuilding integration targets already compiled by native/WASI lanes; this harness behavior adds no fixture, parity row, or coverage-only test. The known LLVM JSON segment-normalization warning remains; the strict aggregate shortfall is 616 lines, 151 branches, 76 functions, and 1,378 regions. Row assertion origins remain separate, and every exact `#[cfg(coverage)]` guard is accounted for by the static non-Pillow origin inventory. | Full semantic manifest execution in a WASM runtime |

The suite does not claim Python and Rust error-type identity. Pillow's exact
exception type/message are retained as oracle evidence, while callers should
use Rust `ImageErrorKind` and optional format for recovery. The Rust diagnostic
message is required where context exists but is not compared with Pillow prose.

The sequence schema is established. Future sequence formats must use it; they
may not fall back to whole-millisecond or rendered-pixel-only assertions.

### Encoder input contract

`encode` does not accept arbitrary encoded image bytes or an abstract "any
image." It accepts one validated `DecodedImage` whose exact mode must be
supported by the selected target encoder. This is consistent with low-level
codec APIs in the ecosystem, but the accepted modes differ per codec.

The following Pillow result was measured locally with Pillow 12.2.0 and its
JPEG, zlib, libtiff, WebP, and AVIF features enabled. Each row used a valid
16×16 `Image.new(mode)` and default save options. Pillow frequently performs a
private pre-save conversion; matching that does not require a public
image-processing API.

| Target | Pillow 12.2.0 accepted source modes | Current Rust accepted direct modes | Minute gap |
| --- | --- | --- | --- |
| JPEG | `1`, `L`, `RGB`, `CMYK`, `YCbCr` | `L8`, `Rgb8` | Missing bilevel normalization, CMYK encode, and any YCbCr transfer mode |
| PNG | `1`, `L`, `LA`, `P`, `RGB`, `RGBA`, `I`, `I;16` | `L1`, `L8`, `La8`, `P8`, `Rgb8`, `Rgba8`, `L16` | Missing Pillow `I` compatibility; Pillow has announced this save path for removal in 13, so pin behavior before deciding |
| GIF | `1`, `L`, `LA`, `P`, `RGB`, `RGBA`, `I`, `F`, `I;16` | `P8`, `L8`, `Rgb8`, `Rgba8` | Missing Pillow's private normalization for bilevel, alpha-luma, integer, float, and 16-bit inputs |
| BMP | `1`, `L`, `P`, `RGB`, `RGBA` | `L1`, `L8`, `P8`, `Rgb8`, `Rgba8` | Core mode surface aligns |
| TIFF | all probed modes: `1`, `L`, `LA`, `P`, `RGB`, `RGBA`, `CMYK`, `YCbCr`, `I`, `F`, `I;16` | `L1`, `L8`, `La8`, `Rgb8`, `Rgba8`, `Cmyk8`, `I32`, `F32`, `L16` | Missing `P8` and YCbCr transfer; broader 16-bit/multichannel TIFF layouts remain absent |
| WebP | all probed modes | `L1`, `L8`, `La8`, `P8`, `Rgb8`, `Rgba8`, `Cmyk8` | Missing integer, float, 16-bit, and YCbCr normalization |
| ICO | `1`, `L`, `LA`, `P`, `RGB`, `RGBA`, `I`, `I;16` | PNG-backed path inherits PNG modes; BMP-backed path accepts only `Rgb8`/`Rgba8` | Missing `I`; no caller-supplied multi-entry model |
| AVIF | all probed modes | Native `L1`, `L8`, `La8`, `P8`, `Rgb8`, `Rgba8`, `Cmyk8` | Missing YCbCr, integer, float, and 16-bit normalization; WASM encoding is absent |

The correct direction is two explicit layers:

1. keep a strict low-level contract where a codec either accepts an exact mode
   or returns `Unsupported`; and
2. add only the private, fixture-proven normalization that the target encoder
   intrinsically requires.

Do not add general `convert`, resize, crop, rotate, or compositing APIs. A
conversion used only to feed a target codec is codec implementation, while a
public reusable conversion layer would violate project scope.

### Common API and data-model backlog

The current API-023/API-036 work-control record now includes the lossy WebP
RGBA alpha-palette source collection and index packing after each 1,024 source pixels and
nearest-delta candidate scan and the lossless VP8L palette
sign/nearest-delta ordering scan, plus lossless VP8L palette-index lookup
candidate scans and image-palette construction and palette-mode index packing after each 1,024 source pixels;
the source collection is charged at that 1,024-pixel boundary, while candidate
values are charged after 64 items (and palette entries after each 64 entries for
the bounded setup and lookup passes), plus Huffman
code-length emission after each 16 compressed token entries. These are covered
by the existing Rust-only feature-gate contract.
The remaining API backlog still covers deeper codec interruption, transient
allocation accounting, progress, and rollback.
| ID | Class | Finding | Attack and acceptance |
| --- | --- | --- | --- |
| API-008 | Missing representation | No YCbCr/YCCK/BGR transfer mode exists, constraining otherwise codec-native JPEG/TIFF/WebP/AVIF input contracts. | Add a mode only when at least one decode or encode fixture needs byte-preserving transfer. Avoid adding modes merely to mirror another library. |
| API-014 | Memory behavior | Lazy materialization retains the complete encoded snapshot and each successful still/sequence decoded payload forever; clones share both, and the retained-payload model now documents the independent caches and borrowed-view distinction. There is no eviction, repeated `verify` reparses independently, and codec temporary allocations are not yet measured. | Keep the retained-payload accounting current; benchmark allocator/peak behavior before adding optional cache release or cached verification. |
| API-017 | Output model | Encoders still produce complete `Vec<u8>` output for their return APIs, and codec working state can remain whole-buffer until a validated sink segment is ready. The dependency-free `OutputSink` contract normalizes write and post-delivery flush rejection to `ImageError::OutputWrite`; every current JPEG, PNG, GIF, BMP, TIFF, WebP, ICO, and native AVIF sink path now emits validated structural segments after exact-length preflight. Every sink path calls `OutputSink::flush` once after complete delivery. JPEG delivery splits marker/scan spans, GIF delivery splits signature/logical-screen, color-table, extension/image sub-block, and trailer segments, WebP delivery splits RIFF/chunk spans, ICO delivery splits its directory from embedded payload, TIFF delivery splits its header, page strip/padding, and IFD/value spans, and native AVIF delivery splits validated ISO-BMFF top-level boxes. These are structural delivery boundaries, not universal streaming; PNG filtered/compressed buffers, BMP row/palette segments, GIF working state, WebP encoded RIFF state, ICO embedded payload, TIFF page/compressed-pixel state, and native AVIF's complete encoded buffer remain bounded working state. A Rust-only contract now exercises a genuine partial second structural write across every still encoder and supported multi-frame GIF/TIFF/WebP/native-AVIF sequence writer available in each feature/target lane, preserving the delivered prefix, reporting the selected `StillEncode` or `SequenceEncode` stage, and avoiding `flush`; broader interrupted-write, rollback, and cleanup behavior remain open. | Reduce transient working buffers one independently enforceable boundary at a time. Keep `Vec` convenience wrappers, preserve the structured output-write cause, and define behavior for every short-write/rollback path before claiming universal streaming. |
| API-018 | Input model | The incremental input contract now covers detection, basic inspection, still decode, and sequence decode (`decode_prefix`/`decode_sequence_prefix`, COR-059) with exact or progress-aware `NeedMoreData { minimum }`; streaming decompression that produces partial pixels before the container completes remains future work. | Keep the same status semantics for any future streaming iterator/reader surface. |
| API-019 | Metadata | PNG known metadata chunks, GIF extensions, JPEG APPn/COM marker payloads, WebP ICCP/EXIF/XMP chunks, TIFF metadata tags, and AVIF top-level unknown/free/skip boxes are retained as raw opaque records. Recognized AVIF `Exif` items and `mime` items with content type `application/rdf+xml` are retained as ordered raw `OpaqueMetadata` records on still and sequence decode; primary AVIF CICP/`clli`/`mdcv` color properties, `prof`/`rICC` ICC profiles, primary `av1C` chroma sample position, and `irot`/`imir`/`pasp`/`clap` item properties remain typed source descriptors. Direct alpha `auxl` provenance is represented by `SourceAlpha::Auxiliary`, the scalar and bounded plural auxiliary-relationship getters, the ordered `SourceDescriptor::avif_grid_item_ids()` list for the supported primary grid, bounded `dimg`/other `iref` edges through `SourceDescriptor::avif_item_relationships()`, filtered `prem` edges through `SourceDescriptor::avif_premultiplied_relationships()`, typed non-primary `colr`/`nclx` CICP declarations through `SourceDescriptor::avif_item_color_properties()`, and raw non-primary `prof`/`rICC` profiles through `SourceDescriptor::avif_item_icc_profiles()`. Full grid topology, unknown item properties, other non-primary/auxiliary color forms, and other item metadata remain open. | Extend the opaque model to the remaining AVIF item/property graph and exact color fields; parsed semantics are optional and format-specific. |
| API-020 | Same-format output | Source format is retained, but encoding always asks for an explicit target. | Keep explicit target selection. Add a same-source convenience only if metadata, sequences, and unsupported modes cannot make it silently lossy. |
| API-023 | Partial capability | Remaining gaps are transient encoded-output allocation/peak accounting (the public policy deliberately makes no recoverable-OOM promise), interior work beyond the current checkpoint set, and complete short-write/rollback semantics. The implemented decode, output-admission, cooperative work checkpoints, JPEG RGB-to-YCbCr conversion and chroma-downsample output after each 1,024 pixels, baseline entropy traversal after each 1,024 MCUs, JPEG forward-DCT/quantization after each completed 8x8 block, optimized baseline Huffman frequency gathering after each 1,024 AC coefficients, progressive scan block-slot generation after each 1,024 blocks, progressive scan-event frequency gathering after each 1,024 events, progressive scan coefficient traversal after each 1,024 coefficients, JPEG baseline/progressive entropy-output after each 1,024 emitted bytes, high-color GIF nearest-palette candidate ordering and bounded scans after each 1,024 work items, lossy WebP RGBA transparent-area cleanup and alpha-palette source collection and index packing plus lossless WebP VP8L hidden-RGB cleanup and image-palette construction and palette-mode index packing after each 1,024 scanned/source pixels, lossy WebP VP8 first-partition coding after each 8, 16, 32, 64, 128, 256, 512, 1,024, 2,048, 4,096, 8,192, 32,768, 65,536, 131,072, and 262,144 logical coded bits and coefficient coding through 1,048,576 logical coded bits, lossless WebP VP8L entropy-mode histogram-cost scans after each 64 symbols, palette-index lookup candidate scans after each 64 palette entries, palette sign collection and nearest-delta candidate scans after each 64 palette entries or candidate values, entropy-bin histogram-clustering min/max and bin-assignment pre-passes after each 64 tile histograms, copy-token cache-population scans after each 256 pixels, Huffman RLE preparation and canonical-code assignment scans after each 64 code-length symbols, Huffman-tree insertion scans after each 64 candidate nodes, Huffman-tree code-length-token frequency and trailing zero-repeat-token trim scans after each 16 compressed token entries, Huffman code-length emission after each 16 compressed token entries, and lossless WebP VP8L bitstream coding after each 8, 16, 32, 64, 128, 256, 512, 1,024, 2,048, 4,096, 8,192, 16,384, 32,768, 65,536, 131,072, 262,144, 524,288, and 1,048,576 logical coded bits are current behavior documented in the architecture/testing contracts, not active roadmap items. | Add one independently enforceable allocation or work dimension at a time; preserve unlimited wrappers, reject before future bounded allocation/work begins, and fixture each inclusive boundary and error-precedence rule. |
| API-026 | Ownership limitation | Decoded samples and palettes are always owned mutable vectors. Callers cannot borrow immutable output, reuse an allocation, or transfer shared backing storage without a copy. | Let the destination-buffer work solve reuse first. Add borrowed/shared public representations only if native and WASM measurements show a material copy cost. |
| API-027 | Sequence scalability | The source-bound `decode_frame` contract is complete with stable per-frame errors, and TIFF has a genuine per-page decode path. GIF, APNG, WebP, and AVIF still decode the full sequence for one frame, and there is no iterator. The owned source now retains a separate lazy sequence cache, but it still materializes every frame. | Extend the per-frame path to GIF/APNG/WebP/AVIF, then add iteration. Keep eager `decode_sequence` as a convenience collector and retain the shared lazy cache as the repeated-call path. |
| API-030 | Error detail | Codec-dispatched failures now retain a stable operation `stage`, the encoded-input byte `offset`, and a container-structure `identity` through the corresponding accessors. Caller-owned sink rejection has the separate `OutputWrite` category with selected output format, encode stage, and diagnostic message; `EncodePolicy` failures carry the selected format, encode operation, typed `EncodedOutputBytes` or `EncodeWorkUnits` resource, maximum, and observed result/checkpoint value. `Unsupported` additionally exposes `unsupported_reason()` for target-unavailable and not-implemented capability failures. BMP header, palette, pixel-span, bitfield, and RLE parse failures now retain stable context, ICO header, directory, entry-range, and embedded PNG/DIB/CUR failures now retain stable ICO context, TIFF compressed strip/tile payload failures now retain `tiff_strip`/`tiff_tile` context, and WebP inspection/container-chunk failures now retain stable WebP context. WebP still and sequence payload-decoder failures now retain `webp_bitstream` at the validated VP8/VP8L payload start, or the current ANMF container offset for animation; finer decoder-internal cursors remain intentionally limited. | Extend structured fields without promising unstable prose. Every newly represented field needs malformed, boundary, capability, and output-destination fixtures. |
| API-033 | Output-sample ambiguity | Callers cannot choose source-preserving versus normalized samples, byte order, alpha association, or a codec-native output colorspace. | Define explicit output policy only for byte-preserving codec needs. The default remains Pillow-observable normalized transfer bytes. |
| API-034 | Missing metadata | PNG source color fields (sRGB intent, gamma, chromaticities, raw ICC profile), primary AVIF CICP/`clli` fields (primaries, transfer, matrix, range, maxCLL, maxPALL), primary AVIF `mdcv` mastering-display fields, primary AVIF `prof`/`rICC` ICC profile bytes, primary `av1C` chroma sample position, and primary AVIF `irot`/`imir`/`pasp`/`clap` declarations are retained. Recognized AVIF EXIF/XMP item payloads are retained raw, without semantic parsing or pixel transforms; direct alpha provenance is represented by `SourceAlpha::Auxiliary` plus scalar and bounded plural source-local relationships, the supported primary grid retains its ordered derived item IDs, bounded `iref` edges—including `prem`—are retained as source descriptors, typed non-primary/auxiliary `colr`/`nclx` CICP declarations retain their source-local item IDs through `SourceDescriptor::avif_item_color_properties()`, and non-primary/auxiliary `prof`/`rICC` profiles retain their exact item IDs and raw profile bytes through `SourceDescriptor::avif_item_icc_profiles()`. Other non-primary/auxiliary color forms, JPEG Adobe/JFIF color interpretation, TIFF colorimetric tags, and WebP color metadata are not yet retained. | Preserve the remaining opaque profiles and exact container fields per format. Never imply that retaining color, metadata, or transform fields means pixel conversion was applied. |
| API-036 | Work control | Remaining gaps are progress semantics, CPU/instruction interruption inside codec work beyond the documented checkpoints, finer WebP stages beyond the current logical-bit, output-byte, and documented codec-internal intervals, JPEG interior work beyond its current 1,024-pixel RGB-to-YCbCr, 1,024-pixel chroma-downsample output, 1,024-MCU baseline entropy traversal, completed 8x8 forward-DCT/quantization-block, optimized baseline Huffman frequency gathering after each 1,024 AC coefficients, progressive scan block-slot generation after each 1,024 blocks, progressive scan-event frequency gathering after each 1,024 events, progressive scan coefficient traversal after each 1,024 coefficients, and 1,024-byte entropy-output intervals, and short-write/rollback cleanup. Current cancellation and sink-boundary behavior belongs in the architecture/testing contracts. | Define progress and rollback semantics without claiming universal interior interruption; add checkpoints only for a real long-running operation and retain a separate Rust-only feature-gate contract when Pillow has no equivalent result. |
| API-038 | Detection policy | Auto-detection cannot be restricted to an allowed-format set. The explicit-format still API validates a selected format, but `DecodePolicy` cannot express an allowed set or combine that restriction with partial-input flow. | Let a decode policy carry an optional allow-list while retaining signature validation and feature-independent `detect_format`; keep it distinct from the explicit-format dispatch API. |
| API-041 | WASM boundary | Rust enums, structured errors, byte ownership, and 64-bit sizes have no stable JavaScript transfer schema. | Design a versioned binding contract after native API semantics settle; preserve precise error kinds and avoid string-only JS failures. |
| API-043 | Partial-input contract | The non-terminal `NeedMoreData { minimum }` state now exists for detection, basic inspection, still decode, and sequence decode, with exact minimum-byte or progress semantics; terminal results must never be retried. | Keep the status stable for any future streaming surface and document per-operation progress. |
| API-044 | Partial capability | Current resource limits are per-call eligibility checks before cache access, are never cached, and cannot be bypassed by cached success. Future output mode, strictness, metadata, or color/alpha policies would still make the single permanent still-decode cache key ambiguous. | Keep resource eligibility outside the cache key. Before API-033 or another result-shaping policy lands, choose separately keyed materialization or explicitly disallow that policy on cached sources. |
| API-045 | Repeated parsing | `EncodedImage::new` detects and inspects once; owned and borrowed source-bound still/sequence decode now reuse that validated format instead of repeating signature detection, but each codec still parses the container for materialization and `verify()` independently reparses it on every call. | Measure the remaining duplicate codec work, then retain an immutable parsed header/index only when every codec can prove that reuse cannot make later validation weaker. |
| API-046 | Output-layout preflight | Callers can preflight exact row bytes, packed-row status, total allocation, and alignment through `TransferLayout` (API-025), but per-plane sizes, byte order, and future codec-native layouts are still absent. `ColorType::bits_per_pixel` is insufficient for source-endian TIFF numeric bytes and future planar data. | Add a checked transfer-layout result for each new layout as it lands. Keep it about byte transport, not image processing. |
| API-047 | Information completeness | `ImageInfo.frame_count: Option<u32>` collapses not-applicable, not-yet-scanned, scan-limited, malformed-later, and genuinely unknown states. A partial demux cannot report “N complete frames seen, another is partial.” | Replace the optional count with a small completeness/result model before incremental inspect or frame enumeration becomes public. |
| API-048 | Source subtype loss | `Decoded<T>` retains only the eight-format enum. It cannot identify APNG versus PNG, classic TIFF versus BigTIFF, ICO versus CUR without inspecting a selected hotspot, VP8/VP8L/VP8X, AVIF item versus sequence source, or the source precision/profile class. | Add codec-specific inspected descriptors behind the format feature. Keep `ImageFormat` as the stable dispatch identity. |
| API-050 | Loop-count ambiguity | `Option<u32>` does not name whether a value means total plays, additional repetitions, a file-format loop field, unknown repetition, or infinity. GIF/WebP convention and libavif's `n + 1` repetition contract differ. | Introduce explicit `Unspecified`, `Finite { total_plays }`, `Infinite`, and, if an oracle exposes it, `Unknown` states with checked per-format conversion. |
| API-051 | Rational-duration identity | Exact numerator/denominator retention raises two distinct equality questions: `1/2` and `2/4` are the same duration but different source fields. Unbounded LCM conversion can also overflow when choosing one sequence timescale. | Preserve raw source fields separately from a reduced semantic duration, use checked arithmetic, and make every encoder's quantization/overflow result explicit. |
| API-052 | Reserved presentation values | A format-neutral `Reserved(u8)` disposal or blend value does not identify the governing format or whether round-trip replay is legal. The same numeric code has no universal meaning across GIF, APNG, and WebP. | Retain a format-qualified raw code beside normalized known semantics; unknown values must not be silently replayed into another target. |
| API-053 | Rendered-frame state | `RenderedCanvas` says the pixel extent but not whether the returned canvas is before blend, after blend, or after disposal, nor which prior frame state was used. That distinction affects frame extraction, seeking, cache reuse, and re-encoding. | Define the exact presentation instant in rustdoc and fixtures. Expose raw source rectangles separately when exact container reconstruction needs them. |
| API-054 | Mixed-frame contract | `DecodedSequence` has no canvas sample mode or palette namespace. Frames may carry different modes and local palettes, while a GIF background index refers to a global table rather than an arbitrary frame palette. | Define allowed mixed-mode sequences and give palette-index backgrounds an explicit palette owner before generic sequence encoding expands. |

### Codec-by-codec capability backlog

The following lists are deliberately more granular than the feature table in
the README. "Missing" does not mean it must all ship before 0.1.0; it means the
capability must not be implied by a broad format name without an explicit
support boundary.

#### JPEG

Current strength:

- baseline and progressive Huffman decode and encode;
- grayscale, RGB, and CMYK decode;
- exact manifest-backed quality, optimization, restart, progressive, EXIF,
  and 4:4:4/4:2:2/4:2:0 output cases; and
- no native runtime dependency.

Minute gaps:

| ID | Finding | Next slice |
| --- | --- | --- |
| JPG-001 | Encoder rejects decoded `Cmyk8` even though the decoder produces it and Pillow saves CMYK JPEG. | Add decode-CMYK→encode-CMYK fixtures and preserve Adobe/JFIF color interpretation. |
| JPG-002 | Pillow accepts bilevel and YCbCr source modes; Rust has neither private bilevel normalization nor a YCbCr input mode. | Add one mode at a time with exact marker, component, and pixel references. |
| JPG-004 | Public options omit Pillow 12.2.0 surfaces including quantization tables, ICC, DPI, comments, `keep_rgb`, separate restart-block/row controls, and stream type. | Inventory pinned `JpegImagePlugin._save`; accept only independently fixture-backed options in a typed JPEG options struct. |
| JPG-006 | Legal JPEG classes beyond the manifest—lossless processes, arithmetic coding, uncommon sampling/component layouts, and 12-bit data—have no support statement. | Classify each as supported, Pillow-rejected, or explicit `Unsupported` using pinned libjpeg/Pillow and upstream corpora. |
| JPG-007 | The README example now rejects every non-`Rgb8` source explicitly, but a complete target-mode table is still absent from rustdoc. | Add a fixture-derived direct-mode table and link it from `encode` documentation. |
| JPG-003 | Source color interpretation is implicit: JFIF, Adobe APP14 transforms, CMYK/YCCK, component IDs, and source/output colorspace are not one retained contract. | Reverse-map Pillow and libjpeg-turbo cases, then preserve source interpretation separately from normalized output mode. |
| JPG-008 | Decode cannot select luma, RGB, BGR, CMYK, YCbCr, or YCCK output even when avoiding a conversion would help the caller or encoder. | Add only codec-native output layouts justified by exact fixtures; keep RGB/luma defaults Pillow-compatible. |
| JPG-009 | Progressive JPEG is decoded as one completed raster; scans and rows cannot be consumed incrementally. | Add scan/row incremental decoding only after API-023/024 limits and destination buffers exist. |
| JPG-010 | Restart recovery, DNL, truncated final scans, fill bytes, and bytes after EOI lack an explicit strictness matrix. | Generate one minimized file per parser decision and classify Pillow-compatible acceptance, warning, or structured failure. |
| JPG-011 | MPO and other multi-picture JPEG containers are neither detected as a sequence nor explicitly rejected as a distinct capability. | Determine Pillow's selected-frame and iteration behavior, then classify still-first and sequence operations separately. |
| JPG-012 | Marker fragmentation and ordering are retained in raw stream order through the APPn/COM metadata records, but exhaustive multi-segment ICC, EXIF, XMP, comments, density, and Adobe collision fixtures are still missing. | Add exact ordered-marker fixtures and collision rules before metadata round-tripping. |
| JPG-013 | Gain-map/UltraHDR JPEG is currently ordinary JPEG with unrepresented auxiliary semantics. | Keep it a P3 container-extension candidate until the common auxiliary-image and color metadata model exists. |
| JPG-014 | Typed custom quantization/Huffman tables, restart units, progressive scan scripts, and arbitrary application segments are absent. | Add each control only when Pillow or a pinned primary encoder supplies a deterministic oracle and validation rules. |
| JPG-015 | Original sample precision, quantization-table precision, component sampling factors, table selectors, and scan structure are not returned by inspection. Two files that decode to the same `Rgb8` buffer can have materially different re-encode requirements. | Add a source JPEG descriptor and exact marker/table fixtures without exposing a generic image-processing model. |
| JPG-016 | Abbreviated JPEG datastreams that rely on externally supplied quantization or Huffman tables are neither recognized as a separate class nor rejected with a specific capability reason. | Decide whether they are explicit-format-only input or always `Unsupported`; never inherit tables from ambient/global state. |
| JPG-017 | 4:4:0, 4:1:1, 4:1:0, asymmetric sampling, nonstandard component order/IDs, and more-than-three color components lack a decoded-output and support matrix. | Generate minimal SOF/SOS cases through libjpeg-turbo and preserve the exact Pillow outcome plus source sampling descriptor. |
| JPG-018 | The compatibility key `restart_interval` does not state whether its unit is MCUs, MCU rows, or restart blocks; Pillow exposes separate restart-block and restart-row controls. | Replace it with typed units and reject simultaneous/conflicting settings before adding more fixtures. |
| JPG-019 | Decoder output parity can change with IDCT method, chroma upsampling, SIMD path, compiler floating-point behavior, and unusual 4:4:0 handling even when the JPEG is legal. | Name the exact reconstruction policy and compare scalar/native/WASM outputs on boundary coefficient and subsampling fixtures. |
| JPG-020 | Decoded coefficient blocks and lossless JPEG-to-JPEG marker/table rewrites are unavailable. A caller must fully reconstruct pixels and incur another lossy generation. | Keep coefficient-domain access P3, but classify it as codec work rather than promising lossless same-format output from ordinary `encode`. |
| JPG-021 | Memory/work limits do not distinguish progressive scan count, coefficient storage, marker bytes, MCU count, or restart recovery work. Width/height limits alone would not bound these paths. | Add JPEG sublimits and one minimized boundary fixture per independent work dimension. |

#### PNG and APNG

Current strength:

- packed bilevel, indexed palette with alpha table, 8-bit gray/gray-alpha/RGB/
  RGBA, 16-bit gray, Adam7 decode, internal zlib/DEFLATE, ancillary output
  fixtures, exact encoded-byte parity for retained still cases, and APNG
  sequence decode with retained controls and exact rendered canvases.
- a minimized, manifest-proven PNG recovery matrix: construction-critical
  pre-`IDAT` CRCs remain fatal, Pillow-deferred `IDAT`/`IEND` and post-`IDAT`
  CRCs are reported as Rust-only diagnostics, and ordering, palette-shape, and
  APNG declaration recoveries remain distinct from fatal verification.

Accepted APNG sequence-decode slice:

- The normative structure and rendering rules come from
  [PNG Third Edition sections 4.9 and 11.3.6](https://www.w3.org/TR/png-3/#4Concepts.APNG).
  Pillow 12.2.0 supplies the rendered-canvas pixel oracle and its observable
  seek/error behavior; direct chunk parsing supplies exact rational timing,
  source rectangles, sequence numbers, and control values that Pillow exposes
  only after normalization.
- `acTL` is recognized only before the first `IDAT` and must be at least eight
  bytes for Pillow compatibility. A zero/out-of-range count or duplicate
  control makes Pillow abandon animation and use the static PNG when possible;
  a control after `IDAT` is likewise not an animation declaration. These
  fallback cases remain still sequences. For a retained animation, the
  declared count must equal the number of `fcTL` chunks; missing controlled
  frame data and declared/actual disagreement are `Malformed`.
- `fcTL` and `fdAT` share one zero-based sequence. The first number is zero and
  every later number is exactly one greater; a gap, duplicate, reversal,
  truncated control, or `fdAT` without a current controlled frame is
  `Malformed`. Every frame has at least one `IDAT`/`fdAT` payload, although an
  individual `fdAT` frame-data field may be empty.
- Frame width and height are non-zero and the checked
  `offset + dimension` rectangle fits the IHDR canvas. A control for the static
  image before `IDAT` additionally covers the full canvas at offset zero.
- The source duration retains the 16-bit numerator and effective denominator
  exactly; encoded denominator zero becomes 100, without millisecond
  conversion or fraction reduction. Disposal 0/1/2 maps to
  keep/background/previous and blend 0/1 maps to source/over. Other values are
  retained as format-qualified reserved values; Pillow renders reserved
  disposal as keep and reserved blend as source. First-frame previous is
  rendered as background as required without erasing the retained source
  value.
- Every accepted frame reuses the IHDR depth, color, compression, filtering,
  interlace, palette, and pre-IDAT properties. Each independent zlib stream is
  decoded through the same still-PNG scanline path, including Adam7.
- Returned frame pixels are complete rendered canvases, matching Pillow bytes.
  Source rectangles and controls remain attached to those pixels. An included
  default frame starts from transparent black; an excluded default image uses
  the pinned Pillow compatibility seed described below. Source replaces the
  rectangle, over uses straight-alpha PNG compositing, and disposal is applied
  only after the displayed canvas is captured.
- If `fcTL` precedes `IDAT`, the static image is animation frame zero and is
  marked `is_default_image`. If `IDAT` precedes the first `fcTL`, the static
  image is retained as Pillow's seekable compatibility entry, marked
  `is_default_image`, and does not consume the animation count declared by
  `acTL`; `loop_count` applies only to the controlled animation. Pillow 12.2.0
  nevertheless seeds its first seek-rendered animation canvas with that static
  image, contrary to PNG Third Edition's transparent-black playback start.
  Exact returned canvases retain this pinned Pillow behavior while the flag and
  source rectangles keep the semantic distinction visible.
- The first fixture family covers RGB full-canvas source frames; RGBA
  subrectangles, source/over, all disposal values, finite/infinite loop counts,
  exact and zero-denominator delays, and a static image excluded from the
  animation. Separate exact-pixel fixtures cover bilevel, grayscale,
  grayscale-alpha, indexed `tRNS`, and an APNG-controlled Adam7 `IDAT`.
  Structural fixtures cover every rule above. Pillow 12.2 cannot materialize
  an Adam7 `fdAT` frame, so that exact combination is not claimed from oracle
  evidence: the retained proof combines the controlled-IDAT Adam7 row with
  independent non-interlaced `fdAT` extraction rows.
- `decode` continues to return the PNG static/default image exactly as Pillow
  does. The root `decode_sequence` operation and the owned-source
  `EncodedImage::decode_sequence` path materialize the complete animation;
  APNG encode remains a separate future slice.

Minute gaps:

| ID | Finding | Next slice |
| --- | --- | --- |
| PNG-003 | Non-indexed `tRNS`, ICC, EXIF, gamma/chromaticity, text variants, physical dimensions, time, and newer color/HDR chunks are not represented on decode. | Route raw chunks into the metadata model; do not add color management or orientation application. |
| PNG-004 | Pillow 12.2.0 still accepts `I` source save, with a deprecation warning for Pillow 13. | Freeze the pinned behavior in a fixture and decide whether compatibility outweighs a soon-removed oracle path. |
| PNG-005 | Encoder always emits non-Adam7 PNG because pinned Pillow ignores the tested interlace option. | Keep behavior, but document this as oracle compatibility rather than general PNG encoder capability. |
| PNG-006 | Inspection scans through pre-IDAT chunks and validates selected CRCs; it is not a fixed 33-byte metadata read. | Document complexity and add limits before advertising cheap inspection on arbitrary inputs. |
| PNG-008 | Text and ICC payloads are not decompressed or parsed, and chunk count and per-chunk size limits are still absent. The total ancillary extent, including retained opaque blocks, is bounded by `max_metadata_bytes` when a caller sets it. | Add explicit compressed-metadata and chunk-count limits under API-023 before semantic parsing. |
| PNG-010 | `sBIT`, `cICP`, `mDCV`, `cLLI`, `iCCP`, `sRGB`, `gAMA`, and `cHRM` precedence is not represented. | Preserve exact fields first and publish a precedence statement without performing color conversion. |
| PNG-011 | Direct 16-bit LA/RGB/RGBA encode modes are absent even though the decoder observes those source depths. | Add one mode at a time with big-endian sample fixtures and exact Pillow/reference output. |
| PNG-012 | Decode remains whole-buffer, and PNG still encoding still prepares filtered rows and compressed output before delivery. The PNG still and one-frame sequence sink paths now stream the validated signature and chunk structures through multiple `OutputSink` writes, but row/pass APIs, Adam7, and compressed-output generation remain non-streaming. | Layer row/pass and compressed-output APIs over shared codec state after limits and transfer layouts settle; extend structural delivery to the remaining PNG sequence/codec cases only with explicit partial-output and cancellation semantics. |
| PNG-013 | Extra compressed streams, split IDAT edge cases, and zlib trailing data lack a named policy; bytes after IEND are covered by the resolved trailing-input contract. | Add consumed/trailing-byte fixtures for the remaining edge cases under the documented trailing policy. |
| PNG-015 | `ImagePalette` cannot distinguish an indexed PLTE from the optional suggested PLTE allowed for truecolor PNG, and it cannot represent `sPLT` palettes with more than 256 entries. | Keep decoded index palettes in the pixel model and retain suggested palettes only as typed/opaque metadata. |
| PNG-016 | The metadata backlog does not enumerate `bKGD`, `hIST`, `sPLT`, `oFFs`, `pCAL`, `sCAL`, `tIME`, text language/translated-keyword fields, and the exact placement rules for `eXIf`. | Add a chunk-property ledger and preserve raw ordered bytes before interpreting any of these values. |
| PNG-017 | Decode transformation policy is implicit. Packing expansion, `tRNS` expansion, alpha stripping/addition, 16-bit stripping/scaling, byte swapping, and `sBIT` normalization cannot be selected or discovered. | Define only transfer transformations needed by byte-preserving codec use; default output remains pinned to Pillow. |
| PNG-018 | Filter method/type validation and Adam7 pass reconstruction are covered by aggregate pixel results, but there is no property map for every filter on first/middle/last rows at each bytes-per-pixel class or for every empty Adam7 pass. | Add generated minimal witnesses and record the filter/pass property, not merely another PNG row. |
| PNG-019 | Encoder controls do not type adaptive versus fixed filters, DEFLATE strategy/window choices, IDAT chunk sizing, or the interaction of `optimize`, level, type, and dictionary. | Inventory Pillow's exact behavior; reject a preset dictionary if it would produce a nonconforming standard PNG stream. |
| PNG-020 | A stream can end after enough bytes to display a frame but before trailing chunks and IEND are validated. The API has no separate “frame available” versus “datastream finished” state. | Make incremental frame success provisional until an explicit finish operation validates the remainder. |

#### GIF

Current strength:

- retained local frame rectangles, palettes, transparency, timing, disposal,
  interlace, loop/background data, LZW, quantization, compositing, and
  multi-frame encode.

Minute gaps:

| ID | Finding | Next slice |
| --- | --- | --- |
| GIF-002 | Pillow accepts `1`, `LA`, `I`, `F`, and `I;16` source saves through private conversion; Rust rejects them. | Add codec-local conversions only where exact output is required. |
| GIF-005 | Exact rational storage is implemented, but the public encode contract for a valid duration that is not an exact centisecond has only defensive-model coverage. | Add a caller-built manifest transform proving the structured `Unsupported` result and document that GIF encoding never rounds timing. |
| GIF-006 | Quantization behavior is exact only for retained images, not the full source-mode/color distribution space. | Add small reverse-mapped palette boundary fixtures before optimizing quantizer performance. |
| GIF-007 | The GIF user-input flag has no field in `DecodedFrame`, so a valid control-extension bit is silently lost. | Add a frame presentation flag with exact Pillow/spec reference evidence. |
| GIF-009 | Frame iteration is eager and allocates one owned raster per frame; callers cannot reuse one output buffer. | Add a bounded streaming iterator after API-027 and prove disposal/compositing state across partial iteration. |
| GIF-010 | Extension order, multiple comments/application blocks, sub-block boundaries, and unknown extension payloads cannot round-trip. | Preserve ordered raw extensions with exact limits and collision rules. |
| GIF-011 | There is no frame-count, cumulative pixel, extension-byte, LZW-work, or total sequence-memory limit. | Add one fixture per limit and distinguish limit exhaustion from malformed LZW. |
| GIF-012 | Quantizer, dither, palette reuse, transparency index, disposal optimization, and interlace choices are not typed controls. | Mirror only deterministic Pillow 12.2.0 choices that can be isolated into exact fixtures. |
| GIF-013 | Zero and missing delays now retain exact rational fields, but absent loop metadata, finite repeats, and infinite repeats still share an underspecified cross-format loop model. | Complete the API-050 loop-semantics contract before adding another animation encoder. |
| GIF-014 | Header version (`87a`/`89a`), logical-screen color resolution, sort flag, pixel aspect ratio, and global-table size bits are parsed only as needed or discarded. | Add a source stream descriptor and prove whether each field is retained, normalized, or intentionally ignored. |
| GIF-015 | Plain Text Extensions are rendering blocks and can be associated with a Graphic Control Extension, but the decoder treats extension data only as ignorable/opaque structure. | Classify plain-text presentation as unsupported retained metadata; do not rasterize text in this crate. |
| GIF-016 | NETSCAPE2.0, ANIMEXTS1.0, repeated loop extensions, malformed sub-blocks, and conflicts between application extensions lack a precedence/round-trip policy. | Preserve ordered application blocks and separately derive one normalized loop value with an explicit conflict diagnostic. |
| GIF-017 | Global versus local palette scope is not explicit in the common sequence. A background index names the logical-screen global table even when frames use unrelated local tables. | Add palette ownership identifiers before generic background or per-frame palette re-encoding. |
| GIF-018 | LZW evidence needs a property matrix for minimum-code-size anomalies, clear at every code width, early/late width growth, dictionary saturation, repeated clear, missing EOI, sub-block termination, and pixels beyond the frame rectangle. | Keep one minimized Pillow-observed case per state transition and one defensive case where Pillow cannot expose the branch. |
| GIF-019 | Pillow's frame optimizer may crop deltas, coalesce identical frames, accumulate their duration, select transparency fills, and change local/global palette use. The public options do not say whether exact source frames or optimized presentation is requested. | Add a typed frame-optimization policy and assert frame count, rectangles, duration, palette scope, and exact bytes. |
| GIF-020 | A second GIF stream, multiple trailers, and an extension after the trailer have no consumed-input policy; a single trailing payload is covered by the resolved trailing-input contract. | Add concatenated-stream fixtures under the documented trailing policy. |
| GIF-021 | Layout-specific evidence now distinguishes GIF source rectangles from Pillow's rendered presentation, but exact raw source-rectangle sample bytes are not independently asserted. | Add a format-structural raw-frame oracle before claiming byte-exact source-frame reconstruction. |

#### BMP and DIB

Current strength:

- multiple header generations, 1/4/8/16/24/32-bit decode, palettes, bitfields,
  RLE, top-down rows, ICO DIB reuse, and Pillow-compatible uncompressed encode
  for the core modes.

Minute gaps:

| ID | Finding | Next slice |
| --- | --- | --- |
| BMP-001 | `ImageFormat::Bmp` detects only `BM` files; Pillow also exposes headerless DIB as a related format and `.dib` alias. | Decide whether DIB belongs under BMP or remains explicitly out of scope; do not silently treat extension support as byte detection. |
| BMP-002 | DPI, color masks/profile data, palette metadata, and header variant identity are not exposed. | Preserve fields only when a caller can round-trip them without image processing. |
| BMP-003 | Alpha interpretation varies across BMP headers and Pillow's `USE_RAW_ALPHA` behavior; current evidence covers only the retained manifest cases. | Add explicit alpha-policy fixtures for 32-bit BI_RGB and bitfield variants. |
| BMP-004 | Encoder deliberately ignores compression/top-down/header requests to match Pillow cases. | Keep the exact behavior, but make ignored options discoverable and target-specific rather than accepting arbitrary keys. |
| BMP-005 | No independent upstream corpus is tracked for OS/2, embedded profiles, and rare bitfield layouts. | Import only small licensed cases with checksums and Pillow outcomes. |
| BMP-006 | `BI_JPEG` and `BI_PNG` embedded payloads are not classified, recursively limited, or exposed as delegated container data. | Decide explicit `Unsupported` versus bounded delegation; never recurse without depth and cumulative-byte limits. |
| BMP-007 | Decode always creates tightly packed output and cannot target a caller row stride, despite BMP's padded and bottom-up row organization. | Use the common transfer-layout/destination API; do not add cropping or other processing. |
| BMP-008 | V4/V5 endpoints, gamma, color-space type, profile offset/size, and rendering intent are discarded. | Retain exact header/profile data under API-034/040 before semantic interpretation. |
| BMP-009 | RLE delta moves, absolute-run padding, early EOL/EOB, top-down restrictions, and trailing compressed bytes need a strictness matrix. | Reverse-map each branch to a minimal Pillow fixture and retain defensive cases separately. |
| BMP-010 | ICO DIB XOR alpha, AND masks, 32-bit source alpha, and palette transparency interact in under-specified ways. | Add cross-product fixtures shared by BMP and ICO before changing either decoder. |
| BMP-011 | Headerless DIB has no explicit input/output type even though it is a distinct useful byte contract. | Keep it out of `detect_format` and the signature-validated common explicit-format API unless an unambiguous DIB contract is added; consider a separate DIB API. |
| BMP-012 | Related OS/2 bitmap-array/icon/pointer signatures (`BA`, `CI`, `CP`, `IC`, `PT`) and Windows `BM` are not one interchangeable format, but the support boundary is not documented. | Keep automatic BMP detection at `BM`; explicitly classify the related signatures and require a separate container decision before adding any. |
| BMP-013 | Header sizes 12, 16, 40, 52, 56, 64, 108, and 124 have overlapping but non-identical field layouts. Header identity and unsupported intermediate variants are not exposed. | Build a header-generation matrix and return a typed variant from inspection. |
| BMP-014 | Bitfield masks lack an explicit validity contract for zero, overlap, non-contiguous bits, bits outside depth, alpha overlap, and default masks when absent. | Reverse-map Pillow acceptance and retain normalized channel extraction separately from exact source masks. |
| BMP-015 | File size, pixel offset, DIB size, palette/mask/profile ranges, `SizeImage`, and actual payload length can disagree or overlap. Current cases do not form one precedence and trailing-byte policy. | Generate structural combinations without large rasters and attach byte-range context to failures. |
| BMP-016 | Palette entries are RGB triples for core headers and RGB quads for later headers; `ClrUsed`, `ClrImportant`, implicit table size, and gap bytes can disagree. | Preserve table encoding and declared counts separately from the decoded RGB palette. |
| BMP-017 | OS/2 Huffman 1D, RLE24, embedded JPEG, and embedded PNG compression codes are not individually classified. | Give each a stable `Unsupported` capability code; bounded delegated decode requires an explicit recursion budget. |
| BMP-018 | A V5 linked profile contains a Windows-encoded filename, while an embedded profile contains bytes. Treating both as one ICC blob would be wrong and could invite unintended I/O. | Retain linked profile names as opaque metadata only; this crate must never open them. |
| BMP-019 | Negative height is legal only for selected uncompressed/bitfield layouts, while core headers use unsigned dimensions. Negative width, minimum signed values, and top-down RLE need exact rejection rules. | Add checked structural fixtures around every signed boundary. |
| BMP-020 | Encoder output fixes one header generation and cannot state whether source alpha, masks, color-space fields, or profile data are preserved versus normalized away. | Publish one exact output-profile descriptor per BMP encode path before adding alternative headers. |

#### TIFF

Current strength:

- classic TIFF strips and tiles, planar/chunky layouts, selected integer/float/
  palette/CMYK/YCbCr modes, LZW/DEFLATE/PackBits decode, predictors, and a
  single-strip classic encoder;
- ordered main-chain multipage decode with per-page dimensions/mode and exact
  pixels; and
- byte-exact multipage raw/LZW/Deflate/PackBits encode from already-decoded
  pages, including mixed dimensions and modes.

Accepted TIFF multipage exploration:

- A classic TIFF page is one IFD in the main next-directory chain. Sequence
  decode must visit each unique IFD in order, stop a repeated offset without
  duplicating a page, and return the later page's structured failure rather
  than silently retaining only page one.
- The common sequence representation treats each page as a source rectangle
  at `(0,0)` with its own exact dimensions and mode. The sequence canvas is the
  maximum page extent. Duration is zero; disposal, blend, loop, and background
  are unspecified because TIFF pages are not animation presentation controls.
- Still decode remains page one. Multipage decode must reuse the same
  directory decoder so still and sequence results cannot diverge for the first
  page.
- Multipage encode accepts already-decoded pages without resizing or color
  conversion. Every page must have zero duration, origin `(0,0)`, a rectangle
  matching its own image, and no animation-only controls. Per-page dimensions
  and modes are retained; unsupported page modes fail with the same direct-mode
  contract as still encode.
- `scripts/explore_tiff_sequence.py` must first map Pillow 12.2.0/libtiff 4.7.1
  output ordering for raw, LZW, Deflate, and PackBits pages, including different
  page dimensions and modes. The implementation may share the still encoder
  only where the emitted IFD entries, payload placement, padding, and next-IFD
  links are proved byte-identical.
- The completed probe is deterministic in all five cases. Pillow concatenates
  the corresponding still TIFF streams at 16-byte boundaries, retains each
  embedded header, relocates external value and strip offsets by the page base,
  links the prior IFD to the relocated next IFD, and pads the final stream to
  the same boundary. The reconstructed raw, LZW, Deflate, PackBits, and
  mixed-size/mixed-mode streams match every Pillow byte.
- Success fixtures compare the complete TIFF byte stream, ordered page
  dimensions/modes, and every page byte. Error fixtures independently cover a
  malformed later IFD, unsupported page modes, geometry, timing, presentation
  controls, loop/background, and classic-offset/output limits.

Minute gaps:

| ID | Finding | Next slice |
| --- | --- | --- |
| TIF-002 | BigTIFF signatures are detected, but complete decode/encode capability is not provided. | Add explicit BigTIFF success/error fixtures and capability reporting. |
| TIF-003 | `P8` and YCbCr inputs accepted by Pillow cannot be encoded from the current model. | Add palette TIFF first; add a YCbCr transfer mode only with exact byte requirements. |
| TIF-005 | Compressed output writes dimensions as TIFF SHORT and therefore imposes an artificial 65,535 ceiling where LONG is legal. | Reverse-map Pillow output for larger dimensions and select field types without allocating impossible fixtures. |
| TIF-006 | Each encoded page is single-strip, chunky, classic TIFF only; there is no tiled, planar, BigTIFF, palette, JPEG/Fax/Zstd/LZMA/WebP compression output. | Add only formats Pillow 12.2.0 actually exercises and the dependency-free implementation can support on WASM. |
| TIF-007 | Orientation, resolution, ICC/EXIF/GPS, arbitrary tags, extra-sample association, and sub-IFDs are not retained. | Design opaque IFD/tag retention with collision rules; never apply orientation in this crate. |
| TIF-008 | Horizontal predictor handling for signed/float samples and unusual bit depths lacks an explicit support matrix. | Use libtiff/Pillow fixtures for predictor × sample-format × endianness before generalizing. |
| TIF-009 | Frame counting scans IFD chains and can return `is_animated=true` with unknown `frame_count`; ordinary still decode returns page one while sequence decode attempts the later IFD and returns its structured failure. | Document the incomplete-count state and bind it to limits/capabilities. |
| TIF-010 | Arbitrary sample counts, mixed bit depths, and non-RGB extra channels cannot be represented by `ImageMode`. | Define a TIFF-native transfer layout only if opaque extra samples must survive; do not force them into RGBA. |
| TIF-011 | Associated and unassociated alpha (`ExtraSamples`) are not distinguished in decoded output. | Add source alpha semantics and prove whether Pillow unpremultiplies, preserves, or drops each case. |
| TIF-012 | Fill order, photometric inversion, sample format, and per-channel depth are only partially visible after normalization; source byte order is now retained. | Expand `SourceDescriptor` one independently proved field at a time while leaving decoded transfer bytes unchanged. |
| TIF-013 | There is no bounded page iterator, page selection, or random-access IFD handle. | Implement API-027 with IFD-cycle detection and preserve per-page dimensions/mode/metadata. |
| TIF-014 | SubIFDs, pyramids, thumbnails, masks, and directory graphs are flattened or ignored. | Model relationships only when a fixture needs them; distinguish primary pages from auxiliary directories. |
| TIF-016 | Strip/tile decode cannot stream into a caller buffer, and encoding cannot incrementally write strips or tiles. | Add bounded chunk APIs after API-024/025; never require full multipage materialization. |
| TIF-017 | IFD cycles/depth, tag counts, strip/tile counts, offset arrays, decompressed bytes, and predictor work have no caller policy. | Add typed TIFF sublimits and minimized cycle/overflow/exhaustion fixtures. |
| TIF-018 | Sparse 64-bit offsets, BigTIFF count/offset boundaries, and host `usize` conversion are not exercised across 32-bit/WASM targets. | Use generated sparse/structural inputs and target-specific checked arithmetic tests without committing huge files. |
| TIF-020 | Photometric support is not catalogued for WhiteIsZero, BlackIsZero, RGB, Palette, Transparency Mask, Separated, YCbCr, CIELAB/ICCLAB/ITULAB, LogL/LogLuv, and CFA classes. | Generate a source photometric capability table; do not coerce unknown extra channels into RGBA. |
| TIF-021 | Compression identity is broader than “compressed”: CCITT variants/options, old/new JPEG, LZW, Deflate/Adobe Deflate, PackBits, PixarLog, SGILog, LZMA, Zstd, LERC, WebP, and vendor values need separate decode/encode capabilities. | Give every observed compression a stable capability code and prevent feature-gated delegated codecs from being inferred accidentally. |
| TIF-022 | The common metadata model cannot retain TIFF field type and count. BYTE/ASCII/SHORT/LONG/RATIONAL, signed variants, FLOAT/DOUBLE, IFD, LONG8/SLONG8/IFD8, inline values, and offset values can carry byte-distinct but numerically similar data. | Add a typed raw tag record with source byte order, exact count, and exact bytes. |
| TIF-023 | YCbCr decode behavior also depends on coefficients, subsampling, positioning, and ReferenceBlackWhite, not only `PhotometricInterpretation`. | Add a complete YCbCr tag cross-product before exposing a YCbCr transfer mode or claiming exact RGB reconstruction. |
| TIF-024 | Old-style JPEG-in-TIFF, new JPEG compression, shared `JPEGTables`, per-strip tables, restart boundaries, and abbreviated JPEG streams are not one path. | Treat embedded JPEG state as a bounded TIFF-owned codec contract with cumulative limits and independent fixtures. |
| TIF-025 | Strips/tiles can overlap, alias, leave gaps, appear out of order, extend past input, or disagree with dimensions and byte counts. The acceptance policy is not explicit. | Add structural offset/count fixtures and define whether identical aliased ranges are accepted, copied once, or rejected. |
| TIF-026 | Missing/zero `StripByteCounts` or `TileByteCounts`, one-strip inference, and last-strip truncation have implementation-specific recovery rules. | Reverse-map Pillow/libtiff behavior and emit a diagnostic when inference preserves a usable image. |
| TIF-027 | Floating-point predictor 3 has byte-plane shuffling semantics distinct from horizontal predictor 2 and depends on sample width and byte order. | Add exact 16/24/32/64-bit float predictor vectors before advertising float-predictor support. |
| TIF-028 | Primary pages, reduced images, masks, SubIFDs, EXIF/GPS IFDs, thumbnails, pyramids, and arbitrary directory links need relationship types, not one flat frame list. | Introduce directory roles and bounded graph traversal before exposing more than the primary IFD chain. |
| TIF-029 | Multipage decode retains each page's dimensions, mode, palette, exact bytes, and source byte order, but not its photometric interpretation, sample type, metadata, compression, or page-indexed error stage. | Extend the per-page source descriptor and add a stable page index/stage to later-page failures. |
| TIF-030 | Edge tiles and final strips have stored padding versus visible dimensions; a caller-buffer API needs to say whether padding bytes are consumed, returned, zeroed, or ignored. | Make visible and storage extents explicit in the chunk-layout contract. |

#### WebP

Current strength:

- in-tree VP8/VP8L decode, lossy and lossless still encode, alpha, extended
  container metadata output, composited animation decode, and full-canvas
  keyframe sequence encode.

Implemented WebP sequence-encode contract:

- The [WebP container specification](https://developers.google.com/speed/webp/docs/riff_container)
  governs RIFF sizes/padding, `VP8X`, `ANIM`, `ANMF`, 24-bit canvas/rectangle/
  duration fields, even-coordinate offsets, background byte order, loop
  bounds, and the legal nested `ALPH`+`VP8` or `VP8L` frame forms. Pillow
  12.2.0 with pinned libwebp 1.6.0 is the exact byte oracle.
- The implementation is intentionally a full-canvas keyframe
  encoder. A rendered-canvas frame already provides every output pixel, even
  when its retained source-history rectangle was smaller; a
  source-rectangle frame must equal the entire canvas. That makes every output
  `ANMF` a source-replace, keep frame and guarantees presentation pixels
  without reverse-engineering a lossy source rectangle from an already
  composited canvas.
- Retained input disposal/blend rectangles describe how an existing source
  produced its rendered canvas; they are not silently claimed as output
  controls. Full-canvas keyframes make prior disposal irrelevant and replace
  the complete canvas. Default-image identity, interlace, reserved controls,
  subrectangles, and non-canvas source pixels remain `Unsupported` until an
  exact mapping exists.
- Frame durations must convert exactly to integral milliseconds and fit
  unsigned 24-bit storage. Loop count must fit unsigned 16-bit storage.
  `AnimationBackground::Rgba` maps to `ANIM` BGRA bytes; no background maps to
  transparent black, and a palette-index background is not a WebP value.
- Each frame passes through the existing validated WebP still pixel
  preparation and VP8/VP8L encoder. Pillow `kmax=1` full keyframes contain
  those exact nested bitstreams for opaque and alpha, lossy and lossless
  inputs. The initial public behavior is therefore
  keyframe-only; `minimize_size`, `kmin`/general `kmax`, mixed compression,
  and frame differencing remain WEP-014 follow-ups.
- `scripts/explore_webp_sequence_encode.py` proves that hypothesis against the
  pinned oracle for RGB/RGBA × lossy/lossless: all eight `ANMF` nested chunk
  sequences are byte-identical to the corresponding still output after
  removing the still-only `VP8X` wrapper. Pillow writes full `(0,0,9,7)`
  rectangles, exact 17/33 ms durations, and flag byte `0x02` (source replace,
  keep) for every forced keyframe.
- Five manifest success rows compare the entire encoded RIFF byte sequence and
  all 10 re-decoded sequence frames, not only the first still frame. Twenty-two
  dedicated error rows cover each rejected model/boundary independently.
  One-frame sequences remain byte-identical to `encode`.

Minute gaps:

| ID | Finding | Next slice |
| --- | --- | --- |
| WEP-003 | Animation decode now returns full rendered canvases while separately retaining exact ANMF rectangles, blend, and disposal. It still does not expose the raw nested frame bitstream needed for exact container reconstruction. | Add the bounded demux/source-frame view described by WEP-011 without changing rendered decode semantics. |
| WEP-004 | Pillow mode normalization is much broader than the current encoder. | Add `LA` first, then integer/float/16-bit/YCbCr only as fixture-backed private conversions. |
| WEP-007 | Encoder and decoder are among the largest source areas, but no native/WASM time, memory, output-size, or compiled-size benchmark exists. | Establish fixed lossy/lossless/alpha/animation benchmark sets before performance refactors. |
| WEP-001 | The API does not retain whether a source was VP8, VP8L, or extended WebP, nor expose intrinsic alpha/animation/container flags separately from normalized mode. | Add source encoding properties to inspection without leaking internal decoder state. |
| WEP-005 | Transparent RGB and straight-versus-premultiplied alpha behavior is not a named contract across lossy, lossless, and animation paths. | Add exact invisible-RGB and alpha-edge fixtures before any optimization changes. |
| WEP-008 | Near-lossless, alpha quality/filter, exact transparent RGB, presets, target size/PSNR, SNS, filtering, partitions, and sharp-YUV controls are untyped or absent. | Compare Pillow 12.2.0 and pinned libwebp 1.6.0; expose only deterministic options that the in-tree encoder implements. |
| WEP-009 | Decoder cannot output BGR(A), premultiplied layouts, or caller-provided YUV/RGB planes/buffers. | Add transfer layouts only when they avoid measured copies or enable exact codec-native handoff. |
| WEP-010 | No incremental VP8/VP8L decode accepts partial input or emits completed rows. | Add after common input limits and destination contracts; preserve the whole-slice wrapper. |
| WEP-011 | Callers cannot inspect raw ANMF rectangles/bitstreams or enumerate RIFF chunks without full compositing. | Define a bounded demux view separate from rendered sequence decode. |
| WEP-012 | Unknown RIFF chunks, duplicate metadata chunks, and chunk order are retained raw (padding normalized away), but declared-size mismatch (truncated chunks) has no strictness policy beyond skipping retention. | Build ordered-container fixtures and align malformed-size outcomes with API-040. |
| WEP-013 | Per-frame dimensions are bounded, but cumulative animation pixels, duration, frame count, metadata, and decode work have no caller limits. | Add WebP-specific sequence limits and overflow fixtures. |
| WEP-014 | The initial keyframe encoder supports exact per-frame durations, RGBA background, loop, and forced `kmax=1`, but rejects `minimize_size`, `kmin`, general `kmax`, `allow_mixed`, and alpha-quality/optimization controls. | Add each optimization only with exact sequence bytes, frame metadata, and invalid interaction fixtures. |
| WEP-015 | VP8X feature flags, reserved bits, canvas size, and actual ICCP/ALPH/EXIF/XMP/ANIM chunks can disagree. Current inspection does not expose a normalized-versus-declared consistency report. | Add one fixture per mismatch and preserve both declared flags and observed chunks. |
| WEP-016 | RIFF's 32-bit size, WebP's approximately 4-GiB container ceiling, 24-bit VP8X canvas fields, and the distinct VP8/VP8L dimension bounds are not preflighted through one public limit/capability contract. | Add checked per-subtype size preflight before allocating or encoding. |
| WEP-017 | ANMF offsets are stored in half-pixel units, rectangles must fit the canvas, duration is 24-bit milliseconds, reserved bits must be zero, and the payload must contain a legal ALPH+VP8 or VP8L frame form. | Build exact boundary fixtures and retain raw frame sub-bitstream identity in the demux view. |
| WEP-018 | The ALPH chunk has compression, filtering, preprocessing, and reserved fields; raw and VP8L-compressed alpha are only two outcomes of a broader header contract. | Add all legal filter/preprocessing values and illegal reserved combinations before claiming complete alpha decode. |
| WEP-019 | A partial WebP demux can know the canvas and N frames while the last frame remains incomplete. Whole-slice APIs collapse this into success or malformed without progress. | Reuse API-043/047 and expose a partial-frame state only in the future streaming demux. |
| WEP-020 | libwebp animation decode reports cumulative timestamps, while the common model stores individual durations. Rounding, zero-duration frames, overflow, and final timestamp must be proved separately. | Assert both exact source duration and checked cumulative presentation time. |
| WEP-021 | Lossy pixel output depends on fancy upsampling, filtering, dithering, cropping/scaling options, and premultiplied/output layout choices in common decoders. Only one implicit reconstruction policy is tested. | Freeze the Pillow-compatible reconstruction path, compare scalar/WASM behavior, and expose alternatives only when they are codec transfer choices rather than processing. |
| WEP-022 | VP8L aggregate fixtures do not publish a witness for each transform combination, meta-prefix group, color-cache boundary, simple/full Huffman tree form, distance mapping, and entropy-image dimension. | Add a property-to-fixture map and minimize first-divergence cases before performance refactors. |

#### ICO and CUR

Current strength:

- best-entry selection, PNG and DIB entry decode, AND-mask behavior, source-sized
  PNG- or BMP-backed single-entry output, and no hidden resize operation.

Minute gaps:

| ID | Finding | Next slice |
| --- | --- | --- |
| ICO-001 | Decoder exposes only the selected largest entry; callers cannot inspect/select/enumerate every stored size. | Add an entry-oriented decode/inspect API whose entries are already-sized images, not an image-processing resize request. |
| ICO-002 | Encoder cannot accept multiple already-sized caller entries. Pillow can save multiple entries and replacement images. | Add an ICO directory model and exact ordering/selection fixtures without generating resized pixels. |
| ICO-004 | Default PNG-backed path inherits PNG's accepted-mode limits; BMP-backed path accepts only RGB/RGBA. | Publish separate entry-backend mode capabilities and fixture every accepted combination. |
| ICO-005 | `ico` transitively enables full `png` and `bmp` features, so it is not an isolated compiled slice. | Retain correctness first; measure binary impact and consider private embedded-entry capability features only if Cargo's additive rules remain clear. |
| ICO-006 | Directory color count, planes, bit depth, duplicate sizes, and tie-breaking are only manifest-bounded. | Add entry-directory edge cases before claiming complete ICO container support. |
| ICO-007 | CUR hotspot retention covers only the selected entry; every unselected directory entry's hotspot remains inaccessible. | Store hotspot per enumerated entry under the entry-oriented model. |
| ICO-008 | Callers cannot inspect each entry's embedded format, stored dimensions, color count, planes/hotspot, bit depth, byte range, and source mode. | Add bounded directory metadata without decoding every payload. |
| ICO-009 | Entry selection is implicit. There is no exact index/size/bit-depth selection or way to distinguish a fallback from an exact match. | Define deterministic selection queries and retain the current default as documented convenience behavior. |
| ICO-010 | Duplicate sizes, malformed high-ranked entries, tie ordering, and fallback to a lower-ranked valid entry need explicit rules. | Use minimized multi-entry fixtures and assert both selected index and error behavior. |
| ICO-011 | Encoder cannot emit a mixed PNG/DIB multi-entry file from already-sized caller images. | Add a directory encoder with per-entry backend/options; never resize caller images. |
| ICO-012 | Directory width/height byte zero means 256, while embedded headers can disagree; zero/overflow and payload-range edges are not exhaustively asserted. | Add structural fixtures before exposing entry enumeration as stable API. |
| ICO-013 | A set-of-sizes view loses directory order, duplicate sizes, color-depth variants, selected index, and exact tie-break information. | Expose an ordered entry list; derive a convenience set only as a lossy query. |
| ICO-014 | Entry payload ranges can overlap, alias exactly, point into the directory, repeat one payload, or run past input. No policy states whether shared exact ranges are legal. | Add byte-range validation and fixtures for every overlap class before zero-copy entry views. |
| ICO-015 | Reserved word, resource type, count, per-entry reserved byte, planes/color-count, and CUR hotspot fields need independent validation. One invalid entry should not silently redefine the whole container. | Return directory-level versus entry-indexed errors and define whether valid lower-ranked entries remain selectable. |
| ICO-016 | Width/height byte zero, color-count zero, planes zero, bit-depth zero, and DIB/PNG-derived values are sentinels rather than ordinary numeric zero. | Retain declared and derived values separately and test every sentinel. |
| ICO-017 | DIB entry height represents XOR plus AND planes, mask rows have independent padding, and 32-bit alpha may suppress or combine with the AND mask. Truncated/mismatched halves need exact rules. | Add XOR-depth × source-alpha × AND-mask × row-padding fixtures shared with BMP-010. |
| ICO-018 | Directory dimensions/bit depth can disagree with embedded PNG IHDR or DIB headers. Current best-entry logic needs a documented trust/validation order. | Retain both declarations, reject impossible ranges, and assert the selected effective dimensions. |
| ICO-019 | The same two directory words mean planes/bit depth for ICO and x/y hotspot for CUR. A shared entry type must not expose both interpretations simultaneously. | Use a tagged ICO/CUR entry descriptor with exact raw words. |
| ICO-020 | Pillow sorts/deduplicates requested output sizes and can choose among appended images with equal size but different depth. A future multi-entry encoder must state ordering and duplicate policy rather than inheriting set semantics accidentally. | Preserve caller order by default; add an explicit compatibility policy only if exact Pillow bytes require sorting/deduplication. |
| ICO-021 | Selected-entry decode is all-or-nothing; callers cannot inspect a valid directory, skip one malformed entry, and decode another exact index with an entry-scoped error. | Separate bounded directory parse from entry payload decode and attach index/range/backend to errors. |

#### AVIF and AV1

Current strength:

- bounded ISO-BMFF inspection;
- fixed native libavif/dav1d/libaom parity for retained stills and sequences;
- exact metadata and option output rows;
- native still output delivery through validated top-level ISO-BMFF sink segments;
- an in-tree, fixture-bounded portable AV1 decoder subset; and
- explicit documentation of the current target boundary.

Minute gaps:

| ID | Finding | Next slice |
| --- | --- | --- |
| AVF-001 | Portable AV1 still decode is a closed subset; sequence decode and all encode are unavailable on WASM. | Complete the existing AVIF plan before treating the feature as target-invariant. |
| AVF-002 | Native AVIF requires C compiler, archiver, dynamic libavif, dav1d, and libaom despite the repository-wide no-dependency end goal. | Treat native support only as the oracle bridge to be removed, not a permanent exception. |
| AVF-003 | Native encode supports fewer Pillow source modes. | Add exact private normalization only after portable encode architecture is selected, so work is not duplicated around FFI. |
| AVF-004 | AVIF primary `prof`/`rICC` ICC profiles and recognized `Exif`/XMP item payloads are now retained on decode; EXIF remains raw and includes the stored AVIF TIFF-offset prefix. Direct alpha and supported grid-derived alpha items report `SourceAlpha::Auxiliary` plus source-local `auxl` relationships, bounded `prem` relationships retain the source's premultiplication declaration without changing normalized decoded samples, typed non-primary `colr`/`nclx` CICP declarations retain source-local item identity, and non-primary `prof`/`rICC` profiles retain exact raw bytes through `SourceDescriptor::avif_item_icc_profiles()`. Exact straight-versus-premultiplied sample semantics beyond that normalized boundary remain open. | Fold the remaining non-alpha/auxiliary metadata into API-019 and define any additional straight-versus-premultiplied decoded-sample semantics. |
| AVF-005 | Sequence encoder rejects offsets and requires every frame to match one canvas; loop semantics are not represented in AVIF output. | Compare pinned Pillow/libavif sequence behavior and state unsupported properties explicitly. |
| AVF-006 | AV1 support is validated by many narrow reverse-mapped fixtures, not the AV1 bitstream specification or conformance suite. | Continue slice-by-slice first-divergence work, then add licensed libavif/AOM corpus classes with independent references. |
| AVF-007 | `avif` on native and WASM is one Cargo feature with materially different operations. | Capability discovery and runtime WASM gates must make the difference machine-readable until eliminated. |
| AVF-008 | Portable transfer is 8-bit normalized output; 10/12-bit samples, monochrome, planar YUV, and high-depth alpha cannot be retained directly. | Add exact source descriptors and one transfer layout at a time after portable AV1 correctness. |
| AVF-009 | Primary-item CICP primaries/transfer/matrix and range, primary-item `av1C` chroma sample position, primary-item `clli` maxCLL/maxPALL, primary-item `mdcv` mastering-display fields, and primary-item `prof`/`rICC` ICC profile bytes now retain through `SourceColor`; typed non-primary/auxiliary `colr`/`nclx` CICP declarations retain source-local item IDs through `SourceDescriptor::avif_item_color_properties()` without merging into the primary color result, and non-primary/auxiliary `prof`/`rICC` profiles retain exact raw profile data through `SourceDescriptor::avif_item_icc_profiles()`. Other non-primary/auxiliary item-color forms remain absent. | Preserve the remaining exact item/property fields without applying tone or color transforms. |
| AVF-011 | Full grid topology/composition, layered/progressive images, sample transforms, and most alternative item relationships have no representation. The bounded primary-grid `dimg` child list, generic `iref` edges including filtered `prem` relationships, alpha relationships from `grid.avif` to its derived color items, and typed non-primary `colr`/`nclx` declarations are retained as source provenance, but tile placement and the grid graph are not exposed as a composable image model. | Classify each as decoded still, auxiliary structure, or explicit `Unsupported` with libavif fixtures and bounded graph traversal. |
| AVF-012 | Gain maps, auxiliary depth, thumbnails, and supplementary images cannot be enumerated or associated with the primary image. Direct alpha and the supported grid-derived alpha links are exposed as source-local item IDs through the scalar and plural `SourceDescriptor` getters, the supported primary grid exposes its ordered derived item IDs, bounded `iref` edges including `prem` are retained, and typed non-primary CICP declarations retain their item IDs; payload selection and broader auxiliary graphs remain inaccessible. | Extend the relationship model for non-alpha, derived, grid, and supplementary content only after fixture-backed use cases; never flatten it silently into RGBA. |
| AVF-013 | Sequence timing uses integer milliseconds and cannot retain exact timescale/duration, repetition, edit lists, or sample timing. | Replace timing through API-009 before claiming exact animated AVIF container parity. |
| AVF-014 | Item/property/reference counts, box depth/size, grid dimensions, sample count, cumulative decoded bytes, and AV1 work have no caller limits. | Add BMFF and AV1 sublimits with independently identified failure context. |
| AVF-015 | Portable encode lacks typed codec, speed, thread, tile, quantizer, chroma, range, tune, and lossless controls matching the native oracle bridge. | Freeze required Pillow/libavif behaviors, then implement only dependency-free, deterministic controls. |
| AVF-016 | Unknown top-level boxes and free/skip padding are retained raw in scan order, and recognized EXIF/XMP item payloads are retained in item order. Compatible brands, unknown item properties, and item/property graph limits are not yet governed by the common model. | Use ordered opaque preservation under API-040 for remaining item-level boxes/properties, with box-size and graph limits. |
| AVF-017 | Native and portable decoders have no common strictness/diagnostic contract, making graceful fallback and parity differences hard to classify. | Normalize error stage, offset/box/OBU identity, unsupported feature, warning, and limit states. |
| AVF-018 | AV1 film grain, operating points, spatial layers, scalability, and still-picture/profile constraints have no capability statement. | Inventory the portable syntax subset against the pinned AOM/libavif corpus before adding syntax. |
| AVF-019 | AVIF input/output cannot use caller-owned YUV/alpha planes or report precise plane allocation requirements. | Add checked plane preflight and ownership only after API-024/025. |
| AVF-020 | Portable encoded output has no independent compatibility lane through libavif and at least one browser decoder. | Require both before replacing the native encoder bridge; Pillow parity alone may reuse the same native stack. |
| AVF-022 | A file can contain both a primary image item and an image-sequence track. The common API does not expose source selection or report which source auto mode chose. | Add item/track capability and explicit source selection, retaining auto as a documented policy. |
| AVF-023 | Progressive/layered AVIF has layer count and partial-detail state, while incremental libavif can expose completed row count. Current decode returns only one terminal raster. | Classify progressive source versus timed sequence and define provisional output/finish semantics before portable support expands. |
| AVF-024 | Sequence random access depends on keyframes and a frame's maximal dependent byte extent. Eager full decode hides this and cannot support bounded range input. | Retain keyframe/dependency information in a source-bound sequence decoder and prove Nth-frame equivalence with sequential decode. |
| AVF-025 | Repetition can be finite additional repeats, infinite, or unknown; libavif defines finite `n` as `n + 1` total plays. The common loop field cannot preserve all states. | Implement API-050 with exact native/Pillow sequence fixtures. |
| AVF-026 | libavif metadata is valid after parse, but outer container properties may change after inner AV1 headers are decoded. The current inspect/decode contract has no “declared versus confirmed” distinction. | Retain both observations when they differ and fail only according to an explicit strictness rule. |
| AVF-027 | Main color, alpha, and gain-map content can be selected independently. A single normalized RGBA result cannot state which auxiliary content was decoded, ignored, or unavailable. | Add content-selection capability flags and auxiliary relationships before gain-map support; applying a gain map remains image processing and out of scope. |
| AVF-028 | Opaque and UUID item properties have association order, essential bits, transform order, a finite unique-property ceiling, and collision rules with properties generated by the encoder. | Preserve ordered raw properties and reject unsafe collisions; never replay unknown essential properties as if understood. |
| AVF-029 | EXIF storage carries a TIFF-header offset, and libavif may derive `irot`/`imir` from EXIF orientation. The model now retains raw EXIF and independent container transform provenance, but does not parse orientation or detect contradictions. | Preserve the independent fields and define contradiction/auxiliary-item policy; document that neither is applied to pixels. |
| AVF-030 | Decoder strict flags, diagnostic text, I/O byte statistics, ignored EXIF/XMP policy, image count/dimension/pixel limits, and waiting-on-I/O state are available in the native reference but have no portable/common mapping. | Define stable stage/limit/progress fields and use the same fixtures against native and portable implementations. |
| AVF-031 | AVIF auxiliary alpha now has `SourceAlpha::Auxiliary` plus bounded source-local auxiliary-item relationships on inspection, still decode, and sequence-frame decode, backed by both direct `alpha.avif` and grid-derived `grid.avif` fixtures. The primary grid also retains its ordered derived item IDs and bounded `dimg` references through `SourceDescriptor::avif_item_relationships()`. Source-local `prem` edges are retained through `SourceDescriptor::avif_premultiplied_relationships()`, typed non-primary `colr`/`nclx` CICP declarations through `SourceDescriptor::avif_item_color_properties()`, and non-primary `prof`/`rICC` profiles through `SourceDescriptor::avif_item_icc_profiles()`; the existing alpha fixture is mutated only in memory to add relationship/property witnesses, and decoded normalized bytes remain identical. Plane range/quality, non-alpha auxiliary payloads and color forms beyond typed CICP/ICC, full grid topology, and exact invisible RGB remain unrepresented. | Add exact plane/relationship fixtures for the remaining auxiliary classes before high-depth alpha; keep source provenance separate from decoded transfer bytes. |
| AVF-032 | `iloc` construction methods, multiple extents, data references, idat/mdat placement, non-sequential extents, and 64-bit range arithmetic are not a named corpus class. | Add structural extent fixtures with a cumulative byte/range limit and precise box/item context. |
| AVF-033 | Grid-derived images and gain maps may use different grids/dimensions from the primary image. The current grid fixture proves and retains the ordered derived-item list plus alpha-to-derived-item relationships, but flattening the grid into one canvas still loses tile placement/topology and partial-failure context. | Retain grid topology and validate every tile independently; composition stays private to decode. |
| AVF-034 | Fragmented sequence files, edit lists, sample groups, sample-description changes, sync-sample tables, and timestamp offsets have no capability statement. | Classify each BMFF track feature before claiming general animated AVIF support. |
| AVF-035 | Compatible brands, major brand, minor version, still/sequence brands, and AV1 codec configuration are not exposed together, so detection, inspection, and actual decoder capability can disagree. | Return a bounded FileTypeBox/source descriptor and generate the capability decision from it. |

### Cargo features, targets, artifacts, and downstream use

Cargo features are additive and unified across the dependency graph. The
official Cargo reference explicitly recommends that features be safe in any
union. That has several consequences for this crate.

| ID | Class | Finding | Attack and acceptance |
| --- | --- | --- | --- |
| FTR-001 | Feature granularity | One format feature includes inspect, decode, sequence code, and encode. A decode-only consumer still compiles that format's encoder. | Measure native/WASM binary sections first. If material, keep the existing format umbrella and add additive internal `*-decode`/`*-encode` features only with a simple supported migration. |
| FTR-002 | Transitive feature cost | `ico = ["bmp", "png"]` enables complete BMP and PNG public codecs, not just embedded-entry internals. | Keep until measurements justify refactoring; any split must still let `ico` work by itself and remain additive. |
| FTR-003 | Feature unification | A transitive dependency enabling `avif` enables its native build requirements for the entire resolved package. | Document prominently for library integrators and expose target capabilities; portable AVIF ultimately removes the surprise. |
| FTR-004 | Native build coupling | Enabling `avif` compiles the C bridge before the caller performs any operation; inspect-only native users still need C tools and libavif. | Portable implementation or additive operation features must make inspection truly Rust-only. |
| FTR-005 | Platform defect | `build.rs` recognizes Windows `.dll`/`.lib` candidates, but `link_library_file` supports only macOS, Linux, and Android and panics for Windows. CI runs only Ubuntu. | Add Windows-native build fixtures or declare native AVIF unsupported there until portable AVIF replaces it. |
| FTR-006 | Cross-compile fragility | The build script falls back to host `cc`/`ar` unless target-specific environment variables exist. | Add documented target-tool lookup tests for representative cross targets; fail with the exact missing tool variable. |
| FTR-007 | Library selection | `IMAGE_SLASH_STAR_AVIF_LIB_NAME` affects fallback linking, while directory scanning selects the first filename beginning `libavif`; multiple candidates can select unexpectedly. | Resolve the requested exact name/version deterministically and report the selected path. |
| FTR-008 | Runtime redistribution | Native builds copy or link a dynamic library into Cargo output and use build-directory rpaths. This does not define how a released downstream executable ships the pinned stack. | Add a release-binary relocation test or remove the native distribution path after portable parity. |
| FTR-009 | Target assurance | CI has no macOS, Windows, 32-bit runtime, or big-endian runtime lane; the WASM runtime lane covers `wasm32-wasip1` only. | Add the smallest target matrix that catches build-script, endian, pointer-width, and runtime differences. |
| FTR-010 | WASM package | There is no JS binding, package manifest, core/extra artifact, loader, or runtime test yet. | Keep this after native semantic parity; define copy behavior, errors, feature mapping, bundler targets, and reproducible compressed/uncompressed sizes. |
| FTR-011 | Package size semantics | Cargo feature selection does not shrink the downloaded `.crate`; it ships source for every codec and required legal texts. The locally produced `6f9c002` archive is 466,477 bytes compressed and 2,540 KiB unpacked. | Document download size separately from linked native/WASM size; optimize source packaging only after legal and reproducibility requirements. |
| FTR-012 | Package verification | `cargo package` warns that both named integration tests are excluded. Package verification therefore does not prove the published first-use test path. | Implement the existing package-policy item with a small packaged smoke test or remove repository-only targets from published metadata. |
| FTR-013 | Approved dependency constraint | `bytemuck` is retained as the only Cargo dependency but current production code references it only as an approved placeholder (`use bytemuck as _`). | Do not remove it contrary to the accepted constraint; either use it for a justified checked byte-layout boundary later or document why it remains intentionally unused. |
| FTR-014 | Accidental release risk | README says the crate is unpublished/pre-release, but `Cargo.toml` does not restrict `publish`. | Add an explicit release checklist and decide whether `publish = false` remains until every release blocker closes. |
| FTR-015 | docs.rs scope | No `[package.metadata.docs.rs]` policy records desired features/targets. docs.rs defaults do not build all features, so the target-sensitive AVIF surface may be underrepresented. | Set an intentional docs.rs feature/target policy that does not require the native AVIF stack and clearly labels restricted behavior. |
| FTR-016 | Default feature weight | Default enables all seven Rust codecs. This is convenient for applications but costly for downstream libraries that forget `default-features = false`. | Keep the current default for 0.1 unless measurements/user feedback justify change; make minimal feature selection the primary library-integration example. |
| FTR-017 | Portability boundary | `wasm32-unknown-unknown` supplies `std`, but filesystem calls fail and thread spawning is unavailable; no audit proves codec paths avoid unsupported host facilities. Runtime tests now execute the feature-gate and capability-table lanes on `wasm32-wasip1`; the full semantic matrix and target-call inventory remain. | Add a target-call inventory and full runtime tests. `no_std + alloc` is an optional P3 goal, not a substitute for the required browser/JS WASM support. |
| FTR-018 | SIMD policy | There is no measured `simd128` build lane, scalar equivalence gate, or target-feature policy. | Benchmark per codec, require exact semantic parity, and keep scalar WASM functional before enabling optional SIMD artifacts. |
| FTR-019 | Threading policy | Native AVIF libraries may use threads, while plain browser WASM has different shared-memory and cross-origin requirements. The public capability model does not expose this. | Make single-thread execution complete first; add threaded WASM only as a separately measured artifact/capability. |
| FTR-020 | JS copy cost | No binding contract states when encoded input and decoded output are copied across the JS/WASM boundary. Common `wasm-bindgen` boxed-slice conversions copy. | Measure `Uint8Array` input/output and document ownership, detachment, memory growth, and reuse before promising zero-copy. |
| FTR-021 | JavaScript targets | Browser ESM, bundler ESM, Node, Deno, workers, and CommonJS packaging are not selected or tested. | Choose the smallest supported target set and test actual imports in each published environment; avoid one ambiguous package. |
| FTR-022 | WASM memory behavior | Peak memory, memory growth, allocation failure, maximum pages, and large `Vec` transfer behavior are undefined. | Bind decode/output limits to measured WASM memory and return typed failures rather than relying on trap/OOM behavior. |
| FTR-023 | Worker integration | Cooperative cancellation now exists for decode and the partial encode boundary (API-036/COR-060/COR-070/COR-072) and never publishes a successful partial result; there is still no worker-safe binding or transferable-buffer policy. | Design the JS/WASM binding after the native API settles; the token's single-threaded `Rc<Cell>` state must be replaced or wrapped for worker transfer. |
| FTR-024 | Core/extra definition | The intended core/extra JS split has no checked membership manifest, feature mapping, loader behavior, or per-codec native/WASM size budget. | Define exact artifact inputs and measure raw, gzip, and Brotli sizes for each revision. Do not infer size from `.crate` source size. |
| FTR-027 | Reproducible WASM package | No pinned binding tool version, generated-glue checksum, deterministic package archive, or clean-consumer install test exists. | Pin the release toolchain and compare produced artifact hashes in a clean CI environment. |
| FTR-029 | Size attribution | There is no per-format attribution for Rust code, data tables, generated bindings, native shims, or compression after link-time optimization. | Produce additive singleton/default/all artifacts with identical compiler flags and report deltas without claiming they sum linearly. |
| FTR-030 | Native oracle provenance | The `.oracle-venv` fallback recursively selects a filename beginning with `libavif` but does not verify libavif, dav1d, or libaom versions. The pkg-config path verifies only libavif's version, not its backend versions. | Query and record every native component version at build/test time and reject a mismatch before parity evidence is produced. |
| FTR-031 | Linkage model | Native AVIF searches only dynamic-library forms and embeds/builds runtime search behavior. There is no static, musl, self-contained, or relocatable downstream artifact contract. | Keep this temporary oracle bridge explicitly unsupported for distribution; portable AVIF is the accepted solution rather than expanding native packaging. |
| FTR-033 | Target-OS classification | Library discovery recognizes Windows files, while linking panics outside macOS/Linux/Android. FreeBSD and other Unix targets are also unclassified; CI proves only Ubuntu. | Generate an explicit native-AVIF target table from build logic and fail capability discovery before compiler/linker side effects. |
| FTR-034 | WASM artifact root | The crate builds as the default Rust library type only; it has no `cdylib`/binding wrapper, exported C/JS ABI, or generated package. A successful `wasm32` rlib build is not a consumable JavaScript codec. | Choose a thin binding crate or deliberate crate-type strategy after the Rust API settles, keeping codec features forwarded explicitly. |
| FTR-035 | WASM target conflation | All `target_arch = "wasm32"` targets share the same AVIF and host-capability branches, although browser `wasm32-unknown-unknown`, WASI, and future component targets have different I/O/thread/runtime contracts. Runtime evidence is now keyed by full triple for `wasm32-wasip1`; `wasm32-unknown-unknown` remains compile/rustdoc-only. | Key capability evidence by full target triple and publish only triples with runtime tests. |
| FTR-037 | Core/extra distribution shape | One Cargo source package can produce many feature builds, but it cannot by itself define two independently versioned JS archives, loader fallback, cache keys, or shared types. | Specify core/extra as reproducible release artifacts generated from one revision and one API schema; do not imply that Cargo feature names alone solve package splitting. |
| FTR-038 | Feature/capability versioning | Format features are public Cargo API. Adding operation subfeatures, changing default membership, or making `ico` stop forwarding `png`/`bmp` can break downstream feature assumptions even if Rust symbols remain. | Include feature-set diffs in the release compatibility gate and publish the umbrella/subfeature rule from FTR-026. |

### Assurance gaps beyond line and branch coverage

| ID | Finding | Required evidence |
| --- | --- | --- |
| QA-001 | Current feature CI runs no-feature, each singleton, default, and all, but not relevant pairwise combinations or the full powerset. | Static cfg inventory plus targeted pairs for shared compression and ICO; a powerset only if runtime cost stays reasonable. |
| QA-002 | The all-feature semantic manifest runs natively only. The feature-gate and capability-table suites now execute on `wasm32-wasip1`, but the full semantic matrix is still not executed in a WASM runtime. | Execute default, singleton, and all supported semantic rows in a real WASM runtime. |
| QA-003 | Coverage is all-feature native coverage. Disabled-feature arms and target-only behavior are partly reached by separate tests or coverage hooks, not one semantic snapshot. | State coverage provenance per lane and compare native/WASM snapshots only where source mappings are compatible. |
| QA-005 | No no-panic matrix exists across all valid public modes, formats, options, and sequence shapes. | Add a compact generated fixture matrix; COR-002 shows validation plus 100% coverage did not guarantee panic freedom. |
| QA-006 | The encode manifest samples many options but is not a Cartesian source-mode × target-format matrix. | Add one row per Pillow-accepted/rejected mode boundary and one cross-format decode→encode row for every claimed transcode. |
| QA-008 | No exact public error-message policy exists, despite retaining oracle messages. | Decide whether Rust messages are stable; test kind plus structured fields, and treat Pillow text as diagnostic evidence rather than equality unless intentionally mapped. |
| QA-009 | No fuzzing, mutation corpus, or differential randomized test runs in CI. | Add format-aware fuzzing after limits; preserve minimized failures as fixtures. |
| QA-010 | No performance, peak-memory, stack, compiled binary, or WASM artifact benchmarks are revision-bound. | Implement the existing benchmark protocol before any "fast", "small", or "lightweight" claim. |
| QA-011 | No semver/public API diff runs before release. | Add a public API snapshot once enum/type decisions settle. |
| QA-012 | Test fixtures prove Pillow 12.2.0 behavior, not every legal file accepted by the format specification. | Maintain a separate format-completeness corpus and classify divergences rather than relabeling them Pillow parity. |
| QA-013 | `cargo package` could not complete locally during this audit because the sandbox could not reach the registry index; file-list and ignored-test warnings were still captured. | Re-run package verification in networked CI and install/use the produced archive in a clean temporary consumer. |
| QA-016 | A dependency-free `OutputSink` contract exists with deterministic `OutputWrite` cause coverage for every enabled still codec and supported sequence path. JPEG, PNG, BMP, GIF still and sequence, WebP still and multi-frame sequence delivery, ICO still delivery, native AVIF still and sequence delivery, and the one-frame JPEG/BMP/WebP/ICO plus multi-page TIFF sequence deliveries now exercise multiple structural writes, policy preflight, sink-triggered cancellation where implemented, and one post-delivery flush with typed flush-failure coverage; JPEG still and one-frame sequence delivery additionally cover marker/scan boundaries, GIF delivery additionally covers signature/logical-screen, color-table, extension/image sub-block, and trailer segments, TIFF still and multi-page sequence delivery additionally cover the header, strip/padding, and IFD/value segments, and AVIF delivery additionally covers top-level ISO-BMFF box boundaries. A Rust-only contract now proves a genuine partial second structural write for every available still writer and each supported multi-frame GIF/TIFF/WebP/native-AVIF sequence writer, with the selected `OutputWrite` cause, exact delivered-prefix preservation, the selected encode stage, and no `flush`; other short/interrupted writes, rollback, and partial-container cleanup remain open. | Define short-write, rollback, and recoverable cleanup behavior for the current structural boundaries before claiming a universal incremental writer. |
| QA-019 | Exact encoded-byte determinism is now proven between the ARM64 native host and `wasm32-wasip1` for a fixed encoder/decoder subset; x86-64, 32-bit, and big-endian lanes are still missing. | Run deterministic fixture subsets across the remaining targets and classify unavoidable native-oracle differences explicitly. |
| QA-020 | Peak stack use and recursion depth are not measured for nested containers, TIFF directory graphs, DEFLATE/Huffman paths, or AV1 syntax. | Add bounded deep-structure fixtures and stack instrumentation before browser/embedded recommendations. |
| QA-021 | Reverse-mapped/generated fixtures do not all retain generator version, parameters, first-divergence purpose, and minimized-input hash in the manifest. | Extend TST-009 with reproducible generation provenance and a regeneration check. |
| QA-022 | WASM compile success provides no browser evidence for boundary copies, memory growth, exceptions, worker use, or real artifact size. | Run a small Playwright/WebDriver-free JS harness in a pinned browser runtime and Node for every published artifact target. |
| QA-023 | Emitted bytes are primarily re-opened through Pillow, which can share libjpeg/libwebp/libtiff/libavif implementations with the oracle path. | Decode representative outputs with an independent implementation or browser and record that evidence separately from Pillow parity. |
| QA-024 | Round-trip tests do not publish a uniform rule separating lossless exact samples, lossy decoded tolerances, and deterministic encoded bytes. | Add an assertion policy per format/mode/option row and reject ambiguous generic “round trip passed” claims. |
| QA-026 | Policy and interruption evidence | Decode/output policy boundaries, cache/retry behavior, sink preflight, structural cancellation, and the currently implemented Rust-only work-budget checkpoints—including JPEG RGB-to-YCbCr conversion and chroma-downsample output after each 1,024 pixels, baseline entropy traversal after each 1,024 MCUs, optimized baseline Huffman frequency gathering after each 1,024 AC coefficients, progressive scan block-slot generation after each 1,024 blocks, progressive scan-event frequency gathering after each 1,024 events, progressive scan coefficient traversal after each 1,024 coefficients, entropy output after each 1,024 emitted bytes, high-color GIF nearest-palette candidate ordering and bounded scans after each 1,024 work items, lossy WebP RGBA transparent-area cleanup and alpha-palette source collection and index packing plus lossless VP8L hidden-RGB cleanup and image-palette construction and palette-mode index packing after each 1,024 scanned/source pixels, VP8 first-partition intervals through 262,144 logical bits and coefficient intervals through 1,048,576 logical bits, VP8 boolean boundaries, 1,024-byte boolean output, lossless VP8L 256-pixel copy-token cache-population scans, palette-index lookup candidate scans after each 64 palette entries, palette sign collection and nearest-delta candidate scans after each 64 palette entries or candidate values, Huffman RLE preparation and canonical-code assignment scans after each 64 code-length symbols, Huffman-tree insertion scans after each 64 candidate nodes, Huffman-tree code-length-token frequency and trailing zero-repeat-token trim scans after each 16 compressed token entries, Huffman code-length emission after each 16 compressed token entries, and lossless VP8L logical bitstream intervals through 1,048,576 bits—are accepted in the feature-gated integration contract. The Pillow manifest remains the source of Pillow-observable success/error/byte evidence; it does not own caller budgets, cancellation, sink prefixes, or rollback. | Add only real remaining codec/interior/allocation/short-write boundaries, keep them in the existing feature-gate contract, and record parity as unchanged regression evidence rather than adding synthetic Pillow rows. |
| QA-027 | Encoder option determinism can be affected by unordered `HashMap` extras and target-native libraries, but cross-process output stability is not checked. | Replace public catch-all options, sort any retained opaque options, and compare independent process runs. |
| QA-028 | Corpus growth is counted in rows, not unique parser states/properties; many rows may exercise the same structural class. | Maintain a compact property-to-fixture map per codec so every claimed syntax/state has a named minimal witness. |
| QA-030 | No benchmark checks output allocation count, retained encoded+decoded cache memory, sequence amplification, or caller-buffer reuse. | Add allocation/peak-memory measurements alongside time and artifact size; never optimize from source line count. |
| QA-031 | Legal-but-unsupported format classes are not a uniform fixture lane. Some are absent entirely, while malformed inputs dominate error coverage. | Add active negative-capability rows for every named legal class and require `Unsupported` rather than incidental `Malformed`. |
| QA-033 | Generator reproducibility is checked through hashes inside generated data, but a clean regeneration/no-diff run is not a mandatory CI gate for every script and asset. | Run generators in a clean checkout, fail on any diff, and record pinned Python/native tool identities. |
| QA-034 | Debug and optimized builds are not compared for exact results. Overflow checks, floating-point/codegen choices, and `cfg(debug_assertions)` can expose behavior that line coverage in one profile misses. | Run a compact deterministic parity subset in both profiles and compare structured errors and artifacts. |
| QA-035 | `EncodedImage` clone/cache concurrency is described but not stress-tested for one initialization, shared success, shared failure, panic recovery, and deterministic observations across threads. | Add bounded concurrent tests without timing assertions; if WASM threads are later supported, repeat under that artifact. |
| QA-036 | Future streaming decoders need lifecycle tests for every prefix boundary, repeated empty append, finish before complete, append after finish, reset, cancellation, and retained partial output. | Derive prefix fixtures from existing files and compare terminal output with one-shot decode. |
| QA-037 | Container-equivalent metamorphic variants are sparse: resegmented PNG IDAT/fdAT, GIF sub-block splits, reordered legal TIFF strips, WebP padding, and AVIF extent partitioning should preserve the same observable result. | Add generated equivalence families while retaining exact original container metadata where the model exposes it. |
| QA-038 | The native AVIF C bridge and pinned native stack are outside Rust's `unsafe_code = deny` evidence and are not run under sanitizer instrumentation in this project. | Run the oracle bridge under ASan/UBSan in a dedicated native lane until it is removed; do not present that as proof of the portable implementation. |
| QA-039 | Oracle identity records Pillow 12.2.0 but not one generated fingerprint of every compiled Pillow feature and linked codec version used to create fixtures. | Store and validate `PIL.features` plus libjpeg/zlib/libtiff/libwebp/libavif/backend versions with the manifest provenance tuple. |
| QA-040 | Exact output rows do not all state whether determinism is expected across process, architecture, native backend, compiler, and optimization profile, or only within the pinned oracle build. | Add a determinism scope field and test only the dimensions it promises. |
| QA-041 | No retained test compares `inspect` facts before decode with source facts confirmed during/after decode when inner payload headers can disagree with outer containers. | Add declared-versus-confirmed fixtures for AVIF first, then any JPEG/TIFF/WebP path with late-discovered source properties. |
| QA-042 | Fixture selection has no explicit coverage for public object mutation after construction: changing dimensions, mode, palette, frame metadata, or pixels between successful validation and encode. | Generate post-construction mutation cases and require every public encoder entry to revalidate without panic. |

### Documentation and release minutiae

These are discoverability/maintenance findings, not codec defects.

| ID | Finding | Required action |
| --- | --- | --- |
| DOC-002 | The documentation audit reports one unlabeled fence in the retained `third_party/image-webp/README.md`. | If the file is maintained locally, label it; if it is an upstream verbatim artifact, exclude third-party documents from style lint and preserve its checksum/provenance. |
| DOC-003 | README feature tables describe formats but cannot express operation-, mode-, sequence-, target-, or verification-level restrictions precisely. | Generate/link the capability and direct-mode tables from active fixture/cfg data rather than maintaining another prose matrix. |
| DOC-004 | README examples are source snippets, not a separately installed clean-consumer test; Cargo package currently excludes the integration targets. | Turn the shortest first-use path into a packaged smoke test and keep its source synchronized with README/rustdoc. |
| DOC-005 | Coverage/parity numbers are revision-bound but can look current after later docs-only or implementation commits. | The claim ledger (COR-039) now pins the revision/hash/coverage tuple; keep every numeric claim consistent with it. |
| DOC-006 | This exhaustive roadmap is intentionally large and will become unmaintainable if completed findings accumulate. | Delete resolved rows during each slice and move only current behavior to README/architecture/rustdoc/testing; never create a second issue ledger. |
| DOC-007 | The changelog contains detailed unreleased implementation claims but no released-version link/reference structure yet. | Add comparison links and release entries only at the first tag; until then, keep every claim consistent with the current branch. |
| DOC-008 | Support, security, contribution, conduct, issue forms and CODEOWNERS exist, but maintainer succession/governance and release recovery are only roadmap statements. | Keep this P2 for a single-maintainer pre-release; define ownership and recovery before inviting production reliance. |

### Ecosystem validation (researched 2026-07-31)

This is a dated comparison against primary project documentation and pinned
APIs. It validates integration expectations; it does not make another library
an oracle, authorize its dependencies or license, or import its
image-processing scope. Pillow 12.2.0 remains the behavioral oracle even though
newer Pillow documentation exists.

Evidence labels in this section mean:

- **current source**: directly observed in this repository at the revision
  named above;
- **external contract**: documented by the linked upstream project; and
- **candidate direction**: useful here only after a fixture-backed design
  slice is accepted.

The second research sweep added 111 distinct minute rows across the common API,
all eight codecs, Cargo/target packaging, and assurance. It compared the
working-tree public API and build script with pinned Pillow plugin source,
current Rust codec APIs, W3C PNG Third Edition and WebCodecs, the GIF89a
specification, Microsoft DIB structures, TIFF 6.0/libtiff, libjpeg-turbo,
libwebp, and the pinned libavif 1.4.1 header. These rows are intentionally
specific enough to become one failing fixture or one bounded design decision;
they do not authorize a broad abstraction rewrite.

#### Common decode/encode comparison

| Library/API | Input, detection, and lazy behavior | Buffers, limits, sequence, and metadata | Validated lesson for this crate |
| --- | --- | --- | --- |
| Pillow 12.2.0 | `Image.open` identifies content and usually defers pixel loading; plugin/file lifetime differs for single- and multi-frame inputs. `save` receives a format or may infer one from a filename. | A high-level `Image` carries mode/info/palette and plugins may privately convert before saving. Frame seeking is lazy relative to the file. | Our content detection and explicit target are sound. `EncodedImage` is genuinely deferred and cached for one full still decode, but is not Pillow-style reader/frame lazy. |
| Rust `image` 0.25.10 | `ImageReader` accepts an explicit format or guesses from content. Format features and `ImageFormat` operation queries are public. | Decoder traits expose limits, metadata and caller buffers; encoder traits write raw samples to a caller writer; animation is iterator-based. | API-001, API-023/024, API-027, and API-034 are ordinary codec integration needs, not image processing. |
| Go `image` | `Decode` auto-detects registered formats from an `io.Reader`; `DecodeConfig` obtains dimensions/color model before full decode. | Generic readers and typed images are used; format registration is extensible. | Content auto-detection plus a cheaper bounded information path is established practice. Dynamic codec registration remains an explicit non-goal. |
| WebCodecs `ImageDecoder` | The caller supplies an explicit MIME type and an `ArrayBuffer`, typed view, or `ReadableStream`; animation preference is explicit. | Browser APIs expose type support, tracks, indexed frames, progressive completeness, pending-work reset, final close, transfer ownership, color-space conversion, and premultiplication choices. Availability is not uniform across browsers. | The JS binding needs explicit byte ownership, frame iteration, provisional/terminal output, lifecycle, color/alpha policy, and environment support; do not rely on WebCodecs as the implementation. |
| `png` 0.18.1 | Header read is separate from full frame/row decode. | Caller output buffers, limits, APNG raw frame control, row/pass iteration, explicit finish, stream output, transformations, and ancillary metadata are public. | PNG-008 through PNG-021 plus the non-fatal diagnostic contract describe established codec surfaces, while cropping/resizing remain unnecessary. |
| `gif` 0.14.2 | Decoder configuration controls color output and per-frame memory behavior. | Frames retain exact centisecond delay, user-input, disposal, offsets, palette and borrowed/owned pixel data; encoding writes to a caller sink. | GIF-007 through GIF-021 close actual presentation, stream-field, and memory-contract gaps. |
| `tiff` 0.11.3 | Decoder handles directories/pages and incremental strips/chunks; BigTIFF is a distinct supported class. | Typed sample results, strips/tiles, arbitrary tags, BigTIFF and incremental directory/image encoders exist. | A complete TIFF codec needs page selection, layouts, tag retention, limits and chunk I/O, but not a processing pipeline. |
| `jpeg-encoder` 0.7.x and `zune-jpeg` 0.5.x | Low-level APIs accept exact layouts; decoder options select output colorspace, strictness, maximum dimensions and scan counts. | JPEG encode layouts include luma, RGB(A), BGR(A), YCbCr, CMYK and YCCK; row-providing buffers, restart/progressive/custom-table controls exist. | `DecodedImage` is a reasonable strict input, but its layout/color model is too narrow for byte-preserving JPEG use. |
| `ravif` 0.13.x | Encoder accepts RGB/RGBA or raw planes with explicit quality/speed/depth choices. | Threading, alpha, EXIF and high-bit-depth controls are part of the codec contract. | Portable AVIF needs typed planes/options and metadata without acquiring ravif as a dependency. |
| `zenwebp` 0.4.x | Documents pure-Rust/WASM decode, header probing, output formats and resource limits. | Animation demux/mux, metadata and streaming surfaces exist. Its license/dependency choices differ from this project. | Use only as independent evidence that WebP limits, demux, metadata and incremental I/O matter; do not copy code or architecture. |
| `zencodec` 0.1.x family | Emerging common traits expose capability discovery, metadata, limits, threading, cancellation, animation and source encoding details. | The family intentionally integrates with broader crates and dependencies and some codecs remain incomplete. | The integration problems are real; its broad abstraction and dependency model are not a blueprint for this zero-dependency codec crate. |
| Wuffs | Sans-I/O codecs consume caller-managed buffers, avoid hidden allocation/syscalls, and emphasize fuzzed bounded behavior. | Streaming status, short read/write handling and work buffers are explicit. | Caller-owned I/O and checked work boundaries are a useful long-term correctness target; a Wuffs-style framework rewrite is not required. |
| libpng/libspng, libjpeg-turbo, libwebp, libavif | Mature C APIs expose progressive/incremental I/O, output callbacks or buffers, resource/configuration limits, source color details, metadata and sequence/container state. | The exact surface is codec-specific rather than one universal image object. | Common policy should cover ownership/errors/limits; codec-native details should stay in typed per-format structures. |

#### Encoder input verdict

The current public signature—one validated `DecodedImage`, an explicit target
`ImageFormat`, and options—is aligned with low-level encoders. It deliberately
does **not** mean "any image":

| Input presented by caller | Current result | Correct long-term contract |
| --- | --- | --- |
| Encoded JPEG/PNG/GIF/etc. bytes | Not accepted by `encode`; use `decode` first | Keep explicit decode→encode. A same-format lossless container rewrite would be a separate operation, not ordinary encode. |
| A valid `DecodedImage` in a target-supported direct mode | Accepted | Keep this strict zero-processing path and publish a generated per-codec mode table. |
| A valid `DecodedImage` in a Pillow-supported but indirect mode | Usually `Unsupported` | Add only codec-private, exact-fixture-proven normalization. Do not expose general color conversion. |
| Arbitrary dimensions/mode/color/palette fields assembled by a caller | Validated at the selected encoder | Preserve the completed structured-error/no-panic cross-product whenever a mode or encoder is added. |
| A sequence of already-decoded frames/pages | Accepted only by implemented sequence encoders and their narrower presentation contract | Add explicit animation/page/entry kinds and never collapse metadata silently. |
| Strided, planar, borrowed, shared, or caller-buffer samples | Not represented | Add minimal codec transfer layouts and destination APIs only where they avoid a measured copy or preserve native samples. |
| Another library's dynamic image object | Not accepted | Keep adapters downstream. This crate should not acquire a `DynamicImage` processing layer. |

Pillow looks more permissive because its high-level `Image` plugins can perform
private pre-save conversions. Rust `image::ImageEncoder`, `jpeg-encoder`, PNG,
TIFF, libwebp, and libavif instead require raw samples plus explicit layout,
dimensions, target configuration, and/or a destination. Therefore the gap is
not "accept any object"; it is to make exact accepted layouts, private
normalization, output ownership, limits, and failures discoverable.

#### Adopt, defer, and reject

Adopt after fixture-backed design:

- typed capability queries, decode limits, exact output-size preflight and
  caller buffers;
- explicit source/output color, alpha, metadata and sequence/container
  descriptors;
- bounded reader/writer or Sans-I/O adapters, cancellation points and
  structured diagnostics;
- exact per-codec mode/option tables generated from active tests; and
- a stable, measured JS/WASM transfer and package contract.

Defer until current eight formats and portable AVIF are complete:

- new formats, `no_std + alloc`, optional SIMD/threaded WASM, and a core/extra
  package split;
- borrowed/shared decoded storage unless allocation measurements justify its
  additional lifetime/API cost; and
- raw container rewriting or unknown-block passthrough.

Reject for this repository:

- resize, crop, rotate, color adjustment, filters, drawing and reusable public
  conversion APIs;
- dynamic codec/plugin registration;
- filesystem/path policy, runtime Pillow, general logging, and native codec
  dependencies as the final implementation; and
- copying APIs whose value depends on an image-processing object model.

#### Primary comparison references

Common and high-level APIs:

- [Pillow `Image.open` and `Image.save`](https://pillow.readthedocs.io/en/stable/reference/Image.html)
  and [file lifecycle](https://pillow.readthedocs.io/en/stable/reference/open_files.html)
- [Pillow 12.2.0 source](https://github.com/python-pillow/Pillow/tree/12.2.0)
  and [format handbook](https://pillow.readthedocs.io/en/stable/handbook/image-file-formats.html)
- [`image::ImageReader` 0.25.10](https://docs.rs/image/0.25.10/image/struct.ImageReader.html),
  [`ImageDecoder`](https://docs.rs/image/0.25.10/image/trait.ImageDecoder.html),
  [`ImageEncoder`](https://docs.rs/image/0.25.10/image/trait.ImageEncoder.html),
  [`ImageFormat`](https://docs.rs/image/0.25.10/image/enum.ImageFormat.html),
  [`Limits`](https://docs.rs/image/0.25.10/image/struct.Limits.html), and
  [`AnimationDecoder`](https://docs.rs/image/0.25.10/image/trait.AnimationDecoder.html)
- [Go `image` package](https://go.dev/pkg/image/)
- [W3C WebCodecs `ImageDecoder`](https://www.w3.org/TR/webcodecs/#image-decoding)
  and its
  [decode/reset/close lifecycle](https://www.w3.org/TR/webcodecs/#image-decoder-interface)

Rust codec APIs:

- [`png` 0.18.1](https://docs.rs/png/0.18.1/png/),
  [`Reader`](https://docs.rs/png/0.18.1/png/struct.Reader.html), and
  [`StreamWriter`](https://docs.rs/png/0.18.1/png/struct.StreamWriter.html)
- [`gif` 0.14.2](https://docs.rs/gif/0.14.2/gif/),
  [`Frame`](https://docs.rs/gif/0.14.2/gif/struct.Frame.html), and
  [`MemoryLimit`](https://docs.rs/gif/0.14.2/gif/enum.MemoryLimit.html)
- [`tiff` 0.11.3 decoder](https://docs.rs/tiff/0.11.3/tiff/decoder/struct.Decoder.html)
  and [encoder](https://docs.rs/tiff/0.11.3/tiff/encoder/struct.TiffEncoder.html)
- [`jpeg-encoder` color layouts](https://docs.rs/jpeg-encoder/latest/jpeg_encoder/enum.ColorType.html)
  and [row-providing `ImageBuffer`](https://docs.rs/jpeg-encoder/latest/jpeg_encoder/trait.ImageBuffer.html)
- [`zune-jpeg`](https://docs.rs/zune-jpeg/latest/zune_jpeg/) and
  [`DecoderOptions`](https://docs.rs/zune-core/latest/zune_core/options/struct.DecoderOptions.html)
- [`ravif::Encoder`](https://docs.rs/ravif/latest/ravif/struct.Encoder.html)
- [`zenwebp`](https://docs.rs/zenwebp/latest/zenwebp/),
  [limits](https://docs.rs/zenwebp/latest/zenwebp/decoder/struct.Limits.html), and
  [mux/demux](https://docs.rs/zenwebp/latest/zenwebp/mux/)
- [`zencodec` common contracts](https://docs.rs/zencodec/latest/zencodec/)
  and [metadata](https://docs.rs/zencodec/latest/zencodec/struct.Metadata.html)

Native/reference codec APIs:

- [Wuffs design and codecs](https://github.com/google/wuffs)
- [PNG Third Edition](https://www.w3.org/TR/png-3/), including
  [APNG structure and sequencing](https://www.w3.org/TR/png-3/#4Concepts.APNG)
- [libpng manual](https://libpng.org/pub/png/libpng-manual.html) and
  [libspng decode API](https://libspng.org/docs/decode/)
- [GIF89a specification](https://giflib.sourceforge.net/gifstandard/GIF89a.html)
- [Microsoft `BITMAPV5HEADER`](https://learn.microsoft.com/en-us/windows/win32/api/wingdi/ns-wingdi-bitmapv5header)
  and [bitmap compression](https://learn.microsoft.com/en-us/windows/win32/gdi/bitmap-compression)
- [TIFF Revision 6.0](https://download.osgeo.org/libtiff/doc/TIFF6.pdf) and
  [libtiff tag/type definitions](https://gitlab.com/libtiff/libtiff/-/blob/master/libtiff/tiff.h)
- [libjpeg-turbo API overview and colorspaces](https://github.com/libjpeg-turbo/libjpeg-turbo#using-libjpeg-turbo)
- [libwebp 1.6.0 API](https://chromium.googlesource.com/webm/libwebp/+/refs/heads/main/doc/api.md)
- [WebP container specification](https://developers.google.com/speed/webp/docs/riff_container)
  and [demux frame/chunk API](https://chromium.googlesource.com/webm/libwebp/+/refs/heads/main/src/webp/demux.h)
- [libavif releases](https://github.com/AOMediaCodec/libavif/releases) and
  [libavif 1.4.1 public header](https://raw.githubusercontent.com/AOMediaCodec/libavif/v1.4.1/include/avif/avif.h)

WASM and packaging:

- [Rust `wasm32-unknown-unknown` target support](https://doc.rust-lang.org/stable/rustc/platform-support/wasm32-unknown-unknown.html)
- [`wasm-bindgen` boxed numeric slices](https://rustwasm.github.io/docs/wasm-bindgen/reference/types/boxed-number-slices.html)
- [`wasm-pack build` targets](https://rustwasm.github.io/docs/wasm-pack/commands/build.html)
- [WebAssembly JavaScript memory](https://developer.mozilla.org/en-US/docs/WebAssembly/Reference/JavaScript_interface/Memory)
- [Cargo features and additive unification](https://doc.rust-lang.org/cargo/reference/features.html)
- [docs.rs build metadata](https://docs.rs/about/metadata)

### Corpus expansion without changing the Pillow oracle

Pillow remains the authority for expected observable behavior. Upstream
format corpora answer a different question: which legal or historically
important classes have not yet been presented to Pillow and this crate?

| Format | Candidate primary corpus | Use |
| --- | --- | --- |
| All eight | [Pillow 12.2.0 `Tests/images`](https://github.com/python-pillow/Pillow/tree/12.2.0/Tests/images) | First source for pinned success, leniency, plugin, mode, metadata, and error behavior |
| JPEG | [libjpeg-turbo 3.1.4.1](https://github.com/libjpeg-turbo/libjpeg-turbo/tree/3.1.4.1) test images and generated regressions | Sampling, progressive scans, restart markers, CMYK/YCCK, malformed marker classes |
| PNG/APNG | [libpng](https://github.com/pnggroup/libpng) and the upstream [`png` crate](https://github.com/image-rs/image-png) tests | Depth/color combinations, Adam7, chunks, CRCs, APNG control/blend/disposal |
| GIF | Pillow fixtures plus pinned giflib regression assets if their license and source are recorded | LZW boundaries, local/global tables, extensions, disposal, damaged streams |
| BMP/ICO/CUR | Pillow fixtures and a pinned subset of image-rs format tests | Header generations, masks, palettes, RLE, DIB, icon directories, cursor hotspots |
| TIFF | [libtiff test images](https://gitlab.com/libtiff/libtiff/-/tree/master/test/images) and [libtiff-pics](https://gitlab.com/libtiff/libtiff-pics) | Endianness, numeric formats, strips/tiles, predictors, planar data, multipage, BigTIFF, tags |
| WebP | [libwebp tests](https://github.com/webmproject/libwebp/tree/main/tests) at the pinned 1.6.0 revision | VP8/VP8L partitions, alpha, animation, metadata, configuration errors |
| AVIF/AV1 | [libavif tests](https://github.com/AOMediaCodec/libavif/tree/v1.4.1/tests) and pinned AOM conformance vectors | Item/grid/sequence structure, alpha, YUV/depth/range, AV1 syntax expansion |

Corpus intake rules:

1. pin a release or commit and record source URL, original path, license,
   SHA-256, and why the case adds a new class;
2. run the exact Pillow 12.2.0 build first and record open/inspect/load/verify/
   sequence/save outcomes;
3. minimize only when the minimized file preserves both the Pillow outcome and
   the targeted codec property;
4. store exact raw and encoded references only when redistribution is allowed;
5. use generation scripts for large or license-restricted inputs when possible;
6. never change an expected result merely because another implementation
   differs from Pillow; record the independent result as supporting evidence;
   and
7. keep every error as an active fixture rather than a planned/skipped row.

#### How to pick the next slice

The expanded inventory is not a requirement to implement every convenience
surface. Select work in this dependency order and close one coherent vertical
slice at a time:

| Order | IDs/classes | Why first | Exit condition |
| --- | --- | --- | --- |
| 1 | API-023/030; QA-026 | Close the remaining allocation/accounting, interior-work, and policy/error-detail gaps before accepting more inputs. | A real remaining resource boundary has a stable typed result, non-Pillow feature-gate evidence where Pillow has no equivalent, and no changed parity behavior. |
| 2 | API-019/034/040 plus the matching metadata rows | Complete the remaining non-alpha AVIF item relationships/properties/color fields without disturbing the established source-provenance model. | Exact source/container state survives decode and, where supported, encode without public processing. |
| 3 | API-027 and codec row/strip/tile/frame slices | Extend source-bound per-frame/page/strip/tile access without introducing a second cache or sequence model accidentally. | Whole-buffer convenience wraps the same bounded engine; caller buffers and eager results remain byte-identical. |
| 4 | API-017/018/036; FTR-017 through FTR-024/027/029; QA-016/019/020/022/030 | Finish remaining incremental I/O, deeper interruption, full native/WASM semantic execution, packaging, and allocation/size measurements. | Real native and WASM runtime lanes pass with reproducible artifacts and measured copy/memory behavior. |
| 5 | FMT-000 through FMT-013 | Format expansion adds the most maintenance and least value while current contracts remain incomplete. | Start only after a separate acceptance decision records every eligibility field listed below. |

Within a row, choose the smallest reverse-mappable codec case and finish its
manifest, API, implementation, documentation, feature matrix, WASM runtime
evidence, and Coverage MCP result before opening the next slice.

### Execution order for the next session

This order turns each discovery into a failing fixture before implementation
and avoids broad rewrites.

The current WebP VP8L work-control surface also polls token-aware cost-manager
cost/length-table initialization after each 1,024 entries, interval-update and
cleanup scans after each 256 cumulative interval entries,
repeated-run hash-chain insertion and long backward-reference result backfills
after each 256 entries, while retaining the
original no-token hot paths. Predictor mode application also checkpoints its
pre-transform source snapshot copy after each 1,024 pixels; its no-token path
retains the original bulk clone. VP8L candidate trials also reuse the
already-emitted prefix and retain only each trial suffix, removing repeated
prefix copy/allocation without changing selected bytes. These are Rust-only
checkpoint/runtime dimensions: Pillow
exposes no caller budget, cancellation token, or sink contract, so they do not
add a parity row or fixture. Finer WebP tree/bitstream work, other codec interiors,
allocation accounting, and complete rollback semantics remain active gaps.

Completed first: COR-001 through COR-072, including exact WebP mode
preparation and alpha payload selection, strict JPEG/WebP option rejection,
lossless one-frame sequence fallback, public-mode validation, and common
decode/sequence error parity, exact sequence evidence, and bounded AVIF brand
detection. APNG sequence decode now retains default-image state, rational
timing, source controls, and exact rendered canvases. WebP sequence encode now
emits Pillow-byte-exact full-canvas keyframes and rejects every unsupported
control through fixture-backed structured errors. TIFF multipage decode/encode
retains page dimensions, modes, and exact bytes across raw and compressed
classic IFD chains, TIFF source byte order now survives inspection, still
decode, and every retained page, and runtime capability discovery mirrors the
current feature/target dispatch. Encode options are target-qualified typed
records with a strict, manifest-tested legacy-pair migration boundary. The
shared decode policy now bounds pre-detection encoded-input bytes, inspected
canvas width, height, pixels, decoded transfer bytes, later frame/page bytes,
cumulative sequence bytes, and retained metadata extents. `EncodePolicy` now
admits only complete encoded results at or below an inclusive output cap and
keeps rejected results out of caller-owned sinks. Transient encoded-output
allocation remains an explicit open resource boundary under API-023/030.
`ImageFormat` now exposes
case-insensitive Pillow-recognized extension aliases plus stable MIME,
canonical-extension, and full alias-list queries with a table-driven contract
test. `DecodePolicy` now also bounds the inspected frame/page count through
`max_frames` before inspection, sequence materialization, or source
construction, while still and lazy still paths remain bounded to the single
materialized frame. Verification callers can now request an explicit strength
and receive a format-qualified `Unsupported` when the codec cannot provide it,
so header-only success is never silently reported as stronger evidence.
`DecodePolicy` now also bounds every later frame/page's decoded bytes and the
cumulative retained sequence bytes inside every sequence decoder before the
next frame's allocation. Every decoder now ignores well-formed trailing bytes
after its container-defined extent, `Decoded::consumed_bytes` reports that
extent where the container defines one unambiguously, and AVIF container
validation accepts trailing bytes exactly as Pillow 12.2.0/libavif do. Every
active malformed class is now catalogued in a generated, CI-checked ledger
with Pillow outcome, Rust error contract, evidence origin, and an explicit
specification status. Near-limit arithmetic is proven at `u64::MAX`/`u32::MAX`
boundaries on small assets, and the allocation policy is decided: checked
preflight gates hostile input while codec-internal allocations remain
infallible with Rust's default OOM abort. Every codec-dispatched failure now
names its public operation through `ImageError::stage()`, while caller-built
errors stay explicitly stage-free. `DecodePolicy` now also bounds the encoded
metadata extent before any inspection or pixel work, with per-format scanners
that exclude primary pixel payload bytes and a SHA-pinned measurement
manifest. Every current decoder work dimension is documented as bounded by the
typed resource set; `EncodePolicy` bounds complete result admission and now
also exposes a deterministic checkpoint work budget, but neither field bounds
the transient allocation performed by whole-buffer encoders or recoverable
OOM. Strictness/output-
mode are recorded as result-shaping policy belonging to the API-033 family
rather than new decode resource limits.
Codec-dispatched failures now name their parse site with a byte offset and
container-structure identity on top of the operation stage. The
BMP header, palette, pixel-span, bitfield, and RLE paths retain that context
through both decode and basic inspection, and ICO header, directory,
entry-range, and embedded PNG/DIB/CUR paths now retain absolute container
context. WebP inspection/container-chunk paths now retain WebP context, and
still/sequence payload-decoder failures now retain `webp_bitstream` at the
validated payload start (or the current ANMF container offset for animation).
Finer decoder-internal cursors remain outside the contract. The BMP, ICO, and
WebP witnesses are Rust-only defensive/error-contract evidence, not
Pillow-parity rows.
revision-bound claim tuple is now machine-checked by a committed ledger and CI
verifier, and the feature-evolution rule pins umbrella stability and additive
subfeatures. Runtime capability tables are now emitted per feature lane on the
native host and `wasm32-wasip1`, executed under Node's WASI preview1 in CI,
and checked against a committed fixture; the exact feature-matrix command is
registered with Coverage MCP. The optional AVIF build script now declares Cargo
rerun triggers for every compiler and archiver variable it consults
(`CC_<target>`, `TARGET_CC`, `CC`, and corresponding `AR` names). The dedicated
build-script decision tests prove target-name normalization and
specific-to-target-to-host precedence; this is build invalidation evidence, not
Pillow parity evidence. AVIF WASM operations now report staged
codec-level `Unsupported` errors that match capability discovery instead of a
stale operation-free gate, and target-unavailable AVIF sequence/encode
failures expose that reason through `unsupported_reason()`. `DecodedSequence`
now carries an explicit
`SequenceKind` so TIFF pages are never conflated with timed animation, and
`SourceDescriptor` records the container-declared alpha association
(`SourceAlpha`) without changing the normalized unassociated transfer layout.
Decoded images and sequences now retain an ordered opaque-block model
(`OpaqueBlock`) for uninterpreted container blocks, starting with PNG unknown
ancillary chunks, with no implicit replay by default encoding, and known PNG
metadata chunks are retained as raw, unparsed `OpaqueMetadata` records without
exposing inflated payloads; structurally recognizable invalid compressed
ancillary members are omitted with stable diagnostics. Exact PNG color fields
(sRGB intent, gamma,
chromaticities, raw ICC profile) are retained in `SourceColor` without
implying color conversion, and GIF comment/plain-text/application extensions
are retained as raw metadata records while unknown labels stay opaque. JPEG
APPn/COM marker payloads are retained in raw stream order (including
multi-segment fragments), with the APP14 Adobe transform still parsed, and
WebP ICCP/EXIF/XMP chunks are retained in scan order while unknown RIFF chunks
stay opaque. TIFF tags retain typed identity and exact stored bytes per page,
with unknown tags opaque and known metadata tags in the metadata records, and
AVIF top-level unknown/free/skip boxes are retained raw while interpreted
boxes stay out. Still decode now preflights its exact transfer-byte length
through `ImageInfo::decoded_bytes` and writes byte-identical pixels into an
exact-size caller destination through `decode_into`, rejecting short or
oversized buffers without partial writes, with the exact transfer layout
(row bytes, packed rows, total bytes, alignment) exposed through
`TransferLayout`. Inspection now distinguishes basic header facts from deep
frame counting through `inspect_basic` and the `frame_count_complete` flag,
and a borrowed `EncodedImageView` provides the same operations without copying
bytes into an owned snapshot. A source-bound `decode_frame` returns exact
frames with stable per-frame errors, with a genuine per-page TIFF path, and
encoded output can be delivered to a caller-owned `OutputSink`; every enabled
still codec now has a Rust-only structural sink witness and rejects before
delivery when its exact output policy fails. JPEG still and one-frame JPEG
sequence, PNG, BMP, GIF, WebP, ICO, TIFF still and multi-page sequence, and
native AVIF still and sequence structural paths plus the other implemented
sequence paths can leave an already-delivered prefix if a later segment fails.
JPEG still and one-frame sequence delivery splits validated marker/scan
segments; WebP
delivery splits its RIFF header from validated chunk headers and
payload/padding spans; TIFF delivery splits its header, each page's
strip/padding span, and IFD/value tail; native AVIF still and sequence delivery splits validated
ISO-BMFF top-level box headers from their non-empty payload spans. All retain their
complete codec working state.
Every current sink path calls `OutputSink::flush` once after complete delivery;
a flush failure is a typed `OutputWrite` and does not roll back the delivered
prefix.
Cross-target determinism is machine-checked: the same SHA-256 golden suite
passes on the ARM64 native host and `wasm32-wasip1`.
Incremental callers get `detect_prefix` and `inspect_basic_prefix` with exact
or progress-aware `NeedMoreData { minimum }` while the input is incomplete,
while the complete-slice APIs keep their terminal classifications.
`decode_prefix` and `decode_sequence_prefix` extend the same non-terminal
status to still and sequence decoding, so callers can decode as soon as the
container and pixel payloads are complete and keep receiving more bytes
otherwise.
Cooperative cancellation (`CancellationToken`) stops token-aware decodes at
structural checkpoints with `ImageError::Cancelled` and no partial state.
Token-aware encodes now cover the public still boundary; PNG additionally polls
adaptive-filter and filtered-row subsegments after each 1,024 row bytes, and
token-aware PNG compression now polls stored-block boundaries, 1,024-byte
stored-block copy intervals, plus every zlib-ng level's matcher, token
expansion, Huffman/bitstream emission, and
Adler-32 stages, while
PNG and BMP also poll row preparation and structural segments in return and sink paths; JPEG
still and one-frame sequence encoding poll internal color/sampling/quantization
optimized baseline Huffman frequency-gathering, progressive scan coefficient,
entropy/progressive-scan
checkpoints, and their structural sinks poll
between validated marker/scan segments; TIFF still encoding polls page
preparation, row prediction,
raw/PackBits/LZW work, and Deflate input-row plus level-six matcher
candidate/insertion/fizzle/position boundaries; sequence
frame/coalescing/page/finalization boundaries are implemented where supported;
native AVIF still and sequence encoding now polls its preparation, frame,
finalization, and top-level box delivery boundaries; ICO still and one-frame ICO sequence sink encoding now poll
source-size validation, embedded PNG work or BMP row assembly, and directory
finalization;
TIFF still and multi-page TIFF sequence sink delivery additionally poll between
the header, each page's strip/padding, and IFD/value segments; lossy WebP still
encoding now polls its RGB/RGBA-to-YUV conversion, RGBA transparent-area cleanup,
macroblock-analysis, and
mode-selection subsegments plus VP8 analysis,
mode-selection, coefficient-probability, 8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, 8,192-bit, 32,768-bit, 65,536-bit, 131,072-bit, and 262,144-bit logical and 16,384-boolean
first-partition-bit, 8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, 8,192-bit, 32,768-bit, 65,536-bit, 131,072-bit, 262,144-bit, 524,288-bit, and 1,048,576-bit logical and 16,384-boolean coefficient-bit intervals, 1,024-byte boolean-bitstream output intervals,
bitstream, and finalization stages, lossless WebP VP8L encoding now polls its
image-palette construction and palette-mode index packing after each 1,024
source pixels, predictor tile scans, mode application, and subtract-green transforms after
each 1,024 pixels, cross-color multiplier search/transform tiles and sampling
scans/compaction, entropy-mode histogram-cost analysis after each 64 symbols,
palette-index lookup candidate scans, palette sign collection and nearest-delta
candidate scans after each 64 palette entries or candidate values,
transform, bounded backward-reference cost/length-table initialization and
length-cost/equal-cost interval setup after each 1,024 entries,
non-saturated interval split/merge after each 1,024 interval-work entries, and
saturated cost-interval fallback scans after each 1,024 entries,
search/match-length/cache/trace, long backward-reference result backfills after
each 256 entries, histogram clustering, Huffman-tree simple-tree symbol-discovery
scans after each 64 code-length slots, Huffman RLE
preparation/tokenization, Huffman-tree insertion scans after each 64 candidate
nodes, Huffman code-length emission after each 16 compressed token entries,
Huffman-tree/group emission, token-stream, and bitstream
stages, while WebP still sink delivery polls
between its RIFF and chunk segments; and other still codecs retain only their
implemented boundary checks. Every
sink path now invokes one finalization flush after complete delivery, with
flush failures reported as `OutputWrite` and no rollback. The first public
`EncodePolicy::max_work_units` contract now bounds those documented
checkpoints and reports `ResourceLimit::EncodeWorkUnits`; remaining
sequence-structural/interior interruption beyond the implemented PNG row,
token-aware PNG stored-block and all-level Deflate checkpoints, BMP
row-conversion subsegments, GIF RGB/RGBA palette quantization, RGBA FASTOCTREE
bucket-sort intervals, and LZW input-symbol intervals, WebP
RGB/RGBA-to-YUV conversion, RGBA transparent-area cleanup, macroblock-analysis,
and mode-selection subsegments, WebP stages, 8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, 8,192-bit, 32,768-bit, 65,536-bit, 131,072-bit, and 262,144-bit logical and 16,384-boolean first-partition-bit,
8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, 8,192-bit, 32,768-bit, 65,536-bit, 131,072-bit, 262,144-bit, 524,288-bit, and 1,048,576-bit logical and 16,384-boolean coefficient-bit intervals, 1,024-byte boolean-bitstream output intervals,
JPEG progressive scan coefficient traversal after each 1,024 coefficients,
and the 8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, 8,192-bit, 16,384-bit, 32,768-bit, 65,536-bit, 131,072-bit, 262,144-bit, 524,288-bit, and 1,048,576-bit logical VP8L bitstream intervals; finer WebP bitstream work
beyond those intervals, other Deflate
emission/structural interruption, progress, transient
allocation accounting, and universal
streaming semantics remain open.
Successful decodes now carry stable non-fatal diagnostics for accepted
recoveries, invalid compressed PNG ancillary metadata, an accepted bad `IDAT`
CRC, an accepted bad `IEND` CRC, an accepted invalid PNG reserved-bit
character, an accepted unknown
ancillary chunk after `IDAT`, an accepted static PNG stream without `IEND`,
accepted duplicate `PLTE`/`tRNS` chunks, tolerated indexed-palette shape damage,
a zero-frame APNG declaration, an out-of-range APNG frame count, malformed
duplicate `acTL` and `acTL`-after-`IDAT` declarations that fall back to the
default image, an overlong `acTL` payload,
valid inflated bytes beyond the first raster, and ignored trailing input; valid
APNG control and frame-data chunks are excluded from the static ordering
diagnostic. Their Rust-only fields are
tested through a separate defensive-model manifest rather than the Pillow
parity matrix. Unsupported valid-shape non-zero
PNG `zTXt`/`iCCP` compression methods remain fatal with `png_chunk` context;
that Pillow-observable boundary is asserted separately without a synthetic
diagnostic field. Primary AVIF CICP declarations
are likewise retained as source provenance through the bounded item parser; the
dedicated contract test is defensive/specification evidence and does not add a
synthetic parity row. Primary AVIF `irot`/`imir`/`pasp`/`clap` declarations now
follow the same source-provenance path; their legal values are validated without changing
decoded pixels, and their contract test likewise adds no synthetic parity row.
Primary AVIF `clli` content-light-level declarations now follow the same
source-provenance path through `SourceColor`; maxCLL and maxPALL are retained
without tone mapping, and the contract test likewise adds no synthetic parity
row.

The COR-061 provenance audit counts 61 diagnostic cases: 38 use committed bytes
that also have a Pillow parity row, so those rows prove only the shared outer
success/pixel result; the other 23 cases construct runtime mutations that are
not matrix rows. The separate diagnostic test keeps asserting the Rust-only
recovery records and baseline-preservation invariant, without adding a
coverage-only hook or synthetic `diagnostics` field to the Pillow matrix.
`scripts/verify_diagnostic_provenance.py` hashes every baseline and requires
exactly one active Pillow row with the same format, asset digest, successful
operation, and `pillow_fixture` operation origin; it also validates the named
runtime-mutation set and the schema boundary. This makes the distinction
machine-checkable: the diagnostic contract is a normal public Rust behavior
test, while any lines it executes appear only in aggregate LLVM coverage and
are not Pillow-parity coverage.

This boundary is required by the oracle schema, not just by test organization:
the generated Pillow matrix has no successful-decode warning field from which
to derive `DiagnosticKind`, stage, offset, or structure identity. A future
parity row for one of the 23 mutations could compare only that mutation's
Pillow outer result; it would still not validate the Rust diagnostic and would
be a separate outer-result expansion. The current 38 unchanged cases already
have active outer-result rows, so COR-061 remains a normal fixture-backed
Rust defensive contract whose execution is incidental aggregate coverage.

The original COR-061 acceptance record at implementation revision
`5c129baba0bfa044b0b79d3842af69736b269519`
was accepted by Coverage MCP run `4f4cc8a0-c716-4667-8720-f0d96e1b77d5`, snapshot
`24fe9c12-7cf7-4f2b-ac41-a1eda7e88828`: 72 tests passed with zero failures,
retaining 48,615/48,938 lines, 6,659/6,714 branches, 2,727/2,783 functions,
and 75,687/76,106 regions. The feature matrix passed 947/947 checks in run
`50c67cfb-d97b-425e-8afc-7508cefd1b90`, with zero package-cache or
build-directory lock waits, and the unchanged Pillow parity matrix passed
1,434/1,434 checks in run `2b13beca-1b3c-471f-b8a9-c386f594427d`; neither
result treats the Rust-only diagnostic fields as Pillow parity. The diagnostic
contract passed in all 22 retained feature-lane executions, and the provenance
verifier passed separately.
Historical COR-061 revalidation is against implementation revision
`f7a8cd7efdf398c4df564ea29ffa2fcc99e6afdf`. Managed feature-matrix run
`42260e83-2f2b-4d7b-9219-76c415a43f0c` passed 991/991 checks; its retained log
contains 22 successful executions of
`diagnostic_manifest_matches_the_non_parity_contract`, one per feature lane,
with no build-directory or package-cache lock-wait matches. Managed Coverage
MCP run `f95bdb91-394f-461e-bc13-ea970997de88` passed 85/85 tests in 69,986 ms
and ingested snapshot `109c8920-2045-4cfb-a894-b2e2842ccfbc`. The managed
Pillow parity run `6e993f5a-d280-4fc5-8191-41086674d433` passed 1,445/1,445
outer-result checks separately; it contains no diagnostic field or claim.
Those records revalidate COR-061 without converting Rust-only
diagnostic execution into Pillow-parity coverage.
Primary AVIF `av1C` chroma sample-position declarations now follow the same
source-provenance path through `SourceColor`; the four legal two-bit codes are
retained as `AvifChromaSamplePosition` without changing decoded pixels or
performing chroma resampling. The contract uses the committed baseline AVIF as
a source witness and adds no synthetic parity row. Primary AVIF `colr` ICC
profiles now follow the same source-provenance path
through `SourceColor`; both `prof` and `rICC` retain their exact profile kind
and bytes, with no color conversion. The contract uses the committed
Pillow-generated encoded metadata output only as a source witness, so it adds
no synthetic parity row; primary AVIF `mdcv` mastering-display fields now follow
the same path through `SourceColor` and are checked by the same defensive
contract. Recognized AVIF `Exif` and XMP item payloads now follow an ordered
raw `OpaqueMetadata` path on still and sequence decode; the committed encoded
metadata output is only a source witness, and no synthetic parity row is added.
Their EXIF bytes include the stored AVIF TIFF-header offset prefix. Non-primary/
auxiliary metadata relationships remain open.
The bounded feature-matrix runtime optimization was benchmarked by run
`f74e711f-c9a2-4327-bc74-d834b6bf399a` at the pre-JPEG harness revision: it
passed 903 checks with zero failures in 298,766 ms, and its terminal log records
`capability tables OK: every native and wasm32-wasip1 lane agrees`. That was
52,789 ms faster than the previous managed run
(`e7755afd-eedf-4fe7-b56d-f24ea54a55e1`, 351,555 ms) with the same 903-check
scope. The current clean revision was then validated by run
`bea69012-22a4-4b55-9ef9-e3859c73ef2e`: 903 checks passed with zero failures in
1,296,952 ms, with the same capability-table terminal record. The two timings
are retained as separate execution evidence rather than treated as a controlled
benchmark because managed cache/build state differs.
The capability probe now lives in the feature-gate integration target, so the
matrix does not compile a second target for the final capability-table check.
Run `0433b3d0-110e-4242-a088-c7acbc3cefa2` passed 925 checks with zero failures
in 865,050 ms and retained the same terminal capability-table record; its 22
additional checks are the reused probe in each native/WASI lane. The run was
submitted before the worktree patch was committed as `45e1922`, so its recorded
parent revision is retained as execution provenance rather than a clean
revision comparison.
The matrix scheduler now admits the next lane as soon as any bounded worker
finishes instead of waiting for a whole launch batch. Run
`de0619ff-e117-4d9d-bc3e-e9ee7fff01bf` passed the same 925 checks with zero
failures in 298,267 ms and retained the same terminal capability-table record.
It was submitted against parent revision
`65c3a4b5714f118e93b62b07b899f2ddc1c64d04` with the scheduler patch uncommitted;
that patch is committed as `766a6dd`. The timing is retained as runtime
evidence, not a controlled speed comparison, because managed cache/build state
differs.
The PNG/BMP still-token slice was then validated by run
`a545e1bb-ec85-4f8d-93c1-3e0e778907c2`: 925 checks passed with zero failures in
1,102,400 ms and retained the same terminal capability-table record. That run
was submitted against parent revision
`66f6159c39f6deae1c98d8bf3da5277f76a2d780` before the implementation commit;
it remains execution provenance rather than a clean revision comparison
because managed cache/build state differs.
The GIF still-token slice was then validated by run
`82750f5a-cad6-4f87-b538-adf6d1e21c29`: 925 checks passed with zero failures in
1,018,825 ms and retained the same terminal capability-table record. That run
was submitted against parent revision
`7e684afa53e45a100ea91a00b1acd1bee7c38ebc` before the implementation commit
`cc1d5c8`; it remains execution provenance rather than a clean revision
comparison because managed cache/build state differs.
The WebP still-token slice was then validated by run
`d323e738-d2ce-4523-bec8-563c2421ad0a`: 925 checks passed with zero failures
in 1,093,331 ms and retained the terminal capability-table record. It ran
against clean revision `2ffb338217cfb71223fb81dfe3b0cdf59b9f9aed`; the
duration is execution evidence rather than a controlled speed comparison
because managed cache/build state differs.
The feature-matrix harness then moved each bounded lane to a temporary
lane-local Cargo target root and pointed the capability probes at those same
roots. Run `91155ed7-9729-4877-8433-d14146428137` passed 925 checks with zero
failures in 110,405 ms, retained the same terminal capability-table record,
and had no build-directory lock-wait records. This is harness correctness and
contention evidence rather than a controlled speed comparison because managed
cache/build state differs. The final capability-table check now consumes the
single row emitted by each full lane test instead of relaunching 22 one-test
probe processes. Run `120f465d-fd8b-43af-8ce2-76497d99fb80` passed the same 925
checks with zero failures in 77,855 ms, retained the same table record, and
again had no build-directory lock-wait records. The observed difference is
execution evidence rather than a controlled speedup claim because managed
cache/build state differs.
The bounded lane-concurrency follow-up then raised the default worker bound
from three to four. Run `5e438aba-378e-4a33-b03f-d4ecd047865e` passed the same
925 checks with zero failures in 67,609 ms, retained the capability-table
record, and again had no build-directory lock-wait records. This remains
execution evidence rather than a controlled speedup claim because managed
cache/build state differs.
The native AVIF still-cancellation slice was then validated by run
`0fe32478-6653-485d-a550-9c914b0d6d2a`: 925 checks passed with zero failures
in 112,375 ms, the `encode_cancellation_is_a_non_parity_contract` test passed
in the retained feature lanes, the capability-table record remained intact,
and no build-directory lock-wait record was present. The test proves a
pre-cancelled AVIF still encode returns the typed `Cancelled` error and that
an uncancelled token-aware encode remains byte-identical to the legacy call.
This is an ordinary Rust contract using a real AVIF fixture, not a Pillow
parity row or a coverage-only hook; portable WASM AVIF encoding remains
target-unavailable. The duration is execution evidence rather than a
controlled speed comparison because managed cache/build state differs.
The ICO still-cancellation slice was then validated by run
`991a26ef-f7a6-40be-b2bb-c98be087bcce`: 925 checks passed with zero failures
in 116,267 ms, the `encode_cancellation_is_a_non_parity_contract` test passed
in all 22 retained feature lanes, the capability-table record remained intact,
and no build-directory lock-wait record was present. The test proves that ICO
still encoding preserves uncancelled bytes and returns a typed `Cancelled`
error before publishing output when the caller token is pre-cancelled. This is
an ordinary Rust contract using the real ICO fixture, not a Pillow parity row
or a coverage-only hook.
The one-frame BMP sequence sink slice was then validated by run
`caeb1194-d307-4305-9e87-e0eef94b205a`: 925 checks passed with zero failures in
82,978 ms, the capability-table record remained
`capability tables OK: every native and wasm32-wasip1 lane agrees`, and the
retained log had no target-directory lock-wait records. Its structural sink,
option-mismatch, multi-frame rejection, exact-length policy, and sink-delivery
assertions are ordinary Rust output contracts; the Pillow parity matrix is
unchanged.

The feature-matrix harness now retains each isolated lane target root between
invocations by default under `target/feature-matrix`; `MATRIX_TARGET_ROOT` can
select a disposable or cold root. A clean population run at commit `a518776`
(`283eef63-e5ee-49d5-ad14-5f775e4c6ac5`) passed 925 checks in 99,851 ms, and
its warm repeat (`4a1f025a-f014-4fcb-b716-e7bfbec95f29`) passed in 17,289 ms.
At the pre-work-budget final source revision after the ICO coverage-edge commit
`ecbd9c2e3f17491f55737ad10a4518bf19518a91`
(`f9dbed4a-b416-4966-93af-5922a7d8bd77`) passed in 61,916 ms while rebuilding
changed lanes; its warm repeat (`6a22af78-9666-4bc9-a936-9d82cf9110ca`) passed
in 15,766 ms. Every run passed the terminal capability record
`capability tables OK: every native and wasm32-wasip1 lane agrees`, with zero
`Blocking waiting for file lock on build directory` matches. Package-cache
waits can still occur while lanes initialize. The timings are execution and
cache-retention evidence, not a universal benchmark claim.
The test-thread and completion-scheduler follow-up was validated on committed
revision `cb0f67d2e76e99eefc2595317fd49fb5202a7162` by run
`d91c3f7c-9487-4648-a575-9737e443b2b0`: 947 checks passed with zero failures
in 14,236 ms. It retained the same terminal capability record and had zero
build-directory lock-wait matches; package-cache lock waits remain observable
while isolated lanes initialize. The previous warm run with the same 947-check
scope (`1eff0861-ffde-4be0-96c7-b297dea9384c`) took 15,307 ms. This is observed
runtime evidence rather than a universal benchmark claim because managed
cache/build state can differ. The harness derives `--test-threads` from host
CPUs and the lane bound (capped at eight), and interleaves native,
`wasm32-unknown-unknown`, and `wasm32-wasip1` lanes under one
completion-driven scheduler without dropping a lane or assertion.

The next scheduling follow-up makes that lane bound host-aware: by default
`MATRIX_JOBS` uses roughly two logical CPUs per active lane, capped at six,
while retaining explicit `MATRIX_JOBS`, `MATRIX_TEST_THREADS`, and
`MATRIX_BUILD_JOBS` overrides. The latter is exported as `CARGO_BUILD_JOBS`
inside each lane so concurrent Cargo compiler fan-out follows the same bound.
Runs `790238ad-e8d8-4fce-9974-71560ffaac5d` and
`53c23521-c3d1-4b4b-9914-4b8d8f50883c` passed all 947 checks in 13,136 ms and
12,116 ms. The four-lane baseline `91c9bc98-5f22-41d2-95ad-d981957f1f82`
passed the same scope in 16,844 ms on the same managed environment. These are
observed scheduling results rather than universal benchmark claims; the
capability-table record remained unchanged and the retained logs had no
build-directory lock waits. The committed revision `125b1b0` then passed the
same scope in runs `b016dd0f-6460-4bcf-8add-765b6ec8a8ee` (16,317 ms) and
`b4bf4180-f72c-4969-a66e-c355c402d9ac` (11,648 ms), confirming the change while
also showing the managed runner's timing variance.
The sink-finalization follow-up was validated on committed revision
`775263335df9680e4c453f666708745f53083e8f` by run
`6ef08e71-abcf-4841-b30f-649529bb3bfc`: 947 checks passed with zero failures
in 65,458 ms, retained the same terminal capability record, and had zero
build-directory lock-wait matches. This is execution evidence rather than a
controlled speed comparison because the managed cache state differed.
Coverage MCP then ingested run
`9bbe6760-7aa9-4ed8-8b31-bbf65444b85a` as snapshot
`f9a2fc69-ad68-493e-9c46-8837d0dd8d52`: 58 tests passed with zero failures,
with 100% line, branch, function, and region coverage (47,943 lines, 6,578
branches, 2,686 functions, and 74,654 regions). The ICO still and one-frame
ICO sequence sink paths, the deterministic encode work-budget contract, and
their real defensive/error branches, including sink finalization failures, are
covered through ordinary Rust contracts; aggregate coverage remains
implementation evidence rather than Pillow-parity coverage. The preceding
committed acceptance revision was `07f7a0977149803f96eec16ac8c2f3c1cb073eee`:
Coverage MCP run `822bf053-61cb-4488-af1c-d2e23b15785c`, snapshot
`512dce77-6eda-4b2d-b8aa-9cbfcdd6a8a6`, passes 58 tests with zero failures or
skips and reports 47,977/47,977 lines, 6,582/6,582 branches, 2,687/2,687
functions, and 74,704/74,704 regions. The feature matrix run
`0d19674c-4a01-4a06-9e54-2831a16c10d7` passes 947/947 checks, and the Pillow
parity run `888ba305-ff93-41c4-8d96-05c12f033c64` passes 1,420/1,420 rows;
those matrices remain separate from the Rust-only TIFF work-budget contract.
Aggregate coverage and runtime matrix results are implementation evidence, not
Pillow-parity coverage.

The preceding committed TIFF still/one-frame structural-sink acceptance revision is
`8e2b3e82d11c8aacfc8f2b05a3931d4464412d53`. Coverage MCP run
`c6231d19-598d-4706-bdfd-9385e3c05b50`, snapshot
`62014cef-25be-485e-a32f-ee1f9e9b606d`, passes 58 tests with zero failures and
reports 48,061/48,062 lines, 6,588/6,588 branches, 2,692/2,693 functions,
and 74,819/74,826 regions. The new `src/codecs/tiff/encode.rs` file is fully
covered for lines, branches, and functions (1,321/1,323 regions); the
aggregate snapshot retains one uncovered line, one function, and seven
regions, and no coverage-only test was added. Feature-matrix run
`a0cc505e-f44f-4b9e-9667-de52dca995b8` passes 947/947 checks in 35,146 ms;
Pillow parity run `d39ff85a-6d2e-41e8-b453-b4356943e3ff` passes 1,420/1,420
rows with zero skips in 33,569 ms. These durations are execution evidence
rather than a controlled runtime comparison; the sink and policy/cancellation
assertions remain ordinary Rust-only contracts, separate from Pillow parity.

The parity-harness runtime follow-up is committed at revision
`ba06d91625ca72f81e94c0951ab6904b03e75ff6`. The active manifest remains 1,417
rows; eight independent format-scoped decode tests and eight encode tests now
run concurrently, with per-row success output opt-in through
`IMAGE_SLASH_STAR_VERBOSE_MATRIX=1`. Coverage MCP run
`aaaf3d94-a362-4cc2-9609-8b930d60f583`, snapshot
`6d386562-ad46-4358-929b-e5b66dcd58ba`, passed 72 tests with zero failures and
retained 48,061/48,062 lines, 6,588/6,588 branches, 2,692/2,693 functions,
and 74,819/74,826 regions. The feature matrix passed 947 checks in
`e38b36f5-a130-48bc-92ea-388fda6893b2` (13,762 ms), and the parity run passed
1,434 checks with zero failures and zero skips in
`608a6de1-a9d0-4820-8ffd-7287267f16f2` (53,544 ms). Retained parity output
reports 17 tests in 12.80 s versus 3 tests in 33.48 s on the preceding run;
these are execution records rather than a universal wall-time benchmark, and
the manifest, fixtures, row assertions, and Pillow provenance are unchanged.

The JPEG work-budget contract is committed at revision
`df03084d90c790993a49364359ef31f11ebc50a2`. It is ordinary Rust-only
evidence: the ample policy preserves JPEG bytes, a bounded policy rejects
mid-encode after more than one checkpoint, and a zero-budget generic sink
remains untouched. Coverage MCP run
`6309b1ae-4e4e-482d-9ee2-7472522bae19`, snapshot
`bc799a47-9076-4c8f-ab2b-65b0cbd7c0d7`, passed 72 tests with zero failures and
retained 48,061/48,062 lines, 6,588/6,588 branches, 2,692/2,693 functions,
and 74,819/74,826 regions. The feature matrix passed 947 checks in
`70861b62-21e4-4aad-a4e0-249a1dc23d09` (40,936 ms), and the unchanged Pillow
parity scope passed 1,434 checks with zero failures and zero skips in
`66d39cf5-514a-46d9-b7a3-6ee4b7651c30` (23,106 ms); no parity row or fixture
was added for the caller-controlled budget.

The work-budget precedence follow-up is committed at revision
`754416b786be09803991b5f04c1d275de49b299a`. It proves that a pre-cancelled
caller token takes precedence over `EncodeWorkUnits` exhaustion for still PNG
and sequence GIF, including the no-write sink boundary. This is ordinary
Rust-only evidence: Pillow has no caller token or checkpoint budget, and no
parity row or fixture was added. Coverage MCP run
`525f42b2-2cb9-49be-8e65-063eec7a0256`, snapshot
`31401e79-faa2-4244-add2-5697811a08d9`, passed 72 tests with zero failures and
retained 48,061/48,062 lines, 6,588/6,588 branches, 2,692/2,693 functions,
and 74,819/74,826 regions. The feature matrix passed 947 checks in
`30593a22-9120-4319-9552-0ae7a68be7b7` (48,022 ms), and the unchanged Pillow
parity scope passed 1,434 checks with zero failures and zero skips in
`531ac749-7aaa-4910-bfed-262e1eb66a20` (33,095 ms).

The WebP still and one-frame sequence structural-sink slice is implemented at
revision `e632222badda34fb29913473556da99b8128d0f8`; the follow-up feature-gate
fix is `63d801c93eabee36e8ec87f22ad20df940283be7`, the sequence dispatch
extension is `93a790a53f806baafd7d5a9c9b0376c7e93e54da`, and the final
multi-frame fallback guard is `745c0af6bc4f4d10ddfebcafa8ef131d88097811`. It
retains the complete WebP working buffer but delivers a validated RIFF header
followed by chunk headers and payload/padding spans, with exact-length
preflight and cancellation between segments for both still and one-frame
sequence stages. At that revision, multi-frame WebP remained the generic
whole-buffer path.
This is ordinary Rust-only sink evidence because Pillow has no caller-owned
destination. Coverage MCP run
`c92f3ac8-7122-487e-a374-a97f9a497813`, snapshot
`2d14db3c-a464-4768-960b-ec6d4c8e8c00`, passed 72 tests with zero failures and
retained 48,169/48,208 lines, 6,603/6,610 branches, 2,698/2,710 functions,
and 74,985/75,042 regions. The corrected feature matrix passed 947 checks
with zero failures in run `b480a67a-f626-4656-aefa-3a47e8521a32` (119,749 ms),
and the unchanged Pillow parity scope passed 1,434 checks with zero failures
and zero skips in run `5196a8d9-7c7b-43b8-b621-1a1a1812ebfa` (79,583 ms); no
parity row or fixture was added.

The feature-matrix compiler-budget follow-up is committed at revision
`87510c76b1bfdafb8bde97d9d8b00427ee428a10`. The harness now records its
selected `lanes=6 test_threads=2 build_jobs=2` budget and exports the bounded
compiler-job count inside every native and WASM lane; no lane, target, or
assertion was removed. Runs `2dac27fc-8b57-401e-a29f-14f78b771813` and
`4d040ed1-79a3-45e0-9eba-1bb794638808` each passed all 947 checks with zero
failures in 14,818 ms and 11,871 ms. Their retained logs contain zero
`Blocking waiting for file lock on build directory` matches; package-cache
waits remain possible while independent lanes initialize. These are same-scope
execution records rather than a universal speedup claim because managed cache
and runner state can differ. The change affects feature-matrix scheduling only;
the Pillow parity manifest, fixtures, row assertions, and provenance boundary
are unchanged.

The GIF still and sequence structural-sink slice is committed at revision
`3f70c5e5e79d8756cd9c590d6fdadd02b82ff238`. It retains the complete GIF
working buffer but delivers the validated signature/logical-screen descriptor,
color tables, extension and image sub-blocks, and trailer as separately
cancelable sink segments after exact output-length preflight for both still and
sequence stages. This is an ordinary Rust-only destination contract because
Pillow has no caller-owned sink; no parity row or fixture was added. Coverage
MCP run `96d01110-737c-40e4-9db3-d976f456e4ac`, snapshot
`626b4ff9-fdeb-4497-ad78-25e26a45368f`, passed 72 tests with zero failures in
180,835 ms and retained 48,340/48,504 lines, 6,619/6,638 branches,
2,709/2,747 functions, and 75,292/75,509 regions. The feature matrix passed
947/947 checks with zero failures in run
`b9267f1d-be16-4214-a2a9-86f129354213` (112,313 ms), and the unchanged Pillow
parity scope passed 1,434/1,434 checks with zero failures and zero skips in run
`9625c19e-86ad-4365-835d-f76c2d5a6b33` (69,210 ms). The feature-matrix log
records `lanes=6 test_threads=2 build_jobs=2`, no build-directory lock waits,
and the terminal native/WASI capability-table agreement; package-cache waits
remain possible. These durations are execution evidence rather than a
universal speed comparison because managed cache and runner state can differ.

The multi-frame WebP structural-sink slice is committed at revision
`ea96e6cb7a2f2e846f251944f4e182e8cab8ef22`. It extends the existing RIFF
delivery parser from still and one-frame sequence output to animated WebP:
the complete animation working buffer is retained, while the RIFF header and
validated chunk headers/payloads/padding are delivered as separately
cancelable segments after exact output-length preflight. This is an ordinary
Rust-only destination contract because Pillow has no caller-owned sink; no
parity row or fixture was added. Coverage MCP run
`ba892b20-8a96-45e6-ae1c-7f7497752631`, snapshot
`613b3652-9444-4c09-8833-8913de472e51`, passed 72 tests with zero failures in
76,095 ms and retained 48,366/48,539 lines, 6,620/6,642 branches,
2,711/2,749 functions, and 75,318/75,552 regions. The feature matrix passed
947/947 checks with zero failures in run
`831cdfc1-1c26-4584-a39e-e13fead8d2fa` (102,142 ms), and the unchanged Pillow
parity scope passed 1,434/1,434 checks with zero failures and zero skips in run
`90f11ef9-cc48-4e9f-a036-1f6017ad25d3` (72,614 ms). The feature-matrix log
records `lanes=6 test_threads=2 build_jobs=2`, no build-directory lock waits,
and the terminal native/WASI capability-table agreement; package-cache waits
remain possible. These durations are execution evidence rather than a
universal speed comparison because managed cache and runner state can differ.

The JPEG still structural-sink slice is committed at revision
`df2053ffec2a1c84d0b2d2fb1bd90f91f16cc001`. It retains the complete JPEG
working buffer but delivers the validated SOI/marker segments, SOS headers,
entropy-coded scan spans, restart markers, and EOI as separately cancelable
segments after exact output-length preflight. Progressive JPEG output uses the
same marker/scan parser. This is an ordinary Rust-only destination contract
because Pillow has no caller-owned sink; no parity row or fixture was added.
Coverage MCP run `d95983e6-b73b-42b3-aa0a-38b162069320`, snapshot
`165e14d0-c196-4e0a-8902-b83aa23f3e41`, passed 72 tests with zero failures in
76,006 ms and retained 48,521/48,761 lines, 6,645/6,688 branches,
2,722/2,765 functions, and 75,541/75,851 regions. The feature matrix passed
947/947 checks with zero failures in run
`de84cca3-57ad-4fab-9d41-ff48fd6d4c24` (57,822 ms), and the unchanged Pillow
parity scope passed 1,434/1,434 checks with zero failures and zero skips in run
`592f4dca-87e5-49d2-a9be-a4441380a66c` (60,047 ms). The feature-matrix log
records `lanes=6 test_threads=2 build_jobs=2`, no build-directory lock waits,
and the terminal native/WASI capability-table agreement; package-cache waits
remain possible. These durations are execution evidence rather than a
universal speed comparison because managed cache and runner state can differ.

The native AVIF still structural-sink slice is committed at revision
`6d708e243103ff27bcc59d3296b1225ae23d9783`. It retains the complete native
encoder buffer but delivers validated ISO-BMFF top-level box headers and
non-empty payload spans as separately cancelable sink segments after exact
output-length preflight. This is ordinary Rust-only destination evidence:
Pillow has no caller-owned sink, so no parity row or fixture was added, and
portable WASM AVIF encoding remains target-unavailable. Coverage MCP run
`53b8ef0b-b5df-45d5-8413-da55eb0c72cb`, snapshot
`58d4ba5a-2413-47a3-b9d3-a51eb869d1a5`, passed 72 tests with zero failures and
reports 48,585/48,903 lines, 6,656/6,710 branches, 2,725/2,781 functions,
and 75,648/76,063 regions. The feature matrix passed 947/947 checks with
zero failures in run `748dc95d-0fa8-45d7-97d1-581f658e6684` (105,976 ms),
and its retained log records `lanes=6 test_threads=2 build_jobs=2`, no
build-directory lock-wait match, and native/WASI capability-table agreement;
package-cache waits remain possible. The unchanged Pillow parity scope
passed 1,434/1,434 checks with zero failures and zero skips in run
`8947fe4d-99bc-4b2e-977e-16ec7b954c88` (76,644 ms). These are execution
records rather than a universal runtime comparison; the sink assertions are
not Pillow parity coverage.

The native AVIF sequence structural-sink slice first landed at revision
`81dae9af403dfa7358dfd833b25ef9c032582b5a` and was accepted at the preceding
revision `5c129baba0bfa044b0b79d3842af69736b269519`. It retains the complete
native encoder buffer, validates ISO-BMFF top-level boxes, and delivers each
box header and non-empty payload span as separate sink segments after exact
output-length preflight. Cancellation is checked between those segments, and
the sequence encoder also checks frame and finalization boundaries. This is
ordinary Rust-only destination evidence: Pillow has no caller-owned sink, so
no parity row or fixture was added, and portable WASM AVIF encoding remains
target-unavailable. Coverage MCP run
`4f4cc8a0-c716-4667-8720-f0d96e1b77d5`, snapshot
`24fe9c12-7cf7-4f2b-ac41-a1eda7e88828`, passed 72 tests with zero failures in
65,498 ms and reports 48,615/48,938 lines, 6,659/6,714 branches,
2,727/2,783 functions, and 75,687/76,106 regions. The feature matrix passed
947/947 checks with zero failures in run
`50c67cfb-d97b-425e-8afc-7508cefd1b90` (15,200 ms), and the unchanged Pillow
parity scope passed 1,434/1,434 checks with zero failures and zero skips in run
`2b13beca-1b3c-471f-b8a9-c386f594427d` (12,864 ms). These are execution
records rather than a universal runtime comparison; the sink assertions are
not Pillow parity coverage.

The runtime-first feature-matrix follow-up is committed at revision
`5c129baba0bfa044b0b79d3842af69736b269519`, after the bounded compiler-job
revision `87510c76b1bfdafb8bde97d9d8b00427ee428a10`. The harness now fetches
the locked host and WASM dependency graphs before lane fan-out, runs the
lanes offline with lane-local target roots and the selected
`lanes=6 test_threads=2 build_jobs=2` budget, gives each concurrent lane a
stable lane-scoped Cargo home that shares only the fetched registry sources,
and persists each capability row from the full native/WASI lane instead of
launching 22 duplicate probe processes. The diagnostic contract also reuses
immutable fixture bytes and baseline decodes within each feature process. The
managed run above retained zero package-cache or build-directory lock waits
and the terminal record `capability tables OK: every native and
wasm32-wasip1 lane agrees`; all 947 checks and feature assertions remain.
This is runtime and harness evidence rather than a controlled speedup claim
because managed cache and runner state can differ.

The final FTR-032 source revision also passed feature-matrix run
`1a0c0f1c-d5d7-4210-a24f-503d001a3d8f` with 947 checks and zero failures, and
Pillow parity run `4ed3cd5c-3e92-4f2b-bd02-1b71a97ad0ed` with 1,420 rows and
zero failures. These durations are execution evidence rather than a controlled
benchmark because managed cache and build state can differ.

The preceding implementation revision was
`7d735af15cc448bd1be76b1569c317b8dcd0d9e7`. The runtime-first parity follow-up
is committed at `8c87e1d`: the active manifest remains 1,417 rows, while the
expensive GIF/WebP encode work is partitioned into hot-row workers with
repeated source assets kept together. Managed parity run
`57b0915e-c5ab-4a67-807e-f2481b1caa03` passed 1,445 checks with zero failures
or skips in 53,645 ms. Its retained output reports eight decode workers and
nineteen encode workers, all with zero failed or skipped rows; the managed
count is 1,417 active rows plus 28 worker test functions. This is scheduling
evidence rather than a controlled speedup claim because runner and cache state
vary, and the manifest, fixtures, assertions, and Pillow provenance are
unchanged. The current managed parity run
`a7791521-25e0-405e-9826-c0f3c3745d6c` also passed 1,445 checks with zero
failures or skips in 60,053 ms at `7d735af`; its 28 worker tests reported zero
failed or skipped rows.

The multi-page TIFF structural-sink slice landed in `128406f`, with the
feature-gating correction in `2147fbf`. TIFF sequence sink delivery now
validates and preflights every page, relocates the page IFD chain, and emits
the header, per-page strip/padding, and IFD/value spans as separately
cancelable writes. Its Rust-only acceptance test proves exact whole-buffer and
sink bytes/length, policy rejection before the first write, and cancellation
after the initial TIFF header. Pillow has no caller-owned `OutputSink`, so no
parity row or fixture was added and no new coverage-only hook was needed.

The one-frame JPEG sequence sink slice landed in `7d735af`. It reuses the
validated JPEG marker/scan structural writer with `SequenceEncode` context;
its Rust-only acceptance test proves exact whole-buffer and sink bytes/length,
token-aware sink byte identity, cancellation after the initial `ff d8` prefix,
encoded-output policy rejection before the first write, and explicit
multi-frame rejection. Pillow has no caller-owned `OutputSink`, so no parity
row or fixture was added and no new coverage-only hook was needed.

Coverage MCP run `e0190ba7-9e19-43ea-b40d-204401d503f8` passed 83 tests with
zero failures and ingested snapshot
`5ec8f2da-7df0-4b87-a30b-bd91ac986825` at this revision. It reports
48,738/49,108 lines, 6,671/6,732 branches, 2,735/2,802 functions, and
75,913/76,394 regions. Feature-matrix run
`918b49db-4fe3-4a7f-8451-3d8823c9baf6` passed 947/947 checks in 99,097 ms;
retained logs show no package-cache or build-directory lock-wait matches and
end with `capability tables OK: every native and wasm32-wasip1 lane agrees`.
These are implementation and target-evidence records, separate from Pillow
parity.

The runtime optimization acceptance revision was
`045a908a580024212a03a1bb96dd83bdc27aa4ba`. The test-runtime follow-up adds a
lightly optimized Cargo test profile (`opt-level = 1`) for the codec-heavy
parity and coverage binaries and keeps the compile-heavy feature probes at
`MATRIX_TEST_OPT_LEVEL=0`. It changes no production profile, manifest row,
fixture, assertion, or Pillow/Rust evidence origin. Managed parity run
`88c2db36-221f-4b1c-bb60-17a04cf12d70` passed 1,445/1,445 checks in 844 ms;
Coverage MCP run `58803e1c-2c6d-401d-9376-825710e8a2cf` passed 83/83 tests in
48,676 ms and ingested snapshot `a893e8ad-895b-40cb-9106-f776d44b62a8` with
the same 48,738/49,108 lines, 6,671/6,732 branches, 2,735/2,802 functions,
and 75,913/76,394 regions. Feature-matrix run
`6c079600-9d20-4ed9-92a0-517068587d84` passed 947/947 checks in 56,641 ms;
its retained log has no package-cache or build-directory lock-wait matches
and ends with `capability tables OK: every native and wasm32-wasip1 lane
agrees`. These are observed execution records, not universal benchmark
claims.

The current PNG interior work-budget slice is implemented at
`0e647e9b3eab31b704b7d2262525ab90a2f835e5`: adaptive filter scoring and
filtered-row emission charge a checkpoint after each 1,024 row bytes in still
and one-frame sequence paths. Its acceptance remains Rust-only because Pillow
exposes neither a caller token nor a work-budget result; the test proves the
typed `EncodeWorkUnits` error and untouched sink before any structural write.
The no-token encoder path is unchanged, so this slice adds no Pillow parity
row, fixture, or diagnostic origin. Managed parity run
`bad36d4a-c88f-4384-91ba-5f9df79eea6e` passed 1,445/1,445 checks in 761 ms.
Coverage MCP run `af8efaba-fdb4-4c89-bb4d-577a9881a958` passed 83/83 tests in
44,235 ms and ingested snapshot
`a00cbf8e-c8f8-491e-981f-95ab9a34c358`; it reports 48,755/49,125 lines,
6,679/6,740 branches, 2,734/2,801 functions, and 75,938/76,421 regions.
The changed PNG encoder file is 599/599 lines, 62/62 branches, 44/44
functions, and 1,014/1,016 regions; Coverage MCP records the LLVM segment
normalization warning for aggregate regions. Feature-matrix run
`a1a01a8d-f719-42b7-930e-ffcc97273c36` passed 947/947 checks in 64,537 ms;
retained logs show no package-cache or build-directory lock-wait matches and
end with `capability tables OK: every native and wasm32-wasip1 lane agrees`.
These are observed implementation and target-evidence records, separate from
Pillow parity. Remaining interior work in other codec rows, deeper
Deflate/structural interruption, allocation accounting, and rollback are
still open.

The current lossy WebP/VP8 work-budget slice is implemented at
`a5c39499a33f06668fb145abf6d6051344f6ba3f`, with its RGB/RGBA contract test
at `90fcc0f0ea2ee8b4ad861e6bf591d359b47d1833`: token-aware VP8 encoding now
charges checkpoints after color conversion, padding, analysis, segment
parameters, mode selection, coefficient-probability adaptation, partition
emission, and final container assembly. The ordinary no-token encoder path is
unchanged. This is Rust-only evidence because Pillow exposes neither a caller
token nor a work-budget result; the contract proves unlimited RGB and
non-opaque RGBA byte identity, typed bounded `EncodeWorkUnits` rejection, and
an untouched sink. No parity row, fixture, diagnostic origin, or
coverage-only hook was added.

Managed Pillow parity run `b4ca4d5c-41b1-4a86-889f-99b328e1a09c` passed
1,445/1,445 checks with zero failures or skips in 1,239 ms. Coverage MCP run
`4c9db66e-57f4-475c-ab37-66cbb419b971` passed 83/83 tests in 44,944 ms and
ingested snapshot `e8bb4f5b-53bc-4a4e-b007-b8b36e209888`; it reports
48,812/49,184 lines, 6,679/6,740 branches, 2,734/2,801 functions, and
75,982/76,486 regions. The VP8 encoder file is 596/597 lines, 34/34
branches, 34/34 functions, and 1,102/1,108 regions; the WebP dispatcher
remains 544/572 lines, 69/74 branches, 44/54 functions, and 911/966 regions
because its pre-existing structural sink error edges remain uncovered. The
aggregate snapshot carries the LLVM segment-normalization warning. The first
exact-head coverage attempt observed one concurrent AVIF sink-byte assertion;
the focused/full feature tests and this managed retry passed, so this retry is
the accepted coverage record. Feature-matrix run
`a5f91636-2289-4d3a-bad6-eb4022605fcf` passed 947/947 checks in 38,521 ms;
its retained log has no package-cache or build-directory lock-wait matches and
ends with `capability tables OK: every native and wasm32-wasip1 lane agrees`.
These are observed implementation and target-matrix records, separate from
Pillow parity. Finer WebP interior work, remaining predictor/cross-color/
analyze-entropy and histogram/Huffman loops, other codec interior work, deeper
Deflate/structural interruption, allocation accounting, and rollback remain open.

The current lossy WebP/VP8 RGB/RGBA-to-YUV interior checkpoint slice is
implemented at `f6ce32f26516c6403970247f1fbd442ab23b4962`. Token-aware lossy VP8
conversion now charges after each batch of 1,024 Y/UV conversion items before
analysis.
The Rust-only `encode_work_budget_is_a_non_parity_result_contract` contract
proves ample-budget byte identity, typed whole-buffer `EncodeWorkUnits`
rejection at the conversion checkpoint, the same direct-sink rejection, and an
untouched sink. Pillow exposes neither caller token nor work-budget result, so
no parity row, fixture, diagnostic origin, or coverage-only hook was added.

Managed Pillow parity run `842329fc-922a-4fb5-95f8-7e85e96967c7` passed
1,445/1,445 checks with zero failures or skips in 44,892 ms. Feature-matrix run
`4795a291-6d9e-47bf-ae0c-7b1b192d5610` passed 991/991 checks in 99,671 ms; its
retained logs show no package-cache or build-directory lock waits and end with
`capability tables OK: every native and wasm32-wasip1 lane agrees`. Coverage MCP
run `98a6658f-cfd2-4eba-a23b-8823a2172d0d` passed 85/85 tests in 74,829 ms and
ingested snapshot `b007350f-a2b0-4969-b051-8ed3694cb161`, reporting
49,378/49,775 lines, 6,787/6,850 branches, 2,751/2,818 functions, and
76,777/77,488 regions. Compared with
`b6d31c5c-e885-48fb-ad48-09a7e153e254`, this adds 18 covered lines (+18 total),
eight covered branches (+8 total), no functions, and 35 covered regions (+38
total). The WebP VP8 encoder is 614/615 lines, 42/42 branches, 34/34
functions, and 1,137/1,146 regions; its only uncovered line 86 is a
pre-existing defensive bridge, not a reason for a synthetic coverage hook. The
aggregate snapshot retains the LLVM segment-normalization warning. These are
Rust-only implementation and target records separate from Pillow parity.
Remaining finer WebP loops, other codec interior work, Deflate
emission/structural interruption, transient allocation accounting,
short-write/rollback, and non-checkpointed work-budget semantics remain open.

The current lossy WebP/VP8 macroblock-analysis checkpoint and duplicate-analysis
removal slice is implemented at `4779c6aedfe8b9decdb994cf3ddb8751ce68da8e`.
Token-aware VP8 encoding now charges after each batch of 1,024 analyzed
macroblocks, and
`select_frame` reuses the already computed `FrameAnalysis` instead of repeating
the full analysis pass. The Rust-only
`encode_work_budget_is_a_non_parity_result_contract` contract uses a 512x512
RGB probe to prove ample-budget byte identity, typed whole-buffer rejection at
the first analysis checkpoint (`maximum: 326`, `observed: 327`), the same
direct-sink rejection, and an untouched sink. Pillow exposes neither caller
token nor work-budget result, so no parity row, fixture, diagnostic origin, or
coverage-only hook was added.

Managed Pillow parity run `2c7adde1-e6a8-4085-a2aa-dfd02dce7fbf` passed
1,445/1,445 checks with zero failures or skips in 40,722 ms. Feature-matrix run
`89752b03-a6f9-4d58-baa3-227d70a9537d` passed 991/991 checks in 83,091 ms; its
retained logs show no package-cache or build-directory lock waits and end with
`capability tables OK: every native and wasm32-wasip1 lane agrees`. Coverage MCP
run `8505d080-cf41-4917-9e71-a1b3895c2ea5` passed 85/85 tests in 46,299 ms and
ingested snapshot `b6f21c5b-55e3-43d4-98e6-8081c86634af`, reporting
49,374/49,771 lines, 6,789/6,852 branches, 2,750/2,817 functions, and
76,782/77,493 regions. Compared with
`b007350f-a2b0-4969-b051-8ed3694cb161`, covered lines are -4 (-4 total),
covered branches are +2 (+2 total), covered functions are -1 (-1 total), and
covered regions are +5 (+5 total); the line/function decrease reflects the
duplicate-analysis refactor removing source rather than an uncovered path. The
analysis file is 470/470 lines, 34/34 branches, 24/24 functions, and 786/786
regions; the frame file is 319/319 lines, 16/16 branches, 14/14 functions, and
527/527 regions. The VP8 encoder is 615/616 lines, 42/42 branches, 34/34
functions, and 1,139/1,148 regions; its only uncovered line 86 is a
pre-existing defensive bridge, not a reason for a synthetic coverage hook. The
aggregate snapshot retains the LLVM segment-normalization warning. These are
Rust-only implementation and target records separate from Pillow parity.
Remaining finer WebP mode-selection, probability, and bitstream loops, other
codec interior work, Deflate emission/structural interruption, transient
allocation accounting, short-write/rollback, and non-checkpointed work-budget
semantics remain open.

The current lossy WebP/VP8 mode-selection checkpoint slice is implemented at
`7383a00c051badbcff99fdb24365f9360cb73a30`. Token-aware VP8 frame selection now
charges after each batch of 1,024 selected macroblocks in both the ordinary and
trellis branches. The Rust-only
`encode_work_budget_is_a_non_parity_result_contract` contract uses the same
512x512 RGB probe to prove typed whole-buffer rejection at the first selection
checkpoint (`maximum: 329`, `observed: 330`), the same direct-sink rejection,
and an untouched sink; the already-covered ample-budget identity remains
unchanged. Pillow exposes neither caller token nor work-budget result, so no
parity row, fixture, diagnostic origin, or coverage-only hook was added.

Managed Pillow parity run `95546cee-c243-4bbf-8dba-ed6c7859a4a1` passed
1,445/1,445 checks with zero failures or skips in 44,173 ms. Feature-matrix run
`676a68b6-6716-421f-8f76-08f5b3eb3156` passed 991/991 checks in 77,896 ms; its
retained logs show no package-cache or build-directory lock waits and end with
`capability tables OK: every native and wasm32-wasip1 lane agrees`. Coverage MCP
run `419ae162-8617-4b45-bb26-d7eea8238873` passed 85/85 tests in 48,808 ms and
ingested snapshot `884e7ae0-ec2c-43d2-9e54-56c338b31576`, reporting
49,391/49,789 lines, 6,791/6,854 branches, 2,750/2,817 functions, and
76,798/77,510 regions. Compared with
`b6f21c5b-55e3-43d4-98e6-8081c86634af`, this adds 17 covered lines (+18 total),
two covered branches (+2 total), no functions, and 16 covered regions (+17
total). The WebP VP8 frame file is 327/327 lines, 18/18 branches, 14/14
functions, and 542/542 regions. The VP8 encoder is 624/626 lines, 42/42
branches, 34/34 functions, and 1,140/1,150 regions; uncovered lines 86 and 188
are the pre-existing defensive bridge and the unexercised `method >= 6` second-
selection result bridge, respectively, not reasons for a synthetic coverage
hook. The aggregate snapshot retains the LLVM segment-normalization warning.
These are Rust-only implementation and target records separate from Pillow
parity. Remaining finer WebP probability/bitstream loops, other codec interior
work, Deflate emission/structural interruption, transient allocation
accounting, short-write/rollback, and non-checkpointed work-budget semantics
remain open.

The current lossy WebP/VP8 coefficient-probability adaptation checkpoint slice
is implemented at `508867ecb743daf1c793e158807452910adc28d7`. Token-aware
adaptation now charges after the first 1,024 nodes of its fixed 1,056-node
probability table. The Rust-only
`encode_work_budget_is_a_non_parity_result_contract` contract uses the same
512x512 RGB probe to prove typed whole-buffer rejection at the first
probability checkpoint (`maximum: 331`, `observed: 332`), the same direct-sink
rejection, and an untouched sink; the already-covered ample-budget identity
remains unchanged. Pillow exposes neither caller token nor work-budget result,
so no parity row, fixture, diagnostic origin, or coverage-only hook was added.

Managed Pillow parity run `9350397b-ae5d-408a-9e51-3c55b347de2f` passed
1,445/1,445 checks with zero failures or skips in 41,272 ms. The first exact-
head feature-matrix attempt `0326589b-f41d-43ac-ac19-7e1156bd80c7` exited with
status 0 but reported 990/991 counters because the unrelated AVIF sequence
sink byte-equality assertion in `output_sinks_receive_the_exact_encoded_bytes`
flaked; the focused local rerun passed. The accepted exact-head retry
`a4722355-fc5f-43db-abb2-17cdecec14af` passed 991/991 checks in 15,528 ms; its
retained log has no package-cache or build-directory lock-wait matches and ends
with `capability tables OK: every native and wasm32-wasip1 lane agrees`.
Coverage MCP run `14de40f0-158f-481d-99e8-0a7a8e0edee9` passed 85/85 tests in
46,113 ms and ingested snapshot `5e7c51a4-fbfd-4421-ade9-28a241d80f61`,
reporting 49,399/49,797 lines, 6,793/6,856 branches, 2,750/2,817 functions,
and 76,811/77,525 regions. Compared with snapshot
`884e7ae0-ec2c-43d2-9e54-56c338b31576`, this adds eight covered lines (+8
total), two covered branches (+2 total), no functions, and 13 covered regions
(+15 total). The VP8 probability file is 223/223 lines, 30/30 branches,
7/7 functions, and 323/323 regions. The VP8 encoder is 626/628 lines,
42/42 branches, 34/34 functions, and 1,144/1,155 regions; uncovered lines 86
and 189 are the pre-existing defensive bridge and the unexercised `method >= 6`
selection-result bridge, respectively, not reasons for a synthetic coverage
hook. The aggregate snapshot retains the LLVM segment-normalization warning.
These are Rust-only implementation and target records separate from Pillow
parity. Remaining finer WebP bitstream loops, other codec interior work,
Deflate emission/structural interruption, transient allocation accounting,
short-write/rollback, and non-checkpointed work-budget semantics remain open.

The current lossy WebP/VP8 coefficient-bitstream emission checkpoint
slice is implemented at `33a8ffd72f1b3484c14e29e022fa1cc230be1ee3`. Token-aware
residual encoding now charges after each batch of 256 completed macroblocks.
The Rust-only `encode_work_budget_is_a_non_parity_result_contract` contract
uses the same 512x512 RGB probe to prove typed whole-buffer rejection at the
first coefficient-emission checkpoint (`maximum: 334`, `observed: 335`), the
same direct-sink rejection, and an untouched sink; the already-covered
ample-budget identity remains unchanged. Pillow exposes neither caller token
nor work-budget result, so no parity row, fixture, diagnostic origin, or
coverage-only hook was added.

Managed Pillow parity run `641c368d-448f-4ce9-99da-6b4019459b86` passed
1,445/1,445 checks with zero failures or skips in 773 ms. Feature-matrix run
`9dffaf91-6c79-4780-9d55-9bc3cafb5bac` passed 991/991 checks in 47,081 ms; its
retained log has no package-cache or build-directory lock-wait matches and ends
with `capability tables OK: every native and wasm32-wasip1 lane agrees`.
Coverage MCP run `88e43ac0-8ef5-4729-b8e2-cd6c99c2ec8b` passed 85/85 tests in
61,909 ms and ingested snapshot `c8e347ae-a2b9-42e6-b933-930cc5e8151b`,
reporting 49,405/49,803 lines, 6,795/6,858 branches, 2,750/2,817 functions,
and 76,821/77,535 regions. Compared with snapshot
`5e7c51a4-fbfd-4421-ade9-28a241d80f61`, this adds six covered lines (+6 total),
two covered branches (+2 total), no functions, and 10 covered regions (+10
total). The VP8 residual file is 199/199 lines, 26/26 branches, 4/4 functions,
and 299/299 regions. The VP8 encoder is 626/628 lines, 42/42 branches, 34/34
functions, and 1,146/1,157 regions; uncovered lines 86 and 189 are the
pre-existing defensive bridge and the unexercised `method >= 6` selection-result
bridge, respectively, not reasons for a synthetic coverage hook. The aggregate
snapshot retains the LLVM segment-normalization warning. These are Rust-only
implementation and target records separate from Pillow parity. Remaining finer
WebP bitstream loops beyond this macroblock checkpoint, other codec interior
work, Deflate emission/structural interruption, transient allocation
accounting, short-write/rollback, and non-checkpointed work-budget semantics
remain open.

The current lossy WebP/VP8 first-partition emission checkpoint slice is
implemented at `c4305758b9b0a3d24d8160596baec39ea4b73c7b`. Token-aware first
partition writing now charges after the fixed 1,024-node coefficient-probability
signaling table and after each batch of 256 macroblock mode decisions. The
Rust-only `encode_work_budget_is_a_non_parity_result_contract` contract uses
the same 512x512 RGB probe to prove typed whole-buffer rejection at the first
partition probability checkpoint (`maximum: 333`, `observed: 334`), at the
first partition mode checkpoint (`maximum: 334`, `observed: 335`), and at the
following coefficient-emission checkpoint (`maximum: 339`, `observed: 340`),
with the same direct-sink rejection and untouched-prefix assertions. Pillow
exposes neither caller token nor work-budget result, so no parity row, fixture,
diagnostic origin, or coverage-only hook was added.

Managed Pillow parity run `bf3a6c1f-a083-4b36-ba5d-28994a61d7ca` passed
1,445/1,445 checks with zero failures or skips in 1,046 ms. Feature-matrix run
`29e78fe1-fa3a-46c6-8172-35ca20b8b8b1` passed 991/991 checks in 74,705 ms; its
retained log has no package-cache or build-directory lock-wait matches and ends
with `capability tables OK: every native and wasm32-wasip1 lane agrees`.
Coverage MCP run `cc7b49bf-8cbe-468b-b200-64e42efef97d` passed 85/85 tests in
50,577 ms and ingested snapshot `15808c6b-9311-4fcb-885a-28c1174089b4`,
reporting 49,427/49,825 lines, 6,799/6,862 branches, 2,750/2,817 functions,
and 76,845/77,560 regions. Compared with snapshot
`c8e347ae-a2b9-42e6-b933-930cc5e8151b`, this adds 22 covered lines (+22 total),
four covered branches (+4 total), no functions, and 24 covered regions (+25
total). The VP8 partition file is 286/286 lines, 52/52 branches, 15/15
functions, and 487/487 regions. The VP8 encoder is 628/630 lines, 42/42
branches, 34/34 functions, and 1,148/1,159 regions; uncovered lines 86 and
189 remain the pre-existing defensive bridge and unexercised `method >= 6`
selection-result bridge, respectively, not reasons for a synthetic coverage
hook. The aggregate snapshot retains the LLVM segment-normalization warning.
These are Rust-only implementation and target records separate from Pillow
parity. Remaining finer WebP bitstream loops beyond the first-partition and
macroblock coefficient checkpoints, other codec interior work, Deflate
emission/structural interruption, transient allocation accounting,
short-write/rollback, and non-checkpointed work-budget semantics remain open.

The current lossy WebP/VP8 coefficient-block bitstream checkpoint slice is
implemented at `f0d9d683392303602f19bc0b6994f463828265e6`. Token-aware residual
writing now charges after each batch of 64 completed coefficient blocks while
retaining the existing charge after each batch of 256 completed macroblocks.
The Rust-only `encode_work_budget_is_a_non_parity_result_contract` contract
uses the same 512x512 RGB probe to prove typed whole-buffer rejection at the
first finer block checkpoint (`maximum: 339`, `observed: 340`) and at the
retained macroblock checkpoint (`maximum: 439`, `observed: 440`), with the
same direct-sink rejection and untouched-prefix assertions. Pillow exposes
neither caller token nor work-budget result, so no parity row, fixture,
diagnostic origin, or coverage-only hook was added.

Managed Pillow parity run `86b3bd13-1de4-498b-ae53-e5c7c45236a4` passed
1,445/1,445 checks with zero failures or skips in 731 ms. Feature-matrix run
`d42b1d98-cec2-4f05-a55a-5042b3c668ae` passed 991/991 checks in 59,899 ms; its
retained log has no package-cache or build-directory lock-wait matches and ends
with `capability tables OK: every native and wasm32-wasip1 lane agrees`.
Coverage MCP run `d2d6c1a6-c221-4378-a3a0-4864d755cdce` passed 85/85 tests in
44,814 ms and ingested snapshot `023c80f1-290b-4acf-b45a-1112460a919b`,
reporting 49,451/49,851 lines, 6,803/6,866 branches, 2,751/2,818 functions,
and 76,876/77,593 regions. Compared with snapshot
`15808c6b-9311-4fcb-885a-28c1174089b4`, this adds 24 covered lines (+26 total),
four covered branches (+4 total), one covered function (+1 total), and 31
covered regions (+33 total). The VP8 residual file is 223/225 lines, 30/30
branches, 5/5 functions, and 330/332 regions; uncovered lines 221 and 253
are the `?` propagation sites for block-checkpoint errors on the Intra16 and
Intra4 branches, respectively, not reasons for a synthetic coverage hook. The
VP8 encoder remains 628/630 lines, 42/42 branches, 34/34 functions, and
1,148/1,159 regions; uncovered lines 86 and 189 remain its pre-existing
defensive bridge and unexercised `method >= 6` selection-result bridge. The
aggregate snapshot retains the LLVM segment-normalization warning. These are
Rust-only implementation and target records separate from Pillow parity.
Remaining finer WebP bitstream loops beyond the coefficient-block and
macroblock checkpoints, other codec interior work, Deflate
emission/structural interruption, transient allocation accounting,
short-write/rollback, and non-checkpointed work-budget semantics remain open.

The current lossy WebP/VP8 coefficient-token bitstream checkpoint slice is
implemented at `d50b7aed4a450fcd489d0b8fcd4be02b358701ff`. Token-aware residual
writing now charges after each batch of 4,000 coefficient tokens, in addition
to the 64-block and 256-macroblock checkpoints. The Rust-only
`encode_work_budget_is_a_non_parity_result_contract` contract uses the same
512x512 constant RGB probe to prove typed whole-buffer rejection at the first
token checkpoint (`maximum: 400`, `observed: 401`) and at the retained
macroblock checkpoint (`maximum: 440`, `observed: 441`), with the same
direct-sink rejection and untouched-prefix assertions. Pillow exposes neither
caller token nor work-budget result, so no parity row, fixture, diagnostic
origin, or coverage-only hook was added.

Managed Pillow parity run `24346ffa-cc4a-47ab-abf6-895ef527fbe1` passed
1,445/1,445 checks with zero failures or skips in 709 ms. Feature-matrix run
`bb2c02a6-4844-48bc-bff7-d832bebf990c` passed 991/991 checks in 62,483 ms; its
retained log has no package-cache or build-directory lock-wait matches and ends
with `capability tables OK: every native and wasm32-wasip1 lane agrees`.
Coverage MCP run `bb398c45-6f15-4dff-87f0-dda659e1cc9f` passed 85/85 tests in
46,112 ms and ingested snapshot `672f9673-d275-4fff-a1b0-e9ffe7562bbb`,
reporting 49,474/49,875 lines, 6,807/6,870 branches, 2,753/2,820 functions,
and 76,905/77,629 regions. Compared with snapshot
`023c80f1-290b-4acf-b45a-1112460a919b`, this adds 23 covered lines (+24 total),
four covered branches (+4 total), two covered functions (+2 total), and 29
covered regions (+36 total). The VP8 residual file is 246/249 lines, 34/34
branches, 7/7 functions, and 359/368 regions; uncovered lines 214, 252, and
284 are the `?` propagation sites for token/block-checkpoint errors in the
coefficient-block helper and the Intra16/Intra4 branches, not reasons for a
synthetic coverage hook. The VP8 encoder remains 628/630 lines, 42/42
branches, 34/34 functions, and 1,148/1,159 regions; uncovered lines 86 and
189 remain its pre-existing defensive bridge and unexercised `method >= 6`
selection-result bridge. The aggregate snapshot retains the LLVM
segment-normalization warning. These are Rust-only implementation and target
records separate from Pillow parity. Remaining finer WebP bitstream loops
beyond coefficient-token checkpoints, other codec interior work, Deflate
emission/structural interruption, transient allocation accounting,
short-write/rollback, and non-checkpointed work-budget semantics remain open.

The current GIF RGB quantization checkpoint slice is implemented at
`b4dcba7e2840bf65c829872dc45a2938c5089f48`. Token-aware GIF RGB preparation
now charges after each 1,024-pixel interval while collecting palette colors and
emitting palette indices; the high-color nearest-palette path also retains
these intervals while collecting, mapping, and emitting. The token is threaded
through ordinary frame preparation and coalesced full-canvas normalization.
The no-token branch retains the existing tight loops and encoded bytes. The
Rust-only `encode_work_budget_is_a_non_parity_result_contract` contract proves
ample-budget byte identity, a typed whole-buffer rejection at the first RGB
quantization interval (`maximum: 6`, `observed: 7`), the same direct-sink
rejection (`maximum: 5`, `observed: 6`), and untouched sink state. Pillow has
no caller token or work-budget result, so this adds no parity row, fixture,
diagnostic origin, or coverage-only hook.

Managed Pillow parity run `88bbb1f8-13ae-499f-8061-d2be953d60f8` passed
1,445/1,445 checks with zero failures or skips in 41,534 ms. Feature-matrix run
`706a6403-779e-4c50-bd4f-4534eee36f20` passed 991/991 checks in 19,149 ms;
its retained log has no build-directory or package-cache lock-wait matches and
ends with `capability tables OK: every native and wasm32-wasip1 lane agrees`.
Coverage MCP run `6df9e965-f7fe-4553-807d-274ca9a35b49` passed 85/85 tests in
47,130 ms and ingested snapshot `28da3c58-30d1-4038-bf0f-d4b0a3329cb7`,
reporting 49,520/49,950 lines, 6,825/6,904 branches, 2,756/2,823 functions,
and 76,979/77,780 regions. Compared with snapshot
`672f9673-d275-4fff-a1b0-e9ffe7562bbb`, this adds 46 covered lines (+75 total),
18 covered branches (+34 total), three covered functions (+3 total), and 74
covered regions (+151 total). `src/codecs/gif/encode.rs` reports 2,202/2,340
lines, 272/296 branches, 145/170 functions, and 3,489/3,699 regions. The
uncovered new paths are the >256-color RGB nearest-palette fallback and its
token-aware collection/mapping/index intervals (current lines 1859-1860,
1947-1956, 1993-2005, and 2025-2035); no synthetic coverage-only input was
added. The aggregate snapshot retains the LLVM segment-normalization warning.
These are Rust-only implementation and target records separate from Pillow
parity. At that RGB-only revision, remaining GIF RGBA/octree and high-color RGB
quantizer loops, other
codec interior work, finer WebP bitstream work, transient allocation
accounting, short-write/rollback, and remaining non-checkpointed work-budget
semantics remain open.

The current GIF RGBA FASTOCTREE palette-preparation checkpoint slice is
implemented at `54af9374f8e322409ebbd87be46f7c5056c89c50`. Token-aware RGBA
preparation now charges after each 1,024-pixel interval while collecting source
colors, accumulating the fine octree, emitting palette indices, and remapping
indices during palette compaction. Separate no-token branches preserve the
existing tight loops and encoded bytes. The Rust-only
`encode_work_budget_is_a_non_parity_result_contract` contract proves ample-budget
byte identity, typed whole-buffer rejection at the first RGBA quantization
interval (`maximum: 6`, `observed: 7`), the same direct-sink rejection
(`maximum: 5`, `observed: 6`), and untouched sink state. Pillow has no caller
token or work-budget result, so this adds no parity row, fixture, diagnostic
origin, or coverage-only hook.

Managed Pillow parity run `ca42340a-4676-4d2c-9b18-7204658b05a0` passed
1,445/1,445 checks with zero failures or skips in 44,513 ms. Feature-matrix run
`323d2793-2fce-4b45-936e-0ee677a68f0e` passed 991/991 checks in 24,121 ms;
its retained log has no build-directory or package-cache lock-wait matches and
ends with `capability tables OK: every native and wasm32-wasip1 lane agrees`.
Coverage MCP run `4b267364-768c-44f6-be61-9116ed5c6e98` passed 85/85 tests in
74,850 ms and ingested snapshot `a0798493-37c3-4990-9f55-ec2ab1fda92c`,
reporting 49,565/50,003 lines, 6,850/6,936 branches, 2,756/2,823 functions,
and 77,069/77,892 regions. Compared with snapshot
`28da3c58-30d1-4038-bf0f-d4b0a3329cb7`, this adds 45 covered lines (+53 total),
25 covered branches (+32 total), no function changes, and 90 covered regions
(+112 total). `src/codecs/gif/encode.rs` reports 2,247/2,393 lines,
297/328 branches, 145/170 functions, and 3,579/3,811 regions. The new
managed coverage gap is the token-aware transparent-pixel normalization path
(current lines 2315-2324), which the contract's opaque RGBA probe intentionally
does not select. The fixed FASTOCTREE cube-copy, bucket-sort/subtraction, and
lookup loops, plus the high-color RGB median-cut loops, remain non-checkpointed;
no synthetic coverage-only input was added. The aggregate snapshot retains the
LLVM segment-normalization warning. These are Rust-only implementation and
target records separate from Pillow parity. Remaining fixed GIF octree work,
high-color RGB quantizer work, other codec interior work, finer WebP bitstream
work, transient allocation accounting, short-write/rollback, and remaining
non-checkpointed work-budget semantics remain open.

The current GIF RGBA FASTOCTREE fixed-cell checkpoint slice is implemented at
`eb458390406a8904bd3d435c1d72c7973b57da22`. Token-aware RGBA preparation now
charges after each 1,024-cell, bucket, or lookup-entry interval while copying
fine/coarse octree cubes, subtracting bucket ranges, and building coarse/fine
lookup cubes. Separate no-token branches preserve the previous tight loops and
encoded bytes. The Rust-only `encode_work_budget_is_a_non_parity_result_contract`
contract proves ample-budget byte identity, typed whole-buffer rejection at the
first RGBA octree cell interval (`maximum: 6`, `observed: 7`), the same direct-
sink rejection (`maximum: 5`, `observed: 6`), and untouched sink state. Pillow
has no caller token or work-budget result, so this adds no parity row, fixture,
diagnostic origin, or coverage-only hook.

Managed Pillow parity run `0b64a36b-fec3-4d83-949a-432bc903c937` passed
1,445/1,445 checks with zero failures or skips in 42,406 ms. Feature-matrix run
`0a721689-c4b3-41fa-917e-642053d25cdb` passed 991/991 checks in 23,962 ms;
its retained log has no build-directory or package-cache lock-wait matches and
ends with `capability tables OK: every native and wasm32-wasip1 lane agrees`.
Coverage MCP run `6e473598-4246-40f6-a3ae-78d26883939b` passed 85/85 tests in
72,760 ms and ingested snapshot `7398b4ba-7c01-4fed-8538-d6747853ffa7`,
reporting 49,622/50,063 lines, 6,868/6,956 branches, 2,758/2,825 functions,
and 77,172/78,005 regions. Compared with snapshot
`a0798493-37c3-4990-9f55-ec2ab1fda92c`, this adds 57 covered lines (+60 total),
18 covered branches (+20 total), two covered functions (+2 total), and 103
covered regions (+113 total). `src/codecs/gif/encode.rs` reports 2,304/2,453
lines, 315/348 branches, 147/172 functions, and 3,682/3,924 regions. The new
managed gaps are the 1,024-entry cancellation edges in token-aware bucket
subtraction and lookup (current lines 2735-2736 and 2769-2770), plus the
second coarse-reduction call at line 2819; transparent-pixel normalization
remains uncovered at lines 2315-2324. The FASTOCTREE bucket-sort and high-color
RGB median-cut loops remain non-checkpointed; no synthetic coverage-only input
was added. The aggregate snapshot retains the LLVM segment-normalization
warning. These are Rust-only implementation and target records separate from
Pillow parity. Remaining GIF bucket-sort work, transparent-normalization
coverage, high-color RGB quantizer work, other codec interior work, finer WebP
bitstream work, transient allocation accounting, short-write/rollback, and
remaining non-checkpointed work-budget semantics remain open.

The current GIF high-color RGB median-cut checkpoint slice is implemented at
`d238b427d979102f8dd4e09aa4c079f8861eb13c`. Token-aware RGB median-cut
preparation now charges checkpoints through hash/order setup, each axis
ordering, median-cut split stages, and 1,024-item split/partition scans.
Separate no-token branches preserve the previous tight loops and encoded bytes.
The Rust-only `encode_work_budget_is_a_non_parity_result_contract` contract
uses 2,048 unique RGB pixels to prove ample-budget byte identity, a typed
whole-buffer rejection at the first high-color median-cut checkpoint
(`maximum: 6`, `observed: 7`), the same direct-sink rejection (`maximum: 5`,
`observed: 6`), and untouched sink state. Pillow has no caller token or
work-budget result, so this adds no parity row, fixture, diagnostic origin, or
coverage-only hook.

Managed Pillow parity run `ea0acbac-c1c2-4c9e-84c6-676fcb671ecd` passed
1,445/1,445 checks with zero failures or skips in 711 ms. Feature-matrix run
`7fac4338-7f6c-4a46-905f-dda1c4693049` passed 991/991 checks in 81,214 ms;
its retained log has no build-directory or package-cache lock-wait matches and
ends with `capability tables OK: every native and wasm32-wasip1 lane agrees`.
Coverage MCP run `1c83d680-4db7-46fd-8a02-52446b32efe4` passed 85/85 tests in
47,897 ms and ingested snapshot `ef77b476-b7f7-4dc8-ae81-c064886facf9`,
reporting 49,794/50,206 lines, 6,930/7,004 branches, 2,763/2,830 functions,
and 77,489/78,263 regions. Compared with snapshot
`7398b4ba-7c01-4fed-8538-d6747853ffa7`, this adds 172 covered lines (+143
total), 62 covered branches (+48 total), five covered functions (+5 total),
and 317 covered regions (+258 total). `src/codecs/gif/encode.rs` reports
2,476/2,596 lines, 377/396 branches, 152/177 functions, and 3,999/4,182
regions. The new median-cut paths are covered by the Rust-only high-color
contract; the remaining managed GIF gaps are the transparent-pixel
normalization path (current lines 2480-2489), the 1,024-entry octree
subtraction and lookup cancellation edges (current lines 2899-2900 and
2933-2934), and the second coarse-reduction call (line 2983). The FASTOCTREE
bucket-sort loops remain non-checkpointed; no synthetic coverage-only input
was added. The aggregate snapshot retains the LLVM segment-normalization
warning. These are Rust-only implementation and target records separate from
Pillow parity. Remaining GIF bucket-sort work, transparent-normalization
coverage, other codec interior work, finer WebP bitstream work, transient
allocation accounting, short-write/rollback, and remaining non-checkpointed
work-budget semantics remain open.

The current GIF RGBA FASTOCTREE bucket-sort checkpoint slice is implemented at
`c430f7be25c17b103a4aed7f7e8462a3ecf8c230`. Token-aware Apple-compatible
bucket sorting now charges after each 1,024 sorting operations across partition
scans, equal-range swaps, and recursive sorting; separate no-token branches
preserve the previous tight loops and encoded bytes. The Rust-only
`encode_work_budget_is_a_non_parity_result_contract` contract reaches the long
sort with an opaque 1x1 RGBA probe and proves whole-buffer and direct-sink
rejection at the sorter checkpoint (`maximum: 8`, `observed: 9`). A diverse
2,048-pixel RGBA probe exercises nontrivial partitions and recursive ranges
while preserving ample-budget byte identity. Pillow has no caller token or
work-budget result, so this adds no parity row, fixture, diagnostic origin, or
coverage-only hook.

Managed Pillow parity run `dbf95b14-36e5-4fbf-8f30-2d570931fdb2` passed
1,445/1,445 checks with zero failures or skips in 768 ms. Feature-matrix run
`c0ca11f1-b4fd-41bf-891a-a1562984597f` passed 991/991 checks in 37,379 ms;
its retained log has no build-directory or package-cache lock-wait matches and
ends with `capability tables OK: every native and wasm32-wasip1 lane agrees`.
Coverage MCP run `09507307-d934-4a54-9334-c831af69bffe` passed 85/85 tests in
46,164 ms and ingested snapshot `cbc19eaa-3399-457f-acc5-81bc29bc279f`,
reporting 49,964/50,398 lines, 6,967/7,048 branches, 2,767/2,835 functions,
and 77,758/78,576 regions. Compared with snapshot
`7f53ad81-b96e-4be4-a816-b14aa7bdcb93`, this adds 50 covered lines, 15 covered
branches, one covered function, and 79 covered regions with no total-count
change. `src/codecs/gif/encode.rs` reports 2,646/2,788 lines, 414/440
branches, 156/182 functions, and 4,268/4,495 regions. The remaining managed
GIF gaps are the transparent-pixel normalization path (current lines
2480-2489), token-aware insertion-sort swap/limit edges (2781-2789), and
short-range/range-swap/fallback/recursive sorter edges (2950, 3031,
3042-3052, 3062, and 3075), plus the 1,024-entry octree subtraction and lookup
cancellation edges (3106-3107 and 3140-3141) and the second coarse-reduction
call (line 3190). The new bucket-sort checkpoint paths are covered by the
Rust-only contract; no synthetic coverage-only input was added. The aggregate
snapshot retains the LLVM segment-normalization warning. These are Rust-only
implementation and target records separate from Pillow parity. Remaining
transparent-normalization coverage, other codec interior work, finer WebP
bitstream work, TIFF Deflate matcher/emission, transient allocation accounting,
short-write/rollback, and remaining non-checkpointed work-budget semantics
remain open.

The current GIF transparent-pixel normalization checkpoint contract is
implemented and tested at `99abec03c2a478bc167caea881980fbf596887c9`.
Token-aware RGBA preparation now proves the 1,024-pixel normalization interval
with 2,048 fully transparent pixels whose RGB channels vary before Pillow's
normalization. The Rust-only `encode_work_budget_is_a_non_parity_result_contract`
contract proves ordinary and ample-budget byte identity, whole-buffer rejection
at the normalization checkpoint (`maximum: 2`, `observed: 3`), the same direct-
sink rejection (`maximum: 1`, `observed: 2`), and untouched sink state. Pillow
has no caller token or work-budget result, so this adds no parity row, fixture,
diagnostic origin, or coverage-only hook.

Managed Pillow parity run `49bf1363-d6ce-49ef-890b-7f3194d810b8` passed
1,445/1,445 checks with zero failures or skips in 41,509 ms. Feature-matrix run
`116aef69-7d74-40e2-b942-0d8d96db3529` passed 991/991 checks in 74,584 ms;
its retained log has no build-directory or package-cache lock-wait matches and
ends with `capability tables OK: every native and wasm32-wasip1 lane agrees`.
Coverage MCP run `b96340a3-712e-4f32-b57a-9a412c64d4ea` passed 85/85 tests in
47,058 ms and ingested snapshot `3e3f5663-e2ae-4199-9fc1-8a91b4778532`,
reporting 49,972/50,398 lines, 6,973/7,048 branches, 2,767/2,835 functions,
and 77,773/78,576 regions. Compared with snapshot
`cbc19eaa-3399-457f-acc5-81bc29bc279f`, this adds eight covered lines, six
covered branches, no covered functions, and 15 covered regions with no
total-count change. `src/codecs/gif/encode.rs` reports 2,654/2,788 lines,
420/440 branches, 156/182 functions, and 4,283/4,495 regions. The
transparent-normalization checkpoint is covered; its remaining managed edge is
the non-transparent skip branch (current lines 2487-2489). The remaining GIF
gaps are the token-aware insertion-sort swap/limit edges (2781-2789),
short-range/range-swap/fallback/recursive sorter edges (2950, 3031,
3042-3052, 3062, and 3075), the 1,024-entry octree subtraction and lookup
cancellation edges (3106-3107 and 3140-3141), and the second coarse-reduction
call (line 3190). No synthetic coverage-only input was added. The aggregate
snapshot retains the LLVM segment-normalization warning. These are Rust-only
implementation and target records separate from Pillow parity. Remaining other
codec interior work, finer WebP bitstream work, TIFF Deflate matcher/emission,
transient allocation accounting, short-write/rollback, and remaining
non-checkpointed work-budget semantics remain open.

The current lossless WebP/VP8L work-budget slice is implemented and tested at
`78439ccc44480df892dfdf81c62dfb337ddb0570`: token-aware lossless encoding now
charges checkpoints around pixel conversion, entropy analysis, transform
selection/application, and bitstream assembly, while the ordinary no-token
path preserves its existing bytes. The Rust-only contract proves unlimited
lossless RGB WebP byte identity, typed bounded `EncodeWorkUnits` rejection,
and an untouched sink. Pillow exposes neither a caller token nor a work-budget
result, so this slice adds no parity row, fixture, diagnostic origin, or
coverage-only hook.

Managed Pillow parity run `0e98c9ee-5624-43a8-9cf7-34f74b8beaf6` passed
1,445/1,445 checks with zero failures or skips in 2,158 ms. Coverage MCP run
`7558225d-959d-40f7-9f52-08fd2d4294e6` passed 83/83 tests in 55,630 ms and
ingested snapshot `7f037e01-f037-47e5-9bb7-2f03a1132625`; it reports
48,838/49,213 lines, 6,679/6,740 branches, 2,735/2,802 functions, and
76,030/76,548 regions. The native VP8L encoder file is 1,244/1,246 lines,
202/202 branches, 69/69 functions, and 1,869/1,882 regions; the WebP
dispatcher is 551/580 lines, 69/74 branches, 44/54 functions, and 916/972
regions. The aggregate snapshot carries the LLVM segment-normalization
warning, and the two uncovered native encoder lines are defensive token-bridge
edges rather than a reason to add a synthetic coverage hook. Feature-matrix
run `e6b0fe5b-ac02-4aeb-a155-a222de76a679` passed 947/947 checks in 84,794 ms;
its retained log has no package-cache or build-directory lock-wait matches and
ends with `capability tables OK: every native and wasm32-wasip1 lane agrees`.
These are observed implementation and target-matrix records, separate from
Pillow parity. Finer WebP interior work, remaining predictor/cross-color/
analyze-entropy and histogram/Huffman loops, other codec interior work, deeper
Deflate/structural interruption, allocation accounting, and rollback remain open.

The current lossless WebP/VP8L interior checkpoint slice is implemented at
`447cc034eaccf85843b59e18310778310b22c5f8`, following the backward-reference
and token-stream work at `9838ae5ea80c28bf8ed87aff08572e2f4c789144`:
predictor tile scans/mode application, cross-color multiplier search and
transform tiles, entropy analysis, histogram clustering, and Huffman-tree/group
emission now poll the same caller token and charge the same work budget. The
no-token path remains byte-preserving. The Rust-only contract adds a materially
larger bounded lossless probe that reaches the long VP8L path before the typed
rejection; Pillow has no caller token, work-budget result, or diagnostic field,
so no parity row, fixture, diagnostic origin, or coverage-only hook is added.

Managed Pillow parity run `2484431d-f90d-4616-a837-0268e268b58c` passed
1,445/1,445 checks with zero failures or skips in 1,113 ms. Coverage MCP run
`feb96568-c64f-4b4d-96a0-1e9a348ad602` passed 83/83 tests in 50,814 ms and
ingested snapshot `33a651a8-3d60-4793-84e6-b08edaa5ecca`; it reports
49,167/49,562 lines, 6,769/6,832 branches, 2,740/2,807 functions, and
76,480/77,126 regions. Predictor is 286/287 lines, 48/48 branches, 23/23
functions, and 522/531 regions; cross-color is 466/475 lines, 73/74
branches, 25/25 functions, and 589/610 regions; histogram is 611/611 lines,
130/130 branches, 32/32 functions, and 945/963 regions; the native VP8L
encoder is 1,350/1,360 lines, 222/222 branches, 69/69 functions, and
1,992/2,045 regions; and the WebP dispatcher is 552/581 lines, 69/74
branches, 44/54 functions, and 918/975 regions. Compared with snapshot
`1e48e6a6-b11e-49bc-abd0-c117a3349b58`, this adds 162 covered lines and 39
covered branches (+3 covered functions and +233 covered regions); the small
rate decreases are attributable to the added code. The aggregate snapshot
retains the LLVM segment-normalization warning; uncovered lines are defensive
unreachable/error-propagation edges, not a reason to add a synthetic hook.
Feature-matrix run `e430da34-5662-456d-b745-9e60b884c658` passed 947/947 checks
in 60,673 ms; its retained log has no package-cache or build-directory
lock-wait matches and ends with `capability tables OK: every native and
wasm32-wasip1 lane agrees`. These are observed implementation and target
evidence, separate from Pillow parity. Remaining finer WebP work, other codec
interior work, deeper Deflate/structural interruption, allocation accounting,
and rollback remain open.

The current TIFF Deflate interior checkpoint slice is implemented at
`e2b060dff1758749a498bc98919143f6d4c2ca6c`: the token-aware level-six matcher
now charges checkpoints inside candidate-chain search, match insertion,
fizzle adjustment, window maintenance, and per-position processing. The
ordinary PNG/general level-six helper remains on its no-token path, so the
existing byte model does not acquire caller-token polling overhead. The
Rust-only contract extends the TIFF page probe with a single wide row whose
bounded budget rejects inside the matcher; Pillow exposes neither a caller
token nor a work-budget result, so this adds no parity row, diagnostic origin,
or coverage-only hook.

Managed Pillow parity run `d1181fff-199c-4bfd-a2ed-aec4f643a7b7` passed
1,445/1,445 checks with zero failures or skips in 814 ms. Coverage MCP run
`46703f57-b9d8-4c27-857e-deda300b162f` passed 83/83 tests in 46,061 ms and
ingested snapshot `abe2f77d-d2e5-4137-91d7-b71f7160ad4e`; it reports
49,232/49,627 lines, 6,769/6,832 branches, 2,746/2,813 functions, and
76,571/77,234 regions. Compared with snapshot
`33a651a8-3d60-4793-84e6-b08edaa5ecca`, this adds 65 covered lines (+65 total),
six covered functions (+6 total), and 91 covered regions (+108 total), with
branch counts unchanged; the small region-rate decrease is attributable to
the new checkpoint error branches. `src/codecs/compression/zlib_ng.rs` is
1,812/1,812 lines, 390/390 branches, 89/89 functions, and 2,818/2,835
regions. The aggregate snapshot retains the LLVM segment-normalization
warning; no coverage-only hook was added. Isolated warm feature-matrix run
`a3967a0c-3758-43b5-a744-620703c367a4` passed 947/947 checks in 15,462 ms,
with no package-cache or build-directory lock-wait matches and the terminal
`capability tables OK: every native and wasm32-wasip1 lane agrees` marker.
These are observed runtime and target-evidence records, not universal
benchmarks and not Pillow-parity coverage. Remaining other codec interior
work, finer WebP work, Deflate emission/structural interruption, transient
allocation accounting, and rollback remain open.

The feature-matrix runtime follow-up is implemented at
`3c10f9ccaf494c96d42982006be1434050bd9c5c`. Native lanes still compile and
execute all 43 feature-gate tests for each of the 11 feature configurations;
they no longer repeat native Clippy and rustdoc, because the repository
quality job already runs those all-feature checks and the lane's test build
already compiles the selected feature set. The matching
`wasm32-unknown-unknown` Clippy/rustdoc lanes, two WASM test-compilation
checks, all 11 `wasm32-wasip1` runtime lanes, the determinism probe, and the
capability-table no-drift check remain unchanged. Managed run
`070f12d9-38e7-4626-94f6-40f19321fc67` passed 947/947 checks in 11,854 ms with
no retained build-directory or package-cache lock-wait matches and the
terminal `capability tables OK: every native and wasm32-wasip1 lane agrees`
marker. A local warm repeat took 11.937 seconds versus 16.788 seconds before
the change; both are observed executions rather than universal benchmarks.
The exact-head Pillow parity run `d70e6d45-b6b1-4b6f-a6e1-ae400adf7e92`
passed 1,445/1,445 checks in 683 ms. Coverage MCP run
`5f409090-d912-4748-98ee-81ab56c91099` passed 83/83 tests in 44,299 ms and
ingested snapshot `30bfa31d-99e7-45b7-bd62-322c2139210f`, retaining
49,232/49,627 lines, 6,769/6,832 branches, 2,746/2,813 functions, and
76,571/77,234 regions. The snapshot is unchanged from the prior Rust source
revision because this slice changes only the test harness; it adds no parity
row, fixture, diagnostic origin, or coverage-only hook.

The current TIFF Deflate emission/structural checkpoint slice is implemented at
`18a8f42297c2ba247b29e8c3c8d4fec2fff51abd`. Token-aware TIFF output now charges
cooperative checkpoints while expanding tokens, analyzing Huffman frequencies
and trees, emitting stored/fixed/dynamic bitstreams, copying stored-block
bytes, and computing the Adler-32 trailer. The ordinary PNG/general no-token
path remains byte-preserving. The Rust-only contract uses a materially larger
budget on the same wide TIFF row to reach this emission path, rejects with the
typed `EncodeWorkUnits` result, and leaves the sink untouched; Pillow exposes
neither a caller token nor a work-budget result, so no parity row, fixture,
diagnostic origin, or coverage-only hook was added.

Managed Pillow parity run `3259b555-199c-4c7f-85cd-a83f3ef6a2df` passed
1,445/1,445 checks with zero failures or skips in 46,451 ms. Coverage MCP run
`dd13608e-9ecc-4c44-b66e-0681cb1a96c4` passed 83/83 tests in 46,770 ms and
ingested snapshot `8316ea85-bbc0-4d25-ba9f-fb49bd82b9fe`, reporting
49,345/49,742 lines, 6,773/6,836 branches, 2,750/2,817 functions, and
76,720/77,428 regions. Compared with the preceding TIFF matcher snapshot
`abe2f77d-d2e5-4137-91d7-b71f7160ad4e`, this adds 113 covered lines (+115
total), four covered branches (+4 total), four covered functions (+4 total),
and 149 covered regions (+194 total); the small rate changes reflect the new
checkpoint branches. The aggregate snapshot retains the LLVM segment
normalization warning. Feature-matrix run
`87eb1796-8fae-4ae9-8dc7-dbcaaf36989d` passed 947/947 checks in 68,760 ms;
its retained log has no package-cache or build-directory lock-wait matches and
ends with `capability tables OK: every native and wasm32-wasip1 lane agrees`.
These are observed implementation and target-evidence records, separate from
Pillow parity. Remaining other codec interior work, finer WebP work, transient
allocation accounting, short/interrupted output, rollback, and any remaining
non-checkpointed work-budget semantics remain open.

The current partial structural sink-write slice is implemented at
`1726f44e381ebc6132a027696a068415ad82806a`, building on the still-codec
coverage in `ac22bf1cbdb43922969bb35172a9515430e753b8` and the sequence
coverage in `c2919b0bf383a308e3ce111c2cfafcb4d8ab22f5`. The Rust-only
`partial_structural_sink_write_preserves_prefix_across_available_encoders`
contract iterates every still encoder and every supported multi-frame
GIF/TIFF/WebP/native-AVIF sequence writer available in each feature/target
lane. Each writer accepts a genuine prefix of its second structural segment,
then rejects; the encoder reports `ImageError::OutputWrite` with the selected
format and `StillEncode` or `SequenceEncode` stage, preserves the exact
delivered prefix, and does not call `flush`. Native AVIF comparisons use one
worker so this contract remains byte-deterministic beside concurrent AVIF
tests. Managed Pillow parity run
`7e5fc725-f121-4639-88cc-84a63b366420` passed 1,445/1,445 checks with zero
skips in 891 ms. Feature-matrix run
`93004110-a3cb-4d1b-9b81-77b48548338d` passed 991/991 checks in 36,830 ms;
its retained log has no build-directory or package-cache lock-wait matches and
ends with `capability tables OK: every native and wasm32-wasip1 lane agrees`.
Coverage MCP run `a15e2a12-4ef8-436b-a3f2-2c6ffc43bb81` passed 85/85 tests in
50,759 ms and ingested snapshot `61ba8d2a-75b9-4679-9450-2881405d5496`,
reporting 49,345/49,742 lines, 6,773/6,836 branches, 2,750/2,817 functions,
and 76,720/77,428 regions, unchanged in aggregate from snapshot
`f97ce72e-2499-4e64-aa24-457fe5e06eb6`. That unchanged coverage is expected:
the slice changes only an integration-test contract, not a measured library
execution path. Pillow has no caller-owned `OutputSink`, so this evidence adds
no parity row, fixture, diagnostic origin, or coverage-only hook. Other
structural paths, interrupted writes, rollback, and partial-container cleanup
remain open.

The current GIF LZW interior checkpoint slice is implemented at
`398e26f5fefb4bb8020427cd9e3f0be6780cab3b`. Token-aware still and sequence GIF
encoding now polls once for each input symbol considered by the dictionary
pass, so a bounded operation can stop inside LZW before compressed bytes are
assembled or delivered. The Rust-only
`encode_work_budget_is_a_non_parity_result_contract` contract proves ample-
budget byte identity, a typed `EncodeWorkUnits` rejection at that interior
interval, the same direct-sink rejection, and an untouched sink. Ordinary
GIF output remains byte-identical. Pillow exposes neither a caller token nor
a work-budget result, so this adds no parity row, fixture, diagnostic origin,
or coverage-only hook.

Managed Pillow parity run `244c84d1-870a-4121-93ec-1273aaa56c5f` passed
1,445/1,445 checks with zero failures or skips in 94,396 ms. Feature-matrix
run `226e28ce-931f-4c8b-91e7-38a881f9da35` passed 991/991 checks in 141,714
ms; its retained log has no build-directory or package-cache lock-wait
matches and ends with `capability tables OK: every native and wasm32-wasip1
lane agrees`. Coverage MCP run `48151e15-8583-43a1-b4c3-dbfcd187fbd3` passed
85/85 tests in 145,381 ms and ingested snapshot
`94811710-aa78-4aad-b64f-7145f8fab17e`, reporting 49,350/49,747 lines,
6,773/6,836 branches, 2,750/2,817 functions, and 76,724/77,432 regions.
Compared with snapshot `61ba8d2a-75b9-4679-9450-2881405d5496`, this adds
five covered lines (+5 total) and four covered regions (+4 total), with
branches and functions unchanged; every new GIF LZW line and branch is
covered. The aggregate snapshot retains the LLVM segment-normalization
warning. These are implementation and target-lane records separate from
Pillow parity. Other codec interior work, finer WebP work, transient
allocation accounting, short/interrupted output, rollback, and remaining
non-checkpointed work-budget semantics remain open.

The current BMP row-conversion interior checkpoint slice is implemented at
`748358a1810cfc00f686f6cc0a056fd9c1e669da`. Token-aware BMP still encoding now
polls after each 1,024 pixels while converting a wide indexed or true-color
row. The Rust-only
`encode_work_budget_is_a_non_parity_result_contract` contract proves ample-
budget byte identity, a typed whole-buffer `EncodeWorkUnits` rejection at the
interior interval, and the same direct-sink rejection while preserving the
already-delivered validated BMP header prefix. Pillow exposes neither a caller
token nor a work-budget result, so this adds no parity row, fixture, diagnostic
origin, or coverage-only hook.

Managed Pillow parity run `60608903-8d58-42a7-a52a-be78651582c1` passed
1,445/1,445 checks with zero failures or skips in 77,292 ms. Feature-matrix
run `3cbbefd8-7244-4596-ae27-cc5dcc8a8f6d` passed 991/991 checks in 94,445
ms; its retained log has no build-directory or package-cache lock-wait
matches and ends with `capability tables OK: every native and wasm32-wasip1
lane agrees`. Coverage MCP run `9f6273f0-80a9-4fef-933f-ea7a4d13fcf8` passed
85/85 tests in 82,885 ms and ingested snapshot
`b1cb1124-85b3-4b40-994f-7b9f8a4f831e`, reporting 49,352/49,749 lines,
6,777/6,840 branches, 2,750/2,817 functions, and 76,733/77,441 regions.
Compared with snapshot `94811710-aa78-4aad-b64f-7145f8fab17e`, this adds two
covered lines (+2 total), four covered branches (+4 total), and nine covered
regions (+9 total), with functions unchanged; every new BMP row-conversion
line and branch is covered. The aggregate snapshot retains the LLVM
segment-normalization warning. These are implementation and target-lane
records separate from Pillow parity. Remaining other codec interior work,
finer WebP work, transient allocation accounting, short/interrupted output,
rollback, and remaining non-checkpointed work-budget semantics remain open.

The GIF LZW no-token runtime follow-up is implemented at
`430e33d3f5dc12319c39b66c7f43f3c39e7306e1`. The ordinary encoder now takes a
no-poll branch, avoiding an optional cancellation-token check for every input
symbol, while token-aware encoding retains the same per-symbol cancellation
and work-budget checkpoints. The emitted bytes and the Rust-only work-control
contract are unchanged. This implementation-only optimization adds no parity
row, fixture, diagnostic origin, or coverage-only hook.

Managed Pillow parity run `ecae85e1-0584-44aa-8028-fcc1e865e386` passed
1,445/1,445 checks with zero failures or skips in 791 ms. Feature-matrix run
`3fe7a570-cae3-4ef0-9e8d-65a4d645fd59` passed 991/991 checks in 36,648 ms;
its retained log has no build-directory or package-cache lock-wait matches and
ends with `capability tables OK: every native and wasm32-wasip1 lane agrees`.
Coverage MCP run `9949a87c-d645-47d3-b95e-0d5578bb7663` passed 85/85 tests in
53,685 ms and ingested snapshot
`b6d31c5c-e885-48fb-ad48-09a7e153e254`, reporting 49,360/49,757 lines,
6,779/6,842 branches, 2,751/2,818 functions, and 76,742/77,450 regions.
Compared with snapshot `b1cb1124-85b3-4b40-994f-7b9f8a4f831e`, this adds
eight covered lines (+8 total), two covered branches (+2 total), one covered
function (+1 total), and nine covered regions (+9 total); every new fast-path
line and branch is covered. The aggregate snapshot retains the LLVM
segment-normalization warning. These are execution and implementation records
separate from Pillow parity. Remaining other codec interior work, finer WebP
work, transient allocation accounting, short/interrupted output, rollback, and
remaining non-checkpointed work-budget semantics remain open.

The current lossy WebP/VP8 coefficient-bit interval slice is implemented at
`cb262736c050d7ea1736c45541b77bf019ef1547`. Token-aware coefficient emission
now charges cancellation and work-budget checkpoints after each 16,384
boolean-coded coefficient bits. The ordinary no-token path uses a
monomorphized no-op checkpoint controller, preserving the existing bytes
without per-bit optional-token polling. The Rust-only
`encode_work_budget_is_a_non_parity_result_contract` contract proves the new
typed rejection at `maximum: 401`, `observed: 402`, in both whole-buffer and
direct-sink paths while leaving the sink unchanged. Pillow has no caller token,
work-budget result, or caller-owned sink, so this adds no parity row, fixture,
diagnostic origin, or coverage-only hook.

Managed Pillow parity run `76e7a249-15c4-4232-9c30-42c5ca9ad545` passed
1,445/1,445 checks with zero failures or skips in 1,288 ms. Feature-matrix run
`c86d57e1-ccbb-4b42-8669-e4865f2ae243` passed 991/991 checks in 68,765 ms;
its retained log has no build-directory or package-cache lock-wait matches and
ends with `capability tables OK: every native and wasm32-wasip1 lane agrees`.
Coverage MCP run `158ab5f4-0c45-4855-844d-244c698741d8` passed 85/85 tests in
54,337 ms and ingested snapshot `d5d5e314-b269-4667-8824-48e917c026de`,
reporting 50,022/50,455 lines, 6,973/7,048 branches, 2,775/2,843 functions,
and 77,860/78,693 regions. Compared with snapshot
`3e3f5663-e2ae-4199-9fc1-8a91b4778532`, this adds 50 covered lines (+57 total),
no covered or total branch changes, eight covered functions (+8 total), and 87
covered regions (+117 total). The aggregate snapshot retains the LLVM
segment-normalization warning. `src/codecs/webp/encode/vp8/residual.rs` is
296/306 lines, 34/34 branches, 15/15 functions, and 446/485 regions; its ten
uncovered lines are defensive checkpoint-error propagation sites, not a reason
for a synthetic coverage hook. These are implementation and target records
separate from Pillow parity. Remaining finer WebP bitstream work beyond this
coefficient-bit interval, other codec interior work, transient allocation
accounting, short/interrupted output, rollback, and remaining non-checkpointed
work-budget semantics remain open.

The current lossy WebP/VP8 first-partition boolean-bit interval slice is
implemented at `10609f5020b1e35afabd3a9afad205a48957b5d6`. Token-aware first
partition coding now charges cancellation and work-budget checkpoints after
each 16,384 boolean-coded bits, while the ordinary no-token path uses a
monomorphized no-op controller and preserves existing bytes. The Rust-only
`encode_work_budget_is_a_non_parity_result_contract` contract uses a patterned
896x512 RGB probe to prove typed whole-buffer and direct-sink rejection at
`maximum: 580`, `observed: 581`, with the sink untouched. The probe is Rust-only:
Pillow has no caller token, work-budget result, or caller-owned sink, so this
adds no parity row, fixture, diagnostic origin, or coverage-only hook.

Managed Pillow parity run `95c1e2f1-2e6f-40df-a07a-d31558580e3e` passed
1,445/1,445 checks with zero failures or skips in 41,163 ms. Feature-matrix run
`967e4e71-4a2c-4113-a4e8-0de8a09a5a4a` passed 991/991 checks in 105,311 ms;
its retained log has no build-directory or package-cache lock-wait matches and
ends with `capability tables OK: every native and wasm32-wasip1 lane agrees`.
Coverage MCP run `21545cac-2700-4ba2-863f-bb7770c39df0` passed 85/85 tests in
71,686 ms and ingested snapshot `d48d6537-f7c4-45ec-9d71-d072315d8eb6`,
reporting 50,153/50,593 lines, 6,977/7,052 branches, 2,784/2,852 functions,
and 78,007/78,888 regions. Compared with snapshot
`d5d5e314-b269-4667-8824-48e917c026de`, this adds 131 covered lines (+138
total), four covered branches (+4 total), nine covered functions (+9 total),
and 147 covered regions (+195 total). The aggregate snapshot retains the LLVM
segment-normalization warning. `src/codecs/webp/encode/vp8/partition.rs` is
417/424 lines, 56/56 branches, 24/24 functions, and 634/682 regions; its seven
uncovered lines are defensive checkpoint-error propagation sites, not a reason
for a synthetic coverage hook. These are implementation and target records
separate from Pillow parity. Remaining finer WebP bitstream work beyond the
implemented first-partition and coefficient-bit intervals, other codec interior
work, transient allocation accounting, short/interrupted output, rollback, and
remaining non-checkpointed work-budget semantics remain open.

The current lossy WebP/VP8 boolean-bitstream output-byte checkpoint slice is
implemented at `d6b4dac5a5775af713935186b07b221751c72f06`. Token-aware
first-partition and coefficient-partition boolean coding now charges
cancellation and work-budget checkpoints after each 1,024 newly emitted
boolean-coder bytes. The ordinary no-token path uses monomorphized no-op
controllers and preserves the existing bytes. The Rust-only
`encode_work_budget_is_a_non_parity_result_contract` contract uses the patterned
896x512 RGB probe to prove typed whole-buffer rejection at `maximum: 589`,
`observed: 590`, and direct-sink rejection at `maximum: 588`, `observed: 589`,
with the sink untouched. Pillow has no caller token, work-budget result, or
caller-owned sink, so this adds no parity row, fixture, diagnostic origin, or
coverage-only hook.

Managed Pillow parity run `cb56a877-b7ff-42a9-b25b-d400b233aabc` passed
1,445/1,445 checks with zero failures or skips in 41,842 ms. Feature-matrix run
`860dd502-4c2a-40fc-8b0e-a1a23ba39906` passed 991/991 checks in 121,216 ms;
its retained log has no build-directory or package-cache lock-wait matches and
ends with `capability tables OK: every native and wasm32-wasip1 lane agrees`.
Coverage MCP run `a210578b-12ca-4ca9-ab6b-f2bedf0c1b52` passed 85/85 tests in
54,289 ms and ingested snapshot `42300a0d-e7c9-4025-aafd-6f4b93757706`,
reporting 50,260/50,702 lines, 6,984/7,060 branches, 2,799/2,867 functions,
and 78,142/79,032 regions. Compared with snapshot
`d48d6537-f7c4-45ec-9d71-d072315d8eb6`, this adds 107 covered lines (+109
total), seven covered branches (+8 total), 15 covered functions (+15 total),
and 135 covered regions (+144 total). The aggregate snapshot retains the LLVM
segment-normalization warning. `src/codecs/webp/encode/vp8/partition.rs` is
452/460 lines, 58/58 branches, 30/30 functions, and 673/727 regions; its seven
uncovered lines are defensive checkpoint-error propagation sites. The residual
file is 332/342 lines, 36/36 branches, 21/21 functions, and 492/530 regions;
its ten uncovered lines are the corresponding defensive propagation sites.
These are implementation and target records separate from Pillow parity.
Remaining finer WebP bitstream work beyond the first-partition, coefficient-bit,
and 1,024-byte output intervals, other codec interior work, transient allocation
accounting, short/interrupted output, rollback, and remaining non-checkpointed
work-budget semantics remain open.

The test-matrix runtime follow-up is implemented at revision
`62508a58b1a16fde150067b6cd43930b6e798dd3`. The feature-matrix harness now
defaults its isolated feature-gate test binaries to `MATRIX_TEST_OPT_LEVEL=1`
instead of level 0 because those lanes execute real codec work-budget and
cancellation contracts. All 33 target/feature lanes, all 45 feature-gate
assertions per lane, and the capability-table no-drift check remain unchanged;
this is a harness-only change with no production profile, manifest row,
fixture, assertion, or provenance-origin change.

On the same host and source tree, the all-feature WASI work-budget contract
took 8.66 seconds at level 0 and 0.60 seconds at level 1; the complete
45-test WASI lane took 16.26 seconds and 1.31 seconds respectively. These are
controlled local optimization observations, not universal benchmarks. Managed
Pillow parity run `07fd0f3d-d120-4cd8-8d9f-c6ded05a68b9` passed 1,445/1,445
checks with zero failures or skips in 1,923 ms. Managed feature-matrix run
`f817e089-b339-43b1-9bbc-1f234d8e35ba` passed 991/991 checks in 6,847 ms;
its retained log has no build-directory or package-cache lock-wait matches and
ends with `capability tables OK: every native and wasm32-wasip1 lane agrees`.
Coverage MCP run `a6e382b2-44d2-43a1-9105-7739009328eb` passed 85/85 tests
in 53,222 ms and ingested snapshot
`5b366b41-e605-43cb-9936-a081644cb707`, retaining 50,260/50,702 lines,
6,984/7,060 branches, 2,799/2,867 functions, and 78,142/79,032 regions.
The snapshot retains the existing LLVM segment-normalization warning. This is
harness runtime evidence, not a new codec capability or Pillow-parity row.

The current lossless WebP/VP8L bitstream-output checkpoint slice is implemented
at `cc6ed8fa71ccce70bcc5014a5bc8fb19f8734056`. Token-aware VP8L bit writing now
charges cancellation and work-budget checkpoints after each 1,024 newly
emitted output bytes, including final buffered-byte flushes; compression-search
trials preserve their checkpoint state when the shortest candidate is selected.
The ordinary no-token path uses a monomorphized no-op controller and preserves
existing bytes. The Rust-only
`encode_work_budget_is_a_non_parity_result_contract` contract uses the patterned
128x128 RGB probe to prove whole-buffer and direct-sink rejection at
`maximum: 56,000`, `observed: 56,001`, with the sink untouched. Pillow has no
caller token, work-budget result, or caller-owned sink, so this adds no parity
row, fixture, diagnostic origin, or coverage-only hook.

Managed Pillow parity run `dfc45aea-1e64-4017-aad6-b4b6e5edb277` passed
1,445/1,445 checks with zero failures or skips in 54,542 ms. Feature-matrix run
`64f0414e-b049-462f-a54d-d44b446e3d8a` passed 991/991 checks in 99,030 ms;
its retained log has no build-directory or package-cache lock-wait matches and
ends with `capability tables OK: every native and wasm32-wasip1 lane agrees`.
Coverage MCP run `da96b55e-13f9-4a82-b1da-3e8918429a9d` passed 85/85 tests in
51,124 ms and ingested snapshot `3713d12e-690f-4eb8-ba03-d11b8a2edde2`,
reporting 50,361/50,803 lines, 6,986/7,062 branches, 2,805/2,873 functions,
and 78,250/79,191 regions. Compared with snapshot
`5b366b41-e605-43cb-9936-a081644cb707`, this adds 101 covered lines (+101
total), two covered branches (+2 total), six covered functions (+6 total), and
108 covered regions (+159 total). `src/codecs/webp/native/encoder.rs` is
1,451/1,461 lines, 224/224 branches, 75/75 functions, and 2,100/2,204
regions; its ten uncovered lines are defensive cancellation/unexpected-token
and codec-error propagation edges, while the 13 partial-branch lines are
boundary alternatives in the writer and encoder paths. The aggregate snapshot
retains the LLVM segment-normalization warning. These are implementation and
target records separate from Pillow parity. Remaining finer VP8L bitstream work
beyond the 1,024-byte output interval, other codec interior work, transient
allocation accounting, short/interrupted output, rollback, and remaining
non-checkpointed work-budget semantics remain open.

The current VP8L logical-bitstream checkpoint slice is implemented at
`f7a8cd7efdf398c4df564ea29ffa2fcc99e6afdf`. Token-aware VP8L bit writing now
charges a checkpoint whenever the accumulated logical bit count crosses a
4,096-bit interval, while retaining the existing checkpoint after each 1,024
newly emitted output bytes, including final buffered-byte flushes. Compression
search trials preserve both counters when the shortest candidate is selected;
the no-token path remains a monomorphized no-op controller. The existing
Rust-only `encode_work_budget_is_a_non_parity_result_contract` uses a patterned
128x128 RGB probe to prove whole-buffer and direct-sink rejection at
`maximum: 56,000`, `observed: 56,001` for the logical-bitstream boundary, and
at `maximum: 55,999`, `observed: 56,000` for the emitted-output boundary; both
sinks remain untouched, and an ample budget preserves byte identity. Pillow
has no caller token, work-budget result, or caller-owned sink, so this adds no
parity row, fixture, diagnostic origin, or coverage-only hook.

Managed Pillow parity run `6e993f5a-d280-4fc5-8191-41086674d433` passed
1,445/1,445 checks with zero failures or skips in 43,482 ms. Feature-matrix run
`42260e83-2f2b-4d7b-9219-76c415a43f0c` passed 991/991 checks in 118,671 ms;
its retained log has no build-directory or package-cache lock-wait matches and
ends with `capability tables OK: every native and wasm32-wasip1 lane agrees`.
Coverage MCP run `f95bdb91-394f-461e-bc13-ea970997de88` passed 85/85 tests in
69,986 ms and ingested snapshot `109c8920-2045-4cfb-a894-b2e2842ccfbc`,
reporting 50,377/50,819 lines, 6,988/7,064 branches, 2,807/2,875 functions,
and 78,276/79,215 regions. Compared with snapshot
`3713d12e-690f-4eb8-ba03-d11b8a2edde2`, this adds 16 covered lines (+16 total),
two covered branches (+2 total), two covered functions (+2 total), and 26
covered regions (+24 total). `src/codecs/webp/native/encoder.rs` is 1,467/1,477
lines, 226/226 branches, 77/77 functions, and 2,126/2,228 regions; its ten
uncovered lines and 13 partial-branch lines remain defensive propagation or
boundary alternatives. The aggregate snapshot retains the LLVM
segment-normalization warning. These are implementation and target records
separate from Pillow parity; aggregate coverage includes the ordinary Rust
work-control contract incidentally.

Remaining finer VP8 bitstream work beyond the 4,096-bit logical first-partition,
16,384-boolean first-partition/coefficient-bit, and 1,024-byte output intervals;
finer VP8L bitstream work beyond the 4,096-bit logical and 1,024-byte output
intervals; other codec interior work, transient allocation accounting,
short/interrupted output, rollback, and remaining non-checkpointed work-budget
semantics remain open.

The current lossy VP8 first-partition logical-checkpoint slice is implemented at
`fb0d1e1cabb23fbdf0d1c64b91bd72f14025f9ed`. Token-aware first-partition boolean
coding now charges a checkpoint after each 4,096 logical coded bits, while the
existing 16,384-boolean first-partition boundary remains independently charged;
the coefficient-bit and 1,024-byte boolean-bitstream-output checkpoints remain
unchanged. The no-token path remains a monomorphized no-op controller. The
Rust-only `encode_work_budget_is_a_non_parity_result_contract` uses a patterned
896x512 RGB probe to prove whole-buffer and direct-sink rejection at
`maximum: 580`, `observed: 581` for the logical first-partition boundary, and at
`maximum: 582`, `observed: 583` for the coarser boolean first-partition boundary;
the existing output-boundary assertions remain `maximum: 589`, `observed: 590`
for whole-buffer and `maximum: 588`, `observed: 589` for the direct sink. All
bounded sinks remain untouched, and an ample budget preserves byte identity.
Pillow has no caller token, work-budget result, or caller-owned sink, so these
are Rust-only resource contracts with no parity row, fixture, diagnostic origin,
or coverage-only hook.

Managed Pillow parity run `31b37ae1-5529-435e-991e-3f8807ffa28c` passed
1,445/1,445 checks with zero failures or skips in 43,443 ms. Feature-matrix run
`1a112fdc-d3fd-4edf-9a0a-bb582e3ea789` passed 991/991 checks in 109,873 ms;
its retained log has no `lock-wait` matches and ends with `capability tables OK:
every native and wasm32-wasip1 lane agrees`. Coverage MCP run
`167567f8-a8da-4189-99b3-63b2d93ca2d9` passed 85/85 tests in 70,274 ms and
ingested snapshot `c12ed30e-2e3f-4c80-b8c7-14eb1eae417a`, reporting
50,383/50,826 lines, 6,990/7,066 branches, 2,807/2,875 functions, and
78,282/79,222 regions. Compared with snapshot
`109c8920-2045-4cfb-a894-b2e2842ccfbc`, this adds six covered lines (+7 total),
two covered branches (+2 total), no functions, and six covered regions (+7
total). The VP8 partition file is 460/467 lines, 60/60 branches, 30/30
functions, and 687/734 regions; its seven uncovered lines are existing
defensive/boundary alternatives. The aggregate snapshot retains the LLVM
segment-normalization warning. These implementation and target records remain
separate from Pillow parity; aggregate coverage includes the ordinary Rust
work-budget contract incidentally.

Remaining finer VP8 bitstream work beyond the 4,096-bit logical first-partition,
16,384-boolean first-partition/coefficient-bit, and 1,024-byte output intervals;
finer VP8L bitstream work beyond its 4,096-bit logical and 1,024-byte output
intervals; other codec interior work, transient allocation accounting,
short/interrupted output, rollback, and remaining non-checkpointed work-budget
semantics remain open.

The current lossy VP8 coefficient logical-checkpoint slice is implemented at
`18a400a27d0a1c28299cbe1f71fb06dfa732b3b5`. Token-aware coefficient boolean
coding now charges a checkpoint after each 4,096 logical coded bits, while the
existing 16,384-boolean coefficient-bit boundary and 1,024-byte emitted-output
boundary remain independently charged. The no-token path remains a
monomorphized no-op controller. The Rust-only
`encode_work_budget_is_a_non_parity_result_contract` uses the constant 512x512
RGB probe to prove whole-buffer and direct-sink rejection at `maximum: 439`,
`observed: 440` for the logical coefficient boundary, and retains the existing
coarser coefficient assertion at `maximum: 401`, `observed: 402`; both sinks
remain untouched. Pillow has no caller token, work-budget result, or
caller-owned sink, so this adds no parity row, fixture, diagnostic origin, or
coverage-only hook.

Managed Pillow parity run `9be396c7-6e6f-4d68-bfa8-73e735319559` passed
1,445/1,445 checks with zero failures or skips in 44,322 ms. Feature-matrix run
`4126647e-af51-4b03-854b-1e5e05d7b584` passed 991/991 checks in 103,270 ms;
its retained log has no `lock-wait` matches and ends with `capability tables OK:
every native and wasm32-wasip1 lane agrees`. Coverage MCP run
`9d550c42-c050-4e1c-ac71-97a0e82a7110` passed 85/85 tests in 70,261 ms and
ingested snapshot `d31517b4-00c5-4de3-9d38-e884424d9fa4`, reporting
50,390/50,833 lines, 6,992/7,068 branches, 2,807/2,875 functions, and
78,288/79,229 regions. Compared with snapshot
`c12ed30e-2e3f-4c80-b8c7-14eb1eae417a`, this adds seven covered lines (+7 total),
two covered branches (+2 total), no functions, and six covered regions (+7
total). The VP8 residual file is 337/349 lines, 38/38 branches, 21/21
functions, and 490/537 regions; its 11 uncovered lines are pre-existing
defensive/error-propagation or boundary alternatives. The aggregate snapshot
retains the LLVM segment-normalization warning. These implementation and target
records remain separate from Pillow parity; aggregate coverage includes the
ordinary Rust work-budget contract incidentally.

Remaining finer VP8 bitstream work beyond the 4,096-bit logical first-partition
and coefficient intervals, the 16,384-boolean first-partition/coefficient-bit
intervals, and the 1,024-byte output intervals; finer VP8L bitstream work beyond
its 4,096-bit logical and 1,024-byte output intervals; other codec interior work,
transient allocation accounting, short/interrupted output, rollback, and
remaining non-checkpointed work-budget semantics remain open.

The initial PNG Deflate work-budget checkpoint slice was implemented at
`66263c8ab08a4f488b3c378c5302477e2f5d9d48`. With a caller token, stored PNG
compression now checks input-chunk and stored-block boundaries plus the final
Adler-32 calculation; default level-six compression uses the shared token-aware
zlib-ng matcher, token expansion, Huffman/bitstream emission, and checksum
stages. The ordinary no-token path remains on the existing helpers, and PNG
compression levels other than 0 and 6 received only a boundary check before
and after their no-token helper in that initial slice. The Rust-only
`encode_work_budget_is_a_non_parity_result_contract` preserves ample-budget
bytes for default, stored, non-final stored-block, and maximum-level PNG
encodes; it proves the existing adaptive-filter rejection at `maximum: 3`,
`observed: 4`, and a default level-six Deflate matcher rejection at
`maximum: 20`, `observed: 21`, in both whole-buffer and direct-sink paths, with
both bounded sinks untouched. Pillow has no caller token, work-budget result,
or caller-owned sink, so this adds no parity row, fixture, diagnostic origin,
or coverage-only hook.

Managed Pillow parity run `d0a4587c-b46b-4747-aeed-b668e3a79e65` passed
1,445/1,445 checks with zero failures or skips in 996 ms. Feature-matrix run
`21c9561c-5b9f-4a7e-a52e-36b961a769a0` passed 991/991 checks in 39,496 ms;
its retained log has no `lock-wait` matches and ends with `capability tables OK:
every native and wasm32-wasip1 lane agrees`. Coverage MCP run
`080acf54-3acf-467a-b160-fabd6fd08d9d` passed 85/85 tests in 56,529 ms and
ingested snapshot `fecafd5b-7690-40c6-938b-78840ac60a72`, reporting
50,451/50,899 lines, 6,996/7,072 branches, 2,810/2,879 functions, and
78,399/79,350 regions. Compared with snapshot
`d31517b4-00c5-4de3-9d38-e884424d9fa4`, this adds 61 covered lines (+66 total),
four covered branches (+4 total), three covered functions (+4 total), and 111
covered regions (+121 total). `src/codecs/compression/deflate.rs` is
601/601 lines, 66/66 branches, 33/33 functions, and 1,113/1,124 regions;
the aggregate snapshot retains the LLVM segment-normalization warning. These
implementation and target records remain separate from Pillow parity;
aggregate coverage includes the ordinary Rust work-budget contract
incidentally.

Remaining deeper stored-block byte-copy interruption, finer VP8/VP8L bitstream
work, other codec interior work,
transient allocation accounting, short/interrupted output, rollback, and
remaining non-checkpointed work-budget semantics remain open.

The current PNG all-level Deflate checkpoint slice is implemented at
`a4bc2eace8ceacca2dd57eedde6a5555f518337c`. Token-aware PNG compression now
covers the level-one quick matcher, levels two through four early matcher,
level five medium matcher, default level six matcher, levels seven and eight
slow matcher, and level nine matcher, followed by the existing token-aware
expansion, Huffman/bitstream, and Adler-32 stages. The ordinary no-token paths
retain their existing byte model. The Rust-only
`encode_work_budget_is_a_non_parity_result_contract` proves ample-budget byte
identity for explicit levels 1–5 and 7–9, and bounded matcher rejection at
`maximum: 20` in whole-buffer and direct-sink paths for every newly covered
level, with bounded sinks untouched. Pillow has no caller token, work-budget
result, or caller-owned sink, so this adds no parity row, fixture, diagnostic
origin, or coverage-only hook.

Managed Pillow parity run `9e0edc6a-fece-4fae-9847-93d756126adc` passed
1,445/1,445 checks with zero failures or skips in 41,562 ms. Feature-matrix run
`e9675875-7299-4899-b2bb-988aa0b5dc40` passed 991/991 checks in 105,928 ms;
its retained log has no `lock-wait` matches and ends with `capability tables OK:
every native and wasm32-wasip1 lane agrees`. Coverage MCP run
`9a59b2bc-007e-4b98-b0ac-122f1fb5ca2b` passed 85/85 tests in 71,963 ms and
ingested snapshot `01651a2e-866b-432c-b298-39e077d8c053`, reporting
50,794/51,259 lines, 7,010/7,094 branches, 2,827/2,896 functions, and
78,932/79,965 regions. Compared with snapshot
`fecafd5b-7690-40c6-938b-78840ac60a72`, this adds 343 covered lines (+360
total), 14 covered branches (+22 total), 17 covered functions (+17 total),
and 533 covered regions (+615 total). `src/codecs/compression/deflate.rs` is
607/610 lines, 66/66 branches, 33/33 functions, and 1,129/1,148 regions;
`src/codecs/compression/zlib_ng.rs` is 2,270/2,286 lines, 408/416 branches,
111/111 functions, and 3,502/3,627 regions. The aggregate snapshot retains
the LLVM segment-normalization warning. These implementation and target
records remain separate from Pillow parity; no coverage-only test was added.

Remaining finer VP8/VP8L bitstream work, other codec interior work, transient
allocation accounting,
short/interrupted output, rollback, and remaining non-checkpointed work-budget
semantics remain open.

The PNG stored-block copy checkpoint slice is implemented at
`31a1c19d2f5503bc05911ff90b649fda44a1e7f0`. The token-aware level-0 path now
copies each stored block in 1,024-byte chunks and polls after each copied
interval; the ordinary no-token path remains a bulk byte append. The existing
Rust-only `encode_work_budget_is_a_non_parity_result_contract` proves ample
budget byte identity and rejects the first stored-block copy checkpoint at
`maximum: 164`, `observed: 165`, in both whole-buffer and direct-sink paths,
with the sink untouched. Pillow has no caller token, work-budget result, or
caller-owned sink, so this adds no parity row, fixture, diagnostic origin, or
coverage-only hook.

Managed Pillow parity run `eb9c7988-dacd-4b3b-a954-6abf0c59aef1` passed
1,445/1,445 checks with zero failures or skips in 45,812 ms. Feature-matrix run
`2d14c587-364b-44da-a3cb-6e6cdb1ace52` passed 991/991 checks in 90,938 ms;
its retained log contained the capability marker `capability tables OK: every
native and wasm32-wasip1 lane agrees` and no `lock-wait` match. Coverage MCP run
`c774a888-7a2b-4872-8660-e277088326fa` passed 85/85 tests in 72,844 ms and
ingested snapshot `33b8c596-907d-47e1-bc99-fbd8cfaf1d5e`, reporting
50,813/51,279 lines, 7,010/7,094 branches, 2,828/2,897 functions, and
78,965/79,999 regions. Compared with snapshot
`01651a2e-866b-432c-b298-39e077d8c053`, this adds 19 covered lines (+20 total),
no covered or total branches, one covered function (+1 total), and 33 covered
regions (+34 total). `src/codecs/compression/deflate.rs` is 626/630 lines,
66/66 branches, 34/34 functions, and 1,162/1,182 regions; four uncovered
lines remain in the aggregate and are recorded rather than hidden with a
coverage-only test. The LLVM JSON segment-normalization warning remains. These
implementation and target records remain separate from Pillow parity.

The feature-matrix scheduler cache-state follow-up is implemented at
`3a24dd85e507a777492267dfd13a01c508f392d3`. It preserves all 33
target/feature lanes, the 45 feature-gate assertions per lane, the capability
table no-drift check, and the existing explicit `MATRIX_*` overrides. A clean
root keeps the previously measured six-lane/two-worker compile profile; a
retained root with native, `wasm32-unknown-unknown`, and `wasm32-wasip1`
all-feature roots switches to up to twelve lanes, one compiler worker per
lane, and two test workers on the measured 12-logical-CPU host. Three local
warm runs passed 991/991 checks in 3.36–3.42 seconds. Managed feature-matrix
run `a662134e-64ff-412c-8dc0-c14944ac6014` passed 991/991 checks in 5,856 ms
at the exact revision; its retained log has no `lock-wait` match and ends with
`capability tables OK: every native and wasm32-wasip1 lane agrees`. These are
observed cache-state/runtime measurements, not universal benchmark claims;
the scheduler changes no production profile, fixture, parity row, assertion,
or evidence origin.

Remaining finer VP8/VP8L bitstream work, other codec interior work, transient
allocation accounting, short/interrupted output, rollback, and remaining
non-checkpointed work-budget semantics remain open.

The finer lossless WebP/VP8L logical-bitstream checkpoint slice is implemented
at `38af2d21830356eefa202f60f5b16c44934b8924`. Token-aware VP8L bit writing now
charges a checkpoint whenever the accumulated logical bit count crosses a
1,024-bit interval, while retaining the 1,024-byte emitted-output interval;
every fourth logical crossing therefore preserves the former 4,096-bit
boundary. The ordinary no-token path remains a monomorphized no-op controller
and preserves existing bytes. The Rust-only
`encode_work_budget_is_a_non_parity_result_contract` proves ample-budget byte
identity and the finer logical-bitstream rejection at `maximum: 55,996`,
`observed: 55,997`, in both whole-buffer and direct-sink paths, with both sinks
untouched. The existing coarser logical and emitted-output probes remain
covered separately. Pillow has no caller token, work-budget result, or
caller-owned sink, so this adds no parity row, fixture, diagnostic origin, or
coverage-only hook.

Managed Pillow parity run `14dfc194-5397-4eff-b8f4-40053a7ab1c4` passed
1,445/1,445 checks with zero failures or skips in 37,753 ms. Feature-matrix run
`ae5dda88-c11d-45e6-80c0-f266ce41ed23` passed 991/991 checks in 73,301 ms;
its retained log had no `lock-wait` match and ended with `capability tables OK:
every native and wasm32-wasip1 lane agrees`. Coverage MCP run
`716cf30f-561a-406e-bada-8a68b7f366e9` passed 85/85 tests in 47,510 ms and
ingested snapshot `5786f56a-8e4e-4cf4-b1ea-7f3fee2e2091`, reporting
50,813/51,279 lines, 7,010/7,094 branches, 2,828/2,897 functions, and
78,966/79,999 regions. Compared with snapshot
`33b8c596-907d-47e1-bc99-fbd8cfaf1d5e`, this adds no covered or total lines,
branches, or functions and one covered region (+0 total). The WebP encoder file
is 1,467/1,477 lines, 226/226 branches, 77/77 functions, and 2,127/2,228
regions; its ten uncovered lines remain defensive cancellation/unexpected-token
or codec-error propagation edges. The LLVM JSON segment-normalization warning
remains. These implementation and target records remain separate from Pillow
parity; aggregate coverage includes the ordinary Rust work-budget contract
incidentally.

The finer lossy WebP/VP8 first-partition logical-bitstream checkpoint slice is
implemented at `4bccbfe102d80c94a492a270a6605d5aaad4c645`. Token-aware
first-partition boolean coding now charges a checkpoint after each 1,024
logical coded bits, while retaining the existing 16,384-boolean first-partition
boundary, the 4,096-bit logical coefficient boundary, the 16,384-boolean
coefficient boundary, and the 1,024-byte boolean-bitstream-output boundary.
The no-token path remains a monomorphized no-op controller. The Rust-only
`encode_work_budget_is_a_non_parity_result_contract` uses the patterned
896x512 RGB probe to prove whole-buffer and direct-sink rejection at
`maximum: 577`, `observed: 578` for the finer logical first-partition
boundary, retains the earlier `maximum: 580`, `observed: 581` logical probe,
and proves the independent coarser first-partition boundary at `maximum: 598`,
`observed: 599`; the existing emitted-output probes remain `maximum: 589`,
`observed: 590` for whole-buffer and `maximum: 588`, `observed: 589` for the
direct sink. Every bounded sink remains untouched, and ample-budget bytes
remain identical. Pillow has no caller token, work-budget result, or
caller-owned sink, so this is Rust-only resource-contract evidence with no
parity row, fixture, diagnostic origin, or coverage-only hook.

Managed Pillow parity run `b711346e-bdab-4fe0-878f-2e3e6dbe76b0` passed
1,445/1,445 checks with zero failures or skips in 40,084 ms. Feature-matrix run
`00bfa332-c507-4f1c-93b6-7682703945a8` passed 991/991 checks in 40,974 ms;
its retained log had no `lock-wait` match and contained
`capability tables OK: every native and wasm32-wasip1 lane agrees`. The first
exact-command LLVM coverage attempt hit the existing native-AVIF sink-byte
assertion at 84/85; the immediate retry `b5c8d957-2506-4beb-9748-a4f7bdd880a2`
passed 85/85 in 45,925 ms and ingested snapshot
`dcaab996-685d-4470-ae30-a8d96790261f`, reporting 50,813/51,279 lines,
7,010/7,094 branches, 2,828/2,897 functions, and 78,962/79,999 regions.
Compared with snapshot `5786f56a-8e4e-4cf4-b1ea-7f3fee2e2091`, coverage
compare reports no line, branch, or function delta and no changed-to-uncovered
lines; the four-region aggregate decrease is retained, not hidden. The VP8
partition file is 460/467 lines, 60/60 branches, 30/30 functions, and
685/734 regions; its six uncovered lines remain defensive/boundary alternatives.
The LLVM JSON segment-normalization warning remains. These implementation and
target records remain separate from Pillow parity; aggregate coverage includes
the ordinary Rust work-budget contract incidentally.

Remaining finer VP8 bitstream work beyond its 1,024-bit logical first-partition,
1,024-bit logical coefficient, 16,384-boolean first-partition/coefficient-bit,
and 1,024-byte output intervals; finer VP8L bitstream work beyond its 1,024-bit
logical and 1,024-byte output intervals; other codec interior work, transient
allocation accounting, short/interrupted output, rollback, and remaining
non-checkpointed work-budget semantics remain open.

The native AVIF auxiliary-alpha provenance slice is implemented at
`bf9dda0de0ce8214cf525ccdba395fa99246d8a6`. The AVIF item graph now maps an
alpha auxiliary item to `SourceAlpha::Auxiliary` in native inspection, still
decode, and sequence decode. The committed `alpha.avif` fixture is asserted by
the feature-gated integration contract
`source_alpha_matches_the_container_contract`; this is Rust source-provenance
evidence, not a Pillow-parity row or a unit/coverage-only test. Pillow's parity
schema has no source descriptor or auxiliary-item provenance field, so parity
cannot express this contract; its unchanged result below is retained only as
outer-output regression evidence. Decoded normalized RGBA bytes are unchanged.

Managed Pillow parity run `002ee279-806e-4de5-acb9-3485f009c2a1` passed
1,445/1,445 checks with zero failures or skips in 41,696 ms. Feature-matrix run
`b204b2a7-d4f4-470c-b6aa-3698ff3a97d1` passed 991/991 checks in 7,053 ms and
retained `capability tables OK: every native and wasm32-wasip1 lane agrees`.
Coverage MCP run `712ff626-3b86-4cb5-aaf2-14b554761541` passed 85/85 tests in
49,016 ms and ingested snapshot `8e804ce4-ac81-4386-a283-c77a12dec7c5`:
50,813/51,279 lines, 7,012/7,096 branches, 2,828/2,897 functions, and
78,967/80,004 regions. Compared with snapshot
`dcaab996-685d-4470-ae30-a8d96790261f`, there is no line regression or
changed-to-uncovered line; the aggregate adds two covered/total branches and
five covered/total regions. The AVIF container and decode files are fully
covered in this snapshot. The LLVM normalization warning remains, and the
coverage-origin verifier still accounts for all 219 exact guards without
assigning any to Pillow parity.

The completed portion removes direct and supported grid-derived auxiliary alpha
and source-local AVIF `prem` relationship retention from the open provenance
gap. Remaining API-019/034/040 work is non-alpha auxiliary item properties and
relationships, grid topology, item identity/plane-range/quality details, and
invisible RGB semantics.

The AVIF premultiplied-alpha relationship slice is implemented at
`2d4b9f622923255617eac62669d32d489ead90c5`. `SourceDescriptor` now retains
bounded source-local `prem` `iref` edges through
`avif_premultiplied_relationships()` on native inspection, still decode, and
sequence-frame decode, while `SourceAlpha::Auxiliary` continues to identify
the separate alpha item. The feature-gated
`source_alpha_matches_the_container_contract` extends its existing real
`alpha.avif` contract with an in-memory `prem` child witness, asserts the
generic and filtered relationship views, and proves decoded normalized bytes
are unchanged. This is Rust source-provenance evidence: Pillow has no source
descriptor or item-relationship result field, so it adds no parity row,
fixture, diagnostic origin, new test function, or coverage-only hook.

Managed Pillow parity run `723e15eb-58e2-417f-9cc1-52c77f458fb4` passed
1,445/1,445 checks with zero failures or skips in 74,171 ms. Clean-revision
feature-matrix run `aace6bcd-f981-479a-97e9-1f6a03cc96ed` passed all 991
checks in 37,636 ms and retained `capability tables OK: every native and
wasm32-wasip1 lane agrees`; its targeted lock-wait/build-directory/package-
cache searches were empty. Coverage MCP run
`c5bcc4ae-1a8b-45cf-83f6-ce410acb8020` passed 85/85 tests in 108,208 ms and
ingested snapshot `5afb834b-bdb7-4f52-a29e-da99b9af4103`: 51,930/52,481
lines, 7,181/7,302 branches, 2,934/3,008 functions, and 80,393/81,607
regions. Compared with snapshot `c5b5dedb-0685-4222-9eee-89dbf6c0a55c`,
covered totals increased by 75 lines, 7 branches, 8 functions, and 96
regions; source totals grew by 75 lines, 8 branches, 8 functions, and 96
regions. The line-only comparison reports four displaced defensive records at
`src/codecs/avif/container.rs:1075`, `:1081`, `:1087`, and `:1167` after the
source-descriptor insertion; these remain visible rather than being hidden.
The LLVM JSON segment-normalization warning remains, and the strict aggregate
shortfall is 551 lines, 121 branches, 74 functions, and 1,214 regions. The
coverage-origin verifier still accounts for all 219 exact guards without
assigning any to Pillow parity.

The feature-matrix serial-tail overlap is
implemented at `da3dfbe43c90320c6cbf92ac7bcfea6bec71c1fe`: the two
`wasm32-unknown-unknown` `feature_gate_tests --no-run` checks run in their
matching `none` and `avif` lanes, and the all-feature
`wasm32-wasip1` determinism compile/run runs in the `all` lane. All 33
target/feature lanes remain, with 45 feature-gate assertions in each native
and WASI runtime lane (990 total), plus the determinism test and the
capability-table no-drift check. In a controlled fresh-root local comparison
with `MATRIX_JOBS=6`, `MATRIX_TEST_THREADS=2`, and `MATRIX_BUILD_JOBS=2`, the
pre-change harness at `842f8edbc2325022108e7fd494b2ec6b7f11c69d` took 81.21
seconds and the new harness took 69.61 seconds. Managed run
`1569fabd-04b5-483a-b971-20fd3e8aca76` passed 991/991 in 3,353 ms and retained
the native/WASI capability agreement marker with no `lock-wait` match. This is
cache- and runner-sensitive harness evidence, not a universal speedup claim;
it changes no production profile, fixture, parity row, diagnostic origin, or
coverage scope.

The finer lossy WebP/VP8 coefficient logical-bitstream checkpoint slice is
implemented at `e3f8d4bfb1f5687d0f5322519b776740748a82fc`. Token-aware residual
encoding now charges after each 1,024 logical coefficient bit crossings while
retaining the 16,384-boolean coefficient-bit and 1,024-byte emitted-output
intervals; the no-token path remains a monomorphized no-op controller. The
Rust-only `encode_work_budget_is_a_non_parity_result_contract` proves ample-
budget byte identity and whole-buffer/direct-sink rejection at the first
logical coefficient checkpoint (`maximum: 361`, `observed: 362`), with both
bounded sinks untouched. Pillow has no caller token, work-budget result, or
caller-owned sink, so this is Rust-only resource-contract evidence with no
parity row, fixture, diagnostic origin, or coverage-only hook.

Managed Pillow parity run `f54e8e8f-5413-425e-b8f2-288f99c45688` passed
1,445/1,445 checks with zero failures or skips in 50,344 ms. Feature-matrix run
`76b54e3e-b339-456e-88a6-c7ed7e3968f1` passed 991/991 checks in 38,706 ms and
retained the native/WASI capability agreement marker with no `lock-wait` match.
Coverage MCP run `7e76aa2c-6bd3-4a24-8029-45fbb6a2b333` passed 85/85 tests in
62,893 ms and ingested snapshot `4cc74646-c229-4ab5-92ec-a511434a893a`:
50,815/51,279 lines, 7,012/7,096 branches, 2,828/2,897 functions, and
78,970/80,004 regions. Compared with snapshot
`8e804ce4-ac81-4386-a283-c77a12dec7c5`, this adds two covered lines and three
covered regions, with no changed-to-uncovered lines, branch delta, or function
delta. The residual file reports 339/349 lines, 38/38 branches, 21/21
functions, and 493/537 regions; retained line gaps remain recorded rather than
hidden. These are Rust-only implementation and target records separate from
Pillow parity.

The finer lossless WebP/VP8L logical-bitstream checkpoint slice is implemented
at `d8abbfb228e53dc704cae8571959e594486fd60c`. Token-aware VP8L bit writing
now charges after each 512-bit logical-bitstream interval while retaining the 1,024-byte
emitted-output interval; compression-search trials preserve their checkpoint
state when the shortest candidate is selected, and the no-token path remains a
monomorphized no-op controller. The existing Rust-only
`encode_work_budget_is_a_non_parity_result_contract` proves ample-budget byte
identity and whole-buffer/direct-sink rejection at the first logical checkpoint
(`maximum: 54,823`, `observed: 54,824`), with both bounded sinks untouched.
Pillow has no caller token, work-budget result, or caller-owned sink, so this is
Rust-only resource-contract evidence with no parity row, fixture, diagnostic
origin, or coverage-only hook.

Managed Pillow parity run `e7fcfaba-c7e0-4c0b-910c-b9b5ed4081f0` passed
1,445/1,445 checks with zero failures or skips in 56,180 ms. Feature-matrix run
`13582bc0-0266-41ed-957b-651696df49a3` passed 991/991 checks in 64,693 ms and
retained the native/WASI capability agreement marker with no `lock-wait` match.
Coverage MCP run `d9b2ba47-76b3-45dc-a1c7-091e532153fb` passed 85/85 tests in
90,751 ms and ingested snapshot `09bee72c-c5cf-4c21-ac25-80fda41c1622`:
50,815/51,279 lines, 7,012/7,096 branches, 2,828/2,897 functions, and
78,970/80,004 regions. Compared with snapshot
`4cc74646-c229-4ab5-92ec-a511434a893a`, coverage has zero line, branch,
function, or region delta and no changed-to-uncovered lines. The VP8L encoder
file reports 1,467/1,477 lines, 226/226 branches, 77/77 functions, and
2,127/2,228 regions; retained gaps remain recorded rather than hidden. The
LLVM JSON segment-normalization warning remains. These are Rust-only
implementation and target records separate from Pillow parity.

The finer lossy WebP/VP8 first-partition logical-checkpoint slice is implemented
at `2af1eed8a117995b6965fde7461480d6586960b1`. Token-aware first-partition
boolean coding now charges after each 512 logical coded bits while retaining the
16,384-boolean first-partition boundary; the existing 1,024-bit logical
coefficient, 16,384-boolean coefficient, and 1,024-byte boolean-bitstream
output intervals remain unchanged. The Rust-only work-budget contract proves
the 512-bit logical boundary at `maximum: 593`, `observed: 594`, the
independent coarser first-partition boundary at `maximum: 613`, `observed:
614`, and the later coefficient-partition reach at `maximum: 700`, `observed:
701`; whole-buffer and direct-sink boundary probes remain separate and every
bounded sink stays untouched. No Pillow parity row or coverage-only hook was
added.

Managed Pillow parity run `805d09c5-9d04-45d9-afe9-d5e80629380c` passed
1,445/1,445 checks in 48,956 ms. Feature-matrix run
`31a5f5f0-a665-4d55-bcab-8ad166cf5eae` passed 991/991 checks in 51,687 ms and
retained the native/WASI capability agreement marker with no `lock-wait` match.
Coverage MCP run `9da0601f-f376-4acf-9d7a-6c5bf88b6781` passed 85/85 tests in
95,449 ms and ingested snapshot `bb9c8a0b-8d68-4b33-bfbc-0eea51aedb75`:
50,816/51,279 lines, 7,012/7,096 branches, 2,828/2,897 functions, and
78,975/80,004 regions. Compared with snapshot
`09bee72c-c5cf-4c21-ac25-80fda41c1622`, this adds one covered line and five
covered regions, with zero branch or function delta. The line-only regression
view still reports `src/codecs/webp/encode/vp8/residual.rs:220` as changed from
one hit to zero; the aggregate shortfall is named rather than hidden. The LLVM
JSON segment-normalization warning remains.

Historical acceptance record: grid-derived AVIF item provenance and warm matrix fanout

The bounded AVIF source-provenance slice is implemented at
`c8c18221d1d3126ac320cfc9a097386ddd007289` and its ordered primary-grid item
list plus existing feature-gated fixture contract were completed at
`fdd7afe988cf9a6b57de9bb69a98cc7dc8d690ca`. Coverage compilation completeness
for the existing structural-state initializers was fixed at
`8607dca5cf813448a8f95bbe62c6e5c07733ecef`.
The committed `grid.avif` fixture has primary item `1`, derived color items
`2` and `3`, and alpha auxiliary items `5` and `6` targeting `2` and `3`.
`SourceDescriptor::avif_auxiliary_relationships()` now retains those exact
source-local links on inspection, still decode, and the still-sequence
fallback; the scalar getter remains `None` because the grid has no direct
primary-item alpha link. `SourceDescriptor::avif_grid_item_ids()` retains the
ordered derived color-item IDs `[2, 3]` on the same three surfaces. The
existing `alpha.avif` contract also verifies the scalar direct link `2`→`1`
and the plural getter's one-element fallback. These descriptors record source
provenance only; they do not compose the grid, transform decoded pixels, or
claim non-alpha graph support.

This evidence deliberately stays outside Pillow parity: the parity schema has
no source descriptor or AVIF item-relationship field. The unchanged Pillow
result is therefore outer-output regression evidence only. The source contract
uses existing real fixtures, adds no test function, parity row, fixture,
diagnostic origin, or coverage-only hook. The test-runtime change at
`576fe356d22e936df04b4c96f1c36f6db5465fa6` is also harness-only: it derives up
to three warm test workers per lane from host CPUs and changes no production
profile or evidence origin. The follow-up at
`9ecf1cd26144aace1146e50784da362d19d40013` defaults the matrix-only
`MATRIX_DEBUG` budget to `0`, removing debugger symbols from isolated dev/test
artifacts while retaining `MATRIX_DEBUG=1` or `2` for local debugging. This
does not change the production profile or any coverage command.

Managed Pillow parity run `c87f6380-690e-4387-96ca-4ae49d1f45a3` passed
1,445/1,445 checks with zero failures or skips in 52,001 ms at the AVIF
implementation revision. Final feature-matrix run
`00558bec-d1de-4a10-9a2d-58b6dc7c5caa` passed 991/991 checks in 82,543 ms at
the source-contract revision; its retained log records `debug=0` and ends with
`capability tables OK: every native and wasm32-wasip1 lane agrees` and has no
build-directory or package-cache lock-wait matches. The runtime tuning itself
was validated separately by managed run `b78b4c94-72cb-45fb-a9e8-1fb4bb49be9e`
at the performance commit (991/991 in 3,501 ms); on the same warm local host,
the default changed from 4.51 s to 3.49 s. These timings are cache- and
runner-sensitive execution evidence, not universal benchmarks. The same
12-logical-CPU host measured the fresh isolated matrix at 72.63 s before that
follow-up and 60.33 s with the new default; a warm rerun of the new roots took
4.06 s. These are controlled local observations, not universal benchmarks,
and all 33 lanes and 991 checks remained enabled.

Managed feature-matrix rerun `77ceedff-203c-4be8-9556-97b993a37a23` at the
runtime follow-up revision passed 991/991 checks in 3,522 ms. Its retained log
records `debug=0`, ends with the native/WASI capability agreement marker, and
has no `lock-wait` match.

Coverage MCP run `44dfb288-00ce-4bb7-ab3a-723b57e67761` passed 85/85 tests in
47,453 ms and ingested snapshot `65e67f5a-a459-40f1-ae93-0fc91a233f39`:
51,285/51,764 lines, 7,083/7,176 branches, 2,871/2,941 functions, and
79,591/80,656 regions. Against baseline snapshot
`92f6ba37-f4eb-4ee8-aeb3-88e94856501a`, covered totals increased by 180 lines,
40 branches, 15 functions, and 270 regions. The line-only comparison reports
two displaced changed-to-uncovered records at
`src/codecs/avif/container.rs:1067` and `src/codecs/avif/container.rs:1079`.
The remaining named gaps are the duplicate-mirror and duplicate-clean-aperture
defensive branches at `src/codecs/avif/container.rs:1066-1067` and
`1078-1079`, the duplicate-alpha-association branch at
`src/codecs/avif/container.rs:1133-1134`, and three partial
`SourceDescriptor::is_empty` outcomes at `src/types/mod.rs:1075-1077`; they
remain visible rather than being hidden by synthetic tests. The LLVM JSON
segment-normalization warning remains.

Remaining AVIF categories are non-alpha and richer auxiliary graphs, item color
forms beyond typed `colr`/`nclx` CICP, grid topology/composition, gain
maps/depth/thumbnails/supplementary content,
premultiplication and plane/range/quality semantics, `iloc` extent variants,
content selection, invisible RGB, and fragmented-track/edit-list behavior.

Historical acceptance record: direct AVIF auxiliary relationship

The direct AVIF auxiliary-alpha relationship slice is implemented at
`fcff8dd9e9bebf22da8b7ee3dd3e93ae13798018` and finalized with the assertion-only
contract checkpoint `4c61ad60eab2be62dcad80f8f4b95550cae2688c`.
`SourceDescriptor::avif_auxiliary_relationship()` retains the direct source-local
`auxl` relationship from auxiliary item `2` to primary item `1` in the committed
`alpha.avif` fixture. The relationship is present on inspection, still decode,
and every sequence frame; it records provenance and does not transform decoded
pixels. Non-alpha auxiliary properties, derived/grid/track relationships,
plane range/quality, premultiplication, and invisible-RGB semantics remain open.

The existing feature-gated integration contract
`source_alpha_matches_the_container_contract` was extended to assert the public
relationship getters. No new test function, Pillow parity row, fixture,
diagnostic origin, or coverage-only hook was added. Pillow's parity schema has
no source descriptor or auxiliary-item identity field, so its unchanged result
is outer-output regression evidence only.

Managed Pillow parity run `4977e46c-43a0-4e3a-bedf-c6d11fdeeff3` passed
1,445/1,445 checks with zero failures or skips in 56,545 ms. Exact-revision
feature-matrix run `81ee974e-a13d-41ed-87d6-e02be077cce3` passed 991/991 checks
in 3,993 ms; its retained log contains the native/WASI capability agreement
marker and no build-directory or package-cache lock-wait match. The comparable
warm-runtime measurement remains 46,976 ms versus the preceding 52,870 ms at
the same scope after reducing warm-lane compiler workers from two to one; these
are cache- and runner-sensitive execution measurements, not universal benchmarks.
Coverage MCP run `3c34f53c-72d8-4240-8ebf-6595f24c7b8d` passed 85/85 tests in
49,923 ms and ingested snapshot `92f6ba37-f4eb-4ee8-aeb3-88e94856501a`:
51,105/51,579 lines, 7,043/7,130 branches, 2,856/2,925 functions, and
79,321/80,378 regions. The only changed-to-uncovered line against the prior
accepted snapshot is the defensive duplicate-alpha-association branch at
`src/codecs/avif/container.rs:1102`; the shortfall is recorded rather than
hidden.

Historical acceptance record: finer VP8 coefficient checkpoint

The finer lossy WebP/VP8 coefficient logical-bitstream checkpoint slice is
implemented at `7c8d97c4f23987a5876b830fd7cd9f1adfb444e9`. Token-aware
coefficient boolean coding now charges after each 512 logical coded bits,
retaining the 16,384-boolean coefficient-bit and 1,024-byte emitted-output
intervals; the no-token path remains a monomorphized no-op controller. The
existing `encode_work_budget_is_a_non_parity_result_contract` uses the same
512x512 RGB probe to prove ample-budget byte identity and whole-buffer/direct-
sink rejection at the fine coefficient boundary (`maximum: 820`, `observed:
821`), the independent 16,384-boolean coefficient boundary (`maximum: 647`,
`observed: 648`), and the coefficient macroblock boundary (`maximum: 466`,
`observed: 467`) in both paths; bounded sinks remain untouched. Pillow has no
caller token, work-budget result, or caller-owned sink, so this adds no parity
row, fixture, diagnostic origin, or coverage-only hook.

Managed Pillow parity run `674b807c-2cd0-4186-a61c-ea84b50c25ca` passed
1,445/1,445 checks with zero failures or skips in 44,511 ms. Feature-matrix
run `05c19cde-06ff-4952-bc8e-dd212629d637` passed 991/991 checks in 50,432 ms;
its retained log contains `capability tables OK: every native and wasm32-wasip1
lane agrees` and has no `lock-wait` match. Coverage MCP run
`83deedf2-4f4c-4053-bc66-7565e06fb36b` passed 85/85 tests in 79,504 ms and
ingested snapshot `9ec60a53-de8c-42c6-99fb-66ab2f1b5129`, reporting
50,816/51,279 lines, 7,012/7,096 branches, 2,828/2,897 functions, and
78,980/80,004 regions. Compared with baseline snapshot
`bb9c8a0b-8d68-4b33-bfbc-0eea51aedb75` at the prior implementation revision,
there is no line, branch, or function delta and five additional covered
regions; the line-only regression view names
`src/codecs/webp/encode/vp8/residual.rs:391` as changed from one hit to zero.
The LLVM JSON segment-normalization warning remains. These aggregate and
source-provenance records remain separate from Pillow parity, and no
coverage-only test was added.

Historical acceptance record: JPEG and WebP interior checkpoints and runtime slice

The JPEG baseline/progressive RGB-to-YCbCr and entropy-output checkpoint slice is
implemented at `9aeac06bfb27b643921d0c5231c5f83e3538e870`. Token-aware RGB
conversion preserves the existing row checks and now charges after each 1,024
converted pixels; token-aware entropy coding tracks the next 1,024-byte
emitted-output boundary without cumulative division on every observation. Both
ordinary no-token paths use monomorphized no-op controllers and preserve the
existing byte producer. The existing
`encode_work_budget_is_a_non_parity_result_contract` proves the new wide-row
conversion boundary with a 2,048x1 RGB probe (`maximum: 3`, `observed: 4`) in
whole-buffer and direct-sink paths, with the sink untouched; the patterned 64x64
RGB entropy probe remains (`maximum: 150`, `observed: 151`) with sentinel
`0x5b`. This is Rust-only resource-contract evidence: Pillow has no caller
token, work-budget result, or caller-owned sink, so it adds no parity row,
fixture, diagnostic origin, or coverage-only hook.

The lossy WebP VP8 RGBA transparent-area cleanup slice is implemented at
`9aeac06bfb27b643921d0c5231c5f83e3538e870`. Token-aware cleanup now charges
after each 1,024 scanned or flattened pixels, while the ordinary no-token path
retains its bulk fill helper through a monomorphized no-op controller. The same
Rust-only contract uses a 128x128 all-transparent RGBA probe to prove ample
budget byte identity, then rejects at `maximum: 400`, `observed: 401` in both
whole-buffer and direct-sink paths with sentinel `0xb4` untouched. Pillow has
no caller token, work-budget result, or caller-owned sink, so this adds no
parity row, fixture, diagnostic origin, or coverage-only hook.

The finer lossy WebP VP8 coefficient logical-bitstream checkpoint slice is
implemented at `9aeac06bfb27b643921d0c5231c5f83e3538e870`. Token-aware
coefficient boolean coding now charges after each 256 logical coded bits while
retaining the existing 512-bit logical, 16,384-boolean coefficient-bit, and
1,024-byte emitted-output intervals. The same Rust-only contract uses the
existing 512x512 RGB probe to reject at `maximum: 820`, `observed: 821` for
the 256-bit boundary, then at `maximum: 821`, `observed: 822` for the retained
512-bit boundary, in both whole-buffer and direct-sink paths with sentinels
`0xb5` and `0xb3` untouched. Pillow has no caller token, work-budget result,
or caller-owned sink, so this adds no parity row, fixture, diagnostic origin,
or coverage-only hook.

The finer lossy WebP VP8 first-partition logical-bitstream checkpoint slice is
implemented at `9aeac06bfb27b643921d0c5231c5f83e3538e870`. Token-aware
first-partition boolean coding now charges after each 256 logical coded bits
while retaining the existing 512-bit logical, 16,384-boolean first-partition,
and 1,024-byte emitted-output intervals. The same Rust-only contract uses the
patterned 896x512 RGB probe to reject at `maximum: 334`, `observed: 335` in
both whole-buffer and direct-sink paths with sentinel `0xb7` untouched. Pillow
has no caller token, work-budget result, or caller-owned sink, so this adds no
parity row, fixture, diagnostic origin, or coverage-only hook.

The finer lossless WebP/VP8L logical-bitstream checkpoint slice is implemented
at `9aeac06bfb27b643921d0c5231c5f83e3538e870`. Token-aware VP8L bit writing
now charges after each 256 logical coded bits while retaining the existing
512-bit logical and 1,024-byte output intervals. The same Rust-only contract
uses the patterned 128x128 RGB lossless probe to reject at `maximum: 54,820`,
`observed: 54,821` for the finer 256-bit boundary and at `maximum: 54,823`,
`observed: 54,824` for the retained 512-bit boundary in both whole-buffer and
direct-sink paths, with sentinels `0xab` and `0xaa` untouched. Pillow has no
caller token, work-budget result, or caller-owned sink, so this adds no parity
row, fixture, diagnostic origin, or coverage-only hook.

The runtime-first matrix slice keeps feature lanes isolated, avoids the shared
Cargo lock, and propagates native/WASI child failures instead of masking them
behind capability-table output. Warm retained roots on the measured
12-logical-CPU host now use one Cargo build worker per lane; explicit
overrides remain available. The exact-head managed matrix passed 991/991 in
46,976 ms, down from the preceding 52,870 ms run at the same scope, and its
retained log ends with the native/WASI capability agreement marker with no
build-directory or package-cache lock-wait match. These are execution
measurements, not controlled universal benchmarks.

Managed Pillow parity run `95fa9817-5693-4a82-9188-3e2de83af18f` passed
1,445/1,445 checks with zero failures or skips in 45,497 ms. Feature-matrix run
`204d59f1-a261-4152-871a-035ead6b464b` passed 991/991 checks in 52,870 ms and
retained the native/WASI capability agreement marker with no build-directory or
package-cache lock-wait match. Coverage MCP run
`4898fcc9-4d09-4d37-b6d8-77cd6cafcd98` passed 87/87 tests in 82,830 ms and
ingested snapshot `5a8b1512-2377-4d21-8951-dd1430d2b653`:
51,010/51,483 lines, 7,031/7,116 branches, 2,846/2,915 functions, and
79,219/80,272 regions. Compared with baseline snapshot
`73947df4-7548-4e22-a789-e739671f57a8`, covered totals changed by +5 lines,
+2 branches, +0 functions, and +7 regions; total source metrics grew by +5
lines, +2 branches, +0 functions, and +9 regions. The line-only comparison
retains six changed-to-uncovered line-number records in
`src/codecs/webp/native/encoder.rs` at lines 476, 602, 783, 1217, 1225, and
1400; aggregate covered totals increased and the LLVM JSON
segment-normalization warning remains. These are existing defensive/error-
propagation mappings, not a reason to add a synthetic coverage hook. These
aggregate and source-provenance records remain separate from Pillow parity, and
no coverage-only test was added.

Earlier acceptance record: JPEG forward-DCT/quantization checkpoint

The JPEG forward-DCT and quantization checkpoint slice is implemented at
`57d5bc3251c43ddc64857463a6faafaa91aaf2d3`. `FdctCheckpoint` keeps the
ordinary no-token path on an inline no-op implementation while the token-aware
path checks at each block row and after every completed 8x8 forward-DCT and
quantization block. The existing
`encode_work_budget_is_a_non_parity_result_contract` uses the committed
`tests/fixtures/input/images/jpeg/33x33.jpg` fixture, proves ample-budget byte
identity, and rejects at `maximum: 70`, `observed: 71` in both whole-buffer and
direct-sink paths; the direct sink remains `[0x5d]` because the checkpoint is
reached before output admission. Pillow has no caller token, work-budget
result, or caller-owned sink, so this is Rust-only resource-contract evidence:
no parity row, fixture, diagnostic origin, new test function, or coverage-only
hook was added.

Managed Pillow parity run `7492a510-409c-4283-a493-906fd65d09c4` passed
1,445/1,445 checks with zero failures or skips in 50,668 ms. The exact-head
feature-matrix run `c8f0e3dc-9158-4c91-82f6-1a7f0ffa5713` passed 991/991 checks
in 30,603 ms; its retained log records
`cache=warm lanes=12 test_threads=3 build_jobs=1 debug=0`, ends with
`capability tables OK: every native and wasm32-wasip1 lane agrees`, and has no
`lock-wait` match.

Coverage MCP rerun `43ec7eb4-9ad2-498c-bbd9-5bd16ce32b23` passed 85/85 tests
in 48,333 ms and ingested snapshot
`a4c6cea0-6547-4ea4-9367-646832657586`, reporting 51,313/51,793 lines,
7,085/7,178 branches, 2,877/2,947 functions, and 79,623/80,697 regions.
Against the prior accepted snapshot
`65e67f5a-a459-40f1-ae93-0fc91a233f39`, covered totals increased by 28 lines,
2 branches, 6 functions, and 32 regions. The line-only view records 20
displaced JPEG line records after the source expansion; the new checkpoint
functions are covered. The JPEG encoder retains 31 uncovered lines and 19
partial branch lines in existing defensive sink/parser paths, while the
AVIF duplicate-property and `SourceDescriptor::is_empty` gaps remain named in
the roadmap. The LLVM JSON segment-normalization warning remains. These
aggregate and source-provenance records remain separate from Pillow parity,
and no coverage-only test was added.

Earlier acceptance record: JPEG chroma-downsample checkpoint

The JPEG chroma-downsample checkpoint slice is implemented at
`64851f7167099721f05f6cb67872e1a20e5f20e6`. `DownsampleCheckpoint` keeps the
ordinary no-token path on an inline no-op implementation while the token-aware
path retains the row checks and adds a checkpoint after each 1,024 produced
chroma pixels in both the full-size and filtered branches. The existing
`encode_work_budget_is_a_non_parity_result_contract` uses the committed
`tests/fixtures/input/images/jpeg/large.jpg` fixture (257x129), proves ample-
budget byte identity, and rejects at `maximum: 228`, `observed: 229` in both
whole-buffer and direct-sink paths; the direct sink remains `[0x5e]`. Pillow
has no caller token, work-budget result, or caller-owned sink, so this is
Rust-only resource-contract evidence: no parity row, parity fixture,
diagnostic origin, new test function, or coverage-only hook was added.

Managed Pillow parity run `a98203b5-3334-4267-b1fc-7897c55793bb` passed
1,445/1,445 checks with zero failures or skips in 44,943 ms. The exact-head
feature-matrix run `d3b167c1-6363-464d-abbe-94e4a7746385` passed 991/991
checks in 50,749 ms; its retained log records
`cache=warm lanes=12 test_threads=3 build_jobs=1 debug=0`, ends with
`capability tables OK: every native and wasm32-wasip1 lane agrees`, and has no
`lock-wait` match.

Coverage MCP run `fe8d2ba4-ca03-4a24-857a-43dd910f5378` passed 85/85 tests
in 101,186 ms and ingested snapshot
`05d26dbd-c771-4e9c-bad6-2cad7dedb802`, reporting 51,353/51,833 lines,
7,089/7,182 branches, 2,883/2,953 functions, and 79,674/80,753 regions.
Against the prior accepted snapshot
`a4c6cea0-6547-4ea4-9367-646832657586`, covered totals increased by 40 lines,
4 branches, 6 functions, and 51 regions; source totals grew by 40 lines,
4 branches, 6 functions, and 56 regions. The JPEG file is 1,414/1,477 lines,
182/202 branches, 76/81 functions, and 2,295/2,373 regions covered, with 31
uncovered lines and 21 partial branch lines. The line-only comparison retains
19 displaced changed-to-uncovered JPEG line records from LLVM source remapping;
the new downsample checkpoint functions and lines are covered. The
segment-normalization warning remains, and no coverage-only test was added.

Earlier acceptance record: JPEG optimized-Huffman frequency checkpoint

The JPEG optimized-baseline-Huffman frequency checkpoint slice is implemented
at `7d7be29a7c3a2dd14b3b3937790983559997803b`. `HuffmanFrequencyCheckpoint`
keeps the ordinary no-token path on an inline no-op implementation while the
token-aware path retains the existing MCU-row checks and adds a checkpoint
after each 1,024 AC coefficients during optimized baseline frequency
gathering. The existing
`encode_work_budget_is_a_non_parity_result_contract` uses the committed
`tests/fixtures/input/images/jpeg/large.jpg` fixture (257x129) with
`optimize=true`, proves ample-budget byte identity, and rejects at
`maximum: 1,220`, `observed: 1,221` in both whole-buffer and direct-sink
paths; the direct sink remains `[0x5f]`. Pillow has no caller token,
work-budget result, or caller-owned sink, so this is Rust-only
resource-contract evidence: no parity row, parity fixture, diagnostic origin,
new test function, or coverage-only hook was added.

Managed Pillow parity run `db1c83cd-566c-4be1-9b31-c0e871abffc8` passed
1,445/1,445 checks with zero failures or skips in 44,331 ms. The exact-head
feature-matrix run `83835d3a-9a40-4c25-bcbb-d02b947d787d` passed 991/991
checks in 51,819 ms; its retained log records
`cache=warm lanes=12 test_threads=3 build_jobs=1 debug=0`, ends with
`capability tables OK: every native and wasm32-wasip1 lane agrees`, and has no
`lock-wait` match.

Coverage MCP run `c06f790c-191f-4e1b-ae89-d2d74d3877cf` passed 85/85 tests
in 77,637 ms and ingested snapshot
`c3b1373a-f326-49a3-9817-4fa39d39dce9`, reporting 51,391/51,871 lines,
7,093/7,186 branches, 2,889/2,959 functions, and 79,723/80,803 regions.
Against the prior accepted snapshot
`05d26dbd-c771-4e9c-bad6-2cad7dedb802`, covered totals increased by 38 lines,
4 branches, 6 functions, and 49 regions; source totals grew by 38 lines,
4 branches, 6 functions, and 50 regions. The JPEG file is 1,452/1,515 lines,
186/206 branches, 82/87 functions, and 2,344/2,423 regions covered, with 31
uncovered lines and 22 partial branch lines. The line-only comparison retains
19 displaced changed-to-uncovered JPEG line records from LLVM source remapping;
the new optimized-frequency checkpoint functions and lines are covered. The
segment-normalization warning remains, and no coverage-only test was added.

Earlier acceptance record: JPEG progressive scan-event checkpoint

The JPEG progressive scan-event checkpoint slice is implemented at
`fdeb8190c1373f39248c22af7870c7392e15bac9`. `ProgressiveScanCheckpoint` keeps
the ordinary no-token path on an inline no-op implementation while the
token-aware path retains row checks and charges after each 1,024 DC/AC scan
block slots, including interleaved padding slots. The existing
`encode_work_budget_is_a_non_parity_result_contract` uses the committed
`tests/fixtures/input/images/jpeg/large.jpg` fixture (257x129) with
`progressive=true`, proves ample-budget byte identity, and rejects at
`maximum: 1,364`, `observed: 1,365` in both whole-buffer and direct-sink
paths; the direct sink remains `[0x60]`. Pillow has no caller token,
work-budget result, or caller-owned sink, so this is Rust-only
resource-contract evidence: no parity row, parity fixture, diagnostic origin,
new test function, or coverage-only hook was added.

Managed Pillow parity run `794bd3d0-9034-4c82-bde0-935398d0a38d` passed
1,445/1,445 checks with zero failures or skips in 58,194 ms. The exact-head
feature-matrix run `e362a6fd-ebaf-4a4f-bf99-683d2c7c6371` passed 991/991
checks in 62,338 ms; its retained log records
`cache=warm lanes=12 test_threads=3 build_jobs=1 debug=0`, ends with
`capability tables OK: every native and wasm32-wasip1 lane agrees`, and has no
`lock-wait` match.

Coverage MCP run `1c19b74a-2e29-414c-976c-abe9fdf3d0c3` passed 85/85 tests
in 91,181 ms and ingested snapshot
`1acfb775-0acd-49fe-83eb-a438e3a72e6c`, reporting 51,428/51,908 lines,
7,095/7,188 branches, 2,894/2,964 functions, and 79,764/80,847 regions.
Against the prior accepted snapshot
`c3b1373a-f326-49a3-9817-4fa39d39dce9`, covered totals increased by 37 lines,
2 branches, 5 functions, and 41 regions; source totals grew by 37 lines,
2 branches, 5 functions, and 44 regions. The JPEG file is 1,489/1,552
lines, 188/208 branches, 87/92 functions, and 2,385/2,467 regions covered,
with 31 uncovered lines and 23 partial branch lines. The line-only comparison
retains 23 displaced changed-to-uncovered JPEG line records from LLVM source
remapping; the new progressive checkpoint functions and lines are covered.
The segment-normalization warning remains, and no coverage-only test was added.

Earlier acceptance record: JPEG progressive event-frequency checkpoint

The JPEG progressive scan-event frequency checkpoint slice is implemented at
`66097efaa012062a636f6525c1ccf36e0b5f8dbd`. `ProgressiveScanCheckpoint` keeps
the ordinary no-token path on an inline no-op implementation while the
token-aware path additionally counts the existing event vector during each
progressive scan's Huffman-frequency gathering and polls after each 1,024
events. The earlier block-slot checkpoint remains a separate boundary. The
existing `encode_work_budget_is_a_non_parity_result_contract` uses the
committed `tests/fixtures/input/images/jpeg/large.jpg` fixture (257x129) with
`progressive=true`, proves ample-budget byte identity, and rejects at
`maximum: 1,378`, `observed: 1,379` in both whole-buffer and direct-sink
paths; the direct sink remains `[0x61]`. Pillow has no caller token,
work-budget result, or caller-owned sink, so this is Rust-only
resource-contract evidence: no parity row, parity fixture, diagnostic origin,
new test function, or coverage-only hook was added.

Managed Pillow parity run `b4a5c443-68fd-4baf-a63f-9d282c78ae1c` passed
1,445/1,445 checks with zero failures or skips in 56,835 ms. The exact-head
feature-matrix run `da1d6843-a86d-4e3e-8d00-0f7b309afb78` passed 991/991 checks
in 61,543 ms; its retained log records
`cache=warm lanes=12 test_threads=3 build_jobs=1 debug=0`, ends with
`capability tables OK: every native and wasm32-wasip1 lane agrees`, and has no
`lock-wait` match.

Coverage MCP run `c671c245-ab6b-4306-bc8a-fb9f5ed3f5db` passed 85/85 tests in
91,124 ms and ingested snapshot
`4bc35eee-3a0d-4b1b-8d8c-48ebfe19427c`, reporting 51,441/51,921 lines,
7,097/7,190 branches, 2,896/2,966 functions, and 79,780/80,863 regions.
Against the prior accepted snapshot
`1acfb775-0acd-49fe-83eb-a438e3a72e6c`, covered totals increased by 13 lines,
2 branches, 2 functions, and 16 regions; source totals grew by 13 lines,
2 branches, 2 functions, and 16 regions. The JPEG file is 1,502/1,565
lines, 190/210 branches, 89/94 functions, and 2,401/2,483 regions covered,
with 31 uncovered lines and 23 partial branch lines. The line-only comparison
retains 21 displaced changed-to-uncovered JPEG/lib records from LLVM source
remapping; the new event-frequency checkpoint lines are covered. The
segment-normalization warning remains, and no coverage-only test was added.

Earlier acceptance record: JPEG progressive coefficient checkpoint

The JPEG progressive scan coefficient checkpoint slice is implemented at
`907c8c88544ad56e06251737186c3a1eddfab183`. `ProgressiveScanCheckpoint` keeps
the ordinary no-token path on an inline no-op implementation while the
token-aware path charges each AC coefficient traversal item during progressive
first/refinement scan event generation and polls after each 1,024 coefficients.
The earlier block-slot and event-frequency checkpoints remain separate
boundaries. The existing `encode_work_budget_is_a_non_parity_result_contract`
uses the constant `DecodedImage::new(257, 129, vec![0; 257 * 129 * 3],
ColorType::Rgb8)` probe with `progressive=true`, proves ample-budget byte
identity, and rejects at `maximum: 1,378`, `observed: 1,379` in both
whole-buffer and direct-sink paths; the direct sink remains `[0x62]`. Pillow
has no caller token, work-budget result, or caller-owned sink, so this is
Rust-only resource-contract evidence: no parity row, parity fixture, diagnostic
origin, new test function, or coverage-only hook was added.

Managed Pillow parity run `aea30bf1-e3f7-477a-9f1c-d4bcfb5f94b5` passed
1,445/1,445 checks with zero failures or skips in 45,735 ms. The exact-head
feature-matrix run `1697d339-6436-414f-b0d8-dffc373ec0ee` passed 991/991 checks
in 50,038 ms; its retained log records
`cache=warm lanes=12 test_threads=3 build_jobs=1 debug=0`, ends with
`capability tables OK: every native and wasm32-wasip1 lane agrees`, and has no
`lock-wait` match.

Coverage MCP run `1ae11279-fe21-4bc8-9113-1924731b4325` passed 85/85 tests in
79,401 ms and ingested snapshot
`43abaa1a-cb03-4809-939d-885e9440d504`, reporting 51,463/51,944 lines,
7,099/7,192 branches, 2,898/2,968 functions, and 79,811/80,902 regions.
Against the prior accepted snapshot
`4bc35eee-3a0d-4b1b-8d8c-48ebfe19427c`, covered totals increased by 22 lines,
2 branches, 2 functions, and 31 regions; source totals grew by 23 lines,
2 branches, 2 functions, and 39 regions. The JPEG file is 1,524/1,588 lines,
192/212 branches, 91/96 functions, and 2,432/2,522 regions covered, with 32
uncovered lines and 27 partial branch lines. The line-only comparison retains
21 displaced changed-to-uncovered JPEG/lib records from LLVM source remapping;
the new coefficient checkpoint lines are covered. The only coverage insight is
the known LLVM JSON segment-normalization warning; no coverage-only test was
added.

Historical acceptance record: feature-matrix successful-log reduction

The feature-matrix harness follow-up is implemented at
`24c1bf6dbf103bab30ac6499e27267361d28a494`. Successful native and WASI lanes
now emit one compact status line by default while retaining their complete
run-scoped logs for the capability-table no-drift check; a failed lane still
replays its full log, and `MATRIX_VERBOSE=1` restores full successful-lane
replay. This is a test-harness-only change: it removes parent-process output
I/O without changing the 33 lanes, the 991-check scope, any fixture, parity
row, assertion origin, diagnostic contract, production profile, or coverage
hook.

On the same warm local host, the pre-change successful-log replay took 7.37 s
and the quiet default took 4.19 s in the first direct repeat. These are
observed I/O-sensitive execution measurements, not universal benchmarks.
Managed matrix run `3d0bb595-e7b3-4dc6-8f7e-6f4917df0854` passed in 6,037 ms;
its retained log contains 33 passing lane markers, records
`cache=warm lanes=12 test_threads=3 build_jobs=1 debug=0 verbose=0`, ends with
`capability tables OK: every native and wasm32-wasip1 lane agrees`, and has no
build-directory or package-cache lock-wait matches.

Historical acceptance record: WebP VP8 128-bit first-partition checkpoint

The finer lossy WebP/VP8 first-partition checkpoint is implemented at
`fca00abc3ece718d49c4ca774d0e4428566f9625`. `TokenPartitionCheckpoint` now
charges a logical poll after each 128 boolean-coded bits while retaining the
256-bit, 512-bit, and 16,384-boolean intervals. The existing
`encode_work_budget_is_a_non_parity_result_contract` proves the new boundary
with the 512x512 analysis probe at `maximum: 333`, `observed: 334` in both
whole-buffer and direct-sink paths; the direct sink remains `[0xB8]`. This is
Rust-only resource-contract evidence: no Pillow row, parity fixture,
diagnostic origin, new test function, or coverage-only hook was added.

Managed Pillow parity run `3c9bcb42-a744-4a4d-abd4-d067bb785528` passed
1,445/1,445 checks in 45,231 ms. The exact-head feature-matrix run
`19e84fd6-d70c-4be4-91c2-71e123b12352` passed in 50,436 ms at the same
revision; its retained log records 33 passing lane markers and
`cache=warm lanes=12 test_threads=3 build_jobs=1 debug=0 verbose=0`, ends with
`capability tables OK: every native and wasm32-wasip1 lane agrees`, and has no
build-directory, package-cache, or lock-wait matches. Coverage MCP run
`29aaba64-8a13-4014-aad0-9423393e8c49` passed 85/85 tests in 78,442 ms and
ingested snapshot `117e1e18-2448-4461-9c51-453006189ccf`, reporting
51,470/51,951 lines, 7,100/7,194 branches, 2,898/2,968 functions, and
79,810/80,909 regions. The known LLVM JSON segment-normalization warning
remains; no coverage-only test was added.

Historical acceptance record: WebP VP8 128-bit coefficient checkpoint

The finer lossy WebP/VP8 coefficient checkpoint is implemented at
`589c01495ad3b8e7a3d2dda5b072d689b2e62818`. `TokenCoefficientCheckpoint`
now charges a logical poll after each 128 boolean-coded coefficient bits
while retaining the 256-bit, 512-bit, and 16,384-boolean intervals. The
existing `encode_work_budget_is_a_non_parity_result_contract` proves the
128-bit boundary at `maximum: 820`, `observed: 821`, the retained 256-bit
boundary at `maximum: 824`, `observed: 825`, and the retained 512-bit
boundary at `maximum: 832`, `observed: 833`, in both whole-buffer and
direct-sink paths; the direct-sink sentinels remain `[0xB5]`, `[0xB3]`,
and `[0xB9]`. It also recalibrates the existing token, macroblock, block,
and 16,384-bit boundary assertions after the added poll. This is Rust-only
resource-contract evidence: no Pillow row, parity fixture, diagnostic
origin, new test function, or coverage-only hook was added.

Managed Pillow parity run `e40fd1fe-8d24-4e95-98ad-166d8f2b5bbe` passed
1,445/1,445 checks in 40,256 ms. The exact-head feature-matrix run
`4793928e-7bff-488c-89e5-0136b0d38663` passed in 46,680 ms; its retained
log records 33 passing lane markers and
`cache=warm lanes=12 test_threads=3 build_jobs=1 debug=0 verbose=0`, ends
with `capability tables OK: every native and wasm32-wasip1 lane agrees`,
and has no `lock-wait`, `build-directory`, or `package-cache` matches.
Coverage MCP run `d59a57b1-5a6f-42d8-8e04-d3b3411e343c` passed 85/85 tests
in 58,887 ms and ingested snapshot
`5abf0bb8-7c28-4b76-9998-8e25f016ad62`, reporting 51,477/51,958 lines,
7,102/7,196 branches, 2,898/2,968 functions, and 79,821/80,916 regions.
The known LLVM JSON segment-normalization warning remains. The parity run
is Pillow-oracle evidence; the policy assertions and aggregate coverage are
implementation/Rust-only evidence. In the same snapshot,
`src/codecs/webp/encode/vp8/residual.rs` has 353/363 covered lines,
42/42 covered branches, and 21/21 covered functions; nine source lines
remain uncovered, and no coverage-only hook was used.

Historical acceptance record: WebP VP8L 128-bit bitstream checkpoint

The finer lossless WebP/VP8L logical bitstream checkpoint is implemented at
`22281579a15d99ead08ff40c6459620dfbc0fea6`. `TokenBitWriterCheckpoint` now
charges a logical poll after each 128 written bit while retaining the
256-bit, 512-bit, and 1,024-byte output intervals. The existing
`encode_work_budget_is_a_non_parity_result_contract` proves the 128-bit
boundary at `maximum: 56010`, `observed: 56011` in the whole-buffer path and
at `maximum: 56009`, `observed: 56010` in the direct-sink path; it retains the
256-bit boundary at 56185/56186 (return) and 56184/56185 (sink), the 512-bit
boundary at 56186/56187, and the 1,024-byte output boundary at 56109/56110
(return) and 56108/56109 (sink). The direct sink retains `[0xAB]`/`[0xAA]`
prefixes. This is Rust-only work-control evidence: no Pillow row, parity
fixture, diagnostic origin, new test function, or coverage-only hook was
added.

Managed Pillow parity run `2f3fe601-09a2-4189-b026-c8bd4cf868e1` passed
1,445/1,445 checks in 43,801 ms. The exact-head feature-matrix run
`484aa790-5c9d-4e92-9515-2ddfebb6a419` passed in 57,610 ms at the same
revision; its retained log records
`cache=warm lanes=12 test_threads=3 build_jobs=1 debug=0 verbose=0`, ends
with `capability tables OK: every native and wasm32-wasip1 lane agrees`, and
has no build-directory, package-cache, or lock-wait matches. Coverage MCP run
`9c398e10-9764-44b6-8709-f11cbcc46ffd` passed 85/85 tests in 67,406 ms and
ingested snapshot `01402870-19bd-4468-81ae-a96b31b1da2d`, reporting
51,482/51,963 lines, 7,104/7,198 branches, 2,898/2,968 functions, and
79,830/80,925 regions. The known LLVM JSON segment-normalization warning
remains. In that snapshot, `src/codecs/webp/native/encoder.rs` has
1,477/1,487 covered lines, 230/230 covered branches, and 77/77 covered
functions; ten source lines remain uncovered, and no coverage-only hook was
used. The parity run is Pillow-oracle evidence; the policy assertions and
aggregate coverage are implementation/Rust-only evidence.

Historical acceptance record: WebP VP8L 64-bit bitstream checkpoint

The finer lossless WebP/VP8L logical bitstream checkpoint is implemented at
`c0194045cb0a0b7f8d5a0b12c739a8ef46156624`. `TokenBitWriterCheckpoint` now
charges a logical poll after each 64 written bit while retaining the 128-bit,
256-bit, 512-bit, and 1,024-byte output intervals. The existing
`encode_work_budget_is_a_non_parity_result_contract` proves the 64-bit
boundary at `maximum: 56185`, `observed: 56186` in the whole-buffer path and
at `maximum: 56184`, `observed: 56185` in the direct-sink path; it retains the
128-bit boundary at 56186/56187 (return) and 56185/56186 (sink), the 256-bit
boundary at 56190/56191 (return) and 56189/56190 (sink), the 512-bit boundary
at 56191/56192 (return) and 56190/56191 (sink), and the 1,024-byte output
boundary at 56237/56238 (return) and 56236/56237 (sink). The direct sink
sentinel is `[0xAC]` for the new boundary and `[0xAB]`/`[0xAA]` remain for the
existing probes. This is Rust-only work-control evidence: no Pillow row,
parity fixture, diagnostic origin, new test function, or coverage-only hook
was added.

Managed Pillow parity run `f5dc4fdf-577d-4363-8497-a38935f8d1e9` passed
1,445/1,445 checks in 44,621 ms. The exact-head feature-matrix run
`5c76af1e-b77e-4b9b-b571-f021cd1976ca` passed in 52,947 ms; its retained log
records `cache=warm lanes=12 test_threads=3 build_jobs=1 debug=0 verbose=0`,
ends with `capability tables OK: every native and wasm32-wasip1 lane agrees`,
and has no `build-directory`, `package-cache`, or `lock-wait` matches.
Coverage MCP run `239239da-cd7e-4ea0-a227-c43cd9ca693f` passed 85/85 tests
in 81,591 ms and ingested snapshot `c603d5cc-6246-4e56-9716-5cc880232f0b`,
reporting 51,486/51,968 lines, 7,106/7,200 branches, 2,898/2,968
functions, and 79,837/80,934 regions. The known LLVM JSON
segment-normalization warning remains. In that snapshot,
`src/codecs/webp/native/encoder.rs` has 1,482/1,492 covered lines, 232/232
covered branches, and 77/77 covered functions; ten source lines remain
uncovered, and no coverage-only hook was used. The parity run is
Pillow-oracle evidence; the policy assertions and aggregate coverage are
implementation/Rust-only evidence.

Historical acceptance record: WebP VP8 64-bit checkpoints and test-runtime reduction

The lossy WebP/VP8 logical checkpoint slice is implemented at
`fa12b4054f6dcb4784e142bce39ccbe66144fd4e`. `TokenPartitionCheckpoint` and
`TokenCoefficientCheckpoint` now poll after each 64 coded bit while retaining
the 128-bit, 256-bit, 512-bit, 16,384-bit, and 1,024-byte boundaries. Their
larger logical intervals share one counter and are nested under the 64-bit
poll, avoiding four redundant modulo tests per coded bit in the token-aware
path. The existing
`encode_work_budget_is_a_non_parity_result_contract` proves partition 64 at
`maximum: 333`, `observed: 334` in both whole-buffer and direct-sink paths,
with `[0xC4]` preserved in the sink. It retains partition 128 at 336/337
(return) and 335/336 (sink), partition 256 at 340/341 and 339/340, partition
512 at 588/589 and 587/588, the 16,384-bit partition interval at 1,062/1,063
and 1,061/1,062, and the 1,024-byte output interval at 826/827 and 825/826.
The corresponding coefficient boundaries are 64-bit at 820/821 (return) and
819/820 (sink), 128-bit at 821/822 and 820/821, 256-bit at 827/828 and
826/827, 512-bit at 835/836 and 834/835, and 16,384-bit at 1,294/1,295 and
1,293/1,294. Sentinels `[0xC5]`, `[0xB5]`, `[0xB3]`, and `[0xB9]` retain the
untouched direct-sink prefixes for the new and existing coefficient probes.
This remains Rust-only work-control evidence: no Pillow row, parity fixture,
diagnostic origin, new test function, or coverage-only hook was added.

The same boundary observations remain unchanged after reducing the patterned
partition probe from 896x512 to the smallest tested 272x272 geometry. In a
clean local repeat of the exact WebP-only contract test, that change reduced
the observed test time from 0.90 s to 0.73 s. This is an execution measurement
for the local host, not a universal benchmark.

Managed Pillow parity run `5a6b0943-5ba2-4526-bdd5-6e0090d9197d` passed
1,445/1,445 checks in 44,608 ms. The exact-head feature-matrix run
`d5b9e780-dd1f-40f8-ae92-575a41b8d529` passed in 49,722 ms; its retained log
records `cache=warm lanes=12 test_threads=3 build_jobs=1 debug=0 verbose=0`,
ends with `capability tables OK: every native and wasm32-wasip1 lane agrees`,
and has no `build-directory`, `package-cache`, or `lock-wait` matches.
Coverage MCP run `ab3fd4b0-e4bc-43ea-93ab-00271fa965ed` passed 85/85 tests in
74,731 ms and ingested snapshot `6e26d1e4-58d7-4af6-b728-aa30a657b0f3`,
reporting 51,483/51,966 lines, 7,109/7,204 branches, 2,898/2,968 functions,
and 79,840/80,940 regions. The known LLVM JSON segment-normalization warning
remains. In that snapshot, `src/codecs/webp/encode/vp8/partition.rs` has
471/480 covered lines, 64/66 covered branches, 30/30 covered functions, and
694/751 covered regions; `src/codecs/webp/encode/vp8/residual.rs` has 353/362
covered lines, 44/44 covered branches, 21/21 covered functions, and 515/554
covered regions. The parity run is Pillow-oracle evidence; the policy
assertions, runtime measurement, and aggregate coverage are
implementation/Rust-only evidence.

Historical acceptance record: WebP 32-bit checkpoints and shared interval traversal

The next lossy/lossless WebP checkpoint slice is implemented at
`fc8f047567f4f053667e482c149b9cd881f0274b`. `TokenPartitionCheckpoint` and
`TokenCoefficientCheckpoint` now charge 32-bit logical polls, with the larger
64/128/256/512/16,384-bit intervals nested under that one counter; the VP8L
writer uses one 32-bit interval walk and nests its larger polls instead of
rescanning each logical range. The existing
`encode_work_budget_is_a_non_parity_result_contract` proves every new boundary
in whole-buffer and direct-sink paths: VP8 first-partition 32/64/128/256/512
return maxima are 339/340/341/349/350 with observed values one higher, and
sink maxima are 338/339/340/348/349 with observed values one higher; its
16,384-bit and 1,024-byte boundaries are 1,574/1,575 and 1,115/1,116 for
return, 1,573/1,574 and 1,114/1,115 for sink. VP8 coefficient 32/64/128/256/512
return boundaries are 821/822, 823/824, 828/829, 838/839, and 858/859;
the sink maxima are one lower with observations equal to the return maxima;
its output and 16,384-bit boundaries are 2,184/2,185 and 2,377/2,378 for
return, 2,183/2,184 and 2,376/2,377 for sink. VP8L 32/64/128/256/512 return
boundaries are 56,182/56,183, 56,184/56,185, 56,188/56,189, 56,196/56,197,
and 56,213/56,214; the sink maxima are one lower with observations equal to
the return maxima, and the 1,024-byte output boundary is 56,493/56,494 for
return and 56,492/56,493 for sink. The small common VP8 probe remains 272x272;
the late VP8 16,384-bit/output cases use a 64x64 patterned probe, and the
late coefficient output/bitstream cases reuse a 64x96 patterned probe. This
is Rust-only resource-contract evidence: Pillow has no caller token, work-budget
result, or caller-owned sink, so no parity row, fixture, diagnostic origin, or
coverage-only hook was added. The clean focused test completed in 0.88 s on the
local host; this is an execution observation, not a universal benchmark.

Managed Pillow parity run `e76fa7f0-18a6-4e16-b207-688fd04a3772` passed
1,445/1,445 checks with zero failures or skips in 41,873 ms at the same commit.
Feature-matrix run `2e822851-d17f-4afb-a5a2-b40e4e2bc8ec` passed all configured
native and WASI lanes in 31,020 ms; its retained log records
`cache=warm lanes=12 test_threads=3 build_jobs=1 debug=0 verbose=0`, ends with
`capability tables OK: every native and wasm32-wasip1 lane agrees`, and contains
no build-directory, package-cache, or lock-wait matches. Coverage MCP run
`d2393a9b-f610-4880-ac20-1806e18caf02` passed 85/85 tests in 48,767 ms and
ingested snapshot `1e17520c-f832-4eea-b41c-829d12a8f173`, reporting 51,500/51,977
lines, 7,117/7,210 branches, 2,898/2,968 functions, and 79,872/80,953 regions.
The known LLVM JSON segment-normalization warning remains. The changed VP8
partition file reports 481/485 lines, 68/68 branches, 30/30 functions, and
717/757 regions; residual reports 359/367 lines, 46/46 branches, 21/21
functions, and 520/560 regions; native VP8L reports 1,483/1,493 lines,
234/234 branches, 77/77 functions, and 2,157/2,256 regions. The parity run is
Pillow-oracle evidence; policy assertions and coverage are implementation/Rust
evidence, with no coverage-only hook.

Historical test-runtime acceptance record: compact late WebP work-budget probes

The work-budget contract retains the same `fc8f047567f4f053667e482c149b9cd881f0274b`
boundary observations after reducing only its late patterned probes: VP8
first-partition uses 64x64 for the 1,574/1,575 return and 1,573/1,574 sink
16,384-bit boundary and the 1,115/1,116 return and 1,114/1,115 sink
1,024-byte boundary; VP8 coefficient output and 16,384-bit checks reuse 64x96
for 2,184/2,185 and 2,377/2,378 return boundaries and 2,183/2,184 and
2,376/2,377 sink boundaries. The existing 272x272 probe and every boundary,
whole-buffer rejection, sink sentinel, and ample-budget identity assertion
remain in place. This is a test-harness-only runtime change: no production
codec, Pillow parity row, fixture, diagnostic origin, or coverage-only hook
changed. Three clean local repeats of the exact all-feature contract reported
0.80 s of test-body time (0.83–0.85 s process wall); the pre-change repeat in
the same workspace reported 0.94 s of test-body time. These are local execution
observations, not universal benchmarks.

A warm repeat of `scripts/test_feature_matrix.sh` also passed all configured
native, `wasm32-unknown-unknown`, and `wasm32-wasip1` lanes in 3.82 s; its
retained output ended with `capability tables OK: every native and
wasm32-wasip1 lane agrees`. The preceding run after invalidating the lane test
artifacts took 21.20 s. These are local harness observations, not universal
benchmarks.

Historical test-runtime acceptance record: reduced work-budget probe runtime

The test-only runtime slice is implemented at
`f3cf56ca2a562b9f6d6b068747efacf9a1e009f9`. The Rust-only
`encode_work_budget_is_a_non_parity_result_contract` retains every exact WebP
logical/output boundary, whole-buffer rejection, and untouched direct-sink
sentinel, while removing redundant ample-budget re-encodes for the late VP8L
and 512x512 analysis fixtures. Their byte-identity contract is already covered
by the smaller lossless and basic VP8 probes. The same test uses the first
1,024-pixel GIF work interval for its palette/normalization probes, a 32x32
LZW probe, and a tiny two-frame caller-built sequence for sequence admission and
cancellation. These are Rust-only work-control probes, not Pillow parity rows
or coverage-only inputs.

Two warm exact all-feature contract repeats passed in 0.52–0.53 s of test-body
time, compared with 0.59–0.60 s immediately before the change in the same
workspace. The full all-feature test run passed 82/82 tests, and a warm
feature-matrix repeat passed all configured native and WASI lanes in 7.93 s,
ending with `capability tables OK: every native and wasm32-wasip1 lane agrees`.
These are local execution observations, not universal benchmarks; no
production codec behavior, Pillow manifest row, fixture provenance, diagnostic
origin, or coverage-only hook changed.

Historical acceptance record: WebP 8-bit checkpoints and shared interval traversal

The 8-bit WebP logical-checkpoint slice is implemented at
`d437a038d1fee21a792762263c2a93e966c352ff`. Token-aware VP8 first-partition,
VP8 coefficient, and VP8L bit writing now poll the first 8-bit logical
interval and retain the larger nested interval walks. The existing
`encode_work_budget_is_a_non_parity_result_contract` proves exact whole-buffer
and direct-sink rejection at the first new interval: VP8 first-partition
return `102/103` and sink `101/102`, VP8 coefficient return `568/569` and sink
`567/568`, and VP8L return `145/146` and sink `144/145` (maximum/observed).
The retained 16/32/64/128/256/512-bit probes use recalibrated compact-fixture
edges, and every bounded sink retains its untouched sentinel prefix. The
focused all-feature contract passed with 0.53 s of test-body time; the full
all-feature suite passed 82/82 tests, including 45 feature-gate tests in 1.83
s. The registered feature matrix run `c261228e-b18d-42fd-a6c8-5c55b6493878`
passed all configured native, `wasm32-unknown-unknown`, and `wasm32-wasip1`
lanes in 37,451 ms; its retained log ends with `capability tables OK: every
native and wasm32-wasip1 lane agrees`. Managed Coverage MCP run
`f4463813-fb07-4ea3-9b2f-65e314e28b60` passed 85/85 tests in 64,386 ms and
ingested snapshot `86553dba-8838-4adf-afd7-611c2b443ce2`, reporting
51,467/52,007 lines, 7,101/7,222 branches, 2,897/2,968 functions, and
79,792/80,991 regions. The changed partition file reports 488/495 lines,
69/72 branches, 30/30 functions, and 719/769 regions; residual reports
368/377, 49/50, 21/21, and 530/572; native VP8L reports 1,492/1,503,
237/238, 77/77, and 2,163/2,270. The known LLVM JSON segment-normalization
warning remains. This is Rust-only work-control evidence: Pillow has no caller
token, work-budget result, or caller-owned sink, so this slice adds no parity
row, fixture, diagnostic origin, or coverage-only hook.

Historical acceptance record: WebP 1,024-bit checkpoints and shared interval traversal

The 1,024-bit WebP logical-checkpoint slice is implemented at
`a073c0ee9320a616de57b387da2649dd4f0fe7a6`. Token-aware VP8 first-partition,
VP8 coefficient, and VP8L bit writing now poll after each 1,024 logical bits
while retaining the larger nested interval walks. The existing
`encode_work_budget_is_a_non_parity_result_contract` proves exact whole-buffer
and direct-sink rejection: VP8 first-partition returns `271/272` and sink
`270/271`, VP8 coefficient returns `773/774` and sink `772/773`, and VP8L
returns `56,139/56,140` and sink `56,138/56,139` (maximum/observed). The
bounded sinks retain untouched sentinels `[0xB3]`, `[0xC0]`, and `[0xA9]`.
The focused contract passed in 0.61 s; the full all-feature test suite passed
82/82 tests and strict all-target Clippy passed. This is Rust-only
work-control evidence: Pillow has no caller token, work-budget result, or
caller-owned sink, so the slice adds no parity row, fixture, diagnostic origin,
new test function, or coverage-only hook.

The current managed Pillow parity run `1878eabc-a77b-4ef0-afba-69b65eb25924`
passed 1,445/1,445 checks in 48,062 ms. The feature-matrix run
`38929122-da1c-410f-bf20-d33b9c29a127` passed all configured native,
`wasm32-unknown-unknown`, and `wasm32-wasip1` lanes in 91,794 ms; its retained
log ends with `capability tables OK: every native and wasm32-wasip1 lane
agrees` and has no `lock-wait` match. Coverage MCP run
`4b01d7a9-abda-4f47-9cff-373376da2cfa` passed 85/85 tests in 244,572 ms and
ingested snapshot `57a4ea82-7122-4e45-8b78-2626fa033bf2`, reporting
51,488/52,030 lines, 7,107/7,228 branches, 2,897/2,968 functions, and
79,808/81,010 regions. The changed partition file reports 495/504 lines,
71/74 branches, 30/30 functions, and 724/775 regions; residual reports
377/386, 51/52, 21/21, and 534/578; native VP8L reports 1,497/1,508,
239/240, 77/77, and 2,170/2,277. The known LLVM JSON
segment-normalization warning remains. These are implementation/Rust coverage
metrics, not Pillow-oracle parity metrics.

Historical acceptance record: WebP 2,048-bit checkpoints and shared interval traversal

The 2,048-bit WebP logical-checkpoint slice is implemented at
`62e446bfc19d54dc99abecf2d5e0f8250a9bf072`. Token-aware VP8 first-partition,
VP8 coefficient, and VP8L bit writing now poll after each 2,048 logical bits
while retaining the smaller nested interval walks and the existing 16,384-bit
boolean boundary. The existing
`encode_work_budget_is_a_non_parity_result_contract` proves exact whole-buffer
and direct-sink rejection: VP8 first-partition returns `527/528` and sink
`526/527`, VP8 coefficient returns `1,124/1,125` and sink `1,123/1,124`, and
VP8L returns `56,505/56,506` and sink `56,504/56,505` (maximum/observed).
The bounded sinks retain untouched sentinels `[0xB2]`, `[0xBF]`, and `[0xA8]`.
The focused contract passed in 0.62 s; the full all-feature test suite passed
82/82 tests and strict all-target Clippy passed. This is Rust-only
work-control evidence: Pillow has no caller token, work-budget result, or
caller-owned sink, so the slice adds no parity row, fixture, diagnostic origin,
new test function, or coverage-only hook.

The current managed Pillow parity run `ceb42648-7a2c-4ce9-88ce-eb4c1440dadd`
passed 1,445/1,445 checks in 1,281 ms. The feature-matrix run
`6d851c7b-b1e9-4a02-ac70-f57100f462aa` passed all configured native,
`wasm32-unknown-unknown`, and `wasm32-wasip1` lanes in 51,535 ms; its retained
log ends with `capability tables OK: every native and wasm32-wasip1 lane
agrees` and has no `lock-wait` match. Coverage MCP run
`7a938bac-8dff-4ba9-96f4-dea15dda6ebe` passed 85/85 tests in 123,268 ms and
ingested snapshot `d3036cb7-1ea5-4fce-8ec2-abaf17950c32`, reporting
51,507/52,049 lines, 7,113/7,234 branches, 2,897/2,968 functions, and
79,826/81,029 regions. The changed partition file reports 502/511 lines,
73/76 branches, 30/30 functions, and 729/781 regions; residual reports
383/393, 53/54, 21/21, and 538/584; native VP8L reports 1,502/1,513,
241/242, 77/77, and 2,176/2,284. The known LLVM JSON
segment-normalization warning remains. These are implementation/Rust coverage
metrics, not Pillow-oracle parity metrics.

Historical acceptance record: WebP 4,096-bit checkpoints and shared interval traversal

The 4,096-bit WebP logical-checkpoint slice is implemented at
`5161bf3619ba0cfd1f969ec28528b1c4a7d618c1`. Token-aware VP8 first-partition,
VP8 coefficient, and VP8L bit writing now poll after each 4,096 logical bits
while retaining the smaller nested interval walks and existing 16,384-bit
boolean boundaries. The existing
`encode_work_budget_is_a_non_parity_result_contract` proves exact whole-buffer
and direct-sink rejection: VP8 first-partition returns `1,125/1,126` and sink
`1,124/1,125`, VP8 coefficient returns `1,593/1,594` and sink `1,592/1,593`,
and VP8L returns `57,019/57,020` and sink `57,018/57,019` (maximum/observed).
The bounded sinks retain untouched sentinels `[0xB1]`, `[0xBE]`, and `[0xA7]`.
The focused contract passed in 0.70 s; the full all-feature test suite passed
82/82 tests and strict all-target Clippy passed. This is Rust-only work-control
evidence: Pillow has no caller token, work-budget result, or caller-owned sink,
so the slice adds no parity row, fixture, diagnostic origin, new test function,
or coverage-only hook.

The current managed Pillow parity run `40e2e724-9c2c-4195-9d2f-12df00913e79`
passed 1,445/1,445 checks in 803 ms. The accepted feature-matrix retry
`967574e7-58e9-4c9f-a174-826f89b4b966` passed all configured native,
`wasm32-unknown-unknown`, and `wasm32-wasip1` lanes in 3,553 ms and ended with
`capability tables OK: every native and wasm32-wasip1 lane agrees`; its retained
logs contain no `lock-wait` match. The first concurrent matrix attempt
`c1e765f6-4636-41a0-9771-961f004f7731` had one native-AVIF sink byte-identity
failure; a targeted optimized native-AVIF lane passed before the retry.
Coverage MCP run `b7733847-937d-483d-b96b-0b7f79c2859e` passed 85/85 tests in
61,503 ms and ingested snapshot `33f78a7a-0258-4224-b399-53842d46d0e4`,
reporting 51,525/52,068 lines, 7,119/7,240 branches, 2,897/2,968 functions,
and 79,847/81,048 regions. Compared with prior accepted snapshot
`d3036cb7-1ea5-4fce-8ec2-abaf17950c32`, covered totals increased by 18 lines,
6 branches, 0 functions, and 21 regions; source totals grew by 19 lines,
6 branches, 0 functions, and 19 regions. The changed partition file reports
509/518 lines, 75/78 branches, 30/30 functions, and 736/787 regions; residual
reports 390/400, 55/56, 21/21, and 544/590; native VP8L reports 1,507/1,518,
243/244, 77/77, and 2,186/2,291. The known LLVM JSON segment-normalization
warning remains. These are implementation/Rust coverage metrics, not
Pillow-oracle parity metrics.

Historical acceptance record: API-003 signature-validated explicit-format still decode

API-003 is implemented at
`b0ab0edc823b2065c182f7cd53cd4bbf37a79d8d`. The new
`decode_with_format` and `decode_with_format_and_policy` entry points accept a
caller-selected `ImageFormat` only after the complete signature agrees; input
limits still run first, and matching inputs continue through normal feature,
inspection, policy, payload-validation, and diagnostic paths. A recognized
different signature returns staged `Parameter`, an incomplete or unknown
complete-slice signature returns staged `Malformed`, and partial input remains
the `decode_prefix` API.

The existing fixture-selected feature-gate contract proves explicit-format
success matches auto-detecting decode for every enabled format, preserves
disabled-feature and portable-WASM AVIF outcomes, rejects a mismatched format,
and rejects a one-byte-too-small encoded-input policy before format dispatch.
The focused contract and full all-feature suite passed, with 82/82 tests and
strict all-target Clippy clean. Managed Pillow parity run
`3588a194-19ec-415e-8f02-a4074e5213cc` passed 1,445/1,445 checks in 780 ms;
feature-matrix run `e3266f6e-7218-481a-af9e-48a13e130107` passed all configured
native and WASM lanes in 30,127 ms and ended with
`capability tables OK: every native and wasm32-wasip1 lane agrees`, with no
targeted `lock-wait` match. Coverage MCP run
`7ac2da92-8cbd-486c-a976-0a85bf248a37` passed 85/85 tests in 58,645 ms and
ingested snapshot `6782355b-73ab-433e-803b-4103212f03f8`: 51,577/52,119
lines, 7,129/7,248 branches, 2,901/2,972 functions, and 79,916/81,113
regions. The strict local verifier still reports the known aggregate shortfall
of 542 lines, 119 branches, 71 functions, and 1,197 regions. This is a
Rust-only API contract: Pillow has no caller-supplied format-hint operation,
so the implementation adds no parity row, fixture, diagnostic origin, new
test function, or coverage-only hook.

Historical acceptance record: API-006 checked `DecodedImage` construction

API-006 is implemented at
`d5a50cd7cc8096aadfed5000622ca8159c3ef09d`. `DecodedImage::try_new` and
`try_with_mode` validate dimensions, exact pixel length, and color/mode
agreement while reusing the caller's pixel vector; `try_with_palette` validates
indexed palette state after attaching a palette. The existing `new`,
`with_mode`, and `with_palette` builders and direct field literals remain
explicitly unchecked for compatibility and staged assembly, while every
consumer remains validation-gated.

The existing fixture-selected feature-gate contract proves valid RGB and
indexed construction, palette validation, pointer identity, dimensions, and
color/mode error classification. The focused contract and full all-feature
suite passed, with 82/82 tests and strict all-target Clippy clean. Managed
Pillow parity run `8d3af3c2-19aa-4ed0-947b-f919b9dd0120` passed 1,445/1,445
checks; feature-matrix run `ba8aa5cf-957d-42f6-8b6b-f8182f609ab6` passed all
configured native and WASM lanes with no targeted `lock-wait` match; coverage
run `b2a23601-7ee7-4ad5-9551-30c27542920e` passed 85/85 tests and ingested
snapshot `439f2d27-2bda-4986-8205-ce6598946e8d`. This is Rust-only
defensive-model evidence: no Pillow caller API changes, parity row or fixture,
diagnostic origin, new test function, or coverage-only hook.

Historical acceptance record: API-012/API-013 lazy sequence cache and decode state

The cache implementation is in
`affb3df61e26df56bb6873fa916e5565292261f2`; the final contract/evidence
revision is `0fe6ea6e2dab8da0dede699ccbc595feb2d93c52`. `EncodedImage` now retains an
independent lazy `DecodedSequence` cache, while `decode_sequence_with_policy`
uses the policy-aware path for limited policies so a resource-limit failure
cannot poison the unlimited compatibility cache. `EncodedImageDecodeState` plus
the still/sequence state accessors distinguish `NotAttempted`, `Succeeded`, and
`Failed`; the existing success-only predicates remain available. The source
contract proves complete animated sequence ordering, clone-visible cache state,
separate still/sequence caches, cached deterministic failures, and policy-failure
isolation without collapsing to the first frame.

The focused source contract and full all-feature suite passed, with 82/82 tests
and strict all-target Clippy clean. Managed Pillow parity run
`5ade141b-71e1-4075-b6d0-2807e6ba56ed` passed 1,445/1,445 checks; feature-matrix
run `b8891003-847f-4118-b7a4-a40b3bfd068c` passed all configured native and WASM
lanes; coverage run `101634e8-2b9d-4446-8a20-b2e0f328b0fe` passed 85/85 tests
in 75,468 ms and ingested snapshot
`061cc413-e997-4cc9-9ce7-9c9fafe9d227`. The revision-bound testing section
records the exact coverage deltas. This is Rust-only source/cache evidence: no Pillow caller API
changes, parity row or fixture, diagnostic origin, new test function, or
coverage-only hook.

Historical acceptance record: API-045 source-bound selected-format dispatch

The runtime slice is implemented at
`50375369951ba73c165e87481fa70e068fbfcc07`. Owned and borrowed source-bound
still and sequence decode now route through the `ImageFormat` retained by
construction after repeating the caller policy checks, so a source decode does
not run signature detection a second time. The root auto-detecting APIs are
unchanged. Codec-specific materialization still parses its container as needed,
and `verify()` still reparses independently; immutable parsed-header/index
retention remains open under API-045 until every codec proves that reuse cannot
weaken later validation.

The focused source contract and full all-feature suite passed, with 82/82 tests
and strict all-target Clippy clean. Managed Pillow parity run
`01b3af4b-b1b6-41b3-8261-1e665d992417` passed 1,445/1,445 checks in 1,039 ms;
feature-matrix run `62e1d190-af03-426c-b320-2612fea93f2a` passed all configured
native and WASM lanes in 19,406 ms with the capability-agreement marker and no
targeted `lock-wait` match; coverage run
`9ebd95dd-c64b-4907-ad3b-06903fe4783f` passed 85/85 tests in 59,553 ms and
ingested snapshot `b7b9d763-98f4-4bb3-b200-7cedd75b02eb`. The revision-bound
testing section records the exact aggregate and changed-file metrics. This is
Rust-only runtime evidence: no Pillow caller API changes, parity row or
fixture, diagnostic origin, new test function, or coverage-only hook.

Historical acceptance record: compact incremental-decode fixture sweep

The test-only runtime slice is implemented at
`a819abb48cd6878ec4ae6c4a41e42a038b81a105`. The existing
`incremental_decode_tracks_truncation_progress_per_format` feature-gate
contract still sweeps every byte boundary and compares `decode_prefix` with
legacy `decode`; it now selects the valid 343-byte
`miniswhite_8bit.tiff` and 294-byte `portable_probe_gray_128.avif` fixtures
instead of the 16,506-byte TIFF and 3,077-byte AVIF fixtures. The boundary
assertions remain fixture-driven and complete for every enabled format, while
the accidental repeated full-raster decode cost is removed. The all-feature
feature-gate body fell from 3.23 s to 0.79 s in the same local workspace. The
warm managed feature-matrix run `d96c8554-834d-4003-bd5c-d72aa0bc87be` passed
all configured native, `wasm32-unknown-unknown`, and `wasm32-wasip1` lanes in
3,231 ms, compared with 6,065 ms at the preceding test revision; its retained
log ended with `capability tables OK: every native and wasm32-wasip1 lane
agrees`, with no targeted lock-wait/build-directory/package-cache match. The
committed Pillow parity run `8f76ea58-4044-4db3-b73c-953606341dda` passed
1,445/1,445 checks in 749 ms. Coverage run
`42cd2f3e-1b41-4658-8c12-e92aac835f50` passed 85/85 tests in 48,216 ms and
ingested snapshot `8861f2ef-8624-461c-80df-4237997e94a1`; aggregate coverage
is unchanged from the preceding accepted snapshot, including the known LLVM
segment-normalization warning and the 542-line, 119-branch, 71-function, and
1,200-region strict-verifier shortfall. This is test-harness-only Rust
evidence: no Pillow parity row, parity-manifest fixture, diagnostic origin,
new test function, or coverage-only hook changed.

Historical test-runtime acceptance record: bounded, cache-aware feature-matrix fanout

The current harness and compact VP8 boundary probes are included in implementation
revision `5f058fecdf63c69a80f4f177f542860264d8cba3`; the feature matrix retains
24 concurrent lanes with the bounded warm worker setting.
In both cache states,
`MATRIX_TEST_THREADS` now defaults to
`floor(logical_cpus / MATRIX_JOBS)`, bounded to at least one and at most eight;
the measured 12-CPU warm host therefore uses one worker for its 24 concurrent
lanes instead of multiplying to 72 workers. `MATRIX_TEST_THREADS` remains an
explicit override. All 991 feature-matrix checks and every native,
`wasm32-unknown-unknown`, and `wasm32-wasip1` lane remain in scope.

Managed feature-matrix run `5f1bab78-086b-44b1-a489-1cc9eece23e4` passed all
991/991 checks across 24/24 configured lanes in 31,864 ms, recorded
`cache=warm lanes=24 test_threads=1 build_jobs=1 debug=0 verbose=0`, ended with
`capability tables OK: every native and wasm32-wasip1 lane agrees`, and had no
targeted lock-wait/build-directory/package-cache matches. These are cache- and
runner-sensitive execution observations, not a universal benchmark and not the
revision-bound allocation/peak-memory evidence still required by QA-010 and
QA-030. The 131,072-bit contract uses a 768×768 RGB probe, the 262,144-bit
contract uses a 1,024×1,024 high-entropy RGB probe, and both coefficient-only
524,288-bit and 1,048,576-bit contracts reuse an 832×832 deterministic RGB
checkerboard probe; the focused combined contract completed in 3.12 s in this
workspace after removing the separate 1,920×1,920 witness. These are targeted
boundary witnesses, not general benchmarks or claims of universal codec speedup;
managed durations remain cache- and runner-sensitive observations.

Current acceptance record: WebP VP8L work-control checkpoints cover entropy
analysis, histogram clustering/population/merge/cost, backward-reference
intervals, repeated-run hash chains, copy-token cache/replay, Huffman
preparation and emission, grayscale preparation, candidate-trial prefix reuse,
lossy alpha-palette collection/packing/scanning, lossless VP8L source-pixel
materialization, image-palette construction, palette ordering/lookup/packing,
hidden-RGB cleanup, long backward-reference result backfills, and the bounded
feature-matrix runtime. The accepted
implementation revision is
`56869ad0a61565012cc039bd6c94f01afb34f098`.

The new token-aware lossless VP8L RGB/RGBA source-materialization branch polls
after each 1,024 source pixels, while the no-token maps remain the original
tight iterators. The existing feature-gated contract proves the source boundary
at `maximum: 2`, `observed: 3`, with sink sentinel `[0xC4]` untouched; later
lossless VP8L boundaries are image-palette construction `6/7` (`[0xBA]`),
hidden-RGB cleanup `18/19` (`[0xB7]`), palette lookup `9,820/9,821`
(`[0xA9]`), and palette-mode packing `5,205/5,206`; the token-aware
cost-manager table initialization leaves only `[0xC3]` before that bounded
sink rejection. The downstream exact work-budget witnesses were recalibrated
for the four conversion intervals, including entropy `23/24`, histogram `62/63`,
combined cost `80/81`, merge `8,258/8,259`, cost estimate `14,092/14,093`,
Huffman-RLE `828/829` (sink `827/828`), grayscale `195/196`, Huffman frequency
`44,001/44,002` (sink `44,000/44,001`), emission `144,869/144,870`, and cache
`136,928/136,929`.

The predictor mode-application path now checkpoints its pre-transform source
snapshot copy after each 1,024 pixels, while the no-token path retains the
original bulk clone. The existing predictor-transform probe exercises that
caller-budget path before its later transform boundary; this is Rust-only
evidence because Pillow has no equivalent token or typed work-budget result.

The token-aware VP8L backward-reference cost manager also initializes its
pixel-sized cost/length tables after each 1,024 entries; capacity reservations
retain the existing no-recoverable-OOM policy. The existing 1,024-pixel
palette-mode sink probe now rejects at `maximum: 5,205`, `observed: 5,206`
before structural delivery and leaves `[0xC3]` untouched. This is Rust-only
resource-contract evidence, not a Pillow parity result.

The token-aware backward-reference result backfill also polls every 256
backfilled entries, so a constant 1×512 RGB probe rejects at
`maximum: 2,516`, `observed: 2,517`; the existing sink path retains the
validated `RIFF`/`WEBP` prefix after its later checkpoint. This is Rust-only
work-control evidence, not a Pillow-observable result.

This is Rust-only resource-contract evidence: Pillow has no caller token, typed
work-budget result, or caller-owned sink/rollback contract, so the revision adds
no parity row, fixture-manifest row, diagnostic origin, new test function, or
coverage-only hook. Pillow parity run
`2109991a-7aae-4653-9835-2823b09cbcfd` passed 1,445/1,445 checks with zero
skips in 1,558 ms. Feature-matrix run
`68863d9e-b8b6-4fc8-b2d2-c15a195456c2` passed all 33 configured lanes in
47,487 ms, retained `cache=warm lanes=12 test_threads=1 build_jobs=1 debug=0
verbose=0`, ended with capability-table agreement, and had no targeted
lock-wait/build-directory/package-cache matches. No AVIF implementation changed
in this revision.

Managed LLVM coverage run `ff9a1499-3e52-4619-b026-1c3b4ee8b9fb` passed 85/85
tests in 66,175 ms and ingested snapshot `295965ae-83c5-4fe2-a09b-396be34d020e`:
53,345/53,961 lines, 7,567/7,718 branches, 3,001/3,077 functions, and
82,549/83,927 regions. Compared with preceding accepted snapshot
`83634c29-ba52-4054-a695-7417262366ff`, covered/source totals changed by
`+7/+6` lines, `+4/+4` branches, `+0/+0` functions, and `+10/+11` regions.
The changed `src/codecs/webp/native/encoder/backward_refs.rs` reports
1,601/1,619 lines, 434/446 branches, 68/68 functions, and 2,444/2,549
regions; the native WebP encoder remains 1,956/2,006 lines, 423/438 branches,
91/91 functions, and 2,871/3,062 regions. The known LLVM segment-normalization
warning remains; the strict aggregate shortfall is 616 lines, 151 branches, 76
functions, and 1,378 regions. These are implementation coverage metrics, not
Pillow parity metrics; the new backfill checkpoint is covered by existing
feature-gate execution, and no coverage-only test was added. Managed durations
remain cache- and runner-sensitive.

## Historical acceptance record: superseded WebP work-control revisions

The token-aware VP8L entropy-mode analysis now charges cooperative checkpoints
after each 64 symbols while scanning its fixed-alphabet histogram costs. The
histogram analysis path likewise charges after each 64 symbols while scanning
histogram populations,
combined entropy costs, and histogram merges. The backward-reference length-cost
table and equal-cost interval setup now charge after each 1,024 entries, and
token-aware cost-manager interval-update and cleanup scans charge after each
256 cumulative interval entries. Token-aware repeated-run hash-chain insertion
charges after each 256 pixels. The
token-aware histogram-clustering min/max and bin-assignment pre-passes now charge
after each 64 tile histograms; the ordinary no-token path retains the existing
algorithm and data. The
token-aware non-saturated interval split/merge path now charges after each
1,024 interval-work entries; the saturated cost-interval fallback and long
length-interval enumeration also charge after each 1,024 entries, while the
ordinary no-token path retains its original tight loops. The VP8L candidate-trial
writer now copies the already-emitted prefix once and retains only each trial
suffix, removing repeated prefix copy/allocation without changing selected
bytes or adding a new public work-budget result. The lossy WebP RGBA
alpha-palette source-collection path now charges a token-aware checkpoint after
each 1,024 source pixels; the no-token path retains its bulk BTreeSet
collection and byte output. The deterministic 16×64 RGBA fixture in the same
existing feature-gate contract proves the boundary at `maximum: 5`, `observed: 6`
in both whole-buffer and direct-sink paths, with the sink sentinel `[0xC1]`
untouched. This is Rust-only work-budget evidence:
Pillow has no caller token, work-budget result, or caller-owned sink, so it adds
no parity row, fixture-manifest row, diagnostic origin, new test function, or
coverage-only hook. The implementation is committed at
`c4a27c560ee6509f2c47c3e78d158ca8866cc7c2`. The same existing feature-gate
contract now also polls lossy WebP RGBA alpha-palette index packing after each
1,024 source pixels. A deterministic 128×8 RGBA fixture cycling monotone alpha
values 0–63 proves ample-budget byte identity, then exact whole-buffer and
caller-owned-sink rejection at `maximum: 11`, `observed: 12`, with sentinel
`[0xC2]` untouched. Pillow has no caller token, typed work-budget result, or
caller-owned sink, so this is Rust-only evidence with no parity row,
fixture-manifest row, diagnostic origin, new test function, or coverage-only
hook. The implementation is committed at
`925ff4d4afa0ebba4cd4a918929a430f273eaa3b`. The lossless VP8L image-palette
construction path now charges a token-aware checkpoint after each 1,024 source
pixels while collecting the source-color set; the no-token path retains its
bulk collection and byte output. The deterministic 64×64 RGB lossless WebP
fixture proves ample-budget byte identity, then exact whole-buffer and
caller-owned-sink rejection at `maximum: 6`, `observed: 7`, with sentinel
`[0xBA]` untouched after accounting for four earlier conversion intervals.
Pillow has no caller token, work-budget result, or sink-rollback contract, so
this remains Rust-only evidence with no parity row, fixture-manifest row,
diagnostic origin, new test function, or coverage-only hook. The implementation
is committed at `53886bfdc7ea4eee996f5a892e1742a8acd91a9b`. The current
source-materialization checkpoint is committed at
`7ab2a043b1e07106370416500bc13ae6af52cefd`: it charges token-aware RGB/RGBA
materialization after each 1,024 source pixels, leaves the no-token maps tight,
and rejects the same 64×64 RGB fixture at `maximum: 2`, `observed: 3`, with
sink sentinel `[0xC4]` untouched. Pillow cannot exercise this caller-budget
contract, so it adds no parity row, fixture-manifest row, diagnostic origin,
new test function, or coverage-only hook. Candidate
scoring and fixed-alphabet Huffman cost paths now charge after each 1,024 tokens
and each 64-symbol population scan. Huffman RLE preparation, canonical-code
assignment, and compressed Huffman-token generation now charge after each 64
code-length symbols. Huffman-tree simple-tree symbol discovery now charges after
each 64 code-length slots; code-length-token frequency accumulation and the
reverse trailing zero-repeat-token trim scan now charge after each 16 compressed
token entries. Huffman code-length emission now charges after each 16 compressed
token entries; its existing feature-gated Rust-only work-control assertion
drives the Huffman-tree path, whose canonical-code assignment and sorted-node insertion
scans charge after each 64 code-length slots or candidate nodes. The preceding
palette-index lookup slice is committed at
`dd1f8be02234d89d49f79c23aacf569768ad1b8e`; the current lossless-RGBA-cleanup
production revision is committed at
`464126042af49a945a63a505cb1675ebe703a904`; the earlier production slice is
committed at revision
`84a9abbd8fca78fc468e3e46be8baa5ca37e005f5`; an earlier production slice is
committed at revision
`b1fafe4bacd60628b2385e14a843bb6bf827c1e2`; the current contract and backward
cost-manager setup are committed at `0675baea3b97104d68636e8fe363ed61ba625c01`
(following `063f00e145aff455c30656b3559c8881b8e51a6f`); the saturated
cost-interval implementation is committed at
`b153381bd9657b1f9da3707ca1d6f015ab174042`; the non-saturated interval
split/merge implementation is committed at
`2dd22a3f8f535563ae5db4f80c55829ddcf2c94f`; the repeated-run hash-chain
checkpoint slice is committed at
`74b1b03b956c0a0074ee4c2ddc2b9c06770b8984`; the current cost-manager
interval-update/cleanup checkpoint slice is committed at
`52623efa026c775b2d1c5157e10cf485e5fca789`; the candidate-trial prefix-reuse
optimization is committed at `3e139ae7fc5bc1bfaeb3440c4112394cb33eeff3`; the
entropy-analysis checkpoint slice is committed at
`1a8cae394ad0265e4f0a3bf84511b80e7e2a7842`; the entropy-bin clustering
pre-pass checkpoint slice is committed at
`4eae86493bad9016611648a498a81a79f90f5551`; the alpha-palette candidate scan
slice is committed at `1b87a06bf0b8c866bd843df3ecb8c63e447f475c`; token-aware copy-token cache
population and traced copy-token replay also charge after each 256 pixels
inside a copy token, while the
ordinary no-token paths retain their original tight loops. The existing
lossy WebP RGBA alpha-palette ordering now charges after each 64 nearest-delta
candidate values in its token-aware path; the no-token path retains the
original first-minimum ordering and bytes. The lossless VP8L palette ordering
path keeps the no-token helper byte-preserving and charges token-aware sign
collection and nearest-delta suffix scans after each 64 palette entries or
candidate values. Palette-index packing keeps its no-token linear lookup
byte-preserving and charges token-aware lookup after each 64 palette candidates.
The deterministic 128-entry 128×4 RGB fixture proves exact
whole-buffer and caller-owned-sink rejection at `maximum: 3,000`,
`observed: 3,001`, with sentinel `[0xA7]`; monotone, mixed short, and
transparent-zero public palette fixtures cover the early-return and rotation
branches. Pillow has no caller token, work-budget result, or sink contract, so
this remains Rust-only evidence with no parity row, fixture-manifest row,
diagnostic origin, new test function, or coverage-only hook. This closes the
next causal interior checkpoint in the current WebP work-control slice. The
deterministic 128×128 RGB lookup probe proves exact whole-buffer and
caller-owned-sink rejection at `maximum: 9,820`, `observed: 9,821`, with
sentinel `[0xA9]` untouched. Pillow has no caller token, work-budget result, or
caller-owned sink, so this lookup checkpoint is Rust-only evidence with no
parity row, fixture-manifest row, diagnostic origin, new test function, or
coverage-only hook. The same existing contract now uses a deterministic 128×8
RGB fixture built from the existing 128-entry palette to reach lossless VP8L
palette-mode index packing. It proves ample-budget byte identity, then exact
whole-buffer and caller-owned-sink rejection at `maximum: 5,205`, `observed:
5,206`; the sink preserves the delivered prefix `[0xC3, 0x52, 0x49, 0x46,
0x46, 0xEA, 0x03, 0x00, 0x00, 0x57, 0x45, 0x42, 0x50]`. The token-aware
packing path polls after each 1,024 source pixels, while the no-token linear
packing loop remains byte preserving. Pillow has no caller token, typed
work-budget result, or caller-owned sink, so this is Rust-only evidence with no
parity row, fixture-manifest row, diagnostic origin, new test function, or
coverage-only hook. The implementation is committed at
`589186a6e3f0a1f8fd47ca84dcc73133620ed9fa`.
`encode_work_budget_is_a_non_parity_result_contract` uses deterministic RGB
probes and proves exact whole-buffer and caller-owned-sink rejection at the
entropy-analysis boundary `maximum: 23`, `observed: 24` with sentinel `[0xAD]`,
the histogram-population boundary `maximum: 62`, `observed: 63` with `[0xB8]`,
the combined entropy-cost boundary `maximum: 80`, `observed: 81` with `[0xAE]`,
the histogram-merge boundary `maximum: 8,258`, `observed: 8,259` with `[0xAF]`
untouched, and the cost-estimate boundary `maximum: 14,092`, `observed: 14,093`
with `[0xB0]` untouched, plus exact Huffman-RLE preparation boundaries at
`maximum: 828`, `observed: 829` for the whole-buffer return path and
`maximum: 827`, `observed: 828` with `[0xB1]` untouched for the caller-owned
sink. The same
existing contract now uses a deterministic 128×128 RGBA grayscale probe to
prove the preparation checkpoint boundary at `maximum: 195`, `observed: 196`
in both whole-buffer and caller-owned-sink paths, with `[0xB2]` untouched.
The same contract proves the histogram-clustering min/max and bin-assignment
pre-pass boundary at `maximum: 5,325`, `observed: 5,326` with `[0xB9]`
untouched. The Huffman-tree frequency boundary remains
`maximum: 44,001`, `observed: 44,002` for the whole-buffer return path and
`maximum: 44,000`, `observed: 44,001` with `[0xB3]` untouched for the
caller-owned sink. The batched code-length-emission contract first proves
normal/ample-budget fixture-byte identity, then rejects both whole-buffer and
caller-owned-sink paths at `maximum: 144,869`, `observed: 144,870`; the new
pre-output palette checkpoint leaves sentinel `[0xB6]` untouched. The cache
probe now rejects both paths at `maximum: 136,928`, `observed: 136,929`; the
new pre-output palette checkpoint leaves sentinel `[0xB5]` untouched. The old
late cost-manager and trailing-trim maxima were tied to the
previous per-token polling schedule and are superseded by the batched emission
poll; the implementation checkpoints remain current behavior, but those stale
exact thresholds are not claimed. Pillow has no caller token, work-budget
result, or sink contract, so this change adds no parity row, fixture-manifest
row, diagnostic origin, new test function, or coverage-only hook.
The same existing contract uses a deterministic 128×128 RGBA lossy WebP probe
with 128 alpha palette values (0–63 and 192–255) to reach the nearest-delta
alpha-palette ordering scan. It proves ordinary and ample-budget byte identity,
then exact whole-buffer and caller-owned-sink rejection at
`maximum: 40`, `observed: 41`; the sentinel `[0xA8]` remains untouched. The
token-aware scan checks after each 64 candidates, while the no-token path keeps
the original first-minimum ordering and byte output. Pillow has no caller work
budget or sink contract, so this is Rust-only evidence with no parity row,
fixture, diagnostic origin, new test function, or coverage-only hook.
The same existing contract now uses a deterministic 128×128 fully transparent
RGBA lossless WebP probe with nonzero hidden RGB values to prove ordinary and
ample-budget byte identity, then exact whole-buffer and caller-owned-sink
rejection at `maximum: 18`, `observed: 19`, with sentinel `[0xB7]` untouched.
The token-aware VP8L cleanup polls after each 1,024 scanned pixels; the
ordinary no-token path retains its bulk loop. Pillow has no caller token,
work-budget result, or sink-rollback contract, so this remains Rust-only
evidence with no parity row, fixture-manifest row, diagnostic origin, new test
function, or coverage-only hook. The implementation is committed at
`464126042af49a945a63a505cb1675ebe703a904`.
This is Rust-only work-control evidence: Pillow has no caller token,
work-budget result, or caller-owned sink, so no parity row, fixture, diagnostic
origin, new test function, or coverage-only hook was added. The existing
feature-gated assertion reaches the earlier setup rejection before the finer
non-saturated interval split/merge checkpoint, so no exact observed boundary is
claimed for that path.

The test harness follow-up that removes duplicate unknown-target integration
linting is committed at
`7303e0d4eeded0f25c98a66fa61155692c4bc744`; the current bounded warm-worker
default is committed at
`5af768432579730f01e6af0bf595ac4f02a371df`. Unknown-target compile-only lanes
now lint the library surface instead of rebuilding integration targets already
compiled by every native and WASI feature lane; all 33 lanes, the two
unknown-target no-run checks, 45 feature-gate assertions per native/WASI lane,
and capability-table agreement remain in scope. Managed Pillow parity run
`229fbfe2-b763-4dcb-a5b1-76b5890040c0` passed 1,445/1,445 checks with zero
skips in 7,064 ms; feature-matrix run
`ad5a4685-5af0-4949-be19-cc254934c83e` passed all configured lanes in 65,228
ms and its retained log ended with the terminal capability agreement;
targeted searches returned no lock-wait, build-directory, or package-cache
matches. Coverage MCP run `d47dff4a-7ff6-4add-9277-e0d8f2b14f52` passed 85/85
tests in 85,710 ms and ingested snapshot
`ae49146a-9507-45ba-ba47-1cd2278fcac9`: 53,286/53,902 lines, 7,551/7,702
branches, 3,000/3,076 functions, and 82,451/83,828 regions. Compared with
the preceding accepted snapshot `4c7d6c97-70f4-4907-b57b-06456f69423f`,
covered/source totals changed by +8/+7 lines, +2/+2 branches, +0/+0
functions, and +11/+11 regions. Native WebP encoder reports 1,919/1,969
lines, 417/432 branches, 90/90 functions, and 2,820/3,011 regions. Coverage
is implementation evidence, not Pillow parity; the known LLVM
segment-normalization warning and the 616-line, 151-branch, 76-function,
1,377-region aggregate shortfall remain. Managed durations remain cache- and
runner-sensitive. The lossless VP8L image-palette-construction, palette-mode
index-packing, and lossy WebP alpha-palette source-collection and
index-packing checkpoints are Rust-only evidence and add no
parity row, fixture-manifest row, diagnostic origin, new test function, or
coverage-only hook.

Historical acceptance record: warm feature-matrix fanout bound

Warm automatic feature-matrix mode now selects one lane per logical CPU, capped
at 24, instead of two cached lanes per logical CPU. The scheduler change is
committed at revision `f015165d345cb35234ac5349de7de4a21d001638`; explicit
`MATRIX_JOBS`, `MATRIX_TEST_THREADS`, and `MATRIX_BUILD_JOBS` overrides remain
unchanged. On this 12-CPU workspace, the default warm run selected
`lanes=12 test_threads=1 build_jobs=1` and completed in about 7.3 seconds,
compared with about 24.1 seconds at the previous 24-lane default. This is a
cache- and runner-sensitive execution observation, not a universal benchmark;
all native, `wasm32-unknown-unknown`, and `wasm32-wasip1` lanes remain in scope.

Managed feature-matrix run `9fca3370-5cb6-451b-9539-ef114a376a53` passed all
configured lanes in 9,006 ms. Its retained log records
`cache=warm lanes=12 test_threads=1 build_jobs=1 debug=0 verbose=0`, ends with
`capability tables OK: every native and wasm32-wasip1 lane agrees`, and has no
targeted lock-wait/build-directory/package-cache matches. Managed Pillow parity
run `7e9f9f8f-5f9f-4ba8-8e38-cce59a01270c` passed 1,445/1,445 checks in 794 ms.
Coverage MCP run `53722a97-62e5-456e-8e77-c337af8451ff` passed 85/85 tests in
54,770 ms and ingested snapshot `96ca2123-2aa7-4524-a6a0-7f9c99b1a773`.
Coverage totals are unchanged at 52,265/52,824 lines, 7,239/7,364 branches,
2,964/3,040 functions, and 80,857/82,084 regions; the known LLVM
segment-normalization warning and 559-line, 125-branch, 76-function,
1,227-region shortfall remain. This harness-only slice adds no parity row,
fixture, diagnostic origin, new test function, or coverage-only hook.

Historical acceptance record: lossless WebP VP8L cross-color sampling interval

The lossless VP8L cross-color sampling reduction now charges a cooperative
work-budget checkpoint after each 1,024 scanned or compacted tile-map samples.
The implementation/test slice is committed at revision
`4b47dc3e980a703902b39703ce683528087951bd`. The existing feature-gated
`encode_work_budget_is_a_non_parity_result_contract` uses a deterministic
8,192x8 RGB probe with a 1,024-entry tile map and proves the inclusive
whole-buffer and caller-owned-sink rejection pair `maximum: 129,499`,
`observed: 129,500`, with sentinel `[0xAC]` untouched. The ordinary no-token
path retains the original scan/copy loops. Pillow has no caller token or
work-budget result, so this is Rust-only resource-contract evidence and adds
no parity row, fixture, diagnostic origin, new test function, or coverage-only
hook.

Managed Pillow parity run `946e0082-8769-4783-8f71-9e033321b48f` passed
1,445/1,445 checks; feature-matrix run
`f5b20dea-e081-43a1-aa3d-8c444129486c` passed all 24/24 configured lanes in
38,941 ms and retained the capability-table agreement with no targeted
lock-wait/build-directory/package-cache matches; and Coverage MCP run
`68a9a58d-0456-4c00-9b16-ba7e0e20fdc4` passed 85/85 tests and ingested snapshot
`daf021be-d1c3-4954-90c3-94a57d3ec7d7`. The snapshot reports 52,265/52,824
lines, 7,239/7,364 branches, 2,964/3,040 functions, and 80,857/82,084
regions. The known LLVM JSON segment-normalization warning remains; the
aggregate shortfall is 559 lines, 125 branches, 76 functions, and 1,227
regions. These are implementation, target, and Rust-only contract records;
the unchanged Pillow run is regression evidence only.

Historical acceptance record: lossless WebP VP8L subtract-green transform interval

The lossless VP8L subtract-green transform now charges a cooperative
work-budget checkpoint after each 1,024 applied pixels. The implementation/test
slice is committed at revision
`72248c6b0985fc01e82c615d3bccd01d82979acc`. The existing feature-gated
`encode_work_budget_is_a_non_parity_result_contract` uses a deterministic
1,024-pixel one-row probe and proves the inclusive whole-buffer and
caller-owned-sink rejection pair `maximum: 19`, `observed: 20`, with sentinel
`[0xAB]` untouched. The ordinary no-token path remains on the original
whole-buffer helper. Pillow has no caller token or work-budget result, so this
is Rust-only resource-contract evidence and adds no parity row, fixture,
diagnostic origin, new test function, or coverage-only hook.

Managed Pillow parity run `9d9b19e7-7d2c-49b3-8dd3-63e1b674a6a5` passed
1,445/1,445 checks; feature-matrix run
`97f350b7-00db-46dd-92e0-3ffbe63df537` passed all 24/24 configured lanes in
61,898 ms and retained the capability-table agreement with no targeted
lock-wait/build-directory/package-cache matches; and Coverage MCP run
`4f248f97-5bec-4352-afb8-5b688e1d0dd4` passed 85/85 tests and ingested snapshot
`e2ca902b-ff80-48e0-bbb9-a8ab7a9bbc5f`. The snapshot reports 52,220/52,775
lines, 7,229/7,352 branches, 2,963/3,039 functions, and 80,766/81,989
regions. The known LLVM JSON segment-normalization warning remains; the
aggregate shortfall is 555 lines, 123 branches, 76 functions, and 1,223
regions. These are implementation, target, and Rust-only contract records;
the unchanged Pillow run is regression evidence only.

Historical acceptance record: lossless WebP VP8L predictor-transform interval

The lossless VP8L predictor's final mode-application pass now charges a
cooperative work-budget checkpoint after each 1,024 applied pixels. The
implementation/test slice was committed at revision
`11501b65ba2b1d72d6b1813f74b7eaa1b267fbd2`. Its existing feature-gated contract
proved the inclusive whole-buffer and caller-owned-sink rejection pair
`maximum: 3,635`, `observed: 3,636`, with sentinel `[0xAA]` untouched. Pillow
has no caller token or work-budget result, so this remained Rust-only
resource-contract evidence and added no parity row, fixture, diagnostic
origin, new test function, or coverage-only hook.

Managed Pillow parity run `4e3cd6c5-fd16-4f97-8948-e6674bbf23c1` passed
1,445/1,445 checks; feature-matrix run
`19b3198f-43fe-413f-943c-9c899e98cba8` passed all 24/24 configured lanes in
36,418 ms and retained the capability-table agreement with no targeted
lock-wait/build-directory/package-cache matches; and Coverage MCP run
`9fe150d0-c322-4add-ad0a-45f7562ea670` passed 85/85 tests and ingested snapshot
`f39a47f3-1a59-4921-b1cf-ff0312a612d4`. The snapshot reported 52,200/52,754
lines, 7,226/7,348 branches, 2,962/3,038 functions, and 80,746/81,961
regions. The known LLVM JSON segment-normalization warning remained; the
aggregate shortfall was 554 lines, 122 branches, 76 functions, and 1,215
regions. These were implementation, target, and Rust-only contract records;
the unchanged Pillow run was regression evidence only.

Historical acceptance record: optimized regular test profile

The regular Cargo test profile now uses `opt-level = 2` at implementation/test
revision `3812762c0756330ff11b963791847e9ace38ddb9`. The feature-matrix script
continues to override its isolated compile-heavy lanes to `MATRIX_TEST_OPT_LEVEL=1`.
In paired warm local observations, the all-feature `feature_gate_tests` suite
completed 45 tests in 2.69 seconds at level 2 versus 3.19 seconds at level 1
with four test workers. This is an execution observation rather than a universal
speedup claim; compile cost, cache state, and runner scheduling vary. The change
does not alter production profiles, fixtures, manifest rows, assertions, or
Pillow/Rust evidence origins.

Managed Pillow parity run `2d33d0f2-13fe-4228-90cb-1024108d31b4` passed
1,445/1,445 checks; feature-matrix run
`d78b33d2-29cf-4f13-b76b-1aac5bf563e7` passed all configured lanes and ended
with the capability-table agreement; Coverage MCP run
`0db71e3e-c0bc-40a6-a33f-92a1f9060ec0` passed 85/85 tests and ingested snapshot
`497947de-526a-475d-8ede-6d9ea903372e`. Coverage totals remain
52,187/52,742 lines, 7,222/7,344 branches, 2,962/3,038 functions, and
80,723/81,937 regions, with the known LLVM segment-normalization warning and
the strict aggregate shortfall unchanged.

Historical acceptance record: JPEG baseline entropy MCU checkpoint and compact probe runtime

The JPEG baseline entropy traversal checkpoint is implemented and tested at
implementation/test revision `79de2f10dab8735abadd1fa19db346963656b670`.
`EntropyOutputCheckpoint` charges after each 1,024 baseline MCUs. The existing
Rust-only `encode_work_budget_is_a_non_parity_result_contract` uses a low-entropy
generated 512x512 RGB probe with exactly 32x32 default 4:2:0 MCUs and proves
whole-buffer and direct-sink rejection at `maximum: 7,720`, `observed: 7,721`,
leaving sentinel `[0x63]` untouched. The focused contract completed in 3.23
seconds locally after reducing unnecessary entropy complexity; that duration is
runner-sensitive rather than a universal benchmark. Pillow has no caller token,
work-budget result, or caller-owned sink, so this adds no parity row, fixture
file, diagnostic origin, new test function, or coverage-only hook.

The same revision's managed Pillow parity run
`3843fdd6-0ae4-4017-97fc-50668fdbbd20` passed 1,445/1,445 checks in 2,184 ms;
feature-matrix run `e1ba65de-3e66-4083-8b82-f53c875ae9ad` passed all 991/991
checks across 24/24 lanes in 27,240 ms; and Coverage MCP run
`54c9e950-1bf1-4467-85c1-1eb9b6ae2673` passed 85/85 tests in 63,746 ms and
ingested snapshot `14149605-982f-4340-bc1c-58c66edab530`. The snapshot retains
52,187/52,742 lines, 7,222/7,344 branches, 2,962/3,038 functions, and
80,723/81,937 regions, unchanged from the preceding accepted 81714bc snapshot;
the known LLVM segment-normalization warning and strict aggregate shortfall
remain.

Historical acceptance record: WebP VP8 coefficient checkpoints and compact probe runtime

The current WebP VP8 work-control test optimization is recorded against
implementation/test revision `5f058fecdf63c69a80f4f177f542860264d8cba3`.
Token-aware coefficient coding charges the 524,288- and 1,048,576-logical-coded-bit
checkpoints; first-partition remains covered through 262,144 bits. The existing
`encode_work_budget_is_a_non_parity_result_contract` reuses one deterministic
quality-100 832×832 RGB checkerboard for both coefficient boundaries. Its
whole-buffer maximum/observed pairs are `187,405/187,406` and
`318,670/318,671`; the direct-sink pairs are `187,404/187,405` and
`318,669/318,670`, with sentinels `[0xD9]` and `[0xDA]` untouched. The
adjacent 262,144-bit first-partition boundary remains exact at whole-buffer
`66,879/66,880` and direct-sink `66,878/66,879`, with sentinel `[0xD7]`
untouched. The
1,920×1,920 witness is no longer allocated by the current contract. This is a
targeted boundary witness, not a general benchmark or a claim of universal
codec speedup. Pillow has no caller token, work-budget result, or caller-owned
sink, so this remains Rust-only resource-contract evidence: no parity row,
fixture file, diagnostic origin, new test function, or coverage-only hook was
added.

The focused one-test contract completed in 3.12 seconds; the full local
all-feature test set passed 82 tests, and strict all-target Clippy, rustfmt,
doctest, and repository provenance gates passed. Managed Pillow parity run
`0f8cb18c-8eec-47c3-86bf-6453dfea9ce3` passed 1,445/1,445 checks with zero
failures or skips in 921 ms. Feature-matrix run
`5f1bab78-086b-44b1-a489-1cc9eece23e4` passed all 991/991 checks across 24/24
configured lanes in 31,864 ms; its retained log records `cache=warm lanes=24
test_threads=1 build_jobs=1 debug=0 verbose=0`, ends with `capability tables OK:
every native and wasm32-wasip1 lane agrees`, and has no targeted lock-wait,
build-directory, or package-cache matches. Coverage MCP run
`3759c978-c56c-41f9-8f29-1ebea991ddc8` passed 85/85 tests in 66,885 ms and
ingested snapshot `366f5243-7d98-4f0f-bc2e-82e2d811439f`:
52,174/52,729 lines, 7,220/7,342 branches, 2,960/3,036 functions, and
80,707/81,921 regions. Compared with `4b35942a-516c-41c7-8421-6cbf8b24aed4`,
covered and source totals changed by `+1/-1/0/-1` for
lines/branches/functions/regions. The VP8 partition file reports 537/541
lines, 88/88 branches, 30/30 functions, and 775/817 regions; residual reports
424/435 lines, 69/70 branches, 21/21 functions, and 589/632 regions. The
known LLVM segment-normalization warning remains; the strict aggregate
shortfall is 555 lines, 122 branches, 76 functions, and 1,214 regions. These
are implementation/coverage records
separate from Pillow parity, and no coverage-only test was used.

Historical acceptance record: WebP VP8 524,288-bit coefficient checkpoint and bounded probe runtime

The WebP VP8 work-control slice was implemented at
`74162955c8edfcbe940f4d6efa6ec8814dbbcfc6`. Token-aware coefficient coding
charged the 524,288-logical-coded-bit checkpoint nested after the existing
262,144-bit checkpoint; first-partition remained covered through 262,144 bits.
The existing `encode_work_budget_is_a_non_parity_result_contract` reused one
deterministic 1,920×1,920 high-entropy RGB probe at quality 100 to prove exact
whole-buffer and direct-sink rejection in both paths: coefficient
maximum/observed counts were `524,287/524,288` and `524,286/524,287`, with
sentinel `[0xD9]` untouched. The 768×768 and 1,024×1,024 probes remained the
compact 131,072-bit and 262,144-bit witnesses; this boundary did not restore
the discarded 2,048×1,024 exploratory allocation. Pillow had no caller token,
work-budget result, or caller-owned sink, so this was Rust-only resource-contract
evidence: no parity row, fixture file, diagnostic origin, new test function, or
coverage-only hook was added.

The focused contract and full local all-feature test set passed; the
45-test feature-gate target finished in 3.78 seconds, and strict all-target
Clippy, rustfmt, and repository provenance gates passed. Managed Pillow parity
run `aa71a8b4-5917-4e4e-93a2-e9621ad27a42` passed 1,445/1,445 checks with zero
failures or skips in 2,752 ms. Feature-matrix retry
`3b67131b-7623-494c-aa23-0bbb87ba7bef` passed all 991/991 checks across 24/24
configured lanes in 10,823 ms; its retained log records `cache=warm lanes=24
test_threads=1 build_jobs=1 debug=0 verbose=0`, ends with `capability tables OK:
every native and wasm32-wasip1 lane agrees`, and has no targeted lock-wait,
build-directory, or package-cache matches. Coverage MCP run
`57f55eb9-3221-4b63-8a6c-2f136038a706` passed 85/85 tests in 161,505 ms and
ingested snapshot `a702f500-9fa2-4fdc-83ad-30655cbaf191`:
52,169/52,724 lines, 7,219/7,340 branches, 2,960/3,036 functions, and
80,703/81,915 regions. Compared with `f8af387e-26a2-43fc-bdc7-65a8b098705f`,
covered totals increased by 12 lines, 6 branches, 0 functions, and 18 regions;
source totals grew by 15 lines, 6 branches, 0 functions, and 18 regions. The
changed coefficient file reported 420/430 lines, 68/68 branches, 21/21
functions, and 586/626 regions. The known LLVM segment-normalization warning
remained; the strict aggregate shortfall was 555 lines, 121 branches, 76
functions, and 1,212 regions. These were implementation/coverage records
separate from Pillow parity, and no coverage-only test was used.

Historical acceptance record: WebP VP8 262,144-bit logical checkpoints and bounded probe runtime

The WebP VP8 work-control slice was implemented at
`11594a532f853ff9817ddca001c2f6144b6d053d`. Token-aware first-partition and
coefficient coding charged the 262,144-logical-coded-bit checkpoint nested
after the existing 131,072-bit checkpoint. The existing
`encode_work_budget_is_a_non_parity_result_contract` reused one deterministic
1,024×1,024 high-entropy RGB probe at quality 100 to prove exact whole-buffer
and direct-sink rejection in both paths: first-partition maximum/observed
counts were `66,874/66,875` and `66,873/66,874`, with sentinel `[0xD7]`
untouched; coefficient counts were `148,071/148,072` and `148,070/148,071`,
with sentinel `[0xD8]` untouched. The 768×768 probe remained the compact
131,072-bit witness, so the boundary did not restore the discarded
2,048×1,024 exploratory allocation. Pillow has no caller token, work-budget
result, or caller-owned sink, so this was Rust-only resource-contract evidence:
no parity row, fixture file, diagnostic origin, new test function, or
coverage-only hook was added.

The focused contract and full local all-feature test set passed; the
45-test feature-gate target finished in 3.24 seconds, and strict all-target
Clippy, rustfmt, and repository provenance gates passed. Managed Pillow parity
run `4f3d8a38-6c06-4806-bea6-7032e84c077a` passed 1,445/1,445 checks with zero
failures or skips in 3,969 ms. Feature-matrix run
`f137e3e0-42c3-4287-9b4a-08a7a0656b16` passed all 991/991 checks across 24/24
configured lanes in 42,579 ms; its retained log ended with
`capability tables OK: every native and wasm32-wasip1 lane agrees`, and
targeted searches found no lock-wait/build-directory/package-cache matches.
Coverage MCP run `7e2fe608-90a3-4095-94e4-3be2328abe2a` passed 85/85 tests in
77,812 ms and ingested snapshot `2e17df97-e1e8-4e01-8486-b8fbbcc54aff`:
52,165/52,719 lines, 7,217/7,338 branches, 2,960/3,036 functions, and
80,696/81,909 regions. Compared with `f8af387e-26a2-43fc-bdc7-65a8b098705f`,
covered totals increased by 8 lines, 4 branches, 0 functions, and 11 regions;
source totals grew by 10 lines, 4 branches, 0 functions, and 12 regions. The
changed partition file reported 536/541 lines, 88/88 branches, 30/30 functions,
and 774/817 regions; the changed coefficient file reported 416/425 lines,
66/66 branches, 21/21 functions, and 579/620 regions. The known LLVM
segment-normalization warning remained; the strict aggregate shortfall was 554
lines, 121 branches, 76 functions, and 1,213 regions. These were
implementation/coverage records separate from Pillow parity, and no
coverage-only test was used.

Historical acceptance record: WebP VP8 131,072-bit logical checkpoints and compact probe runtime

The current WebP VP8 work-control slice is implemented at
`4642de73cea4500a26df37b0935280934ef59727`. Token-aware first-partition and
coefficient coding now charge the 131,072-logical-coded-bit checkpoint nested
after the existing 65,536-bit checkpoint. The existing
`encode_work_budget_is_a_non_parity_result_contract` reuses one deterministic
768×768 high-entropy RGB probe at quality 100 to prove exact whole-buffer and
direct-sink rejection in both paths: first-partition maximum/observed counts
are `33,524/33,525` and `33,523/33,524`, with sentinel `[0xD5]` untouched;
coefficient counts are `75,692/75,693` and `75,691/75,692`, with sentinel
`[0xD6]` untouched. The compact probe reaches both boundaries without the
discarded 2,048×1,024 exploratory allocation. This is Rust-only
resource-contract evidence: Pillow has no caller token, work-budget result,
or caller-owned sink, so no parity row, fixture file, diagnostic origin, new
test function, or coverage-only hook was added.

The focused contract and full local all-feature test set passed; the
45-test feature-gate target finished in 2.83 seconds, and strict all-target
Clippy, rustfmt, and repository provenance gates passed. Managed Pillow parity
run `80f593b8-68ef-42b9-a3ec-ecb8e9025d62` passed 1,445/1,445 checks with zero
failures or skips in 4,247 ms. Feature-matrix run
`32b78172-49dc-42d9-9c7e-f30fb3626047` passed all 991/991 checks across 24/24
configured lanes in 37,095 ms with warm `test_threads=1`; its retained log
ends with `capability tables OK: every native and wasm32-wasip1 lane agrees`.
Coverage MCP run `23feb620-aac5-4be4-b7bf-58de1fa9642d` passed 85/85 tests in
69,289 ms and ingested snapshot `f8af387e-26a2-43fc-bdc7-65a8b098705f`:
52,157/52,709 lines, 7,213/7,334 branches, 2,960/3,036 functions, and
80,685/81,897 regions. Compared with `bb395085-1498-442a-9fae-ada84a71f90e`,
covered totals increased by 14 lines, 5 branches, 0 functions, and 18 regions;
source totals grew by 10 lines, 4 branches, 0 functions, and 12 regions. The
known LLVM JSON segment-normalization warning remains, and the strict aggregate
shortfall is 552 lines, 121 branches, 76 functions, and 1,212 regions. These
are implementation/coverage and target-matrix records separate from Pillow
parity; no coverage-only test was used.

Historical acceptance record: AVIF non-primary ICC item properties

The AVIF non-primary ICC slice is implemented at
`0b8a6ff257aec7e054ec4dc79ef60c5be40f893d`. Native and portable AVIF metadata
parsers now retain non-primary `colr`/`prof` and `colr`/`rICC` declarations as
source-local `AvifItemIccProfile` records through
`SourceDescriptor::avif_item_icc_profiles()`. Inspection, still decode, and
sequence-frame decode preserve the item ID, exact profile kind, and exact raw
profile bytes without merging them into primary `SourceColor` or changing
decoded pixels. The existing
`source_alpha_matches_the_container_contract` test mutates `alpha.avif` only
in memory to associate a distinguishable `prof` payload with auxiliary item 2.
Pillow exposes neither AVIF item identity nor an item-level ICC result, so this
is Rust source-provenance evidence: no parity row, fixture file, diagnostic
origin, new test function, or coverage-only hook was added.

The focused contract and local all-feature suite passed; strict all-target
Clippy, rustfmt, and the repository provenance gates also passed. Managed
Pillow parity run `8433290d-5d75-410a-8bb5-8859508b9a8a` passed 1,445/1,445
checks in 2,619 ms with zero failures or skips. Feature-matrix run
`d27d8ef4-da63-4f4c-93b4-3738cd8b3946` passed all 991/991 checks in 83,313 ms;
its retained log records 24/24 configured lanes passed, with warm
`test_threads=1`, and no failure diagnostics. Coverage MCP run
`aa1c6ac2-db85-4f26-baec-da19d012110a` passed 85/85 tests in 91,100 ms and
ingested snapshot `bb395085-1498-442a-9fae-ada84a71f90e`:
52,143/52,699 lines, 7,208/7,330 branches, 2,960/3,036 functions, and
80,667/81,885 regions. Compared with the preceding accepted snapshot
`60efcff6-ade0-465f-a1fc-6a08f8dd655f`, covered totals increased by 87 lines,
7 branches, 12 functions, and 116 regions; source totals grew by 93 lines,
8 branches, 14 functions, and 121 regions. The known LLVM JSON
segment-normalization warning remains, and the strict aggregate shortfall is
556 lines, 122 branches, 76 functions, and 1,218 regions. These are Rust
implementation/coverage metrics, not Pillow-oracle parity metrics; other
unknown item properties and non-primary/auxiliary color forms remain open.

Historical acceptance record: WebP VP8 65,536-bit logical checkpoints

The current WebP VP8 work-control slice is implemented at
`4a7e2d525c1c5d920d3a6a1c2cb32fda3641816f`, with its runtime-reduced contract
probe recorded at `4db2c9bd6c7036a26eb854686d17a497eecce8ad`. The existing
`encode_work_budget_is_a_non_parity_result_contract` reuses a deterministic
patterned RGB probe (`1,024×1,024`, 64×64 macroblocks) to prove exact
whole-buffer/direct-sink rejection at the distinct 65,536-bit logical
checkpoints. First-partition maximum/observed counts are `19,010/19,011` for
the whole-buffer path and `19,009/19,010` for the direct-sink path, with
sentinel `[0xD3]` untouched; coefficient counts are `35,929/35,930` and
`35,928/35,929`, with sentinel `[0xD4]` untouched. The production checks nest
after the existing 32,768-bit checkpoints, so the counted work remains
inclusive and deterministic. This is Rust-only resource-contract evidence:
Pillow has no caller token, work-budget result, or caller-owned sink, so no
parity row, parity fixture, diagnostic origin, new test function, or
coverage-only hook was added.

The focused contract and local all-feature test suite passed; strict
all-target Clippy and rustfmt also passed. Managed Pillow parity run
`5fc321c3-a66d-438c-ac5b-07ad5d3467b3` passed 1,445/1,445 checks in 3,354 ms
with zero failures or skips. The same-revision feature-matrix run
`d856ec45-2311-4797-a2f5-5ef7e5dc2ea9` passed all 991/991 checks in 22,491 ms;
the warm repeat `0ceeb8bf-bedc-491b-9411-845ff9f474e2` passed all 991/991 in
8,467 ms, recorded `cache=warm lanes=24 test_threads=1 build_jobs=1 debug=0
verbose=0`, ended with `capability tables OK: every native and
wasm32-wasip1 lane agrees`, and had no `lock-wait`, `build-directory`, or
`package-cache` log matches. These are cache- and runner-sensitive observations,
not a universal benchmark; reducing the probe from 1,152×1,024 to 1,024×1,024
preserved the exact contract while lowering its input footprint from 3.4 MiB to
3.0 MiB and the focused direct test from 3.50 s to 2.53 s in this workspace.

Coverage MCP run `baafbb1b-c782-4896-948a-1aa308dc6f32` passed 85/85 tests in
54,369 ms and ingested snapshot `60efcff6-ade0-465f-a1fc-6a08f8dd655f` at the
preceding production implementation revision; the 4db2c9b change is test-only
and leaves those source coverage totals unchanged:
52,056/52,606 lines, 7,201/7,322 branches, 2,948/3,022 functions, and
80,551/81,764 regions. Compared with the preceding accepted snapshot
`90ed26c2-f559-4f03-807a-2a87c0227260`, covered totals increased by 7 lines,
3 branches, 0 functions, and 9 regions; source totals grew by 10 lines,
4 branches, 0 functions, and 12 regions. The known LLVM JSON
segment-normalization warning remains, and the strict aggregate shortfall is
550 lines, 121 branches, 74 functions, and 1,213 regions. These are Rust
implementation/coverage metrics, not Pillow-oracle parity metrics, and no
coverage-only test was used to fill the remaining gaps.

Historical acceptance record: WebP VP8 32,768-bit logical checkpoints

The historical WebP VP8 work-control slice is implemented at
`6ac422f915fce9d8ec871de7f398908a46084ce7`. The existing
`encode_work_budget_is_a_non_parity_result_contract` reuses a deterministic
patterned RGB probe for the first-partition path (`1024×960`) and the existing
coefficient probe (`512×512`) to prove exact whole-buffer/direct-sink rejection
at the distinct 32,768-bit logical checkpoints. First-partition
maximum/observed counts are `9,427/9,428` for the whole-buffer path and
`9,426/9,427` for the direct-sink path, with sentinel `[0xD1]` untouched;
coefficient counts are `11,187/11,188` and `11,186/11,187`, with sentinel
`[0xD2]` untouched. The production checks nest after the existing
16,384-boolean checkpoints, so the counted work remains inclusive and
deterministic. This is Rust-only resource-contract evidence: Pillow has no
caller token, work-budget result, or caller-owned sink, so no parity row,
parity fixture, diagnostic origin, new test function, or coverage-only hook
was added.

The focused contract and local all-feature test suite passed; strict
all-target Clippy and rustfmt also passed. Managed Pillow parity run
`e2061743-f544-40a6-b2bc-964b589b5d8f` passed 1,445/1,445 checks in 880 ms
with zero failures or skips. The same-revision feature-matrix run
`d3c24a6c-1e02-48b3-9ffb-1dccca182d63` passed all 991/991 checks in 5,468 ms,
ended with `capability tables OK: every native and wasm32-wasip1 lane agrees`,
and had no `lock-wait`, `build-directory`, or `package-cache` log matches.

Coverage MCP run `d8036f00-64a6-403a-962c-4a36b139097a` passed 85/85 tests in
48,466 ms and ingested snapshot `90ed26c2-f559-4f03-807a-2a87c0227260`:
52,049/52,596 lines, 7,198/7,318 branches, 2,948/3,022 functions, and
80,542/81,752 regions. Compared with the preceding accepted snapshot
`20285bd6-f3fe-4d9e-888f-5603aac397d5`, covered totals increased by 14 lines,
6 branches, 0 functions, and 17 regions; source totals grew by 10 lines,
4 branches, 0 functions, and 12 regions. The known LLVM JSON
segment-normalization warning remains, and the strict aggregate shortfall is
547 lines, 120 branches, 74 functions, and 1,210 regions. These are Rust
implementation/coverage metrics, not Pillow-oracle parity metrics, and no
coverage-only test was used to fill the remaining gaps.

Historical acceptance record: WebP VP8L 1,048,576-bit checkpoint

The lossless WebP VP8L work-control slice is implemented at
`c9525654b82c9cf14c61029219ec88ccf2ccd006`. The existing
`encode_work_budget_is_a_non_parity_result_contract` reuses the deterministic
656×656 high-entropy RGB probe and proves exact whole-buffer/direct-sink
rejection at the 1,048,576-logical-coded-bit checkpoint: maximum/observed
`458,751/458,752` and `458,750/458,751`, with the caller-owned sink sentinel
`[0x9D]` untouched. The production checkpoint nests after the existing
524,288-bit interval, so the counted work remains inclusive and deterministic.
This is Rust-only resource-contract evidence: Pillow has no caller token,
work-budget result, or caller-owned sink, so no parity row, parity fixture,
diagnostic origin, new test function, or coverage-only hook was added; the
unchanged Pillow run is regression evidence only.

The focused contract passed, and the local all-feature test suite passed all
82 tests; strict all-target Clippy and rustfmt also passed. Managed Pillow
parity run `799e8df6-5899-4f68-963e-baf407b5b808` passed 1,445/1,445 checks
in 2,443 ms with zero failures or skips. The first feature-matrix run
`81c35206-803e-4c16-99b3-2af83eee3600` failed one existing AVIF sequence sink
byte-identity assertion in the native/all lane; the same optimized assertion
passes locally, and the fresh exact-command retry
`0f49920c-24a1-4800-a654-bb1966974205` passed all 991/991 checks in 6,445 ms,
ended with `capability tables OK: every native and wasm32-wasip1 lane agrees`,
and had no `lock-wait`, `build-directory`, or `package-cache` log matches.

Coverage MCP run `3268cbbd-333d-4e9c-8a33-436ae07f8fc8` passed 85/85 tests in
66,295 ms and ingested snapshot
`20285bd6-f3fe-4d9e-888f-5603aac397d5`: 52,035/52,586 lines, 7,192/7,314
branches, 2,948/3,022 functions, and 80,525/81,740 regions. Compared with
the preceding accepted snapshot `b959c940-1ed9-4e1a-9c66-f0d4a9274a69`,
covered totals increased by 5 lines, 2 branches, 0 functions, and 6 regions;
the changed `src/codecs/webp/native/encoder.rs` reports 1,549/1,558 lines,
260/260 branches, 77/77 functions, and 2,243/2,347 regions, with the new
1,048,576-bit checkpoint lines and branches covered. The known LLVM JSON
segment-normalization warning remains, and the strict aggregate shortfall is
551 lines, 122 branches, 74 functions, and 1,215 regions. These are Rust
implementation/coverage metrics, not Pillow-oracle parity metrics, and no
coverage-only test was used to hide the remaining gaps.

Historical acceptance record: WebP VP8L 524,288-bit checkpoint

The next lossless WebP VP8L work-control slice is implemented at
`6af6809d57c0c1d4e3255b6f21b3edaf4849dbb8`. The existing
`encode_work_budget_is_a_non_parity_result_contract` reuses the deterministic
656×656 high-entropy RGB probe and proves exact whole-buffer/direct-sink
rejection at the 524,288-logical-coded-bit checkpoint: maximum/observed
`327,679/327,680` and `327,678/327,679`, with the caller-owned sink sentinel
`[0x9E]` untouched. The production checkpoint nests after the existing
262,144-bit interval, so the counted work remains inclusive and deterministic.
This is Rust-only resource-contract evidence: Pillow has no caller token,
work-budget result, or caller-owned sink, so no parity row, parity fixture,
diagnostic origin, new test function, or coverage-only hook was added; the
unchanged Pillow run is regression evidence only.

The focused contract passed, and the local all-feature test suite passed all
82 tests; strict all-target Clippy and rustfmt also passed. Managed Pillow
parity run `4e41cc77-2b6f-49ef-bcf0-3926d3321d40` passed 1,445/1,445 checks
in 1,797 ms with zero failures or skips. Feature-matrix run
`b8b48278-4ee6-4751-af26-e31cfc163123` passed all 991/991 checks in 38,494 ms,
ended with `capability tables OK: every native and wasm32-wasip1 lane agrees`,
and had no `lock-wait`, `build-directory`, or `package-cache` log matches.

Coverage MCP run `9c044f67-06b3-4520-bea2-5cadc8893fdd` passed 85/85 tests in
71,495 ms and ingested snapshot
`b959c940-1ed9-4e1a-9c66-f0d4a9274a69`: 52,030/52,581 lines, 7,190/7,312
branches, 2,948/3,022 functions, and 80,519/81,733 regions. Compared with
the preceding accepted snapshot `80bdf23c-8b1c-4459-ae66-fd0b789d3eb7`,
covered totals increased by 5 lines, 2 branches, 0 functions, and 7 regions;
the changed `src/codecs/webp/native/encoder.rs` reports 1,544/1,553 lines,
258/258 branches, 77/77 functions, and 2,237/2,340 regions, with the new
524,288-bit checkpoint lines and branches covered. The known LLVM JSON
segment-normalization warning remains, and the strict aggregate shortfall is
551 lines, 122 branches, 74 functions, and 1,214 regions. These are Rust
implementation/coverage metrics, not Pillow-oracle parity metrics, and no
coverage-only test was used to hide the remaining gaps.

Historical acceptance record: WebP VP8L 262,144-bit checkpoint

The current lossless WebP VP8L work-control slice is implemented at
`cc765b33d2b2846b7f17171292660cc275fb431b`. The existing
`encode_work_budget_is_a_non_parity_result_contract` extends the deterministic
high-entropy RGB probes through the 262,144-logical-coded-bit checkpoint. The
128×128 probe proves exact 32,768-bit and 65,536-bit whole-buffer/direct-sink
maximum/observed pairs of `9,287/9,288`, `9,286/9,287`, `9,288/9,289`, and
`9,287/9,288`; the 256×256 probe proves the 131,072-bit pairs
`41,439/41,440` and `41,438/41,439`, while the 656×656 probe proves the
262,144-bit pairs `262,143/262,144` and `262,142/262,143`. The caller-owned
sink sentinels `[0xA2]`, `[0xA1]`, `[0xA0]`, and `[0x9F]` remain untouched. The
probes are generated from fixed LCGs rather than added as fixtures. This is
Rust-only resource-contract evidence: Pillow has no caller work budget, typed
work-unit result, or caller-owned sink, so the change adds no parity row, parity
fixture, diagnostic origin, new test function, or coverage-only hook; the
unchanged Pillow run is regression evidence only.

Managed Pillow parity run `21f761ef-f87d-46c6-9763-7833fa6bc1e5` passed
1,445/1,445 checks with zero skips in 55,361 ms; its retained test result
reported 0.75 s of test time. Feature-matrix run
`a06a2d59-cde5-42c0-92ef-e4bd492ecb6c` passed all configured native,
`wasm32-unknown-unknown`, and `wasm32-wasip1` lanes in 57,512 ms; its
retained log ends with `capability tables OK: every native and
wasm32-wasip1 lane agrees`, with no targeted lock-wait, build-directory, or
package-cache matches. These durations are observed execution evidence only;
cache and runner state differ. Coverage MCP run
`3a3b5f99-e82d-4661-8bbf-9b9f25eb048e` passed 85/85 tests in 85,899 ms and
ingested snapshot `c5b5dedb-0685-4222-9eee-89dbf6c0a55c`, reporting
51,855/52,406 lines, 7,174/7,294 branches, 2,926/3,000 functions, and
80,297/81,511 regions. Compared with the accepted baseline snapshot
`3c69fa6e-f1ff-4a91-8685-62d07133af7d`, covered totals increased by 20 lines,
8 branches, 0 functions, and 26 regions while source totals grew by 20 lines,
8 branches, 0 functions, and 28 regions. The changed WebP native encoder
reports 1,539/1,548 lines, 256/256 branches, 77/77 functions, and
2,230/2,333 regions. The known LLVM segment-normalization warning remains;
aggregate covered deltas versus the preceding implementation snapshot
`c15b8b21-62c0-43de-a35c-e3630fcdac04` are +5 lines, +2 branches, +0
functions, and +6 regions, and the strict local verifier reports a 551-line,
120-branch, 74-function, and 1,214-region shortfall. No coverage-only test was
added.

Historical acceptance record: WebP VP8L 131,072-bit checkpoint

The historical 131,072-bit lossless WebP VP8L work-control slice was implemented at
`a2977127f25fc95d7b67e8e231bb2127d629219d`. The existing
`encode_work_budget_is_a_non_parity_result_contract` extends the deterministic
high-entropy RGB probes through the 131,072-logical-coded-bit checkpoint. The
128×128 probe proves exact 32,768-bit and 65,536-bit whole-buffer/direct-sink
maximum/observed pairs of `9,287/9,288`, `9,286/9,287`, `9,288/9,289`, and
`9,287/9,288`; the 256×256 probe proves the 131,072-bit pairs
`41,439/41,440` and `41,438/41,439`. The caller-owned sink sentinels `[0xA2]`,
`[0xA1]`, and `[0xA0]` remain untouched. The probes are generated from fixed
LCGs rather than added as fixtures. This is Rust-only resource-contract
evidence: Pillow has no caller work budget, typed work-unit result, or
caller-owned sink, so the change adds no parity row, parity fixture, diagnostic
origin, new test function, or coverage-only hook; the unchanged Pillow run is
regression evidence only.

Managed Pillow parity run `1705886c-d05a-4d08-9273-c8c480e1af6c` passed
1,445/1,445 checks with zero skips in 57,669 ms; its retained test result
reported 0.87 s of test time. Feature-matrix run
`35d59b2e-dc62-434a-beaa-b39e8c85a397` passed all configured native,
`wasm32-unknown-unknown`, and `wasm32-wasip1` lanes in 60,493 ms; its
retained log ends with `capability tables OK: every native and
wasm32-wasip1 lane agrees`, with no targeted lock-wait, build-directory, or
package-cache matches. These durations are observed execution evidence only;
cache and runner state differ. Coverage MCP run
`b221fd17-9b75-4006-be6d-4c54905085dc` passed 85/85 tests in 82,055 ms and
ingested snapshot `c15b8b21-62c0-43de-a35c-e3630fcdac04`, reporting
51,850/52,401 lines, 7,172/7,292 branches, 2,926/3,000 functions, and
80,291/81,504 regions. Compared with the accepted baseline snapshot
`3c69fa6e-f1ff-4a91-8685-62d07133af7d`, covered totals increased by 15 lines,
6 branches, 0 functions, and 20 regions while source totals grew by 15 lines,
6 branches, 0 functions, and 21 regions. The changed WebP native encoder
reports 1,534/1,543 lines, 254/254 branches, 77/77 functions, and
2,224/2,326 regions. The known LLVM segment-normalization warning remains;
aggregate covered deltas versus the preceding implementation snapshot
`183761da-aa0e-4a88-bdf8-e3bfbbb5c9c0` are +11 lines, +5 branches, +0
functions, and +15 regions, and the strict local verifier reports a 551-line,
120-branch, 74-function, and 1,213-region shortfall. No coverage-only test was
added.

Historical acceptance record: WebP VP8L 65,536-bit checkpoint

The historical 65,536-bit lossless WebP VP8L work-control slice was implemented at
`3f551029fb433c7ae5473f5e31ae135b23f67d31`. The existing
`encode_work_budget_is_a_non_parity_result_contract` extends the deterministic
128×128 high-entropy RGB probe through the 65,536-logical-coded-bit checkpoint.
It proves exact boundary rejection in both output paths: the 32,768-bit
whole-buffer/direct-sink maximum/observed pairs are `9,287/9,288` and
`9,286/9,287`, while the 65,536-bit pairs are `9,288/9,289` and `9,287/9,288`;
the caller-owned sink sentinels `[0xA2]` and `[0xA1]` remain untouched. The
probe is generated from a fixed LCG rather than added as a fixture. This is
Rust-only resource-contract evidence: Pillow has no caller work budget, typed
work-unit result, or caller-owned sink, so the change adds no parity row,
parity fixture, diagnostic origin, new test function, or coverage-only hook;
the unchanged Pillow run is regression evidence only.

Managed Pillow parity run `c6b53af9-5387-4dad-8d97-608203c71d12` passed
1,445/1,445 checks with zero skips in 47,795 ms; its test result reported
0.75 s of test time. Feature-matrix run
`2d55f6e1-d323-4bd4-ac81-558cc9193739` passed all configured native,
`wasm32-unknown-unknown`, and `wasm32-wasip1` lanes in 52,638 ms; its
retained log ends with `capability tables OK: every native and
wasm32-wasip1 lane agrees`, with no targeted lock-wait, build-directory, or
package-cache matches. The preceding warm-fanout record was 17,342 ms, but
cache and runner state differ, so these durations are observed execution
evidence rather than a universal benchmark claim. Coverage MCP run
`435b4e0f-9a1b-48b1-88ee-4a73b2ac6a03` passed 85/85 tests in 83,598 ms and
ingested snapshot `183761da-aa0e-4a88-bdf8-e3bfbbb5c9c0`, reporting
51,839/52,396 lines, 7,167/7,290 branches, 2,926/3,000 functions, and
80,276/81,497 regions. Compared with the accepted baseline snapshot
`3c69fa6e-f1ff-4a91-8685-62d07133af7d`, covered totals increased by 4 lines,
1 branch, 0 functions, and 5 regions while source totals grew by 10 lines,
4 branches, 0 functions, and 14 regions. The changed WebP native encoder
reports 1,523/1,538 lines, 249/252 branches, 77/77 functions, and
2,209/2,319 regions. The known LLVM segment-normalization warning remains;
aggregate covered deltas versus the preceding implementation snapshot
`ac2630f1-8876-4f2d-b3f1-323d9565c1aa` are zero, and the strict local verifier
reports a 557-line, 123-branch, 74-function, and 1,221-region shortfall. No
coverage-only test was added.

Historical acceptance record: warm feature-matrix fanout

The runtime-first harness change is implemented at
`a2da5260ae868fde67c6ef0b377bbabed3700c50`. In warm auto mode,
`scripts/test_feature_matrix.sh` now admits up to 24 independent cached lanes
(two per logical CPU, capped at 24), while cold mode retains six lanes and the
same compiler/build fanout. Explicit `MATRIX_JOBS`, `MATRIX_TEST_THREADS`, and
`MATRIX_BUILD_JOBS` overrides remain available. The scheduler change does not
alter the fixture manifest, parity assertions, feature-gate assertions, or
capability-table semantics.

Managed feature-matrix run `15ff59ab-53a7-4baf-bf08-bc6609d6ee89` passed all
configured native, `wasm32-unknown-unknown`, and `wasm32-wasip1` lanes in
17,342 ms. Its retained log ends with `capability tables OK: every native and
wasm32-wasip1 lane agrees`, and targeted searches found no lock-wait,
build-directory, or package-cache matches. The preceding managed record was
23,644 ms, but cache and runner state differ, so this is observed runtime
evidence rather than a universal benchmark claim. Managed Pillow parity run
`2579b92b-582b-4790-a782-11307b1d9b81` remained green at 1,445/1,445 checks;
the test result reported 0.75 s of test time within its 57,399 ms managed run.
Coverage MCP run `764072d3-885d-474c-898a-d1fc112faa8e` passed 85/85 tests
in 56,630 ms and ingested snapshot
`ac2630f1-8876-4f2d-b3f1-323d9565c1aa`. Aggregate coverage is unchanged from
the preceding implementation snapshot `54da90a3-8ade-48ca-957a-cf9cd11c7016`:
51,839/52,391 lines, 7,167/7,288 branches, 2,926/3,000 functions, and
80,276/81,490 regions. The known LLVM segment-normalization warning and the
552-line, 121-branch, 74-function, and 1,214-region strict-verifier shortfall
remain. No Pillow parity row, fixture, diagnostic origin, new test function, or
coverage-only hook changed.

Historical acceptance record: WebP VP8L 32,768-bit checkpoint

The historical 32,768-bit lossless WebP VP8L work-control slice was implemented at
`e8a2c7e1ad2ae341d359c224f3b272e533dd44fd`. The existing
`encode_work_budget_is_a_non_parity_result_contract` extends the deterministic
128×128 high-entropy RGB probe through the 32,768-logical-coded-bit
checkpoint. It proves exact boundary rejection in both output paths:
whole-buffer maximum/observed `9,287/9,288`, direct-sink
`9,286/9,287`, with the caller-owned sink sentinel `[0xA2]` untouched. The
probe is generated from a fixed LCG rather than added as a fixture. This is
Rust-only resource-contract evidence: Pillow has no caller work budget, typed
work-unit result, or caller-owned sink, so the change adds no parity row,
parity fixture, diagnostic origin, new test function, or coverage-only hook;
the unchanged Pillow run is regression evidence only.

Managed Pillow parity run `63d52d50-fde9-455b-bcd5-cc0c99dd790d` passed
1,445/1,445 checks in 829 ms. Feature-matrix run
`aab509a0-920a-428e-b040-4cfaa2bad803` passed all configured native,
`wasm32-unknown-unknown`, and `wasm32-wasip1` lanes in 23,644 ms; its
retained log ends with `capability tables OK: every native and
wasm32-wasip1 lane agrees`, with no targeted lock-wait, build-directory, or
package-cache matches. Coverage MCP run
`186a30c6-15f6-46b1-b7d0-8215a0524992` passed 85/85 tests in 47,387 ms and
ingested snapshot `54da90a3-8ade-48ca-957a-cf9cd11c7016`, reporting
51,839/52,391 lines, 7,167/7,288 branches, 2,926/3,000 functions, and
80,276/81,490 regions. Compared with accepted snapshot
`3c69fa6e-f1ff-4a91-8685-62d07133af7d`, covered totals increased by 4 lines,
1 branch, 0 functions, and 5 regions while source totals grew by 5 lines,
2 branches, 0 functions, and 7 regions. The changed WebP native encoder
reports 1,523/1,533 lines, 249/250 branches, 77/77 functions, and
2,209/2,312 regions. The known LLVM segment-normalization warning remains;
the strict local verifier reports a 552-line, 121-branch, 74-function, and
1,214-region aggregate shortfall. These are implementation/Rust coverage
metrics, not Pillow-oracle parity metrics.

Historical acceptance record: AVIF non-alpha item relationships

The preceding AVIF source-provenance slice is implemented across
`489351caa15dbdbba7e9c7d41b01a87aebfd457b` and the coverage-fixture
completeness fix `a09bda379ced7abc8b88ba09982de3a4d012ce91`. The existing
`source_alpha_matches_the_container_contract` feature-gated contract now
asserts that the committed `grid.avif` fixture retains the ordered non-alpha
`dimg` edges `1`→`2` and `1`→`3` through inspection, still decode, and
sequence-frame decode. Direct alpha `auxl` edges remain in the dedicated alpha
relationship fields, so alpha is not duplicated into the generic list. This
retains source-local relationship provenance only: it does not compose the
grid, decode auxiliary payloads, apply transforms, or expose a complete item
graph. The existing fixture and test function were reused; no Pillow parity
row, fixture, diagnostic origin, new test function, or coverage-only hook was
added. Pillow has no source-descriptor or AVIF item-graph result field, so this
is Rust-only contract evidence and the unchanged Pillow result is outer-output
regression evidence only.

Managed Pillow parity run `f007ba3a-68f9-4095-8dd4-a5f44b5688db` passed
1,445/1,445 checks in 936 ms; its retained test body passed 28/28 in 0.85 s.
Feature-matrix run `58a205d6-594a-4fd8-a431-856e96390b2e` passed all
configured native, `wasm32-unknown-unknown`, and `wasm32-wasip1` lanes in
59,367 ms; its retained log ends with `capability tables OK: every native and
wasm32-wasip1 lane agrees`, with no targeted lock-wait, build-directory, or
package-cache matches. Coverage MCP run
`65d286b2-9924-4725-b286-dfd2bccadd19` passed 85/85 tests in 58,290 ms and
ingested snapshot `3c69fa6e-f1ff-4a91-8685-62d07133af7d`. The snapshot reports
51,835/52,386 lines, 7,166/7,286 branches, 2,926/3,000 functions, and
80,271/81,483 regions. Compared with accepted snapshot
`755fa610-acb0-4c69-95cd-e357da8558a4`, covered totals increased by 85 lines,
11 branches, 10 functions, and 107 regions; source totals grew by 94 lines,
12 branches, 13 functions, and 116 regions. The known LLVM
segment-normalization warning remains; the strict local verifier reports a
551-line, 120-branch, 74-function, and 1,212-region aggregate shortfall. These
are implementation/Rust coverage metrics, not Pillow-oracle parity metrics.

Historical acceptance record: WebP VP8L 16,384-bit checkpoint

The preceding WebP lossless VP8L work-control slice is implemented at
`54de3e3f8ded6c889b59416727285297016a891e`. The existing
`encode_work_budget_is_a_non_parity_result_contract` uses a deterministic
128×128 high-entropy RGB probe to exercise the added 16,384-logical-coded-bit
checkpoint. It proves exact boundary rejection in both output paths:
whole-buffer maximum/observed `9,286/9,287`, direct-sink
`9,285/9,286`, with the caller-owned sink sentinel `[0xA3]` untouched. The
probe is generated in the test from a fixed LCG rather than added as a
fixture; it remains a feature-gate contract, not a Pillow-parity case, because
Pillow has no caller work budget, typed work-unit result, or caller-owned sink.
No parity row, fixture, diagnostic origin, new test function, or coverage-only
hook was added.

Managed Pillow parity run `e990d872-9e9b-42b0-9784-86c6558ef37a` passed
1,445/1,445 checks in 786 ms; its retained test body passed 28/28 in 0.71 s.
Feature-matrix run `966cf917-c7d9-4356-bda9-66488069ee12` passed all
configured native, `wasm32-unknown-unknown`, and `wasm32-wasip1` lanes in
27,716 ms and ended with `capability tables OK: every native and
wasm32-wasip1 lane agrees`; targeted searches found no
lock-wait/build-directory/package-cache match. Coverage MCP run
`c7561ec0-faf7-418f-9eca-e1c144f94ffd` passed 85/85 tests in 47,094 ms and
ingested snapshot `755fa610-acb0-4c69-95cd-e357da8558a4`, reporting
51,750/52,292 lines, 7,155/7,274 branches, 2,916/2,987 functions, and
80,164/81,367 regions. Compared with accepted snapshot
`229ca3ff-97d5-4f1c-b306-9a777eb3d65d`, covered totals increased by 7 lines,
3 branches, 0 functions, and 10 regions while source totals grew by 5 lines,
2 branches, 0 functions, and 7 regions. The changed WebP native encoder
reports 1,519/1,528 lines, 248/248 branches, 77/77 functions, and
2,204/2,305 regions. The known LLVM segment-normalization warning remains;
the strict local verifier shortfall is 542 lines, 119 branches, 71 functions,
and 1,203 regions. These are implementation/Rust coverage metrics, not
Pillow-oracle parity metrics.

Historical acceptance record: GIF high-color nearest-palette checkpoints

The high-color GIF RGB nearest-palette interior slice is implemented at
`6d851a1ca259598c3fa0056c0e3b25f7073cea51`. The token-aware path now reuses
candidate and merge scratch buffers, charges stable candidate ordering and the
bounded nearest-candidate scan after each 1,024 work items, and leaves the
legacy no-token byte path unchanged. The existing
`encode_work_budget_is_a_non_parity_result_contract` uses its deterministic
1,024-unique-color RGB probe to prove ample-budget byte identity and typed
`EncodeWorkUnits` rejection at the new interior boundary: whole-buffer
`2,048/2,049` and direct-sink `2,047/2,048` maximum/observed values, with both
caller-owned prefixes untouched. This is Rust-only work-control evidence:
Pillow has no caller token, work-budget result, or caller-owned sink, so the
slice adds no parity row, fixture, diagnostic origin, new test function, or
coverage-only hook.

Managed Pillow parity run `df964824-099d-4705-83ce-c05bd3321748` passed
1,445/1,445 checks with zero skips. Warm feature-matrix repeat
`1023a5a5-e7d3-4232-92b1-f74841526b6b` passed all configured native,
`wasm32-unknown-unknown`, and `wasm32-wasip1` lanes in 5,832 ms; its retained
log ended with `capability tables OK: every native and wasm32-wasip1 lane
agrees`, and targeted searches found no lock-wait/build-directory/package-cache
match. Coverage MCP run `197b4efe-230a-4979-a7bc-b8bfbd6834cb` passed 85/85
tests in 49,393 ms and ingested snapshot
`229ca3ff-97d5-4f1c-b306-9a777eb3d65d`, reporting 51,743/52,287 lines,
7,152/7,272 branches, 2,916/2,987 functions, and 80,154/81,360 regions.
Compared with accepted snapshot `8861f2ef-8624-461c-80df-4237997e94a1`,
covered totals increased by 89 lines, 21 branches, 4 functions, and 137
regions while source totals grew by 91 lines, 22 branches, 4 functions, and
143 regions. The changed GIF encoder reports 2,685/2,879 lines, 418/462
branches, 159/186 functions, and 4,322/4,638 regions; the defensive
single-candidate sort return at lines 3288–3289 remains uncovered. The known
LLVM segment-normalization warning remains, and the strict local verifier
shortfall is 544 lines, 120 branches, 71 functions, and 1,206 regions. These
are implementation/Rust coverage metrics, not Pillow-oracle parity metrics.

Historical acceptance record: WebP 8,192-bit checkpoints and shared interval traversal

The 8,192-bit WebP logical-checkpoint slice is implemented at
`d862d74eabd125539a577123d403aa808861cae5`. Token-aware VP8 first-partition,
VP8 coefficient, and VP8L bit writing now poll after each 8,192 logical bits
while retaining the nested 8/16/32/64/128/256/512/1,024/2,048/4,096 walks and
existing 16,384-bit boolean boundaries. The existing
`encode_work_budget_is_a_non_parity_result_contract` proves exact whole-buffer
and direct-sink rejection: VP8 first-partition returns `2,384/2,385` and sink
`2,383/2,384`, VP8 coefficient returns `4,343/4,344` and sink `4,342/4,343`,
and VP8L returns `58,043/58,044` and sink `58,042/58,043` (maximum/observed).
The bounded sinks retain untouched sentinels `[0xB0]`, `[0xBD]`, and `[0xA4]`.
The focused contract passed in 0.72 s; the full all-feature test suite passed
82/82 tests and strict all-target Clippy passed. This is Rust-only work-control
evidence: Pillow has no caller token, work-budget result, or caller-owned sink,
so the slice adds no parity row, fixture, diagnostic origin, new test function,
or coverage-only hook.

The current managed Pillow parity run `f3077d75-2370-48cc-9845-fdd9cfa6f698`
passed 1,445/1,445 checks in 60,630 ms. The feature-matrix run
`8336a26a-e489-4656-b92a-bd643552ba0b` passed all configured native,
`wasm32-unknown-unknown`, and `wasm32-wasip1` lanes in 101,238 ms and ended
with `capability tables OK: every native and wasm32-wasip1 lane agrees`; its
retained logs contain no `lock-wait` match. Coverage MCP run
`510d580f-3c3d-4aef-a706-e7918d300d3b` passed 85/85 tests in 279,366 ms and
ingested snapshot `a113e926-ad23-4b7e-bf48-1484830f09df`, reporting
51,540/52,081 lines, 7,127/7,246 branches, 2,897/2,968 functions, and
79,872/81,067 regions. Compared with prior accepted snapshot
`33f78a7a-0258-4224-b399-53842d46d0e4`, covered totals increased by 15 lines,
8 branches, 0 functions, and 25 regions; source totals grew by 13 lines,
6 branches, 0 functions, and 19 regions. The changed partition file reports
513/521 lines, 78/80 branches, 30/30 functions, and 744/793 regions; residual
reports 396/405, 58/58, 21/21, and 553/596; native VP8L reports 1,512/1,523,
245/246, 77/77, and 2,194/2,298. The known LLVM JSON segment-normalization
warning remains. The strict local verifier still reports the aggregate
shortfall as 541 lines, 119 branches, 71 functions, and 1,195 regions. These
are implementation/Rust coverage metrics, not Pillow-oracle parity metrics.

Historical acceptance record: WebP 16-bit checkpoints and shared interval traversal

The 16-bit WebP logical-checkpoint slice is implemented at
`1378f119a65ebd06f1d848f4757684c83e597444`. Token-aware VP8 first-partition,
VP8 coefficient, and VP8L bit writing now charge the first 16-bit logical
interval while nesting the larger 32/64/128/256/512-bit walks under that same
traversal. The existing
`encode_work_budget_is_a_non_parity_result_contract` proves exact first-interval
rejection in both whole-buffer and direct-sink paths: VP8 first-partition
return `102/103` and sink `101/102`, VP8 coefficient return `289/290` and sink
`288/289`, and VP8L return `145/146` and sink `144/145` (maximum/observed).
The retained 32/64/128/256/512 assertions use the compact fixtures' actual
interval edges, and every bounded sink retains its untouched sentinel prefix.
Three warm exact-contract repeats passed in 0.58–0.59 s of test-body time; the
full `cargo test --all-features --locked --tests` run passed 82 tests with zero
failures. The feature matrix passed all configured native,
`wasm32-unknown-unknown`, and `wasm32-wasip1` lanes and ended with
`capability tables OK: every native and wasm32-wasip1 lane agrees`. This is
Rust-only work-control evidence: Pillow has no caller token, work-budget result,
or caller-owned sink, so this slice adds no parity row, fixture, or diagnostic
origin. Coverage MCP run `6f4470e1-cc88-479c-8ab7-a908134fcb07` passed 85/85
tests in 46,644 ms and ingested snapshot
`10f0f8c4-e13c-4665-b95b-25f747dc8268`, reporting 51,512/51,992 lines,
7,121/7,216 branches, 2,898/2,968 functions, and 79,881/80,972 regions. The
known LLVM JSON segment-normalization warning remains. The changed partition
file reports 484/490 lines, 68/70 branches, 30/30 functions, and 715/763
regions; residual reports 362/372 lines, 48/48 branches, 21/21 functions, and
526/566 regions; native VP8L reports 1,489/1,498 lines, 236/236 branches,
77/77 functions, and 2,162/2,263 regions. This is implementation/Rust
coverage, not Pillow-oracle parity, and no coverage-only hook was added.

Remaining work is finer WebP bitstream and other interior work beyond the
current logical-bit, output-byte, and documented codec-internal checkpoints.
Finer Huffman/tree and other uncheckpointed work remain open, as do JPEG
interior work beyond
the current 1,024-pixel RGB-to-YCbCr and chroma-downsample output, completed 8x8 JPEG
baseline entropy traversal after each 1,024 MCUs, completed 8x8
JPEG forward-DCT/quantization-block, optimized baseline Huffman frequency gathering,
progressive scan block slots, event-frequency items, and coefficient traversal
items, and 1,024-byte entropy intervals, other codec
interior and transient-allocation boundaries,
short-write/rollback semantics, and the other roadmap categories below.

1. Finish the remaining API-023/030 and QA-026 work-control/error-detail gaps:
   transient encoded-output allocation and peak-memory accounting; interior
   encode interruption beyond the documented checkpoints, including PNG
   stored-block/copy and all-level Deflate work, BMP row-conversion
   subsegments, and finer JPEG/WebP codec work; progress and CPU/instruction
   interruption semantics; stable structured error detail where a new
   codec-internal identity or offset can be defined; and short-write,
   rollback, and partial-container cleanup. Keep allocation accounting and
   structural writing as distinct open resource boundaries.
2. Complete the remaining non-alpha non-primary/auxiliary AVIF item-metadata,
   relationship, and color-item retention under API-019/034/040. Typed
   non-primary `colr`/`nclx` CICP declarations are now retained through
   `SourceDescriptor::avif_item_color_properties()`; direct and
   supported grid-derived auxiliary-alpha provenance is represented by
   `SourceAlpha::Auxiliary` plus the scalar and plural `SourceDescriptor`
   relationship getters, bounded `iref` edges are now retained through
   `SourceDescriptor::avif_item_relationships()`, and filtered `prem` edges
   through `SourceDescriptor::avif_premultiplied_relationships()`;
   track-only, non-alpha payload semantics, non-primary ICC/other color forms,
   grid topology, and richer plane semantics remain open.
3. Extend the source-bound per-frame, strip, tile, and sequence paths under
   API-027.
4. Complete incremental structural writing, full encode interruption and
   remaining work-budget semantics, full native/WASM runtime execution, packaging,
   fuzzing, and allocation/size measurements.
5. Complete portable AVIF packaging/artifact decisions, then start deferred formats
   only after the separate acceptance decision below.

Every slice ends with:

- exact Pillow success/error fixture parity;
- exact encoded bytes where the oracle output is deterministic;
- exact decoded format, mode, dimensions, palette, metadata, sequence fields,
  and sample bytes that the public model claims to retain;
- no-panic mode/option/sequence boundary tests;
- strict Clippy and rustfmt;
- native feature-matrix tests;
- relevant WASM runtime tests; and
- Coverage MCP with line, branch, function, and region results recorded at the
  committed revision; any aggregate shortfall is named, and no coverage-only
  test is used to hide it.

## Priority 0: release blockers

### Portable AVIF

Replace the native AVIF runtime path with a repository-owned AV1/AVIF
implementation for every active still, sequence, metadata, option, success, and
error case.

Acceptance is defined in [AVIF support](avif.md#completion-criteria).

### Runtime WASM parity

Run the semantic manifest in a real WASM runtime. Cross-compiling Clippy,
rustdoc, and test binaries is not runtime evidence.

Acceptance:

- default and individually selected codec features execute in WASM;
- supported output and structured errors match native fixture expectations;
- AVIF capability differences are removed or explicitly fail the release gate;
  and
- JS/WASM artifact sizes are revision-bound and reproducible.

### Caller-controlled resource limits

Encoded-input bytes, inspected primary-canvas width, height, pixels, decoded
transfer bytes, the inspected frame/page count, every later frame/page's
decoded bytes, the cumulative retained sequence bytes, and the encoded
metadata extent are implemented through `DecodePolicy`, and every current
decoder work dimension is bounded by that set (see the architecture
reference). `EncodePolicy::max_output_bytes` now bounds complete encoded
result admission before return or the first sink write, and
`EncodePolicy::max_work_units` bounds documented cooperative encode
checkpoints with a typed `EncodeWorkUnits` failure, including PNG's adaptive
filter and filtered-row subsegments after each 1,024 row bytes, BMP row
conversion after each 1,024 pixels, and GIF RGB/RGBA palette quantization after each 1,024
pixels plus LZW's input-symbol intervals inside its dictionary pass. JPEG RGB-to-YCbCr conversion and
chroma-downsample output charge after each 1,024 pixels, baseline entropy traversal charges after each
1,024 MCUs, and JPEG's other implemented checkpoints remain in the feature-gated contract. Lossy WebP VP8 RGB/RGBA-to-YUV conversion
and RGBA transparent-area cleanup after each 1,024 scanned or flattened pixels,
and RGBA alpha-palette source collection and index packing after each 1,024 source pixels,
macroblock-analysis, and mode-selection subsegments, analysis, coefficient-
probability adaptation, 8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, 8,192-bit, 32,768-bit, 65,536-bit, 131,072-bit, and 262,144-bit logical and 16,384-boolean first-partition-bit,
8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, 8,192-bit, 32,768-bit, 65,536-bit, 131,072-bit, 262,144-bit, 524,288-bit, and 1,048,576-bit logical and 16,384-boolean coefficient-bit intervals, 1,024-byte boolean-bitstream output intervals, and bitstream
assembly, plus lossless WebP VP8L RGB/RGBA source-pixel materialization,
image-palette construction and palette-mode index packing, and predictor
source-snapshot copying after each 1,024 source pixels, predictor and mode application
after each 1,024 pixels, cross-color/entropy/transform,
bounded backward-reference search/match-length/cache/trace and repeated-run
hash-chain insertion after each 256 pixels, histogram/Huffman,
token-stream, 8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, 8,192-bit, 16,384-bit, 32,768-bit, 65,536-bit, 131,072-bit, 262,144-bit, 524,288-bit, and 1,048,576-bit logical bitstream intervals, and 1,024-byte output stages, now
charge additional checkpoints.
Whole-buffer encoders still allocate before the output-admission check; the
JPEG, PNG, BMP, ICO, and TIFF still sink paths, plus the one-frame JPEG
sequence sink path, preflight their complete lengths. PNG, BMP, GIF, and WebP
already charge the documented interior stages, including the RGBA transparent-area
cleanup checkpoint, while WebP VP8 currently has
8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, 8,192-bit, 32,768-bit, 65,536-bit, 131,072-bit, and 262,144-bit logical first-partition plus 8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, 8,192-bit, 32,768-bit, 65,536-bit, 131,072-bit, 262,144-bit, 524,288-bit, and 1,048,576-bit coefficient intervals, 16,384-boolean
first-partition/coefficient-bit intervals, and 1,024-byte boolean-bitstream
output intervals; VP8L has 8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, 8,192-bit, 16,384-bit, 32,768-bit, 65,536-bit, 131,072-bit, 262,144-bit, 524,288-bit, and 1,048,576-bit logical bitstream and 1,024-byte output
intervals. Transient encoded-output allocation/peak accounting (recoverable OOM
is deliberately outside the public contract),
remaining codec interior/deeper interruption, finer WebP work, complete
short-write/rollback semantics, and the remaining work-budget measurements are
the next resource/I/O boundaries. Complete those guarantees before
recommending the crate for arbitrary hostile inputs. Any new limit must remain
distinguishable from malformed input and be covered by complete fixtures where
observable through Pillow, or clearly labeled model-only defensive cases
otherwise.

Limit failures must be distinguishable from malformed input and must be covered
by complete fixtures where observable through Pillow, or clearly labeled
model-only defensive cases otherwise.

### Stability and support contract

Before the first registry release:

- define MSRV and supported-target policy;
- define what is semver-public;
- state pre-1.0 breaking-change expectations;
- document deprecation and migration behavior;
- define support and end-of-life windows; and
- establish a release and failed-release recovery procedure.

## Priority 1: integration quality

### Metadata preservation

Define an opaque metadata model for ICC, EXIF, XMP, textual chunks, orientation,
and format-specific blocks.

Preserve bytes first. Parsed semantic metadata should be added only where the
format contract and round-trip behavior are clear.

### Incremental I/O

Add reader/writer or incremental adapters without adding filesystem policy or
new dependencies. The byte-slice API remains the simplest path.

Acceptance must define partial input, buffering, cancellation/early exit,
output errors, and resource limits.

## Priority 2: assurance and release evidence

### Fuzzing

Add format-aware parser and round-trip fuzz targets. Fuzzing complements the
oracle matrix; it does not establish Pillow parity or security.

### Performance and size

Establish reproducible native and WASM benchmarks before making performance
claims. Every result must name the revision, toolchain, target, hardware,
features, workload, sample count, aggregation, and artifact form.

### API compatibility

Add a release-time public API diff and semver review once the surface is stable
enough to publish.

### Package policy

Resolve Cargo's warning about repository-only integration targets without
shipping the full oracle corpus or publishing an incomplete test target.
Verify the exact `.crate` contents and first-use path outside the source
checkout.

## Possible format expansion

New formats are lower priority than completing the current contracts. A format
is eligible only when:

- it is independently feature-gated;
- decode and encode are codec work rather than image processing;
- it works natively and on WASM;
- it adds no dependency;
- a fixed oracle or specification-backed reference exists;
- errors and exact outputs have manifest fixtures; and
- licenses and provenance are complete.

Candidate inventory is research-only. None is accepted while portable AVIF,
runtime WASM, limits, and the current eight-format contracts remain open.

| ID | Candidate | Why it may fit | Blocking decision/minute scope |
| --- | --- | --- | --- |
| FMT-000 | Raw AV1 still-picture bitstream | AV1 is already codec work required inside AVIF. | It is not itself a general image container. Keep AV1 internal unless a real caller needs raw OBU/Annex-B still input/output with its own color/dimension contract. |
| FMT-001 | QOI | Small specification, lossless RGB/RGBA, deterministic bytes, natural Rust/WASM fit. | Pillow 12.2.0 has no built-in oracle; require a pinned specification/reference exception and exhaustive malformed/overflow cases before eligibility. |
| FMT-002 | Netpbm (`PBM`/`PGM`/`PPM`/`PAM`) | Simple uncompressed family, Pillow coverage, useful high-depth and parser-limit fixtures. | Decide whether one feature/format enum covers six magic values, ASCII/binary variants, tuple types, comments, maxval scaling, and exact whitespace output. |
| FMT-003 | TGA | Pillow-readable/writable, palette/RLE/direct-color variants, bounded container complexity. | Origin bits, color-map ranges, 15/16-bit alpha, RLE crossing rows, extensions, developer area, and footer identity all need exact policy. |
| FMT-004 | PCX | Pillow-readable/writable and a contained palette/RLE implementation. | Plane layouts, bytes-per-line padding, EGA/VGA palettes, truncated RLE, version/header variants, and multi-plane output need fixtures. |
| FMT-005 | ICNS | Multi-entry image container fits the existing no-resize ICO direction. | Embedded PNG/JPEG 2000/legacy pixel and mask blocks make capability transitive; entry enumeration must exist first. |
| FMT-006 | farbfeld | Tiny lossless 16-bit RGBA specification and deterministic stream. | No built-in Pillow oracle; low ecosystem value means it follows QOI and current 16-bit transfer work. |
| FMT-007 | Radiance HDR/RGBE | Image codec work with float/high-range semantics and Pillow decode evidence. | Confirm a deterministic encode oracle, orientation/exposure/gamma contract, RLE variants, float transfer mode, and non-processing color policy. |
| FMT-008 | DDS | Common container with raw and block-compressed texture payloads. | Many variants are GPU texture formats rather than ordinary raster codecs; mipmaps, arrays, cubemaps, BCn encode scope and oracle choice are unresolved. |
| FMT-009 | JPEG 2000/JP2 | Pillow can use OpenJPEG and the format is relevant to ICNS/TIFF ecosystems. | Wavelet codec size, precinct/tile/progression/color-box complexity, native-oracle licensing, zero-dependency and WASM cost make it parked. |
| FMT-010 | JPEG XL | Modern still/animation, high depth, color and metadata capabilities. | No pinned Pillow 12.2.0 built-in oracle and substantial transform/modular/VarDCT scope; parked until an acceptable fixed oracle and independent vectors exist. |
| FMT-011 | HEIF/HEIC | ISO-BMFF work could share bounded container concepts with AVIF. | HEVC implementation, licensing/patent review, grids/auxiliary images and missing pinned Pillow oracle make it parked. AVIF completion does not imply HEIC. |
| FMT-012 | OpenEXR | Important high-depth/multipart image interchange. | Arbitrary channels, deep data, tiled levels, compression families, metadata and float layouts require a much broader transfer model; parked. |
| FMT-013 | SVG/PDF/video | Frequently accepted by image tools at a higher layer. | Explicitly ineligible: rendering/vector/page/video processing is not raster image codec implementation for this repository. |

Format acceptance requires a short decision record in this table first:
container identity, source/output modes, still/sequence kind, metadata, limits,
oracle version, exact-output determinism, upstream corpus, license, WASM
runtime, feature dependencies, and estimated native/WASM size.

## Explicit non-goals

The roadmap does not include:

- resizing, cropping, rotating, drawing, filters, or general compositing;
- a `DynamicImage` compatibility layer;
- path traversal or filesystem sandbox policy inside the crate;
- runtime Python or Pillow;
- plugin-loaded or dynamically registered codecs;
- logging infrastructure;
- weakening exact byte/error assertions to broaden nominal support; or
- replacing dependency-free implementations with native libraries.

## Documentation maintenance

Maintain only four files under `docs/`:

- `architecture.md` for current contracts and boundaries;
- `testing.md` for oracle and verification workflow;
- `avif.md` for the one target-dependent codec boundary; and
- `roadmap.md` for planned work.

When an item ships, remove it from the roadmap and update the current contract.
Do not create per-sweep, per-branch, completion-audit, or downstream-project
documents here. Git history, commits, Coverage MCP artifacts, and the owning
downstream repository retain that evidence.
