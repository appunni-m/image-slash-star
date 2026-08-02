# Oracle, fixtures, tests, and coverage

Status: current contributor reference

Reviewed: 2026-08-02 on the committed tree based on revision `775263335df9680e4c453f666708745f53083e8f`

Correctness in this repository means matching a fixed Pillow oracle for every
active manifest case. It does not mean that tests or coverage prove complete
format support, production readiness, or security.

## Sources of truth

Use these sources in order:

1. `pillow-oracle.lock.yaml` fixes the Python, platform, wheel, extension, and
   bundled codec identities.
2. `manifest.yaml` declares public input/output and error cases.
3. `tests/fixtures/coverage_matrix.json` is the generated executable projection
   of the manifest.
4. `tests/fixtures/encode_option_acceptance_manifest.json` and
   `tests/fixtures/encode_option_error_manifest.json` define the strict
   legacy-pair migration boundary for typed encoder options.
5. `tests/fixtures/decode_policy_manifest.json` defines caller-controlled
   policy boundaries that Pillow does not expose.
6. `tests/fixtures/diagnostic_manifest.json` defines non-fatal diagnostic
   fields whose stable Rust representation has no Pillow result field.
7. `tests/fixtures/outputs/` contains exact expected metadata, pixels, frames,
   entropy traces, and encoded files.
8. The Rust integration harnesses execute those contracts.

Implementation comments and prose do not override the generated fixture
contract.

Every active row also records the semantic target/feature lane, assertion
origins, and SHA-256 for its source plus every retained pixel, palette, frame,
or encoded artifact. The Rust harness recomputes those hashes independently;
generated lengths or paths alone are not accepted as provenance.
Each row also declares the expected result of every applicable public
operation. A decode row covers detect, inspect, verify, still decode, and
sequence decode; an encode row separately classifies still and sequence encode
as success, error, or not applicable.

## Pinned oracle

The primary oracle is:

| Component | Fixed identity |
| --- | --- |
| Pillow | 12.2.0 |
| Python | CPython 3.12 |
| Wheel profile | macOS 11 arm64 |
| Wheel SHA-256 | recorded in `pillow-oracle.lock.yaml` |
| Imaging extension SHA-256 | recorded in `pillow-oracle.lock.yaml` |

Bundled codec versions are also pinned, including libjpeg-turbo 3.1.4.1,
zlib-ng 2.3.3, libtiff 4.7.1, libwebp 1.6.0, libavif 1.4.1, dav1d 1.5.3, and
libaom 3.13.2.

The generator refuses to replace references when the environment identity does
not match the lock. A different wheel is a different oracle.

AVIF detection has one deliberate evidence split. Pillow's 16-byte plugin
predicate admits any `mif1`/`msf1` major brand so its libavif backend can make
the complete decision. The common Rust detector has no plugin-fallthrough
stage, so the generated detection result instead follows AVIF v1.1 and pinned
libavif: generic HEIF major brands require `avif` or `avis` in the complete
bounded `ftyp` compatible-brand list. Pillow still supplies the exact final
open, inspect, verify, and load outcomes for those same full-file fixtures.

## What parity compares

Depending on the manifest row, the harness compares:

- format detection;
- explicit success or error for every applicable public operation;
- inspection status and metadata;
- verification status;
- success or error outcome;
- exact Pillow exception type/message where the oracle raises an exception;
- separate stable Rust error kind, selected format, diagnostic-presence policy,
  and evidence origin for fatal outcomes;
- encoded storage bit depth and the origin class of that assertion;
- structural source byte order for successful TIFF inspection, still decode,
  and every retained TIFF page, plus the origin class of each assertion;
- decoded mode and dimensions;
- exact pixel or palette-index bytes;
- exact inspect and decoded palette state, RGB bytes, and retained alpha bytes;
- sequence canvas, loop, background, frame order, source rectangle, exact
  rational timing, disposal, blend, interlace, default-image state, pixel
  layout, mode, size, and exact rendered frame bytes where Pillow exposes the
  same layout; or
- exact encoded container bytes.

Expected failures are fixture outcomes, not skipped tests. A case that Pillow
rejects passes only when the Rust path returns the corresponding structured
failure.

Approximate similarity, hashes without byte comparison, and file-size-only
assertions are not parity evidence.

### Pillow parity versus non-fatal diagnostics

The generated Pillow parity matrix has no `diagnostics` field: Pillow exposes
the decoded result and, for failures, an exception, but not a portable
structured warning/recovery record. Its rows therefore prove the observable
outer result—success or error, pixels, metadata, frames, and encoded bytes—only
where Pillow exposes that result. The separate
`tests/fixtures/diagnostic_manifest.json` is labeled `defensive_model`; it
asserts the Rust-only `DiagnosticKind`, operation stage, byte offset, and
stable structure identity. Its `pillow_outcome: "ok"` and unchanged pixels are
supporting fixture evidence, not proof that Pillow returned an equivalent
diagnostic. The accepted cases are a non-standard GIF graphic-control size,
Pillow-tolerated invalid compressed payloads in PNG `zTXt`/`iCCP`/`iTXt`, a bad
PNG `IDAT` CRC that Pillow accepts through `load()`, a bad PNG `IEND` CRC that
Pillow accepts through both `load()` and `verify()`, a static PNG stream without
`IEND` that Pillow accepts through `load()`, and the existing trailing-input
policy. It also records Pillow-tolerated duplicate `PLTE` and `tRNS` members,
which keep the first palette result, and an unknown ancillary chunk whose third
type character violates PNG's reserved-bit rule but Pillow accepts. The
defensive manifest also records a Pillow-tolerated unknown ancillary `teSt`
chunk after `IDAT` at offset `57`; valid APNG control and frame-data chunks
(`acTL`, `fcTL`, and `fdAT`) are not treated as ordering recoveries, while
malformed duplicate `acTL` and `acTL`-after-`IDAT` declarations are tracked
separately. The
manifest also records Pillow-tolerated indexed-palette shape damage
(`png_trns_overlong`, `png_missing_plte`, `png_empty_plte`, `png_partial_plte`,
and `png_trns_without_plte`), an APNG declaration with zero animation frames
(`png_apng_zero_frames`), an out-of-range APNG frame count at offset `33`
(`png_apng_frame_count_out_of_range`), malformed APNG declarations that fall back to the
default PNG image (`png_duplicate_actl` at offset `53` and
`png_actl_after_idat` at offset `3681`), an overlong APNG `acTL` payload at
offset `33` (`png_actl_overlong`), and valid inflated PNG bytes beyond the
first raster (`png_oversized_scanline`). These cases retain the usable Pillow-observed
pixels while their Rust-only diagnostics expose the recovered structure. CRC
mutations after the first `IDAT` additionally cover APNG `fcTL`/`fdAT`, a late
`acTL`, and an unknown `teSt` ancillary member. They use
`png_fcTL_crc`, `png_fdAT_crc`, `png_acTL_crc`, or `png_post_idat_crc`; a late
declaration or ancillary-order case records both applicable diagnostics in
manifest order.
`bad_idat_crc.png` parity row still owns the outer success/pixel result and the
separate diagnostic rows own only the Rust `RecoveredStructure` records. The
`missing_iend.png` parity row owns Pillow's load-success/pixel result and
`verify()` error; its separate defensive rows own only the Rust
`png_missing_iend` record at EOF offset `3643`. Rust `verify()` remains a fatal
CRC and missing-terminator boundary, including the defensive bad-`IEND`-CRC
rows even though Pillow's `verify()` accepts that fixture. The duplicate `PLTE` and `tRNS` parity rows
own their Pillow success/pixel results; separate defensive rows own only
`png_duplicate_plte` at offset `51` and `png_duplicate_trns` at offset `65`.
The malformed APNG declarations likewise own separate defensive records:
`png_duplicate_actl` identifies the ignored second declaration and
`png_actl_after_idat` identifies the late declaration; both retain the usable
default-image result. The `apng_long_actl.png` parity row owns the successful
two-frame result, while separate defensive rows own `png_actl_overlong` for
the ignored trailing byte in its `acTL` payload. The
`apng_control_after_idat_apng_large_frame_count` parity row owns Pillow's
one-frame fallback result; separate defensive rows own
`png_apng_frame_count_out_of_range` for the ignored out-of-range declaration.
Unsupported compression methods in PNG
`zTXt`/`iCCP` are not accepted recoveries: Pillow rejects those files, so they
remain outside this contract. No coverage-only unit or `cfg(coverage)` test is
used to manufacture these paths: real fixtures and the defensive manifest
drive them. The oversized-scanline status is returned by the existing bounded
one-pass inflater; no second decompression or coverage-only hook is used.

The separate `png_unsupported_compressed_metadata_methods_remain_fatal`
contract covers that Pillow-observable fatal boundary for valid-shape non-zero
`zTXt`/`iCCP` methods. It asserts only the fatal error kind, operation stage,
and `png_chunk` parse context; it does not add a Rust diagnostic field to the
Pillow parity matrix.

The `tiff_compressed_payload_failures_retain_parse_context` contract applies
the same separation to a malformed Deflate strip: its still and sequence
assertions retain the fixture's byte offset `122` and `tiff_strip` identity as
Rust error-detail evidence, without adding those fields to Pillow parity.

The `tiff_capability_and_destination_failures_are_structured` contract extends
that boundary to the existing unknown-compression fixture (offset `140`,
`tiff_strip`), a valid `La16` caller mode that TIFF cannot encode, and rejecting
still/sequence `OutputSink` destinations. The compression context, unsupported
mode, and destination results are Rust-only structured contracts; they do not
add fields or caller-owned encoder state to the Pillow parity matrix.

The final part of `output_sinks_receive_the_exact_encoded_bytes` covers the
generic whole-buffer fallback for every enabled JPEG, GIF, TIFF, WebP, and
native AVIF still encoder. ICO still and one-frame sequence delivery use the
structural path described below. Each real public call must normalize a rejecting
destination to `OutputWrite` with the selected format and `StillEncode` stage,
without an input offset, container identity, or `UnsupportedReason`. These are
Rust-only destination contracts: Pillow has no caller-owned sink, so the cases
are not parity rows and any aggregate coverage from them is incidental evidence.

The test boundary is deliberate. `diagnostic_manifest_matches_the_non_parity_contract`
in `tests/feature_gate_tests.rs` and
`trailing_input_policy_manifest_matches_the_public_contract` in
`tests/decode_policy_tests.rs` are ordinary fixture-backed Rust behavior
contracts, not generated parity rows. They assert the Rust-only fields while
comparing the mutated result with the unmutated fixture baseline; the baseline
asset's Pillow result is separate supporting evidence. Adding those expected
fields to `coverage_matrix.json` would invent oracle data that Pillow does not
return and would mislabel defensive policy as parity evidence. Any coverage
obtained while these real contracts run is incidental evidence; no
diagnostic-specific `cfg(coverage)` hook supplies the contract.

The `UnsupportedReason` field follows the same boundary. The
`unsupported_reasons_are_non_parity_capability_contracts` test asserts
`TargetUnavailable` and `NotImplemented` as Rust capability reasons, while
input-class incompatibilities retain `None`. The generated Pillow matrix has
no portable equivalent field, so these assertions are not parity rows.

`ImageError::OutputWrite` has the same boundary: Pillow does not accept this
crate's caller-owned `OutputSink`, so a destination rejection cannot be a
Pillow-parity fixture. The existing
`output_sinks_receive_the_exact_encoded_bytes` contract test asserts that this
Rust-only error also has no `UnsupportedReason`; its coverage is incidental to
the real sink contract, not a coverage-only test.

`EncodePolicy::max_output_bytes` is another Rust-only boundary. Pillow has no
caller-controlled maximum-output policy and no equivalent sink contract, so
`encoded_output_policy_is_a_non_parity_result_contract` is deliberately not a
parity row. It runs real PNG/BMP still and GIF sequence encodes, admits the exact
result, rejects a one-byte-smaller result with the typed
`EncodedOutputBytes` limit, and verifies that policy rejection leaves the sink
unchanged. The test proves result admission before the first sink write; the
PNG and BMP still paths additionally preflight their complete lengths before
structural delivery. It does not prove transient allocation limits or
recoverable OOM behavior. Its aggregate coverage is incidental evidence, not
Pillow parity coverage, and no coverage-only hook is added.

`EncodePolicy::max_work_units` follows the same boundary. Pillow has no
caller-controlled checkpoint budget or equivalent result, so
`encode_work_budget_is_a_non_parity_result_contract` is an ordinary Rust-only
contract rather than a generated parity row. It proves that an ample budget
preserves PNG bytes, a zero budget returns the typed
`ResourceLimit::EncodeWorkUnits` error for still and sequence dispatch, and a
zero-budget sink remains untouched. A work unit is one documented cooperative
encode checkpoint; the budget is deterministic work control, not CPU-time,
instruction-count, transient-allocation, or recoverable-OOM accounting. The
test's aggregate coverage is incidental evidence, and no coverage-only hook
or synthetic Pillow row is added.

Encode cancellation follows the same evidence boundary. Pillow has no
`CancellationToken`, no caller-owned `OutputSink`, and no equivalent
interruption result, so `encode_cancellation_is_a_non_parity_contract` and
the structural assertions in `output_sinks_receive_the_exact_encoded_bytes`
are ordinary fixture-backed Rust contracts rather than generated parity rows.
They check byte identity for uncancelled JPEG/PNG/BMP/TIFF/GIF/WebP/ICO still,
native AVIF still, GIF-sequence output, and one-frame ICO sequence sink output;
stable pre-cancelled errors, successful token-aware sink writes, and PNG/BMP/ICO
still sinks that can cancel between structural writes while retaining only the
delivered prefix. JPEG's codec-local coverage drill fires deterministic
internal row/block/scan checkpoints; the public test intentionally avoids
timing-sensitive interruption. The PNG and BMP still paths poll while
preparing rows and between emitted structural segments in both return and sink
paths; TIFF still encoding now polls page
preparation, row prediction, raw/PackBits/LZW work, and deflate boundaries;
GIF still encoding reuses the GIF block/frame/coalescing/output-assembly
checkpoints; WebP still encoding polls preparation, codec-result, and
metadata-assembly boundaries; native AVIF still encoding polls its preparation,
frame, and finalization checkpoints; GIF, TIFF, WebP, and native AVIF sequence
paths poll their implemented frame/coalescing/page/finalization checkpoints.
ICO still encoding polls source-size validation, embedded PNG work or BMP row
assembly, and directory finalization.
The AVIF assertion is native-only because portable WASM AVIF encoding remains
target-unavailable. This slice does not claim
universal interior interruption, deeper deflate/structural interruption,
progress callbacks, short-write semantics, or rollback cleanup; the separate
checkpoint work-budget contract is covered below.
Every current sink path does call `OutputSink::flush` once after complete
delivery; a flush failure is a typed `OutputWrite` and does not roll back an
already-delivered prefix.

The codec-local `#[cfg(coverage)]` cancellation drills fire deterministic
checkpoint counts so the implemented error edges are executed in the managed
run. They are internal Rust coverage evidence, not Pillow parity rows and not
a reason to add synthetic entries to `coverage_matrix.json`. Aggregate
coverage therefore includes parity, ordinary Rust contracts, and permitted
private state models while their evidence origins remain separate.
The BMP encoder's private coverage model is limited to post-validation row,
sink-checkpoint, cancellation, and overflow guardrails that a valid Pillow
image cannot select; its real palette-rejection and pre-cancelled sink cases
remain in the ordinary Rust contract test above.

GIF source rectangles are not mislabeled as rendered-pixel parity: their
source/presentation metadata is independently asserted, while exact raw source
sample bytes remain a documented gap. The parity schema does not compare
auxiliary retained metadata such as EXIF, XMP, text, or orientation. Primary
AVIF ICC, `mdcv`, EXIF, and XMP item metadata are covered by the separate
defensive/specification contract below, not by synthetic parity rows.

## Current revision-bound evidence

For the current committed tree based on revision
`775263335df9680e4c453f666708745f53083e8f`, the generated matrix reports:

| Metric | Count |
| --- | ---: |
| Active cases | 1,417 |
| Decode/inspect/verify cases | 1,024 |
| Encode cases | 393 |
| Planned or unwired cases | 0 |
| Formats | 8 |

The two typed-option adapter manifests add 97 accepted translations and 69
format-qualified structured-error cases. Accepted rows are labeled
`compatibility_contract`; rejected unknown, duplicate, syntax, shape, and
enum-value rows are labeled `defensive_model`. They test the public migration
boundary without weakening the primary Pillow encode/decode contract.

The decode-policy manifest adds 87 `defensive_model` cases. It covers below,
at, and above the encoded-input, canvas-width, canvas-height, canvas-pixel, and
primary-decoded-byte maxima for inspection, still decode, sequence decode,
immutable-source construction, and lazy materialization. Successful paths
reuse Pillow case `size_1x1` and compare the same exact metadata and decoded
bytes. A zero-limit unknown-signature case proves input rejection precedes
detection; two malformed preflight rows prove inspection errors propagate
through policy-aware decode. A manifest-described AVIF `ispe` mutation proves
that an unrepresentable primary transfer length remains a format-qualified
`Dimensions` failure. Error paths assert the complete typed limit fields and
retry-safe lazy-cache behavior. Eight additional rows prove extreme
`u64::MAX`/`u32::MAX` maxima succeed on the tiny `size_1x1` asset, so the
inclusive comparisons and `expected_bytes` checked arithmetic are exercised
without allocating enormous fixtures.

The sequence-policy manifest adds 19 `defensive_model` cases for the
frame-count resource. It covers below, at, and above a three-frame GIF's
declared count for inspection, sequence decode, and immutable-source
construction; the zero/one/two boundaries for still and lazy still
materialization; unknown-signature precedence; and encoded-bytes, pixels, and
primary-bytes precedence rows. Success paths reuse Pillow case
`animated_3frame` and compare exact inspected metadata, still pixels, and all
three retained frame source/presentation contracts. Frame-count rejection
happens after encoded-input and primary-canvas checks and before sequence
materialization; GIF/TIFF chains whose inspection cannot prove an exact count
remain governed by the inspection-completeness model rather than this
resource.

The same manifest grows to 35 cases for the later-frame and cumulative
decoded-byte resources. `max_frame_decoded_bytes` covers zero/below/at/above
the first later frame's transfer-byte length, and `max_sequence_decoded_bytes`
covers the primary-only, zero, total-below/at/above cumulative boundaries.
Precedence rows prove encoded bytes, frame count, primary bytes, later-frame
bytes, and cumulative bytes reject in that order, with `CodecError` preserving
the structured `LimitExceeded` value across the codec boundary. A second
fixture-driven test runs the same boundaries against real three-frame GIF,
two-frame APNG, animated WebP, multipage TIFF, and animated AVIF assets in
their enabled lanes, including a palette-less later GIF frame.
Three near-maximum rows (`u32::MAX` frames, `u64::MAX` later-frame bytes, and
`u64::MAX` cumulative bytes) pass on the same small GIF.

The metadata-policy manifest pins one independently measured encoded metadata
extent per format (JPEG 625, PNG 57, GIF 33, BMP 54, TIFF 110, WebP 20, ICO
22, AVIF 282) with SHA-256 asset digests. The Rust harness runs
`max_metadata_bytes` below/at/above, zero, `u64::MAX`, and encoded-bytes and
primary-canvas precedence rows across inspection, still decode, sequence
decode, source construction, and lazy decode, plus a malformed-scan
propagation row. The metadata rule counts every encoded byte that is not
primary pixel payload data, and the scanner must agree with the independently
measured manifest values. In AVIF this includes recognized EXIF/XMP item
payloads stored in `mdat`; only sample spans referenced by the primary or
auxiliary planes are excluded.

The work-budget analysis in the architecture reference maps every current
codec work dimension to the resource that bounds it (encoded bytes, canvas and
primary-byte limits, per-frame and cumulative sequence limits, and the
metadata extent); the policy manifests cited there are the active boundary
evidence at below/at/above and `u64::MAX`/`u32::MAX` extremes.

The claim ledger (`tests/fixtures/claim_ledger.json`) pins the revision-bound
tuple: the base revision, Pillow manifest SHA-256, generated-matrix SHA-256,
the Coverage MCP run/snapshot identifiers, and every fixture-manifest SHA-256.
`scripts/verify_claim_ledger.py` recomputes every hash, validates the revision
and identifiers, and requires the four maintained documents to name the same
revision; CI runs the verifier so the tuple cannot drift.

The runtime capability-table fixture (`tests/fixtures/capability_tables.json`)
is generated by `scripts/generate_capability_tables.py` from the probe in
`tests/capability_table.rs`, which is included by the `feature_gate_tests`
integration target. The probe executes in every feature lane on the native
host and on `wasm32-wasip1` under Node's WASI preview1 runtime. The standalone
script still launches those probes concurrently when run by itself; the
feature-matrix command runs the full target with `--nocapture`, reads the one
capability row already emitted by each lane log, and performs the same
no-drift check without launching 22 duplicate probe processes. Reusing that
target means the capability check does not compile a second integration test
target after the feature matrix. The fixture keys evidence by full target
triple for the WASM lane and records the generating native host triple; CI
regenerates the tables in memory and fails on any drift, so native and WASM
capability tables cannot diverge from the implementation. The same script is
part of the feature-matrix command registered with Coverage MCP as
`feature-matrix-runtime-tables`.

The trailing-input manifest pins the per-format trailing policy: three payloads
appended to a valid asset of every format must produce identical still pixels,
identical sequence frames, and identical inspection results, with
`consumed_bytes` unchanged and a `TrailingDataIgnored` diagnostic naming the
first ignored byte on formats with a defined extent. JPEG, PNG, GIF, WebP,
TIFF, and AVIF report the
container-defined extent; BMP and ICO report `None`. Pillow 12.2.0 accepts all
three payloads for every format (fixture evidence), while the consumed values
and diagnostic fields are `defensive_model` evidence. The manifest is
SHA-256-pinned and exercised only in each format's enabled feature lane.

The verification-strength contract is table-driven rather than manifest rows:
for every format, the enabled feature lane loads the smallest Pillow-verified
fixture and asserts `ImageFormat::verification_scope()` and
`EncodedImage::verification_scope()` agreement, weaker/equal
`verify_with_scope` success, stronger-request failure with the exact format
and a non-empty diagnostic, and the never-provided `FullPixels` boundary.

The sequence-kind contract is also table-driven rather than manifest rows:
one animated or multipage fixture per sequence-capable format plus every still
fallback asserts `DecodedSequence::kind` (`TimedAnimation` for GIF, APNG,
animated WebP, and AVIF; `UntimedPages` for TIFF; `SingleFrame` for still
fallbacks), and TIFF pages additionally assert exact zero durations so they
are never described as timed animation. AVIF sequence decode is native-only,
so its row is skipped on `wasm32` targets.

The source-alpha contract is table-driven as well: inspect and decode must
agree on `SourceDescriptor::alpha()` for GIF transparency (`BinaryMask`),
PNG/WebP/TIFF/AVIF alpha (`Straight`), and opaque fixtures of every format
(`None`). TIFF `ExtraSamples` 1 maps to `Premultiplied`; no committed fixture
declares associated alpha, so that mapping arm is retained through the
coverage model rather than a Pillow fixture. Decoded transfer bytes remain
the normalized unassociated layout regardless of the source declaration.

The opaque-block contract is table-driven: a minimal PNG is extended with
unknown ancillary chunks (safe-to-copy and unsafe-to-copy names, a duplicate,
a post-IDAT chunk, and a critical chunk) and the test asserts exact
kind/payload/order/safe-to-copy on still and sequence decode, empty retention
for unmodified fixtures, APNG sequence retention, and that default encoding
never replays the retained chunk types. A caller-set `max_metadata_bytes`
rejects the same input before retention can bypass the policy extent.

The metadata-record contract is table-driven as well: known PNG metadata
chunks (tEXt/zTXt/iCCP/eXIf) inserted into a minimal PNG must appear as raw,
unparsed `OpaqueMetadata` records in original order while unknown chunks stay
in `opaque_blocks`; valid compressed payloads are bounded-validated and then
asserted byte-for-byte without exposing inflated text/profile bytes. Separate
diagnostic-manifest rows prove that Pillow-tolerated invalid compressed
`zTXt`/`iCCP`/`iTXt` payloads are ignored with usable pixels and a stable
diagnostic instead of being retained as metadata. The same defensive manifest
records the accepted bad-`IDAT`-CRC recovery at chunk offset `33` with
`png_IDAT_crc`, the accepted bad-`IEND`-CRC recovery at chunk offset `57` with
`png_IEND_crc`, the accepted post-`IDAT` CRC recoveries for APNG and ancillary
chunks, plus the accepted `prvt` reserved-bit mutation at the same offset with
`png_reserved_bit`, and the accepted static `missing_iend.png`
recovery at EOF offset `3643` with `png_missing_iend`; the Pillow parity matrix
does not gain a diagnostic field. Duplicate `PLTE`/`tRNS` rows likewise remain
Rust-only structural diagnostics rather than parity fields.
Still, fallback-sequence, and APNG sequence decode must agree, and default
encoding must not replay any metadata chunk.

The source-color contract is table-driven: well-formed PNG sRGB/gAMA/cHRM/iCCP
chunks are parsed into `SourceColor` (intent, gamma, chromaticities, raw
profile) while duplicates and malformed payloads fall back to ordered raw
metadata records and unknown chunks stay opaque. Still, fallback-sequence, and
APNG sequence decode must agree, unmodified fixtures carry an empty
descriptor, and encoded output must not replay color chunks.

The AVIF source-color contract is separate from Pillow parity. The bounded
item parser reads the primary item's `colr`/`nclx` CICP, `av1C` chroma sample
position, and `clli` content-light-level fields into `SourceColor`; the
contract test asserts
inspect, still decode, and fallback-sequence agreement plus rejection of
reserved range-flag bits, extra payloads, and truncated fields. Pillow rows
continue to assert the observable outer result, mode, and pixels, but they do
not claim to prove item-level AVIF color metadata because Pillow exposes no
equivalent structured result. No parity row is added for this source-
provenance field; malformed parser cases remain defensive-model evidence.

The same source-color contract reads primary-item `colr` ICC profiles with
`prof` and `rICC` kinds into `SourceColor`, retaining the exact profile kind
and bytes and rejecting an empty profile. It uses the committed
Pillow-generated encoded metadata output only as a source witness, then
asserts inspect, still decode, fallback-sequence, pixel-preservation, and
malformed-profile behavior as separate defensive/specification evidence. No
row is added to `coverage_matrix.json`.

The contract also associates a deliberately distinguishable 24-byte primary
`mdcv` property and asserts the exact green/blue/red wire-order mapping into
the public red/green/blue descriptor, the mastering luminance fields, inspect/
decode/fallback-sequence agreement, unchanged pixels, and rejection of
truncated, overlong, or duplicate properties. This is the same
defensive/specification lane: Pillow has no structured item-level `mdcv` result,
so no parity row or coverage-only test is added.

The same AVIF contract retains recognized `Exif` item payloads and MIME XMP
items whose content type is exactly `application/rdf+xml` as ordered raw
`OpaqueMetadata` records on still and sequence decode. The committed
`Encode.avif_enc_metadata.bin` output is used as a source witness because
Pillow exposes no equivalent structured decoded metadata field; it is not a
parity fixture and no `coverage_matrix.json` row is added. The EXIF bytes are
asserted exactly as stored, including the AVIF TIFF-header offset prefix. The
private `cfg(coverage)` drills for missing locations, empty/invalid extents,
capacity overflow, and pixel-span accounting are aggregate Rust coverage
evidence only; they do not suppress or inflate Pillow parity coverage.

The AVIF item-property contract is separate from Pillow parity. A committed
AVIF orientation output supplies `irot`; the test mutates all four legal
rotation values, both `imir` axes, and associated `pasp` and `clap` properties,
then asserts inspect, still decode, and fallback-sequence source descriptors
agree while pixel bytes remain unchanged. Reserved values, zero spacings or
fractions, duplicate associations, and malformed property payloads are
rejected by both bounded parsers. The test is specification/defensive-model
evidence because Pillow's observable result does not expose item properties or
structured provenance.

The GIF-extension contract is table-driven: comment, plain-text, and
non-NETSCAPE application extensions inserted into a minimal GIF must appear as
ordered `OpaqueMetadata` records with exact payload bytes, the NETSCAPE loop
extension must remain interpreted into `loop_count`, unknown labels must stay
in `opaque_blocks`, still and sequence decode must agree, and encoded output
must not replay any retained extension.

The JPEG-marker contract is table-driven: APP1/APP2/COM/multi-APP2/APP14
segments inserted after SOI must appear as exact ordered metadata records on
still and sequence decode, the unmodified fixture must retain only its JFIF
APP0 record, truncated metadata markers must carry the `jpeg_metadata`
identity, and encoded output must not replay retained marker payloads.

The WebP-chunk contract is table-driven: an extended WebP built from a
fixture's VP8 chunk plus ICCP/EXIF/XMP/unknown/duplicate-ICCP chunks must
retain exact ordered metadata and opaque records on still and sequence decode,
skip a truncated ICCP, keep the unmodified fixture empty, avoid replay in
encoded output, and retain the same records on animated sequence decode.

The TIFF-tag contract is table-driven: a minimal TIFF built with inline and
offset unknown tags, a duplicate unknown tag, a rational tag, and ASCII/ICC
metadata tags must retain exact typed records (tag bytes in file byte order,
stored value bytes) on still decode and per-page sequence decode, keep an
unmodified fixture empty, and avoid replay in encoded output.

The AVIF-box contract is table-driven: unknown and free/skip boxes appended
after a fixture's mdat must be retained as exact raw opaque records on still
and sequence decode, a truncated trailing box must be ignored, the unmodified
fixture must stay empty, and encoded output must not replay retained boxes.

The destination-buffer contract is table-driven: for every decoded mode family
(L1 packed, P8, L8-family, La8, Rgb8, Rgba8) across PNG/GIF/TIFF/WebP/JPEG/
BMP/ICO/AVIF fixtures, `ImageInfo::decoded_bytes` must equal the decoded pixel
length, `decode_into` must copy byte-identical pixels into an exact buffer,
short and oversized destinations must be rejected with `Parameter` and left
untouched, and policy limits must still apply before the length check.

The transfer-layout contract is table-driven: for L1, RGB, RGBA, indexed, and
La8 fixtures, `TransferLayout` fields must agree with inspection and decoded
bytes, row bytes must satisfy the packing rules (packed L1 rows pad the final
byte), and `decode_into` must accept exactly `total_bytes`.

The basic-inspection contract is table-driven: `inspect_basic` must agree with
`inspect` on format, dimensions, mode, depth, and palette, and must report
`frame_count_complete` truthfully — animated GIF/TIFF/WebP fixtures report
`frame_count=None` with completeness `false`, single-frame deep formats report
a complete count of one (GIF trailer peek, TIFF next-IFD offset, WebP still),
and header-bound formats (PNG/JPEG/BMP/ICO/AVIF) return the full result.

The incremental-input contract is machine-checked for all eight formats:
`detect_prefix` must identify complete signatures, return exact or
progress-aware `NeedMoreData { minimum }` for incomplete prefixes, and return
terminal `UnknownFormat` for bytes that can never match; `inspect_basic_prefix`
must return the same header facts as `inspect_basic` as soon as they are
provable. The evidence is a committed defensive-model manifest
(`tests/fixtures/incremental_input_manifest.json`, hashed in the claim
ledger) pinning every detection edge case with its exact minimum or terminal
classification, per-format signature and need-more spot checks with exact
minimums, and the documented legacy divergence for a size-one AVIF `ftyp`
box that the Pillow predicate still recognizes. The feature-gate suite also
sweeps every byte boundary of one valid fixture per format and asserts that
each retry minimum exceeds the current prefix, that retrying with `minimum`
bytes makes progress, and that the complete-slice APIs never expose the
non-terminal status: legacy detection reports `UnknownFormat` for incomplete
signatures and legacy inspection maps codec-level truncation back to
`Malformed` with unchanged messages. Terminal results must never be retried.

The incremental-decode contract extends the same sweep to `decode_prefix` and
`decode_sequence_prefix`: every byte boundary of one valid fixture per format
must either decode identically to the legacy API or return
`NeedMoreData { minimum }` whose retry makes progress while legacy `decode`
keeps its terminal classification, and policy variants must re-evaluate
limits on the current prefix. The manifest pins per-format decode spot checks
with exact minimums (PNG chunk header, GIF sub-block, BMP header field, TIFF
strip span, JPEG marker read, WebP native read, ICO entry span, AVIF box).

The cancellation contract is machine-checked for all eight formats: a
never-cancelled token must produce byte-identical results to the legacy APIs,
a pre-cancelled token must stop with `ImageError::Cancelled` carrying the
format and operation stage without publishing partial state, clones must
share cancellation state, truncated input must still report
`NeedMoreData { minimum }`, and policy limits must still reject before codec
work. A multi-frame GIF retry with a fresh token proves cancelled attempts
never corrupt reusable state. The coverage drills additionally fire the token
after a fixed number of polls so every structural checkpoint is exercised.

The borrowed-view contract is table-driven: `EncodedImageView` over
PNG/GIF/WebP/TIFF fixtures must match the free functions exactly for inspect,
still and sequence decode, policy variants, verification scope behavior,
transfer layout, and resource-limit rejection, without copying the borrowed
bytes.

The frame-decode contract is table-driven: `EncodedImage::decode_frame` and
`EncodedImageView::decode_frame` over animated GIF/APNG/WebP and multipage
TIFF fixtures must return exactly the corresponding `decode_sequence` frame,
out-of-range indices must fail with `Parameter`, and still formats must report
exactly one frame.

The output-sink contract is table-driven: `encode_to_sink` and
`encode_sequence_to_sink` over PNG/BMP/ICO still, one-frame BMP/ICO sequence,
and GIF sequence fixtures must write bytes identical to `encode`/`encode_sequence`
with matching lengths for both `Vec<u8>` and `&mut Vec<u8>` sinks. PNG, BMP, and
ICO still additionally prove multiple structural writes, policy preflight
before the first write, and cancellation between writes; one-frame BMP and ICO
sequence additionally prove multiple structural writes and policy preflight,
while GIF sequence remains a whole-buffer comparison. ICO's structural split is
a fixed 22-byte directory header followed by the complete embedded PNG/DIB
payload. A deterministic failing write or flush must be reported as
`ImageError::OutputWrite` with the selected format and encode stage. The
current contract proves one post-delivery flush call and explicitly preserves
the delivered prefix on flush failure. Short writes and rollback cleanup remain
future writer evidence, not claims made by this contract.
The same contract exercises invalid-input errors through the generic JPEG
whole-buffer fallback, with no sink write; those cases are Rust API/error
evidence and are not added to the Pillow parity matrix.

The cross-target determinism contract is machine-checked: a
`determinism_tests` target computes SHA-256 over exact encoder output and
decoded pixels for 15 fixed cases and compares them with the committed golden
hashes. The feature-matrix command runs the same suite natively and on
`wasm32-wasip1`, so a green CI run proves byte-identical output between the
native host and the WASM runtime.

The operation-stage contract is a separate feature-gate test: one real
failure is driven through inspection, still decode, sequence decode, source
construction, verification, still encode, and sequence encode, and each error
must report the exact `ImageErrorStage` alongside its stable kind. Caller-built
validation and option-construction errors remain stage-free by design, so the
test also pins which public paths produce a stage.

The same test now asserts the parse-site byte offset and container-structure
identity on codec-dispatched failures: truncated PNG chunks across
inspect/still/sequence/source/verify, GIF image descriptors, JPEG markers,
TIFF IFDs, truncated AVIF boxes, BMP header/palette/pixel-span/bitfield/RLE
cases, ICO header/directory/entry-range/embedded PNG/DIB/CUR cases, and WebP
inspection/container-chunk cases all carry `identity` values with offsets.
Still and sequence WebP payload-decoder failures carry
`webp_bitstream` at the validated payload start (or current ANMF container
offset); finer decoder-internal cursors remain outside the contract. Encode
and option-construction errors stay offset-free. The BMP, ICO, and WebP
witnesses are ordinary Rust error-contract cases, not generated Pillow-parity
rows.

The malformed-class ledger is generated from the coverage matrix by
`scripts/generate_malformed_ledger.py` and checked in CI with `--check`, so
every active decode-error class must stay catalogued. Each class records the
Pillow outcome (exception type/message where the oracle throws), the Rust
error contract per operation, evidence origins, and one specification status:

| Format | Classes | `spec_violation` | `truncated` | `not_the_format` | `ambiguous` |
| --- | ---: | ---: | ---: | ---: | ---: |
| JPEG | 75 | 43 | 26 | 5 | 1 |
| PNG | 50 | 36 | 12 | 2 | 0 |
| GIF | 47 | 15 | 28 | 4 | 0 |
| BMP | 58 | 19 | 32 | 7 | 0 |
| TIFF | 56 | 44 | 9 | 3 | 0 |
| WebP | 112 | 69 | 36 | 7 | 0 |
| ICO | 28 | 11 | 14 | 3 | 0 |
| AVIF | 16 | 7 | 2 | 7 | 0 |

Totals: 442 classes (244 `spec_violation`, 159 `truncated`, 38
`not_the_format`, 1 `ambiguous`) carrying 442 `pillow_fixture` and 16
`specification_reference` origin labels. Status changes are intentional
contract changes because the committed ledger must regenerate byte-for-byte.

The counts are reproducible from the generated artifact:

```bash
jq '.summary' tests/fixtures/coverage_matrix.json
```

The accepted Coverage MCP result for the current implementation state is:

| Metric | Covered | Total |
| --- | ---: | ---: |
| Lines | 47,943 | 47,943 |
| Branches | 6,578 | 6,578 |
| Functions | 2,686 | 2,686 |
| Regions | 74,654 | 74,654 |

The same managed run executed every active manifest case with zero failures or
skips.

The bounded feature-matrix runtime optimization was benchmarked by run
`f74e711f-c9a2-4327-bc74-d834b6bf399a` at the pre-JPEG harness revision: 903
checks passed with zero failures in 298,766 ms, and its terminal capability-table
record says `capability tables OK: every native and wasm32-wasip1 lane agrees`.
That was 52,789 ms faster than the previous managed run
(`e7755afd-eedf-4fe7-b56d-f24ea54a55e1`, 351,555 ms) with the same 903-check
scope. The current clean revision was then validated by run
`bea69012-22a4-4b55-9ef9-e3859c73ef2e`: 903 checks passed with zero failures in
1,296,952 ms, with the same capability-table terminal record. These are separate
execution records rather than a controlled speed comparison because managed
cache/build state differs. The matrix uses a bounded completion-driven scheduler
for independent native, `wasm32-unknown-unknown`, and `wasm32-wasip1` runtime
lanes; each full native/WASI lane emits its capability row and the final check
reads those logs. `MATRIX_JOBS` and `CAPABILITY_JOBS` default to four. A lane
completion releases a slot immediately, rather than holding it until the
slowest lane in a launch batch finishes. After the
probe-target reuse change, run
`0433b3d0-110e-4242-a088-c7acbc3cefa2` passed 925 checks with zero failures in
865,050 ms and retained `capability tables OK: every native and
wasm32-wasip1 lane agrees`. Its 22 additional checks are the capability probe
now run inside the already-built feature-gate target. The parent revision was
recorded when the worktree was submitted; the working-tree patch was committed
as `45e1922`. This remains target/runtime evidence rather than a controlled
speed comparison because managed cache/build state differs. It does not turn
aggregate coverage, defensive/specification contracts, or Rust-only diagnostic
tests into Pillow-parity coverage.

The completion-driven scheduler was then validated by run
`de0619ff-e117-4d9d-bc3e-e9ee7fff01bf`: 925 checks passed with zero failures in
298,267 ms and the terminal capability-table record was unchanged. That run was
submitted against parent revision
`65c3a4b5714f118e93b62b07b899f2ddc1c64d04` with the scheduler patch in the
working tree; the patch was committed as `766a6dd`. The shorter duration is
retained as runtime evidence, not a controlled speedup claim, because the
managed target/cache state differs from the earlier 865,050 ms execution.
The subsequent PNG/BMP still-token validation run
`a545e1bb-ec85-4f8d-93c1-3e0e778907c2` passed 925 checks with zero failures in
1,102,400 ms and retained the same terminal capability-table record. It was
submitted against parent revision
`66f6159c39f6deae1c98d8bf3da5277f76a2d780` before the implementation commit;
this remains execution provenance rather than a controlled speed comparison.

The GIF still-token validation run
`82750f5a-cad6-4f87-b538-adf6d1e21c29` passed 925 checks with zero failures in
1,018,825 ms and retained the same terminal capability-table record. It was
submitted against parent revision
`7e684afa53e45a100ea91a00b1acd1bee7c38ebc` before the implementation commit
`cc1d5c8`; this remains execution provenance rather than a controlled speed
comparison.

The WebP still-token matrix run
`d323e738-d2ce-4523-bec8-563c2421ad0a` passed 925 checks with zero failures in
1,093,331 ms and retained the terminal capability-table record. It ran against
clean revision `2ffb338217cfb71223fb81dfe3b0cdf59b9f9aed`; this duration is
execution evidence rather than a controlled speed comparison because managed
cache/build state differs.

The lane-local Cargo target-root optimization was then validated by run
`91155ed7-9729-4877-8433-d14146428137`: 925 checks passed with zero failures in
110,405 ms and retained the same terminal capability-table record. Each bounded
native, `wasm32-unknown-unknown`, and `wasm32-wasip1` lane now gets a temporary
target root, while the capability-table probes reuse those roots rather than
creating a second build. The retained log has zero `Blocking waiting for file
lock on build directory` matches; package-cache waits can still occur while
parallel lanes initialize. This is a correctness and lock-contention result,
not a controlled speedup claim, because the managed cache/build state differs
from the earlier runs.

The duplicate-probe elimination was validated by run
`120f465d-fd8b-43af-8ce2-76497d99fb80`: the same 925 checks passed with zero
failures in 77,855 ms, the terminal capability-table record remained unchanged,
and the retained log again had zero build-directory lock-wait matches. The
22 capability assertions are still exercised inside the full native/WASI test
lanes; only the second probe launches were removed. The observed 32,550 ms
difference from the preceding run is execution evidence rather than a
controlled speedup claim because managed cache/build state can differ.

The bounded lane-concurrency follow-up was validated by run
`5e438aba-378e-4a33-b03f-d4ecd047865e`: 925 checks passed with zero failures in
67,609 ms, the capability-table terminal record remained
`capability tables OK: every native and wasm32-wasip1 lane agrees`, and the
retained log had zero build-directory lock-wait matches. Raising the default
from three to four is retained as execution evidence rather than a controlled
speedup claim because managed cache/build state can differ.

The ICO still-token matrix run
`991a26ef-f7a6-40be-b2bb-c98be087bcce` passed 925 checks with zero failures in
116,267 ms. The cancellation contract passed in all 22 retained feature lanes,
the terminal record remained `capability tables OK: every native and
wasm32-wasip1 lane agrees`, and the retained log had zero build-directory
lock-wait matches. It ran against clean revision
`112a26868428278cf49c12a64451c3ccbc156d30`; the later all-feature coverage
revision is recorded below. This is ordinary Rust operation-boundary evidence,
not a generated Pillow parity row.

The final BMP-sequence sink matrix run
`caeb1194-d307-4305-9e87-e0eef94b205a` passed 925 checks with zero failures in
82,978 ms. Its terminal record remained
`capability tables OK: every native and wasm32-wasip1 lane agrees`, and the
retained log had zero target-directory lock-wait matches. The one-frame BMP
sequence structural sink, option mismatch, multi-frame rejection, exact-length
policy preflight, and sink-delivery cases are ordinary Rust output contracts;
the Pillow parity matrix is unchanged.

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

The sink-finalization follow-up was validated on committed revision
`775263335df9680e4c453f666708745f53083e8f` by run
`6ef08e71-abcf-4841-b30f-649529bb3bfc`: 947 checks passed with zero failures
in 65,458 ms, retained the same terminal capability record, and had zero
build-directory lock-wait matches. This is execution evidence rather than a
controlled speed comparison because the managed cache state differed.

Coverage MCP run: `f8875a27-27a0-4cec-85cc-be73e8e8e552`

Snapshot: `78e65eb9-1f09-4a3f-9a65-e1c25d23f1a8`

Coverage revision: `775263335df9680e4c453f666708745f53083e8f`

Coverage MCP recorded 56 passed tests with zero failures and 100% line, branch,
function, and region coverage: 47,943 lines, 6,578 branches, 2,686 functions,
and 74,654 regions. The ICO still and one-frame ICO sequence sink cases,
sink finalization errors, and
the deterministic encode work-budget contract execute the real dispatcher and
structural/error paths; this is internal Rust evidence, not a synthetic Pillow
parity case.

Manifest SHA-256:
`bffa47f55b0a4ef2d64979392410e7544617fcebdedcd4086cd76532a4c936e3`

Generated matrix SHA-256:
`b087396b064ed216a03ed789d9a6171d1f97ec99491f2f90f0c134bce29bf510`

Typed-option acceptance manifest SHA-256:
`604225c8e89b13066f49f072899829c7745f4262d698f43b203645e8da0368f5`

Typed-option error manifest SHA-256:
`657f8d9d82cb33b337ac6355aec08038686c2289ee004f8847481ac2270469eb`

Decode-policy manifest SHA-256:
`b9fdc778cb73d4d9ea99daa9adcdd8812ebc9cc90d6698d047619966620d6b6a`

Sequence-policy manifest SHA-256:
`3916244bc02725f21883792dd0e16a3ff64a5ab23ab08c3326d051a306af2d51`

Trailing-input manifest SHA-256:
`b98ccdeedb66b93b40b1d057dd8f443d1550a9c0f545106c19d153f866176abb`

Malformed-class ledger SHA-256:
`66d11684fb9601ae7fcfe83d7ed10e6e4a87657f6780c76eb99da70a719caf72`

Metadata-policy manifest SHA-256:
`5f7ccbf7303a2152c6dcc69f7f82d97b2dfa8a329e61f82ff51e7eb1a814b0ef`

Diagnostic manifest SHA-256:
`bb537f7eb5e0159793d52335fb9ba03326801c0fe8042a2876b3b98347b4c822`

Coverage-origin manifest SHA-256:
`f6bc79c4d8011b1e4f7d41cc0fdd869d355f70f4ac6bc4b0ec0f3d0aecd0b043`

The TIFF source-descriptor slice contains 93 successful inspection assertions
(88 little-endian and 5 big-endian), 71 successful still-decode assertions
(66 little-endian and 5 big-endian), four exact source multipage frame
assertions, and 10 exact successfully re-encoded multipage frame assertions.
`scripts/explore_tiff_source_byte_order.py` independently pins Pillow
12.2.0/libtiff 4.7.1 and verifies that Pillow's per-page directory prefix
matches the complete fixture header before and after materialization.

The same all-feature semantic run asserts the complete public capability table
against the manifest and current target. Strict Clippy compilation also passes
with no features, each isolated codec feature, default features, and all
features on native and `wasm32-unknown-unknown`. The capability-table probe
and feature-gate suite additionally execute at runtime in every lane on the
native host and `wasm32-wasip1`, with the committed capability fixture checked
for drift in CI. Those isolated lanes prove cfg and API behavior; they are not
presented as separate runtime coverage snapshots.

These measurements prove execution of the retained implementation under that
suite. They do not extend the compatibility promise beyond the active
manifest.

## Fixture flow

```text
manifest.yaml
    │
    ├─ scripts/generate_test_assets.py
    │      └─ tests/fixtures/input/
    │
    ├─ scripts/generate_decode_refs.py
    │      ├─ tests/fixtures/coverage_matrix.json
    │      └─ tests/fixtures/outputs/
    │
    ├─ independent AV1 reference generators
    │      └─ entropy and reconstruction JSON
    │
    ├─ tests/fixtures/encode_option_*_manifest.json
    │      └─ strict typed-option adapter success/error contracts
    │
    ├─ tests/fixtures/decode_policy_manifest.json
    │      └─ caller-limit boundary and cache contracts
    │
    └─ Rust integration harnesses
           ├─ exact public parity assertions
           ├─ exact typed-option adapter assertions
           └─ typed resource-limit assertions
```

Inputs and outputs are committed because they define the reviewable byte
contract. `.oracle-venv/`, `.coverage-mcp/`, LLVM reports, and `target/` are
local generated state and remain ignored.

## Create the oracle environment

Oracle regeneration is currently locked to CPython 3.12 on macOS arm64:

```bash
python3.12 -m venv .oracle-venv
.oracle-venv/bin/python -m pip install \
  --require-hashes \
  -r oracle-requirements.txt
```

Then regenerate:

```bash
.oracle-venv/bin/python scripts/generate_test_assets.py
.oracle-venv/bin/python scripts/generate_decode_refs.py
```

Do not regenerate committed references from another Pillow wheel or platform.
Contributors without the pinned oracle can run existing references but cannot
truthfully replace them.

## Test tiers

### Exact parity

The normal public-behavior gate is:

```bash
cargo test --locked --all-features --test coverage_matrix_tests
```

Native all-feature execution requires the exact AVIF stack described in
[AVIF support](avif.md).

### Evidence surfaces and coverage accounting

The repository keeps semantic parity evidence separate from Rust-only
defensive contracts. The distinction matters because an aggregate coverage
report answers "which implementation paths executed?"; it does not answer
"which paths matched Pillow?".

| Evidence surface | Authoritative inputs and command | Proves | Does not prove |
| --- | --- | --- | --- |
| Pillow parity | `tests/fixtures/coverage_matrix.json`; `coverage_matrix_tests` (registered as `pillow-byte-pixel-parity`) | Pillow-observable success/error results, pixels, metadata, frames, and deterministic encoded bytes for the active rows | Rust-only diagnostics, policy decisions, or parser states Pillow cannot expose |
| Defensive/specification contracts | `diagnostic_manifest.json`, decode/sequence policy manifests, feature-gate and other table-driven contract tests | Stable Rust fields and behavior that are required even when Pillow has no equivalent result field; unchanged Pillow output is supporting fixture evidence where available | A Pillow warning/diagnostic that the oracle never returned |
| Aggregate coverage | CI/Coverage MCP `cargo llvm-cov --all-features --branch --json` over the complete test suite | Execution coverage across parity tests, defensive contracts, and permitted private `cfg(coverage)` state models | Parity completeness, semantic correctness, security, or production readiness |
| Coverage-origin inventory | `tests/fixtures/coverage_origin_manifest.json`; `scripts/verify_coverage_origins.py` | Static one-to-one accounting of every exact `#[cfg(coverage)]` guard and its non-Pillow origin | Test execution coverage or Pillow-observable behavior |

The aggregate line, branch, function, and region totals must therefore never
be described as "Pillow parity coverage". A defensive contract may contribute
executed lines to the aggregate report without becoming a generated parity
row, and a private coverage model may exercise an unreachable state without
being user-facing behavior.

### Feature and target matrix

The feature script checks:

- no features;
- each codec feature independently;
- default features;
- all features;
- native Clippy, rustdoc, and feature-gate tests;
- matching `wasm32-unknown-unknown` Clippy and rustdoc builds;
- WASM test compilation for no features and AVIF;
- `wasm32-wasip1` execution of the feature-gate suite in every lane through
  Node's WASI preview1 runtime; and
- regeneration of the runtime capability tables and a no-drift check against
  the committed fixture.

Run:

```bash
scripts/test_feature_matrix.sh
```

The matrix uses isolated retained Cargo target roots to avoid build-directory
lock contention and interleaves native, `wasm32-unknown-unknown`, and
`wasm32-wasip1` lanes under one bounded completion-driven scheduler. It derives
the Rust test-harness worker count from the host CPU count and `MATRIX_JOBS`
(capped at eight). `MATRIX_TEST_THREADS` can override that derived value for a
constrained CI runner. This bounds aggregate test-thread fan-out without
dropping any lane or assertion.

Cross-compilation proves compilation, not semantic browser or WASM runtime
parity. The `wasm32-wasip1` lanes are real runtime evidence for feature-gate
and capability-table behavior; full semantic manifest execution in a WASM
runtime remains planned.

### Coverage

Repository agents must run coverage only through Coverage MCP and request line,
branch, function, and region metrics. CI independently runs `cargo llvm-cov`
and `scripts/verify_llvm_coverage.py`.

The coverage-origin inventory is a static source check, not a Rust test and not
a coverage hook. It currently accounts for 219 exact `#[cfg(coverage)]` guards
across 74 Rust files. Each guard is assigned to `defensive_model`,
`independent_implementation`, or `specification_reference`; the verifier
rejects a Pillow-parity origin. This keeps aggregate LLVM execution evidence
separate from the 1,417-row Pillow manifest and from ordinary Rust-only
diagnostic contracts.

Pillow-observable behavior should reach semantic acceptance through a complete
parity manifest fixture. Rust-only behavior should reach acceptance through an
explicit defensive or specification fixture. Both kinds of tests may execute
under the aggregate coverage command, but their evidence origins remain
separate.
`cfg(coverage)` hooks are reserved for private state machines, defensive
overflow paths, or generated states that cannot be represented by a valid
Pillow input.

The historical multi-thousand-line sweep logs were removed from the maintained
documentation set. Git history retains their reverse mappings and run-by-run
investigation.

## Adding or fixing a case

Before implementation:

1. Record the Pillow-observable case in `manifest.yaml`.
2. Generate or mutate a complete encoded file; prefix-only probes are not
   sufficient for signature/error behavior.
3. Generate the exact oracle result with the pinned environment.
4. Identify the first divergent parser, transform, or writer stage.
5. Implement the smallest codec-local correction.
6. Run formatting, strict Clippy, the relevant feature matrix, exact parity,
   and Coverage MCP.

When reverse mapping a branch, work backward from the branch predicate to the
required encoded syntax and then to a complete Pillow-generatable or
independently justified input. Do not add an unrelated helper solely to make a
line executable.

## Documentation-only changes

Documentation changes do not require regenerating codec fixtures when they do
not alter executable behavior. They still require:

```bash
cargo fmt --all -- --check
RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps --locked
cargo test --doc --all-features --locked
python3 scripts/verify_third_party_licenses.py
```

Run strict Clippy when Rust documentation comments or public examples change.
Use the open-source documentation audit for repository navigation and local
link checks.

## Packaging and provenance gates

`cargo package` intentionally excludes the large oracle corpus. The packaged
library, README, maintained documentation, security/contribution routes, legal
files, and source must still be present and usable. The repository-only
`scripts/test_feature_matrix.sh` is deliberately not packaged because it
depends on the excluded integration targets and fixture corpus.

Verify:

```bash
cargo package --allow-dirty --locked
python3 scripts/verify_third_party_licenses.py
cargo deny check
```

The test targets are repository-only because their exact fixture corpus is not
shipped to downstream users. Cargo currently reports that exclusion while
preparing the package; the packaged library must still compile successfully.

## Troubleshooting

| Symptom | Likely cause | Check or recovery |
| --- | --- | --- |
| Oracle generator refuses to run | Python, platform, wheel, extension, or codec identity differs | Compare the environment with `pillow-oracle.lock.yaml`; do not overwrite references |
| Native AVIF fails to link | Exact libavif stack is unavailable | Set `IMAGE_SLASH_STAR_AVIF_LIB_DIR` or follow `docs/avif.md` |
| A codec returns `FeatureDisabled` | The matching Cargo feature is off | Enable only the required format feature |
| WASM AVIF returns `Unsupported` | The operation or AV1 class is outside the portable subset | Use a proven manifest class or a native build |
| Matrix count changed unexpectedly | Generated matrix and manifest differ | Regenerate with the pinned oracle and review the full diff |
| Coverage falls below 100% | New executable paths lack retained evidence | Query Coverage MCP, reverse-map missing branches, and prefer complete fixtures |

For ordinary bugs, open a GitHub issue with the format, smallest non-sensitive
fixture, expected Pillow result, actual Rust result, enabled features, target,
and commit. Follow `SECURITY.md` for private vulnerability reports.
