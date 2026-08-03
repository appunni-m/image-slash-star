# Oracle, fixtures, tests, and coverage

Status: current contributor reference

Reviewed: 2026-08-03 against current implementation revision
`38af2d21830356eefa202f60f5b16c44934b8924`; the claim-ledger baseline remains
`f1048bc0399fad9801559ca7fcfd3163427b5832`.

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

The overlap between the two inventories is deliberately narrow. Of the 61
diagnostic cases, 38 use committed bytes that also have a Pillow parity row, so
those rows prove only the shared outer success/pixel result. The other 23 cases
construct runtime mutations from parity baselines (bad CRCs, reserved-bit or
ordering changes, and invalid compressed metadata); those mutated bytes are
not `coverage_matrix.json` rows and must not be counted as Pillow-parity
execution. In both groups, the Rust test compares decoded pixels or frames
with the unmodified baseline only to assert the Rust recovery invariant. Its
`pillow_outcome` field is supporting fixture annotation, not a live Pillow
diagnostic result, and no parity row can legitimately assert `DiagnosticKind`,
stage, offset, or identity because Pillow does not return those fields.

`python3 scripts/verify_diagnostic_provenance.py` performs this distinction as
a static audit: it checks the `defensive_model` origin, hashes every diagnostic
baseline, requires exactly one active Pillow row with the same format, asset
digest, successful operation, and `pillow_fixture` operation origin, confirms
the 38 unchanged cases and 23 named runtime mutations, rejects Rust-only
diagnostic or mutation fields in the Pillow matrix, and checks these counts in
the maintained docs. It executes no Rust test and contributes no coverage; it
prevents a basename collision or an unrelated unit test from being presented
as Pillow-parity evidence as the defensive manifest grows.

This is why COR-061 is not a parity-row expansion. The generated
`coverage_matrix_tests.rs` harness can compare Pillow's outer success/error,
pixels, metadata, frames, and (where applicable) verification exception, but
Pillow exposes no successful-decode warning channel for `DiagnosticKind`,
stage, offset, or structure identity. Adding those fields to a parity row
would invent oracle data. Adding one of the 23 runtime-mutated byte streams to
the matrix would be a separate outer-result comparison and still could not
prove the Rust diagnostic; the 38 unchanged cases already map that shared
outer-result evidence to active Pillow rows. The normal
`diagnostic_manifest_matches_the_non_parity_contract` test is therefore the
correct acceptance boundary. Its executed lines may appear in aggregate LLVM
coverage, but that execution is not Pillow-parity coverage and no
coverage-only test is used to manufacture it.

The original COR-061 acceptance record was collected against implementation revision
`5c129baba0bfa044b0b79d3842af69736b269519` and is covered by Coverage MCP run
`4f4cc8a0-c716-4667-8720-f0d96e1b77d5`, snapshot
`24fe9c12-7cf7-4f2b-ac41-a1eda7e88828`: 72 tests passed with zero failures and
the snapshot retains 48,615/48,938 lines, 6,659/6,714 branches,
2,727/2,783 functions, and 75,687/76,106 regions. The feature matrix passed
947/947 checks in run `50c67cfb-d97b-425e-8afc-7508cefd1b90`; retained logs
show the diagnostic contract passing in all 22 feature-lane executions, with
zero package-cache and build-directory lock waits. The unchanged Pillow
parity matrix passed 1,434/1,434 checks in run
`2b13beca-1b3c-471f-b8a9-c386f594427d`; it remains outer-result evidence only.
These records are separate: the diagnostic test contributes only ordinary
Rust execution to the aggregate coverage snapshot, not a Pillow diagnostic
claim.

Current COR-061 revalidation is against implementation revision
`f7a8cd7efdf398c4df564ea29ffa2fcc99e6afdf`. Managed feature-matrix run
`42260e83-2f2b-4d7b-9219-76c415a43f0c` passed 991/991 checks; its retained log
contains 22 successful executions of
`diagnostic_manifest_matches_the_non_parity_contract`, one per feature lane,
with no build-directory or package-cache lock-wait matches. Managed Coverage
MCP run `f95bdb91-394f-461e-bc13-ea970997de88` passed 85/85 tests in 69,986 ms
and ingested snapshot `109c8920-2045-4cfb-a894-b2e2842ccfbc`. The current managed
Pillow parity run `6e993f5a-d280-4fc5-8191-41086674d433` passed 1,445/1,445
outer-result checks separately; it contains no diagnostic field or claim.
These current records revalidate COR-061 without converting Rust-only
diagnostic execution into Pillow-parity coverage.

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
format-specific structural paths for every enabled still and supported
sequence encoder. The generic whole-buffer fallback remains only as defensive
dispatcher behavior for a future or unassigned path; no current enabled format
uses it. JPEG still and one-frame sequence, TIFF still and multi-page sequence,
GIF still and GIF sequence, WebP still and sequence, ICO still, native AVIF
still and sequence, and the other one-frame sequence deliveries use the
structural paths described below. Each real public
call must normalize a rejecting destination to
`OutputWrite` with the selected format and corresponding `StillEncode` or
`SequenceEncode` stage, without an input offset, container identity, or
`UnsupportedReason`. These are Rust-only destination contracts: Pillow has no
caller-owned sink, so the cases are not parity rows and any aggregate
coverage from them is incidental evidence.

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
structural delivery. The WebP still structural-sink assertions in
`output_sinks_receive_the_exact_encoded_bytes` prove the same preflight and
delivery boundary. The JPEG still and one-frame sequence, plus TIFF still and
multi-page sequence, sink contracts separately prove exact-length preflight,
multiple header/strip/IFD or marker/scan writes, option mismatch, and
cancellation-prefix behavior. They do not prove
transient allocation limits or recoverable OOM behavior. Their aggregate coverage
is incidental evidence, not Pillow parity coverage, and no coverage-only hook
is added.

`EncodePolicy::max_work_units` follows the same boundary. Pillow has no
caller-controlled checkpoint budget or equivalent result, so
`encode_work_budget_is_a_non_parity_result_contract` is an ordinary Rust-only
contract rather than a generated parity row. It proves that an ample budget
preserves PNG bytes, a zero budget returns the typed
`ResourceLimit::EncodeWorkUnits` error for still and sequence dispatch, and a
zero-budget sink remains untouched. A work unit is one documented cooperative
encode checkpoint; the budget is deterministic work control, not CPU-time,
instruction-count, transient-allocation, or recoverable-OOM accounting. The
same contract now also proves JPEG still byte identity under an ample budget,
a bounded mid-encode rejection after more than one checkpoint, and a typed
zero-budget no-write result through the JPEG structural sink path. The
contract also proves that a pre-cancelled caller token takes precedence over a
zero work budget for still PNG and sequence GIF, without touching the sink.
A long PNG adaptive-filter row additionally charges a deterministic checkpoint
after each 1,024 filtered bytes while candidate filters are scored or the row
is emitted; the contract proves the resulting typed interior rejection and
untouched sink. Token-aware PNG compression additionally checks each input
chunk and stored-block boundary, each 1,024-byte stored-block-copy interval,
and the Adler-32 calculation for compression level 0, and checks every zlib-ng level's matcher, token expansion,
Huffman/bitstream emission, and checksum stages. The contract proves
ample-budget byte identity for default, stored, non-final stored-block,
explicit levels 1–5 and 7–9, and maximum-level PNG probes, plus typed matcher
rejection at `maximum: 20` in both whole-buffer and direct-sink paths; the
stored-block byte copy now charges after each 1,024 copied bytes in the
token-aware path, while the no-token path remains a bulk byte append. This is
Rust-only evidence because Pillow has no caller token,
work-budget result, or caller-owned sink. BMP row conversion additionally
charges an interior checkpoint
after each 1,024 pixels; the contract proves ample-budget byte identity and a
typed whole-buffer rejection, while its direct structural sink preserves the
validated header prefix before the same interior rejection. GIF RGB/RGBA palette quantization
additionally charges after each 1,024 pixels while preparing palette/index data;
High-color RGB median-cut preparation also charges around hash/order setup, axis
ordering, median-cut split stages, and 1,024-item split/partition scans. GIF LZW
charges an input-symbol checkpoint inside its dictionary pass. RGBA FASTOCTREE
bucket sorting additionally charges after each 1,024 sorting operations. The
contract proves ample-budget byte identity and typed RGB/RGBA palette-quantization rejection
in both whole-buffer and direct-sink paths, as well as the existing LZW
interior rejection and untouched sink. TIFF Deflate
additionally charges at each supplied input-row
boundary and inside the level-six matcher, then while expanding tokens,
analyzing Huffman trees, emitting stored/fixed/dynamic bitstreams, copying
stored-block bytes, and computing the Adler-32 trailer; the TIFF contract uses
a single wide row plus a materially larger budget to prove bounded rejection
inside the matcher and emission path. Lossy WebP VP8 additionally charges
after each batch of 1,024 RGB/RGBA-to-YUV conversion items, each batch of 1,024
analyzed macroblocks, and each batch of 1,024 frame-selection macroblocks, then
after color conversion, padding, analysis,
segment parameters,
mode selection, coefficient-probability
adaptation, partition emission, after each 4,096-bit logical first-partition
interval, after each 16,384-boolean first-partition bit interval, after each
4,096-bit logical coefficient interval, after each 16,384-boolean coefficient-bit
interval, and after each
1,024-byte boolean-bitstream output interval before final container assembly.
Lossless WebP
VP8L additionally charges around predictor tile scans/mode application,
cross-color multiplier search/transform tiles, entropy analysis, transform
selection/application, bounded backward-reference search/match-length/cache/
trace, histogram clustering, Huffman-tree/group emission, token-stream
intervals, 1,024-bit logical bitstream intervals, and 1,024-byte bitstream
output intervals; the same contract
proves unlimited lossless RGB byte identity, bounded typed rejection, and an
untouched sink, including separate exact-boundary probes for the logical
bitstream and emitted-output intervals. This is still Rust-only work-control
evidence: no Pillow row,
fixture, diagnostic field, or coverage-only hook is added.
The test's aggregate coverage is incidental evidence, and no coverage-only
hook or synthetic Pillow row is added.

Encode cancellation follows the same evidence boundary. Pillow has no
`CancellationToken`, no caller-owned `OutputSink`, and no equivalent
interruption result, so `encode_cancellation_is_a_non_parity_contract` and
the structural assertions in `output_sinks_receive_the_exact_encoded_bytes`
are ordinary fixture-backed Rust contracts rather than generated parity rows.
They check byte identity for uncancelled JPEG/PNG/BMP/TIFF/GIF/WebP/ICO still,
native AVIF still and sequence, GIF-sequence output, and one-frame
JPEG/BMP/WebP/ICO sequence sink output;
stable pre-cancelled errors, successful token-aware sink writes, and
JPEG/PNG/BMP/GIF/WebP/ICO/TIFF/native AVIF still and sequence sinks plus the
one-frame JPEG, WebP, and multi-page TIFF sequence sinks that can cancel
between structural writes while retaining only the delivered prefix. Native AVIF
still and sequence
sink delivery polls between validated top-level ISO-BMFF box segments. JPEG's
codec-local coverage
drill fires deterministic
internal row/block/scan checkpoints; the public test intentionally avoids
timing-sensitive interruption. The PNG and BMP still paths poll while
preparing rows; PNG additionally polls adaptive-filter and filtered-row
subsegments after each 1,024 row bytes, and token-aware PNG compression polls
stored-block boundaries, each 1,024-byte stored-block-copy interval, and every
zlib-ng level's matcher/emission/checksum
stages; the still and sink paths poll
between emitted structural segments; TIFF still encoding now polls page
preparation, row prediction, raw/PackBits/LZW work, and Deflate input-row plus
level-six matcher candidate/insertion/fizzle/position boundaries;
BMP still encoding also polls 1,024-pixel row-conversion subsegments;
GIF still encoding reuses the GIF block/frame/coalescing/output-assembly
checkpoints, polls RGB/RGBA palette quantization intervals, RGBA FASTOCTREE
bucket-sort intervals, and GIF LZW input-symbol intervals; WebP still encoding
polls
preparation, lossy VP8 RGB/RGBA-to-YUV conversion, macroblock-analysis, and
mode-selection subsegments plus analysis/coefficient-probability, 4,096-bit
logical first-partition intervals, 16,384-boolean first-partition bit intervals,
4,096-bit logical coefficient intervals, 16,384-boolean coefficient-bit intervals,
and
1,024-byte boolean-bitstream output intervals, and bitstream stages, lossless
VP8L predictor/cross-color/entropy/transform, bounded backward-reference
search/match-length/cache/trace, histogram/Huffman, token-stream, 1,024-bit
logical bitstream intervals, and 1,024-byte bitstream-output stages, codec-result,
metadata-assembly, and RIFF/chunk delivery boundaries; native AVIF still
encoding polls its preparation,
frame, and finalization checkpoints; GIF, TIFF, WebP, and native AVIF sequence
paths poll their implemented frame/coalescing/page/finalization checkpoints,
with native AVIF sequence delivery additionally polling between validated
top-level box segments.
ICO still encoding polls source-size validation, embedded PNG work or BMP row
assembly, and directory finalization.
The AVIF assertion is native-only because portable WASM AVIF encoding remains
target-unavailable. This slice does not claim universal interior interruption
beyond the implemented PNG row and token-aware stored-block/all-level Deflate
subsegments, TIFF Deflate matcher/emission
checkpoints, WebP RGB/RGBA-to-YUV conversion, macroblock-analysis, and
mode-selection subsegments, WebP coefficient-probability adaptation and
4,096-bit logical first-partition, 16,384-boolean first-partition-bit,
4,096-bit logical coefficient, and 16,384-boolean coefficient-bit intervals
plus the 1,024-byte boolean-bitstream output
intervals, the 1,024-bit logical VP8L bitstream intervals, and VP8L stages,
remaining finer WebP bitstream work beyond those intervals, progress callbacks, short-write
semantics, or rollback cleanup;
the separate checkpoint work-budget contract is covered below.
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

For the current implementation revision
`f7a8cd7efdf398c4df564ea29ffa2fcc99e6afdf`, the generated matrix reports:

| Metric | Count |
| --- | ---: |
| Active cases | 1,445 |
| Decode/inspect/verify cases | 1,024 |
| Encode cases | 421 |
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
`encode_sequence_to_sink` over JPEG/PNG/BMP/GIF/WebP/ICO/TIFF/native AVIF
still and sequence, one-frame JPEG/BMP/WebP/ICO sequence, multi-page TIFF
sequence,
GIF sequence, and multi-frame WebP sequence
fixtures must write bytes
identical to `encode`/`encode_sequence` with matching lengths for both `Vec<u8>`
and `&mut Vec<u8>` sinks. JPEG, PNG, BMP, GIF, WebP, ICO, and TIFF still, GIF
sequence, native AVIF still and sequence, one-frame JPEG/WebP sequence, and
multi-frame WebP sequence additionally
prove
multiple structural writes, policy preflight before the first write, and
cancellation between writes; one-frame JPEG/BMP and ICO, plus multi-page TIFF
sequence, additionally prove multiple structural writes and policy preflight.
GIF's structural split
is its signature and logical-screen descriptor, color tables, extension/image
sub-blocks, and trailer; the same validated block parser serves still and
sequence delivery. WebP's structural split is its 12-byte RIFF
header followed by each validated chunk header and payload/padding span. ICO's
structural split is a fixed 22-byte directory header followed by the complete
embedded PNG/DIB payload. TIFF's split is its header, strip/padding span, and
IFD/value tail. JPEG's split is its validated marker segments, SOS headers,
entropy-coded scan spans, restart markers, and EOI. Native AVIF's split is
each validated ISO-BMFF top-level box header followed by its non-empty payload
span; the complete native encoder buffer remains working state. A deterministic
failing write or flush must be reported as `ImageError::OutputWrite` with the
selected format and encode stage. The current contract proves one
post-delivery flush call and explicitly preserves
the delivered prefix on flush failure. The Rust-only
`partial_structural_sink_write_preserves_prefix_across_available_encoders`
contract also proves that every available still writer and each supported
multi-frame GIF/TIFF/WebP/native-AVIF sequence writer can accept a partial
prefix of a structural segment, reject the write as `OutputWrite`, preserve the
exact delivered prefix, report the selected `StillEncode` or `SequenceEncode`
stage, and avoid `flush`. Pillow has no caller-owned `OutputSink`, so this is
not a parity row; short-write behavior on other paths, rollback, and
partial-container cleanup remain future writer evidence, not claims made by
this contract.
The same contract exercises invalid-input errors through the JPEG structural
writer, with no sink write; those cases are Rust API/error evidence and are not
added to the Pillow parity matrix.

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

The preceding committed acceptance result was Coverage MCP run
`822bf053-61cb-4488-af1c-d2e23b15785c`, snapshot
`512dce77-6eda-4b2d-b8aa-9cbfcdd6a8a6`, at revision
`07f7a0977149803f96eec16ac8c2f3c1cb073eee`:

| Metric | Covered | Total |
| --- | ---: | ---: |
| Lines | 47,977 | 47,977 |
| Branches | 6,582 | 6,582 |
| Functions | 2,687 | 2,687 |
| Regions | 74,704 | 74,704 |

The same managed run executed 58 tests with zero failures or skips. The
TIFF Deflate work-budget contract includes distinct late row, tokenization,
and output checkpoints; these are ordinary Rust work-control evidence, not
synthetic Pillow parity rows.

The same committed revision passed the adaptive feature matrix in run
`0d19674c-4a01-4a06-9e54-2831a16c10d7` with 947 checks and zero failures in
30,478 ms, and the Pillow parity matrix in run
`888ba305-ff93-41c4-8d96-05c12f033c64` with 1,420 rows, zero failures, and zero
skips in 27,932 ms. The timings are retained execution evidence rather than a
universal benchmark claim because managed cache and build state can differ.

The earlier committed TIFF structural-sink acceptance result is Coverage MCP
run `c6231d19-598d-4706-bdfd-9385e3c05b50`, snapshot
`62014cef-25be-485e-a32f-ee1f9e9b606d`, at revision
`8e2b3e82d11c8aacfc8f2b05a3931d4464412d53`:

| Metric | Covered | Total |
| --- | ---: | ---: |
| Lines | 48,061 | 48,062 |
| Branches | 6,588 | 6,588 |
| Functions | 2,692 | 2,693 |
| Regions | 74,819 | 74,826 |

The managed run executed 58 tests with zero failures. The TIFF encoder file
itself is fully covered for lines, branches, and functions (1,321/1,323
regions); the aggregate snapshot retains one uncovered line, one function,
and seven regions. No coverage-only test was added. The same revision passed
the adaptive feature matrix in run
`a0cc505e-f44f-4b9e-9667-de52dca995b8` with 947 checks and zero failures in
35,146 ms, and the Pillow parity matrix in run
`d39ff85a-6d2e-41e8-b453-b4356943e3ff` with 1,420 rows, zero failures, and zero
skips in 33,569 ms. These durations are execution evidence rather than a
controlled runtime comparison;
the TIFF sink, policy, and cancellation cases are Rust-only contracts, not
Pillow-parity rows.

The parity-harness runtime follow-up was committed at revision
`ba06d91625ca72f81e94c0951ab6904b03e75ff6`. It keeps the same 1,417 active
manifest rows and partitions the independent decode and encode work into eight
format-scoped tests each; per-row success logging is opt-in through
`IMAGE_SLASH_STAR_VERBOSE_MATRIX=1`, while failure diagnostics and per-format
summaries remain unconditional. Coverage MCP run
`aaaf3d94-a362-4cc2-9609-8b930d60f583`, snapshot
`6d386562-ad46-4358-929b-e5b66dcd58ba`, passed 72 tests with zero failures and
retained 48,061/48,062 lines, 6,588/6,588 branches, 2,692/2,693 functions,
and 74,819/74,826 regions. The feature matrix passed 947 checks in
`e38b36f5-a130-48bc-92ea-388fda6893b2` (13,762 ms); the parity registration
passed 1,434 checks with zero failures and zero skips in
`608a6de1-a9d0-4820-8ffd-7287267f16f2` (53,544 ms). Its retained test output
reports 17 tests in 12.80 s, versus 3 tests in 33.48 s for the preceding
parity run `d39ff85a-6d2e-41e8-b453-b4356943e3ff`; total managed wall time is
not a controlled benchmark because build/cache state differs. The partition
changes scheduling and diagnostics only: no parity row, fixture, assertion,
or Pillow provenance boundary changed.

The JPEG work-budget contract was then added at revision
`df03084d90c790993a49364359ef31f11ebc50a2`. It remains an ordinary Rust-only
contract: Pillow has no checkpoint budget, cancellation token, or caller-owned
sink. Coverage MCP run `6309b1ae-4e4e-482d-9ee2-7472522bae19`, snapshot
`bc799a47-9076-4c8f-ab2b-65b0cbd7c0d7`, passed 72 tests with zero failures and
retained the same 48,061/48,062 lines, 6,588/6,588 branches, 2,692/2,693
functions, and 74,819/74,826 regions. The feature matrix passed 947 checks in
`70861b62-21e4-4aad-a4e0-249a1dc23d09` (40,936 ms), and the unchanged Pillow
parity scope passed 1,434 checks with zero failures and zero skips in
`66d39cf5-514a-46d9-b7a3-6ee4b7651c30` (23,106 ms). No parity row or fixture
was added for the caller-controlled work-budget behavior.

The work-budget precedence follow-up is committed at revision
`754416b786be09803991b5f04c1d275de49b299a`. It proves that caller cancellation
remains distinct from `EncodeWorkUnits` exhaustion for still and sequence
dispatch, including the no-write sink boundary; this is Rust-only behavior
because Pillow has no caller token or checkpoint budget. Coverage MCP run
`525f42b2-2cb9-49be-8e65-063eec7a0256`, snapshot
`31401e79-faa2-4244-add2-5697811a08d9`, passed 72 tests with zero failures and
retained 48,061/48,062 lines, 6,588/6,588 branches, 2,692/2,693 functions,
and 74,819/74,826 regions. The feature matrix passed 947 checks in
`30593a22-9120-4319-9552-0ae7a68be7b7` (48,022 ms), and the unchanged Pillow
parity scope passed 1,434 checks with zero failures and zero skips in
`531ac749-7aaa-4910-bfed-262e1eb66a20` (33,095 ms). No parity row or fixture
was added.

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
This is an ordinary Rust-only sink contract: Pillow has no caller-owned
destination, so no parity row or fixture was added. Coverage MCP run
`c92f3ac8-7122-487e-a374-a97f9a497813`, snapshot
`2d14db3c-a464-4768-960b-ec6d4c8e8c00`, passed 72 tests with zero failures and
retained 48,169/48,208 lines, 6,603/6,610 branches, 2,698/2,710 functions,
and 74,985/75,042 regions. The corrected feature matrix passed 947 checks
with zero failures in run `b480a67a-f626-4656-aefa-3a47e8521a32` (119,749 ms),
and the unchanged Pillow parity scope passed 1,434 checks with zero failures
and zero skips in run `5196a8d9-7c7b-43b8-b621-1a1a1812ebfa` (79,583 ms).
The managed parity scope and Pillow provenance remain unchanged.

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
reads those logs. By default `MATRIX_JOBS` derives roughly two logical CPUs per
active lane, capped at six; `CAPABILITY_JOBS` follows that bound. Both values
remain explicit overrides for constrained or unusually large runners. The
harness also derives `MATRIX_BUILD_JOBS` from the same lane bound and exports it
as `CARGO_BUILD_JOBS` inside each lane, keeping aggregate compiler fan-out near
the host CPU count; it remains an explicit override when a runner needs a
different build budget. A lane
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

The adaptive lane-bound follow-up was validated by managed runs
`790238ad-e8d8-4fce-9974-71560ffaac5d` and
`53c23521-c3d1-4b4b-9914-4b8d8f50883c`: both passed all 947 checks with zero
failures in 13,136 ms and 12,116 ms. The preceding four-lane baseline
`91c9bc98-5f22-41d2-95ad-d981957f1f82` also passed all 947 checks in 16,844 ms
on the same managed environment. This is observed scheduling evidence rather
than a universal benchmark claim; the retained run logs show no build-directory
lock waits and the capability-table result remains unchanged. The committed
revision `125b1b0` then passed the same scope in runs
`b016dd0f-6460-4bcf-8add-765b6ec8a8ee` (16,317 ms) and
`b4bf4180-f72c-4969-a66e-c355c402d9ac` (11,648 ms), illustrating the managed
runner variance that makes these observations non-universal benchmarks.

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
`81dae9af403dfa7358dfd833b25ef9c032582b5a` and is accepted at current
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

## Latest implementation acceptance

The preceding implementation revision was
`7d735af15cc448bd1be76b1569c317b8dcd0d9e7`. The runtime-first parity follow-up
is committed at `8c87e1d`: it keeps the active manifest at 1,417 rows and
partitions the expensive GIF/WebP encode work into hot-row workers while
keeping repeated source assets together. The preceding managed parity run
`57b0915e-c5ab-4a67-807e-f2481b1caa03` passed 1,445 checks with zero failures
or skips in 53,645 ms. Its retained output reports eight decode workers and
nineteen encode workers, every one of which reports zero failed or skipped
rows; the managed total is 1,417 active rows plus 28 worker test functions.
The worker partition changes scheduling only: no manifest row, fixture,
assertion, or Pillow provenance boundary changed. Managed durations are
runner/cache-sensitive execution records, not a controlled speedup claim.

The subsequent managed parity run
`a7791521-25e0-405e-9826-c0f3c3745d6c` passed 1,445 checks with zero failures
or skips in 60,053 ms at `7d735af`. Its retained output again reports 28
worker tests with zero failed or skipped rows; the managed total is 1,417
active rows plus 28 worker test functions. This confirmed the JPEG sequence
sink change did not alter the Pillow-parity manifest or its oracle boundary.

The multi-page TIFF structural-sink implementation landed in
`128406f` and the final feature-gating correction is `2147fbf`. TIFF sequence
sink delivery now validates and preflights every page, relocates the page IFD
chain, and emits the header, per-page strip/padding, and IFD/value spans as
separate writes with cancellation checks between segments. The Rust-only
acceptance test proves exact whole-buffer/sink byte identity and length,
policy rejection before the first write, and cancellation after the initial
TIFF header. Pillow has no caller-owned `OutputSink`, so this adds no parity
row or fixture and no new coverage-only hook.

The one-frame JPEG sequence sink slice landed in `7d735af`. It reuses the
validated JPEG marker/scan structural writer with `SequenceEncode` context;
the Rust-only acceptance test proves exact whole-buffer and sink bytes/length,
token-aware sink byte identity, cancellation after the initial `ff d8` prefix,
encoded-output policy rejection before the first write, and explicit
multi-frame rejection. Pillow has no caller-owned `OutputSink`, so the slice
adds no parity row or fixture and no coverage-only hook.

Coverage MCP run `e0190ba7-9e19-43ea-b40d-204401d503f8` passed 83 tests with
zero failures and ingested snapshot
`5ec8f2da-7df0-4b87-a30b-bd91ac986825` at revision `7d735af`. The snapshot
reports 48,738/49,108 lines, 6,671/6,732 branches, 2,735/2,802 functions,
and 75,913/76,394 regions. The feature matrix run
`918b49db-4fe3-4a7f-8451-3d8823c9baf6` passed 947/947 checks in 99,097 ms;
its retained log has no package-cache or build-directory lock-wait matches
and ends with `capability tables OK: every native and wasm32-wasip1 lane
agrees`. These aggregate and target-matrix results remain implementation
evidence, separate from Pillow parity.

The runtime optimization acceptance revision was
`045a908a580024212a03a1bb96dd83bdc27aa4ba`. The test-runtime follow-up adds a
lightly optimized Cargo test profile (`opt-level = 1`) for the codec-heavy
parity and coverage binaries; the feature-matrix script explicitly resets its
compile-heavy capability probes to `MATRIX_TEST_OPT_LEVEL=0`. No manifest row,
fixture, assertion, diagnostic-origin boundary, or production profile changed.
At this revision, managed Pillow parity run
`88c2db36-221f-4b1c-bb60-17a04cf12d70` passed 1,445/1,445 checks with zero
failures or skips in 844 ms. Coverage MCP run
`58803e1c-2c6d-401d-9376-825710e8a2cf` passed 83/83 tests in 48,676 ms and
ingested snapshot `a893e8ad-895b-40cb-9106-f776d44b62a8`; it retains
48,738/49,108 lines, 6,671/6,732 branches, 2,735/2,802 functions, and
75,913/76,394 regions. Feature-matrix run
`6c079600-9d20-4ed9-92a0-517068587d84` passed 947/947 checks in 56,641 ms;
its log has no package-cache or build-directory lock-wait matches and ends
with `capability tables OK: every native and wasm32-wasip1 lane agrees`.
These are observed execution records, not universal benchmark claims.

The current PNG interior work-budget slice is implemented at
`0e647e9b3eab31b704b7d2262525ab90a2f835e5`: adaptive filter scoring and
filtered-row emission charge a checkpoint after each 1,024 row bytes in still
and one-frame sequence paths. This is Rust-only evidence because Pillow
exposes neither a caller token nor a work-budget result; the contract test
proves the typed `EncodeWorkUnits` error and an untouched sink before any
structural write. The no-token encoder path is unchanged, so no parity row,
fixture, diagnostic field, or coverage-only hook was added.

Managed Pillow parity run `bad36d4a-c88f-4384-91ba-5f9df79eea6e` passed
1,445/1,445 checks with zero failures or skips in 761 ms. Coverage MCP run
`af8efaba-fdb4-4c89-bb4d-577a9881a958` passed 83/83 tests in 44,235 ms and
ingested snapshot `a00cbf8e-c8f8-491e-981f-95ab9a34c358`; it reports
48,755/49,125 lines, 6,679/6,740 branches, 2,734/2,801 functions, and
75,938/76,421 regions. The changed PNG encoder file is 599/599 lines,
62/62 branches, 44/44 functions, and 1,014/1,016 regions; Coverage MCP
records the LLVM segment normalization warning for aggregate regions. Feature
matrix run `a1a01a8d-f719-42b7-930e-ffcc97273c36` passed 947/947 checks in
64,537 ms; its retained log has no package-cache or build-directory lock-wait
matches and ends with `capability tables OK: every native and wasm32-wasip1
lane agrees`. These are observed implementation and target-matrix records,
separate from Pillow parity. Interior work in other codec rows, deeper
Deflate/structural interruption, allocation accounting, and rollback remain
open.

The current lossy WebP/VP8 work-budget slice is implemented at
`a5c39499a33f06668fb145abf6d6051344f6ba3f`, with its RGB/RGBA contract test
at `90fcc0f0ea2ee8b4ad861e6bf591d359b47d1833`: token-aware VP8 encoding now
charges checkpoints after color conversion, padding, analysis, segment
parameters, mode selection, coefficient-probability adaptation, partition
emission, and final container assembly. The ordinary no-token encoder path is
unchanged. This remains Rust-only evidence because Pillow exposes neither a
caller token nor a work-budget result; the contract proves unlimited RGB and
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
indices during palette compaction. The token is threaded through ordinary frame
preparation and coalesced full-canvas normalization. Separate no-token branches
preserve the existing tight loops and encoded bytes. The Rust-only
`encode_work_budget_is_a_non_parity_result_contract` contract proves ample-budget
byte identity, a typed whole-buffer rejection at the first RGBA quantization
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
contract proves ample-budget byte identity, a typed whole-buffer rejection at
the first RGBA octree cell interval (`maximum: 6`, `observed: 7`), the same
direct-sink rejection (`maximum: 5`, `observed: 6`), and untouched sink state.
Pillow has no caller token or work-budget result, so this adds no parity row,
fixture, diagnostic origin, or coverage-only hook.

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
second coarse-reduction call at line 2819; the transparent-pixel normalization
path remains uncovered at lines 2315-2324. The FASTOCTREE bucket-sort and
high-color RGB median-cut loops remain non-checkpointed; no synthetic
coverage-only input was added. The aggregate snapshot retains the LLVM
segment-normalization warning. These are Rust-only implementation and target
records separate from Pillow parity. Remaining GIF bucket-sort work,
transparent-normalization coverage, high-color RGB quantizer work, other codec
interior work, finer WebP bitstream work, transient allocation accounting,
short-write/rollback, and remaining non-checkpointed work-budget semantics
remain open.

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
checkpoint branches. The aggregate snapshot retains the LLVM segment-
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
`62508a58b1a16fde150067b6cd43930b6e798dd3`. It changes only
`scripts/test_feature_matrix.sh`: the default isolated feature-gate test
binaries now use `MATRIX_TEST_OPT_LEVEL=1` instead of level 0. All 33 native,
`wasm32-unknown-unknown`, and `wasm32-wasip1` lanes, all 45 feature-gate
assertions per lane, and the capability-table no-drift check remain in place;
no production profile, manifest row, fixture, assertion, or evidence origin
changed.

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
harness runtime evidence; it adds no parity row, fixture, diagnostic origin,
or coverage-only hook.

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
newly emitted output bytes, including final buffered-byte flushes. The
compression-search trials carry both counters in their cloned writer state
when the shortest candidate is selected; the no-token controller remains a
monomorphized no-op path. The Rust-only
`encode_work_budget_is_a_non_parity_result_contract` contract uses the
patterned 128x128 RGB probe to prove whole-buffer and direct-sink rejection at
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

Remaining finer VP8L bitstream work beyond the 4,096-bit logical interval and
1,024-byte output interval, other codec interior work, transient allocation
accounting, short/interrupted output, rollback, and remaining non-checkpointed
work-budget semantics remain open.

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
for whole-buffer and `maximum: 588`, `observed: 589` for the direct sink. Both
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
compression checks input-chunk and stored-block boundaries plus the final
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

At that revision, remaining PNG non-level-0/6 interior checkpoints, deeper
stored-block byte-copy interruption, finer VP8/VP8L bitstream work, other codec interior work,
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

Remaining finer VP8 bitstream work beyond its 4,096-bit logical first-partition,
16,384-boolean first-partition/coefficient-bit, and 1,024-byte output intervals;
finer VP8L bitstream work beyond its 1,024-bit logical and 1,024-byte output
intervals; other codec interior work, transient allocation accounting,
short/interrupted output, rollback, and remaining non-checkpointed work-budget
semantics remain open.

Historical claim-ledger acceptance record:

Coverage MCP run: `9bbe6760-7aa9-4ed8-8b31-bbf65444b85a`

Snapshot: `f9a2fc69-ad68-493e-9c46-8837d0dd8d52`

Coverage revision: `f1048bc0399fad9801559ca7fcfd3163427b5832`

Coverage MCP recorded 58 passed tests with zero failures and 100% line, branch,
function, and region coverage: 47,943 lines, 6,578 branches, 2,686 functions,
and 74,654 regions. The ICO still and one-frame ICO sequence sink cases,
sink finalization errors, and
the deterministic encode work-budget contract execute the real dispatcher and
structural/error paths; this is internal Rust evidence, not a synthetic Pillow
parity case. The build-script decision tests likewise prove target-tool
environment precedence as build invalidation evidence, not codec coverage.

The same committed revision passed the feature matrix in run
`1a0c0f1c-d5d7-4210-a24f-503d001a3d8f` (947 checks, zero failures) and the
Pillow parity matrix in run `4ed3cd5c-3e92-4f2b-bd02-1b71a97ad0ed` (1,420 rows,
zero failures). Their durations are retained execution evidence rather than a
controlled benchmark because managed cache and build state can differ.

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
`f6a5689522677786e9f1023cd910a09506866a9ee3ee1b7d23b0927811ca7aaf`

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
| Diagnostic provenance audit | `tests/fixtures/diagnostic_manifest.json`; `scripts/verify_diagnostic_provenance.py` | Static separation of unchanged parity baselines, runtime mutations, and Rust-only diagnostic fields | A Pillow diagnostic or additional parity behavior |

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
- native feature-gate tests for every feature lane;
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
`wasm32-wasip1` lanes under one bounded completion-driven scheduler. On a cold
root, it derives `MATRIX_JOBS` from host logical CPUs (roughly two logical CPUs
per lane, capped at six), then derives the Rust test-harness and Cargo compiler
worker counts from that bound. When all three retained all-feature target roots
are present, the scheduler treats the root as warm: it allows up to twelve
independent lanes, uses one Cargo compiler worker per lane, and caps the
derived test-harness budget at two workers. `MATRIX_JOBS`,
`MATRIX_TEST_THREADS`, and `MATRIX_BUILD_JOBS` can override the derived values
for a constrained or unusually large CI runner. This bounds aggregate process,
compiler, and test-thread fan-out without dropping any lane or assertion while
avoiding a cold-build fan-out on disposable roots.

The matrix defaults `MATRIX_TEST_OPT_LEVEL` to `1`, matching the repository's
lightly optimized test profile. The feature-gate suite executes real codec
work-budget and cancellation contracts, so an unoptimized level-0 binary makes
the WASI runtime lane disproportionately slow; callers may override this value
when compile time matters more than runtime.

Cross-compilation proves compilation, not semantic browser or WASM runtime
parity. The `wasm32-wasip1` lanes are real runtime evidence for feature-gate
and capability-table behavior; full semantic manifest execution in a WASM
runtime remains planned.

When the optional native AVIF bridge is enabled, `build.rs` declares Cargo
rerun triggers for every compiler and archiver variable it consults:
`CC_<target>`, `TARGET_CC`, `CC`, and the corresponding `AR` names. The
`build_script_tests` target checks target-name normalization and the specific
to-target-wide-to-host precedence. This is build invalidation evidence, not
Pillow parity or implementation coverage for the codec itself.

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

The test targets have separate evidence roles:

| Target or command | Evidence origin | What its coverage means |
| --- | --- | --- |
| `pillow-byte-pixel-parity` (`cargo test --all-features --test coverage_matrix_tests`) | `pillow_fixture` | Generated outer-result assertions from the pinned Pillow matrix; it has no successful-decode diagnostic field. |
| `feature_gate_tests` and `decode_policy_tests` | `defensive_model` or another explicit non-Pillow contract | Public Rust behavior such as diagnostics, policies, cancellation, and sink semantics; any LLVM execution is incidental aggregate coverage. |
| `all-features-llvm-cov-json-nightly-branch` | aggregate instrumentation | Combined execution across parity and non-parity targets; it is not a Pillow-parity coverage number. |

The first row can execute a shared baseline asset, and the second row can use
the same asset or a runtime mutation derived from it, but those are different
claims. A Pillow row proves only the fields Pillow returns; the Rust contract
proves the additional defensive fields. Coverage percentages do not assign
ownership to either origin, so the repository never labels the aggregate
snapshot as parity coverage.

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
