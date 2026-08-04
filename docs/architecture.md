# Architecture and public contract

Status: current implementation reference

Reviewed: 2026-08-04 against the committed tree based on
`c9525654b82c9cf14c61029219ec88ccf2ccd006`; the claim-ledger baseline remains
`f1048bc0399fad9801559ca7fcfd3163427b5832`.

This document explains the stable mental model and ownership boundaries of
`image-slash-star`. The generated Rust API documentation remains the
declaration-level reference.

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
`AvifTransformProperties` on the primary item without rotating, mirroring,
rescaling, or cropping decoded samples. Other codecs currently return an empty
descriptor. AVIF direct alpha `auxl` relationships are retained as
source-local item IDs through `SourceDescriptor::avif_auxiliary_relationship()`
when present, and the bounded
`SourceDescriptor::avif_auxiliary_relationships()` list also retains alpha
links to supported grid-derived color items. For a primary grid,
`SourceDescriptor::avif_grid_item_ids()` also retains its ordered derived
color-item list, `SourceDescriptor::avif_item_relationships()` retains
bounded `iref` edges such as the grid's ordered `dimg` references, and
`SourceDescriptor::avif_premultiplied_relationships()` filters source-local
`prem` edges. A source descriptor is structural provenance, not opaque
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
`SourceColor` or changing decoded samples. Non-primary ICC profiles and other
item color/property forms remain outside this typed boundary.

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
`SourceDescriptor`; full grid topology, track-only content,
unknown-item-property semantics, and auxiliary-item decoding remain outside
this model.
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
Non-ICC profiles, track-only/auxiliary item properties, item color/property
forms beyond typed CICP, grid topology, and derived/grid composition remain
outside the current model; bounded direct, supported grid-derived alpha
`auxl`, ordinary `iref`, `prem`, and typed non-primary `colr`/`nclx`
declarations are the explicitly retained exceptions.

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
| `decode_with_format_and_policy(&[u8], ImageFormat, &DecodePolicy)` | Apply the encoded-input limit, validate the selected signature, then apply metadata/canvas/frame/decoded-byte limits before decode |
| `decode_prefix`, `decode_prefix_with_policy` | Incremental still decode with the non-terminal `NeedMoreData { minimum }` status |
| `decode_with_token`, `decode_with_token_and_policy` | Still decode that polls a `CancellationToken` at structural checkpoints |
| `decode_sequence(&[u8])` | Auto-detect and retain every supported frame plus presentation metadata |
| `decode_sequence_prefix`, `decode_sequence_prefix_with_policy` | Incremental sequence decode with the same non-terminal status |
| `decode_sequence_with_token`, `decode_sequence_with_token_and_policy` | Sequence decode with per-frame/page cancellation |
| `CancellationToken` | Dependency-free cooperative cancellation: `Rc<Cell>` state shared by clones, `cancel()` fires every clone, single-threaded by design |
| `inspect_with_policy`, `decode_with_policy`, `decode_sequence_with_policy` | Apply one caller-selected policy before the corresponding operation |
| `decode_into`, `decode_into_with_policy` | Decode into an exact-size caller-provided destination after rejecting short or oversized buffers without partial writes |
| `ImageInfo::decoded_bytes` | Preflight the exact transfer-byte length from inspection alone; zero-copy destination decode remains future work |
| `TransferLayout` | Minimal decoded byte contract: canvas, mode, row bytes, total bytes, packed-row status, and 1-byte alignment, produced by the same arithmetic as `decode_into` |
| `DecodedImage::try_new`, `try_with_mode`, `try_with_palette` | Validate dimensions, exact mode/color state, pixel length, and indexed palette state while reusing owned pixel buffers; unchecked constructors remain available for staged assembly |
| `encode(&DecodedImage, ImageFormat, &EncodeOptions)` | Validate and encode one image to an explicit target |
| `encode_with_policy`, `encode_sequence_with_policy` | Apply an inclusive complete-result cap and optional cooperative checkpoint budget, returning typed `EncodedOutputBytes` or `EncodeWorkUnits` failures |
| `encode_with_token`, `encode_with_token_and_policy` | Still encode with cancellation before/after encoding; GIF now polls block/frame/coalescing/output-assembly, RGB/RGBA palette quantization, and LZW input-symbol checkpoints, WebP polls preparation, lossy VP8 RGB/RGBA-to-YUV conversion, RGBA transparent-area cleanup after each 1,024 scanned or flattened pixels, analysis/mode-selection/coefficient-probability/8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, and 8,192-bit logical and 16,384-boolean first-partition-bit/8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, and 8,192-bit logical and 16,384-boolean coefficient-bit/1,024-byte boolean-bitstream-output/bitstream stages, lossless VP8L predictor tile scans/mode application, cross-color multiplier search/transform tiles, entropy/transform stages, bounded backward-reference search/match-length/cache/trace, histogram clustering, Huffman-tree/group emission, 8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, 8,192-bit, 16,384-bit, 32,768-bit, 65,536-bit, 131,072-bit, 262,144-bit, 524,288-bit, and 1,048,576-bit logical bitstream intervals, 1,024-byte output, token-stream, codec-result, and metadata-assembly boundaries, PNG and BMP also poll row preparation, PNG stored-block boundaries, 1,024-byte stored-block-copy intervals, and every zlib-ng level's matcher/expansion/Huffman/bitstream/checksum stages, BMP row-conversion subsegments, and structural segments in return and sink paths, JPEG polls RGB-to-YCbCr conversion and chroma-downsample output after each 1,024 pixels, optimized baseline Huffman frequency gathering after each 1,024 AC coefficients, progressive scan block slots after each 1,024 blocks, progressive scan-event frequency items and progressive scan coefficient traversal items after each 1,024 events or coefficients, row/block/scan checkpoints, and 1,024-byte entropy-output intervals, and TIFF polls page preparation, predictor, raw/PackBits/LZW, Deflate input-row, level-six matcher candidate/insertion/fizzle/position, expansion, Huffman, bitstream, stored-block, and checksum boundaries |
| `encode_default(&DecodedImage, ImageFormat)` | Encode one image with format defaults |
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
short entry points. `max_encoded_bytes` is inclusive and checked against the
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

`EncodePolicy::max_work_units` is an independent inclusive bound on the
documented cooperative encode checkpoints. A checkpoint charges one unit
before it continues; when the next charge would exceed the maximum, encoding
returns `ImageError::LimitExceeded` with
`ResourceLimit::EncodeWorkUnits`. The budget is layered over a caller token,
so caller cancellation still has precedence and remains `Cancelled`. TIFF
Deflate tokenization additionally charges at each supplied input-row boundary
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
scanned or flattened RGBA transparent-area cleanup pixels, each batch of 1,024
analyzed macroblocks, and each batch of 1,024 frame-selection macroblocks, then
after color conversion, padding, analysis, segment parameters, mode selection,
coefficient-probability
adaptation, partition emission, each 8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, and 8,192-bit logical first-partition interval,
each 16,384-boolean first-partition-bit interval,
each 8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, and 8,192-bit logical coefficient interval, each 16,384-boolean coefficient-bit
interval, each 1,024-byte boolean-bitstream
output interval, and final container assembly.
JPEG baseline and progressive RGB-to-YCbCr conversion and chroma downsampling
additionally charge after each 1,024 converted or produced pixels, forward-DCT/
quantization charges after each completed 8x8 block, optimized baseline Huffman
frequency gathering charges after each 1,024 AC coefficients, progressive scan
block-slot generation charges after each 1,024 blocks, progressive scan-event
frequency gathering charges after each 1,024 events, progressive scan coefficient
traversal charges after each 1,024 coefficients, and entropy coding charges
after each 1,024 emitted entropy bytes; its no-token path remains on the
ordinary byte producer.
Lossless WebP
VP8L additionally charges around predictor tile scans/mode application,
cross-color multiplier search/transform tiles, entropy analysis, transform
selection/application, bounded backward-reference search/match-length/cache/
trace, histogram clustering, Huffman-tree/group emission, token-stream
intervals, each 8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, 8,192-bit, 16,384-bit, 32,768-bit, 65,536-bit, 131,072-bit, 262,144-bit, 524,288-bit, and 1,048,576-bit logical bitstream interval, and each 1,024-byte
VP8L bitstream-output interval. This is
deterministic work control, not CPU-time,
instruction-count, transient-memory, or recoverable-OOM accounting.

Token-aware encode variants are a separate cooperative work-control boundary.
Still encodes check the token before dispatch and after the codec returns; the
GIF still writer also polls at its block/frame/coalescing/output-assembly and
RGB/RGBA palette quantization, RGB median-cut hash/order, axis-ordering,
split, and partition checkpoints, and fixed RGBA FASTOCTREE cell/bucket/lookup
and bucket-sort checkpoints plus GIF LZW input-symbol intervals, the WebP still writer polls at
preparation, lossy VP8 RGB/RGBA-to-YUV conversion, RGBA transparent-area
cleanup after each 1,024 scanned or flattened pixels, macroblock-analysis, and
mode-selection subsegments plus analysis/coefficient-probability, 8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, and
8,192-bit logical first-partition intervals, 16,384-boolean first-partition-bit intervals,
8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, and 8,192-bit logical coefficient intervals, 16,384-boolean coefficient-bit intervals,
1,024-byte boolean-bitstream output intervals, and bitstream stages, lossless
VP8L
predictor/cross-color/entropy/transform, bounded backward-reference
search/match-length/cache/trace, histogram/Huffman, token-stream, 8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, and
2,048-bit, 4,096-bit, 8,192-bit, 16,384-bit, 32,768-bit, 65,536-bit, 131,072-bit, 262,144-bit, 524,288-bit, and 1,048,576-bit logical bitstream intervals, and 1,024-byte bitstream-output stages, codec-result, and metadata-assembly
boundaries, and the JPEG still writer additionally polls after each 1,024
converted RGB or chroma-downsample output pixel, each 1,024 AC coefficients
during optimized baseline Huffman frequency gathering, each 1,024 progressive
scan block slots, each 1,024 progressive scan coefficient items, and each
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
not roll the prefix back. Progress callbacks, transient working-state
reduction, short-write/rollback cleanup, and interruption beyond the
documented checkpoints—including remaining finer WebP bitstream work beyond the
implemented 8-bit/16-bit/32-bit/64-bit/128-bit/256-bit/512-bit/1,024-bit/2,048-bit/4,096-bit/8,192-bit logical VP8 first-partition and 8-bit/16-bit/32-bit/64-bit/128-bit/256-bit/512-bit/1,024-bit/2,048-bit/4,096-bit/8,192-bit logical VP8 coefficient intervals,
the 16,384-boolean first-partition and coefficient-bit intervals, and the
1,024-byte boolean-bitstream-output
intervals, the 8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, 8,192-bit, 16,384-bit, 32,768-bit, 65,536-bit, 131,072-bit, 262,144-bit, 524,288-bit, and 1,048,576-bit logical VP8L bitstream intervals, and CPU work inside codec
rows other than the implemented PNG adaptive-filter subsegments, BMP
row-conversion subsegments, token-aware PNG stored-block/all-level Deflate
stages, and LZW input-symbol intervals, WebP
RGB/RGBA-to-YUV conversion, RGBA transparent-area cleanup, macroblock-analysis,
and mode-selection subsegments, JPEG optimized-Huffman frequency work beyond
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
`SequenceEncode`, and `flush` is not called. Short-write behavior on other
paths, rollback, and partial-container cleanup remain open. Every current codec
sink writer reports the same structured cause if any validated emitted segment
is rejected.

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

`EncodedImage::new_with_policy` applies the input limit before inspection and
primary-canvas dimension, pixel, decoded-byte, and frame-count limits
immediately afterward. `decode_with_policy` checks encoded bytes and retained
`ImageInfo` before consulting the `OnceLock`: a policy failure is never cached,
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
remaining gap are tracked in the [roadmap](roadmap.md).

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
