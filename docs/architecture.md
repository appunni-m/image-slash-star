# Architecture and public contract

Status: current implementation reference

Reviewed: 2026-08-11 against measured source/evidence revision
`36b939696415a962285d37f9120ff389aebf0205`. Current aggregate native
all-feature coverage is recorded in [roadmap-new.md](roadmap-new.md); the
claim-ledger base revision is the same measured revision. The current managed
parity run is `84716077-aee7-4396-8328-e6735202b044` and the current exact
coverage snapshot is `05b6674e-e7d9-43f4-b62b-a63a2ca45cf6`. Historical run
records elsewhere in this document retain their original revision scope.
The historical exact-head managed Pillow parity run recorded below is
`49d95968-7a17-4a9d-9002-c6504922610b` (1,445/1,445 passed in 584 ms) at
its recorded revision. Feature matrix run
`2f75bfbc-866c-44de-b118-e00e2cd0936b` terminated with 44 passed and 1 failed;
the failing `source_alpha_matches_the_container_contract` lane reports the
pre-existing native AVIF decoder status-5 failure. Nightly Coverage MCP run
`f37739ea-a252-4112-8234-268e86be2798` likewise terminated 84/85 and ingested
no snapshot because its required artifact was `skipped_stale`. The same
native failure is not evidence against the current FileTypeBox/source-descriptor
slice. The historical accepted Coverage MCP snapshot was
`44cec31e-7345-4673-a9a4-e9f8fa21cc08` from run
`beda2230-4d77-446c-8ce4-91700552cdc4` at revision
`1d1b36100925f830408f5d41f0026e71fd220d6e`: 55,926/56,803 lines, 8,011/8,228
branches, 3,122/3,218 functions, and 85,972/87,930 regions. The snapshot
retains the known LLVM JSON segment-normalization
warning. Histogram coverage is 872/873 lines, 184/184 branches, and 43/43
functions; predictor coverage is 366/366 lines, 68/68 branches, and 24/24
functions; cross-color coverage is 517/530 lines, 83/86 branches, and 27/27
functions. The WebP encoder projection records 2,405/2,489 lines,
511/540 branches, 89/89 functions, and 3,471/3,751 regions; its backward-
reference file records 1,881/1,935 lines, 497/530 branches, 72/72 functions,
and 2,813/2,973 regions. The lossless-transform projection records 452/452
lines, 30/30 branches, 25/25 functions, and 883/883 regions. The Huffman
decoder projection records 351/353 lines, 56/58 branches, 13/13 functions,
and 504/511 regions. The lossless-decoder projection records 1,255/1,257
lines, 130/134 branches, 53/53 functions, and 1,620/1,624 regions. The WebP
container-decoder projection records 805/805 lines, 90/90 branches, 36/36
functions, and 1,405/1,406 regions. The VP8 decoder projection records
1,615/1,615 lines, 165/166 branches, 58/58
functions, and 2,917/2,920 regions. These are Rust implementation/coverage
metrics, not
Pillow-oracle coverage or allocator/OOM accounting.

This document explains the stable mental model and ownership boundaries of
`image-slash-star`. The generated Rust API documentation remains the
declaration-level reference.

The current feature-matrix harness runs its codec-heavy native and
`wasm32-wasip1` test binaries at `MATRIX_TEST_OPT_LEVEL=2`, matching the
regular Cargo test profile. That is validation-harness configuration, not a
production-codegen or Pillow-oracle setting; callers may override it when
compile fan-out is the limiting resource.

## What the crate owns

`image-slash-star` is a byte-oriented image codec library. It owns:

- encoded-image signature detection;
- header and container inspection;
- validated still-image and sequence decoding;
- signature-validated explicit-format still decoding for trusted out-of-band
  dispatch;
- explicit-format still-image and sequence encoding;
- source format, structural source descriptors, sample mode, color layout,
  palette, alpha, frame timing, disposal, and background transfer models;
- format-qualified typed encoder configuration with a strict legacy-pair
  migration boundary;
- one shared decode policy for pre-detection encoded bytes and inspected
  canvas width, height, and pixels;
- structured codec errors; and
- immutable encoded-byte snapshots with shared lazy materialization.

It does not own filesystem policy or general image processing. Resizing,
cropping, rotation, drawing, filtering, arbitrary compositing, color
adjustment, and mutable editing belong in downstream applications or
processing libraries.

Transforms required by a codec—JPEG IDCT, PNG filtering, AV1 prediction,
sample reconstruction, color conversion, animation disposal, and encoder
quantization—remain private implementation details.

## Mental model

Encoded format and decoded sample layout answer different questions:

```text
encoded bytes
    │
    ├─ DecodePolicy input limit
    │       │
    │       ├─ reject ────────────────► LimitExceeded (no selected format)
    │       │
    │       └─ accept
    │
    ├─ detect_format() ───────────────► ImageFormat
    │
    ├─ inspect() ─────────────────────► ImageInfo
    │
    ├─ decode() ──────────────────────► Decoded<DecodedImage>
    │                                      ├─ source ImageFormat
    │                                      ├─ ImageMode + ColorType + pixels
    │                                      └─ non-fatal diagnostics
    │
    ├─ decode_with_format(expected) ───► validate complete expected signature
    │                                      ├─ mismatch ─────► Parameter
    │                                      ├─ no signature ─► Malformed
    │                                      └─ match ────────► same decode path
    │
    └─ decode_sequence() ─────────────► Decoded<DecodedSequence>

DecodedImage / DecodedSequence
    └─ encode*(explicit ImageFormat) ──► encoded bytes
```

`ImageFormat` identifies the encoded container, such as PNG or JPEG.
`ImageMode` identifies the observable decoded byte layout. `ColorType`
describes its unpacked channel representation. `Decoded<T>` retains the source
format separately so decoding never pretends that a pixel buffer has an
intrinsic output format.

Palette indices use `ImageMode::P8` and an optional `ImagePalette`. They are not
treated as luminance merely because both layouts use one byte per sample.

`ImageInfo::is_indexed()` classifies the `P8` sample mode, while
`ImageInfo::has_palette_table()` reports whether an explicit table was
retained. These can differ for a tolerated indexed container with a missing or
empty palette table.

The shared `ico` codec also recognizes CUR. `cursor_hotspot: Some(...)` on
`ImageInfo` and `DecodedImage` preserves CUR identity and its selected entry's
activation point; ordinary ICO uses `None`.

`ImageInfo::source` and `DecodedImage::source` carry an extensible
`SourceDescriptor`. TIFF records the exact `II`/`MM` container declaration as
`SourceByteOrder::Little` or `SourceByteOrder::Big` on inspection and on every
decoded page. AVIF item `irot`/`imir`/`pasp`/`clap` properties are retained as
`AvifTransformProperties` on the primary item; `AvifTransformProperties::order()`
retains the source association order of the typed declarations without
rotating, mirroring, rescaling, or cropping decoded samples. Other codecs currently return an empty
descriptor. AVIF direct alpha `auxl` relationships are retained as
source-local item IDs through `SourceDescriptor::avif_auxiliary_relationship()`
when present, and the bounded
`SourceDescriptor::avif_auxiliary_relationships()` list also retains alpha
links to supported grid-derived color items. For a primary grid,
`SourceDescriptor::avif_grid_item_ids()` also retains its ordered derived
color-item list, and `SourceDescriptor::avif_grid_properties()` retains the
validated version, raw flags, row/column counts, and declared output canvas;
`SourceDescriptor::avif_item_relationships()` retains
bounded `iref` edges such as the grid's ordered `dimg` references, and
`SourceDescriptor::avif_premultiplied_relationships()` filters source-local
`prem` edges. AVIF `FileTypeBox` declarations are retained through
`SourceDescriptor::avif_file_type()` as `AvifFileTypeProperties`: major brand,
minor version, and ordered compatible brands bounded at 1,024 entries. The
record is propagated on inspection, still decode, and each sequence frame; it
is declaration provenance, not a complete decoder-capability decision. A
source descriptor is structural provenance, not opaque
ICC/EXIF/XMP metadata and not an instruction to reinterpret every normalized
pixel buffer.

`DecodedSequence::first()` returns the complete first `DecodedFrame`.
`first_image()` is a deliberately lossy convenience that drops the frame's
source rectangle, duration, disposal, blend, interlace, default-image state,
and pixel-layout identity. Internal sequence-to-still fallback validation does
not use that convenience.

`DecodedSequence::kind` distinguishes the container meaning of a retained
sequence: `TimedAnimation` (GIF, APNG, animated WebP, and AVIF), `UntimedPages`
(TIFF), and `SingleFrame` (still decode fallback or caller-built still
sequence). TIFF pages keep exact zero durations and are never described as
timed animation.

`Decoded<T>::diagnostics` is a dependency-free list of stable non-fatal
records returned beside successful still or sequence decode. Each
`ImageDiagnostic` carries a `DiagnosticKind`, format, operation stage, encoded
byte offset, and container-structure identity; prose is intentionally absent.
The current manifest-proven kinds are ignored trailing data, accepted
non-standard GIF graphic-control size, ignored invalid compressed PNG
ancillary metadata, an accepted bad PNG `IDAT` CRC, an accepted invalid PNG
reserved-bit chunk name, an accepted unknown ancillary chunk after `IDAT`, an
accepted static PNG stream without `IEND`, accepted duplicate PNG
palette/transparency chunks, an accepted bad PNG `IEND` CRC, and accepted APNG
declaration-length damage.
A diagnostic reports a recoverable condition; it does not change pixels or turn
Pillow's result into a new parity field. The
`IDAT` and `IEND` CRCs remain fatal at Rust `verify()`.

`SourceDescriptor::alpha()` records the alpha association declared by the
encoded source: straight (PNG, WebP, and TIFF `ExtraSamples` 2), premultiplied
(TIFF `ExtraSamples` 1), binary mask (GIF transparency), or auxiliary (AVIF
alpha items whose samples are carried by a separate image). For bounded direct
and supported grid-derived alpha relationships,
`SourceDescriptor::avif_auxiliary_relationship()` and
`SourceDescriptor::avif_auxiliary_relationships()` retain the source-local
auxiliary and target item IDs. They never change decoded transfer bytes, which
stay the documented normalized unassociated layout unless a codec explicitly
retains source-order bytes. AVIF `prem` relationships are retained separately
through `SourceDescriptor::avif_premultiplied_relationships()` and likewise do
not request a decoded-sample transformation. Typed non-primary AVIF
`colr`/`nclx` declarations are retained as bounded `AvifItemColorProperties`
records through `SourceDescriptor::avif_item_color_properties()`, preserving
the source-local item ID and CICP values without replacing the primary
`SourceColor` or changing decoded samples. Raw non-primary `prof`/`rICC`
profiles are retained as bounded `AvifItemIccProfile` records through
`SourceDescriptor::avif_item_icc_profiles()`, preserving exact item IDs and
profile bytes without replacing `SourceColor`; other item color/property forms
remain outside this typed boundary. Unknown and known non-primary
`clli`/`mdcv`/`irot`/`imir`/`pasp`/`clap` properties are retained as
`AvifItemProperty` records through `SourceDescriptor::avif_item_properties()`
with source-local item ID, four-byte kind, and exact payload; they are not
interpreted or applied to decoded samples. Known non-primary and auxiliary
`auxC`/`auxi` declarations are included in those records with their
original kind and exact full-box payload. This records auxiliary-type
provenance only; it does not select or decode auxiliary payloads or change
normalized samples. The raw `AvifItemProperty` records also retain the source
`ipma` essential bit in
association order through `AvifItemProperty::is_essential()`; this records
container intent only and does not make an unknown property executable. The
primary typed transform descriptor retains its corresponding declaration order
through `AvifTransformProperties::order()` and `AvifTransformKind`.
Both bounded AVIF parsers reject an `ipco` property table after 2,048 entries,
independently of their enclosing-box and association budgets. The existing
feature-gated source/container contract exercises the 2,049-entry boundary in
inspection, still decode, and sequence parsing; this is Rust parser-resource
evidence, not Pillow parity, because Pillow exposes no item-property table
budget or result field.

Known non-primary and auxiliary
`ispe`/`pixi` declarations are retained as `AvifItemPlaneProperties` records
through `SourceDescriptor::avif_item_plane_properties()`, preserving source-local
item ID, optional dimensions, and optional uniform channel depth; this is
structural provenance only and does not expose planes, compose tiles, infer
range/quality, or transform decoded samples.
Known non-primary and auxiliary AVIF `av1C` declarations are retained as
`AvifItemCodecProperties` records through
`SourceDescriptor::avif_item_codec_properties()`, preserving source-local item
ID, exact payload, declared bit depth, and chroma sample position. This is
source provenance only: it does not select a decoder, expose planes, compose
tiles, infer range/quality, or transform decoded samples. Pillow has no
item-level codec-configuration result, so the real alpha/grid assertions and
duplicate-association rejection remain Rust-only feature-gated evidence with
no parity row or coverage hook.

Decoded images and sequences carry `opaque_blocks` (`Vec<OpaqueBlock>`):
payload-only records with a format kind, the raw encoded payload, and the
container's safe-to-copy flag, kept in original stream order including
duplicates. PNG decode retains every uninterpreted ancillary chunk; critical
and interpreted chunks are never opaque, and default encoding never replays
retained blocks. Other containers extend the same model as their parsers
retain unknown blocks. Retained blocks count toward the caller-set
`max_metadata_bytes` extent.

Known PNG metadata chunks (tEXt/zTXt/iTXt/eXIf/tIME/pHYs/bKGD/hIST/sBIT) are
retained in a separate ordered `metadata` list of raw, unparsed
`OpaqueMetadata` records. Valid compressed members are checked with the
dependency-free DEFLATE path under a fixed 1 MiB validation output bound, but
their inflated contents are never exposed. Pillow-tolerated invalid compressed
payloads in structurally recognizable `zTXt`, `iTXt`, and `iCCP` members are
omitted and produce an `InvalidMetadataIgnored` diagnostic; malformed field
shapes retain their raw bytes. Method-only `zTXt`/`iCCP` mutations are outside
this recovery contract because Pillow rejects them. The encoded metadata extent
remains bounded by `max_metadata_bytes`. Pillow-tolerated bad `IDAT` CRCs are
decoded with a `RecoveredStructure` diagnostic (`png_IDAT_crc`) and remain
rejected by structural verification. Pillow-tolerated bad `IEND` CRCs are
decoded with a `RecoveredStructure` diagnostic (`png_IEND_crc`) while Rust
structural verification remains strict. Pillow-tolerated unknown ancillary chunk
names with a lowercase reserved third character are decoded with a
`RecoveredStructure` diagnostic (`png_reserved_bit`).
Pillow-deferred CRC failures after the first `IDAT` are likewise decoded with
`RecoveredStructure`: `png_acTL_crc`, `png_fcTL_crc`, and `png_fdAT_crc` name
the APNG members, while `png_post_idat_crc` names an uninterpreted ancillary
member. A late declaration or ancillary-order recovery can therefore produce
two records for one chunk; the records retain the same offset and remain
separate from the Pillow parity result. Pillow-tolerated unknown ancillary
chunks after `IDAT` produce the same diagnostic kind with identity
`png_ancillary_after_idat`; valid APNG control and frame-data chunks (`acTL`,
`fcTL`, and `fdAT`) are excluded from this static ordering diagnostic. A static
PNG stream that reaches EOF without an `IEND` chunk produces
`RecoveredStructure` with identity `png_missing_iend` and
the EOF offset; structural verification still rejects the missing terminator.
Duplicate `PLTE` and `tRNS` chunks keep the first palette result and produce
`png_duplicate_plte` or `png_duplicate_trns` at the ignored chunk offset.
Pillow-tolerated indexed-palette shape damage keeps the usable first result and
produces `png_trns_overlong`, `png_missing_plte`, `png_empty_plte`,
`png_partial_plte`, or `png_trns_without_plte`. A zero-frame APNG declaration
falls back to the default PNG image with `png_apng_zero_frames`; an out-of-range
APNG frame count is reported as `png_apng_frame_count_out_of_range` while the
usable default image is retained; malformed APNG
declarations that Pillow also accepts by falling back produce
`png_duplicate_actl` or `png_actl_after_idat`; an overlong `acTL` payload
produces `png_actl_overlong`; and valid inflated bytes beyond the first raster
produce `png_oversized_scanline`. These are Rust-only
defensive diagnostics because Pillow exposes the successful pixels but no
equivalent structured warning field.

Exact PNG color fields are retained in `source_color` (`SourceColor`): the
sRGB rendering intent, the gAMA value (scaled by 100,000), the eight cHRM
chromaticity values, and the raw iCCP profile (keyword plus method/profile
payload, never exposed inflated). The first well-formed occurrence of each
chunk is parsed; duplicates and malformed payloads fall back to raw metadata
records. Retaining color metadata never implies that color conversion was
applied.

For GIF, comment (0xFE), plain-text (0x01), and non-NETSCAPE application
(0xFF) extensions are retained as ordered `OpaqueMetadata` records with the
label byte as kind and the exact bytes after the label (size, sub-blocks,
terminator) as data. The NETSCAPE loop extension stays interpreted into
`loop_count`; unknown extension labels are retained as `OpaqueBlock` records
and recorded safe to copy because GIF89a requires decoders to ignore
extensions they do not understand. Still decode attaches the container records
to the returned image, and default encoding never replays extensions.

For JPEG, APPn (0xE0–0xEF) and COM (0xFE) marker payloads are retained as
ordered `OpaqueMetadata` records with the marker byte as kind and the exact
payload bytes after the length field as data. Multi-segment ICC/EXIF fragments
keep their stream order, and the APP14 Adobe transform byte remains parsed for
CMYK decoding while the payload stays retained. Truncated metadata markers
fail with the `jpeg_metadata` parse-site identity, and default encoding never
replays retained markers.

For WebP, ICCP, EXIF, and XMP chunks are retained as ordered `OpaqueMetadata`
records with the fourcc as kind and exact payload bytes as data, including
duplicates in scan order. Unknown RIFF chunks are retained as `OpaqueBlock`
records and recorded safe to copy because WebP defines no safe-to-copy bit and
unknown chunks are ignorable by decoders. Interpreted chunks (VP8/VP8L/ALPH/
VP8X/ANIM/ANMF) stay out, truncated chunks whose declared range exceeds the
input are not retained, and default encoding never replays retained chunks.

For TIFF, every non-interpreted tag is retained as a raw record with typed
identity: the tag number as two bytes in the file's original byte order and
the exact stored value bytes (inline when the value fits four bytes,
otherwise at its offset), preserving inline-versus-offset storage and
duplicates in entry order. Unknown tags become `OpaqueBlock` records recorded
safe to copy (TIFF defines no safe-to-copy bit; unknown tags are ignorable by
baseline readers), while known metadata tags (ImageDescription, Software,
DateTime, Artist, Copyright, ICC Profile) become `OpaqueMetadata` records.
Records attach per page (the still image and each sequence frame), and default
encoding never replays them.

For AVIF, unknown top-level BMFF boxes and `free`/`skip` padding boxes are
retained as raw `OpaqueBlock` records with the fourcc as kind and the full box
bytes as data, in scan order, under the documented BMFF convention (no
safe-to-copy bit; unknown boxes are ignorable). Interpreted boxes (ftyp/meta/
moov/mdat) stay out, truncated trailing boxes are ignored exactly as before,
and default encoding never replays retained boxes.
Recognized `Exif` item types and `mime` items with content type exactly
`application/rdf+xml` are retained as ordered raw `OpaqueMetadata` records on
still and sequence decode, with kinds `Exif` and `XMP `. The raw EXIF record
includes the AVIF item's stored TIFF-header offset prefix; no EXIF/XMP parsing,
orientation application, or implicit encode replay is performed. Direct and
supported grid-derived alpha `auxl` relationships, the bounded grid-derived
item list, bounded `iref` edges, and filtered `prem` relationships are retained in
`SourceDescriptor`; validated primary-grid payload topology is retained through
`SourceDescriptor::avif_grid_properties()`, while tile placement/composition,
track-only content, interpretation/replay of unknown or known raw item
properties, and auxiliary-item decoding remain outside this model.
The primary AVIF item's `colr`/`nclx` CICP declaration, `av1C` chroma sample
position, `clli` content-light-level property, `mdcv` mastering-display color
volume, and `colr`/`prof` or `rICC` ICC profile are retained in `SourceColor` on
`ImageInfo`, decoded still images, and still-sequence fallbacks: primaries,
chroma sample position, transfer characteristics, matrix coefficients, the
full-range flag, maxCLL, maxPALL, exact mastering-display coordinates/luminance
fields, and the exact ICC profile kind and bytes. They record source provenance
and never perform color conversion, chroma resampling, or tone mapping. These
fields are not part of the Pillow
parity matrix; the committed contract test is defensive/specification evidence
and uses a Pillow-generated encoded metadata output only as a source witness for
ICC.
AVIF `irot` and `imir`
properties are likewise retained in `SourceDescriptor`; their legal values are
validated, but no rotation or mirroring is applied. The primary item's `pasp`
declaration is retained in the same descriptor as positive horizontal and
vertical spacing values, and `clap` retains its positive width/height
fractions plus signed offsets. No pixel rescaling or cropping is applied.
Other non-primary/auxiliary profiles beyond raw ICC, track-only item semantics,
item color/property forms beyond typed CICP, raw ICC, raw unknown, the six known
raw declarations, plane, and codec declarations, grid tile
placement/composition, and broader derived/grid graph semantics remain outside
the current model; bounded direct,
supported grid-derived alpha
`auxl`, ordinary `iref`, `prem`, typed non-primary `colr`/`nclx` declarations,
the six known raw properties, plus non-primary `prof`/`rICC` profiles are the
explicitly retained exceptions.

Public enums whose vocabularies can grow with codec support are non-exhaustive.
This includes formats, verification strengths, transfer modes, disposal,
blend, frame layout, backgrounds, sequence kinds, source alpha, capabilities,
errors, limits, and encoder options. Downstream matches require a fallback;
internal dispatch matches stay exhaustive so each new variant forces a codec
review. `SourceByteOrder` remains exhaustive because its represented domain is
exactly little- or big-endian.

`ImageFormat::from_name` accepts the canonical format names and every
Pillow-recognized extension alias except headerless DIB: JPEG
`jpg`/`jpeg`/`jfif`/`jpe`, PNG `png`/`apng`, TIFF `tiff`/`tif`, ICO/CUR
`ico`/`cur`, and AVIF `avif`/`avifs`. `mime_type()`, `canonical_extension()`,
and `extensions()` return stable dependency-free metadata in canonical-first
order; `from_path` uses the same table without touching the filesystem.

## Canonical public surface

Codec modules and dispatchers are private. Callers use the root API so format
detection, feature availability, decoded-buffer validation, and error
translation cannot be bypassed.

| API | Contract |
| --- | --- |
| `detect_format(&[u8])` | Identify a supported signature without invoking a codec |
| `detect_prefix(&[u8])` | Incremental detection: complete signature, or `NeedMoreData { minimum }` for an incomplete prefix; terminal `UnknownFormat` otherwise |
| `inspect(&[u8])` | Read `ImageInfo` without materializing compressed pixels |
| `inspect_basic(&[u8])` | Read the same header facts without deep frame counting; `frame_count_complete` distinguishes known from unknown counts |
| `inspect_basic_prefix(&[u8])` | Incremental basic inspection: header facts when provable, `NeedMoreData { minimum }` while the basic header is incomplete |
| `decode(&[u8])` | Auto-detect and decode the still/first-image view |
| `decode_with_format(&[u8], ImageFormat)` | Validate the complete signature against a caller-selected format, then decode through the normal feature and codec dispatch |
| `decode_with_format_and_policy(&[u8], ImageFormat, &DecodePolicy)` | Apply the encoded-input limit, validate the selected signature, enforce the optional format allow-list, then apply metadata/canvas/frame/decoded-byte limits before decode |
| `decode_prefix`, `decode_prefix_with_policy` | Incremental still decode with the non-terminal `NeedMoreData { minimum }` status |
| `decode_with_token`, `decode_with_token_and_policy` | Still decode that polls a `CancellationToken` at structural checkpoints |
| `decode_sequence(&[u8])` | Auto-detect and retain every supported frame plus presentation metadata |
| `decode_sequence_prefix`, `decode_sequence_prefix_with_policy` | Incremental sequence decode with the same non-terminal status |
| `decode_sequence_with_token`, `decode_sequence_with_token_and_policy` | Sequence decode with per-frame/page cancellation |
| `CancellationToken` | Dependency-free cooperative cancellation: `Rc<Cell>` state shared by clones, `cancel()` fires every clone, single-threaded by design |
| `inspect_with_policy`, `decode_with_policy`, `decode_sequence_with_policy` | Apply one caller-selected format restriction and resource policy before the corresponding operation |
| `decode_into`, `decode_into_with_policy` | Decode into an exact-size caller-provided destination after rejecting short or oversized buffers without partial writes |
| `ImageInfo::decoded_bytes` | Preflight the exact transfer-byte length from inspection alone; zero-copy destination decode remains future work |
| `TransferLayout` | Minimal decoded byte contract: canvas, mode, row bytes, total bytes, packed-row status, and 1-byte alignment, produced by the same arithmetic as `decode_into` |
| `DecodedImage::try_new`, `try_with_mode`, `try_with_palette` | Validate dimensions, exact mode/color state, pixel length, and indexed palette state while reusing owned pixel buffers; unchecked constructors remain available for staged assembly |
| `encode(&DecodedImage, ImageFormat, &EncodeOptions)` | Validate and encode one image to an explicit target |
| `encode_with_policy`, `encode_sequence_with_policy` | Apply an inclusive complete-result cap and optional cooperative checkpoint budget, returning typed `EncodedOutputBytes` or `EncodeWorkUnits` failures |
| Uncapped `EncodePolicy` work-budget fast path | `Some(u64::MAX)` retains token-aware cancellation while reusing an uncapped token; finite work budgets retain their independent counter and typed limit semantics |
| `encode_with_token`, `encode_with_token_and_policy` | Still encode with cancellation before/after encoding; GIF now polls block/frame/coalescing/output-assembly, RGB/RGBA palette quantization, and LZW input-symbol checkpoints, WebP polls L1/P8/L8/La8/CMYK source-mode preparation and RGBA alpha/RGB extraction after each 1,024 source pixels, RGB-equal grayscale preparation after each 1,024 pixels, the remaining preparation stages, lossy VP8 RGB/RGBA-to-YUV conversion, RGBA transparent-area cleanup after each 1,024 scanned or flattened pixels, and RGBA alpha-palette source collection and index packing after each 1,024 source pixels, lossless VP8L RGB/RGBA source-pixel materialization, predictor source-snapshot copying and predictor mode-application wide source-row copies in completed 1,024-pixel chunks, image-palette construction and palette-mode index packing after each 1,024 source pixels, analysis histogram construction after each 64 completed 4×4 blocks, analysis/mode-selection/coefficient-probability/8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, 8,192-bit, 32,768-bit, 65,536-bit, 131,072-bit, and 262,144-bit logical and 16,384-boolean first-partition-bit/8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, 8,192-bit, 32,768-bit, 65,536-bit, 131,072-bit, 262,144-bit, 524,288-bit, 1,048,576-bit, and 2,097,152-bit logical and 16,384-boolean coefficient-bit/1,024-byte boolean-bitstream-output/bitstream stages, lossless VP8L predictor tile scans/mode application, cross-color multiplier search/transform tiles, entropy/transform stages, bounded backward-reference search/match-length/cache/trace, cost-manager interval-update and cleanup scans after each 256 cumulative interval entries, repeated-run hash-chain insertion, and copy-token cache-population scans after each 256 pixels, plus token/Huffman cost scans after each 1,024 tokens or 64 symbols, Huffman RLE preparation and in-run code-length scans after each 64 symbols, canonical-code assignment scans after each 64 symbols, Huffman-tree insertion scans after each 64 candidate nodes, Huffman-tree code-length-token frequency and trailing zero-repeat token trim scans after each 16 compressed token entries, Huffman code-length emission after each 16 compressed token entries, histogram clustering, Huffman-tree/group emission, 8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, 8,192-bit, 16,384-bit, 32,768-bit, 65,536-bit, 131,072-bit, 262,144-bit, 524,288-bit, and 1,048,576-bit and 2,097,152-bit logical bitstream intervals, 1,024-byte output, lossy VP8/ALPH RIFF payload-copy, lossless VP8L RIFF frame-copy, token-stream, codec-result, and metadata-assembly boundaries, PNG and BMP also poll row preparation, PNG stored-block boundaries, 1,024-byte stored-block-copy intervals, and every zlib-ng level's matcher/expansion/Huffman/bitstream/checksum stages, BMP row-conversion subsegments, and structural segments in return and sink paths, JPEG polls RGB-to-YCbCr conversion and chroma-downsample output after each 1,024 pixels, baseline entropy after each 1,024 MCUs, optimized baseline Huffman frequency gathering after each 1,024 AC coefficients, progressive scan block slots after each 1,024 blocks, progressive scan-event frequency items and progressive scan coefficient traversal items after each 1,024 events or coefficients, row/block/scan checkpoints, and 1,024-byte entropy-output intervals, and TIFF polls page preparation, predictor, raw/PackBits/LZW, Deflate input-row, level-six matcher candidate/insertion/fizzle/position, expansion, Huffman, bitstream, stored-block, and checksum boundaries |
| Lossless WebP VP8L 2,097,152-bit checkpoint | The token-aware bit writer polls at the 2,097,152 logical-bit interval; the existing no-token path remains unchanged. The exact whole-buffer/direct-sink boundary is covered by the Rust-only feature-gate contract because Pillow has no caller token, typed work-budget result, or caller-owned sink. |
| Lossless WebP VP8L predictor row-copy checkpoints | Token-aware predictor mode application copies each wide source row in completed 1,024-pixel chunks and polls after each completed chunk; the no-token path retains its original bulk row copy |
| Lossless WebP VP8L entropy-analysis pixel checkpoints | Token-aware entropy-mode pixel histogram analysis polls after each completed 1,024-pixel chunk on rows wider than 1,024 pixels; narrower rows remain bounded by existing row-start polls and the no-token traversal is direct |
| Lossless WebP VP8L Huffman-RLE fill checkpoints | Token-aware long-run marking and normalized-count fills poll after each 64 code-length values; the no-token helper retains its bulk fills, and the exact Rust-only boundary is covered by the existing feature-gate contract |
| Lossless WebP VP8L Huffman-RLE reverse-tail scan checkpoints | Token-aware Huffman-RLE preparation scans the fixed code-length alphabet backward toward its last nonzero slot and polls after each 64 scanned entries; the no-token path retains its original `rposition` fast path, and the exact Rust-only boundary is covered by the existing feature-gate contract |
| Lossless WebP VP8L Huffman-RLE token-materialization checkpoints | Token-aware code-length RLE expansion polls after each 16 emitted compressed tokens; the no-token helper retains its original bulk/token construction path, and the exact Rust-only boundary is covered by the existing feature-gate contract |
| WebP source-mode preparation checkpoints | Token-aware L1/P8/L8/L16/La8/CMYK expansion and RGBA alpha/RGB extraction poll after each 1,024 source pixels; L16 follows Pillow's I;16-to-RGB clamp-to-255 conversion; no-token maps and iterators retain their original tight paths and byte behavior |
| Lossless WebP VP8L backward-reference result-backfill checkpoints | Token-aware long result backfills poll after each 256 entries; the no-token path keeps its original tight loop |
| Lossless WebP VP8L backward-reference trace checkpoints | Token-aware backward-reference dynamic-programming trace, path reconstruction, and token replay poll after each 256 consumed pixels; the no-token path keeps its 1,024-pixel cadence through a const-specialized implementation |
| Lossless WebP VP8L token-stream checkpoints | Token-aware reference emission polls after each 256 consumed pixels, including every boundary crossed by one copy token; the no-token reference loop retains its original tight path |
| Lossless WebP VP8L hash-chain candidate-trial checkpoints | Token-aware backward-reference candidate selection polls after each 64 completed hash-chain trials across the pass; the no-token candidate loop retains its original tight path |
| Lossless WebP VP8L palette-mode box-chain candidate-trial checkpoints | Token-aware palette-mode box-chain selection polls after each 64 completed low-distance candidate offsets across the pass; the no-token box-chain loop retains its original tight path |
| Lossless WebP VP8L meta-histogram sampling checkpoints | Token-aware row/column comparisons and symbol compaction poll after each 1,024 symbols; no-token paths retain their original tight loops |
| Lossless WebP VP8L Huffman-node ordering checkpoints | Token-aware stable bottom-up ordering polls after each 64 comparisons; the no-token path retains the original stable sort |
| Lossless WebP VP8L Huffman run-scan checkpoints | Token-aware code-length run scans poll whenever each 64-symbol boundary is crossed, including before a long equal-length run finishes; the no-token path retains the original tight helper |
| Lossy WebP VP8 boolean-output flush checkpoints | Token-aware boolean flushes drain pending output runs through the existing 1,024-byte output accounting before returning; no-token encoding keeps the original flush helper |
| Lossy WebP alpha-stream buffer-copy checkpoints | Token-aware compressed and raw alpha streams copy in 1,024-byte chunks and poll after each complete chunk; the existing final stage check covers a short tail, while the no-token path keeps one bulk copy |
| Lossy WebP VP8/ALPH RIFF container-copy checkpoints | Token-aware native VP8 and extended ALPH/VP8 container payloads copy in 1,024-byte chunks; the no-token path retains one bulk copy |
| Lossless WebP VP8L candidate-trial suffix-copy checkpoints | Token-aware VP8L candidate selection copies the winning suffix in 1,024-byte chunks; the no-token path retains one bulk suffix copy |
| Lossless WebP VP8L RIFF frame-copy checkpoints | Token-aware native VP8L container assembly copies the complete frame payload in 1,024-byte chunks; the no-token path retains one bulk copy |
| WebP container/metadata assembly checkpoints | Token-aware sequence and metadata assembly copies caller-sized chunk/payload bytes in 1,024-byte intervals; the no-token path retains one bulk copy and structural sink delivery still owns its prefix semantics |
| `encode_default(&DecodedImage, ImageFormat)` | Encode one image with format defaults |
| Lossy WebP RGBA alpha-palette checkpoints | Token-aware source collection and index packing poll after each 1,024 source pixels; the no-token branch avoids token polling and retains its existing byte-preserving loop |
| Lossy WebP VP8 padded-plane checkpoints | Token-aware shared Y/U/V edge-replication polls after each 1,024 padded items when dimensions require padding; aligned planes take ownership without a clone, while the no-token path retains the original tight helper and byte behavior |
| Lossy WebP VP8 analysis histogram checkpoints | Token-aware histogram construction polls after each 64 completed 4×4 blocks; the no-token path retains the original tight transform loop |
| Lossy WebP VP8 segment-assignment checkpoints | Token-aware analysis segment assignment polls after each 1,024 macroblocks; the no-token path retains the original tight rewrite pass |
| Lossy WebP VP8 mode-selection checkpoints | Token-aware intra4 selection polls after each candidate-trial stage, each forward- and inverse-transform row/column subpass, each non-trellis quantization coefficient, each method-6 trellis-quantization coefficient candidate and path-reconstruction node, each squared-error pixel, each spectral-distortion weighted-transform row/column pass, each residual-cost coefficient, each candidate, and each completed luma 4×4 block, while the outer checkpoint remains after each 64 completed macroblocks for intra16/chroma and completed-decision work; the no-token path retains the original tight loop and each other individual stage remains one uninterruptible unit |
| Lossy WebP VP8 filter-edge adjustment checkpoints | Token-aware filter-edge adjustment polls after each 1,024 selected macroblocks; the no-token path retains the original tight adjustment pass |
| Lossy WebP VP8 coefficient-statistics checkpoints | Token-aware coefficient-statistics collection polls after each 1,024 selected macroblocks; the no-token path retains the original tight traversal |
| Lossy WebP VP8 segment-probability prepass checkpoints | Token-aware first-partition segment-probability collection polls after each 1,024 selected macroblocks; the no-token path retains the original tight count pass |
| `encode_sequence(&DecodedSequence, ImageFormat, &EncodeOptions)` | Encode one frame to any enabled format or multiple frames to GIF, TIFF, WebP, or native AVIF |
| `encode_sequence_with_token`, `encode_sequence_with_token_and_policy` | Sequence encode with frame/coalescing/page/finalization cancellation where the target exposes those checkpoints; still fallbacks retain the public boundary only |
| `encode_to_sink_with_policy`, `encode_sequence_to_sink_with_policy` | Apply the complete-result cap before an admitted buffer or structural segment reaches a caller-owned sink |
| `encode_to_sink`, `encode_sequence_to_sink` | Encode exact output into a caller-owned `OutputSink`; return APIs remain whole-buffer, while every current codec sink writer emits validated structural header/payload boundaries, then every path calls `OutputSink::flush` once |
| `encode_to_sink_with_token`, `encode_sequence_to_sink_with_token` | Token-aware sink encoding; structural writers can stop after an already-written prefix when cancellation fires |
| `ImageFormat::capabilities()` | Describe operation availability for one format in the current build |
| `all_capabilities()` | Return the same typed record for every public format in stable order |
| `EncodedImage::new(bytes)` | Snapshot encoded bytes, inspect immediately, defer decoding, and reuse the retained format for source-bound dispatch |
| `EncodedImage::decode_sequence`, `decode_sequence_with_policy` | Lazily retain the complete decoded sequence independently from the still cache; limited policies use the policy-aware selected-format uncached path |
| `EncodedImage::decode_state`, `sequence_decode_state` | Report separate not-attempted, succeeded, and failed lazy-cache states for still and sequence materialization |
| `EncodedImageView::new(&[u8])` | Borrow encoded bytes for the same operations without copying into an owned snapshot; no cache, so decodes reparse |
| `EncodedImage::decode_frame(index)` | Return the exact frame at an index; TIFF decodes only that page's IFD, other sequence formats currently use an eager fallback that matches `decode_sequence` |

The WebP VP8L token-aware list includes entropy-mode histogram-cost scans after
each 64 symbols, cost-manager interval-update and cleanup scans after each 256
cumulative interval entries, repeated-run hash-chain insertion and long
backward-reference result backfills after each 256 entries, hash-chain
candidate selection after each 64 completed trials, palette-mode box-chain
candidate offsets after each 64 completed offsets, and copy-token
cache-population checkpoints after each 256 pixels, Huffman
code-length emission after each 16 compressed token entries, Huffman-tree
simple-tree symbol-discovery checkpoints
after each 64 code-length slots, code-length-token frequency, and trailing
zero-repeat token trim checkpoints after each 16 compressed token entries; Huffman
RLE code-length run scans poll whenever a 64-symbol boundary is crossed inside
 a long equal-length run, while long-run materialization polls after each 16
 emitted compressed code-length tokens; the
histogram-clustering populated-tile collection, min/max, and bin-assignment
pre-passes also checkpoint after each 64 tile histograms; the
meta-histogram sampling row/column comparisons and symbol compaction also
checkpoint after each 1,024 symbols; the
token-aware Huffman-node ordering path uses a stable bottom-up merge sort and
polls after each 64 comparisons, while the no-token path retains its original
stable sort; the
no-token paths retain their original tight loops. Candidate trials leave the
already-emitted prefix in the parent writer and retain only each trial's
suffix, while recycling losing or replaced winning suffix allocations as
scratch. This avoids redundant prefix clone/re-copy work and fresh per-trial
suffix allocations without changing the selected bitstream or adding a new
public work-budget result; the token-aware winner suffix is copied in 1,024-byte
intervals, while the no-token winner copy remains bulk.

Lossless VP8L palette discovery has the same bounded-mode invariant. The
ordinary no-token scan returns a sorted 257-entry sentinel as soon as it sees
the 257th distinct ARGB color, because palette mode is only eligible through
256 entries and the full unique-color set is otherwise dead state. Inputs that
remain within the palette limit keep their exact sorted values. The token-aware
scan deliberately retains its complete ordered drain so its established
caller-budget checkpoints remain observable. This is an internal runtime and
allocation boundary, not Pillow-oracle coverage or a public palette contract.

WebP ALPH assembly also separates ordinary result selection from the
caller-controlled path. The no-token encoder compares the already-known raw
alpha length with the encoded VP8L payload and reuses that allocation for
either candidate: a raw winner clears it, writes the uncompressed header, and
copies the raw plane into place, while a compressed winner inserts the
one-byte ALPH header in place. The token-aware path retains its separate
compressed and raw copies and 1,024-byte copy checkpoints so its existing
Rust-only work-budget/sink contract does not change. Pillow parity can
regression-check the resulting bytes and errors, but exposes neither this
allocation ownership nor the caller-token behavior.

The ordinary no-token WebP still encoder then reuses the completed VP8L frame
allocation for the final RIFF result. It reserves space for the 20-byte
RIFF/WEBP/VP8L prefix and optional pad byte, shifts the payload in place, and
writes the container fields without allocating a second output vector. The
token-aware path retains its separate output buffer and chunk-copy checkpoints
so caller-controlled work and sink behavior do not change. This is an internal
allocation boundary: Pillow parity verifies the resulting bytes and errors,
while Rust-only feature-gated evidence owns allocation and token semantics.

Within each VP8L token-stream candidate trial, the ordinary no-token winner
suffix is now appended directly to the parent writer after capacity is
reserved, then its empty vector is returned to the reusable output scratch.
The token-aware path still copies the suffix in checkpointed chunks so
cancellation and caller-sink behavior remain unchanged. This removes an
internal result-copy boundary; it does not add a Pillow-observable contract.

`detect_format` recognizes all eight container signatures even when a codec
feature is disabled. An operation that requires a disabled codec returns
`ImageError::FeatureDisabled`. For AVIF, `avif` and `avis` major brands are
direct signatures; generic `mif1`/`msf1` majors additionally require an
`avif` or `avis` compatible brand in the complete bounded `ftyp` box.

The incremental surface (`detect_prefix`, `inspect_basic_prefix`) shares the
same parsers as the complete-slice APIs but exposes short reads as the
non-terminal `ImageError::NeedMoreData { minimum }` status. `minimum` is the
total input length the next parse needs before it can either succeed or fail
terminally: exact for fixed signatures, and progress-aware for containers
whose structure declares its own extent (WebP chunk payloads and AVIF box
payloads, where a truncated declared box reports its declared end). Codec
read helpers classify "input ends before the requested slice" separately from
"declared structure is inconsistent"; the latter stays terminal `Malformed`.
Slices already bounded by a validated declared length (ICO entries wrapping a
PNG, ANMF sub-chunks, nested AVIF boxes) also stay terminal, because appending
more file bytes cannot repair them. Legacy complete-slice APIs are unchanged:
they map every internal truncation back to `Malformed` with the same message,
so manifest parity and error stages are preserved.

`decode_with_format` is the complete-slice entry point for callers that know a
candidate format out of band. It checks the encoded-input limit first, then
requires `detect_format` to return the same format before any policy metadata
preflight or codec dispatch. A recognized different signature is a staged
`Parameter` error; an incomplete or otherwise unknown complete-slice signature
is a staged `Malformed` error. The policy-aware variant keeps the same ordering
and limits, and neither explicit-format API bypasses feature availability or
payload validation. Partial input remains the `decode_prefix` contract.

Decoding uses the same classification. `decode_prefix` and
`decode_sequence_prefix` run the identical codec paths as `decode` and
`decode_sequence` but expose internal truncation as `NeedMoreData`: exact
minimums when the container declares the missing extent (PNG chunks, BMP/ICO
pixel spans, TIFF strip/tile spans, WebP RIFF payloads, AVIF boxes) and
progress-aware minimums otherwise (JPEG marker/scan reads, GIF sub-blocks,
WebP native reads). Compressed payloads bounded by a complete declared
structure (GIF LZW sub-block streams, TIFF strip payloads, JPEG entropy
zero-padding) keep their terminal classification because appending file bytes
cannot repair them. Policy limits are re-evaluated on every retry against the
current input length.

Cooperative cancellation polls a caller token at structural and selected
codec-internal checkpoints: dispatch entry, PNG chunk boundaries and
1,024-byte adaptive-filter/filtered-row subsegments, BMP row-conversion
subsegments after each 1,024 pixels, GIF block and frame boundaries plus RGB/RGBA
palette quantization intervals and LZW input-symbol intervals, TIFF
still page preparation/predictor/PackBits/LZW work, Deflate level-six matcher
intervals, plus sequence page and
strip/tile boundaries, JPEG color/sampling/quantization rows, progressive scan
block slots after each 1,024 blocks, progressive scan-event frequency items
after each 1,024 events, progressive scan coefficient items after each 1,024
coefficients, and entropy/progressive scan batches, WebP
frame boundaries, BMP RLE commands,
ICO directory entries, and AVIF frames. A fired token returns
`ImageError::Cancelled` with the format and operation stage and never publishes
partial results; codec state is per-call, so a fresh token can retry the same
input. The token is `Rc<Cell>` based and neither `Send` nor `Sync`, matching
the single-threaded execution model.

Capability discovery mirrors this dispatch without parsing input.
`Capability::ManifestBounded` means the operation can be attempted within the
fixture-defined codec contract; `Restricted` names a narrower target subset;
and `Unavailable` distinguishes a disabled feature, unavailable target, or
unimplemented operation. Detection remains manifest-bounded for every format.
Sequence capability means genuine multi-image retention or emission, not the
common one-frame fallback. Enabled native PNG reports sequence decode only;
GIF, TIFF, WebP, and AVIF report sequence decode and encode; JPEG, BMP, and ICO
report neither. Enabled `wasm32` AVIF reports restricted portable still decode
and target-unavailable encode and sequence operations.

Every `EncodeOptions` value contains exactly one codec-specific record and
reports that target through `EncodeOptions::format()`. The explicit
`ImageFormat` argument remains canonical; dispatch rejects a mismatched option
target before entering a codec. `EncodeOptions::for_format()` creates
target-specific defaults, while the strict legacy-pair adapter exists only for
migration and is never consulted by an encoder.

`DecodePolicy::default()` is the unlimited compatibility policy used by the
short entry points. `DecodeFormatSet` is an optional caller-selected
allow-list: the absent value accepts every detected format, while an explicit
empty set accepts none. Policy-aware inspection and decode paths enforce the
allow-list after signature detection, including prefix, token, explicit-format,
and source-bound operations. `detect_format` remains independent so callers
can inspect a signature without applying a decode policy. A denied format is a
typed `Unsupported` result with `UnsupportedReason::PolicyDenied` and the
operation stage. `max_encoded_bytes` is inclusive and checked against the
complete byte slice before signature detection. This ordering bounds AVIF
compatible-brand scanning as well as every codec parser, but intentionally
leaves `ImageError::format()` as `None` on rejection.

`decode_with_format_and_policy` uses the same first check, then validates the
caller-selected format against the complete signature before any metadata or
canvas preflight. A signature mismatch is `Parameter`; no complete supported
signature is `Malformed`; and a matching signature proceeds through the same
feature, inspection, limit, and payload-validation path as auto-detecting
decode.

`max_width`, `max_height`, `max_pixels`, and
`max_primary_decoded_bytes` are inclusive limits on the exact inspected
`ImageInfo` canvas. The byte limit uses the primary mode's transfer layout,
including byte-aligned packed `L1` rows. They run after header inspection and
before primary pixel decode, so their `LimitExceeded` errors retain the
selected format. The error also carries the exact `CodecOperation`,
`ResourceLimit`, configured maximum, and observed value. A policy-aware direct
decode performs an inspection preflight before the codec's decode parse;
unlimited wrappers avoid this extra pass. These are primary-canvas limits, not
bounds on later TIFF pages or animation frames, source rectangles, cumulative
sequence memory, metadata, codec work, other allocations, or transient encoded
output allocation. `EncodePolicy` is the separate encode-side result policy.

`max_frames` is an inclusive limit on the exact inspected frame/page count.
Inspection, sequence decode, and immutable-source construction reject a source
whose declared count exceeds the maximum before sequence pixel work begins.
Still decode and lazy still materialization retain exactly one frame, so only
a zero maximum rejects them. The check runs after the encoded-input and
primary-canvas checks and retains the selected format. GIF and TIFF chains
whose inspection cannot prove a complete count report `frame_count: None` and
remain unlimited for this resource; the inspection-completeness model governs
that boundary rather than this limit.

`max_frame_decoded_bytes` and `max_sequence_decoded_bytes` apply to sequence
materialization only. The per-frame limit rejects any later frame/page whose
transfer-byte length exceeds the maximum before that frame's pixel work; the
cumulative limit charges the inspected primary frame first, then rejects
before the frame whose addition would exceed the total. GIF/PNG/WebP/TIFF/AVIF
sequence decoders receive a crate-internal `SequenceDecodeBudget` and reserve
each later frame before its allocation; the structured `LimitExceeded` value
is preserved verbatim through `CodecError::LimitExceeded`. These limits do not
bind still decode, immutable-source construction, or lazy still
materialization, which remain governed by the primary-canvas and frame-count
checks.

`max_metadata_bytes` bounds the encoded metadata extent: every encoded byte
that is not primary pixel payload data, measured by a per-format container
scan (PNG chunk scan minus `IDAT`/`fdAT` data, GIF block scan minus image
sub-block payloads, JPEG marker scan minus entropy spans, WebP RIFF scan minus
top-level `VP8 `/`VP8L`/`ALPH` payloads, TIFF IFD walk minus strip/tile payload
bytes, BMP bytes before the declared pixel offset, ICO header plus directory,
and AVIF box/item scan minus sample spans referenced by the primary or
auxiliary planes, including metadata item payloads stored in `mdat`). The scan runs after detection and
before any inspection preflight on all five policy paths, so an oversized
metadata extent is rejected before codec work begins; malformed containers
propagate their structured codec error from the scan.

`EncodePolicy::max_output_bytes` is an inclusive admission check on the
complete encoded length. Still and sequence encodes apply it before return;
the policy-aware sink wrappers apply it before the first sink write. An
oversized result returns `ImageError::LimitExceeded` with
`ResourceLimit::EncodedOutputBytes`, and the sink remains untouched. The
The whole-buffer codecs construct the complete buffer before this check. Every
current still sink path—JPEG, PNG, GIF, BMP, ICO, TIFF, WebP, and native AVIF—
and the supported one-frame or multi-frame sequence sink paths instead
preflight their complete lengths before emitting validated structures. ICO
still delivery splits a fixed 22-byte directory header from its complete
embedded PNG/DIB payload. TIFF delivery splits its header, strip/padding span,
and IFD/value tail. The other writers use their corresponding marker, chunk,
block, RIFF, or ISO-BMFF boundaries. PNG prepares filtered rows and compressed
payload, BMP prepares bounded palette/row segments, and every codec may retain
complete working state until a validated segment is ready. These are
structural-delivery boundaries, not transient-allocation or recoverable-OOM
guarantees.

BMP-backed ICO entries assemble converted BGR/BGRA rows directly into their
pre-sized DIB buffer, avoiding a separate converted-pixel staging buffer. The
directory still owns the complete embedded payload because its fixed header
must precede that payload; this slice removes only the inner pixel staging copy
and does not claim complete allocator accounting, recoverable-OOM handling, or
streaming.

The shared PNG/TIFF zlib-ng compressor seeds its bit writer with the zlib
header and returns that owned buffer directly, then appends the Adler-32
trailer. This removes the separate full bitstream-to-output copy in both
ordinary and token-aware paths without changing encoded bytes or checkpoint
counts. It is a bounded transient-allocation optimization, not complete
allocator accounting, recoverable-OOM handling, or a streaming guarantee.

WebP animation has separate ownership paths. The ordinary no-token path writes
RIFF/VP8X/ANIM first, appends each completed frame's nested VP8/VP8L chunks
directly into the final RIFF buffer, releases that frame buffer, and patches
the VP8X alpha flag after the last frame. The token-aware path retains
completed frame buffers until the canvas alpha flag is known, then writes the
fixed ANMF prefix and nested chunks directly into the final RIFF buffer while
preserving its existing cancellation checkpoints. Both paths remove the
temporary copied chunk and ANMF-payload staging allocations and preserve
encoded bytes. This is a bounded ownership optimization, not allocator/OOM
accounting or universal streaming.

Lossless WebP VP8L backward-reference cost management reuses its interval state
in place. Interval updates no longer allocate an applicable-interval scratch
vector, cleanup compacts the existing interval vector, length-interval tables
are borrowed instead of cloned for each push, and interval split/rebuild work
reuses bounded manager scratch vectors. Cost-model population histograms are
transformed in their existing vectors, so fixed-alphabet cost arrays do not
allocate temporary conversion vectors, sequential candidate cost estimates
reuse their bounded green histogram scratch, and cache-bit token transforms
reuse bounded cache-table scratch. The trace replay reuses its completed
dynamic-programming cache. Cost decisions, checkpoint ordering,
encoded bytes, and sink output remain unchanged. This is a bounded
internal-allocation optimization, not allocator/OOM accounting, recoverable-OOM
handling, or a streaming guarantee.

WebP VP8L candidate trials retain their `GroupCodes` objects in bounded scratch.
Each group's five channel length/code arrays resize and reset in place, then
remain live until all token references for that trial have been emitted. This
removes repeated per-group buffer allocation without changing histogram
ownership, token-reference lookup, checkpoint ordering, encoded bytes, errors,
or sink output. It is a Rust-only allocation optimization, not allocator/OOM
accounting, recoverable-OOM handling, or a streaming guarantee.

WebP VP8L Huffman tree writing reuses one compressed code-length token buffer
across the sequential channel and histogram-group trees. The buffer is cleared
and refilled only after the prior tree has finished consuming it, preserving
token-aware checkpoints, the no-token path, encoded bytes, errors, and sink
output. This is a Rust-only allocation optimization, not allocator/OOM
accounting, recoverable-OOM handling, or a streaming guarantee.

WebP VP8L Huffman tree construction reuses one optimized-frequency buffer
across sequential trees. It copies each frequency slice into that retained
storage before the existing RLE optimization, then leaves all tree ownership
and ordering unchanged. The no-token path remains free of optional polling;
checkpoint sites, encoded bytes, errors, and sink output are unchanged. This is
a Rust-only allocation optimization, not allocator/OOM accounting,
recoverable-OOM handling, or a streaming guarantee.

WebP VP8L Huffman tree writing stores its at-most-three simple-tree symbols in
a fixed array instead of a heap vector. Token-aware and no-token scans retain
the previous early-stop behavior and checkpoint schedule; tree selection,
encoded bytes, errors, and sink output are unchanged. This is a Rust-only
allocation optimization, not allocator/OOM accounting, recoverable-OOM
handling, or a streaming guarantee.

WebP VP8L Huffman decoding stores simple two-symbol trees in an inline
`TwoNode` representation instead of allocating the fixed three-node tree and
two-entry lookup table used by the general representation. Low-bit selection,
one-bit consumption, `peek_symbol` results, secondary-symbol acceptance, and
short-read or malformed-stream errors remain unchanged. This is a Rust-only
decoder allocation/layout optimization: Pillow parity verifies the resulting
bytes and errors, while the representation and ownership behavior belong to
the separate Rust coverage and feature-gate evidence; it is not allocator/OOM
accounting, recoverable-OOM handling, or a streaming guarantee.

WebP VP8L Huffman code-length decoding keeps its fixed 19-entry code-length
alphabet on the stack and borrows it through the Huffman builder instead of
allocating a temporary heap vector. Ordinary and color-cache-enlarged decoded
code-length buffers now share one fixed `[u16; 2_328]` stack workspace: the
format bounds this to 280 ordinary green symbols plus the 2,048 symbols from
the 11-bit color-cache field. The active slice is borrowed only while
`build_implicit` copies its values into the owned tree, and the workspace is
zeroed before each sequential non-simple tree. Code ordering, bit consumption,
encoded bytes, errors, and sink output remain unchanged. This is a Rust-only
fixed-workspace optimization: Pillow parity verifies final bytes and errors,
while storage ownership belongs to the separate Rust coverage and feature-gate
evidence; it is not allocator/OOM accounting, recoverable-OOM handling, or a
streaming guarantee.

The VP8L color-cache table remains dynamically sized to the actual
`1 << color_cache_bits` entries. Although the format bounds it at 2,048
entries (8 KiB), forcing the maximum table inline would enlarge every
cache-bearing `HuffmanInfo`. A dirty local probe did not establish a benefit,
and its direct-child peak includes build/cache effects, so it is not accepted
as a performance claim. Peak-stack and allocator measurements are not yet part
of the release contract; that boundary therefore remains open and is not
represented as a completed optimization.

WebP still VP8L decode now writes directly into a caller-owned RGB buffer for
opaque, untransformed frames, avoiding a full RGBA staging vector and the
follow-up RGB copy. The header's alpha-used bit and transform list gate this
path; alpha-bearing or transformed frames retain the four-byte internal
representation required by color-cache and inverse-transform semantics. This
is Rust-only transient-storage evidence; Pillow parity verifies only final
bytes/errors, not the staging ownership, and it is not allocator/OOM or
streaming evidence.

Opaque animated WebP VP8L frames follow the enclosing `VP8X` alpha/output
contract. When that contract is RGB, `read_frame` uses the existing direct RGB
VP8L decoder and passes a three-byte frame to the compositor; alpha-bearing
frames retain RGBA. The compositor already writes opaque RGB frames into its
RGBA canvas with alpha 255, so animation geometry, blending, disposal, output
bytes, and errors are unchanged. This is Rust-only transient-storage evidence:
Pillow parity observes final bytes and errors only, not staging ownership or
allocation counts; lossless `ALPH` remains on the full ARGB path because its
green channel can carry transforms and color-cache/backward-reference state.

WebP VP8L palette-mode packing writes each packed pixel into the prefix of the
existing mutable source-pixel buffer instead of allocating a second
image-scaled `Vec<u32>`. The encoder no longer needs the source pixels after
this branch, and left-to-right overlap is safe because every destination index
is at or before the source group being read; single-pixel groups are read
before their same slot is replaced. Palette lookup order, partial-group
packing, checkpoint cadence, encoded bytes, errors, and sink output remain
unchanged. This is Rust-only transient-storage evidence: Pillow parity checks
final bytes and errors, not allocation ownership or counts, and it is not
allocator/OOM or streaming evidence.

WebP VP8L color-indexing transform tables are bounded to 256 RGBA entries and
are retained in decoder-owned `[u8; 1024]` storage. The transform record keeps
only the table size; inverse application slices the fixed table after the main
image stream is decoded. The table must survive until reverse transform order
runs, so it cannot be reused from the decoded image buffer. This removes the
color-map heap allocation while preserving map adjustment, lookup order,
decoded bytes, errors, and sink output. Pillow parity verifies only final bytes
and errors, not this ownership boundary or allocation counts, so the result is
Rust-only feature-gate and coverage evidence rather than a Pillow-parity
claim; it is not allocator/OOM accounting, recoverable-OOM handling, or a
streaming guarantee.

WebP ALPH palette collection uses the same fixed 256-value alphabet as the
palette-index and delta workspaces. `collect_alpha_palette` therefore returns a
fixed `[u8; 256]` plus length instead of allocating a palette `Vec<u8>`. The
raw alpha plane remains borrowed for the compressed-versus-uncompressed choice,
and the image-scaled packed `Vec<u32>` remains necessary because the entropy
writer needs random-access packed pixels while the raw candidate stays
available. Pillow parity verifies final bytes and errors only; this bounded
workspace boundary is Rust-only feature-gate and coverage evidence, not
allocator/OOM accounting, recoverable-OOM handling, or a streaming guarantee.

WebP VP8L transform-order decoding stores the at-most-four transform IDs in
`[u8; 4]` plus a length instead of a heap `Vec`. The bitstream permits at most
one instance of each of the four transform types, so the fixed capacity is
complete; duplicate-transform rejection, reverse application order, encoded
bytes, errors, and sink output remain unchanged. This is a Rust-only
fixed-workspace optimization: Pillow parity verifies final bytes and errors,
while storage ownership belongs to the separate Rust coverage and feature-gate
evidence; it is not allocator/OOM accounting, recoverable-OOM handling, or a
streaming guarantee.

WebP VP8L Huffman decoding maps a valid complete canonical code-length form
with exactly two one-bit symbols to the same inline `TwoNode` representation
used by the simple bitstream form. This avoids the general heap table/tree for
that bounded case while preserving canonical symbol order, bit consumption,
decoded values, errors, and sink output. Pillow parity verifies only final
bytes and errors; it cannot observe which internal tree representation a
bitstream selected, so this remains Rust-only coverage and feature-gate
evidence rather than a Pillow-parity claim.

WebP VP8L Huffman decoding stores general-tree branch offsets as checked `u32`
values instead of `usize`. The VP8L alphabet bounds keep the constructed arena
below the 32-bit range, so the narrower field preserves the tree topology while
reducing node storage; conversion failures remain defensive Rust-only errors.
Pillow parity verifies only final bytes and errors, not this internal layout,
so this is Rust-only storage evidence backed by feature-gate and coverage
records; it is not allocator/OOM accounting, recoverable-OOM handling, or a
streaming guarantee.

WebP VP8L Huffman decoding packs each general-tree slot into one tagged `u32`:
zero is empty, `symbol + 1` is a leaf, and the high-bit tag plus a validated
31-bit value is a child offset. This halves the node word width while preserving
tree topology, canonical symbol order, decoded values, errors, and sink output;
the validated VP8L alphabet bounds keep every offset below the tag bit. Pillow
parity verifies only final bytes and errors, not this internal representation,
so this is Rust-only storage evidence backed by feature-gate and coverage
records; it is not allocator/OOM accounting, recoverable-OOM handling, or a
streaming guarantee.

General VP8L Huffman trees now co-locate their primary lookup table and packed
secondary nodes in one `Vec<u32>` allocation. Primary entries continue to
address secondary nodes relative to the table boundary, so canonical tree
topology, symbol ordering, bit consumption, decoded values, errors, and sink
output remain unchanged. Pillow parity verifies only final bytes and errors;
the one-allocation ownership boundary is Rust-only feature-gate and coverage
evidence, not allocator/OOM accounting, recoverable-OOM handling, or a
streaming guarantee.

The VP8L Huffman group vector reserves its metadata-derived bounded length
before the group trees are parsed, avoiding repeated geometric growth while
the group order and owned tree lifetimes remain unchanged. This is Rust-only
workspace-planning evidence; Pillow parity verifies only final bytes and
errors, and allocation counts or recoverable-OOM behavior are not claimed.

The VP8L lossless decoder materializes its sampled meta-Huffman image in one
`Vec<u16>` allocation, decodes through a byte view, and compacts the retained
first two source bytes of each pixel in place before returning the group-index
image. This removes the transient byte buffer and second typed allocation while
preserving source-byte interpretation, group selection, decoded bytes, errors,
and sink output. Pillow parity covers only those observable bytes and errors;
the allocation boundary is Rust-only feature-gate and coverage evidence, not
allocator/OOM accounting, recoverable-OOM handling, or a streaming guarantee.

The VP8 decoder's size-unknown final arithmetic partition is read through a
bounded 16 KiB stack buffer and appended as `[u8; 4]` words, preserving the
short-final-word padding and logical byte count without a transient heap byte
buffer. Pillow parity covers the resulting bytes and errors only; the
transient-storage boundary is Rust-only feature-gate and coverage evidence, not
allocator/OOM accounting, recoverable-OOM handling, or a streaming guarantee.

The VP8 frame header's two-bit partition count bounds the three-byte partition
size table at 21 bytes, so the decoder keeps that table on the stack while
retaining the same partition sizes and arithmetic-decoder inputs. Pillow parity
covers only the resulting bytes and errors; this bounded-workspace boundary is
Rust-only feature-gate and coverage evidence, not allocator/OOM accounting,
recoverable-OOM handling, or a streaming guarantee.

WebP VP8L Huffman decoding stores valid complete canonical forms with a
sixteen-entry primary table in an inline `InlineTable16` representation. The
maximum code length is four, so canonical completeness proves that no
secondary nodes are needed; larger forms retain the general table/tree.
Primary-table lookup, symbol ordering, bit consumption, decoded values, errors,
and sink output remain unchanged. Pillow parity verifies only final bytes and
errors, not the internal representation, so this is Rust-only storage/layout
evidence backed by the feature-gate and coverage records; it is not
allocator/OOM accounting, recoverable-OOM handling, or a streaming guarantee.

WebP VP8L Huffman decoding stores valid complete canonical forms with an
eight-entry primary table in an inline `InlineTable8` representation. The
maximum code length is three, so canonical completeness proves that no
secondary nodes are needed; larger forms retain the general table/tree.
Primary-table lookup, symbol ordering, bit consumption, decoded values, errors,
and sink output remain unchanged. Pillow parity verifies only final bytes and
errors, not the internal representation, so this is Rust-only storage/layout
evidence backed by the feature-gate and coverage records; it is not
allocator/OOM accounting, recoverable-OOM handling, or a streaming guarantee.

WebP VP8L Huffman decoding stores valid complete canonical forms with a
four-entry primary table in an inline `InlineTable4` representation. The
maximum code length is two, so canonical completeness proves that no secondary
nodes are needed; larger forms retain the general table/tree. Primary-table
lookup, symbol ordering, bit consumption, decoded values, errors, and sink
output remain unchanged. Pillow parity verifies only final bytes and errors,
not the internal representation, so this is Rust-only storage/layout evidence
backed by the feature-gate and coverage records; it is not allocator/OOM
accounting, recoverable-OOM handling, or a streaming guarantee.

WebP VP8L Huffman-RLE optimization reuses one boolean good-mask buffer across
sequential channel and histogram-group tree builds. The mask is resized and
cleared before each RLE pass, so token-aware and no-token RLE decisions remain
unchanged while the previous per-tree mask allocation is removed. This is a
Rust-only allocation optimization, not allocator/OOM accounting,
recoverable-OOM handling, or a streaming guarantee.

WebP VP8L multi-group token streams reuse one meta-pixel materialization buffer
across candidate trials. The buffer is cleared and refilled after histogram
sampling, then consumed completely by the recursive meta-stream write before
the next candidate uses it; metadata grouping, encoded bytes, errors, and sink
output remain unchanged. This is a Rust-only allocation optimization, not
allocator/OOM accounting, recoverable-OOM handling, or a streaming guarantee.

WebP VP8L Huffman construction retains the leaf-node vector, token-aware
merge-sort buffer, and compact branch arena across sequential tree builds.
Weighted node indices are copied during stable ordering, so merged subtrees do
not allocate boxed children or deep-clone during sort; the traversal stack
remains local. Ordering, tree selection, checkpoint behavior, encoded bytes,
errors, and sink output remain unchanged. This is a Rust-only bounded
allocation optimization, not allocator/OOM accounting, recoverable-OOM handling,
or a streaming guarantee.

WebP VP8L multi-group token streams retain an optional child `TokenStreamScratch`
for the sampled metadata image, so its bounded group and Huffman buffers survive
the outer candidate loop. The metadata stream disables further recursion; the
candidate suffix remains independently selected and the parent writer's prefix,
checkpoint ordering, encoded bytes, errors, and sink output remain unchanged.
This is a Rust-only nested-scratch allocation optimization, not allocator/OOM
accounting, recoverable-OOM handling, or a streaming guarantee.

The nested metadata image's candidate output buffer is also retained by the
parent scratch boundary. Losing suffixes remain reusable trial storage, and
the winning suffix's capacity is returned after its bytes are delivered to the
parent writer. Candidate selection, checkpoint behavior, encoded bytes, errors,
and sink output remain unchanged. This is a Rust-only output-scratch allocation
optimization, not allocator/OOM accounting, recoverable-OOM handling, or a
streaming guarantee.

WebP VP8L cache-bit candidate transforms retain a reusable transformed-token
buffer alongside the existing color-cache table. Each trial swaps its output
with the current best candidate, returning the replaced vector to scratch;
only the selected token vector remains independently owned. Cache-bit ordering,
checkpoint behavior, encoded bytes, errors, and sink output remain unchanged.
This is a Rust-only candidate-buffer allocation optimization, not
allocator/OOM accounting, recoverable-OOM handling, or a streaming guarantee.

WebP VP8L trace-back candidate improvement retains the dynamic-programming
cache, path-length reconstruction buffer, and transformed-token output buffer
across sequential trace attempts. A selected trace keeps its token vector
independently owned; a rejected trace or replaced candidate returns its vector
to scratch. Trace ordering, checkpoint behavior, encoded bytes, errors, and sink
output remain unchanged. This is a Rust-only trace-scratch allocation
optimization, not allocator/OOM accounting, recoverable-OOM handling, or a
streaming guarantee.

WebP VP8L trace setup retains the CostManager pixel-cost and path-length tables,
match-length cost and equal-cost interval tables, active interval state, and
interval split/rebuild scratch across sequential trace attempts. Each attempt
resets candidate-specific values and preserves the token-aware initialization
checkpoints; the no-token path remains tight. Trace ordering, checkpoint
behavior, encoded bytes, errors, and sink output remain unchanged. This is a
Rust-only CostManager allocation optimization, not allocator/OOM accounting,
recoverable-OOM handling, or a streaming guarantee.

WebP VP8L trace cost-model construction retains and resets the green histogram
and fixed channel/distance histograms across sequential trace attempts. The
population-cost transformation still runs in place with the same token-aware
checkpoints, and the no-token path remains direct. Trace ordering, checkpoint
behavior, encoded bytes, errors, and sink output remain unchanged. This is a
Rust-only CostModel histogram allocation optimization, not allocator/OOM
accounting, recoverable-OOM handling, or a streaming guarantee.

WebP VP8L candidate construction reuses one source-token buffer across the
sequential LZ77, RLE, and optional low-distance box-chain candidates. Cache-bit
selection still reads each source independently, and selected candidate vectors
remain independently owned. Candidate ordering, checkpoint behavior, encoded
bytes, errors, and sink output remain unchanged. This is a Rust-only
candidate-source allocation optimization, not allocator/OOM accounting,
recoverable-OOM handling, or a streaming guarantee.

WebP VP8L's optional low-distance box-chain pass repopulates the existing
primary hash-chain storage in place after the primary candidate has been
consumed, avoiding a second pixel-sized `(distance, length)` result vector.
The box-chain search, candidate ordering, checkpoint behavior, encoded bytes,
errors, and sink output remain unchanged. This is a Rust-only box-chain storage
optimization, not allocator/OOM accounting, recoverable-OOM handling, or a
streaming guarantee.

WebP VP8L Huffman depth traversal uses a bounded fixed stack sized for the
largest VP8L alphabet instead of allocating a temporary heap vector for each
tree. Tree shape, code lengths, checkpoint behavior, encoded bytes, errors, and
sink output remain unchanged. This is a Rust-only Huffman traversal storage
optimization, not allocator/OOM accounting, recoverable-OOM handling, or a
streaming guarantee.

WebP VP8L hash-chain construction uses the final distance/length result table
as temporary predecessor-link storage during descending best-match
materialization. Each link points to an earlier position, so overwriting a
finalized entry cannot affect later traversal; the result table, candidate
ordering, checkpoint behavior, encoded bytes, errors, and sink output remain
unchanged. This is a Rust-only hash-chain storage optimization, not
allocator/OOM accounting, recoverable-OOM handling, or a streaming guarantee.

WebP VP8L frame, palette, and lossy ALPH substreams share one bounded
image-stream scratch object per encoder invocation. Its trial-output and token-stream
buffers retain capacity across sequential streams, including nested metadata
streams, while each stream still resets its logical contents before writing.
For WebP animation, one encoder invocation spans the sequential frames, so
transform, histogram, token, predictor, cross-color, and bitstream scratch
capacity also survives frame boundaries for both lossless VP8L and lossy ALPH;
every returned frame retains an independent encoded buffer. Stream and frame
boundaries, candidate ordering, encoded bytes, errors, cancellation
checkpoints, and sink output remain unchanged. This is a Rust-only image-stream
scratch optimization, not allocator/OOM accounting, recoverable-OOM handling,
or a streaming guarantee.

The lossy ALPH packed transform image also reuses its `Vec<u32>` capacity across
animation frames; its logical contents are cleared and rebuilt before each
stream. This is a Rust-only workspace optimization with the same byte, error,
checkpoint, and sink-output contract, not allocator/OOM accounting,
recoverable-OOM handling, or a streaming guarantee.

Lossy RGBA WebP also retains the extracted alpha-channel buffer capacity in the
encoder across sequential frames, clearing its logical contents after each
ALPH attempt. The extraction cadence and encoded bytes, errors, checkpoints,
and sink output remain unchanged. This is a Rust-only staging optimization;
Pillow has no allocation-lifetime or caller-budget result to compare.

Opaque/lossy VP8 RIFF assembly reuses the completed VP8 payload allocation on
the ordinary no-token path by shifting it behind the RIFF and VP8 chunk headers.
The token-aware path keeps its separate output buffer and 1,024-byte copy
checkpoints, and the extended RGBA VP8X/ALPH path remains separate. Encoded
bytes, errors, cancellation checkpoints, and sink output remain unchanged.
This is a Rust-only output-buffer ownership optimization; Pillow supplies only
the final byte/error regression oracle.

The extended RGBA VP8X/ALPH path likewise reuses the completed VP8 payload
allocation on the ordinary no-token path, shifting it behind the fixed VP8X and
ALPH chunks and copying only the alpha payload before writing the VP8 chunk.
Token-aware assembly retains its separate output buffer and checkpoints. This
is a Rust-only output-buffer ownership optimization; Pillow still supplies the
final byte/error regression oracle rather than allocation evidence.

Still WebP metadata attachment likewise retains the completed RIFF allocation
on the ordinary no-token path. It shifts the existing image chunks behind the
VP8X header and optional ICCP chunk, then writes EXIF/XMP after those chunks in
the established order, removing the second complete output allocation and
full encoded-chunk copy. Token-aware attachment keeps its separate output
buffer and 1,024-byte copy checkpoints. This is a Rust-only output-buffer
ownership optimization; Pillow supplies final byte/error regression, not
allocation-lifetime evidence.

Lossy WebP VP8 plane preparation now moves the owned Y/U/V vectors through the
shared padding helper. Aligned dimensions return those vectors directly; only
dimensions requiring edge replication allocate padded planes. The replication
order, token-aware padding checkpoints, encoded bytes, errors, and sink output
remain unchanged. Pillow parity sees only final bytes/errors; plane ownership
is Rust-only evidence.

ALPH final-output preparation now recycles the retained nested candidate suffix
allocation into the next final ALPH bitstream writer. The nested candidate
trials refill the scratch vector after it is taken, so sequential frames avoid
another transient final-output allocation while candidate ordering, encoded
bytes, errors, token-aware copy checkpoints, and sink output remain unchanged.
Pillow parity sees only final bytes/errors; transient allocation ownership and
retained capacity are Rust-only evidence.

Lossless VP8L final-frame preparation uses the same ownership boundary: the
retained nested output-scratch allocation is moved into the next final frame
writer, and nested candidate trials refill the scratch vector after it is
taken. The returned frame remains independently owned, while sequential frames
avoid an additional transient final-output allocation. Encoded bytes, errors,
token-aware bit checkpoints, and sink output remain unchanged. Pillow parity
sees only final bytes/errors; output ownership and retained capacity are
Rust-only evidence.

Ordinary no-token lossy VP8 frame assembly pre-sizes its final bitstream from
the fixed frame header and the already encoded first-partition and coefficient
lengths, avoiding growth reallocations while those partitions are appended.
The token-aware path retains its prior header allocation and checkpoint
behavior. Encoded bytes, errors, cancellation checkpoints, and sink output
remain unchanged. Pillow parity sees only final bytes/errors; allocation
capacity and reallocation counts are Rust-only evidence.

WebP VP8L token streams retain one bounded histogram-clustering scratch object
per encoder image-stream scratch. Its original-tile histograms, cluster copies,
symbol map, and remapped group histograms reset their logical contents and are
reused across sequential candidate streams; cache-dependent population lengths
still resize before each use. Nested metadata streams retain their own scratch
object, so no histogram state crosses an active stream boundary. Clustering
ordering, encoded bytes, errors, and sink output remain unchanged. This is a
Rust-only histogram scratch optimization, not allocator/OOM accounting,
recoverable-OOM handling, or a streaming guarantee.

WebP VP8L backward-reference construction retains one bounded candidate
workspace per token stream. The hash-chain result table, 18-bit hash-head table,
box-chain run counts, source-token buffer, cost-estimate storage, cache-transform
storage, and trace storage reset their logical contents before each candidate
construction and reuse capacity across sequential image streams. Candidate token
vectors remain independently owned after selection, and nested metadata streams
retain their own workspace. Candidate ordering, encoded bytes, errors, and sink
output remain unchanged. This is a Rust-only backward-reference scratch
optimization, not allocator/OOM accounting, recoverable-OOM handling, or a
streaming guarantee.

WebP VP8L image-stream writing returns each candidate token vector to a bounded
two-vector pool after its trial has been emitted. A pooled vector can seed the
next cache-selection pass, while active candidates remain independently owned
until their trial completes and nested metadata streams keep separate pools.
Candidate ordering, encoded bytes, errors, and sink output remain unchanged.
This is a Rust-only candidate-result allocation optimization, not
allocator/OOM accounting, recoverable-OOM handling, or a streaming guarantee.

The VP8L candidate result list itself retains its small outer allocation across
image streams. The standard and optional low-distance box-chain candidates are
drained for each trial and the list storage is restored to scratch, including
the cancellation/error return path; candidate token vectors remain independently
owned by the existing bounded pool or active trial. Candidate ordering,
cache-bit selection, checkpoint behavior, encoded bytes, errors, and sink
output remain unchanged. This is a Rust-only result-list allocation
optimization, not allocator/OOM accounting, recoverable-OOM handling, or a
streaming guarantee.

The VP8L box-chain search now filters its bounded offset-code table into fixed
stack arrays instead of allocating temporary `Vec` values for the full and
incremental offset sets. The 32-entry bound, offset order, chain selection,
checkpoint behavior, encoded bytes, errors, and sink output remain unchanged.
This is a Rust-only box-chain workspace optimization, not allocator/OOM
accounting, recoverable-OOM handling, or a streaming guarantee.

VP8L entropy-mode analysis stores its fixed 13-entry cost table, fixed
13-by-256-value histogram accumulation table, and four-or-five-entry mode
candidate set in stack arrays instead of allocating temporary heap vectors.
The histogram and cost traversal order, candidate ordering, cancellation/error
propagation, mode selection, encoded bytes, and sink output remain unchanged.
The bounded workspaces' lifetimes are local to `analyze_entropy`, so no state
crosses an image-stream boundary. These are Rust-only entropy-analysis
workspace optimizations, not allocator/OOM accounting, recoverable-OOM
handling, or a streaming guarantee.

VP8L color-indexing transform stores its bounded 256-entry RGBA source table
and its 256-entry packed-byte expansion table in stack arrays. It also reads
each packed index into a scalar while expanding rows from right to left, so
the former dimension-dependent per-row packed-index heap buffer is gone. The
largest specialized expansion is 8,192 bytes. Color-table padding,
packed-index ordering, decoded bytes, errors, and sink behavior remain
unchanged. This is a Rust-only transform workspace optimization, not a claim
that the full transform is allocation-free, or that allocator/OOM,
stack-depth, or streaming behavior is proven.

WebP VP8L alpha encoding now builds the delta table directly from the retained
u8 palette values instead of materializing a second shifted `Vec<u32>`. Palette
ordering, delta arithmetic, encoded bytes, errors, and sink output remain
unchanged. This is a Rust-only alpha-palette allocation optimization, not
allocator/OOM accounting, recoverable-OOM handling, or a streaming guarantee.

WebP VP8L palette-mode writing computes its at-most-256 palette deltas into a
fixed stack array instead of collecting a temporary heap vector. The source
palette remains intact for index packing; palette order, delta arithmetic,
encoded bytes, errors, and sink output remain unchanged. This is a Rust-only
palette-delta workspace optimization, not allocator/OOM accounting,
recoverable-OOM handling, or a streaming guarantee.

WebP RGBA alpha encoding computes its at-most-256 palette deltas into a fixed
stack array instead of collecting a second heap vector. The sorted alpha
palette remains intact for index lookup; palette order, delta arithmetic,
encoded bytes, errors, and sink output remain unchanged. This is a Rust-only
alpha-palette workspace optimization, not allocator/OOM accounting,
recoverable-OOM handling, or a streaming guarantee.

WebP RGBA alpha-palette collection records the fixed 8-bit alphabet in a
stack presence table before emitting the required sorted unique palette `Vec`,
replacing the bounded `BTreeSet` node allocation. Palette order, checkpoint
cadence, encoded bytes, errors, and sink output remain unchanged; the returned
palette allocation is still required by the later ordering and index passes.
This is a Rust-only alpha-palette collection workspace optimization, not
allocator/OOM accounting, recoverable-OOM handling, or a streaming guarantee.

WebP VP8L predictor and cross-color transform selection retain their
pixel-scaled work maps in the image-stream scratch object. Predictor mode
selection reuses its mode map, source snapshot, and upper/current rows across
candidate modes; cross-color selection reuses and truncates its tile-sized
color map before the map is emitted. Transform ordering, encoded bytes, errors,
and sink output remain unchanged. These are Rust-only scratch-ownership
optimizations, not allocator/OOM accounting, recoverable-OOM handling, or a
streaming guarantee.

WebP VP8L histogram clustering reuses one `Histogram` merge workspace across
entropy-bin, stochastic, and greedy combinations instead of cloning all five
channel vectors for each trial. The merge is completed in scratch before the
source histogram is swapped and removed, so cancellation rollback, cluster
ordering, encoded bytes, errors, and sink output remain unchanged. This is a
Rust-only merge-scratch allocation optimization, not allocator/OOM accounting,
recoverable-OOM handling, or a streaming guarantee.

WebP VP8L histogram clustering also retains one pair queue in
`HistogramScratch` across stochastic and greedy passes. It clears the queue
between passes and keeps capacity only up to 4,096 `Pair` entries; larger
transient queues are released instead of becoming retained memory. Candidate
ordering, merge decisions, encoded bytes, errors, and sink output remain
unchanged. This is a Rust-only pair-queue allocation optimization, not
allocator/OOM accounting, recoverable-OOM handling, or a streaming guarantee.

GIF sequence encoding consumes prepared frame ownership after the complete
transparency scan. It keeps a small global-palette copy for table comparisons,
moves the first prepared frame into the emission loop, and moves later frames
from the iterator, removing per-frame `PreparedImage` clones and the two full
first-frame raster copies. Palette decisions, output bytes, and explicit token
checkpoints remain unchanged. This is a bounded ownership optimization, not
allocator/OOM accounting or a streaming guarantee.

GIF output assembly retains the previous emitted frame's palette and indices
for unchanged-pixel masking, comparing palette entries directly instead of
materializing a full RGB comparison buffer for every frame. The indexed state
is copied before the current frame's transparent substitutions, so the next
comparison sees the same pre-mask colors as before. Frame decisions, bytes, and
checkpoints remain unchanged; this is a bounded allocation optimization, not
allocator/OOM accounting or a streaming guarantee.

JPEG baseline and progressive entropy writers take the already-built JPEG
output buffer as their bit-writer storage. Restart markers and progressive scan
headers remain in that buffer, while checkpoint lengths are measured relative
to the current entropy segment so the old reset semantics are unchanged. The
writer returns the same buffer after each segment, removing the former entropy
staging copy without claiming allocator/OOM accounting or universal streaming.

JPEG grayscale encoding borrows the immutable source luminance pixels directly;
RGB encoding still owns the YCbCr planes required by its representation
conversion. Existing row cancellation polls, encoded bytes, and work-budget
checkpoints remain unchanged. This is a bounded ownership optimization, not
allocator/OOM accounting or a streaming guarantee.

PNG filtering borrows immutable source pixels for every mode that needs no
representation change. The L16 branch alone materializes a big-endian buffer;
all other source modes avoid a full pre-filter raster clone. This is a bounded
ownership optimization and does not claim destination-buffer reuse,
allocator/OOM accounting, or streaming.

TIFF page encoding borrows source pixels unless horizontal prediction is paired
with LZW or Deflate. Those two combinations alone receive a mutable owned
working copy; raw, PackBits, and non-predictive compressed paths do not. The
resulting compressed payload and page layout remain owned buffers, and this
conditional reuse does not claim allocator/OOM accounting or streaming.

TIFF sequence length planning iterates the already-owned encoded page buffers
directly instead of allocating a second vector containing only page lengths.
The same alignment and classic-TIFF overflow checks still run before output
admission or relocation, so this is a bounded bookkeeping optimization rather
than a streaming, rollback, or complete allocator guarantee.

TIFF multi-page sink delivery derives each aligned page base from the running
delivered output position while relocating pages instead of allocating a
page-count-sized base vector. Next-IFD links, relocated offsets, page alignment,
overflow checks, cancellation, sink segment boundaries, and output bytes remain
unchanged. This is bounded sink-path bookkeeping, not a streaming, rollback, or
complete allocator guarantee.

TIFF Deflate pages pass the repeated row length and row count directly to the
level-six zlib-ng tokenizer instead of allocating a temporary row-length vector.
The specialized token-aware and no-token paths replay the same input-row
boundaries, matcher behavior, checkpoint cadence, encoded bytes, and errors.
This removes one image-height-sized temporary allocation per Deflate page; it
does not claim complete allocator/OOM accounting, rollback, or streaming.

PNG’s still encoder passes the repeated filtered-row length and height directly
to the stored-block and zlib-ng compressor paths for compression levels 0
through 9, avoiding a temporary row-length vector. The ordinary and
token-aware strategies replay the same input-row boundaries, matcher behavior,
checkpoint cadence, bytes, errors, and sink output. This is a bounded
allocation optimization, not complete allocator/OOM accounting, rollback, or a
streaming guarantee.

BMP row conversion reuses one scratch buffer for one-bit, indexed, RGB, and
RGBA rows within each encoder invocation. The synchronous writer consumes the
row before the next row is prepared, so this removes per-row allocation churn
without changing structural delivery, encoded bytes, cancellation checkpoints,
or sink behavior. It is a bounded transient-allocation optimization, not
allocator/OOM accounting or a streaming guarantee.

`EncodePolicy::max_work_units` is an independent inclusive bound on the
documented cooperative encode checkpoints. A checkpoint charges one unit
before it continues; when the next charge would exceed the maximum, encoding
returns `ImageError::LimitExceeded` with
`ResourceLimit::EncodeWorkUnits`. The budget is layered over a caller token,
so caller cancellation still has precedence and remains `Cancelled`.
The `u64::MAX` policy value is treated as observationally uncapped: policy
plumbing reuses the caller token, or an uncapped token for a source-less call,
while preserving the token-aware codec path. This avoids a redundant budget
cell and counter mutation at every checkpoint; every finite maximum retains
the independent budget state and exact exhaustion behavior described above.

TIFF Deflate tokenization additionally charges at each supplied input-row boundary
and inside the level-six matcher candidate, insertion, fizzle, window, and
position intervals, so a bounded page cannot consume the complete matcher pass
between public checkpoints. TIFF Deflate emission additionally charges while
expanding tokens, analyzing Huffman trees, emitting stored/fixed/dynamic
bitstreams, copying stored-block bytes, and computing the Adler-32 trailer.
With a caller token, PNG stored compression additionally checks input-chunk and
stored-block boundaries, each 1,024-byte stored-block-copy interval, and its
Adler-32 calculation, while every zlib-ng PNG
level uses token-aware matcher, token expansion, Huffman/bitstream emission,
and checksum stages; no-token PNG paths remain on the ordinary byte-producing
helpers.
PNG adaptive filtering and filtered-row emission charge
additional checkpoints after each 1,024 row bytes, including while candidate
filters are scored. BMP row conversion additionally charges after each 1,024
pixels. GIF RGB/RGBA palette quantization additionally charges after each 1,024
pixels while preparing palette/index data. High-color RGB median-cut preparation
also charges around hash/order setup, axis ordering, median-cut split stages,
and 1,024-item split/partition scans; its nearest-palette candidate ordering
and bounded candidate scan also charge after each 1,024 work items. RGBA
FASTOCTREE preparation also
charges after each 1,024-cell, bucket, or lookup-entry interval, and its
Apple-compatible bucket sorting charges after each 1,024 sorting operations.
GIF LZW charges
an input-symbol interval for each dictionary-pass input symbol. Lossy WebP VP8 additionally charges after each
batch of 1,024 RGB/RGBA-to-YUV conversion items and each batch of 1,024
scanned or flattened RGBA transparent-area cleanup pixels, RGBA alpha-palette
source collection and index packing after each 1,024 source pixels, and each
batch of 64 nearest-delta candidate values, each batch of 1,024 analyzed
macroblocks, and each batch of 64 frame-selection macroblocks (roughly 1,024
luma 4×4 blocks), with token-aware intra4 selection also checking after each
candidate-trial stage, forward- and inverse-transform row/column subpass,
non-trellis quantization coefficient, each method-6 trellis-quantization
coefficient candidate and path-reconstruction node, squared-error pixel,
spectral-distortion weighted-transform row/column pass, residual-cost
coefficient, candidate, and completed luma 4×4 block, then
after color conversion, padding, analysis, segment parameters, mode selection,
coefficient-probability
adaptation, required padded Y/U/V edge-replication after each 1,024 padded
items (aligned planes take ownership without a clone), analysis histogram construction
after each 64 completed 4×4 blocks, analysis and segment assignment after each
1,024 macroblocks, mode selection after each 64 completed
macroblocks (roughly 1,024 luma blocks), filter-edge adjustment,
coefficient-statistics collection, and the first-
partition segment-probability prepass after each 1,024 selected macroblocks,
partition emission, each 8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, 8,192-bit, 32,768-bit, 65,536-bit, 131,072-bit, and 262,144-bit logical first-partition interval,
each 16,384-boolean first-partition-bit interval,
each 8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, 8,192-bit, 32,768-bit, 65,536-bit, 131,072-bit, 262,144-bit, 524,288-bit, 1,048,576-bit, and 2,097,152-bit logical coefficient interval, each 16,384-boolean coefficient-bit
interval, each 1,024-byte boolean-bitstream
output interval, and final container assembly. Lossless WebP VP8L RGB/RGBA
source-pixel materialization, image-palette source scans, and ordered
unique-color palette drains after each 1,024 source pixels or colors,
palette-index lookup candidate
scans, palette sign collection, and nearest-delta ordering
likewise charge after each 64 palette
entries or candidate values in the token-aware path.
JPEG baseline and progressive RGB-to-YCbCr conversion and chroma downsampling
additionally charge after each 1,024 converted or produced pixels, forward-DCT/
quantization charges after each completed 8x8 block, optimized baseline Huffman
frequency gathering charges after each 1,024 AC coefficients, progressive scan
block-slot generation charges after each 1,024 blocks, progressive scan-event
frequency gathering charges after each 1,024 events, progressive scan coefficient
traversal charges after each 1,024 coefficients, baseline entropy traversal
additionally charges after each 1,024 MCUs, and entropy coding charges after
each 1,024 emitted entropy bytes; its no-token path remains on the
ordinary byte producer.
Lossless WebP VP8L RGB/RGBA source-pixel materialization, image-palette
source scans, ordered unique-color palette drains, and palette-mode index
packing additionally charge after each 1,024 source pixels or colors; sampled
meta-pixel materialization additionally charges after each 1,024 retained
histogram symbols; it also charges around RGBA
hidden-RGB cleanup after each
1,024 scanned pixels, RGB-equal grayscale preparation after each 1,024 pixels,
predictor source-snapshot copying, mode-application wide source-row copies in
completed 1,024-pixel chunks, tile scans, mode application, and
subtract-green transforms after each 1,024 pixels,
cross-color multiplier search/transform tiles and sampling scans/compaction,
including meta-histogram row/column comparisons and symbol compaction after
each 1,024 symbols,
entropy-mode histogram-cost analysis after each 64 symbols, transform
selection/application, bounded backward-reference
length-cost table and equal-cost interval setup after each 1,024 entries,
token-aware cost-manager interval-update and cleanup scans after each 256
cumulative interval entries,
non-saturated interval split/merge after each 1,024 interval-work entries, and
saturated cost-interval fallback scans after each 1,024 entries,
repeated-run hash-chain insertion, long backward-reference result backfills
after each 256 entries, hash-chain candidate selection after each 64 completed
trials, palette-mode box-chain candidate offsets after each 64 completed
offsets, search/match-length/cache, token-aware trace, path reconstruction, and
token replay after each 256 consumed pixels (the no-token trace/replay retains
its 1,024-pixel cadence), and copy-token
cache-population scans after each 256 pixels, plus token/Huffman cost
scans after each 1,024 tokens or 64 symbols,
Huffman-tree simple-tree symbol-discovery scans after each 64 code-length slots,
Huffman RLE preparation, including reverse-tail fixed-alphabet scans, and
in-run code-length scans after each 64 symbols,
canonical-code assignment scans after each 64 symbols, Huffman-tree ordering comparisons after each 64 comparisons,
Huffman-tree insertion scans after each 64 candidate nodes,
Huffman-tree code-length-token frequency, trailing zero-repeat token trim, and
code-length-emission scans after each 16 compressed token entries, histogram-clustering
populated-tile collection, min/max, and bin-assignment pre-passes after each 64
tile histograms, histogram clustering
(including token-aware population scans after each 64
symbols), Huffman-tree/group emission, token-stream
intervals, each 8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, 8,192-bit, 16,384-bit, 32,768-bit, 65,536-bit, 131,072-bit, 262,144-bit, 524,288-bit, 1,048,576-bit, and 2,097,152-bit logical bitstream interval, and each 1,024-byte
VP8L bitstream-output interval. The token-aware source-materialization branch
is separate; no-token VP8L source maps retain their original tight iterators.
This is
deterministic work control, not CPU-time,
instruction-count, transient-memory, or recoverable-OOM accounting.

Token-aware encode variants are a separate cooperative work-control boundary.
Still encodes check the token before dispatch and after the codec returns; the
GIF still writer also polls at its block/frame/coalescing/output-assembly and
RGB/RGBA palette quantization, RGB median-cut hash/order, axis-ordering,
split, and partition checkpoints, and fixed RGBA FASTOCTREE cell/bucket/lookup
and bucket-sort checkpoints plus GIF LZW input-symbol intervals, the WebP still writer polls at
preparation, lossy VP8 RGB/RGBA-to-YUV conversion, required padded-plane edge
replication, analysis histogram construction after each 64 completed 4×4
blocks, analysis and segment assignment after each 1,024 macroblocks,
intra4 mode selection after each candidate-trial stage, forward- and inverse-transform row/column subpass, non-trellis quantization coefficient, each method-6 trellis-quantization coefficient candidate and path-reconstruction node, squared-error pixel, spectral-distortion weighted-transform row/column pass, residual-cost coefficient, candidate, and completed luma 4×4 block
and its outer 64-macroblock batch for intra16/chroma work,
filter-edge adjustment, RGBA transparent-area cleanup after each 1,024 scanned
or flattened pixels, RGBA alpha-palette source
collection and index packing after each 1,024 source pixels, lossy WebP VP8/ALPH
RIFF payload and compressed/raw alpha-stream buffer copies after each 1,024
output bytes,
nearest-delta
candidate values after each 64 candidates, macroblock-analysis, and
intra4 mode selection after each candidate-trial stage, forward- and inverse-transform row/column subpass, non-trellis quantization coefficient, each method-6 trellis-quantization coefficient candidate and path-reconstruction node, candidate, and completed luma 4×4 block
and its outer 64-macroblock batch for intra16/chroma work, plus
mode-selection subsegments
plus
analysis/segment-assignment/coefficient-
probability, filter-edge adjustment, and first-partition segment-probability
prepass after each 1,024
selected macroblocks, 8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, and
8,192-bit, 32,768-bit, 65,536-bit, 131,072-bit, and 262,144-bit logical first-partition intervals, 16,384-boolean first-partition-bit intervals,
8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, 8,192-bit, 32,768-bit, 65,536-bit, 131,072-bit, 262,144-bit, 524,288-bit, 1,048,576-bit, and 2,097,152-bit logical coefficient intervals, 16,384-boolean coefficient-bit intervals,
1,024-byte boolean-bitstream output intervals, and bitstream stages, lossless
VP8L RGB/RGBA source-pixel materialization, image-palette source scans,
ordered unique-color palette drains, and palette-mode index packing after each
1,024 source pixels or colors, sampled meta-pixel materialization after each
1,024 retained histogram symbols, and hidden-RGB cleanup
after each 1,024 scanned pixels, plus
palette-index lookup candidate scans after each 64 palette entries, palette sign
and nearest-delta candidate scans after each 64 palette entries
or candidate values, predictor source-snapshot copying and mode-application
wide source-row copies in completed 1,024-pixel chunks,
predictor/cross-color/entropy/transform, bounded backward-reference
cost/length-table initialization and length-cost/equal-cost interval setup
after each 1,024 entries,
token-aware cost-manager interval-update and cleanup scans after each 256
cumulative interval entries,
non-saturated interval split/merge after each 1,024 interval-work entries, and
saturated cost-interval fallback scans after each 1,024 entries,
repeated-run hash-chain insertion, long backward-reference result backfills
after each 256 entries, search/match-length/cache, token-aware trace, path
reconstruction, and token replay after each 256 consumed pixels (the no-token
trace/replay retains its 1,024-pixel cadence), and copy-token
cache-population scans after each 256 pixels, plus token/Huffman cost
scans after each 1,024 tokens or 64 symbols,
Huffman-tree simple-tree symbol-discovery scans after each 64 code-length slots,
Huffman RLE preparation, including reverse-tail fixed-alphabet scans, and
in-run code-length scans after each 64 symbols,
canonical-code assignment scans after each 64 symbols, Huffman-tree ordering comparisons after each 64 comparisons,
Huffman-tree insertion scans after each 64 candidate nodes,
Huffman-tree code-length-token frequency, trailing zero-repeat token trim, and
code-length-emission scans after each 16 compressed token entries, histogram-clustering
populated-tile collection, min/max and bin-assignment pre-passes after each 64
tile histograms, histogram population,
combined entropy-cost, and
histogram-merge scans after each 64 symbols, histogram/Huffman, token-stream, 8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, and
2,048-bit, 4,096-bit, 8,192-bit, 16,384-bit, 32,768-bit, 65,536-bit, 131,072-bit, 262,144-bit, 524,288-bit, 1,048,576-bit, and 2,097,152-bit logical bitstream intervals, and 1,024-byte bitstream-output stages, codec-result, and metadata-assembly
boundaries, and the JPEG still writer additionally polls after each 1,024
converted RGB or chroma-downsample output pixel, each 1,024 AC coefficients
during optimized baseline Huffman frequency gathering, each 1,024 progressive
scan block slots, each 1,024 progressive scan coefficient items, each 1,024
baseline entropy MCUs, and each
1,024-byte entropy-output interval; the JPEG, PNG,
BMP, ICO, and TIFF still
writers plus the one-frame JPEG/BMP/ICO and multi-page TIFF sequence sink
writers poll while
preparing rows, embedded payloads, or TIFF page state, including PNG adaptive
filter and all-level token-aware compression subsegments, and between emitted
structural segments in their sink paths. ICO still delivery has the same
source-size, payload, and directory boundaries; TIFF sink delivery checks
between its header, strip/padding, and IFD/value segments. GIF, TIFF, WebP,
and native AVIF sequence encoders
additionally poll at their retained-frame, coalescing/page, and finalization
checkpoints. A structural sink cancellation may leave the already-written
prefix because no rollback contract exists. A sink flush/finalization failure
is normalized to `ImageError::OutputWrite` after delivery and likewise does
not roll the prefix back. The token-aware VP8L traced backward-reference
dynamic-programming pass, path reconstruction, and token replay now poll after
each 256 consumed pixels; the const-specialized no-token path retains its
1,024-pixel cadence. Progress
callbacks, transient working-state
reduction, short-write/rollback cleanup, and interruption beyond the
documented checkpoints—including remaining finer WebP bitstream work beyond the
implemented 8-bit/16-bit/32-bit/64-bit/128-bit/256-bit/512-bit/1,024-bit/2,048-bit/4,096-bit/8,192-bit/32,768-bit/65,536-bit/131,072-bit/262,144-bit logical VP8 first-partition and 8-bit/16-bit/32-bit/64-bit/128-bit/256-bit/512-bit/1,024-bit/2,048-bit/4,096-bit/8,192-bit/32,768-bit/65,536-bit/131,072-bit/262,144-bit/524,288-bit/1,048,576-bit/2,097,152-bit logical VP8 coefficient intervals,
the 16,384-boolean first-partition and coefficient-bit intervals, and the
1,024-byte boolean-bitstream-output
intervals, the 8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, 8,192-bit, 16,384-bit, 32,768-bit, 65,536-bit, 131,072-bit, 262,144-bit, 524,288-bit, 1,048,576-bit, and 2,097,152-bit logical VP8L bitstream intervals, and CPU work inside codec
rows other than the implemented PNG adaptive-filter subsegments, BMP
row-conversion subsegments, token-aware PNG stored-block/all-level Deflate
stages, and LZW input-symbol intervals, WebP
RGB/RGBA-to-YUV conversion, RGBA transparent-area cleanup, macroblock-analysis,
and analysis histogram construction beyond the implemented 64 completed 4×4
block checkpoint, and mode-selection work beyond the implemented intra4
candidate-trial-stage,
forward- and inverse-transform row/column subpass, non-trellis
quantization-coefficient, method-6 trellis-quantization coefficient-candidate
and path-reconstruction-node, squared-error pixel, spectral-distortion
weighted-transform row/column pass, residual-cost coefficient, per-block, and
outer 64-macroblock checkpoints, JPEG baseline entropy traversal
beyond the
implemented 1,024-MCU interval, optimized-Huffman frequency work beyond
the implemented 1,024-AC interval, progressive scan block-slot work beyond
the implemented 1,024-block interval, progressive scan-event frequency work
beyond the implemented 1,024-event interval, progressive scan coefficient work
beyond the implemented 1,024-coefficient interval, TIFF Deflate path, and the
remaining finer WebP/Deflate work—remain
open.

### Codec work is bounded by the resource set

Every work dimension of the current codecs is bounded by one of the typed
resources above, so no codec work can grow independently of output size:

| Codec | Work dimension | Bounding resource |
| --- | --- | --- |
| PNG | chunk scan and count | `max_encoded_bytes` + `max_metadata_bytes` |
| PNG | compressed ancillary validation | fixed 1 MiB inflated-prefix bound |
| PNG | IDAT/fdAT inflation and scanline filtering | `max_pixels`/`max_primary_decoded_bytes` (inflated length equals canvas bytes) |
| GIF | LZW decompression and deinterlace | `max_pixels`/`max_primary_decoded_bytes`/`max_frame_decoded_bytes` |
| GIF | extension and block walk | `max_encoded_bytes` + `max_metadata_bytes` |
| JPEG | marker scan and progressive scan count | `max_encoded_bytes` (entropy spans are inside the input) |
| TIFF | directory walk | `max_metadata_bytes` |
| TIFF | strip/tile decompression and predictors | `max_frame_decoded_bytes`/`max_sequence_decoded_bytes` |
| WebP | RIFF chunk walk | `max_metadata_bytes` |
| WebP | VP8/VP8L/ALPH decompression | `max_pixels`/`max_primary_decoded_bytes` |
| ICO | directory walk and entry decode | `max_metadata_bytes` + `max_pixels`/`max_primary_decoded_bytes` |
| AVIF | top-level box/item walk minus referenced sample spans | `max_metadata_bytes` |
| AVIF | AV1 tile/block reconstruction | `max_encoded_bytes` + canvas limits (portable classes have fixed extents) |

The boundary manifests exercise each resource at below/at/above and
`u64::MAX`/`u32::MAX` extremes on small assets; a future codec or container
feature that introduces a work dimension outside this set must add a typed
limit before acceptance. Caller-visible policy options that shape results
(lenient-versus-strict parsing, requested output mode) are result policy, not
resource limits, and belong with the API-033 family.

### Allocation and arithmetic policy

Every caller-bounded allocation is preceded by checked preflight arithmetic:
the inclusive `DecodePolicy` limits compare exact observed lengths, and
`ImageMode::expected_bytes` uses checked multiplication before any primary,
later-frame, or cumulative byte claim. Boundary manifests test just-below,
at, and above every limit, plus extreme `u64::MAX`/`u32::MAX` maxima, using
small assets so no enormous fixture is ever allocated.

Codec-internal `Vec` allocations remain infallible and the crate deliberately
does not use `try_reserve` or recoverable out-of-memory errors: Rust's default
allocation abort is the documented OOM behavior, and the release gate for
hostile decode input is the checked preflight above rather than
allocation-error recovery. `EncodePolicy` rejects an oversized completed
result, but it does not change that allocation policy. This is the retained
QA-015 decision: near-limit arithmetic is fixture-proven without enormous
allocations, and no public API promises a recoverable allocation failure.

## Decoded sample layouts

`DecodedImage::pixels` is tightly packed and row-major. There is no implicit
row stride.

### Trailing input and consumed extent

Every decoder parses only its container-defined extent and ignores well-formed
trailing bytes; trailing bytes never change the decoded result. `Decoded::consumed_bytes`
names that extent when the container defines one unambiguously: JPEG ends at
the EOI marker, PNG at the IEND chunk, GIF at the trailer, WebP at the
RIFF-declared size, TIFF at the end of the final main-chain IFD, and AVIF at
the last successfully parsed top-level BMFF box. BMP and ICO do not declare a
total extent, so they report `None` and the complete input remains the source.
AVIF container validation tolerates an unparseable tail only after a complete
still or sequence structure has been parsed, matching Pillow 12.2.0/libavif;
truncated or conflicting structure remains `Malformed`.
When a defined extent is shorter than the supplied input, the successful
`Decoded<T>` envelope also carries `DiagnosticKind::TrailingDataIgnored` with
the first ignored byte's offset. Containers without an unambiguous extent
(`BMP` and `ICO`) do not emit this diagnostic.

| Mode | Layout |
| --- | --- |
| `L1` | One bit per sample, most-significant bit first, each row byte-aligned |
| `P8` | One palette index per byte |
| `L8`, `La8` | Interleaved 8-bit luminance and optional alpha |
| `Rgb8`, `Rgba8`, `Cmyk8` | Interleaved 8-bit channels |
| `L16`, `La16`, `Rgb16`, `Rgba16` | Interleaved little-endian 16-bit channels |
| `F32`, `I32` | Exact Pillow-observable 32-bit luminance bytes. These are byte-preserving modes, not portable typed-scalar views: TIFF `I`/`F` can retain the file byte order. |
| `Rgb32F`, `Rgba32F` | Native-endian 32-bit floating-point RGB(A) samples |

Code that needs numeric TIFF `I32`/`F32` values must read
`DecodedImage::source.byte_order()` before parsing the bytes. This distinction
is required for Pillow 12.2.0 parity: on a little-endian host, Pillow
`tobytes()` still returns big-endian bytes for a big-endian TIFF `I` or `F`
source. The descriptor reports source-container order; it does not override
the documented little-endian `L16` transfer layout or affect 8-bit modes. TIFF
encoding does not consume the descriptor and treats the supplied `I32`/`F32`
buffer as exact Pillow-observable transfer bytes. The manifest proves
uncompressed, Deflate plus horizontal-predictor, and LZW plus
horizontal-predictor output from detached big-endian source bytes.

`DecodedImage::new` and `DecodedImage::with_mode` record caller-supplied buffers
without validating them. `DecodedImage::try_new` and `try_with_mode` validate
the same state while reusing the supplied pixel vector on success;
`try_with_palette` validates indexed palette state after attaching a palette.
Direct field literals and the compatibility builders remain unchecked.
`DecodedImage::validate`, every encoder, and sequence validation reject:

- zero dimensions;
- arithmetic overflow;
- a byte length inconsistent with dimensions and mode;
- a `ColorType` inconsistent with `ImageMode`;
- palettes on non-indexed images;
- invalid RGB or alpha table lengths; and
- palette indices outside the retained palette.

`DecodedSequence` validates its canvas, requires at least one frame, validates
each frame image, and rejects frame rectangles outside the canvas.

## Errors and recovery

Codec-dispatched failures attach the public operation that was executing when
the failure escaped (`ImageError::stage()`): inspection, still decode, still
encode, sequence decode, sequence encode, or verification. Caller-built
validation failures, option-construction errors, and target/availability
checks remain intentionally stage-free because they do not belong to one
operation; `UnknownFormat` and `FeatureDisabled` have no stage, and
`LimitExceeded` already carries the typed `CodecOperation` instead. Stages are
stable recovery fields; the diagnostic message remains non-contractual prose.

Where a codec parser can name the failing container structure, it also attaches
the encoded-input byte offset (`ImageError::offset()`) and a stable structure
identity (`ImageError::identity()`): PNG chunk boundaries, GIF blocks/images/
extensions, JPEG markers/segments, TIFF IFDs, WebP chunks on the metadata-scan
path, AVIF boxes, BMP header/palette/pixel-span/bitfield/RLE boundaries, ICO
header/directory/entry-range/embedded PNG/DIB/CUR boundaries, and WebP
inspection/container-chunk boundaries. Still and sequence WebP payload-decoder
failures also retain `webp_bitstream` at the validated payload start (or the
current ANMF container offset for an animation). The decoder does not promise a
finer inner bitstream cursor. Both fields are stable recovery data, never prose.

When `encode_to_sink` or `encode_sequence_to_sink` receives an error from its
caller-owned `OutputSink`, it normalizes that rejection to
`ImageError::OutputWrite`. Every available still codec and supported multi-frame
sequence writer has an explicit Rust contract for this normalization. The error retains the selected
output format, the `StillEncode` or `SequenceEncode` stage, and the sink's diagnostic message;
input offset and container identity are `None` because the failure is on the
destination side. This boundary defines one post-delivery `OutputSink::flush`
call; a flush failure is also `ImageError::OutputWrite` and may follow a
complete prefix. The Rust-only partial-write contract additionally proves that
every available still codec and each supported multi-frame GIF/TIFF/WebP/native-
AVIF sequence writer may reject after accepting a partial structural prefix: the
delivered prefix remains observable, the error stage is `StillEncode` or
`SequenceEncode`, and `flush` is not called. Test revision
`d5f7e416b30862819dbddb38f8b6027cc4219076` reinforces the same boundary in the
exact-byte sink paths and both TIFF still compression options plus sequence
delivery. Short-write behavior on other paths, rollback, and partial-container
cleanup remain open. Every current codec sink writer reports the same
structured cause if any validated emitted segment is rejected.

The token-aware lossless WebP VP8L predictor tile scan also copies each
image-width source row in 1,024-pixel chunks and polls after each completed
chunk. Its no-token branch retains the original bulk row copy. This is a
Rust-only work-budget boundary; Pillow parity does not model caller tokens,
typed work limits, or sink rollback.

The token-aware lossless WebP VP8L entropy-mode analysis also polls its pixel
histogram after each completed 1,024-pixel chunk on rows wider than 1,024
pixels. Narrower rows already have a row-start poll at every row, and the
no-token traversal remains a direct loop over the existing heap-backed
histogram table. This is Rust-only work-budget evidence; Pillow has no caller
token, typed work limit, or sink rollback result.

Non-fatal recovery is separate from `ImageError`: successful decode returns
`Decoded<T>::diagnostics`, while fatal parser failures remain `ImageError`.
The Rust diagnostic fields are a defensive/specification contract, not a
Pillow-parity field. The committed diagnostic manifest proves the stable
kind/stage/offset/identity values for accepted GIF recovery, invalid
compressed PNG ancillary members, accepted PNG structural recoveries including
duplicate palette chunks, missing `IEND`, bad `IEND` CRC, and post-`IDAT` CRC
recovery, and trailing input; Pillow success and unchanged pixels are recorded
as supporting fixture evidence.

All canonical fallible operations return `ImageResult<T>`. `ImageError` is
non-exhaustive so downstream matches need a fallback arm.

| Error | Meaning | Typical recovery |
| --- | --- | --- |
| `UnknownFormat` | No supported signature matched | Check the complete input or select another parser |
| `FeatureDisabled` | The format is recognized but its Cargo feature is off | Enable that feature or reject the format |
| `Malformed` | The selected decoder rejected the encoded bytes | Do not retry unchanged bytes |
| `Unsupported` | The format is valid enough to identify, but the requested operation/class is unavailable | Choose another target, option, or implementation |
| `Dimensions` | Dimensions, frame bounds, or sample length are invalid | Correct or constrain the caller-supplied data |
| `Parameter` | An encoder option, palette, mode combination, or other parameter is invalid | Correct the named input |
| `LimitExceeded` | A caller-configured resource maximum was exceeded | Reduce the input or raise the selected policy maximum |
| `NeedMoreData` | An incremental prefix is incomplete and names the minimum total input length for retry | Append input to reach `minimum_input()` and retry |
| `Cancelled` | A token-aware decode or encode stopped at a cooperative checkpoint | Start a fresh operation with a new token |
| `OutputWrite` | A caller-owned encoded-output destination rejected an emitted segment | Inspect the diagnostic, repair or replace the destination, and retry the encode; a structural writer may have left a prefix |

`ImageError::kind()` is the stable recovery category and
`ImageError::format()` identifies the selected codec when one exists.
`Dimensions` and `Parameter` retain optional format plus the high-level
diagnostic that crossed the codec boundary. `ImageError::message()` exposes
that diagnostic for logs; its prose may become more specific and is not a
commitment to preserve every internal parser phrase as public API.
`ImageError::unsupported_reason()` separately reports
`TargetUnavailable` or `NotImplemented` for capability failures and returns
`None` when the unsupported result is specific to the input class or retained
metadata. This is a Rust capability contract, not a Pillow-parity field.

## Immutable source lifecycle

`EncodedImage::new` converts input into an `Arc<[u8]>`, detects the format, and
inspects the header. It does not decode pixels. The owned and borrowed
source-bound still and sequence methods reuse that validated format for
dispatch, avoiding a second signature-detection scan; codec parsing and
verification remain independent until a codec-specific parsed representation
can be proved safe to retain.

The first call to `decode()` initializes a shared still `OnceLock`, and the
first call to `decode_sequence()` initializes an independent sequence
`OnceLock`:

- clones observe the same encoded-byte snapshot and metadata;
- successful still and sequence materialization are reused independently;
- deterministic failures from the unlimited compatibility operations are
  cached in their corresponding cache;
- `decode_state()` and `sequence_decode_state()` distinguish
  `NotAttempted`, `Succeeded`, and `Failed`, while the `is_*_decoded()` helpers
  remain success-only compatibility predicates; and
- `verify()` runs independently and does not populate or modify either decode
  cache.

The retained source payload is additive: an owned source keeps one shared copy
of the encoded bytes, plus the inspected metadata and whichever decoded cache
results have succeeded. A still cache retains its decoded pixel vector and
palette/metadata payload; a sequence cache retains every retained frame and
its presentation/metadata payload. Calling both operations therefore retains
both decoded payload sets, while cloning an `EncodedImage` adds no encoded or
decoded buffer copy. A borrowed `EncodedImageView` owns none of the input
bytes and has no cache, so each returned decode owns its result for that call.
These are retained-payload bounds, not a total allocator peak: codec parser,
decompressor, and temporary materialization buffers may add transient memory,
and no recoverable-OOM or allocator-count contract is promised.

`EncodedImage::new_with_policy` applies the input limit and format allow-list
before inspection and primary-canvas dimension, pixel, decoded-byte, and
frame-count limits immediately afterward. `decode_with_policy` checks encoded
bytes, the retained format, and retained `ImageInfo` before consulting the
`OnceLock`: a policy failure is never cached,
a later sufficient policy can initialize the ordinary cache, and an earlier
cached success cannot bypass a later stricter policy. The policy is per
operation rather than permanently attached to the source. The sequence policy
variant follows the same resource-limit semantics through the selected-format
dispatch path; it does not poison the unlimited sequence cache with a
policy-dependent failure.

`ImageFormat::verification_scope()` and
`EncodedImage::verification_scope()` distinguish `Structure` from
`HeaderOnly`. Header-only is Pillow 12.2.0's base `ImageFile.verify` behavior:
successful construction/inspection is the complete check, so later pixel
decompression can still fail. PNG has Pillow's structural scan; this crate
also retains independently proved JPEG and WebP structural verifiers.

`VerificationScope::provides()` orders `HeaderOnly` < `Structure` <
`FullPixels`. `EncodedImage::verify_with_scope(requested)` accepts every scope
the format provides and fails with a format-qualified `Unsupported` for a
stronger request, never reporting weaker evidence as sufficient. No codec
currently provides `FullPixels`, so requesting it always fails. The default
`verify()` remains the format's Pillow-compatible scope; verification still
reparses independently without a caller work budget, which remains backlog
under API-023/030.

The crate performs no filesystem or network I/O and emits no logs. Applications
decide where bytes come from and how errors are recorded.

## Features and targets

Default features are `jpeg`, `png`, `gif`, `bmp`, `tiff`, `webp`, and `ico`.
`avif` is opt-in. ICO intentionally enables PNG and BMP because entries can use
either representation.

| Feature | Native | `wasm32-unknown-unknown` |
| --- | --- | --- |
| `jpeg` | Rust inspect/decode/encode | Build-verified Rust path |
| `png` | Rust still/APNG sequence decode and still encode | Build-verified Rust path |
| `gif` | Rust still/sequence decode and encode | Build-verified Rust path |
| `bmp` | Rust inspect/decode/encode | Build-verified Rust path |
| `tiff` | Rust still/multipage decode and encode | Build-verified Rust path |
| `webp` | Rust still/sequence decode and still/keyframe-sequence encode | Build-verified Rust path |
| `ico` | Rust inspect/decode and source-sized encode | Build-verified Rust path |
| `avif` | Fixed native inspect/decode/sequence/encode stack | Portable inspect and a manifest-bounded still-decode subset; sequence decode and encode unsupported |

The compatibility guarantee is limited to active manifest cases. Default
native and WASM builds select the same Rust codec code, but the complete
semantic matrix has not yet executed in a WASM runtime. Every feature lane
also executes feature-gate and capability-table evidence on `wasm32-wasip1`
under Node's WASI preview1; `wasm32-unknown-unknown` remains
build/rustdoc-verified.

See [AVIF support](avif.md) for the native dependency and portable boundary.

## Internal source organization

| Path | Responsibility |
| --- | --- |
| `src/lib.rs` | crate contract, signature detection, canonical root API |
| `src/source.rs` | immutable encoded snapshots and lazy decode cache |
| `src/encode_options.rs` | typed codec option records and strict legacy-pair migration |
| `src/types/` | formats, modes, palettes, images, frames, sequences, errors, validation |
| `src/codecs/mod.rs` | private feature dispatch and availability checks |
| `src/codecs/error.rs` | private codec failures and public error translation |
| `src/codecs/compression/` | DEFLATE/zlib behavior shared by PNG and TIFF |
| `src/codecs/<format>/` | format-local inspection, decoding, and encoding |
| `src/codecs/avif/native.rs` | safe ownership model around the opt-in unsafe FFI calls |
| `src/codecs/avif/native/bridge.c` | narrow libavif C bridge |
| `src/codecs/avif/av1/` | portable AV1 parsing, entropy, partition, and reconstruction work |

The folder `src/codecs/webp/native/` is a Rust port organized around upstream
algorithm boundaries. It does not link a native WebP library.

## Dependency and unsafe boundary

`bytemuck` is the only Cargo dependency. Default codecs do not link native
codec libraries.

Crate-wide unsafe Rust is denied. The only module-level exception is
`src/codecs/avif/native.rs`, which owns the optional libavif handles and
documents pointer, buffer, and deallocation invariants adjacent to each unsafe
operation. The separately compiled C bridge is enabled only for native builds
with the `avif` feature.

## Resource behavior and current limit

The public return APIs are whole-buffer based:

- inputs are borrowed byte slices or immutable encoded snapshots;
- decoded pixels and return APIs allocate complete buffers; current sink
  delivery can emit validated structural segments from its working state
  without an additional dispatcher buffer;
- sequence APIs retain complete supported frame data; and
- `OutputSink` is the caller-owned writer boundary; every current codec sink
  delivery can emit validated structural segments, while complete codec
  working state may remain in memory until those segments are ready.

`DecodePolicy` already bounds encoded input, the inspected primary canvas and
transfer bytes, the inspected frame/page count, later-frame and cumulative
sequence bytes, and the encoded metadata extent. `EncodePolicy` additionally
rejects a complete encoded result above its caller-selected maximum before a
return or the first sink write. The current crate should still not be
described as hardened for arbitrary hostile inputs: whole-buffer encoders and
PNG's filtered/compressed working state remain infallible allocations, and no
recoverable out-of-memory contract is promised. Resource limits and this
remaining gap are tracked in the [canonical roadmap](roadmap-new.md).

## Retained and removed scope

Retained by design:

- exact, version-controlled Pillow-oracle fixtures;
- source-derived codec implementations with complete provenance;
- private coverage hooks for defensive states no valid fixture can reach;
- explicit target formats for encoding; and
- the fixed native AVIF path until portable parity replaces it.

Removed from earlier iterations:

- the `pillow-rs-image` package identity;
- a mutable/general image buffer and processing layer;
- public format-specific codec entry points;
- duplicate `try_*` and `Option`-returning fallible APIs;
- Serde development dependencies;
- isolated unit tests in place of the manifest harness;
- size-only or approximate output comparisons; and
- ICO resizing and implicit multi-resolution generation.

Downstream projects may add processing or bindings, but those contracts do not
expand this crate's public scope.
