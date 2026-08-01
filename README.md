# image-slash-star

[![CI](https://github.com/appunni-m/image-slash-star/actions/workflows/ci.yml/badge.svg)](https://github.com/appunni-m/image-slash-star/actions/workflows/ci.yml)
[![License: multi-license](https://img.shields.io/badge/license-see%20NOTICE-blue.svg)](#license-and-attribution)

Dependency-constrained Rust codecs for detecting, inspecting, decoding, and
encoding JPEG, PNG, GIF, BMP, TIFF, WebP, ICO/CUR, and AVIF bytes.

The crate targets exact observable compatibility with a pinned Pillow 12.2.0
oracle for every active manifest case: success or error, format, mode,
dimensions, metadata, frames, pixels, palettes, and deterministic encoded
bytes.

> **Pre-release status:** version 0.1.0 is not published to crates.io. The
> compatibility guarantee is limited to committed manifest cases, not every
> legal file in each format specification. Encoded-input bytes, inspected
> primary-canvas dimensions/pixels/decoded bytes, the inspected frame count,
> every later frame/page's decoded bytes, cumulative sequence bytes, and the
> encoded metadata extent can be bounded. Encoded-output and internal
> allocations remain outside the policy, with no recoverable out-of-memory
> contract; the current crate should not be treated as hardened for arbitrary
> hostile inputs.
> Breaking API changes may occur before 1.0.

## Why use it?

- One auto-detecting, structured `Result` API across all codecs.
- Rust-only default JPEG, PNG, GIF, BMP, TIFF, WebP, and ICO execution.
- `bytemuck` as the only Cargo dependency.
- Independent per-format Cargo features.
- Exact fixture-backed success, error, pixel, frame, and encoded-byte checks.
- Byte-buffer APIs that work without filesystem or networking assumptions.
- Default codec feature combinations cross-compile to
  `wasm32-unknown-unknown`, and every feature lane executes in a real WASM
  runtime (`wasm32-wasip1` under Node's WASI preview1).

The crate deliberately does not resize, crop, rotate, draw, filter, adjust, or
otherwise process decoded images. Applications keep image processing in a
downstream library.

## Quick start

Until the first registry release, depend on the repository:

```toml
[dependencies.image-slash-star]
git = "https://github.com/appunni-m/image-slash-star"
default-features = false
features = ["png", "jpeg"]
```

Cargo package names use hyphens; Rust imports use underscores.

```rust,no_run
use image_slash_star::{
    decode, encode_default, ImageError, ImageFormat, ImageMode, ImageResult,
};

fn opaque_rgb_png_to_jpeg(input: &[u8]) -> ImageResult<Vec<u8>> {
    let decoded = decode(input)?;
    if decoded.format != ImageFormat::Png {
        return Err(ImageError::Unsupported {
            format: Some(decoded.format),
            message: "expected a PNG source".to_owned(),
            reason: None,
        });
    }
    if decoded.content.mode != ImageMode::Rgb8 {
        return Err(ImageError::Unsupported {
            format: Some(ImageFormat::Jpeg),
            message: "JPEG example requires opaque RGB8 input".to_owned(),
            reason: None,
        });
    }
    encode_default(&decoded.content, ImageFormat::Jpeg)
}
```

`decode` detects the source format from the complete byte slice and preserves
that format separately from the decoded sample mode. Encoding always requires
an explicit output format.

The example deliberately accepts only opaque `Rgb8` PNG pixels. RGBA, indexed,
bilevel, and sixteen-bit PNG inputs need an explicit conversion policy in a
downstream processing library; this codec crate does not silently discard
alpha, expand palettes, or change sample depth. For a full program, read bytes
with `std::fs`, call the function above, and write the returned bytes. The crate
itself never opens paths.

## Supported features

Default features enable every codec except AVIF.

| Feature | Default | Native behavior | `wasm32-unknown-unknown` |
| --- | --- | --- | --- |
| `jpeg` | yes | Rust inspect/decode/encode | Build-verified Rust path |
| `png` | yes | Rust still/APNG sequence decode and still encode | Build-verified Rust path |
| `gif` | yes | Rust still/sequence decode and encode | Build-verified Rust path |
| `bmp` | yes | Rust inspect/decode/encode | Build-verified Rust path |
| `tiff` | yes | Rust still/multipage decode and encode | Build-verified Rust path |
| `webp` | yes | Rust still/sequence decode and still/keyframe-sequence encode | Build-verified Rust path |
| `ico` | yes | Rust ICO/CUR inspect/decode and source-sized ICO encode | Build-verified Rust path |
| `avif` | no | Fixed native inspect/decode/sequence/encode stack | Portable inspect and restricted still decode; sequence decode and encode unsupported |

The `ico` feature recognizes both ICO and CUR signatures and accepts `.ico`
and `.cur` aliases. Inspection and decode retain the selected CUR hotspot in
`ImageInfo::cursor_hotspot` and `DecodedImage::cursor_hotspot`; `None`
distinguishes ordinary ICO. The feature enables PNG and BMP because an entry
can use either representation. Encoding currently writes ICO only, with one
entry at the supplied raster dimensions, and never resizes pixels.

Feature evolution rule: format umbrella features (`jpeg`, `png`, `gif`, `bmp`,
`tiff`, `webp`, `ico`, `avif`) are stable public Cargo API. Any future
operation-level subfeature must be additive: it may only narrow an umbrella's
optional surface, never disable behavior that a subset of features already
enables, and Cargo's additive unification must compose subfeatures without
changing umbrella semantics. This rule is committed before any split is
accepted.

WASM feature combinations are cross-compiled in CI. The feature-gate and
capability-table suites also execute in a real WASM runtime
(`wasm32-wasip1` under Node's WASI preview1) for no features, every isolated
codec, default features, and all features. Executing the complete semantic
fixture matrix in a WASM runtime remains planned.

AVIF is the remaining portability boundary. Native parity uses fixed
libavif 1.4.1, dav1d 1.5.3, and libaom 3.13.2 builds. The WASM path has a
growing in-tree AV1 subset. See [AVIF support](docs/avif.md) for exact
capabilities and setup.

## API and data model

| API | Purpose |
| --- | --- |
| `detect_format(&[u8])` | Identify a supported container signature |
| `detect_prefix(&[u8])` | Incremental detection: identify a complete signature, or report `NeedMoreData { minimum }` while the input is still an incomplete prefix |
| `inspect(&[u8])` | Read `ImageInfo` without decoding compressed pixels |
| `inspect_basic(&[u8])` | Read header facts without counting every frame/page; `frame_count_complete` reports whether the count is known |
| `inspect_basic_prefix(&[u8])` | Incremental basic inspection: return header facts as soon as the detected format can prove them, or report `NeedMoreData { minimum }` while the basic header is incomplete |
| `decode(&[u8])` | Decode the still/first-image view and retain source format |
| `decode_prefix(&[u8])`, `decode_prefix_with_policy` | Incremental still decode: return the decoded image when the input is complete, or `NeedMoreData { minimum }` while structures are still incomplete |
| `decode_with_token(&[u8], &CancellationToken)`, `decode_with_token_and_policy` | Still decode with cooperative cancellation at structural checkpoints |
| `decode_sequence(&[u8])` | Retain supported frames and presentation metadata |
| `decode_sequence_prefix(&[u8])`, `decode_sequence_prefix_with_policy` | Incremental sequence decode with the same non-terminal status |
| `decode_sequence_with_token(&[u8], &CancellationToken)`, `decode_sequence_with_token_and_policy` | Sequence decode with per-frame cancellation |
| `Decoded<T>::diagnostics` | Stable non-fatal recovery records returned beside successful decode |
| `inspect_with_policy`, `decode_with_policy`, `decode_sequence_with_policy` | Apply caller-controlled limits before the corresponding operation |
| `decode_into`, `decode_into_with_policy` | Decode into an exact-size caller-provided buffer, rejecting short/oversized destinations without partial writes |
| `ImageInfo::decoded_bytes` | Preflight the exact transfer-byte length from the inspected canvas and mode without decoding |
| `ImageInfo::transfer_layout`, `DecodedImage::transfer_layout` | Describe row bytes, total bytes, packed-row status, and alignment for the decoded contract |
| `encode(&DecodedImage, ImageFormat, &EncodeOptions)` | Encode one image with explicit options |
| `encode_with_policy`, `encode_sequence_with_policy` | Apply an inclusive encoded-result cap and return a typed `EncodedOutputBytes` limit failure when the complete result is too large |
| `encode_with_token`, `encode_with_token_and_policy` | Encode one image with cooperative cancellation at the public codec boundary |
| `encode_default(&DecodedImage, ImageFormat)` | Encode one image with defaults |
| `encode_sequence(&DecodedSequence, ImageFormat, &EncodeOptions)` | Encode one frame to any enabled format or multiple frames to GIF, TIFF, WebP, or native AVIF |
| `encode_sequence_with_token`, `encode_sequence_with_token_and_policy` | Encode a still/sequence with cancellation at retained-frame and finalization checkpoints where the target supports them |
| `encode_to_sink_with_policy`, `encode_sequence_to_sink_with_policy` | Apply the same encoded-result cap before writing to a caller-owned sink; a rejected result leaves the sink untouched |
| `encode_to_sink`, `encode_sequence_to_sink` | Encode into a caller-owned dependency-free `OutputSink`; sink rejection is reported as `ImageError::OutputWrite`; PNG and BMP still output cross structural write boundaries |
| `encode_to_sink_with_token`, `encode_sequence_to_sink_with_token` | Combine token-aware encoding with a caller-owned sink; structural writers can stop after an already-written prefix when cancellation fires |
| `ImageFormat::capabilities()` | Query detection, inspection, still, and genuine multi-image support for the current feature set and target |
| `all_capabilities()` | Return the same typed capability record for every public format |
| `EncodedImage::new(bytes)` | Inspect an immutable source now and decode it lazily |
| `EncodedImage::*_with_policy(...)` | Enforce the same limits during source construction or lazy materialization |
| `EncodedImage::verify_with_scope(scope)` | Verify with an explicit requested strength; stronger requests fail instead of downgrading |
| `EncodedImageView::new(&[u8])` | Borrow an immutable encoded view with the same inspect/verify/decode operations and no copy or cache |
| `EncodedImage::decode_frame(index)`, `EncodedImageView::decode_frame(index)` | Decode exactly one retained frame/page with stable per-frame errors; TIFF uses a genuine per-page path |

`Decoded::consumed_bytes` reports the encoded bytes of the container-defined
extent when the container defines one unambiguously (JPEG after EOI, PNG after
IEND, GIF after the trailer, WebP's RIFF size, TIFF's final IFD, and AVIF's
last top-level box). BMP and ICO report `None` because they declare no total
extent. Decoders ignore well-formed trailing bytes after that extent and never
let them change the decoded result. Successful envelopes for formats with a
defined extent also report `DiagnosticKind::TrailingDataIgnored` with the
first ignored byte's offset. The trailing-input manifest pins the unchanged
Pillow-observable result, while the consumed extent and diagnostic fields are
the separate defensive-model contract for all eight formats.

Incremental callers that are still receiving encoded input use `detect_prefix`
and `inspect_basic_prefix`. Both return `ImageError::NeedMoreData { minimum }`
when the input is an incomplete prefix: append enough bytes to reach
`minimum` (the exact total input length the next parse needs) and retry.
Minimums are exact for fixed signatures and progress-aware for containers
that declare their own extent (WebP RIFF chunks and AVIF boxes). Every other
result is terminal: an incomplete signature that can never match is
`UnknownFormat`, and a recognized-but-truncated container remains `Malformed`
on the complete-slice APIs. The incremental surface never turns a terminal
result into an implicit retry loop.
The incremental contract now extends to decoding: `decode_prefix` and
`decode_sequence_prefix` (plus their policy variants) return the decoded
result once the input is complete and `NeedMoreData { minimum }` while
container structures or pixel payloads are still incomplete. Minimums are
exact when the container declares the missing extent (PNG chunks, BMP/ICO
pixel spans, TIFF strip/tile spans, WebP RIFF payloads, AVIF boxes) and
progress-aware otherwise.
`CancellationToken` adds cooperative cancellation: clones share state,
`cancel()` fires every clone, and token-aware decodes poll at chunk, frame,
page, strip, and tile boundaries, stopping with `ImageError::Cancelled`
without publishing partial state. Token-aware encode APIs check before and
after whole-buffer still codecs; PNG and BMP still sink encoding also poll
while preparing rows and between emitted segments, while GIF, TIFF, WebP, and
native AVIF sequence paths poll at their frame/coalescing/page/finalization
boundaries. A structural sink cancellation may leave its delivered prefix;
progress callbacks, work-budget exhaustion, and universal structural writing
remain separate roadmap work. Legacy APIs never cancel.

Signature detection is feature-independent. Disabled codec operations report
`Unavailable(FeatureDisabled)` through capability discovery and return
`ImageError::FeatureDisabled` when attempted. Sequence capabilities mean
genuine multi-image decode or encode; the validated one-frame fallback follows
the corresponding still capability. On `wasm32`, AVIF inspection remains
manifest-bounded, still decode reports the restricted portable subset, and
encode plus sequence operations report target unavailability.

`ImageFormat::from_name` accepts canonical names and extension aliases
case-insensitively: JPEG `jpg`/`jpeg`/`jfif`/`jpe`, PNG `png`/`apng`, TIFF
`tiff`/`tif`, ICO/CUR `ico`/`cur`, and AVIF `avif`/`avifs`. Headerless `.dib`
remains an explicit-format scope decision, not an automatic BMP alias.
`mime_type()`, `canonical_extension()`, and `extensions()` expose stable,
dependency-free format metadata in canonical-first order; `from_path` uses the
same table without touching the filesystem.

`VerificationScope` orders `HeaderOnly` < `Structure` < `FullPixels`.
`EncodedImage::verify()` runs the format's Pillow-compatible default scope;
`verify_with_scope(requested)` fails with a format-qualified `Unsupported` when
the codec cannot provide the requested strength, so header-only success is
never silently reported as structural or full-pixel evidence. No codec
currently provides `FullPixels`.

The core model separates:

```text
ImageFormat                ImageMode + ColorType
encoded container         decoded sample-byte layout
PNG / JPEG / GIF / ...    P8 / L8 / RGB8 / RGBA8 / ...
             \             /
              Decoded<T>
```

`DecodedImage::pixels` is tightly packed and row-major. Indexed samples use
`ImageMode::P8` and retain an `ImagePalette` when the source exposes one; they
are not silently interpreted as grayscale. Caller-built images and sequences
are validated before encoding.

`ImageInfo::source` and `DecodedImage::source` retain structural source facts
without changing the transfer bytes. TIFF currently records its exact
`SourceByteOrder`; `I32`/`F32` pixels preserve that order, while normalized
modes keep their documented transfer layout. AVIF primary-item `irot`/`imir`,
`pasp`, and `clap` properties are retained through
`SourceDescriptor::avif_transform()` as source provenance; decoded pixels are
never rotated, mirrored, rescaled, or cropped. Codecs without a retained
structural fact currently return an empty descriptor.

`DecodedSequence::first()` returns the complete `DecodedFrame`, including its
source and presentation metadata. `first_image()` is available when a caller
intentionally wants only the first frame's pixels and accepts that metadata
loss.

`DecodedSequence::kind` names the container meaning: `TimedAnimation` for GIF,
APNG, animated WebP, and AVIF sequences; `UntimedPages` for TIFF multipage
sequences; and `SingleFrame` for still decode fallbacks and caller-built still
sequences. TIFF pages always retain exact zero durations and are never
described as timed animation.

`SourceDescriptor::alpha()` reports the alpha association declared by the
encoded container: straight/unassociated alpha (PNG alpha channels and palette
tRNS, WebP VP8X/VP8L alpha, TIFF `ExtraSamples` 2, AVIF alpha items), TIFF
`ExtraSamples` 1 as premultiplied/associated, GIF transparency as a binary
mask, and a reserved auxiliary variant for future separate alpha channels.
Decoded transfer bytes remain the documented normalized unassociated layout;
the descriptor records only what the source declares.

Decoded images and sequences retain `OpaqueBlock` records for container blocks
the codec does not interpret, in original order with duplicates and the
container's safe-to-copy flag (currently PNG unknown ancillary chunks).
Known PNG metadata chunks (text, EXIF, time, and resolution blocks) are
retained separately as raw, unparsed `OpaqueMetadata` records; compressed
payloads are bounded-validated but never exposed inflated. Pillow-tolerated
invalidly compressed `zTXt`, `iCCP`, and `iTXt` payloads are omitted and
produce `DiagnosticKind::InvalidMetadataIgnored`; malformed field shapes stay
raw metadata. Method-only `zTXt`/`iCCP` mutations are outside this recovery
contract because Pillow rejects them.
GIF comment, plain-text, and non-NETSCAPE application extensions are retained
the same way (label byte as kind, exact payload bytes as data), while unknown
extension labels stay in `opaque_blocks` and the NETSCAPE loop extension
remains interpreted into `loop_count`.
JPEG APPn and COM marker payloads are retained as ordered metadata records
(marker byte as kind, exact payload bytes as data), including multi-segment
ICC/EXIF fragments in stream order; the APP14 Adobe transform byte stays
parsed for CMYK decoding.
WebP ICCP, EXIF, and XMP chunks are retained as ordered metadata records
(fourcc as kind, exact payload bytes as data, duplicates kept), while unknown
RIFF chunks stay in `opaque_blocks`; truncated chunks are not retained.
TIFF tag retention preserves every non-interpreted tag with typed identity
(tag number in the file's byte order) and exact stored value bytes — inline
when the value fits four bytes, otherwise at its offset — with unknown tags in
`opaque_blocks` and known metadata tags (text, date, software, artist,
copyright, ICC) in the metadata records, per page.
AVIF top-level BMFF retention keeps unknown boxes and `free`/`skip` padding
boxes as raw opaque records (fourcc as kind, full box bytes as data) while
interpreted boxes (ftyp/meta/moov/mdat) stay out.
Recognized AVIF `Exif` items and MIME items whose content type is exactly
`application/rdf+xml` are retained as ordered raw `OpaqueMetadata` records on
still and sequence decode (`Exif` and `XMP ` kinds). Their item extent bytes
are preserved exactly; the EXIF record therefore includes the AVIF TIFF-offset
prefix. This is source retention only: default encoding never replays it, and
non-primary/auxiliary item relationships and other item metadata remain open.
Exact PNG color fields additionally surface through `source_color`
(`SourceColor`): sRGB rendering intent, gamma, chromaticity values, and the
raw ICC profile bytes. Retaining them records what the source declares; it
never implies that color conversion was applied to decoded samples.
AVIF primary items likewise retain `colr`/`nclx` CICP fields, the `av1C`
chroma sample position, and the `clli` content-light-level property through
`SourceColor` (primaries, transfer characteristics, matrix coefficients,
range, maxCLL, and maxPALL) on
inspection and decode. This is source provenance, not color conversion or tone
mapping; the item-level AVIF color contract is defensive/specification
evidence because the Pillow parity oracle does not expose an equivalent
structured result. Chroma sample position is retained as source provenance
only; it does not cause chroma resampling.
Default encoding never replays retained blocks implicitly; an explicit replay
API would have to define collisions with encoder-generated blocks first.

Codec/capability vocabulary enums are non-exhaustive, including `ImageFormat`,
`VerificationScope`, `ImageMode`, `SequenceKind`, `SourceAlpha`, and animation
presentation enums. Downstream `match` expressions must include a fallback so
a later format or transfer mode does not become an accidental source break.
Closed domains such as `SourceByteOrder` remain exhaustive.

### Typed encoder options

`EncodeOptions` always identifies one target codec. Construct the corresponding
record directly or use `EncodeOptions::for_format` for that format's defaults:

```rust
use image_slash_star::{
    encode, EncodeOptions, ImageFormat, JpegEncodeOptions, JpegSubsampling,
};

# fn example(image: &image_slash_star::DecodedImage)
#     -> image_slash_star::ImageResult<Vec<u8>> {
let options = EncodeOptions::from(JpegEncodeOptions {
    quality: Some(90),
    subsampling: Some(JpegSubsampling::Cs444),
    ..JpegEncodeOptions::default()
});
encode(image, ImageFormat::Jpeg, &options)
# }
```

Passing JPEG options with a PNG target, for example, returns a
format-qualified `Parameter` error before codec dispatch. There is no
format-neutral `EncodeOptions::default()` because codec defaults and option
domains are not interchangeable.

`EncodeOptions::try_from_legacy_pairs` is a strict migration boundary for the
former string-pair configuration. It rejects unknown and duplicate keys,
validates each value, and produces a typed record; encoders never inspect
string keys. New integrations should construct codec records directly.

### Caller-controlled limits

The unlimited entry points remain convenient for trusted inputs.
`DecodePolicy` provides inclusive maxima for the complete encoded byte slice,
inspected canvas width, height, and pixel count, and the primary image's
decoded transfer-byte length, the inspected frame/page count, every later
frame/page's decoded byte length, and the cumulative retained sequence bytes:

```rust
use image_slash_star::{decode_with_policy, DecodePolicy, ImageResult};

fn decode_at_most_one_mebibyte(
    input: &[u8],
) -> ImageResult<image_slash_star::Decoded<image_slash_star::DecodedImage>> {
    let policy = DecodePolicy::new()
        .with_max_encoded_bytes(1024 * 1024)
        .with_max_width(4096)
        .with_max_height(4096)
        .with_max_pixels(16_000_000)
        .with_max_primary_decoded_bytes(64 * 1024 * 1024)
        .with_max_frames(1000)
        .with_max_frame_decoded_bytes(4 * 1024 * 1024)
        .with_max_sequence_decoded_bytes(256 * 1024 * 1024)
        .with_max_metadata_bytes(8 * 1024 * 1024);
    decode_with_policy(input, &policy)
}
```

The encoded-byte check occurs before signature detection and codec parsing. An
oversized input returns a typed `LimitExceeded` error with the operation,
`ResourceLimit::EncodedBytes`, configured maximum, and observed length. It has
no selected format because no format parsing occurred.

Canvas limits use exact `ImageInfo` width, height, `width × height`, mode, and
primary decoded byte length. Packed `L1` rows are byte-aligned; other modes use
their exact transfer bytes per pixel. They run after format-qualified
inspection and before primary pixel materialization, so their errors retain
the selected format. Policy-aware direct decode may inspect then parse again;
unlimited wrappers do not gain that additional pass.

`max_frames` uses the exact inspected frame/page count and runs after the
canvas checks but before sequence materialization. Inspection and sequence
decode reject a source whose declared count exceeds the maximum; still decode
and lazy still materialization retain exactly one frame, so only a zero frame
maximum rejects them. Sources whose inspection cannot prove an exact count
remain unlimited for this resource.

`max_frame_decoded_bytes` and `max_sequence_decoded_bytes` apply inside every
sequence decoder before the next frame's pixel work: the per-frame limit
rejects any later frame/page whose transfer-byte length exceeds the maximum,
and the cumulative limit charges the inspected primary first and rejects
before the frame whose addition would exceed the total. Both failures retain
the format and typed resource; the primary and still-only paths remain bounded
by the primary-canvas limits.

`max_metadata_bytes` bounds the encoded metadata extent — every encoded byte
that is not primary pixel payload data — measured by a per-format container
scan before inspection or pixel work on all five policy paths.
For AVIF, the scan includes item metadata payloads stored in `mdat` and
subtracts only sample spans referenced by the decoded primary/auxiliary planes.

`EncodePolicy::max_output_bytes` is the encode-side result-admission limit. It
is inclusive and applies to still and sequence encodes, including their sink
wrappers: the complete encoded length must fit before it is returned or the
first sink write, or the operation returns `LimitExceeded` with
`ResourceLimit::EncodedOutputBytes`. Whole-buffer codecs still build their
complete `Vec<u8>` first. The PNG and BMP still sink paths preflight their
complete lengths, then emit validated container structures without assembling a
second final `Vec<u8>`; PNG's filtered rows and compressed payload remain
transient working allocations, while BMP prepares bounded palette/row segments.
Neither path yet provides a transient-allocation cap, recoverable OOM behavior,
or universal incremental encoding.

`inspect_with_policy`, `decode_sequence_with_policy`,
`EncodedImage::new_with_policy`, and `EncodedImage::decode_with_policy` use the
same boundary. A rejected lazy decode is not cached, and an already cached
decode cannot bypass a later stricter policy. This is not yet a complete
hostile-input budget: transient encoded-output and other internal allocation
behavior remain outside the policy, and no recoverable allocation-failure
contract exists.

See [architecture and public contract](docs/architecture.md) for byte layouts,
validation invariants, lazy source lifecycle, memory behavior, feature
dispatch, and internal boundaries. Generate declaration-level API
documentation with:

```bash
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --locked
```

## Errors

Every canonical fallible API returns `ImageResult<T>`.

| Error | Meaning |
| --- | --- |
| `UnknownFormat` | No supported signature matched |
| `FeatureDisabled` | The format is recognized but its Cargo feature is off |
| `Malformed` | The selected codec rejected the encoded bytes |
| `Unsupported` | The requested operation or valid input class is unavailable |
| `Dimensions` | Dimensions, frame bounds, or sample length are invalid |
| `Parameter` | An option, palette, mode combination, or other parameter is invalid |
| `LimitExceeded` | A caller-configured resource maximum was exceeded |
| `NeedMoreData` | An incremental prefix is incomplete and reports the minimum total input length for retry |
| `Cancelled` | A token-aware decode or encode stopped at a cooperative checkpoint |
| `OutputWrite` | A caller-owned encoded-output destination rejected an emitted segment |

Codec-dispatched failures additionally report the public operation that
produced them through `ImageError::stage()` (`Inspection`, `StillDecode`,
`StillEncode`, `SequenceDecode`, `SequenceEncode`, or `Verification`).
Caller-built validation and option-construction errors remain stage-free;
`UnknownFormat`, `FeatureDisabled`, and `LimitExceeded` keep their existing
contracts (`LimitExceeded` already carries the typed operation). Sink failures
from `encode_to_sink` and `encode_sequence_to_sink` carry the selected output
format and encode stage through `OutputWrite`; their offset and identity are
`None` because the failure is on the destination side. Whole-buffer codecs
still write one complete validated buffer, while the PNG and BMP still paths
write validated structural segments. Short-write, flush, and structural
cleanup semantics remain future incremental-writer work.

Where the parser can name the failing container structure, codec-dispatched
errors also report the encoded-input byte offset (`ImageError::offset()`) and
a stable structure identity (`ImageError::identity()`, for example
`png_chunk`, `jpeg_marker`, or `tiff_ifd`). BMP header, palette, pixel-span,
bitfield, and RLE failures additionally expose stable BMP identities. ICO
header, directory, entry-range, and embedded PNG/DIB/CUR failures likewise
expose stable ICO identities. WebP inspection/container-chunk failures expose
stable WebP identities; WebP bitstream decode internals remain detail-free.

`ImageError` is non-exhaustive; downstream `match` expressions need a fallback
arm. Unchanged malformed bytes should not be retried. Feature and unsupported
errors can usually be handled by selecting another compiled capability.
`ImageError::unsupported_reason()` additionally distinguishes
`TargetUnavailable` and `NotImplemented` when the failure is a capability
boundary; it returns `None` for input-class and metadata incompatibilities.
This Rust-only reason is not Pillow-parity evidence.

Non-fatal recovery is not an error: successful `Decoded<T>` values expose
`diagnostics` with stable kind, stage, offset, and structure identity fields.
The diagnostic fixture contract is intentionally separate from Pillow parity,
because Pillow has no equivalent structured warning field.

Use `error.kind()` for stable recovery policy and `error.format()` for the
selected input/output format when one is known. `error.message()` returns
retained high-level codec/parameter/destination diagnostics; `LimitExceeded` instead
exposes typed fields directly and has no prose message. In particular,
`Dimensions` and `Parameter` retain both optional format and diagnostic
context; callers do not need to parse `Display` output. Diagnostic prose may
become more specific, so it is not a substitute for `ImageErrorKind`.

## Correctness evidence

The generated matrix in this tree contains 1,417 active cases:
1,024 decode/inspect/verify cases and 393 encode cases, with zero
planned or unwired rows. Expected errors are active fixture outcomes, and
every decode-error class is catalogued in a generated, CI-checked
malformed-class ledger with Pillow outcome, Rust error contract, evidence
origin, and specification status.

Runtime capability tables for every feature lane are emitted per target and
committed as `tests/fixtures/capability_tables.json`; CI regenerates them in
memory and rejects drift between the native host and `wasm32-wasip1` tables
and the committed fixture.
Encoded bytes and decoded pixels for a fixed encoder/decoder subset are also
SHA-256-pinned in `tests/fixtures/determinism.json`, and the same test runs
natively and in the WASM runtime so cross-target output stays byte-identical.

The accepted Coverage MCP snapshot for that implementation state reports 100%
line, branch, function, and region coverage. Coverage proves execution under
the retained suite; it does not prove complete format support or security.

The oracle identity, regeneration workflow, exact comparison contract, test
tiers, current run identifiers, and troubleshooting are in
[oracle, fixtures, tests, and coverage](docs/testing.md).

## Build from source

The Rust version and WASM target are pinned in `rust-toolchain.toml`.

```bash
git clone https://github.com/appunni-m/image-slash-star.git
cd image-slash-star
cargo check --locked
```

Native builds using default features need no external codec library. Enabling
`avif` requires the exact native stack described in
[AVIF support](docs/avif.md#native-setup).

## Documentation

- [Architecture and public contract](docs/architecture.md)
- [Oracle, fixtures, tests, and coverage](docs/testing.md)
- [AVIF support and portability boundary](docs/avif.md)
- [Roadmap](docs/roadmap.md)
- [Changelog](CHANGELOG.md)
- [Contributing](CONTRIBUTING.md)
- [Security policy](SECURITY.md)
- [Third-party provenance](third_party/README.md)

Current behavior belongs in the README, architecture reference, rustdoc, and
testing contract. Planned work belongs only in the roadmap. Historical
investigation logs remain available through Git history rather than the active
documentation tree.

## Support, contributing, and security

Use [GitHub issues](https://github.com/appunni-m/image-slash-star/issues) for
questions, non-sensitive bugs, and feature proposals. Include the commit,
target, enabled features, smallest non-sensitive fixture, expected Pillow
result, and actual Rust result.

Read [CONTRIBUTING.md](CONTRIBUTING.md) before changing codecs, fixtures, or
ported code. The repository requires strict Clippy, exact manifest parity,
feature/target checks, complete retained coverage, and provenance updates.

Report vulnerabilities privately as described in
[SECURITY.md](SECURITY.md). Do not publish exploit details or malicious
fixtures in an issue.

## License and attribution

Original project code is available under your choice of Apache-2.0 or MIT.
The combined distribution also includes source-derived portions under
BSD-2-Clause, BSD-3-Clause, Zlib, IJG, and MIT-CMU terms.

[NOTICE.md](NOTICE.md) maps repository paths to applicable terms.
[third_party/README.md](third_party/README.md) records exact upstream versions,
revisions, hashes, roles, and retained texts. The root [PATENTS](PATENTS) file
contains the Alliance for Open Media patent license required for AV1
distribution.

This software is based in part on the work of the Independent JPEG Group.
