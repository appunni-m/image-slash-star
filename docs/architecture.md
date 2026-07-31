# Architecture and public contract

Status: current implementation reference

Reviewed: 2026-08-01 against the working tree based on `c430132`

This document explains the stable mental model and ownership boundaries of
`image-slash-star`. The generated Rust API documentation remains the
declaration-level reference.

## What the crate owns

`image-slash-star` is a byte-oriented image codec library. It owns:

- encoded-image signature detection;
- header and container inspection;
- validated still-image and sequence decoding;
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
    │                                      └─ ImageMode + ColorType + pixels
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
decoded page. Other codecs currently return an empty descriptor. A source
descriptor is structural provenance, not opaque ICC/EXIF/XMP metadata and not
an instruction to reinterpret every normalized pixel buffer.

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

`SourceDescriptor::alpha()` records the alpha association declared by the
encoded source: straight (PNG, WebP, AVIF, TIFF `ExtraSamples` 2),
premultiplied (TIFF `ExtraSamples` 1), binary mask (GIF transparency), or the
reserved auxiliary class. It never changes decoded transfer bytes, which stay
the documented normalized unassociated layout unless a codec explicitly
retains source-order bytes.

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
`OpaqueMetadata` records; compressed payloads are never inflated, so no
decompression limit is needed before retention. Semantic parsing of text and
ICC payloads remains future work under explicit limits.

Exact PNG color fields are retained in `source_color` (`SourceColor`): the
sRGB rendering intent, the gAMA value (scaled by 100,000), the eight cHRM
chromaticity values, and the raw iCCP profile (keyword plus method/profile
payload, never inflated). The first well-formed occurrence of each chunk is
parsed; duplicates and malformed payloads fall back to raw metadata records.
Retaining color metadata never implies that color conversion was applied.

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
| `inspect(&[u8])` | Read `ImageInfo` without materializing compressed pixels |
| `decode(&[u8])` | Auto-detect and decode the still/first-image view |
| `decode_sequence(&[u8])` | Auto-detect and retain every supported frame plus presentation metadata |
| `inspect_with_policy`, `decode_with_policy`, `decode_sequence_with_policy` | Apply one caller-selected policy before the corresponding operation |
| `encode(&DecodedImage, ImageFormat, &EncodeOptions)` | Validate and encode one image to an explicit target |
| `encode_default(&DecodedImage, ImageFormat)` | Encode one image with format defaults |
| `encode_sequence(&DecodedSequence, ImageFormat, &EncodeOptions)` | Encode one frame to any enabled format or multiple frames to GIF, TIFF, WebP, or native AVIF |
| `ImageFormat::capabilities()` | Describe operation availability for one format in the current build |
| `all_capabilities()` | Return the same typed record for every public format in stable order |
| `EncodedImage::new(bytes)` | Snapshot encoded bytes, inspect immediately, and defer decoding |

`detect_format` recognizes all eight container signatures even when a codec
feature is disabled. An operation that requires a disabled codec returns
`ImageError::FeatureDisabled`. For AVIF, `avif` and `avis` major brands are
direct signatures; generic `mif1`/`msf1` majors additionally require an
`avif` or `avis` compatible brand in the complete bounded `ftyp` box.

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
sequence memory, metadata, codec work, other allocations, or encoded output.

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
and AVIF box scan minus `mdat` payloads). The scan runs after detection and
before any inspection preflight on all five policy paths, so an oversized
metadata extent is rejected before codec work begins; malformed containers
propagate their structured codec error from the scan.

### Codec work is bounded by the resource set

Every work dimension of the current codecs is bounded by one of the typed
resources above, so no codec work can grow independently of output size:

| Codec | Work dimension | Bounding resource |
| --- | --- | --- |
| PNG | chunk scan and count | `max_encoded_bytes` + `max_metadata_bytes` |
| PNG | IDAT/fdAT inflation and scanline filtering | `max_pixels`/`max_primary_decoded_bytes` (inflated length equals canvas bytes) |
| GIF | LZW decompression and deinterlace | `max_pixels`/`max_primary_decoded_bytes`/`max_frame_decoded_bytes` |
| GIF | extension and block walk | `max_encoded_bytes` + `max_metadata_bytes` |
| JPEG | marker scan and progressive scan count | `max_encoded_bytes` (entropy spans are inside the input) |
| TIFF | directory walk | `max_metadata_bytes` |
| TIFF | strip/tile decompression and predictors | `max_frame_decoded_bytes`/`max_sequence_decoded_bytes` |
| WebP | RIFF chunk walk | `max_metadata_bytes` |
| WebP | VP8/VP8L/ALPH decompression | `max_pixels`/`max_primary_decoded_bytes` |
| ICO | directory walk and entry decode | `max_metadata_bytes` + `max_pixels`/`max_primary_decoded_bytes` |
| AVIF | top-level box walk | `max_metadata_bytes` |
| AVIF | AV1 tile/block reconstruction | `max_encoded_bytes` + canvas limits (portable classes have fixed extents) |

The boundary manifests exercise each resource at below/at/above and
`u64::MAX`/`u32::MAX` extremes on small assets; a future codec or container
feature that introduces a work dimension outside this set must add a typed
limit before acceptance. Caller-visible policy options that shape results
(lenient-versus-strict parsing, requested output mode) are result policy, not
resource limits, and belong with the API-029/033 family.

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
hostile input is the checked preflight above rather than allocation-error
recovery. This is the retained QA-015 decision: near-limit arithmetic is
fixture-proven without enormous allocations, and no public API promises a
recoverable allocation failure.

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

`DecodedImage::new` and `DecodedImage::with_mode` record caller-supplied
buffers without validating them. `DecodedImage::validate`, every encoder, and
sequence validation reject:

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
path, and AVIF boxes. BMP, ICO, and WebP decode internals intentionally remain
detail-free. Both fields are stable recovery data, never prose.

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

`ImageError::kind()` is the stable recovery category and
`ImageError::format()` identifies the selected codec when one exists.
`Dimensions` and `Parameter` retain optional format plus the high-level
diagnostic that crossed the codec boundary. `ImageError::message()` exposes
that diagnostic for logs; its prose may become more specific and is not a
commitment to preserve every internal parser phrase as public API.

## Immutable source lifecycle

`EncodedImage::new` converts input into an `Arc<[u8]>`, detects the format, and
inspects the header. It does not decode pixels.

The first call to `decode()` initializes a shared `OnceLock`:

- clones observe the same encoded-byte snapshot and metadata;
- successful materialization is reused;
- deterministic decode failures are cached too;
- `is_decoded()` is true only for a cached success; and
- `verify()` runs independently and does not populate or modify the decode
  cache.

`EncodedImage::new_with_policy` applies the input limit before inspection and
primary-canvas dimension, pixel, decoded-byte, and frame-count limits
immediately afterward. `decode_with_policy` checks encoded bytes and retained
`ImageInfo` before consulting the `OnceLock`: a policy failure is never cached,
a later sufficient policy can initialize the ordinary cache, and an earlier
cached success cannot bypass a later stricter policy. The policy is per
operation rather than permanently attached to the source.

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

The public API is whole-buffer based:

- inputs are borrowed byte slices or immutable encoded snapshots;
- decoded pixels and encoded output allocate complete buffers;
- sequence APIs retain complete supported frame data; and
- no streaming reader/writer interface exists.

Caller-controlled decode limits are not implemented. The current crate should
not be described as hardened for arbitrary hostile inputs. Resource limits are
a release-blocking item in the [roadmap](roadmap.md).

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
