# Roadmap

Status: accepted direction; items below are planned unless marked implemented

Reviewed: 2026-08-02 on the committed tree based on revision `cb0f67d2e76e99eefc2595317fd49fb5202a7162`

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
- 100% line, branch, function, and region coverage.

Native AVIF is migration debt, not an exception to the final WASM constraint.

## Implemented foundation

The following decisions are already implemented and are not roadmap work:

- canonical auto-detecting root APIs;
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
`cb0f67d2e76e99eefc2595317fd49fb5202a7162`, identified by manifest SHA-256
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

The current all-feature Coverage MCP run
`f47985c1-50c8-4752-8d83-ad71973fc7c7`, snapshot
`14b3897e-1477-4a60-96bf-4ddff5d56e02`, passed 56 tests with zero failures
or skips and reports 47,926/47,926 lines, 6,578/6,578 branches,
2,683/2,683 functions, and 74,618/74,618 regions. The same committed
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

The active manifest contains 1,024 decode/inspect/verify rows and 393 encode rows.
There are no planned or unwired rows. That is strong revision-bound evidence,
but row count and 100% structural coverage do not expand the assertion schema.

| Surface | What is asserted now | Missing from the oracle assertion |
| --- | --- | --- |
| Detection | Explicit operation success/error and expected common `ImageFormat`; Pillow registration predicates cover seven formats, while AVIF uses the bounded specification/libavif compatibility rule and retains Pillow's final open outcomes | Extension aliases, ICO-versus-CUR identity, and the separate headerless-DIB scope decision |
| Decode | Explicit still-operation success/error, exact width, height, mode, palette state and table bytes, decoded byte length, every decoded pixel/sample byte, and exact TIFF source byte order | Decoded auxiliary metadata and non-byte-order source descriptors |
| Inspect | Explicit operation success/error, format, width, height, mode, encoded bit depth, bit-depth evidence origin, exact palette state and table bytes for successful decode rows, animation flag, optional frame count, and exact TIFF source byte order | ICC/EXIF/XMP/text/orientation; independent palette bytes for inspect-success/decode-error rows; broader source descriptors |
| Sequence decode | Exact canvas, loop, background, frame count/order, source rectangle, rational duration, disposal, blend, interlace, default-image state, pixel layout, mode/size, exact TIFF per-page source byte order, and exact rendered frame bytes where Pillow exposes the same layout | Exact raw GIF source-rectangle bytes and auxiliary per-frame metadata |
| Encode success | Explicit still/sequence operation applicability, exact complete encoded bytes, container checks, and exact re-decoded reference pixels when applicable | Systematic coverage of every Pillow input mode × target format; metadata not represented by the source model |
| Encode/decode error | Explicit per-operation failure; exact Pillow exception type/message when an exception exists; separately asserted Rust kind, selected format, non-empty contextual diagnostic policy, and evidence origin | Pillow has no equivalent fields for operation stage, byte offset, chunk/marker/tag identity, typed limit reason, cancellation, or output-write cause; those are separate Rust contracts |
| Lazy source | Inspection before decode, one shared successful or failed still decode, concurrency, and clone identity for a selected success per format | Lazy sequences; not-attempted versus cached-failure state; cache eviction; repeated verification cost |
| Coverage | 100% aggregate native all-feature line, branch, function, and region metrics across parity, defensive contracts, and permitted private coverage models; row assertion origins remain separate; every exact `#[cfg(coverage)]` guard is accounted for by the static non-Pillow origin inventory | Full semantic manifest execution in a WASM runtime |

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

| ID | Class | Finding | Attack and acceptance |
| --- | --- | --- | --- |
| API-003 | Missing capability | Common decode is auto-detect only. There is no explicit-format decode for trusted out-of-band format knowledge or ambiguous/partial containers. | Decide whether `decode_with_format` improves codec-only use without duplicating dispatch. If accepted, it must still validate the format signature/contract and never bypass safety checks. |
| API-006 | Invalid-state surface | `DecodedImage` exposes both `color` and `mode`; callers can create disagreement. Palette/image/sequence fields are also all public. | Keep zero-copy construction possible, but consider checked constructors/builders and clearly label direct field construction as unchecked. Every consumer remains validation-gated. |
| API-008 | Missing representation | No YCbCr/YCCK/BGR transfer mode exists, constraining otherwise codec-native JPEG/TIFF/WebP/AVIF input contracts. | Add a mode only when at least one decode or encode fixture needs byte-preserving transfer. Avoid adding modes merely to mirror another library. |
| API-012 | Lazy-loading limitation | `EncodedImage` lazily caches only `DecodedImage`, not `DecodedSequence`. Animated callers must use eager `decode_sequence`. | Add a separate cached sequence path or a source view with independently named still/sequence caches; prove no accidental first-frame collapse. |
| API-013 | Lazy-state ambiguity | `is_decoded()` is false for both "never attempted" and "failure already cached." | Add a small decode-state enum if callers need observability; do not expose internal synchronization details. |
| API-014 | Memory behavior | Lazy materialization retains the complete encoded snapshot and decoded raster forever; clones share both, but there is no eviction. Repeated `verify` reparses independently. | Document peak memory now; benchmark before adding optional cache release or cached verification. |
| API-017 | Output model | Encoders still produce complete `Vec<u8>` output for their return APIs and for every remaining whole-buffer sink path. The dependency-free `OutputSink` contract normalizes sink rejection to `ImageError::OutputWrite`; PNG and BMP still sinks, one-frame BMP sequence sinks, ICO still sinks, and one-frame ICO sequence sinks now emit validated structural segments after exact-length preflight. ICO delivery splits a fixed 22-byte directory header from its embedded PNG/DIB payload; PNG's filtered/compressed working buffers, BMP row/palette segments, and ICO embedded payload remain bounded working state rather than universal streaming. | Extend the structural writer to the remaining codecs and remaining sequence-capable paths, and reduce transient working buffers one independently enforceable boundary at a time. Keep `Vec` convenience wrappers, preserve the structured output-write cause, and define short-write/flush/rollback semantics before claiming universal streaming. |
| API-018 | Input model | The incremental input contract now covers detection, basic inspection, still decode, and sequence decode (`decode_prefix`/`decode_sequence_prefix`, COR-059) with exact or progress-aware `NeedMoreData { minimum }`; streaming decompression that produces partial pixels before the container completes remains future work. | Keep the same status semantics for any future streaming iterator/reader surface. |
| API-019 | Metadata | PNG known metadata chunks, GIF extensions, JPEG APPn/COM marker payloads, WebP ICCP/EXIF/XMP chunks, TIFF metadata tags, and AVIF top-level unknown/free/skip boxes are retained as raw opaque records. Recognized AVIF `Exif` items and `mime` items with content type `application/rdf+xml` are now retained as ordered raw `OpaqueMetadata` records on still and sequence decode; primary AVIF CICP/`clli`/`mdcv` color properties, `prof`/`rICC` ICC profiles, primary `av1C` chroma sample position, and `irot`/`imir`/`pasp`/`clap` item properties remain typed source descriptors. Non-primary/auxiliary item relationships, unknown item properties, and other item metadata remain open. | Extend the opaque model to the remaining AVIF item/property graph and exact color fields; parsed semantics are optional and format-specific. |
| API-020 | Same-format output | Source format is retained, but encoding always asks for an explicit target. | Keep explicit target selection. Add a same-source convenience only if metadata, sequences, and unsupported modes cannot make it silently lossy. |
| API-023 | Partial capability | One typed, defaulted `DecodePolicy` now bounds encoded bytes, the inspected primary canvas width/height/pixels, primary decoded transfer bytes, the inspected frame/page count, every later frame/page's decoded bytes, the cumulative retained sequence bytes, and the encoded metadata extent across inspect/still/sequence/lazy paths. `EncodePolicy::max_output_bytes` caps the complete encoded result after whole-buffer codec work and before return or sink write; `EncodePolicy::max_work_units` independently caps the number of documented cooperative encode checkpoints before the next checkpoint proceeds. PNG and BMP still, one-frame BMP sequence, ICO still, and one-frame ICO sequence sink delivery additionally compute and check their complete lengths before their first structural writes. Every current decoder work dimension is bounded by the decode resource set; transient encoder allocations, ICO embedded payload working buffers, interior CPU/instruction work, and remaining non-structural sink assembly remain outside both public policies. Lenient-versus-strict parsing and requested output mode are result-shaping policy belonging with the API-033 family. | Extend the resource contract one independently enforceable allocation/work dimension at a time, including transient encoded-output accounting, interior work accounting, structural writing for remaining paths, and deeper interruption. Preserve the unlimited convenience wrappers, reject before any future bounded allocation/work begins, and fixture every inclusive boundary and error-precedence rule. |
| API-026 | Ownership limitation | Decoded samples and palettes are always owned mutable vectors. Callers cannot borrow immutable output, reuse an allocation, or transfer shared backing storage without a copy. | Let the destination-buffer work solve reuse first. Add borrowed/shared public representations only if native and WASM measurements show a material copy cost. |
| API-027 | Sequence scalability | The source-bound `decode_frame` contract is complete with stable per-frame errors, and TIFF has a genuine per-page decode path. GIF, APNG, WebP, and AVIF still decode the full sequence for one frame, and there is no iterator or cache policy. | Extend the per-frame path to GIF/APNG/WebP/AVIF, then add iteration and cache policy. Keep eager `decode_sequence` as a convenience collector. |
| API-030 | Error detail | Codec-dispatched failures now retain a stable operation `stage`, the encoded-input byte `offset`, and a container-structure `identity` through the corresponding accessors. Caller-owned sink rejection has the separate `OutputWrite` category with selected output format, encode stage, and diagnostic message; `EncodePolicy` failures carry the selected format, encode operation, typed `EncodedOutputBytes` or `EncodeWorkUnits` resource, maximum, and observed result/checkpoint value. `Unsupported` additionally exposes `unsupported_reason()` for target-unavailable and not-implemented capability failures. BMP header, palette, pixel-span, bitfield, and RLE parse failures now retain stable context, ICO header, directory, entry-range, and embedded PNG/DIB/CUR failures now retain stable ICO context, TIFF compressed strip/tile payload failures now retain `tiff_strip`/`tiff_tile` context, and WebP inspection/container-chunk failures now retain stable WebP context. WebP still and sequence payload-decoder failures now retain `webp_bitstream` at the validated VP8/VP8L payload start, or the current ANMF container offset for animation; finer decoder-internal cursors remain intentionally limited. | Extend structured fields without promising unstable prose. Every newly represented field needs malformed, boundary, capability, and output-destination fixtures. |
| API-033 | Output-sample ambiguity | Callers cannot choose source-preserving versus normalized samples, byte order, alpha association, or a codec-native output colorspace. | Define explicit output policy only for byte-preserving codec needs. The default remains Pillow-observable normalized transfer bytes. |
| API-034 | Missing metadata | PNG source color fields (sRGB intent, gamma, chromaticities, raw ICC profile), primary AVIF CICP/`clli` fields (primaries, transfer, matrix, range, maxCLL, maxPALL), primary AVIF `mdcv` mastering-display fields, primary AVIF `prof`/`rICC` ICC profile bytes, primary `av1C` chroma sample position, and primary AVIF `irot`/`imir`/`pasp`/`clap` declarations are retained. Recognized AVIF EXIF/XMP item payloads are retained raw, without semantic parsing or pixel transforms. Non-primary/auxiliary item color properties, JPEG Adobe/JFIF color interpretation, TIFF colorimetric tags, and WebP color metadata are not yet retained. | Preserve the remaining opaque profiles and exact container fields per format. Never imply that retaining color, metadata, or transform fields means pixel conversion was applied. |
| API-036 | Work control | Cooperative cancellation now exists for still and sequence decode and for a partial encode surface: JPEG still encoding polls color/sampling/quantization rows plus entropy/progressive scan batches; TIFF still encoding polls page preparation, row prediction, raw/PackBits/LZW checkpoints, and deflate boundaries; PNG and BMP still encoding, plus one-frame BMP sequence sink encoding, poll row preparation and structural segments in both return and sink paths; GIF still encoding now routes through block/frame/coalescing/output-assembly checkpoints; WebP still encoding polls preparation, codec-result, and metadata-assembly boundaries; native AVIF still encoding now polls preparation, frame, and finalization checkpoints; ICO still and one-frame ICO sequence sink encoding now poll source-size validation, embedded PNG work or BMP row assembly, and directory finalization; GIF/TIFF/WebP/native-AVIF sequence encoders poll their implemented frame/coalescing/page/finalization checkpoints. `EncodePolicy::max_work_units` now charges those documented cooperative encode checkpoints and returns a typed `EncodeWorkUnits` limit while preserving caller-cancellation precedence. A structural sink cancellation may leave the delivered prefix. Progress callbacks, interior CPU/instruction interruption, deeper deflate/structural interruption, and interruption for the remaining still codecs remain future work. | Extend polling and budget checkpoints into the remaining long still codecs and structural writers, then define interior/progress semantics and destination rollback without claiming that boundary cancellation is universal interior interruption. |
| API-038 | Detection policy | Auto-detection cannot be restricted to an allowed-format set or supplied a trusted format hint. This matters for partial data and downstream policy. | Let a decode policy carry an optional format hint/allow-list while retaining signature validation and feature-independent `detect_format`. |
| API-041 | WASM boundary | Rust enums, structured errors, byte ownership, and 64-bit sizes have no stable JavaScript transfer schema. | Design a versioned binding contract after native API semantics settle; preserve precise error kinds and avoid string-only JS failures. |
| API-043 | Partial-input contract | The non-terminal `NeedMoreData { minimum }` state now exists for detection, basic inspection, still decode, and sequence decode, with exact minimum-byte or progress semantics; terminal results must never be retried. | Keep the status stable for any future streaming surface and document per-operation progress. |
| API-044 | Partial capability | Current resource limits are per-call eligibility checks before cache access, are never cached, and cannot be bypassed by cached success. Future output mode, strictness, metadata, or color/alpha policies would still make the single permanent still-decode cache key ambiguous. | Keep resource eligibility outside the cache key. Before API-033 or another result-shaping policy lands, choose separately keyed materialization or explicitly disallow that policy on cached sources. |
| API-045 | Repeated parsing | `EncodedImage::new` detects and inspects once, but `decode()` calls the top-level decoder, which detects and parses the container again; `verify()` independently reparses it on every call. | Measure the duplicate work, then retain an immutable parsed header/index only when every codec can prove that reuse cannot make later validation weaker. |
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
  does. `decode_sequence` is the only operation that materializes animation;
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
| BMP-011 | Headerless DIB has no explicit input/output type even though it is a distinct useful byte contract. | Keep it out of `detect_format` unless an unambiguous signature exists; consider explicit-format-only DIB APIs under API-003. |
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
| ICO-003 | Resolved: CUR shares the ICO container identity and the selected entry's hotspot is retained. | Keep COR-010 fixtures; extend retention to every entry through ICO-007. |
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
- an in-tree, fixture-bounded portable AV1 decoder subset; and
- explicit documentation of the current target boundary.

Minute gaps:

| ID | Finding | Next slice |
| --- | --- | --- |
| AVF-001 | Portable AV1 still decode is a closed subset; sequence decode and all encode are unavailable on WASM. | Complete the existing AVIF plan before treating the feature as target-invariant. |
| AVF-002 | Native AVIF requires C compiler, archiver, dynamic libavif, dav1d, and libaom despite the repository-wide no-dependency end goal. | Treat native support only as the oracle bridge to be removed, not a permanent exception. |
| AVF-003 | Native encode supports fewer Pillow source modes. | Add exact private normalization only after portable encode architecture is selected, so work is not duplicated around FFI. |
| AVF-004 | AVIF primary `prof`/`rICC` ICC profiles and recognized `Exif`/XMP item payloads are now retained on decode; EXIF remains raw and includes the stored AVIF TIFF-offset prefix. Premultiplied-alpha metadata and its decoded-sample semantics are not represented. | Fold remaining alpha/auxiliary metadata into API-019 and define straight-versus-premultiplied decoded sample semantics. |
| AVF-005 | Sequence encoder rejects offsets and requires every frame to match one canvas; loop semantics are not represented in AVIF output. | Compare pinned Pillow/libavif sequence behavior and state unsupported properties explicitly. |
| AVF-006 | AV1 support is validated by many narrow reverse-mapped fixtures, not the AV1 bitstream specification or conformance suite. | Continue slice-by-slice first-divergence work, then add licensed libavif/AOM corpus classes with independent references. |
| AVF-007 | `avif` on native and WASM is one Cargo feature with materially different operations. | Capability discovery and runtime WASM gates must make the difference machine-readable until eliminated. |
| AVF-008 | Portable transfer is 8-bit normalized output; 10/12-bit samples, monochrome, planar YUV, and high-depth alpha cannot be retained directly. | Add exact source descriptors and one transfer layout at a time after portable AV1 correctness. |
| AVF-009 | Primary-item CICP primaries/transfer/matrix and range, primary-item `av1C` chroma sample position, primary-item `clli` maxCLL/maxPALL, primary-item `mdcv` mastering-display fields, and primary-item `prof`/`rICC` ICC profile bytes now retain through `SourceColor`; non-primary item color properties remain absent from the common model. | Preserve the remaining exact item/property fields without applying tone or color transforms. |
| AVF-011 | Grids, layered/progressive images, derived items, sample transforms, and alternative item relationships have no representation. | Classify each as decoded still, auxiliary structure, or explicit `Unsupported` with libavif fixtures and bounded graph traversal. |
| AVF-012 | Gain maps, auxiliary depth/alpha, thumbnails, and supplementary images cannot be enumerated or associated with the primary image. | Add a generic auxiliary-image relationship only after a fixture-backed use case; never flatten it silently into RGBA. |
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
| AVF-031 | Alpha can have distinct plane storage, range/quality, auxiliary association, and premultiplication relationship. `Rgba8` alone cannot prove straight-alpha semantics or exact invisible RGB. | Add source alpha descriptors and exact plane/relationship fixtures before high-depth alpha. |
| AVF-032 | `iloc` construction methods, multiple extents, data references, idat/mdat placement, non-sequential extents, and 64-bit range arithmetic are not a named corpus class. | Add structural extent fixtures with a cumulative byte/range limit and precise box/item context. |
| AVF-033 | Grid-derived images and gain maps may use different grids/dimensions from the primary image. Flattening them into one canvas loses tile relationships and partial-failure context. | Retain grid topology and validate every tile independently; composition stays private to decode. |
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
| FTR-032 | Build invalidation | `build.rs` consults target-specific `CC_<target>`, `TARGET_CC`, `AR_<target>`, and `TARGET_AR`, but emits rerun directives only for plain `CC` and `AR`. Changing the selected cross tools may not rerun the build script. | Emit every consulted environment key and add a build-script decision test. |
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
| QA-016 | A dependency-free `OutputSink` contract exists with deterministic `OutputWrite` cause coverage for every enabled whole-buffer still codec. PNG and BMP still delivery, ICO still delivery, and the one-frame BMP and ICO sequence deliveries now exercise multiple structural writes, policy preflight, and sink-triggered cancellation where implemented. Short/interrupted writes, flush/finalize errors, rollback, and partial-container cleanup at every boundary remain open. | Extend deterministic sinks to the remaining sequence/structural boundaries and define short-write, flush/finalize, and recoverable cleanup behavior before claiming a universal incremental writer. |
| QA-019 | Exact encoded-byte determinism is now proven between the ARM64 native host and `wasm32-wasip1` for a fixed encoder/decoder subset; x86-64, 32-bit, and big-endian lanes are still missing. | Run deterministic fixture subsets across the remaining targets and classify unavoidable native-oracle differences explicitly. |
| QA-020 | Peak stack use and recursion depth are not measured for nested containers, TIFF directory graphs, DEFLATE/Huffman paths, or AV1 syntax. | Add bounded deep-structure fixtures and stack instrumentation before browser/embedded recommendations. |
| QA-021 | Reverse-mapped/generated fixtures do not all retain generator version, parameters, first-divergence purpose, and minimized-input hash in the manifest. | Extend TST-009 with reproducible generation provenance and a regeneration check. |
| QA-022 | WASM compile success provides no browser evidence for boundary copies, memory growth, exceptions, worker use, or real artifact size. | Run a small Playwright/WebDriver-free JS harness in a pinned browser runtime and Node for every published artifact target. |
| QA-023 | Emitted bytes are primarily re-opened through Pillow, which can share libjpeg/libwebp/libtiff/libavif implementations with the oracle path. | Decode representative outputs with an independent implementation or browser and record that evidence separately from Pillow parity. |
| QA-024 | Round-trip tests do not publish a uniform rule separating lossless exact samples, lossy decoded tolerances, and deterministic encoded bytes. | Add an assertion policy per format/mode/option row and reject ambiguous generic “round trip passed” claims. |
| QA-026 | Decode output-size limits and cumulative sequence limits have boundary, cache-state, and retry tests under API-023. `EncodePolicy` has exact-result, one-byte-below, and no-sink-write coverage for PNG/BMP still, BMP one-frame sequence, ICO still, and ICO one-frame sequence sinks; JPEG and TIFF still encoding now have deterministic Rust-only checkpoint drills; PNG/BMP and ICO still structural writers poll deterministic row/segment or header/payload checkpoints with exact-length preflight and sink-triggered cancellation evidence, while the one-frame BMP and ICO sequence structural writers prove their preflight and delivery boundaries; GIF still and WebP still preparation/codec-result/metadata-assembly cancellation remain covered at their public and codec checkpoints, as does implemented sequence cancellation. `encode_work_budget_is_a_non_parity_result_contract` now proves ample-budget byte identity, zero-budget still/sequence rejection, typed `EncodeWorkUnits` errors, and no-sink-write behavior as ordinary Rust-only evidence. Interior interruption for the remaining encoders, short/interrupted output, and cleanup remain under API-036 and API-017/QA-016. | Extend encode work-budget and interruption tests across every structural boundary, then add short-write/flush/finalize/rollback cleanup cases; preserve both the output-write cause and output-policy/work-budget no-write guarantee across every destination failure. |
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
| 1 | API-023/030; QA-026 | Prevent unbounded work and evidence overclaims before adding more accepted inputs. TST-001 through TST-010, QA-032, the verification-strength contract, the trailing-input policy, the malformed-class ledger, the near-limit/allocation policy, the operation-stage contract, and the work-budget analysis are complete. | Fixture fails first, stable structured outcome is defined, exact oracle/model evidence is retained, and the completed no-panic matrix remains green. |
| 2 | API-019/034/040 plus the matching PNG/GIF/TIFF/WebP/ICO/AVIF metadata and sequence rows | Avoid implementing remaining multipage, multi-entry, or animated surfaces into a lossy common model. The sequence/container kind (API-028), source alpha semantics (API-035), PNG opaque blocks (API-040), PNG opaque metadata (API-019), PNG source color descriptors (API-034), GIF extension retention (API-019/040), JPEG marker retention (API-019), WebP chunk retention (API-019/040), TIFF tag retention (API-019/040), and AVIF box plus recognized EXIF/XMP retention (API-040) are complete; API-019/034 remain for non-primary/auxiliary AVIF item properties and color items. | Exact source/container state survives decode and, where supported, encode without public processing. |
| 3 | API-027 and codec row/strip/tile/frame slices | Bound memory and enable large/sequence inputs without a second unrelated API family. The checked output-size preflight, exact-size still destination API (API-024), minimal transfer-layout descriptor (API-025), basic/deep inspection split (API-032), borrowed source views (API-037), and the source-bound frame-decode contract with TIFF per-page decoding (API-027) are complete. | Whole-buffer convenience wraps the same bounded engine; caller buffers and eager results are byte-identical. |
| 4 | API-017/018/036; FTR-017 through FTR-024/027/029; QA-016/019/020/022/030 | Complete native/WASM integration, incremental I/O, cancellation, packaging and measurements. The dependency-free output-sink contract (API-017), the ARM64-native versus WASI cross-target determinism evidence (QA-019), the incremental input contract through still/sequence decode (API-018/043), decode cancellation, PNG/BMP still and one-frame BMP sequence structural writing and row/segment cancellation, ICO still and one-frame ICO sequence fixed-header/payload delivery and cancellation, GIF still block/frame/coalescing/output-assembly checkpoints, WebP still preparation/codec-result/metadata-assembly checkpoints, native AVIF still preparation/frame/finalization checkpoints, and the partial encode-cancellation boundary plus JPEG and TIFF still checkpoint slices (API-036) are complete. The first deterministic encode work-budget contract is now also present; incremental structural writing for the remaining codecs, full interior interruption/work-budget coverage, deeper deflate interruption, and the remaining measurements remain. | Real native and WASM runtime lanes pass with reproducible artifacts and measured copy/memory behavior. |
| 5 | FMT-000 through FMT-013 | Format expansion adds the most maintenance and least value while current contracts remain incomplete. | Start only after a separate acceptance decision records every eligibility field listed below. |

Within a row, choose the smallest reverse-mappable codec case and finish its
manifest, API, implementation, documentation, feature matrix, WASM runtime
evidence, and Coverage MCP result before opening the next slice.

### Execution order for the next session

This order turns each discovery into a failing fixture before implementation
and avoids broad rewrites.

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
registered with Coverage MCP. AVIF WASM operations now report staged
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
whole-buffer still codec now has a Rust-only destination-rejection witness and
rejects before its single write, while the PNG, BMP, and ICO still structural
paths plus one-frame sequence paths can leave an already-delivered prefix if a
later segment fails.
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
Token-aware encodes now cover the public still boundary; PNG and BMP additionally
poll row preparation and structural segments in return and sink paths; JPEG
polls internal color/sampling/quantization and entropy/progressive-scan
checkpoints; TIFF still encoding polls page preparation, row prediction,
raw/PackBits/LZW work, and deflate boundaries; sequence
frame/coalescing/page/finalization boundaries are implemented where supported;
native AVIF still encoding now polls its preparation, frame, and finalization
boundaries; ICO still and one-frame ICO sequence sink encoding now poll
source-size validation, embedded PNG work or BMP row assembly, and directory
finalization;
and other still codecs retain only their implemented boundary checks. The first
public `EncodePolicy::max_work_units` contract now bounds those documented
checkpoints and reports `ResourceLimit::EncodeWorkUnits`; remaining
still-codec/interior interruption, deeper deflate/structural interruption,
progress, transient allocation accounting, and universal structural writers
remain open.
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
cache/build state can differ. The harness now derives `--test-threads` from
host CPUs and the lane bound (capped at eight), and interleaves native,
`wasm32-unknown-unknown`, and `wasm32-wasip1` lanes under one
completion-driven scheduler without dropping a lane or assertion.
Coverage MCP then ingested run
`f47985c1-50c8-4752-8d83-ad71973fc7c7` as snapshot
`14b3897e-1477-4a60-96bf-4ddff5d56e02`: 56 tests passed with zero failures,
with 100% line, branch, function, and region coverage (47,926 lines, 6,578
branches, 2,683 functions, and 74,618 regions). The ICO still and one-frame
ICO sequence sink paths, the deterministic encode work-budget contract, and
their real defensive/error branches are covered through ordinary Rust
contracts; aggregate coverage remains implementation evidence rather than
Pillow-parity coverage.
Aggregate coverage and runtime matrix results are implementation evidence, not
Pillow-parity coverage.

1. Finish the remaining API-023/030 and QA-026 work-control/error-detail gaps:
   transient encoded-output allocation accounting, interior encode interruption
   and remaining work-budget semantics, remaining codec/sequence structural writers, and
   short-write/flush/finalize/rollback cleanup. Keep allocation accounting and
   structural writing as distinct open resource boundaries.
2. Complete the remaining non-primary/auxiliary AVIF item-metadata and
   color-item retention under API-019/034/040.
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
- Coverage MCP at 100% line, branch, function, and region coverage.

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
checkpoints with a typed `EncodeWorkUnits` failure. Whole-buffer encoders
allocate before that check; the PNG and BMP still sink paths preflight their
complete lengths, with PNG retaining filtered/compressed working buffers and
BMP preparing bounded palette/row segments. Transient
encoded-output allocation, interior interruption, universal structural writing,
and remaining work-budget semantics remain the next resource/I/O boundaries.
Complete those remaining
guarantees before recommending the crate for arbitrary hostile inputs. Any new
limit must remain distinguishable from malformed input and be covered by
complete fixtures where observable through Pillow, or clearly labeled
model-only defensive cases otherwise.

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
