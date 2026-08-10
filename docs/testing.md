# Oracle, fixtures, tests, and coverage

Status: current contributor reference

Reviewed: 2026-08-10 against production implementation and Rust test/runtime
revision `1d1b36100925f830408f5d41f0026e71fd220d6e`, and benchmark-protocol
revision `4415a84463103d3d0916821a3ed8637b832442d6`; the claim-ledger fixture
tuple remains anchored to base revision
`487348d01389eb8d100b8a668c9921d97634c022`.
The latest exact-head managed validation runs are Pillow parity
`bbd0f95f-d55d-4c90-b097-eacfdb96c372` (1,445/1,445 passed in 3,779 ms) and
feature matrix `34791756-b280-4de5-9428-accc71974d13` (passed in 18,536 ms);
both recorded checkout HEAD
`1d1b36100925f830408f5d41f0026e71fd220d6e`.
The accepted Coverage MCP snapshot is
`44cec31e-7345-4673-a9a4-e9f8fa21cc08` from run
`beda2230-4d77-446c-8ce4-91700552cdc4`; it records 55,926/56,803 lines,
8,011/8,228 branches, 3,122/3,218 functions, and 85,972/87,930 regions at
the same source revision. The histogram file records 872/873 lines, 184/184
branches, and 43/43 functions; predictor records 366/366 lines, 68/68
branches, and 24/24 functions; cross-color records 517/530 lines, 83/86
branches, and 27/27 functions. The known LLVM JSON segment-normalization
warning remains. These exact-head records are test-result and Rust coverage
evidence, not Pillow allocator or parity metrics.

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
fixture-derived `33x33.jpg` JPEG probe additionally proves the forward-DCT and
quantization checkpoint after a completed 8x8 block at `maximum: 70`,
`observed: 71`, in both whole-buffer and direct-sink paths with the sink
untouched. The patterned 64x64 JPEG probe additionally proves the 1,024-byte
entropy-output interval rejection at `maximum: 150`, `observed: 151`, in both
whole-buffer and direct-sink paths with the sink untouched. The
fixture-derived `large.jpg` JPEG probe additionally proves the chroma-
downsample output-pixel checkpoint at `maximum: 228`, `observed: 229`, in both
whole-buffer and direct-sink paths with the sink untouched. The
same `large.jpg` probe with `optimize=true` additionally proves the optimized
baseline Huffman frequency checkpoint after each 1,024 AC coefficients at
`maximum: 1,220`, `observed: 1,221`, in both whole-buffer and direct-sink paths
with the sink untouched. The optimized-frequency boundary is part of the same
Rust-only contract because Pillow has no caller token, work-budget result, or
caller-owned sink.
The low-entropy generated 512x512 RGB probe reaches exactly 32x32 default
4:2:0 MCUs and proves the separate baseline entropy traversal checkpoint after
each 1,024 MCUs at whole-buffer and direct-sink `maximum: 7,720`,
`observed: 7,721`, with sentinel `[0x63]` untouched. Its focused contract
completed in 3.23 seconds locally after reducing unnecessary entropy
complexity; this is a runner-sensitive observation, not a universal benchmark.
The MCU boundary is Rust-only because Pillow has no caller token, work-budget
result, or caller-owned sink, so no parity row, fixture, diagnostic origin, new
test function, or coverage-only hook was added.
The same committed `large.jpg` probe with `progressive=true` additionally
proves progressive DC/AC scan-event block-slot checkpointing after each 1,024
slots at `maximum: 1,364`, `observed: 1,365`, in both whole-buffer and
direct-sink paths with sentinel `[0x60]` untouched. This progressive boundary
is also Rust-only because Pillow has no caller token, work-budget result, or
caller-owned sink.
The same progressive probe additionally proves the separate scan-event
frequency-gathering checkpoint after each 1,024 events at `maximum: 1,378`,
`observed: 1,379`, in both whole-buffer and direct-sink paths with sentinel
`[0x61]` untouched. This event-frequency boundary is also Rust-only because
Pillow has no caller token, work-budget result, or caller-owned sink.
The separate constant `DecodedImage::new(257, 129, vec![0; 257 * 129 * 3],
ColorType::Rgb8)` progressive probe additionally exercises the AC coefficient
traversal checkpoint after each 1,024 coefficients at `maximum: 1,378`,
`observed: 1,379`, in both whole-buffer and direct-sink paths with sentinel
`[0x62]` untouched. This coefficient boundary is also Rust-only because
Pillow has no caller token, work-budget result, or caller-owned sink; the
existing event-frequency probe remains a separate fixture-derived boundary.
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
ordering, median-cut split stages, and 1,024-item split/partition scans, and its
nearest-palette candidate ordering and bounded candidate scan after each 1,024
work items. GIF LZW
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
after each batch of 1,024 RGB/RGBA-to-YUV conversion items and each batch of
1,024 scanned or flattened RGBA transparent-area cleanup pixels, compressed/raw
alpha-stream buffer copies and WebP container/metadata assembly copies after
each 1,024 output bytes, each batch of
1,024 required padded-plane items, each 64 completed 4×4 histogram blocks,
each 64-value segment-clustering alpha-domain chunk, analyzed macroblocks, and
segment-assignment macroblocks,
plus each batch of 64 frame-selection
macroblocks
(roughly 1,024 luma 4×4 blocks), then
after color conversion, padding, analysis,
segment parameters,
mode selection, coefficient-probability adaptation, and first-partition
segment-probability prepass after each 1,024 selected macroblocks, then
partition emission, after each 8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, 8,192-bit, 32,768-bit, 65,536-bit, 131,072-bit, and 262,144-bit logical first-partition
interval, after each 16,384-boolean first-partition bit interval, after each
8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, 8,192-bit, 32,768-bit, 65,536-bit, 131,072-bit, 262,144-bit, 524,288-bit, 1,048,576-bit, and 2,097,152-bit logical coefficient intervals, after each 16,384-boolean coefficient-bit
interval, and after each
1,024-byte boolean-bitstream output interval before final container assembly.
The token-aware native lossless VP8L writer also copies its complete RIFF
frame payload in 1,024-byte chunks; the no-token path retains one bulk copy.
Lossless WebP
VP8L additionally charges around predictor mode-application wide source-row
copies in completed 1,024-pixel chunks, tile
scans/mode application and
subtract-green transforms after each 1,024 pixels,
cross-color multiplier search/transform tiles and sampling scans/compaction,
sampled meta-pixel materialization after each 1,024 retained histogram symbols,
including meta-histogram row/column comparisons and symbol compaction after
each 1,024 symbols,
entropy-mode histogram-cost analysis after each 64 symbols, transform
selection/application, and wide-row entropy-mode pixel histogram scans after
each completed 1,024-pixel chunk, bounded backward-reference
length-cost table and equal-cost interval setup after each 1,024 entries,
non-saturated interval split/merge after each 1,024 interval-work entries, and
saturated cost-interval fallback scans after each 1,024 entries,
search/match-length/cache, token-aware trace, path reconstruction, and token replay
after each 256 consumed pixels (the no-token trace/replay retains its 1,024-pixel cadence), repeated-run hash-chain insertion, long
backward-reference result backfills after each 256 entries, copy-token
cache-population scans after each 256 pixels, and histogram-cluster token-to-row
transitions after each 256 rows, plus token/Huffman cost
scans after each 1,024 tokens or 64 symbols,
Huffman-tree simple-tree symbol-discovery scans after each 64 code-length slots,
Huffman RLE preparation, including reverse-tail fixed-alphabet scans, and
in-run code-length scans after each 64 symbols,
Huffman RLE token materialization after each 16 emitted compressed code-length
tokens,
canonical-code assignment scans after each 64 code-length symbols, Huffman-tree ordering comparisons after each 64 comparisons,
Huffman-tree insertion scans after each 64 candidate nodes,
Huffman-tree active-symbol census, leaf-materialization, and maximum-depth
scans after each 64 code-length slots,
Huffman-tree code-length-token frequency, trailing zero-repeat token trim, and
code-length-emission scans after each 16 compressed token entries, entropy-mode histogram-cost
analysis after each 64 symbols,
histogram-clustering populated-tile collection, min/max, and bin-assignment
pre-passes after each 64 tile histograms, token-to-row transitions after each
256 rows, and histogram clustering (including token-aware
population scans after each 64
symbols), Huffman-tree/group emission, token-stream
intervals, 8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, 8,192-bit, 16,384-bit, 32,768-bit, 65,536-bit, 131,072-bit, 262,144-bit, 524,288-bit, 1,048,576-bit, and 2,097,152-bit logical bitstream intervals, and 1,024-byte
bitstream output intervals; the same contract
proves unlimited lossless RGB byte identity, bounded typed rejection, and an
untouched sink, including exact endpoint probes for the 8-bit, 32-bit, 128-bit,
512-bit, 2,048-bit, and 8,192-bit logical intervals, with the nested 16-bit,
64-bit, 256-bit, 1,024-bit, and 4,096-bit intervals traversed by those calls;
the larger 16,384-bit, 32,768-bit, 65,536-bit, 131,072-bit, 262,144-bit,
524,288-bit, 1,048,576-bit, and 2,097,152-bit logical intervals, and the emitted-output
intervals. This is still Rust-only work-control
evidence: no Pillow row,
fixture, diagnostic field, or coverage-only hook is added.
The historical 16-bit extension is implemented at
`1378f119a65ebd06f1d848f4757684c83e597444`. The same contract now proves exact
whole-buffer/direct-sink rejection for the first 16-bit logical interval in
each WebP path: VP8 first-partition at `maximum: 102`, `observed: 103` and
`maximum: 101`, `observed: 102`; VP8 coefficient at `maximum: 289`,
`observed: 290` and `maximum: 288`, `observed: 289`; and VP8L at
`maximum: 145`, `observed: 146` and `maximum: 144`, `observed: 145`.
The retained 32/64/128/256/512-bit probes now use their actual compact-fixture
edges, and every bounded sink remains at its sentinel prefix. Three warm exact
contract repeats passed in 0.58–0.59 seconds of test-body time; the full
all-feature `cargo test --all-features --locked --tests` run passed 82 tests
with zero failures. These are Rust-only work-control results: Pillow has no
caller token, work-budget result, or caller-owned sink, so this adds no parity
row, fixture, diagnostic origin, or coverage-only hook.
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
internal RGB-to-YCbCr conversion, chroma-downsample output, row/block/scan,
progressive scan block slots, progressive scan-event frequency items,
progressive scan coefficient items, optimized baseline Huffman frequency, and
1,024-byte entropy-output
checkpoints; the public
test intentionally avoids
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
preparation, lossy VP8 RGB/RGBA-to-YUV conversion, RGBA transparent-area cleanup,
macroblock-analysis, analysis histogram construction after each 64 completed
4×4 blocks, intra4 mode-selection candidate-trial stages, forward- and
inverse-transform row/column subpasses, non-trellis quantization coefficients,
method-6 trellis-quantization coefficient candidates and path-reconstruction
nodes, squared-error pixels, spectral-distortion weighted-transform
row/column passes, residual-cost coefficients, candidates, and each completed
luma 4×4 block,
the outer 64-macroblock mode-selection checkpoint, and
mode-selection subsegments plus analysis/coefficient-probability, 8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, 8,192-bit, 32,768-bit, 65,536-bit, 131,072-bit, and 262,144-bit
logical first-partition intervals, 16,384-boolean first-partition bit intervals,
8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, 8,192-bit, 32,768-bit, 65,536-bit, 131,072-bit, 262,144-bit, 524,288-bit, 1,048,576-bit, and 2,097,152-bit logical coefficient intervals, 16,384-boolean coefficient-bit intervals,
and
1,024-byte boolean-bitstream output intervals, and bitstream stages, lossless
VP8L predictor/cross-color/entropy/transform, bounded backward-reference
cost/length-table initialization and length-cost/equal-cost interval setup
after each 1,024 entries,
non-saturated interval split/merge after each 1,024 interval-work entries, and
saturated cost-interval fallback scans after each 1,024 entries,
search/match-length/cache, token-aware trace, path reconstruction, and token replay
after each 256 consumed pixels (the no-token trace/replay retains its 1,024-pixel cadence), repeated-run hash-chain insertion, long
backward-reference result backfills after each 256 entries, and copy-token
cache-population scans after each 256 pixels, plus token/Huffman cost
scans after each 1,024 tokens or 64 symbols,
Huffman-tree simple-tree symbol-discovery scans after each 64 code-length slots,
Huffman RLE preparation, including reverse-tail fixed-alphabet scans, and
in-run code-length scans after each 64 symbols,
Huffman RLE token materialization after each 16 emitted compressed code-length
tokens,
canonical-code assignment scans after each 64 code-length symbols, Huffman-tree ordering comparisons after
each 64 comparisons, Huffman-tree insertion scans after each 64 candidate nodes,
and Huffman-tree active-symbol census, leaf-materialization, and maximum-depth
scans after each 64 code-length slots,
Huffman-tree code-length-token frequency, trailing
zero-repeat-token trim, and code-length-emission scans after each 16 compressed token entries, histogram
population, combined entropy-cost, and
histogram-merge scans after each 64 symbols, histogram/Huffman, token-stream, 8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit,
512-bit, 1,024-bit, 2,048-bit, 4,096-bit, 8,192-bit, 16,384-bit, 32,768-bit, 65,536-bit, 131,072-bit, 262,144-bit, 524,288-bit, 1,048,576-bit, and 2,097,152-bit logical bitstream intervals, and 1,024-byte bitstream-output stages, codec-result,
metadata-assembly, and RIFF/chunk delivery boundaries; JPEG still encoding
additionally polls after each 1,024 progressive scan block slots, each 1,024
progressive scan-event frequency items, each 1,024 progressive scan coefficient
items, and emitted entropy bytes; native AVIF
still
encoding polls its preparation,
frame, and finalization checkpoints; GIF, TIFF, WebP, and native AVIF sequence
paths poll their implemented frame/coalescing/page/finalization checkpoints,
with native AVIF sequence delivery additionally polling between validated
top-level box segments.
ICO still encoding polls source-size validation, embedded PNG work or BMP row
assembly, and directory finalization.
The AVIF assertion is native-only because portable WASM AVIF encoding remains
target-unavailable. This slice does not claim universal interior interruption
beyond the implemented JPEG 1,024-pixel RGB-to-YCbCr conversion, 1,024-pixel
chroma-downsample output, baseline entropy traversal after each 1,024 MCUs,
optimized baseline Huffman frequency gathering after
each 1,024 AC coefficients, progressive scan block slots after each 1,024
blocks, progressive scan-event frequency items after each 1,024 events,
progressive scan coefficient items after each 1,024 coefficients, and 1,024-byte
entropy-output intervals, PNG row and
token-aware stored-block/all-level Deflate
subsegments, TIFF Deflate matcher/emission
checkpoints, WebP RGB/RGBA-to-YUV conversion, required padded-plane edge
replication, RGBA transparent-area cleanup, macroblock-analysis, 64-value
segment-clustering alpha-domain chunks, and segment-assignment, intra4
mode-selection candidate-trial stages, forward- and
inverse-transform row/column subpasses, non-trellis quantization coefficients,
method-6 trellis-quantization coefficient candidates and path-reconstruction
nodes, squared-error pixels, spectral-distortion weighted-transform
row/column passes, residual-cost coefficients, candidates, and each completed
luma 4×4 block,
the outer 64-macroblock mode-selection checkpoint, and mode-selection work
beyond those implemented boundaries, WebP coefficient-probability
adaptation and first-partition segment-probability prepass after each 1,024
selected macroblocks, and
8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, 8,192-bit, 32,768-bit, 65,536-bit, 131,072-bit, and 262,144-bit logical first-partition, 16,384-boolean first-partition-bit,
8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, 8,192-bit, 32,768-bit, 65,536-bit, 131,072-bit, 262,144-bit, 524,288-bit, 1,048,576-bit, and 2,097,152-bit logical coefficient, and 16,384-boolean coefficient-bit intervals
plus the 1,024-byte boolean-bitstream output
intervals, the 8-bit, 16-bit, 32-bit, 64-bit, 128-bit, 256-bit, 512-bit, 1,024-bit, 2,048-bit, 4,096-bit, 8,192-bit, 16,384-bit, 32,768-bit, 65,536-bit, 131,072-bit, 262,144-bit, 524,288-bit, 1,048,576-bit, and 2,097,152-bit logical VP8L bitstream intervals, lossless VP8L palette-index lookup candidate scans after each 64 palette entries, palette sign collection and nearest-delta candidate scans after each 64 palette entries or candidate values, and VP8L stages,
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

Current acceptance record: WebP VP8L fixed color-cache Huffman code-length
workspace

The production and Rust test/runtime slice is implemented at
`1d1b36100925f830408f5d41f0026e71fd220d6e`, following the WebP alpha-palette
fixed-storage boundary at `ccf9c32bb9746629263d3028c430448223df64e7`.
`read_huffman_codes` now reuses one format-bounded `[u16; 2_328]` workspace
across sequential non-simple Huffman trees. The maximum covers the 280
ordinary green symbols plus the 2,048 symbols selected by the format's 11-bit
color-cache field; `HuffmanTree::build_implicit` still copies each borrowed
slice into the owned tree before reuse. Huffman ordering, bit consumption,
decoded bytes, errors, and sink output remain unchanged.

This is Rust implementation and Rust-only fixed-workspace evidence. Existing
WebP fixture rows provide byte/error regression, not proof of storage ownership
or allocation counts; feature-gated Rust contracts and the feature matrix are
the separate non-Pillow evidence. No parity row, fixture-manifest row,
diagnostic origin, new test function, coverage-only hook, or unit test was
added.

The clean schema-`@3` benchmark measured Pillow parity at 0.919695 s wall /
2.734602 user s / 0.184715 sys s / 240,648,192-byte peak RSS and the separate
Rust-only feature-gate workload at 2.678621 s wall / 2.632672 user s /
0.442356 sys s / 254,590,976-byte peak RSS. The native release build measured
6.601950 s wall / 32.388708 user s / 0.326316 sys s / 874,430,464-byte peak
RSS and produced a 7,970,488-byte `rlib`; the WASM compile measured 1.655656 s
wall / 1.299288 user s / 0.650604 sys s / 489,865,216-byte peak RSS and
produced a 24,071,618-byte artifact. These are host/cache/toolchain
observations, not comparative or universal performance claims; allocation
counts, retained cache bytes, caller-buffer reuse, peak stack/recursion, and
WASM runtime resources remain unmeasured.

Exact-head managed Pillow parity run
`bbd0f95f-d55d-4c90-b097-eacfdb96c372` passed 1,445/1,445 checks. Exact-head
feature-matrix run `34791756-b280-4de5-9428-accc71974d13` passed all configured
native/WASM lanes in 18,536 ms. Nightly LLVM run
`beda2230-4d77-446c-8ce4-91700552cdc4` passed 85/85 tests in 56,141 ms and
ingested snapshot `44cec31e-7345-4673-a9a4-e9f8fa21cc08`: 55,926/56,803
lines, 8,011/8,228 branches, 3,122/3,218 functions, and 85,972/87,930
regions. Compared with preceding accepted snapshot
`68fda68a-4889-4d44-b57d-f2a7ab388677`, covered and total lines, branches,
functions, and regions were unchanged. The changed
`src/codecs/webp/native/lossless.rs` projection is 1,255/1,257 lines,
130/134 branches, 53/53 functions, and 1,620/1,624 regions; its two uncovered
lines and eight partial-branch lines remain existing implementation gaps. The
known LLVM JSON segment-normalization warning remains. These are
implementation/Rust coverage metrics, not Pillow-parity coverage.

Current acceptance record: WebP alpha-palette fixed storage

The production and Rust test/runtime slice is implemented at
`ccf9c32bb9746629263d3028c430448223df64e7`, following the WebP VP8L
color-indexing transform table storage boundary at
`c8b3cbe6815de601795c5f5482ce0a3738c31b9d`. Alpha values have a fixed
8-bit alphabet, so `collect_alpha_palette` now returns a bounded
`[u8; 256]` plus length instead of allocating a `Vec<u8>` before the existing
fixed palette-index and delta workspaces are used. The raw alpha plane remains
borrowed for the compressed-versus-uncompressed representation decision, and
the image-scaled packed `Vec<u32>` remains because the entropy writer needs
random-access packed pixels while the raw candidate must stay available. Alpha
ordering, palette deltas, encoded bytes, errors, and sink output remain
unchanged.

This is Rust implementation and Rust-only bounded-workspace evidence. The
existing Pillow ALPH rows provide byte/error regression, not proof of storage
ownership or allocation counts; feature-gated Rust contracts and the feature
matrix are the separate non-Pillow evidence. No parity row, fixture-manifest
row, diagnostic origin, new test function, coverage-only hook, or unit test was
added.

The clean schema-`@3` benchmark measured Pillow parity at 0.921355 s wall /
2.786805 user s / 0.192402 sys s / 254,771,200-byte peak RSS and the separate
Rust-only feature-gate workload at 2.620295 s wall / 2.654380 user s /
0.430012 sys s / 254,803,968-byte peak RSS. The native release build measured
6.620850 s wall / 32.840098 user s / 0.328304 sys s / 876,085,248-byte peak
RSS and produced a 7,968,952-byte `rlib`; the WASM compile measured 3.051974 s
wall / 14.448787 user s / 0.892179 sys s / 854,409,216-byte peak RSS and
produced a 24,073,320-byte artifact. These are host/cache/toolchain
observations, not comparative or universal performance claims; allocation
counts, retained cache bytes, caller-buffer reuse, peak stack/recursion, and
WASM runtime resources remain unmeasured.

Exact-head managed Pillow parity run
`2dae75fa-5011-4d42-9777-c6beadbf65e9` passed 1,445/1,445 checks in 7,628 ms.
Exact-head feature-matrix run
`eac7905f-ec64-4deb-a6da-137ad7f75d3b` passed all configured native/WASM lanes
in 27,088 ms. Nightly LLVM run
`81d3eb83-1dcb-419f-adc6-445a4ea1c6ff` passed 85/85 tests in 62,670 ms and
ingested snapshot `68fda68a-4889-4d44-b57d-f2a7ab388677`: 55,926/56,803
lines, 8,011/8,228 branches, 3,122/3,218 functions, and 85,972/87,930
regions. Compared with preceding accepted snapshot
`46bdf0fa-d59a-40ab-9a1c-f0e85f28e02b`, covered/total lines rose 6/6,
branches 0/0, functions 0/0, and regions 4/4. The changed WebP encoder
projection is 2,405/2,489 lines, 511/540 branches, 89/89 functions, and
3,471/3,751 regions; its 34 uncovered-line and 34 partial-branch-line counts
are unchanged. The known LLVM JSON segment-normalization warning remains.
These are implementation/Rust coverage metrics, not Pillow-parity coverage.

Current acceptance record: WebP VP8L color-indexing transform table storage

The production and Rust test/runtime slice is implemented at
`c8b3cbe6815de601795c5f5482ce0a3738c31b9d`, following in-place VP8L palette
packing at `aea0c723a07e4ae3a8ac43fe76824197c5016427`. VP8L color-indexing
transforms are bounded to 256 RGBA entries, so `LosslessDecoder` now retains
the decoded table in decoder-owned `[u8; 1024]` storage and
`ColorIndexingTransform` keeps only the table size. The table remains alive
until inverse transforms run in reverse order; reusing the main decoded image
buffer would lose it before that point. The change removes the color-map heap
allocation while preserving map adjustment, lookup order, decoded bytes,
errors, and sink output.

This is Rust implementation and Rust-only retained-storage evidence. The
existing WebP color-index fixture rows provide Pillow byte/error regression,
not proof of storage ownership or allocation counts; the feature-gated Rust
contracts and feature matrix are the separate non-Pillow evidence. No parity
row, fixture-manifest row, diagnostic origin, new test function,
coverage-only hook, or unit test was added.

The clean schema-`@3` benchmark measured Pillow parity at 1.290621 s wall /
3.399657 user s / 0.272636 sys s / 291,880,960-byte peak RSS and the separate
Rust-only feature-gate workload at 3.661911 s wall / 3.652647 user s /
0.378899 sys s / 270,696,448-byte peak RSS. The native release build measured
8.533493 s wall / 34.118605 user s / 0.386520 sys s / 894,255,104-byte peak
RSS and produced a 7,968,632-byte `rlib`; the WASM compile measured 3.779423 s
wall / 19.476761 user s / 1.134973 sys s / 930,447,360-byte peak RSS and
produced a 24,076,058-byte artifact. These are host/cache/toolchain
observations, not comparative or universal performance claims; allocation
counts, retained cache bytes, caller-buffer reuse, peak stack/recursion, and
WASM runtime resources remain unmeasured.

Exact-head managed Pillow parity run
`21bd0d97-6209-415f-8568-7713b3be1f62` passed 1,445/1,445 checks in 7,902 ms.
Exact-head feature-matrix run
`109d0d03-8498-4b94-9f2c-15ddbedc9ddd` passed all configured native/WASM lanes
in 33,368 ms. Nightly LLVM run
`fdb26765-f319-434c-a273-a74b6456c052` passed 85/85 tests in 67,046 ms and
ingested snapshot `46bdf0fa-d59a-40ab-9a1c-f0e85f28e02b`: 55,920/56,797
lines, 8,011/8,228 branches, 3,122/3,218 functions, and 85,968/87,926
regions. Compared with preceding accepted snapshot
`453dca6d-6dcd-44c4-819e-34978e048685`, covered/total lines rose 19/19,
branches 0/0, functions 0/0, and regions 17/17. The changed lossless decoder
projection is 1,255/1,257 lines, 130/134 branches, 53/53 functions, and
1,620/1,624 regions; the transform projection is 452/452 lines, 30/30
branches, 25/25 functions, and 883/883 regions. The known LLVM JSON
segment-normalization warning remains. These are implementation/Rust coverage
metrics, not Pillow-parity coverage.

Current acceptance record: WebP VP8L in-place palette packing

The production and Rust test/runtime slice is implemented at
`aea0c723a07e4ae3a8ac43fe76824197c5016427`, following opaque animated VP8L RGB
staging at `9c388c63c4acd3af2699fdcfa0c46339da2ddd18`. Lossless VP8L palette
mode now writes each packed pixel into the prefix of the existing mutable
source-pixel buffer instead of allocating a second image-scaled `Vec<u32>`.
`encode_frame_stream` does not reuse the source pixels after this branch, and
the left-to-right overlap is safe because each destination index is at or
before the source group being read; single-pixel groups read before replacing
their same slot. Palette lookup order, partial-group packing, checkpoint
cadence, encoded bytes, errors, and sink output remain unchanged.

This is Rust implementation and Rust-only transient-storage evidence. The
existing WebP palette fixture rows provide Pillow byte/error regression, not
proof of allocation ownership or allocation counts; the feature-gated Rust
contracts and feature matrix are the separate non-Pillow evidence. No parity
row, fixture-manifest row, diagnostic origin, new test function, coverage-only
hook, or unit test was added.

The clean schema-`@3` benchmark measured Pillow parity at 1.114840 s wall /
3.070315 user s / 0.281348 sys s / 263,028,736-byte peak RSS and the separate
Rust-only feature-gate workload at 2.795992 s wall / 2.727358 user s /
0.351815 sys s / 253,886,464-byte peak RSS. The native release build measured
7.285978 s wall / 33.597351 user s / 0.413870 sys s /
905,494,528-byte peak RSS and produced a 7,967,312-byte `rlib`; the WASM
compile measured 3.215426 s wall / 11.564499 user s / 0.877052 sys s /
841,728,000-byte peak RSS and produced a 24,076,697-byte artifact. These are
host/cache/toolchain observations, not comparative or universal performance
claims; allocation counts, retained cache bytes, caller-buffer reuse, peak
stack/recursion, and WASM runtime resources remain unmeasured.

Exact-head managed Pillow parity run
`bda2f29b-f49b-44bc-abd9-1a866c20a493` passed 1,445/1,445 checks in 8,024 ms.
Exact-head feature-matrix run
`e7e77c73-e0cf-4111-a554-c2db85dd7a57` passed all configured native/WASM lanes
in 29,627 ms. Nightly LLVM run
`12093cdf-3471-47f6-a451-201e20000124` passed 85/85 tests in 63,823 ms and
ingested snapshot `453dca6d-6dcd-44c4-819e-34978e048685`: 55,901/56,778
lines, 8,011/8,228 branches, 3,122/3,218 functions, and 85,951/87,909
regions. The changed `src/codecs/webp/native/encoder.rs` projection is
2,399/2,483 lines, 511/540 branches, 89/89 functions, and 3,467/3,747
regions; its 34 uncovered-line and 34 partial-branch-line counts are unchanged
from the preceding accepted snapshot. Compared with snapshot
`f6604b1c-1821-41d7-9706-b8b0ad077ddc`, covered/total lines rose 8/12,
branches 0/0, functions 0/0, and regions 9/14. The known LLVM JSON
segment-normalization warning remains. These are implementation/Rust coverage
metrics, not Pillow-parity coverage.

Current acceptance record: WebP opaque animated VP8L RGB staging

The production and Rust test/runtime slice is implemented at
`9c388c63c4acd3af2699fdcfa0c46339da2ddd18`, following Huffman group-vector
capacity planning at `6fe013745b2773fdb7da7efad9e7a0a28ce21d96`. In animated
WebP `VP8L` frames whose enclosing `VP8X` contract has no alpha,
`WebPDecoder::read_frame` now uses the existing `LosslessDecoder::decode_frame_rgb`
path and hands a three-byte frame to `extended::composite_frame`; alpha-bearing
animation frames retain the four-byte path. This avoids a transient RGBA frame
buffer for opaque animations while keeping the public RGB/RGBA selection,
canvas alpha, frame geometry, decoded bytes, and errors unchanged.

This is Rust implementation and Rust-only transient-storage evidence. The
existing animated RGB fixture rows provide Pillow byte/error regression, not
proof of staging ownership or allocation counts; the feature-gated Rust
contracts and feature matrix are the non-Pillow evidence. Lossless `ALPH` was
not changed: its implicit VP8L green plane may use transforms and full ARGB
state remains required for color-cache/backward-reference semantics. No parity
row, fixture-manifest row, diagnostic origin, new test function, coverage-only
hook, or unit test was added.

The clean schema-`@3` benchmark measured Pillow parity at 0.990773 s wall /
2.850616 user s / 0.283772 sys s / 267,190,272-byte peak RSS and the separate
Rust-only feature-gate workload at 1.590117 s wall / 2.248188 user s /
0.133602 sys s / 193,445,888-byte peak RSS. The native release build measured
7.070663 s wall / 35.106365 user s / 0.609290 sys s /
874,184,704-byte peak RSS and produced a 7,965,432-byte `rlib`; the WASM
compile measured 1.729399 s wall / 2.017501 user s / 0.633944 sys s /
524,451,840-byte peak RSS and produced a 24,086,496-byte artifact. These are
host/cache/toolchain observations, not comparative or universal performance
claims; allocation counts, retained cache bytes, caller-buffer reuse, peak
stack/recursion, and WASM runtime resources remain unmeasured.

Exact-head managed Pillow parity run
`acacb1bb-cab7-4618-bc5a-19236f46ed4f` passed 1,445/1,445 checks in 831 ms.
Exact-head feature-matrix run
`3ce18e19-2bd1-42a6-98fa-162b10d88164` passed all configured native/WASM lanes
in 20,873 ms. Nightly LLVM run
`3e4cbd04-12fa-4cbd-8934-c51090e82cca` passed 85/85 tests in 59,916 ms and
ingested snapshot `f6604b1c-1821-41d7-9706-b8b0ad077ddc`: 55,893/56,766
lines, 8,011/8,228 branches, 3,122/3,218 functions, and 85,942/87,895
regions. The changed `src/codecs/webp/native/decoder.rs` projection is
805/805 lines, 90/90 branches, 36/36 functions, and 1,405/1,406 regions;
it has no uncovered lines, branches, or functions. Compared with preceding
accepted snapshot `d48161a8-970b-41fe-b273-707d3b3aa4dd`, covered/total lines
rose 4/4, branches 2/2, functions 0/0, and regions 10/11. The known LLVM JSON
segment-normalization warning remains. These are implementation/Rust coverage
metrics, not Pillow-parity coverage.

Current acceptance record: WebP VP8L Huffman group-vector capacity reservation

The production and Rust test/runtime slice is implemented at
`6fe013745b2773fdb7da7efad9e7a0a28ce21d96`, following Huffman table/tree
storage coalescing at `9c51f9004198123e00e1737c75cd2c7d720b611c`. After the
metadata image establishes its bounded Huffman-group count,
`read_huffman_codes` now reserves the exact `hufftree_groups` capacity before
parsing the groups. This removes repeated geometric `Vec` growth while
preserving group order, tree construction, decoded bytes, errors, and sink
output.

This is Rust implementation and Rust-only workspace-planning evidence. Pillow
exposes only final decoded bytes and errors, so the existing WebP fixture rows
are byte/error regression evidence rather than proof of vector-capacity
planning. The feature-gated Rust contracts and feature matrix remain the
non-Pillow evidence; no parity row, fixture-manifest row, diagnostic origin,
new test function, coverage-only hook, or unit test was added. The reservation
is bounded by the already validated metadata geometry; allocation counts and
recoverable-OOM behavior remain unmeasured.

The clean schema-`@3` benchmark measured Pillow parity at 1.020091 s wall /
2.925439 user s / 0.281256 sys s / 266,469,376-byte peak RSS and the separate
Rust-only feature-gate workload at 1.771848 s wall / 2.477250 user s /
0.143801 sys s / 219,398,144-byte peak RSS. The native release build measured
7.527097 s wall / 35.771982 user s / 0.564947 sys s /
875,806,720-byte peak RSS and produced a 7,965,160-byte `rlib`; the WASM
compile measured 1.488599 s wall / 1.368381 user s / 0.640137 sys s /
508,854,272-byte peak RSS and produced a 24,086,186-byte artifact. These are
single-host/cache/toolchain observations, not comparative or universal
performance claims; retained cache bytes, caller-buffer reuse, peak stack
depth, allocation counts, and WASM runtime resources remain unmeasured.

Exact-head managed Pillow parity run
`dcdaf993-5ee5-4168-8fe0-f5178234ff4c` passed 1,445/1,445 checks in 2,158 ms.
Exact-head feature-matrix run
`93e4b693-ddbf-4a74-aab2-4a49a907f50a` passed all configured native/WASM lanes
in 27,391 ms. Nightly LLVM run
`a55f6370-a51e-488e-b492-4a7b37a30ad0` passed 85/85 tests in 65,843 ms and
ingested snapshot `d48161a8-970b-41fe-b273-707d3b3aa4dd`: 55,889/56,762
lines, 8,009/8,226 branches, 3,122/3,218 functions, and 85,932/87,884
regions. The changed `src/codecs/webp/native/lossless.rs` projection is
1,236/1,238 lines, 130/134 branches, 53/53 functions, and 1,603/1,607
regions. Compared with the preceding accepted snapshot
`915db5ec-3cdc-4a2f-beec-6e09b68d902a`, line, branch, and function totals were
unchanged; covered and total regions rose by 1/1. The aggregate shortfall is
873 lines, 217 branches, 96 functions, and 1,952 regions. The known LLVM JSON
segment-normalization warning remains. These are implementation/Rust coverage
metrics, not Pillow-parity coverage.

Current acceptance record: WebP VP8L Huffman table/tree storage coalescing

The production and Rust test/runtime slice is implemented at
`9c51f9004198123e00e1737c75cd2c7d720b611c`, following opaque VP8L RGB direct
decode at `13624949dbe405fd636ba7d4a765d3706039b173`. General VP8L Huffman
trees now keep the primary lookup table and packed secondary nodes in one
`Vec<u32>` allocation. Primary entries still address secondary nodes relative
to the table boundary, and the single-node, two-node, and inline primary-table
representations are unchanged. Huffman symbol ordering, bit consumption,
decoded bytes, errors, and sink output remain unchanged.

This is Rust implementation and Rust-only retained-workspace/ownership
evidence. Pillow exposes only final decoded bytes and errors, so the existing
WebP fixture rows are byte/error regression evidence rather than proof that one
heap allocation replaces two. The feature-gated Rust contracts and feature
matrix are the non-Pillow evidence; no parity row, fixture-manifest row,
diagnostic origin, new test function, coverage-only hook, or unit test was
added. The changed Huffman projection's remaining branch/error gaps stay
visible; no synthetic test was added merely to improve coverage.

The clean schema-`@3` benchmark measured Pillow parity at 1.091468 s wall /
2.919180 user s / 0.253003 sys s / 255,983,616-byte peak RSS and the separate
Rust-only feature-gate workload at 1.669266 s wall / 2.377772 user s /
0.156222 sys s / 197,099,520-byte peak RSS. The native release build measured
8.758619 s wall / 38.879859 user s / 0.796985 sys s /
934,068,224-byte peak RSS and produced a 7,965,016-byte `rlib`; the WASM
compile measured 5.746370 s wall / 24.149650 user s / 1.518327 sys s /
876,593,152-byte peak RSS and produced a 24,084,066-byte artifact. These are
single-host/cache/toolchain observations, not comparative or universal
performance claims; allocation counts, retained cache bytes, caller-buffer
reuse, peak stack depth, and WASM runtime resources remain unmeasured.

Exact-head managed Pillow parity run
`76c046ab-7494-46ff-95d1-4363e5af3b02` passed 1,445/1,445 checks in 926 ms.
Exact-head feature-matrix run
`9212d1eb-e4b2-406f-affa-216639b4416f` passed all configured native/WASM lanes
in 44,433 ms. Nightly LLVM run
`0b92a3eb-e455-430a-9f93-5f0af996322a` passed 85/85 tests in 65,268 ms and
ingested snapshot `915db5ec-3cdc-4a2f-beec-6e09b68d902a`: 55,889/56,762
lines, 8,009/8,226 branches, 3,122/3,218 functions, and 85,931/87,883
regions. The changed `src/codecs/webp/native/huffman.rs` projection is
351/353 lines, 56/58 branches, 13/13 functions, and 514/521 regions. Compared
with the preceding accepted snapshot `325b653e-2ab1-4380-bb92-1ea32c4b9a16`,
covered and total lines rose by 11/11, while branch and function totals were
unchanged and covered and total regions rose by 10/10. The aggregate shortfall
remains 873 lines, 217 branches, 96 functions, and 1,952 regions. The known
LLVM JSON segment-normalization warning remains. These are implementation/Rust
coverage metrics, not Pillow-parity coverage.

Current acceptance record: WebP VP8L opaque RGB direct decode

The production and Rust test/runtime slice is implemented at
`13624949dbe405fd636ba7d4a765d3706039b173`, following color-cache Huffman
code-length scratch reuse at
`b2056a98c95c2d8224149a5ce58759b095509590`. Still, non-alpha VP8L decode now
asks `LosslessDecoder::decode_frame_rgb` to write directly into the caller's
three-byte RGB buffer. The direct path is selected only when the VP8L header
reports no alpha and no transforms; alpha-bearing or transformed streams keep
the prior four-byte RGBA workspace and RGB copy. The generic decoder preserves
four-byte color-cache semantics, while the direct RGB helper represents the
known-opaque alpha as 255. Decoded bytes, errors, and sink output remain
unchanged.

This is Rust implementation and Rust-only transient-storage/ownership
evidence. Pillow exposes only final decoded bytes and errors, so the existing
lossless WebP fixture rows are byte/error regression evidence rather than proof
of the staging boundary. No parity row, fixture-manifest row, diagnostic
origin, new test function, coverage-only hook, or unit test was added. Existing
feature-gated Rust contracts and the feature matrix remain the non-Pillow
evidence. Coverage deliberately leaves unselected alpha/transform fallback and
RGB-copy alternatives visible; no synthetic tests were added merely to fill
those branches.

The clean schema-`@3` benchmark measured Pillow parity at 0.929518 s wall /
2.785749 user s / 0.177436 sys s / 257,490,944-byte peak RSS and the separate
Rust-only feature-gate workload at 1.549529 s wall / 2.231820 user s /
0.085314 sys s / 142,000,128-byte peak RSS. The native release build measured
6.383624 s wall / 31.416379 user s / 0.342882 sys s /
893,632,512-byte peak RSS and produced a 7,963,072-byte `rlib`; the WASM
compile measured 1.644607 s wall / 1.965322 user s / 0.593520 sys s /
553,484,288-byte peak RSS and produced a 24,083,457-byte artifact. These are
single-host/cache/toolchain observations, not comparative or universal
performance claims; allocation counts, retained cache bytes, caller-buffer
reuse, peak stack depth, and WASM runtime resources remain unmeasured.

Exact-head managed Pillow parity run
`e7debdc6-3fac-4970-bce4-0c53c0faa3f9` passed 1,445/1,445 checks in 676 ms.
Exact-head feature-matrix run
`cabf6e64-e632-47ee-a4c2-08fa8fc4db23` passed all configured native/WASM lanes
in 13,793 ms. Nightly LLVM run
`f0b0a6e8-9070-45e7-9468-f3744649e71c` passed 85/85 tests in 51,202 ms and
ingested snapshot `325b653e-2ab1-4380-bb92-1ea32c4b9a16`: 55,878/56,751
lines, 8,009/8,226 branches, 3,122/3,218 functions, and 85,921/87,873
regions. The changed `src/codecs/webp/native/lossless.rs` projection is
1,236/1,238 lines, 130/134 branches, 53/53 functions, and 1,602/1,606
regions; `src/codecs/webp/native/decoder.rs` is fully covered at 801/801
lines, 88/88 branches, 36/36 functions, and 1,395/1,395 regions. Compared
with the preceding accepted snapshot `50893a3c-090f-4c8d-8fff-e8dfa1caf8da`,
covered and total lines rose by 79/81, branches by 8/12, functions by 10/10,
and regions by 134/138. The aggregate shortfall is 873 lines, 217 branches,
96 functions, and 1,952 regions. The lossless projection's remaining gaps are
the alpha/transform alternative, fallback decode stream, normalized existing
path line, and RGB-copy branch alternatives; the known LLVM JSON
segment-normalization warning remains. These are implementation/Rust coverage
metrics, not Pillow-parity coverage.

Current acceptance record: WebP VP8L color-cache Huffman code-length scratch reuse

The production and Rust test/runtime slice is implemented at
`b2056a98c95c2d8224149a5ce58759b095509590`, following the bounded VP8
partition-size table at
`003df53b49ad0638412147756f39f81b2995fbae`. `read_huffman_codes` now owns one
reusable `Vec<u16>` scratch for the enlarged color-cache green alphabet and
passes it through sequential non-simple Huffman trees. The buffer is zeroed
and resized in place before each parse; `HuffmanTree::build_implicit` copies
the values into its owned tree before the next tree reuses the allocation.
The ordinary 280-symbol code-length path remains stack-backed, and decoded
tree ordering, bytes, errors, and sink output are unchanged.

This is Rust implementation and Rust-only transient-allocation evidence.
Pillow exposes only final decoded bytes and errors, so the existing WebP
fixture rows are byte/error regression evidence rather than proof of the
scratch ownership boundary. Existing feature-gated Rust contracts and the
feature matrix remain the non-Pillow evidence; no parity row,
fixture-manifest row, diagnostic origin, new test function, coverage-only
hook, or unit test was added. The clean schema-`@3` benchmark passed Pillow
parity in 0.941327 s wall / 2.793948 user s / 0.185762 sys s /
247,873,536-byte peak RSS and the separate Rust-only feature-gate suite in
1.604130 s wall / 2.285745 user s / 0.104333 sys s /
172,490,752-byte peak RSS. The native release build measured 6.438554 s wall /
32.354094 user s / 0.339675 sys s / 839,237,632-byte peak RSS and produced a
7,944,656-byte `rlib`; the WASM compile measured 1.708504 s wall /
1.210154 user s / 0.651320 sys s / 485,998,592-byte peak RSS and produced a
24,028,594-byte artifact. These are single-host/cache/toolchain observations,
not comparative or universal performance claims; allocation counts, retained
cache bytes, caller-buffer reuse, peak stack depth, and WASM runtime resources
remain unmeasured.

Exact-head managed Pillow parity run
`a1e85638-8329-4956-a535-df8a46dce70b` passed 1,445/1,445 checks in 629 ms.
Exact-head feature-matrix run
`edddada5-ec35-47aa-aa47-435df1a00861` passed all configured native/WASI lanes
in 15,138 ms. Nightly LLVM run
`fa6cc74a-9e0c-4ee9-a826-81f8c23515c1` passed 85/85 tests in 51,227 ms and
ingested snapshot `50893a3c-090f-4c8d-8fff-e8dfa1caf8da`: 55,799/56,670
lines, 8,001/8,214 branches, 3,112/3,208 functions, and 85,787/87,735
regions. The changed `src/codecs/webp/native/lossless.rs` projection is
1,153/1,153 lines, 122/122 branches, 43/43 functions, and 1,456/1,456
regions. Compared with the preceding accepted snapshot
`a7e5a4f6-3a2d-4dae-bb76-02775dffdd98`, covered and total lines rose by 6/6,
branch and function totals were unchanged, and covered and total regions rose
by 4/4; no prior uncovered implementation path was suppressed. The aggregate
shortfall is 871 lines, 213 branches, 96 functions, and 1,948 regions. These
are implementation/Rust coverage metrics, not Pillow-parity coverage; the
known LLVM JSON segment-normalization warning remains.

Current acceptance record: WebP VP8 bounded partition-size table

The production and Rust test/runtime slice is implemented at
`003df53b49ad0638412147756f39f81b2995fbae`, following final-partition
byte-staging removal at `0f5fe1841ee9bc78a25a28f146ea8b12c41111db`.
`init_partitions` now keeps its three-byte partition-size table in a fixed
21-byte stack array. The VP8 frame header derives the partition count from two
bits, so the maximum of eight partitions requires only `3 * 8 - 3` size bytes;
valid partition data, arithmetic-decoder word storage, decoded bytes, errors,
and sink output remain unchanged.

This is Rust implementation and Rust-only bounded-workspace evidence. Pillow
exposes only final decoded bytes and errors, so the existing WebP fixture rows
are byte/error regression evidence rather than proof of this stack boundary.
Existing feature-gated Rust contracts and the feature matrix remain the
non-Pillow evidence; no parity row, fixture-manifest row, diagnostic origin,
new test function, coverage-only hook, or unit test was added. The clean
schema-`@3` benchmark passed Pillow parity in 0.923416 s wall / 2.747843 user
s / 0.173908 sys s / 261,259,264-byte peak RSS and the separate Rust-only
feature-gate suite in 1.576009 s wall / 2.244138 user s / 0.098319 sys s /
192,118,784-byte peak RSS. The native release build measured 7.300954 s wall
with a 7,946,712-byte `rlib`; the WASM compile measured 2.098203 s wall with
a 24,025,694-byte artifact. These are single-host/cache/toolchain
observations, not comparative or universal performance claims; allocation
counts, retained cache bytes, caller-buffer reuse, peak stack depth, and WASM
runtime resources remain unmeasured.

Exact-head managed Pillow parity run
`0e60c846-0131-4302-b5ef-48e69fbb4b1f` passed 1,445/1,445 checks in 661 ms.
Exact-head feature-matrix run
`498a3f9f-3ed2-43e6-82ab-de65732e0bda` passed all configured native/WASI lanes
in 14,421 ms. Nightly LLVM run
`5ac1169d-79ef-4a32-869f-c3e57bb0c684` passed 85/85 tests in 51,073 ms and
ingested snapshot `a7e5a4f6-3a2d-4dae-bb76-02775dffdd98`: 55,793/56,664
lines, 8,001/8,214 branches, 3,112/3,208 functions, and 85,783/87,731
regions. The changed `src/codecs/webp/native/vp8.rs` projection is
1,615/1,615 lines, 165/166 branches, 58/58 functions, and 2,917/2,920
regions. Compared with the preceding final-partition snapshot, covered and
total lines rose by 1/1, branch and function counts were unchanged, and
covered and total regions rose by 2/2; no prior uncovered implementation path
was suppressed. The aggregate shortfall is 871 lines, 213 branches, 96
functions, and 1,948 regions. These are implementation/Rust coverage metrics,
not Pillow-parity coverage; the known LLVM JSON segment-normalization warning
remains.

Current acceptance record: WebP VP8 final-partition byte-staging removal

The production and Rust test/runtime slice is implemented at
`0f5fe1841ee9bc78a25a28f146ea8b12c41111db`, following the VP8L meta-Huffman
image materialization boundary at `414147af7ef6278345802cbe59b3c2e3c4187ddd`.
`init_final_partition` now reads the size-unknown final arithmetic partition
through a bounded 16 KiB stack buffer and appends `[u8; 4]` words directly to
the retained partition vector. It preserves short-final-word zero padding and
the logical byte count while removing the transient heap `Vec<u8>` and its
second heap allocation/copy into word storage.

This is Rust implementation and Rust-only transient-storage evidence. Pillow
exposes only final decoded bytes and errors, so the existing WebP fixture rows
are byte/error regression evidence rather than proof of this allocation
boundary. Existing feature-gated Rust contracts and the feature matrix remain
the non-Pillow evidence; no parity row, fixture-manifest row, diagnostic
origin, new test function, coverage-only hook, or unit test was added. The
clean schema-`@3` benchmark passed Pillow parity in 1.125495 s wall /
2.980989 user s / 0.190036 sys s / 263,929,856-byte peak RSS and the separate
Rust-only feature-gate suite in 1.738416 s wall / 2.498572 user s /
0.124517 sys s / 200,900,608-byte peak RSS. The native release build measured
6.507687 s wall with a 7,946,336-byte `rlib`; the WASM compile measured
2.258399 s wall with a 24,026,453-byte artifact. These are
single-host/cache/toolchain observations, not comparative or universal
performance claims; allocation counts, retained cache bytes, caller-buffer
reuse, peak stack depth, and WASM runtime resources remain unmeasured.

Exact-head managed Pillow parity run
`9248b89a-ab1e-4615-bf4b-f584e6e7c4e8` passed 1,445/1,445 checks in 725 ms.
Exact-head feature-matrix run
`a9b9e7d6-f867-46b9-b98f-b4c5ba361f04` passed all configured native/WASI lanes
in 17,583 ms. Nightly LLVM run
`ecbb1ab4-84bc-4c2b-97a8-3c846788dd86` passed 85/85 tests in 49,277 ms and
ingested snapshot `4497a38a-aad6-41fc-a5de-7df8bad30f08`: 55,792/56,663
lines, 8,001/8,214 branches, 3,112/3,208 functions, and 85,781/87,729
regions. The changed `src/codecs/webp/native/vp8.rs` projection is
1,614/1,614 lines, 165/166 branches, 58/58 functions, and 2,915/2,918
regions. Compared with the preceding meta-Huffman image snapshot, covered
and total lines rose by 16/16, covered and total branches rose by 5/6,
function counts were unchanged, and covered and total regions rose by 24/27;
no prior uncovered implementation path was suppressed. The new
`ErrorKind::Interrupted` retry branch is not selected by the existing fixture
or feature-gated inputs; it remains an explicit defensive Rust-only path
rather than a reason to add a synthetic unit test. The aggregate shortfall is
871 lines, 213 branches, 96 functions, and 1,948 regions. These are
implementation/Rust coverage metrics, not Pillow-parity coverage; the known
LLVM JSON segment-normalization warning remains.

Current acceptance record: WebP VP8L meta-Huffman image materialization reuse

The production and Rust test/runtime slice is implemented at
`414147af7ef6278345802cbe59b3c2e3c4187ddd`, following packed Huffman node
storage at `fb2ce854ac541095e04f0a98e89895cfdb7ed97c`. In
`LosslessDecoder::read_huffman_codes`, the sampled metadata image now uses one
`Vec<u16>` allocation viewed as bytes for decoding; each retained first-two-byte
group index is converted to native-endian `u16` and compacted in place before
the buffer is truncated. This removes the transient byte `Vec` and the second
`Vec<u16>` allocation/`collect` while preserving source-byte interpretation,
group-count selection, decoded bytes, errors, and sink output.

This is Rust implementation and Rust-only allocation/storage evidence. Pillow
exposes only the final byte/error result, so the existing fixture matrix is
byte/error regression evidence rather than proof of this allocation boundary.
No parity row, fixture-manifest row, diagnostic origin, new test function,
coverage-only hook, or unit test was added. The clean schema-`@3` benchmark
passed Pillow parity in 1.059718 s wall / 2.830082 user s / 0.214017 sys s /
244,563,968-byte peak RSS and the separate Rust-only feature-gate suite in
1.619626 s wall / 2.300844 user s / 0.117317 sys s /
193,953,792-byte peak RSS. The native release build measured 8.141822 s wall
with a 7,952,296-byte `rlib`; the WASM compile measured 4.855283 s wall with
a 24,022,574-byte artifact. These are single-host/cache/toolchain observations,
not comparative or universal performance claims; allocation counts, retained
cache bytes, caller-buffer reuse, peak stack depth, and WASM runtime resources
remain unmeasured.

Exact-head managed Pillow parity run
`d2e6deef-024b-4517-affc-aab08f5d6560` passed 1,445/1,445 checks in 705 ms.
Exact-head feature-matrix run
`99aad051-796d-4d05-8399-170286502a96` passed all configured native/WASI lanes
in 21,692 ms. Nightly LLVM run
`14cacf5d-2111-44a2-9aab-753b8e97c536` passed 85/85 tests in 52,763 ms and
ingested snapshot `f09fe738-cd7c-4818-aab3-fc2b4b9f5c38`: 55,776/56,647
lines, 7,996/8,208 branches, 3,112/3,208 functions, and 85,757/87,702
regions. The changed `src/codecs/webp/native/lossless.rs` projection is
1,147/1,147 lines, 122/122 branches, 43/43 functions, and 1,452/1,452
regions. Compared with the packed-node snapshot, covered and total lines rose
by 1/1, branch counts were unchanged, function counts fell by 1/1, and
covered and total regions rose by 11/11; no prior uncovered implementation
path was suppressed. The aggregate shortfall is 871 lines, 212 branches, 96
functions, and 1,945 regions. These are implementation/Rust coverage metrics,
not Pillow-parity coverage; the known LLVM JSON segment-normalization warning
remains.

Current acceptance record: WebP VP8L Huffman node packing

The production and Rust test/runtime slice is implemented at
`fb2ce854ac541095e04f0a98e89895cfdb7ed97c`, following the checked branch-offset
storage boundary at
`d74b71b5869008eaec2e3abc21efb312e58ed410`. General VP8L Huffman tree slots
now use one tagged `u32`: zero represents an empty slot, `symbol + 1`
represents a `u16` leaf, and the high-bit tag plus a validated 31-bit value
represents a child offset. This halves the node word width while preserving
tree topology, canonical symbol order, decoded bytes, errors, and sink output;
the validated VP8L alphabet bounds keep every constructed offset below the tag
bit.

This is Rust implementation and Rust-only node-layout evidence. Pillow exposes
only the final byte/error result, so the existing fixture matrix is byte/error
regression evidence rather than proof of this storage boundary. No parity row,
fixture-manifest row, diagnostic origin, new test function, coverage-only hook,
or unit test was added. The clean schema-`@3` benchmark passed Pillow parity in
1.128109 s wall / 2.852262 user s / 0.192440 sys s /
258,310,144-byte peak RSS and the separate Rust-only feature-gate suite in
3.255994 s wall / 2.808708 user s / 0.386944 sys s /
266,305,536-byte peak RSS. The native release build measured 7.368309 s wall
with a 7,954,480-byte `rlib`; the WASM compile measured 6.385956 s wall with a
24,038,221-byte artifact. These are single-host/cache/toolchain observations,
not comparative or universal performance claims; allocation counts, retained
cache bytes, caller-buffer reuse, peak stack depth, and WASM runtime resources
remain unmeasured.

Exact-head managed Pillow parity run
`b1f50d4f-06f5-497d-a89e-b4cb315eb338` passed 1,445/1,445 checks in 5,669 ms.
Exact-head feature-matrix run
`9fdf3c9e-ba03-4beb-9be2-fd628eb5777b` passed all configured native/WASI lanes
in 58,221 ms. Nightly LLVM run
`0f31d9d4-98a7-4a89-9c68-81729a0cfc70` passed 85/85 tests in 54,865 ms and
ingested snapshot `a480ff61-8156-4ffc-baa0-eeaa681a0b24`: 55,775/56,646
lines, 7,996/8,208 branches, 3,113/3,209 functions, and 85,746/87,691
regions. The changed `src/codecs/webp/native/huffman.rs` projection is
340/342 lines, 56/58 branches, 13/13 functions, and 504/511 regions.
Compared with the branch-offset snapshot, covered and total line counts rose
by 18/19, covered and total branch counts rose by 3/4, covered and total
function counts rose by 3/3, and covered and total region counts rose by 21/22.
The aggregate line, branch, and region rates moved slightly down because the
new representation adds defensive tag-boundary and representation-decoding
regions that valid Pillow-observable VP8L inputs cannot independently select;
all changed functions are covered. These are implementation/Rust coverage
metrics, not Pillow-parity coverage; the known LLVM JSON segment-normalization
warning and aggregate shortfall remain visible.

Current acceptance record: WebP VP8L Huffman branch-offset storage

The production and Rust test/runtime slice is implemented at
`d74b71b5869008eaec2e3abc21efb312e58ed410`, following the inline sixteen-entry
Huffman-table boundary at
`aa179d90d36536791e3a6a270dba39d860fcf330`. General Huffman-tree branch nodes
now store their child offsets as checked `u32` values instead of `usize`,
cutting the offset field width while preserving the tree topology, canonical
symbol order, decoded bytes, errors, and sink output. The VP8L alphabet bounds
keep the constructed arena below the 32-bit range; conversion failures remain
defensive Rust-only errors rather than Pillow-observable outcomes.

This is Rust implementation and Rust-only node-layout evidence. Pillow
exposes only the final byte/error result, so the existing fixture matrix is
byte/error regression evidence rather than proof of this storage boundary. No
parity row, fixture-manifest row, diagnostic origin, new test function,
coverage-only hook, or unit test was added. The clean schema-`@3` benchmark
passed Pillow parity in 1.316901 s wall / 3.157887 user s / 0.198710 sys s /
273,465,344-byte peak RSS and the separate Rust-only feature-gate suite in
2.110834 s wall / 2.774360 user s / 0.120745 sys s / 253,820,928-byte peak RSS.
The native release build measured 8.208153 s wall with a 7,947,944-byte
`rlib`; the WASM compile measured 7.078736 s wall with a 24,034,724-byte
artifact. These are single-host/cache/toolchain observations, not comparative
or universal performance claims; allocation counts, retained cache bytes,
caller-buffer reuse, peak stack depth, and WASM runtime resources remain
unmeasured.

Exact-head managed Pillow parity run
`41bf32b0-79c8-483e-bd13-1b263a070109` passed 1,445/1,445 checks in 598 ms.
Exact-head feature-matrix run
`580101e5-5e8e-4f8b-a1f4-f34839d979b1` passed all configured native/WASI lanes
in 24,278 ms. Nightly LLVM run
`72cacd35-7d51-4d15-a181-54996a34745f` passed 85/85 tests in 49,924 ms and
ingested snapshot `d840b5cc-8134-4f96-87f1-bdf477b50cc8`: 55,757/56,627
lines, 7,993/8,204 branches, 3,110/3,206 functions, and 85,725/87,669
regions. The changed `src/codecs/webp/native/huffman.rs` projection is
322/323 lines, 53/54 branches, 10/10 functions, and 483/489 regions.
Compared with the inline-sixteen-entry snapshot, covered and total line counts
rose by 3, branches and functions were unchanged, and covered regions rose by
10 while total regions rose by 13. The line rate rose slightly; the branch
rate was unchanged and the region rate moved slightly down because three
defensive conversion regions are unreachable under the validated VP8L
alphabet bounds. These are implementation/Rust coverage metrics, not
Pillow-parity coverage; the known LLVM JSON segment-normalization warning and
aggregate shortfall remain visible.

Current acceptance record: WebP VP8L inline sixteen-entry Huffman table

The production and Rust test/runtime slice is implemented at
`aa179d90d36536791e3a6a270dba39d860fcf330`, following the inline eight-entry
Huffman-table boundary at
`5dcf6e2953b222c1f4a2d4b19e6afa14d7bbba45`. `HuffmanTree::build_implicit`
recognizes a valid complete canonical form whose primary table has exactly
sixteen entries (maximum code length four) and stores those entries in
`InlineTable16`; canonical completeness proves that this bounded form has no
secondary nodes, while larger trees retain the general table/tree. Symbol
ordering, bit consumption, decoded bytes, errors, and sink output remain
unchanged.

This is Rust implementation and Rust-only representation evidence. Pillow
exposes only the final byte/error result, not the internal table selection, so
the existing fixture matrix is byte/error regression evidence rather than
proof of this storage boundary. No parity row, fixture-manifest row,
diagnostic origin, new test function, coverage-only hook, or unit test was
added. The clean schema-`@3` benchmark passed Pillow parity in 0.951677 s wall /
2.822434 user s / 0.214295 sys s / 250,314,752-byte peak RSS and the separate
Rust-only feature-gate suite in 1.559766 s wall / 2.253902 user s / 0.102208
sys s / 164,184,064-byte peak RSS. The native release build measured 6.417643 s
wall with a 7,945,120-byte `rlib`; the WASM compile measured 3.287724 s wall
with a 24,035,701-byte artifact. These are single-host/cache/toolchain
observations, not comparative or universal performance claims; allocation
counts, retained cache bytes, caller-buffer reuse, peak stack depth, and WASM
runtime resources remain unmeasured.

Exact-head managed Pillow parity run
`de1aae27-89a3-4ba0-8bed-f0c40a991e10` passed 1,445/1,445 checks in 631 ms.
Exact-head feature-matrix run
`f11f2ead-97b8-4716-8c3d-291d73194745` passed all configured native/WASI lanes
in 21,788 ms. Nightly LLVM run
`99259505-135c-4e4f-8c35-017f155d3176` passed 85/85 tests in 49,460 ms and
ingested snapshot `6a9430d0-ee19-4608-ab2b-7cca7da6d011`: 55,754/56,624
lines, 7,993/8,204 branches, 3,110/3,206 functions, and 85,715/87,656
regions. The changed `src/codecs/webp/native/huffman.rs` projection is
319/320 lines, 53/54 branches, 10/10 functions, and 473/476 regions.
Compared with the inline-eight-entry snapshot, covered and total line counts
rose by 25, covered and total branch counts rose by 6, functions were
unchanged, and covered and total regions rose by 38. Line, branch, and region
rates rose. Existing fixture rows exercise the new `InlineTable16` path,
including 213 bounded-table construction/return hits and 2,165 nonzero-symbol
entries. The remaining projection gaps are the existing or shifted general
`read_symbol`/`peek_symbol` line-normalization gaps. These are
implementation/Rust coverage metrics, not Pillow-parity coverage; the known
LLVM JSON segment-normalization warning and aggregate shortfall remain visible.

Current acceptance record: WebP VP8L inline eight-entry Huffman table

The production and Rust test/runtime slice is implemented at
`5dcf6e2953b222c1f4a2d4b19e6afa14d7bbba45`, following the inline four-entry
Huffman-table boundary at
`2bc95ea75758b944c3d40cf16b806561d18a401e`. `HuffmanTree::build_implicit`
recognizes a valid complete canonical form whose primary table has exactly
eight entries (maximum code length three) and stores those entries in
`InlineTable8`; canonical completeness proves that this bounded form has no
secondary nodes, while larger trees retain the general table/tree. Symbol
ordering, bit consumption, decoded bytes, errors, and sink output remain
unchanged.

This is Rust implementation and Rust-only representation evidence. Pillow
exposes only the final byte/error result, not the internal table selection, so
the existing fixture matrix is byte/error regression evidence rather than
proof of this storage boundary. No parity row, fixture-manifest row,
diagnostic origin, new test function, coverage-only hook, or unit test was
added. The clean schema-`@3` benchmark passed Pillow parity in 1.032231 s wall /
2.786608 user s / 0.193083 sys s / 245,219,328-byte peak RSS and the separate
Rust-only feature-gate suite in 1.579105 s wall / 2.248473 user s / 0.106248
sys s / 163,790,848-byte peak RSS. The native release build measured 7.947735 s
wall with a 7,944,248-byte `rlib`; the WASM compile measured 4.808797 s wall
with a 24,033,702-byte artifact. These are single-host/cache/toolchain
observations, not comparative or universal performance claims; allocation
counts, retained cache bytes, caller-buffer reuse, peak stack depth, and WASM
runtime resources remain unmeasured.

Exact-head managed Pillow parity run
`e3438c7c-0123-4f14-9554-f9c3cfac2813` passed 1,445/1,445 checks in 1,206 ms.
Exact-head feature-matrix run
`c9723065-4d75-43d7-b1d3-33bbb9be2228` passed all configured native/WASI lanes
in 24,215 ms. Nightly LLVM run
`db1639ad-966f-4e4f-8a04-be074c60ab6b` passed 85/85 tests in 53,446 ms and
ingested snapshot `d314cc7f-c1b0-42d5-b136-c506770604fa`: 55,729/56,599
lines, 7,987/8,198 branches, 3,110/3,206 functions, and 85,677/87,618
regions. The changed `src/codecs/webp/native/huffman.rs` projection is
294/295 lines, 47/48 branches, 10/10 functions, and 435/438 regions.
Compared with the inline-four-entry snapshot, covered and total line counts
rose by 25, covered and total branch counts rose by 6, functions were
unchanged, and covered regions rose by 37 while total regions rose by 38.
Line and branch rates rose; the region rate moved slightly down because one
newly reported region remains uncovered after LLVM segment normalization.
Existing fixture rows exercise the new `InlineTable8` path, including 217
bounded-table construction/return hits and 1,224 nonzero-symbol fill entries.
The remaining projection gaps are the existing or shifted general
`read_symbol`/`peek_symbol` line-normalization gaps. These are
implementation/Rust coverage metrics, not Pillow-parity coverage; the known
LLVM JSON segment-normalization warning and aggregate shortfall remain visible.

Current acceptance record: WebP VP8L inline four-entry Huffman table

The production and Rust test/runtime slice is implemented at
`2bc95ea75758b944c3d40cf16b806561d18a401e`, following the ordinary Huffman
code-length workspace at
`dec274536d13ff70e9e985b6ce2ba2f7b175fa80`. `HuffmanTree::build_implicit`
recognizes a valid complete canonical form whose primary table has exactly
four entries (maximum code length two) and stores those entries in
`InlineTable4`; canonical completeness proves that this bounded form has no
secondary nodes, while larger trees retain the general table/tree. Symbol
ordering, bit consumption, decoded bytes, errors, and sink output remain
unchanged.

This is Rust implementation and Rust-only representation evidence. Pillow
exposes only the final byte/error result, not the internal table selection, so
the existing fixture matrix is byte/error regression evidence rather than
proof of this storage boundary. No parity row, fixture-manifest row,
diagnostic origin, new test function, coverage-only hook, or unit test was
added. The clean schema-`@3` benchmark passed Pillow parity in 0.979389 s wall /
2.873243 user s / 0.243163 sys s / 250,904,576-byte peak RSS and the separate
Rust-only feature-gate suite in 1.615377 s wall / 2.329001 user s / 0.123208
sys s / 221,085,696-byte peak RSS. The native release build measured 6.833085 s
wall with a 7,943,352-byte `rlib`; the WASM compile measured 3.554548 s wall
with a 24,032,181-byte artifact. These are single-host/cache/toolchain
observations, not comparative or universal performance claims; allocation
counts, retained cache bytes, caller-buffer reuse, peak stack depth, and WASM
runtime resources remain unmeasured.

Exact-head managed Pillow parity run
`8f4b27e7-b920-4ab8-a09a-1223dbbc04ce` passed 1,445/1,445 checks in 1,013 ms.
Exact-head feature-matrix run
`810c2b06-8cca-40a0-8996-85cb4bc38e0f` passed all configured native/WASI lanes
in 36,302 ms. Nightly LLVM run
`4ec9620a-460e-4c2c-8fdd-d0cdfad9803e` passed 85/85 tests in 68,136 ms and
ingested snapshot `ef05b4d2-93d8-49d8-8c47-fda53ce7e566`: 55,704/56,574
lines, 7,981/8,192 branches, 3,110/3,206 functions, and 85,640/87,580
regions. The changed `src/codecs/webp/native/huffman.rs` projection is
269/270 lines, 41/42 branches, 10/10 functions, and 398/400 regions.
Compared with the ordinary-workspace snapshot, covered and total line counts
rose by 25, covered and total branch counts rose by 6, functions were
unchanged, and covered regions rose by 37 while total regions rose by 38.
Line and branch rates rose; the region rate moved slightly down because one
newly reported region remains uncovered after LLVM segment normalization. The
new `InlineTable4` path is covered; the remaining projection gaps are the
existing or shifted `peek_symbol` line-normalization gaps. These are
implementation/Rust coverage metrics, not Pillow-parity coverage; the known
LLVM JSON segment-normalization warning and aggregate shortfall remain visible.

Current acceptance record: WebP VP8L ordinary Huffman code-length workspace

The production and Rust test/runtime slice is implemented at
`dec274536d13ff70e9e985b6ce2ba2f7b175fa80`, following the fixed
code-length-alphabet workspace at
`0d5371c223e42fedf15ae28d06f6d52083ab47c1`. Ordinary decoded code-length
buffers now use a fixed 280-entry stack array; only the green alphabet enlarged
by an optional color cache remains heap-backed because it can reach 2,328
symbols. The caller-owned buffer is borrowed only while
`HuffmanTree::build_implicit` copies the values into the owned tree, preserving
code ordering, decoded bytes, errors, and sink output.

This is Rust implementation and Rust-only fixed-workspace evidence. Pillow
exposes only the final byte/error result, not allocation ownership or the
selected buffer, so the existing fixture matrix is byte/error regression
evidence rather than proof of this storage boundary. No parity row,
fixture-manifest row, diagnostic origin, new test function, coverage-only hook,
or unit test was added. The clean schema-`@3` benchmark passed the
Pillow-parity workload in 0.945526 s wall / 2.802712 user s / 0.204796 sys s /
258,703,360-byte peak RSS and the separate Rust-only feature-gate workload in
1.552767 s wall / 2.238526 user s / 0.098918 sys s / 172,064,768-byte peak RSS.
The native release build measured 6.494259 s wall / 31.710438 user s /
0.340062 sys s / 865,681,408-byte peak RSS and produced a 7,939,544-byte
`rlib`; the `wasm32-unknown-unknown` determinism compile measured 2.650204 s
wall / 10.953236 user s / 0.800769 sys s / 798,294,016-byte peak RSS and
produced a 24,030,339-byte artifact. These are single-host/cache/toolchain
observations, not comparative or universal performance claims; allocation
counts, retained cache bytes, caller-buffer reuse, peak stack depth, and WASM
runtime resources remain unmeasured.

Exact-head managed Pillow parity run
`8d094ec7-7150-4803-a5e7-511045e7776a` passed 1,445/1,445 checks in 621 ms.
Exact-head feature-matrix run
`2610304f-6767-44cd-9678-84f5063b7339` passed all configured native/WASI lanes
in 24,064 ms. Nightly LLVM run
`9aa248ae-bed5-4547-9951-8a8129738a31` passed 85/85 tests in 57,208 ms and
ingested snapshot `e1299845-0513-4fb9-9fa8-52fa84207f38`: 55,679/56,549
lines, 7,975/8,186 branches, 3,110/3,206 functions, and 85,603/87,542
regions. The changed `src/codecs/webp/native/lossless.rs` projection is fully
covered at 1,146/1,146 lines, 122/122 branches, 44/44 functions, and
1,441/1,441 regions; the Huffman projection remains 244/245 lines, 35/36
branches, 10/10 functions, and 361/362 regions. Compared with the c41
snapshot, source and covered line totals rose by 13, covered and total
branches rose by 2, functions were unchanged, and covered/total regions rose
by 13; aggregate coverage increased and no implementation path was
suppressed. These are implementation/Rust coverage metrics, not
Pillow-parity coverage; the known LLVM JSON segment-normalization warning and
aggregate shortfall remain visible.

Current acceptance record: WebP VP8L canonical two-symbol Huffman storage

The production and Rust test/runtime slice is implemented at
`c41a42081876636e073160a4f49b22ef6c4ac9af`, following the transform-order
storage boundary at
`037a2a8b883c20e9250ae302f61ff565fdc38d3f`. `HuffmanTree::build_implicit`
now maps a valid complete canonical code-length form with exactly two one-bit
symbols to the inline `TwoNode` representation instead of allocating the
general table/tree. Canonical symbol order, bit consumption, decoded values,
errors, and sink output remain unchanged.

This is Rust implementation and Rust-only representation evidence. Pillow
exposes only the final byte/error result; it cannot observe which internal
Huffman representation a bitstream selected, so the existing fixture matrix
is byte/error regression evidence rather than proof of this storage boundary.
No parity row, fixture-manifest row, diagnostic origin, new test function,
coverage-only hook, or unit test was added. The clean schema-`@3` benchmark
passed the Pillow-parity workload in 1.647349 s wall / 3.448850 user s /
0.276813 sys s / 288,817,152-byte peak RSS and the separate Rust-only
feature-gate workload in 2.048556 s wall / 2.678063 user s / 0.155604 sys s /
242,810,880-byte peak RSS. The native release build measured 7.121365 s wall /
33.045848 user s / 0.385616 sys s / 862,846,976-byte peak RSS and produced a
7,937,960-byte `rlib`; the `wasm32-unknown-unknown` determinism compile
measured 1.640074 s wall / 1.063399 user s / 0.653404 sys s /
471,465,984-byte peak RSS and produced a 24,029,119-byte artifact. These are
single-host/cache/toolchain observations, not comparative or universal
performance claims; allocation counts, retained encoded/decoded cache bytes,
caller-buffer reuse, peak stack depth, and WASM runtime resources remain
unmeasured.

Exact-head managed Pillow parity run
`8f9a08e8-d2db-4680-8a33-7caab5af416c` passed 1,445/1,445 checks in 930 ms.
Exact-head feature-matrix run
`4d89465f-3ff9-4839-9c6b-7613e46fdb7f` passed all configured native/WASI lanes
in 19,867 ms. Nightly LLVM run
`759cdd2e-c067-43c2-a84d-16c84780e7e2` passed 85/85 tests in 61,223 ms and
ingested snapshot `e62ff6e2-ea0a-4326-9843-8072e06cb92c`: 55,666/56,536
lines, 7,973/8,184 branches, 3,110/3,206 functions, and 85,590/87,529
regions. The changed `src/codecs/webp/native/huffman.rs` projection is
244/245 lines, 35/36 branches, 10/10 functions, and 361/362 regions;
`src/codecs/webp/native/lossless.rs` remains fully covered at 1,133/1,133
lines, 120/120 branches, 44/44 functions, and 1,428/1,428 regions. Compared
with the 037 snapshot, source and covered line totals rose by 9, covered and
total branches rose by 6, functions were unchanged, and covered/total regions
rose by 16. The line-only comparison reports the pre-existing uncovered
`peek_symbol` branch at its shifted current line 330 (previously line 317),
not a newly suppressed or uncovered implementation path; aggregate coverage
increased and the known LLVM segment-normalization warning remains visible.

Current acceptance record: WebP VP8L transform-order storage

The production and Rust test/runtime slice is implemented at
`037a2a8b883c20e9250ae302f61ff565fdc38d3f`, following the Huffman code-length
workspace boundary at
`0d5371c223e42fedf15ae28d06f6d52083ab47c1`. `LosslessDecoder` now stores the
at-most-four transform IDs in `[u8; 4]` plus a length instead of a heap
`Vec`; duplicate-transform rejection and reverse application order remain
unchanged because the VP8L bitstream permits at most one instance of each of
the four transform types. Encoded bytes, errors, and sink output remain
unchanged.

This is Rust implementation and Rust-only fixed-workspace evidence. Pillow
exposes only the final byte/error result, not transform-order storage,
allocation ownership, or OOM behavior. The existing fixture matrix is
therefore byte/error regression evidence, not proof of the internal workspace
contract; no parity row, fixture-manifest row, diagnostic origin, new test
function, coverage-only hook, or unit test was added. The clean schema-`@3`
benchmark passed the Pillow-parity workload in 6.676605 s wall / 19.346702
user s / 1.339534 sys s / 897,744,896-byte peak RSS and the separate Rust-only
feature-gate workload in 3.624539 s wall / 3.501017 user s / 0.451302 sys s /
253,001,728-byte peak RSS. The native release build measured 19.506975 s wall /
40.918605 user s / 0.595401 sys s / 870,105,088-byte peak RSS and produced a
7,938,712-byte `rlib`; the `wasm32-unknown-unknown` determinism compile
measured 4.195934 s wall / 20.235524 user s / 1.139258 sys s /
911,425,536-byte peak RSS and produced a 24,028,585-byte artifact. These are
single-host/cache/toolchain observations, not comparative or universal
performance claims; allocation counts, retained encoded/decoded cache bytes,
caller-buffer reuse, peak stack depth, and WASM runtime resources remain
unmeasured.

Exact-head managed Pillow parity run
`ac3a6dac-e804-4486-a047-371d7413c7e9` passed 1,445/1,445 checks in 4,505 ms.
Exact-head feature-matrix run
`3b225e6e-3fc7-4289-8b3c-c0adb95b48ac` passed all configured native/WASI lanes
in 34,305 ms. Nightly LLVM run
`06311cb9-b697-46ad-9d9a-ad741cc54625` passed 85/85 tests in 67,860 ms and
ingested snapshot `72f62562-63ab-428c-b054-07ffa38c30e1`: 55,657/56,527
lines, 7,967/8,178 branches, 3,110/3,206 functions, and 85,574/87,513
regions. The changed `src/codecs/webp/native/huffman.rs` projection is
235/236 lines, 29/30 branches, 10/10 functions, and 346/346 regions;
`src/codecs/webp/native/lossless.rs` is fully covered at 1,133/1,133 lines,
120/120 branches, 44/44 functions, and 1,428/1,428 regions. Compared with the
0d snapshot, source and covered line totals rose by 13 and covered/total
regions rose by 2; branch and function totals were unchanged, and no prior
coverage was suppressed. These are implementation/Rust coverage metrics, not
Pillow-parity coverage; the known LLVM JSON segment-normalization warning and
aggregate shortfall remain visible.

Current acceptance record: WebP VP8L Huffman code-length workspace

The production and Rust test/runtime slice is implemented at
`0d5371c223e42fedf15ae28d06f6d52083ab47c1`, following the separate
simple two-symbol Huffman-node storage boundary at
`023172675e23bdea8a59b49cf9b07c80904fb4b8`. The fixed 19-entry code-length
alphabet now remains on the stack and is borrowed through the Huffman builder;
the dynamic decoded code-length buffer remains heap-backed for the possible
2,328-symbol color-cache alphabet. Code ordering, bytes, errors, and sink
output remain unchanged.

This is Rust implementation and Rust-only fixed-workspace evidence. Pillow
exposes only the final byte/error result, not the code-length storage,
allocation ownership, or OOM behavior. The existing fixture matrix is
therefore byte/error regression evidence, not proof of the internal workspace
contract; no parity row, fixture-manifest row, diagnostic origin, new test
function, coverage-only hook, or unit test was added. The clean schema-`@3`
benchmark passed the Pillow-parity workload in 0.952934 s wall / 2.794763
user s / 0.202921 sys s / 252,887,040-byte peak RSS and the separate Rust-only
feature-gate workload in 2.258811 s wall / 2.640830 user s / 0.137484 sys s /
212,762,624-byte peak RSS. The native release build measured 12.807931 s wall /
38.694404 user s / 0.601411 sys s / 866,091,008-byte peak RSS and produced a
7,937,704-byte `rlib`; the `wasm32-unknown-unknown` determinism compile
measured 2.450167 s wall / 1.637902 user s / 0.683698 sys s /
511,524,864-byte peak RSS and produced a 24,029,289-byte artifact. These are
single-host/cache/toolchain observations, not comparative or universal
performance claims; allocation counts, retained cache bytes, caller-buffer
reuse, peak stack depth, and WASM runtime resources remain unmeasured.

Exact-head managed Pillow parity run
`f999017c-2936-4018-9a87-739de7e31216` passed 1,445/1,445 checks in 964 ms.
Exact-head feature-matrix run
`a679f61d-72da-4a83-94cf-d44e95d0c85d` passed all configured native/WASI lanes
in 21,984 ms. Nightly LLVM run
`d2d141ea-c0b1-45ab-bfc2-0faf46362842` passed 85/85 tests in 60,450 ms and
ingested snapshot `5d0c6041-fb61-4fd1-bc43-3e5792bdf29d`: 55,644/56,514
lines, 7,967/8,178 branches, 3,110/3,206 functions, and 85,572/87,511
regions. The changed `src/codecs/webp/native/huffman.rs` projection is
235/236 lines, 29/30 branches, 10/10 functions, and 345/346 regions;
`src/codecs/webp/native/lossless.rs` is fully covered at 1,120/1,120 lines,
120/120 branches, 44/44 functions, and 1,426/1,426 regions. Compared with the
023 snapshot, line, branch, and function totals are unchanged and only one
covered/total region was added; no prior coverage was suppressed. These are
implementation/Rust coverage metrics, not Pillow-parity coverage; the known
LLVM JSON segment-normalization warning and aggregate shortfall remain visible.

Current acceptance record: WebP VP8L simple Huffman-node storage

The production and Rust test/runtime slice is implemented at
`023172675e23bdea8a59b49cf9b07c80904fb4b8`, following the separate
color-index row-scratch removal at
`d64e773338e1f7316f04653eddc55e306641d465`. Simple two-symbol VP8L Huffman
codes now use an inline `TwoNode` representation instead of allocating the
fixed three-node tree and two-entry lookup table used by the general tree.
The decoder preserves low-bit selection, one-bit consumption, `peek_symbol`
results, secondary-symbol acceptance, and short-read or malformed-stream
errors.

This is Rust implementation and Rust-only allocation/layout evidence. Pillow
exposes only the final byte/error result, not the Huffman representation,
allocation ownership, or OOM behavior. The existing fixture matrix is
therefore byte/error regression evidence, not proof of the internal storage
contract; no parity row, fixture-manifest row, diagnostic origin, new test
function, coverage-only hook, or unit test was added. The clean schema-`@3`
benchmark passed the Pillow-parity workload in 1.565999 s wall / 3.784783
user s / 0.288242 sys s / 284,524,544-byte peak RSS and the separate Rust-only
feature-gate workload in 2.756269 s wall / 3.191832 user s / 0.183618 sys s /
174,243,840-byte peak RSS. The native release build measured 18.622114 s wall /
39.881323 user s / 0.562186 sys s / 853,770,240-byte peak RSS and produced a
7,937,480-byte `rlib`; the `wasm32-unknown-unknown` determinism compile
measured 7.452798 s wall / 21.093572 user s / 1.173980 sys s /
896,040,960-byte peak RSS and produced a 24,035,252-byte artifact. These are
single-host/cache/toolchain observations, not comparative or universal
performance claims; allocation counts, retained cache bytes, caller-buffer
reuse, peak stack depth, and WASM runtime resources remain unmeasured.

Exact-head managed Pillow parity run
`771e1182-ee56-48a3-ac4a-495fc135b2c8` passed 1,445/1,445 checks in 947 ms.
Exact-head feature-matrix run
`350101d6-fad0-444f-bfa4-16007aac7039` passed all configured native/WASI lanes
in 37,015 ms. Nightly LLVM run
`f9278f21-a329-4d8b-a07f-1bc51f402b40` passed 85/85 tests in 70,705 ms and
ingested snapshot `5e675882-5527-4353-8462-e20339e29480`: 55,644/56,514
lines, 7,967/8,178 branches, 3,110/3,206 functions, and 85,571/87,510
regions. The changed `src/codecs/webp/native/huffman.rs` projection is
235/236 lines, 29/30 branches, 10/10 functions, and 343/344 regions. Compared
with the d64 snapshot, covered totals changed by +1 line, +3 branches, and +6
regions while functions stayed unchanged because the new inline storage and
dispatch paths were added; no prior uncovered path was suppressed. These are
implementation/Rust coverage metrics, not Pillow-parity coverage; the known
LLVM JSON segment-normalization warning and aggregate shortfall remain visible.

Current acceptance record: WebP VP8L color-index row scratch removal

The production and Rust test/runtime slice is implemented at
`d64e773338e1f7316f04653eddc55e306641d465`, following the separate
color-indexing lookup workspace at
`d68596f0d8b4a3d34a73bdcfc52eeeaac9a775f3`. The small-palette VP8L
color-indexing transform now expands each row from right to left, reading one
packed green-channel index before its output can overwrite higher offsets.
This removes the dimension-dependent `packed_indices_for_row` heap vector while
preserving the in-place input/output overlap, table lookup order, decoded
bytes, errors, and sink output.

This is Rust implementation and Rust-only workspace-allocation evidence.
Pillow exposes only the final byte/error result, not the row traversal,
temporary index storage, caller buffers, or OOM behavior. The existing fixture
matrix is therefore byte/error regression evidence, not proof of the internal
workspace contract; no parity row, fixture-manifest row, diagnostic origin,
new test function, coverage-only hook, or unit test was added. The clean
schema-`@3` benchmark at this revision passed the Pillow-parity workload in
0.964975 s wall / 2.800562 user s / 0.227752 sys s / 268,615,680-byte peak RSS
and the separate Rust-only feature-gate workload in 1.618895 s wall /
2.527168 user s / 0.125569 sys s / 201,310,208-byte peak RSS. The native
release build measured 6.758479 s wall / 32.583818 user s / 0.416777 sys s /
871,940,096-byte peak RSS and produced a 7,934,936-byte `rlib`; the
`wasm32-unknown-unknown` determinism compile measured 1.563952 s wall /
1.189490 user s / 0.678877 sys s / 461,799,424-byte peak RSS and produced a
24,038,051-byte artifact. These are single-host/cache/toolchain observations,
not comparative or universal performance claims; allocation counts, retained
cache bytes, caller-buffer reuse, stack depth, and WASM runtime resources
remain unmeasured.

Exact-head managed Pillow parity run
`07566aa7-b67c-4e05-b299-16754d67e65b` passed 1,445/1,445 checks in 606 ms.
Exact-head feature-matrix run
`f6c7b993-9d58-45bf-a881-0298515fb9ea` passed all configured native/WASI lanes
in 17,782 ms; its retained log includes
`capability tables OK: every native and wasm32-wasip1 lane agrees` and has no
`lock-wait` match. Nightly LLVM run
`4dc03bb2-781a-4f5e-9da7-c29c40025791` passed 85/85 tests in 54,914 ms and
ingested snapshot `8ffd9361-0f7d-4524-abec-633bf4fedadf`: 55,643/56,512
lines, 7,964/8,174 branches, 3,110/3,206 functions, and 85,565/87,503
regions. The changed lossless-transform projection is 452/452 lines, 30/30
branches, 25/25 functions, and 883/883 regions; the right-to-left row path
was covered. These are implementation/Rust coverage metrics, not
Pillow-parity coverage; the known LLVM JSON segment-normalization warning and
aggregate shortfall remain visible.

Current acceptance record: WebP VP8L color-indexing lookup workspace

The production and Rust test/runtime slice is implemented at
`d68596f0d8b4a3d34a73bdcfc52eeeaac9a775f3`, following the separate
entropy-analysis mode workspace at
`efc5491e1806741e796b8e74118c2db66bee382e`. VP8L color-indexing now stores
the bounded 256-entry RGBA source table and each mutually exclusive
256-entry packed-byte expansion table in function-local stack arrays instead
of temporary heap vectors. The largest specialized expansion is 8,192 bytes;
the dimension-dependent per-row packed-index buffer remains a heap workspace.
Color-table padding, packed-index order, decoded bytes, errors, and sink output
remain unchanged.

This is Rust implementation and Rust-only workspace-allocation evidence.
Pillow exposes the final byte/error result, not lookup-table storage, stack
footprint, the remaining row buffer, caller buffers, or OOM behavior. The
existing fixture matrix is therefore byte/error regression evidence, not proof
of the internal workspace contract; no parity row, fixture-manifest row,
diagnostic origin, new test function, coverage-only hook, or unit test was
added. The clean schema-`@3` benchmark at this revision passed the
Pillow-parity workload in 0.968788 s wall / 2.807162 user s / 0.242987 sys s /
256,425,984-byte peak RSS and the separate Rust-only feature-gate workload in
1.595171 s wall / 2.266171 user s / 0.116244 sys s / 220,102,656-byte peak
RSS. The native release build measured 7.387181 s wall / 34.677971 user s /
0.452437 sys s / 880,115,712-byte peak RSS and produced a 7,936,704-byte
`rlib`; the `wasm32-unknown-unknown` determinism compile measured 3.882671 s
wall / 21.532846 user s / 1.184014 sys s / 894,304,256-byte peak RSS and
produced a 24,050,665-byte artifact. These are single-host/cache/toolchain
observations, not comparative or universal performance claims; allocation
counts, retained cache bytes, caller-buffer reuse, stack depth, and WASM
runtime resources remain unmeasured.

Exact-head managed Pillow parity run
`183071da-d525-4f61-bf46-e5e27fb62378` passed 1,445/1,445 checks in 733 ms.
Exact-head feature-matrix run
`0060304b-12f3-42a5-9609-7fd04bfbcd90` passed all configured native/WASI lanes
in 33,108 ms; its retained log includes
`capability tables OK: every native and wasm32-wasip1 lane agrees` and has no
`lock-wait` match. Nightly LLVM run
`c7e64ad4-ff06-47fe-b6aa-bea2a1d5ac08` passed 85/85 tests in 65,381 ms and
ingested snapshot `86396b11-7b70-4a25-8363-70be4073b522`: 55,653/56,522
lines, 7,964/8,174 branches, 3,110/3,206 functions, and 85,574/87,512
regions. The changed lossless-transform projection is 462/462 lines, 30/30
branches, 25/25 functions, and 892/892 regions; selected large- and
small-palette lookup paths were covered. These are implementation/Rust
coverage metrics, not Pillow-parity coverage; the known LLVM JSON
segment-normalization warning and aggregate shortfall remain visible.

Current acceptance record: WebP VP8L entropy-analysis mode workspace

The production and Rust test/runtime slice is implemented at
`efc5491e1806741e796b8e74118c2db66bee382e`, following the separate
entropy-analysis histogram workspace at
`34006c8768b69866dde9dad37d2cd0f3e8623f67`. VP8L `analyze_entropy` now keeps
its four mandatory and one optional palette entropy-mode candidates in a
function-local fixed array, using a four- or five-entry view instead of a
temporary heap vector. Candidate ordering, palette selection, mode selection,
encoded bytes, cancellation/error behavior, and sink output remain unchanged.

This is Rust implementation and Rust-only workspace-allocation evidence.
Pillow exposes only the existing byte/error fixture matrix, not candidate
storage, stack footprint, caller buffers, or OOM behavior. The fixture matrix
is therefore regression evidence, not proof of the internal workspace
contract; no parity row, fixture-manifest row, diagnostic origin, new test
function, coverage-only hook, or unit test was added. The clean schema-`@3`
benchmark at this revision passed the Pillow-parity workload in 1.150759 s
wall / 3.199841 user s / 0.244846 sys s / 298,680,320-byte peak RSS and the
separate Rust-only feature-gate workload in 1.633563 s wall / 2.309190 user s
/ 0.117427 sys s / 164,167,680-byte peak RSS. The native release build
measured 7.087748 s wall / 32.935389 user s / 0.356649 sys s /
859,308,032-byte peak RSS and produced a 7,960,592-byte `rlib`; the
`wasm32-unknown-unknown` determinism compile measured 4.215155 s wall /
22.041927 user s / 1.139986 sys s / 898,236,416-byte peak RSS and produced a
24,148,829-byte artifact. These are single-host/cache/toolchain observations,
not comparative or universal performance claims; allocation counts, stack
depth, and WASM runtime resources remain unmeasured.

Exact-head managed Pillow parity run
`a8c83ffb-379a-4556-8e69-2c02c33a9962` passed 1,445/1,445 checks in 809 ms.
Exact-head feature-matrix run
`7be7cb89-2795-42ef-a74a-086636379971` passed all configured native/WASI lanes
in 40,436 ms; its retained log includes
`capability tables OK: every native and wasm32-wasip1 lane agrees` and has no
`lock-wait` match. Nightly LLVM run
`5e6bdbee-7bec-4013-ab2f-72a3233f8b9d` passed 85/85 tests in 62,810 ms and
ingested snapshot `8421d772-0f33-4398-90fa-938da1b595fa`: 55,659/56,528
lines, 7,964/8,174 branches, 3,112/3,208 functions, and 85,580/87,518
regions. The changed WebP encoder projection is 2,391/2,471 lines, 511/540
branches, 89/89 functions, and 3,458/3,733 regions; the selected mode-array
lines were covered, including both the palette and non-palette paths. These
are implementation/Rust coverage metrics, not Pillow-parity coverage; the
known LLVM JSON segment-normalization warning and aggregate shortfall remain
visible.

Current acceptance record: WebP VP8L entropy-analysis histogram workspace

The production and Rust test/runtime slice is implemented at
`34006c8768b69866dde9dad37d2cd0f3e8623f67`, following the separate
entropy-analysis cost-table workspace at
`e8e3414584a62f600047c0cce49afa9a7f246d1f`. VP8L `analyze_entropy` now keeps
its fixed 13-channel by 256-value histogram accumulation table in a
function-local stack array instead of allocating a heap vector. The histogram
contents, traversal order, cancellation/error behavior, selected mode, encoded
bytes, and sink output remain unchanged.

This is Rust implementation and Rust-only workspace-allocation evidence.
Pillow exposes only the existing byte/error fixture matrix, not the table's
storage location, stack footprint, caller buffers, or OOM behavior. The
fixture matrix is therefore regression evidence, not proof of the internal
workspace contract; no parity row, fixture-manifest row, diagnostic origin,
new test function, coverage-only hook, or unit test was added. The exact-head
nightly LLVM run still reports the existing aggregate shortfall rather than
using a synthetic test to change it.

The clean schema-`@3` benchmark at this revision passed the Pillow-parity
workload in 1.661635 s wall / 3.428037 user s / 0.285070 sys s /
300,695,552-byte peak RSS and the separate Rust-only feature-gate workload in
2.760955 s wall / 2.846212 user s / 0.145434 sys s /
239,583,232-byte peak RSS. The native release build measured 10.124262 s wall /
33.562244 user s / 0.380754 sys s / 873,693,184-byte peak RSS and produced a
7,962,872-byte `rlib`. The `wasm32-unknown-unknown` determinism compile
measured 4.649578 s wall / 21.405451 user s / 1.078726 sys s /
922,484,736-byte peak RSS and produced a 24,155,354-byte artifact. These are
single-host/cache/toolchain observations, not comparative or universal
performance claims; allocation counts, retained cache bytes, stack depth, and
WASM runtime resources remain unmeasured.

Exact-head managed Pillow parity run
`1d20dd7a-197d-4ec5-babc-e0fef93fa5b4` passed 1,445/1,445 checks in 779 ms.
Exact-head feature-matrix run
`af84d5be-3a52-4d87-baf1-6e363fcf2af6` passed all configured native/WASI lanes
in 28,971 ms; its retained log includes
`capability tables OK: every native and wasm32-wasip1 lane agrees` and has no
`lock-wait` match. Nightly LLVM run
`b3a3d4cc-d096-4377-bffd-0db2941d81a4` passed 85/85 tests in 60,076 ms and
ingested snapshot `23975329-0598-40eb-85e6-092d7265d944`: 55,655/56,524
lines, 7,964/8,174 branches, 3,112/3,208 functions, and 85,578/87,516
regions. The changed-file projection is
`src/codecs/webp/native/encoder.rs`: 2,387/2,467 lines, 511/540 branches,
89/89 functions, and 3,456/3,731 regions; selected histogram-analysis lines
and branches were covered. These are Rust implementation/coverage records,
not Pillow-parity coverage; the known LLVM JSON segment-normalization warning
remains.

Current acceptance record: WebP VP8L no-token candidate suffix transfer

The production and Rust test/runtime slice is implemented at
`e86a3f8575ba3b6911ee122b9911ddc1d13b61b4`, following the no-token candidate
suffix transfer at `f33e303453cd2d5c3c67cfedf3a825e49922692e`. After the
candidate trial with the shortest encoded result wins, the ordinary no-token
path appends the owned suffix directly to the parent bitstream after reserving
capacity, then returns the now-empty vector to output scratch. The token-aware
path keeps the existing chunked suffix copy and cancellation checkpoints.

This is Rust implementation and Rust-only allocation/copy-ownership evidence.
Pillow exposes the final encoded bytes and errors but not the candidate suffix
allocation or transfer. The existing Pillow fixture matrix is therefore only
byte/error regression evidence; no parity row, fixture-manifest row, diagnostic
origin, new test function, coverage-only hook, or unit test was added. The
existing feature-gated Rust contract remains the separate evidence source for
caller-token, sink, and rollback behavior.

The clean schema-`@3` benchmark at this revision passed the Pillow-parity
workload in 0.943499 s wall / 2.773747 user s / 0.177703 sys s /
258,539,520-byte peak RSS and the separate Rust-only feature-gate workload in
1.552704 s wall / 2.244751 user s / 0.092927 sys s /
181,600,256-byte peak RSS. The native release build measured 6.492864 s wall /
32.260906 user s / 0.333621 sys s / 902,512,640-byte peak RSS and produced a
7,963,504-byte `rlib`. The `wasm32-unknown-unknown` determinism compile
measured 3.047466 s wall / 12.727719 user s / 0.834269 sys s /
823,017,472-byte peak RSS and produced a 24,170,169-byte artifact. These are
single-host/cache/toolchain observations, not comparative or universal
performance claims; allocation counts and WASM runtime resources remain
unmeasured.

Exact-head managed Pillow parity run
`bc24d021-fefa-4457-805e-e40781a6198d` passed 1,445/1,445 checks in 584 ms.
Exact-head feature-matrix run
`794e585d-e7a8-4d14-bea2-46fc8ce70098` passed all configured native/WASI lanes
in 18,523 ms; its retained log includes
`capability tables OK: every native and wasm32-wasip1 lane agrees` and has no
`lock-wait` match. Nightly LLVM run
`9e67827e-508a-4534-99a9-3acfd2b53f60` passed 85/85 tests in 50,151 ms and
ingested snapshot `67dd5c90-a55d-4227-8171-5d7b4dec263e`: 55,655/56,524
lines, 7,964/8,174 branches, 3,112/3,208 functions, and 85,578/87,516
regions. The changed-file projection is
`src/codecs/webp/native/encoder.rs`: 2,387/2,467 lines, 511/540 branches,
89/89 functions, and 3,456/3,731 regions. These are Rust
implementation/coverage records, not Pillow-parity coverage; the known LLVM
JSON segment-normalization warning remains.

Current acceptance record: WebP still RIFF output-buffer reuse

The production and Rust test/runtime slice is implemented at
`a03cd555652232b8fa909deae0983b7c93e99e1d`, following the WebP ALPH no-token
result allocation reuse at `940c9d82124fe0f2fc9b597fa2d7fb80e94fc4ea`.
After the ordinary no-token VP8L frame is complete, the encoder reuses that
frame allocation for the final RIFF/WEBP/VP8L result: it reserves the header
and pad capacity, shifts the payload behind the 20-byte prefix, and writes the
container fields in place. The token-aware path retains its separate output
buffer, chunk copy, and existing checkpoint behavior, so caller-controlled
work and sink semantics remain unchanged.

This is Rust implementation and Rust-only allocation-ownership evidence.
Pillow exposes the outer encoded bytes and errors but not the buffer lifetime,
capacity, or caller-token behavior. The existing Pillow fixture matrix is
therefore byte/error regression evidence, not proof of this internal boundary;
no parity row, fixture-manifest row, diagnostic origin, new test function, or
coverage-only hook was added, and no unit test was added. The existing
feature-gated Rust contract remains the separate evidence source for
token-aware behavior.

The clean schema-`@3` benchmark at this revision passed the Pillow-parity
workload in 0.966362 s wall / 2.844907 user s / 0.205257 sys s /
277,069,824-byte peak RSS and the separate Rust-only feature-gate workload in
1.562677 s wall / 2.256819 user s / 0.094236 sys s /
176,013,312-byte peak RSS. The native release build measured 6.238503 s wall /
31.647897 user s / 0.314244 sys s / 883,490,816-byte peak RSS and produced a
7,963,648-byte `rlib`. The `wasm32-unknown-unknown` determinism compile
measured 2.068798 s wall / 4.239620 user s / 0.593295 sys s /
586,809,344-byte peak RSS and produced a 24,166,071-byte artifact. These are
single-host/cache/toolchain observations, not comparative or universal
performance claims; allocation counts and WASM runtime resources remain
unmeasured.

Exact-head managed Pillow parity run
`17a93723-8330-4283-a4f9-ace16ccd9f77` passed 1,445/1,445 checks in 706 ms.
Exact-head feature-matrix run
`5703cd8d-a9a0-4673-8d6e-a5e5a74e6998` passed all configured native/WASI lanes
in 16,892 ms; its retained log includes
`capability tables OK: every native and wasm32-wasip1 lane agrees` and has no
`lock-wait` match. Nightly LLVM run
`c5b7aa40-544f-4e52-bbf7-f620f637815f` passed 85/85 tests in 59,176 ms and
ingested snapshot `223f74bc-97c4-4fe9-93f5-514e034ef16f`: 55,654/56,522
lines, 7,964/8,174 branches, 3,112/3,208 functions, and 85,576/87,511
regions. The changed-file projection is
`src/codecs/webp/native/encoder.rs`: 2,386/2,465 lines, 511/540 branches,
89/89 functions, and 3,454/3,726 regions. These are Rust
implementation/coverage records, not Pillow-parity coverage; the known LLVM
JSON segment-normalization warning remains.

Current acceptance record: WebP ALPH no-token result allocation reuse

The production and Rust test/runtime slice is implemented at
`940c9d82124fe0f2fc9b597fa2d7fb80e94fc4ea`, following the VP8L ordinary
palette-overflow short-circuit at `51cba9c0ec33cb3dbb20f4dc80442686af648476`.
After the lossless VP8L alpha payload is encoded, the ordinary no-token path
compares the known raw alpha length with the encoded payload before building
ALPH candidates. A raw winner allocates only its final header-plus-plane
vector; a compressed winner reuses the encoded payload allocation and inserts
the one-byte compression header in place. The token-aware path retains its
separate compressed/raw copies and 1,024-byte copy checkpoints, preserving the
existing caller-budget and sink boundaries.

This is Rust implementation and Rust-only allocation-ownership evidence.
Pillow exposes the outer encoded bytes and errors but not candidate allocation,
ownership, or caller-token behavior. The existing Pillow fixture matrix is
therefore byte/error regression evidence, not proof of this internal boundary;
no parity row, fixture-manifest row, diagnostic origin, new test function, or
coverage-only hook was added, and no unit test was added. The existing
feature-gated Rust contract remains the separate evidence source for
caller-budget and sink behavior.

The clean schema-`@3` benchmark at this revision passed the Pillow-parity
workload in 0.917503 s wall / 2.766210 user s / 0.152718 sys s /
237,633,536-byte peak RSS and the separate Rust-only feature-gate workload in
1.547690 s wall / 2.233890 user s / 0.089973 sys s /
169,263,104-byte peak RSS. The native release build measured 6.414562 s wall /
31.978778 user s / 0.345877 sys s / 883,359,744-byte peak RSS and produced a
7,961,920-byte `rlib`. The `wasm32-unknown-unknown` determinism compile
measured 3.101297 s wall / 16.856935 user s / 0.948080 sys s /
850,214,912-byte peak RSS and produced a 24,148,946-byte artifact. These are
host/cache/toolchain observations, not comparative or universal performance
claims; allocation counts and WASM runtime resources remain unmeasured.

Exact-head managed Pillow parity run
`98106af8-144c-4327-8c20-be3ede236be8` passed 1,445/1,445 checks in 622 ms.
Exact-head feature-matrix run
`9b692aa6-db01-4936-9875-c08cce3f92b6` passed all configured native/WASI lanes
in 19,227 ms; its retained log includes
`capability tables OK: every native and wasm32-wasip1 lane agrees` and has no
`lock-wait` match. Nightly LLVM run
`f9892ce9-ed50-4e19-ac77-d5a8a7b7f7ee` passed 85/85 tests in 49,615 ms and
ingested snapshot `a90850c1-69cc-46ff-bff6-be399f3aa542`: 55,638/56,506
lines, 7,960/8,170 branches, 3,112/3,208 functions, and 85,541/87,476
regions. The changed-file projection is
`src/codecs/webp/native/encoder.rs`: 2,370/2,449 lines, 507/536 branches,
89/89 functions, and 3,419/3,691 regions. These are Rust
implementation/coverage records, not Pillow-parity coverage; the known LLVM
JSON segment-normalization warning remains.

Current acceptance record: WebP VP8L ordinary palette overflow short-circuit

The production and Rust test/runtime slice is implemented at
`51cba9c0ec33cb3dbb20f4dc80442686af648476`, following the alpha-palette
presence-table workspace at `05e823facedf3ece60767f02e371fc8bcc1a69a4`.
Lossless WebP palette discovery now stops the ordinary no-token path after the
257th distinct ARGB color and returns a sorted 257-entry sentinel: palette mode
is only eligible through 256 entries, so the full unique-color set is dead once
that threshold is crossed. Inputs with at most 256 colors retain the exact
sorted result; the token-aware path retains its complete ordered drain and
1,024-pixel/unique-color checkpoint contract. The resulting entropy mode,
encoded bytes, errors, and sink output remain unchanged.

This is Rust implementation and Rust-only runtime/allocation evidence. Pillow
exposes only the existing byte/error fixture matrix, not the internal unique-
color cutoff, retained set, allocator behavior, or OOM result. The fixture
matrix is therefore regression evidence, not proof of this internal boundary;
no parity row, fixture-manifest row, diagnostic origin, new test function, or
coverage-only hook was added. The existing feature-gated Rust contract remains
the separate evidence source for caller-budget and sink behavior; no unit test
was added. The current changed-file projection is
`src/codecs/webp/native/encoder.rs`: 2,367/2,435 lines, 506/532 branches,
89/89 functions, and 3,416/3,664 regions. Existing uncovered and partial
branches remain visible; no synthetic unit or parity input was used to alter
them.

The clean schema-`@3` benchmark at the preceding functional revision
`b99cd77b27e1b1172307043542665c00cae06b64` observed the Pillow-parity
workload at 1.587047 s wall / 3.634331 user s / 0.358316 sys s /
296,157,184-byte peak RSS and the separate Rust-only feature-gate workload at
2.903817 s wall / 3.201076 user s / 0.169898 sys s /
254,033,920-byte peak RSS. The native release `rlib` was 7,961,640 bytes;
native release compilation measured 12.407440 s wall with 907,804,672-byte
peak RSS. The `wasm32-unknown-unknown` determinism artifact was 24,138,450
bytes; its compile measured 4.553721 s wall with 951,140,352-byte peak RSS.
The amended `51cba9c0ec33cb3dbb20f4dc80442686af648476` changes only an
explanatory source comment, so this clean functional benchmark remains
applicable. These are host/cache/toolchain observations, not comparative or universal
performance claims; allocation counts, retained encoded/decoded cache bytes,
caller-buffer reuse, stack depth, and WASM runtime resources remain unmeasured.

Exact-head managed Pillow parity run
`606ffe6f-b475-4f86-b5ad-073813159d36` passed 1,445/1,445 checks in 8,261 ms.
Exact-head feature-matrix run
`78993824-2837-4f5f-8c66-1430c96d5396` passed all configured native/WASI lanes
in 32,371 ms; its retained log includes
`capability tables OK: every native and wasm32-wasip1 lane agrees` and has no
`lock-wait` match. Nightly LLVM run
`8e1683e3-2ea6-4197-8fb1-42cd10e79afc` passed 85/85 tests in 68,978 ms and
ingested snapshot `071bde59-b2e6-4872-a689-be46fa30ddd9`: 55,635/56,492
lines, 7,959/8,166 branches, 3,112/3,208 functions, and 85,538/87,449
regions. Compared with the preceding accepted snapshot, source and covered
line totals rose by 1, branch totals rose by 2, and region totals rose by 8;
function totals were unchanged. The aggregate shortfall remains 857 lines,
207 branches, 96 functions, and 1,911 regions. These are Rust
implementation/coverage records, not Pillow-parity coverage; the known LLVM
JSON segment-normalization warning remains.

Current acceptance record: WebP RGBA alpha-palette presence-table workspace

The production and Rust test/runtime slice is implemented at
`05e823facedf3ece60767f02e371fc8bcc1a69a4`, following the initial presence-
table implementation at `95944b05de49cf5ae4172f2f0fe90fa2a727a1c1` and the
RGBA alpha-palette delta stack workspace at
`ea5f77781d0ca530bf23fd3b3fc12fc84da3dada`. WebP RGBA alpha-palette
collection records membership in a fixed `[bool; 256]` stack table, counts
newly seen values during the same scan, and reserves the exact sorted unique
palette `Vec` length. This removes the bounded `BTreeSet` node allocation and
avoids reserving unused palette capacity for sparse alpha planes. The
1,024-source-pixel cancellation cadence, sorted palette result, encoded bytes,
errors, and sink output remain unchanged.

This is Rust implementation and Rust-only bounded-workspace evidence. Pillow
exposes only the existing byte/error fixture matrix, not the presence-table
storage location, stack footprint, caller buffers, or OOM behavior. The
fixture matrix is therefore regression evidence, not proof of the internal
workspace contract; no parity row, fixture-manifest row, diagnostic origin,
new test function, or coverage-only hook was added. The existing feature-gated
Rust contract remains the separate evidence source for caller-budget and sink
behavior; no unit test was added. The current changed-file projection is
`src/codecs/webp/native/encoder.rs`: 2,366/2,434 lines, 504/530 branches,
89/89 functions, and 3,408/3,656 regions. Existing uncovered and partial
branches remain visible; no synthetic unit or parity input was used to alter
them.

The clean schema-`@3` benchmark passed the Pillow-parity workload in 0.938141 s
wall / 2.815871 user s / 0.194917 sys s / 252,428,288-byte peak RSS and the
separate Rust-only feature-gate workload in 1.555419 s wall / 2.236458 user s
/ 0.102730 sys s / 166,215,680-byte peak RSS. The native release `rlib` was
7,981,024 bytes and the `wasm32-unknown-unknown` determinism artifact was
24,307,411 bytes; native release compilation measured 6.404714 s wall with
889,634,816-byte peak RSS, and the WASM determinism compile measured 2.954595
s wall with 781,221,888-byte peak RSS. These are host/cache/toolchain
observations, not comparative or universal performance claims; allocation
counts, retained encoded/decoded cache bytes, caller-buffer reuse, stack depth,
and WASM runtime resources remain unmeasured.

Exact-head managed Pillow parity run
`15b2b36b-ddde-492c-94f9-85493146e74c` passed 1,445/1,445 checks in 865 ms.
Exact-head feature-matrix run
`7d670eae-978f-42ab-8e2a-2e0c30ca9dc8` passed all configured native/WASI lanes
in 15,478 ms; its retained log includes
`capability tables OK: every native and wasm32-wasip1 lane agrees` and has no
`lock-wait` match. Nightly LLVM run
`06e03a38-f8bb-45d1-b40b-6182b7167f49` passed 85/85 tests in 53,126 ms and
ingested snapshot `b67dfb0a-615c-4872-a1c2-76c95870ac2c`: 55,634/56,491
lines, 7,957/8,164 branches, 3,112/3,208 functions, and 85,530/87,441
regions. Compared with the preceding accepted snapshot, source and covered
line totals rose by 5, branch totals rose by 2, and region totals rose by 7
as the exact-capacity count and branch were compiled into the current slice;
function totals were unchanged. The aggregate shortfall remains 857 lines,
207 branches, 96 functions, and 1,911 regions. These are
Rust implementation/coverage records, not Pillow-parity coverage; the known
LLVM JSON segment-normalization warning remains.

Current acceptance record: WebP RGBA alpha-palette delta stack workspace

The production and Rust test/runtime slice is implemented at
`ea5f77781d0ca530bf23fd3b3fc12fc84da3dada`, following the VP8L palette-delta
stack workspace at `e965689f4a3e7b620cc9393d16e1b158a267cda0`. WebP RGBA alpha
encoding now computes the at-most-256 palette deltas into a fixed stack array
instead of collecting a second heap vector. The sorted alpha palette remains
intact for index lookup; palette order, delta arithmetic, encoded bytes,
errors, and sink output remain unchanged.

This is Rust implementation and Rust-only bounded-workspace evidence. Pillow
exposes only the existing byte/error fixture matrix, not the alpha-palette
delta storage location, stack footprint, caller buffers, or OOM behavior. The
fixture matrix is therefore regression evidence, not proof of the internal
workspace contract; no parity row, fixture-manifest row, diagnostic origin,
new test function, or coverage-only hook was added. The current changed-file
projection is `src/codecs/webp/native/encoder.rs`: 2,363/2,431 lines,
502/528 branches, 89/89 functions, and 3,400/3,648 regions. Existing
uncovered and partial branches remain visible; no synthetic unit or parity
input was used to alter them.

The clean schema-`@3` benchmark at this revision passed the Pillow-parity
workload in 0.948695 s wall / 2.844942 user s / 0.241720 sys s /
266,600,448-byte peak RSS and the separate Rust-only feature-gate workload in
1.584676 s wall / 2.280832 user s / 0.120655 sys s /
183,975,936-byte peak RSS. The native release `rlib` was 7,988,936 bytes and
the `wasm32-unknown-unknown` determinism artifact was 24,649,436 bytes;
native release compilation measured 6.707265 s wall with 888,242,176-byte
peak RSS, and the WASM determinism compile measured 3.130467 s wall with
810,467,328-byte peak RSS. These are host/cache/toolchain observations, not
comparative or universal performance claims; allocation counts, retained
encoded/decoded cache bytes, caller-buffer reuse, stack depth, and WASM runtime
resources remain unmeasured.

Exact-head managed Pillow parity run
`fa25b44b-8e5b-4502-a497-2279769a46fd` passed 1,445/1,445 checks in 1,105 ms.
Exact-head feature-matrix run
`296847d9-8302-4da0-b87d-c055fa547255` passed all configured native/WASI lanes
in 21,358 ms; its retained log includes
`capability tables OK: every native and wasm32-wasip1 lane agrees` and has no
`lock-wait` match. Nightly LLVM run
`f0c767b4-e9ce-4a8d-9ee9-21558f73403e` passed 85/85 tests in 57,493 ms and
ingested snapshot `ea481c1a-dd44-4f5d-bdd8-a72d1291ee47`: 55,631/56,488
lines, 7,955/8,162 branches, 3,112/3,208 functions, and 85,522/87,433
regions. Compared with the preceding accepted snapshot, line, branch, and
function totals were unchanged; source and covered region totals fell by 2
because the temporary alpha-palette delta collection was removed. The
aggregate shortfall remains 857 lines, 207 branches, 96 functions, and 1,911
regions. These are Rust implementation/coverage records, not Pillow-parity
coverage; the known LLVM JSON segment-normalization warning remains.

Current acceptance record: WebP VP8L palette-delta stack workspace

The production and Rust test/runtime slice is implemented at
`e965689f4a3e7b620cc9393d16e1b158a267cda0`, following alpha-palette
transformed-view reuse at `154339de3db7521b10ce623deb0487c52517aea2`. VP8L
palette-mode writing now computes the at-most-256 palette deltas into a fixed
stack array instead of collecting a temporary heap vector. The source palette
remains intact for the later index-packing pass; palette order, delta
arithmetic, encoded bytes, errors, and sink output remain unchanged.

This is Rust implementation and Rust-only bounded-workspace evidence. Pillow
exposes only the existing byte/error fixture matrix, not the palette-delta
storage location, stack footprint, caller buffers, or OOM behavior. The fixture
matrix is therefore regression evidence, not proof of the internal workspace
contract; no parity row, fixture-manifest row, diagnostic origin, new test
function, or coverage-only hook was added. The current changed-file projection
is `src/codecs/webp/native/encoder.rs`: 2,363/2,431 lines, 502/528 branches,
89/89 functions, and 3,402/3,650 regions. Existing uncovered and partial
branches remain visible; no synthetic unit or parity input was used to alter
them.

The clean schema-`@3` benchmark at this revision passed the Pillow-parity
workload in 0.941541 s wall / 2.817685 user s / 0.198685 sys s /
252,870,656-byte peak RSS and the separate Rust-only feature-gate workload in
1.578799 s wall / 2.254704 user s / 0.097419 sys s /
190,332,928-byte peak RSS. The native release `rlib` was 7,988,488 bytes and
the `wasm32-unknown-unknown` determinism artifact was 24,653,766 bytes;
native release compilation measured 6.777176 s wall with 870,793,216-byte
peak RSS, and the WASM determinism compile measured 3.652705 s wall with
880,197,632-byte peak RSS. These are host/cache/toolchain observations, not
comparative or universal performance claims; allocation counts, retained
encoded/decoded cache bytes, caller-buffer reuse, stack depth, and WASM runtime
resources remain unmeasured.

Exact-head managed Pillow parity run
`e9f0ea46-29ad-4f5c-b9e8-5eb6ba0fbd82` passed 1,445/1,445 checks in 834 ms.
Exact-head feature-matrix run
`35d008b4-4017-493e-9561-c2db59d2e044` passed all configured native/WASI lanes
in 27,436 ms; its retained log includes
`capability tables OK: every native and wasm32-wasip1 lane agrees` and has no
`lock-wait` match. Nightly LLVM run
`062495cd-0c59-46e9-bc37-dd14b8f8f08e` passed 85/85 tests in 54,357 ms and
ingested snapshot `86aa5ea2-7ea7-4846-afa0-7791ddb297fc`: 55,631/56,488
lines, 7,955/8,162 branches, 3,112/3,208 functions, and 85,524/87,435
regions. Compared with the preceding accepted snapshot, branch and region
totals were unchanged; source and covered totals fell by 3 lines and function
totals fell by 1 because the temporary palette-delta collection was removed.
The aggregate shortfall remains 857 lines, 207 branches, 96 functions, and
1,911 regions. These are Rust implementation/coverage records, not
Pillow-parity coverage; the known LLVM JSON segment-normalization warning
remains.

Current acceptance record: WebP VP8L alpha-palette transformed-view reuse

The production and Rust test/runtime slice is implemented at
`154339de3db7521b10ce623deb0487c52517aea2`, following entropy-analysis cost
table reuse at `e8e3414584a62f600047c0cce49afa9a7f246d1f`. VP8L `encode_alpha`
now builds the palette delta table directly from the retained u8 palette values
instead of materializing a second shifted `Vec<u32>`. Palette ordering, delta
arithmetic, encoded bytes, errors, and sink output remain unchanged.

This is Rust implementation and Rust-only workspace-allocation evidence.
Pillow exposes only the existing byte/error fixture matrix, not the temporary
transformed palette, stack footprint, caller buffers, or OOM behavior. The
fixture matrix is therefore regression evidence, not proof of the internal
allocation contract; no parity row, fixture-manifest row, diagnostic origin,
new test function, or coverage-only hook was added. The current changed-file
projection is `src/codecs/webp/native/encoder.rs`: 2,366/2,434 lines,
502/528 branches, 90/90 functions, and 3,402/3,650 regions. Existing
uncovered and partial branches remain visible; no synthetic unit or parity
input was used to alter them.

The clean schema-`@3` benchmark at this revision passed the Pillow-parity
workload in 1.460114 s wall / 3.392521 user s / 0.285452 sys s /
291,454,976-byte peak RSS and the separate Rust-only feature-gate workload in
2.731251 s wall / 2.875439 user s / 0.144347 sys s /
234,274,816-byte peak RSS. The native release `rlib` was 7,991,440 bytes and
the `wasm32-unknown-unknown` determinism artifact was 24,682,951 bytes;
native release compilation measured 10.978130 s wall with 880,672,768-byte
peak RSS, and the WASM determinism compile measured 5.554196 s wall with
866,074,624-byte peak RSS. These are host/cache/toolchain observations, not
comparative or universal performance claims; allocation counts, retained
encoded/decoded cache bytes, caller-buffer reuse, stack depth, and WASM runtime
resources remain unmeasured.

Exact-head managed Pillow parity run
`d306336b-2006-468d-8269-941d4d7c4f0a` passed 1,445/1,445 checks in 882 ms.
Exact-head feature-matrix run
`65a5320a-2e88-4332-afd8-2405ddd47a96` passed all configured native/WASI lanes
in 30,948 ms; its retained log includes
`capability tables OK: every native and wasm32-wasip1 lane agrees` and has no
`lock-wait` match. Nightly LLVM run
`8c06bc47-fa81-436b-83d9-d22a5cbd17cb` passed 85/85 tests in 64,064 ms and
ingested snapshot `1ce4dcce-7dec-44c8-b6dc-0a1d132e14b4`: 55,634/56,491
lines, 7,955/8,162 branches, 3,113/3,209 functions, and 85,524/87,435
regions. Compared with the preceding accepted snapshot, branch totals were
unchanged; source and covered totals fell by 3 lines, function totals fell by
1, and region totals fell by 4 because the redundant transformed palette was
removed. The aggregate shortfall therefore remains 857 lines, 207 branches,
96 functions, and 1,911 regions. These are Rust implementation/coverage
records, not Pillow-parity coverage; the known LLVM JSON segment-normalization
warning remains.

Current acceptance record: WebP VP8L entropy-analysis cost table reuse

The production and Rust test/runtime slice is implemented at
`e8e3414584a62f600047c0cce49afa9a7f246d1f`, following box-chain offset
workspace reuse at `4c40da5796b0ca8aba6c42da55887e6254ee2522`. VP8L
`analyze_entropy` now stores the fixed 13-entry histogram-cost table in a
stack array instead of collecting it into a temporary `Vec`. The cost
traversal order, short-circuit error propagation, cancellation behavior, mode
selection, encoded bytes, and sink output remain unchanged.

This is Rust implementation and Rust-only workspace-allocation evidence.
Pillow exposes only the existing byte/error fixture matrix, not the entropy
cost table's storage location, stack footprint, caller buffers, or OOM
behavior. The fixture matrix is therefore regression evidence, not proof of
the internal workspace contract; no parity row, fixture-manifest row,
diagnostic origin, new test function, or coverage-only hook was added. The
current changed-file projection is
`src/codecs/webp/native/encoder.rs`: 2,369/2,437 lines, 502/528 branches,
91/91 functions, and 3,406/3,654 regions. Existing uncovered and partial
branches remain visible; no synthetic unit or parity input was used to alter
them.

The clean schema-`@3` benchmark at this revision passed the Pillow-parity
workload in 0.961444 s wall / 2.856234 user s / 0.199489 sys s /
246,530,048-byte peak RSS and the separate Rust-only feature-gate workload in
1.533239 s wall / 2.216942 user s / 0.092648 sys s /
152,043,520-byte peak RSS. The native release `rlib` was 7,994,848 bytes and
the `wasm32-unknown-unknown` determinism artifact was 24,704,717 bytes;
native release compilation measured 6.174461 s wall with 852,541,440-byte
peak RSS, and the WASM determinism compile measured 3.732592 s wall with
926,416,896-byte peak RSS. These are host/cache/toolchain observations, not
comparative or universal performance claims; allocation counts, retained
encoded/decoded cache bytes, caller-buffer reuse, stack depth, and WASM runtime
resources remain unmeasured.

Exact-head managed Pillow parity run
`2320e1bb-c82d-4345-9ba3-895846731544` passed 1,445/1,445 checks in 578 ms.
Exact-head feature-matrix run
`178fc148-246b-46fc-8e85-fefe35c5c7c1` passed all configured native/WASI lanes
in 29,320 ms; its retained log includes
`capability tables OK: every native and wasm32-wasip1 lane agrees` and has no
`lock-wait` match. Nightly LLVM run
`a7e175fe-957c-4788-998d-85e4a86c185e` passed 85/85 tests in 61,142 ms and
ingested snapshot `c6c61771-3eea-488b-af60-6c78d27de70b`: 55,637/56,494
lines, 7,955/8,162 branches, 3,114/3,210 functions, and 85,528/87,439
regions. Compared with the preceding accepted snapshot, branch totals were
unchanged; source and covered totals fell by 2 lines, 1 function, and 8
regions because the heap-collect path was removed. The aggregate shortfall
therefore remains 857 lines, 207 branches, 96 functions, and 1,911 regions.
These are Rust implementation/coverage records, not Pillow-parity coverage;
the known LLVM JSON segment-normalization warning remains.

Current acceptance record: WebP VP8L box-chain offset workspace reuse

The production and Rust test/runtime slice is implemented at
`4c40da5796b0ca8aba6c42da55887e6254ee2522`, following candidate-result-list
reuse at `5e56c103068056e71617695a5c8bc0e47d240634`. VP8L `box_chain` now
filters its nonzero offset-code candidates into fixed 32-entry stack arrays
instead of allocating temporary vectors for the full and incremental offset
sets. The arrays preserve offset order and the same full-versus-incremental
chain selection; encoded bytes, errors, checkpoint behavior, and sink output
remain unchanged.

This is Rust implementation and Rust-only workspace-allocation evidence.
Pillow exposes only the existing byte/error fixture matrix, not temporary
offset storage, stack footprint, retained capacity, caller buffers, or OOM
behavior. The fixture matrix is therefore regression evidence, not proof of
the internal workspace contract; no parity row, fixture-manifest row,
diagnostic origin, new test function, or coverage-only hook was added. The
current changed-file projection is
`src/codecs/webp/native/encoder/backward_refs.rs`: 1,881/1,935 lines,
497/530 branches, 72/72 functions, and 2,813/2,973 regions. Existing
uncovered and partial branches remain visible; no synthetic unit or parity
input was used to alter them.

The clean schema-`@3` benchmark at this revision passed the Pillow-parity
workload in 0.977759 s wall / 2.830277 user s / 0.246740 sys s /
275,857,408-byte peak RSS and the separate Rust-only feature-gate workload in
1.579698 s wall / 2.267066 user s / 0.108978 sys s /
220,512,256-byte peak RSS. The native release `rlib` was 7,993,344 bytes and
the `wasm32-unknown-unknown` determinism artifact was 24,738,297 bytes;
native release compilation measured 6.710649 s wall with 873,447,424-byte
peak RSS, and the WASM determinism compile measured 3.719729 s wall with
880,050,176-byte peak RSS. These are host/cache/toolchain observations, not
comparative or universal performance claims; allocation counts, retained
encoded/decoded cache bytes, caller-buffer reuse, stack depth, and WASM runtime
resources remain unmeasured.

Exact-head managed Pillow parity run
`e049fbe4-a180-44ac-ad01-d6e86b2f8bd1` passed 1,445/1,445 checks in 849 ms.
Exact-head feature-matrix run
`a178cf7f-f22e-49d7-b7b9-641e62407093` passed all configured native/WASI lanes
in 28,688 ms; its retained log includes
`capability tables OK: every native and wasm32-wasip1 lane agrees` and has no
`lock-wait` match. Nightly LLVM run
`2f38dff3-0177-4cae-bb93-49ee17f99c7c` passed 85/85 tests in 65,042 ms and
ingested snapshot `74c1c1ec-ab1b-4c0e-ab55-f69b1ea58c8e`: 55,639/56,496
lines, 7,955/8,162 branches, 3,115/3,211 functions, and 85,536/87,447
regions. Compared with the preceding accepted snapshot, covered totals rose
by 2 lines, 2 branches, and 5 regions, while the reported function totals
fell by 1; source totals changed by the same amounts. These are Rust
implementation/coverage records, not Pillow-parity coverage; the known LLVM
JSON segment-normalization warning remains.

Current acceptance record: WebP VP8L candidate-result list reuse

The production and Rust test/runtime slice is implemented at
`5e56c103068056e71617695a5c8bc0e47d240634`, following the candidate-result
token-pool reuse at `aa65af084624175a0279f42ffe904107e921db8b`. The outer
`Vec<(Vec<Token>, u8)>` returned by VP8L candidate construction now retains its
small allocation in `CandidateScratch` across image streams. Each stream drains
the bounded standard and optional box-chain list, and the list storage is
restored even when a token-aware trial returns an error; candidate token
vectors remain independently owned by the existing bounded pool or active
trial. Candidate ordering, cache-bit selection, checkpoint behavior, encoded
bytes, errors, and sink output remain unchanged.

This is Rust implementation and Rust-only allocation-ownership evidence.
Pillow exposes only the existing byte/error fixture matrix, not result-list
allocation, retained capacity, caller buffers, or OOM behavior. The fixture
matrix is therefore regression evidence, not proof of the internal result-list
contract; no parity row, fixture-manifest row, diagnostic origin, new test
function, or coverage-only hook was added. Current changed-file projections are
`src/codecs/webp/native/encoder.rs`: 2,371/2,439 lines, 502/528 branches,
92/92 functions, and 3,414/3,662 regions; and
`src/codecs/webp/native/encoder/backward_refs.rs`: 1,879/1,933 lines,
495/528 branches, 73/73 functions, and 2,808/2,968 regions. Existing
uncovered and partial branches remain visible; no synthetic unit or parity
input was used to alter them.

The clean schema-`@3` benchmark at this revision passed the Pillow-parity
workload in 1.029030 s wall / 2.766949 user s / 0.204220 sys s /
252,788,736-byte peak RSS and the separate Rust-only feature-gate workload in
1.551884 s wall / 2.214894 user s / 0.101167 sys s /
166,510,592-byte peak RSS. The native release `rlib` was 8,001,576 bytes and
the `wasm32-unknown-unknown` determinism artifact was 24,771,874 bytes; native
release compilation measured 6.315052 s wall with 873,398,272-byte peak RSS,
and the WASM determinism compile measured 3.984604 s wall with
947,126,272-byte peak RSS. These are host/cache/toolchain observations, not
comparative or universal performance claims; allocation counts, retained
encoded/decoded cache bytes, caller-buffer reuse, stack depth, and WASM runtime
resources remain unmeasured.

Exact-head managed Pillow parity run
`0e315e94-47d6-48be-b27b-1c44a4b19413` passed 1,445/1,445 checks in 820 ms.
Exact-head feature-matrix run
`ef7b94c0-c2b6-4823-b3b8-a9a1ff514028` passed all configured native/WASI lanes
in 30,771 ms; its retained log includes
`capability tables OK: every native and wasm32-wasip1 lane agrees` and has no
`lock-wait` match. Nightly LLVM run
`6a000cd0-8920-4fcb-aa1b-ac9479777b6f` passed 85/85 tests in 51,239 ms and
ingested snapshot `574b447c-e80b-4cec-a7b7-179cf7a0d9a4`: 55,637/56,494
lines, 7,953/8,160 branches, 3,116/3,212 functions, and 85,531/87,442
regions. Compared with the preceding accepted snapshot, covered totals rose
by 7 lines, 0 branches, 1 function, and 15 regions; the line-rate increase is
source growth plus maintained coverage. These are Rust implementation/
coverage records, not Pillow-parity coverage; the known LLVM JSON
segment-normalization warning remains.

Current acceptance record: WebP VP8L histogram pair-queue scratch reuse

The production and Rust test/runtime slice is implemented at
`dc65e760117e9bc5155c16fdf68ffffe97524c25`, following histogram merge scratch
reuse at `bb654ca65ec0bc5a15000d32f7cf924b233a9738`. Histogram clustering now
retains one pair queue in `HistogramScratch` across stochastic and greedy
passes, clears it between passes, and releases it when capacity exceeds 4,096
`Pair` entries. This removes repeated queue allocation for ordinary candidate
streams while bounding retained memory; pair ordering, merge decisions,
cancellation behavior, encoded bytes, errors, and sink output remain
unchanged.

This is Rust implementation and Rust-only allocation-ownership evidence.
Pillow exposes only the existing byte/error fixture matrix, not pair-queue
allocation, retained capacity, caller buffers, or OOM behavior. The existing
fixture matrix is therefore regression evidence, not proof of the internal
queue contract; no parity row, fixture-manifest row, diagnostic origin, new
test function, or coverage-only hook was added. The existing Coverage MCP
snapshot covers 872/873 histogram lines, 184/184 branches, and 43/43
functions; its one uncovered line is the `greedy_combine` call at line 898,
which the managed fixture inputs and existing coverage witness do not reach.
That internal branch remains visible as a coverage gap rather than being
masked with a synthetic unit test or parity input.

The clean schema-`@3` benchmark at this revision passed the Pillow-parity
workload in 1.057417 s wall / 2.812377 user s / 0.209874 sys s /
264,159,232-byte peak RSS and the separate Rust-only feature-gate workload in
1.549025 s wall / 2.218592 user s / 0.099256 sys s /
167,968,768-byte peak RSS. The native release `rlib` was 7,996,872 bytes and
the `wasm32-unknown-unknown` determinism artifact was 24,751,983 bytes; native
release compilation measured 6.453998 s wall with 897,515,520-byte peak RSS,
and the WASM determinism compile measured 3.907174 s wall with
914,833,408-byte peak RSS. These are host/cache/toolchain observations, not
comparative or universal performance claims; allocation counts, retained
encoded/decoded cache bytes, caller-buffer reuse, stack depth, and WASM runtime
resources remain unmeasured.

Exact-head managed Pillow parity run
`1935d463-5cce-4016-998a-7035d20c34a9` passed 1,445/1,445 checks in 658 ms.
Exact-head feature-matrix run
`5a524032-009d-47c2-ba48-7f3ca1e29178` passed all configured native/WASI lanes
in 21,808 ms; its retained log includes
`capability tables OK: every native and wasm32-wasip1 lane agrees` and has no
`lock-wait` match. Nightly LLVM run
`077e16ef-8ec6-4530-8b4f-ed5a1088d1c6` passed 85/85 tests in 52,814 ms and
ingested snapshot `a6858d88-f16e-4f18-9d85-059afa70045f`: 55,630/56,487
lines, 7,953/8,160 branches, 3,115/3,211 functions, and 85,516/87,427
regions. Compared with the preceding accepted snapshot, covered totals rose
by 26 lines, 2 branches, 2 functions, and 27 regions; the line-rate decrease
is a source-growth effect. These are Rust implementation/coverage records,
not Pillow-parity coverage; the known LLVM JSON segment-normalization warning
remains.

Current acceptance record: WebP VP8L histogram merge scratch reuse

The production and Rust test/runtime slice is implemented at
`bb654ca65ec0bc5a15000d32f7cf924b233a9738`, following predictor and cross-color
transform scratch reuse at `533f97ee45bcc750fb0373da6272c3955963ce22`.
Entropy-bin, stochastic, and greedy VP8L histogram combinations now merge into
one retained `Histogram` scratch value, then swap the completed result into
the cluster only after the merge succeeds. This removes repeated full
five-channel `Histogram` clones while preserving cancellation rollback,
cluster ordering, checkpoint behavior, encoded bytes, errors, and sink output.
This is Rust implementation and Rust-only allocation-ownership evidence.
Pillow observes only the existing byte/error fixture matrix, not allocator
ownership, caller buffers, or OOM behavior. No parity row, fixture-manifest
row, diagnostic origin, new test function, or coverage-only hook was added;
the existing private coverage exerciser was only updated for the refactored
combiner signatures.

The clean schema-`@3` benchmark at this revision passed the Pillow-parity
workload in 0.936337 s wall / 2.783766 user s / 0.220714 sys s /
257,900,544-byte peak RSS and the separate Rust-only feature-gate workload in
1.588121 s wall / 2.257646 user s / 0.099453 sys s /
147,521,536-byte peak RSS. The native release `rlib` was 7,996,080 bytes and
the `wasm32-unknown-unknown` determinism artifact was 24,753,956 bytes; native
release compilation measured 6.607998 s wall with 881,819,648-byte peak RSS,
and the WASM determinism compile measured 3.927014 s wall with
923,222,016-byte peak RSS. These are host/cache/toolchain observations, not
comparative or universal performance claims; allocation counts, retained
encoded/decoded cache bytes, caller-buffer reuse, stack depth, and WASM runtime
resources remain unmeasured.

Exact-head managed Pillow parity run
`9d3512a2-8270-42b1-a907-c058fd882677` passed 1,445/1,445 checks in 590 ms.
Exact-head feature-matrix run
`11a51b59-415c-46ab-8697-d3c4cafa865b` passed all configured native/WASI lanes
in 22,092 ms; its retained log includes
`capability tables OK: every native and wasm32-wasip1 lane agrees` and has no
`lock-wait` match. Nightly LLVM run
`c67a5431-6c99-43d2-9d4e-dd380bc378d7` passed 85/85 tests in 53,762 ms and
ingested snapshot `b6280073-65f9-44b9-b49e-89f75eee32cb`: 55,604/56,460
lines, 7,951/8,158 branches, 3,113/3,209 functions, and 85,489/87,400
regions. Histogram reports 846/846 lines, 182/182 branches, and 41/41
functions; predictor reports 366/366 lines, 68/68 branches, and 24/24
functions; cross-color reports 517/530 lines, 83/86 branches, and 27/27
functions. These are Rust implementation/coverage records, not Pillow-parity
coverage; the known LLVM JSON segment-normalization warning remains.

Current acceptance record: WebP VP8L predictor and cross-color transform
scratch reuse

The production and Rust test/runtime slice is implemented at
`533f97ee45bcc750fb0373da6272c3955963ce22`, following predictor-transform
scratch reuse at `7b99b1e4f1c3ee65a6533b9b80bcec2c5bd7c9f4`. The image-stream
workspace now retains predictor mode-map, source-snapshot, and upper/current-row
storage across predictor candidates, and retains the cross-color tile map
across sequential stream use before truncating it for emission. Predictor and
cross-color transform ordering, encoded bytes, errors, and sink output remain
unchanged; no-token paths retain their direct tight loops and copies.

This is Rust implementation and Rust-only allocation-ownership evidence.
Pillow observes only the existing byte/error fixture matrix, not allocator
ownership, caller buffers, or OOM behavior. The existing WebP encode matrix
(28/13/47 rows), full fixture matrix, all 45 feature-gated Rust contracts, all
83 local all-feature tests, strict Clippy, rustfmt, and all configured
native/WASI feature-matrix lanes passed. No parity row, fixture-manifest row,
diagnostic origin, new test function, or coverage-only hook was added; the
existing private coverage exerciser was only updated for refactored scratch
function signatures.

The clean schema-`@3` benchmark at this revision passed the Pillow-parity
workload in 0.937318 s wall / 2.751656 user s / 0.171839 sys s /
246,284,288-byte peak RSS and the separate Rust-only feature-gate workload in
1.527709 s wall / 2.193692 user s / 0.093055 sys s /
175,439,872-byte peak RSS. The native release `rlib` was 7,997,064 bytes and
the `wasm32-unknown-unknown` determinism artifact was 24,751,650 bytes. These
are host/cache/toolchain observations, not comparative or universal
performance claims; allocation counts, retained cache bytes, caller-buffer
reuse, stack depth, and WASM runtime resources remain unmeasured.

Exact-head managed Pillow parity run
`331169e5-e7b7-4328-94cb-57c8779f807f` passed 1,445/1,445 checks in 568 ms.
Exact-head feature-matrix run
`3bbf783f-a160-4efe-be66-8e79bf185700` passed all configured native/WASI lanes
in 26,665 ms; its retained log includes
`capability tables OK: every native and wasm32-wasip1 lane agrees` and has no
`lock-wait` match. Nightly LLVM run
`ad600045-94ee-4806-9d02-144befaddba1` passed 85/85 tests in 51,274 ms and
ingested snapshot `3a4b9d14-b1c4-4576-8d00-da3f3c89596c`: 55,607/56,463
lines, 7,951/8,158 branches, 3,113/3,209 functions, and 85,467/87,378
regions. Predictor reports 366/366 lines, 68/68 branches, and 24/24
functions; cross-color reports 517/530 lines, 83/86 branches, and 27/27
functions. These are Rust implementation/coverage records, not Pillow-parity
coverage; the known LLVM JSON segment-normalization warning remains.

The lossless WebP VP8L backward-reference scratch-reuse slice is implemented at
production and Rust test/runtime revision
`4c76598e9bb71133e626f42bfb94bcf1544bfa84`, following histogram-clustering
scratch reuse at `36ce85ba244d7195baef8d5fea7adcdd3cbcc613`. Each token stream
now retains one candidate workspace: the hash-chain result table, 18-bit
hash-head table, box-chain run counts, source-token buffer, cost-estimate
storage, cache-transform storage, and trace storage reset their logical contents
before reuse across sequential image streams. Selected candidate token vectors
remain independently owned, and nested metadata streams retain their own
workspace. Candidate ordering, encoded bytes, errors, and sink output remain
unchanged. The existing WebP encode matrix (28/13/47 rows), full fixture matrix,
all 45 feature-gated Rust contracts, all 83 local all-feature tests, strict
Clippy, rustfmt, and all configured native/WASI feature-matrix lanes passed. The
clean warm-2 `fixture-benchmark@3` observation at source checkout
`4c76598e9bb71133e626f42bfb94bcf1544bfa84` passed the Pillow-parity workload in
0.929043 s wall / 2.791228 user s / 0.169988 sys s /
251,412,480-byte peak RSS, and the separate Rust-only feature-gate workload in
1.569235 s wall / 2.246073 user s / 0.097882 sys s /
155,566,080-byte peak RSS. The native release `rlib` was 8,006,736 bytes and
the `wasm32-unknown-unknown` determinism artifact was 24,815,459 bytes. These
are host/cache/toolchain observations, not comparative or universal performance
claims; peak RSS is a direct-child POSIX observation. Pillow cannot observe
allocator ownership, so the existing Pillow fixture rows provide byte/error
regression only; backward-reference scratch ownership is Rust-only evidence. No
parity row, fixture-manifest row, diagnostic origin, new test function, or
coverage-only hook was added. Exact-head managed Pillow parity run
`cb42a1d3-2a09-4baa-a75e-3614bd30dfab` passed 1,445/1,445 checks, and exact-head
feature-matrix run `465b92d3-87b1-4c70-8cd8-e71cd8d16568` passed all configured
native/WASI lanes in 24,514 ms with the capability agreement marker and no
`lock-wait` match. Both managed runs have `coverage_ingest.status=not_configured`;
they are test-result evidence, not Coverage MCP metrics.

The lossless WebP VP8L histogram-clustering scratch-reuse slice is implemented
at production and Rust test/runtime revision
`36ce85ba244d7195baef8d5fea7adcdd3cbcc613`, following image-stream scratch reuse
at `87a42863ca46c2539aff75d18b85a669f7dac88b`. Token streams now retain one
`HistogramScratch` workspace per image stream: original tile histograms,
cluster copies, the symbol map, and remapped group histograms reset their
logical contents and are reused across sequential candidate streams, while
cache-dependent population lengths resize before each use. Nested metadata
streams retain their own workspace, so histogram state never crosses an active
stream boundary. Clustering order, encoded bytes, errors, and sink output remain
unchanged. The existing WebP encode matrix (28/13/47 rows), full fixture matrix,
all 45 feature-gated Rust contracts, all 83 local all-feature tests, strict
Clippy, rustfmt, and all configured native/WASI feature-matrix lanes passed.
The clean warm-2 `fixture-benchmark@3` observation at source checkout
`36ce85ba244d7195baef8d5fea7adcdd3cbcc613` passed the Pillow-parity workload in
0.933144 s wall / 2.815010 user s / 0.180552 sys s /
248,283,136-byte peak RSS, and the separate Rust-only feature-gate workload in
1.598661 s wall / 2.270305 user s / 0.112728 sys s /
163,905,536-byte peak RSS. The native release `rlib` was 7,993,648 bytes and
the `wasm32-unknown-unknown` determinism artifact was 24,803,389 bytes. These
are host/cache/toolchain observations, not comparative or universal performance
claims; peak RSS is a direct-child POSIX observation. Pillow cannot observe
allocator ownership, so the existing Pillow fixture rows provide byte/error
regression only; histogram scratch ownership is Rust-only evidence. No parity
row, fixture-manifest row, diagnostic origin, new test function, or
coverage-only hook was added. Exact-head managed Pillow parity run
`ffc34aad-6216-4b75-8baf-f907746ca9da` passed 1,445/1,445 checks, and exact-head
feature-matrix run `42deab70-5f43-4cdd-a6d2-a54be3923c50` passed all configured
native/WASI lanes in 23,394 ms with the capability agreement marker and no
`lock-wait` match. Both managed runs have `coverage_ingest.status=not_configured`;
they are test-result evidence, not Coverage MCP metrics.

The lossless WebP VP8L image-stream scratch-reuse slice is implemented at
production and Rust test/runtime revision
`87a42863ca46c2539aff75d18b85a669f7dac88b`, following hash-chain result
storage at `8a5a1e5aef3fc44e7cb2a9d956e6395c4389d5a7`. Frame, palette, and alpha
substreams now share one bounded image-stream scratch object per encoder
invocation; trial-output and token-stream buffers retain capacity across
sequential streams, including nested metadata streams, while each stream resets
its logical contents before writing. Stream boundaries, candidate ordering,
encoded bytes, errors, and sink output remain unchanged. The existing WebP
encode matrix (28/13/47 rows), full fixture matrix, all 45 feature-gated Rust
contracts, all 83 local all-feature tests, strict Clippy, rustfmt, and all
configured native/WASI feature-matrix lanes passed. The clean warm-2
`fixture-benchmark@3` observation at source checkout
`87a42863ca46c2539aff75d18b85a669f7dac88b` passed the Pillow-parity workload in
0.926429 s wall / 2.784499 user s / 0.184165 sys s /
255,311,872-byte peak RSS, and the separate Rust-only feature-gate workload in
1.574040 s wall / 2.264394 user s / 0.094817 sys s /
170,229,760-byte peak RSS. The native release `rlib` was 7,995,656 bytes and
the `wasm32-unknown-unknown` determinism artifact was 24,832,078 bytes. These
are host/cache/toolchain observations, not comparative or universal performance
claims; peak RSS is a direct-child POSIX observation. Pillow remains the
byte/error oracle, while image-stream scratch ownership is Rust-only evidence:
no parity row, fixture-manifest row, diagnostic origin, new test function, or
coverage-only hook was added. Exact-head managed Pillow parity run
`57ca3c13-4763-4f32-a13c-d5513772742d` passed 1,445/1,445 checks, and exact-head
feature-matrix run `906235c8-99d0-488a-905a-2f6a7903e151` passed all configured
native/WASI lanes in 26,613 ms with the capability agreement marker and no
`lock-wait` match. Both managed runs have `coverage_ingest.status=not_configured`;
they are test-result evidence, not Coverage MCP metrics.

The lossless WebP VP8L hash-chain result-storage slice is implemented at
production and Rust test/runtime revision
`8a5a1e5aef3fc44e7cb2a9d956e6395c4389d5a7`, following Huffman traversal
fixed-stack storage at `26e39ed56ba25159bea3d35cd5cc8045ee3acd06`. Hash-chain
construction now uses the final distance/length result table as temporary
predecessor-link storage during descending best-match materialization. Each
link points to an earlier position, so finalized entries can be overwritten
without affecting later traversal; the result table, candidate ordering,
checkpoint behavior, encoded bytes, errors, and sink output remain unchanged.
The existing WebP encode matrix (28/13/47 rows), full fixture matrix, all 45
feature-gated Rust contracts, all 83 local all-feature tests, strict Clippy,
rustfmt, and all configured native/WASI feature-matrix lanes passed. The clean
warm-2 `fixture-benchmark@3` observation at source checkout
`8a5a1e5aef3fc44e7cb2a9d956e6395c4389d5a7` passed the Pillow-parity workload in
0.935276 s wall / 2.795164 user s / 0.192718 sys s /
256,262,144-byte peak RSS, and the separate Rust-only feature-gate workload in
1.784594 s wall / 2.307308 user s / 0.253276 sys s /
170,393,600-byte peak RSS. A companion warm-3 run measured 0.992945 s wall for
Pillow parity and 1.587283 s wall for the Rust-only workload. The native
release `rlib` was 7,997,000 bytes and the `wasm32-unknown-unknown`
determinism artifact was 24,833,297 bytes. These are host/cache/toolchain
observations, not comparative or universal performance claims; peak RSS is a
direct-child POSIX observation. Pillow remains the byte/error oracle, while
hash-chain storage ownership is Rust-only evidence: no parity row,
fixture-manifest row, diagnostic origin, new test function, or coverage-only
hook was added. Exact-head managed Pillow parity run
`051353b1-c718-46d8-8d44-550a6e9fc52a` passed 1,445/1,445 checks, and exact-head
feature-matrix run `23a172a8-648c-493a-b066-66a1e72ceaed` passed all configured
native/WASI lanes in 14,791 ms with the capability agreement marker and no
`lock-wait` match. Both managed runs have `coverage_ingest.status=not_configured`;
they are test-result evidence, not Coverage MCP metrics.

The lossless WebP VP8L Huffman traversal fixed-stack storage slice is implemented
at production and Rust test/runtime revision
`26e39ed56ba25159bea3d35cd5cc8045ee3acd06`, following box-chain storage reuse
at `da2b9489fc3ac1ffcf94de5f4a685705d80d8702`. Huffman tree depth traversal now
uses a bounded fixed stack sized for the largest VP8L alphabet instead of a
temporary heap vector per tree; tree shape, code lengths, checkpoint behavior,
encoded bytes, errors, and sink output remain unchanged. The existing WebP
encode matrix (28/13/47 rows), full fixture matrix, all 45 feature-gated Rust
contracts, all 83 local all-feature tests, strict Clippy, rustfmt, and all
configured native/WASI feature-matrix lanes passed. The clean warm
`fixture-benchmark@3` observation at source checkout
`26e39ed56ba25159bea3d35cd5cc8045ee3acd06` passed the Pillow-parity workload in
1.088299 s wall / 3.084727 user s / 0.286170 sys s /
284,033,024-byte peak RSS, and the separate Rust-only feature-gate workload in
1.613453 s wall / 2.308328 user s / 0.107688 sys s /
193,626,112-byte peak RSS. The native release `rlib` was 7,996,704 bytes and
the `wasm32-unknown-unknown` determinism artifact was 24,839,363 bytes. These
are host/cache/toolchain observations, not comparative or universal performance
claims; peak RSS is a direct-child POSIX observation. Pillow remains the
byte/error oracle, while fixed-stack ownership is Rust-only evidence: no parity
row, fixture-manifest row, diagnostic origin, new test function, or coverage-only
hook was added. Exact-head managed Pillow parity run
`9d57dbfd-8f64-4a43-911f-994fbad04fce` passed 1,445/1,445 checks, and exact-head
feature-matrix run `84746d1c-b3cc-4a10-a659-7dad38e728f4` passed all configured
native/WASI lanes in 30,514 ms with the capability agreement marker and no
`lock-wait` match. Both managed runs have `coverage_ingest.status=not_configured`;
they are test-result evidence, not Coverage MCP metrics.

The lossless WebP VP8L box-chain storage-reuse slice is implemented at
production and Rust test/runtime revision
`da2b9489fc3ac1ffcf94de5f4a685705d80d8702`, following candidate-source token
scratch reuse at `3c6638abe1e32d33f4cfa8fc00d4fbba3bef4a32`. The optional
low-distance box-chain pass now repopulates the existing primary hash-chain
storage in place after the primary candidate has been consumed, avoiding a
second pixel-sized `(distance, length)` result vector. The existing WebP encode
matrix (28/13/47 rows), full fixture matrix, all 45 feature-gated Rust
contracts, full all-feature suite, strict Clippy, rustfmt, and all configured
native/WASI feature-matrix lanes passed locally. The clean warm
`fixture-benchmark@3` observation at source checkout
`da2b9489fc3ac1ffcf94de5f4a685705d80d8702` passed the Pillow-parity workload in
0.929563 s wall / 2.758877 user s / 0.163403 sys s /
251,461,632-byte peak RSS, and the separate Rust-only feature-gate workload in
1.578949 s wall / 2.245215 user s / 0.093334 sys s /
175,734,784-byte peak RSS. The native release `rlib` was 7,989,624 bytes and
the `wasm32-unknown-unknown` determinism artifact was 24,850,890 bytes. These
are host/cache/toolchain observations, not comparative or universal performance
claims; peak RSS is a direct-child POSIX observation. Pillow remains the
byte/error oracle, while box-chain storage ownership is Rust-only evidence: no
parity row, fixture-manifest row, diagnostic origin, new test function, or
coverage-only hook was added. Exact-head managed Pillow parity run
`5e03a0c6-ae7b-49dc-8f06-aab4b6545ec8` passed 1,445/1,445 checks, and exact-head
feature-matrix run `0eae8aea-3241-4fbc-9293-80e07d6ed1fd` passed all configured
native/WASI lanes with the capability agreement marker and no `lock-wait` match.
Both managed runs have `coverage_ingest.status=not_configured`; they are
test-result evidence, not Coverage MCP metrics.

The lossless WebP VP8L candidate-source token scratch-reuse slice is implemented
at production and Rust test/runtime revision
`3c6638abe1e32d33f4cfa8fc00d4fbba3bef4a32`, following trace CostModel histogram
reuse at `98f1e5e8b154cab176e227e41f7b0bde83d52f7b`. Candidate construction now
reuses one source-token buffer across the sequential LZ77, RLE, and optional
low-distance box-chain candidates. Cache-bit selection still reads each source
independently, and selected candidate vectors remain independently owned. The
existing WebP encode matrix (28/13/47 rows), full fixture matrix, all 45
feature-gated Rust contracts, full all-feature suite, strict Clippy, rustfmt,
and all configured native/WASI feature-matrix lanes passed locally. The clean
warm `fixture-benchmark@3` observation at source checkout
`3c6638abe1e32d33f4cfa8fc00d4fbba3bef4a32` passed the Pillow-parity workload in
1.071854 s wall / 3.069491 user s / 0.283397 sys s /
292,110,336-byte peak RSS, and the separate Rust-only feature-gate workload in
1.694006 s wall / 2.401989 user s / 0.159787 sys s /
206,192,640-byte peak RSS. The native release `rlib` was 7,990,432 bytes and
the `wasm32-unknown-unknown` determinism artifact was 24,854,581 bytes. These
are host/cache/toolchain observations, not comparative or universal performance
claims; peak RSS is a direct-child POSIX observation. Pillow remains the
byte/error oracle, while source-token buffer ownership is Rust-only evidence:
no parity row, fixture-manifest row, diagnostic origin, new test function, or
coverage-only hook was added. Exact-head managed Pillow parity run
`23c83e83-43a6-4b9f-9527-a3dfbb599d9a` passed 1,445/1,445 checks, and exact-head
feature-matrix run `e447924d-e4b3-43a0-8fb8-022278e16a44` passed all configured
native/WASI lanes with the capability agreement marker and no `lock-wait` match.
Both managed runs have `coverage_ingest.status=not_configured`; they are
test-result evidence, not Coverage MCP metrics.

The lossless WebP VP8L trace CostModel histogram-reuse slice is implemented at
production and Rust test/runtime revision
`98f1e5e8b154cab176e227e41f7b0bde83d52f7b`, following trace CostManager
buffer reuse at `fc56627eb07deb931da462c077ec81dab9c6e702`. Trace cost-model
construction now retains and resets the green histogram and fixed
channel/distance histograms across sequential trace attempts. The population-
cost transformation still runs in place with the same token-aware checkpoints,
and the no-token path remains direct. The existing WebP encode matrix (28/13/47
rows), full fixture matrix, all 45 feature-gated Rust contracts, full
all-feature suite, strict Clippy, rustfmt, and all configured native/WASI
feature-matrix lanes passed locally. The clean warm `fixture-benchmark@3`
observation at source checkout `98f1e5e8b154cab176e227e41f7b0bde83d52f7b`
passed the Pillow-parity workload in 0.930263 s wall / 2.780541 user s /
0.183454 sys s / 238,485,504-byte peak RSS, and the separate Rust-only
feature-gate workload in 1.605898 s wall / 2.267140 user s /
0.116672 sys s / 174,587,904-byte peak RSS. The native release `rlib` was
7,997,768 bytes and the `wasm32-unknown-unknown` determinism artifact was
24,860,447 bytes. These are host/cache/toolchain observations, not comparative
or universal performance claims; peak RSS is a direct-child POSIX observation.
Pillow remains the byte/error oracle, while CostModel histogram ownership is
Rust-only evidence: no parity row, fixture-manifest row, diagnostic origin, new
test function, or coverage-only hook was added. Exact-head managed Pillow parity
run `b458bb94-f780-4df6-9782-c45134425418` passed 1,445/1,445 checks, and
exact-head feature-matrix run `2e8db749-bdf5-4020-b420-869feee0c76f` passed all
configured native/WASI lanes with the capability agreement marker and no
`lock-wait` match. Both managed runs have `coverage_ingest.status=not_configured`;
they are test-result evidence, not Coverage MCP metrics.

The lossless WebP VP8L trace CostManager buffer-reuse slice is implemented at
production and Rust test/runtime revision
`fc56627eb07deb931da462c077ec81dab9c6e702`, following trace path/output
scratch reuse at `a7538a957a04efed5950b7ea16ff98b42ebff7da`. Trace setup now
retains the CostManager pixel-cost and path-length tables, match-length cost and
equal-cost interval tables, active interval state, and interval split/rebuild
scratch across sequential trace attempts. Each attempt resets candidate-specific
values and preserves the token-aware initialization checkpoints; the no-token
path remains tight. The existing WebP encode matrix (28/13/47 rows), full
fixture matrix, all 45 feature-gated Rust contracts, full all-feature suite,
strict Clippy, rustfmt, and all configured native/WASI feature-matrix lanes
passed locally. The clean `fixture-benchmark@3` warm observation at final
checkout `f83435351aadf13e0b320dd7a42f830d52c84895` passed the Pillow-parity
workload in 1.153682 s wall / 3.423246 user s / 0.247221 sys s /
293,060,608-byte peak RSS, and the separate Rust-only feature-gate workload in
1.782899 s wall / 2.567871 user s / 0.150656 sys s /
231,702,528-byte peak RSS. The native release `rlib` was 7,996,744 bytes and
the `wasm32-unknown-unknown` determinism artifact was 24,857,623 bytes. These
are host/cache/toolchain observations, not comparative or universal performance
claims; peak RSS is a direct-child POSIX observation.
Pillow remains the byte/error oracle, while CostManager scratch ownership is
Rust-only evidence: no parity row, fixture-manifest row, diagnostic origin, new
test function, or coverage-only hook was added. Exact-head managed Pillow parity
run `5db3e841-8bc3-4288-8e5c-ab2160394d33` passed 1,445/1,445 checks, and
exact-head feature-matrix run `a130342c-215b-4493-b53b-11d93a8ee540` passed all
configured native/WASI lanes with the capability agreement marker and no
`lock-wait` match. Both managed runs have `coverage_ingest.status=not_configured`;
they are test-result evidence, not Coverage MCP metrics.

The lossless WebP VP8L trace path/output scratch-reuse slice is implemented at
production and Rust test/runtime revision
`a7538a957a04efed5950b7ea16ff98b42ebff7da`, following cache-transform
output-scratch reuse at `d7a43f6314b2570baefbc048d0ef532395154f3e`. Trace-back
candidate improvement now retains the dynamic-programming cache, path-length
reconstruction buffer, and transformed-token output buffer across sequential
trace attempts. A selected trace keeps its token vector independently owned;
a rejected trace or replaced candidate returns its vector to scratch. Trace
ordering, checkpoint behavior, encoded bytes, errors, and sink output remain
unchanged. The existing WebP encode matrix (28/13/47 rows), full fixture
matrix, all 45 feature-gated Rust contracts, full all-feature suite, strict
Clippy, rustfmt, and all configured native/WASI feature-matrix lanes passed
locally. Clean `fixture-benchmark@3` observations at this revision passed the
Pillow-parity workload in 1.040943 s wall / 2.796161 user s / 0.205729 sys s /
250,462,208-byte peak RSS, and the separate Rust-only feature-gate workload in
1.591423 s wall / 2.260406 user s / 0.113248 sys s /
165,330,944-byte peak RSS. The native release `rlib` was 7,983,152 bytes and
the `wasm32-unknown-unknown` determinism artifact was 24,866,097 bytes. These
are host/cache/toolchain observations, not comparative or universal
performance claims; peak RSS is a direct-child POSIX observation. Pillow
remains the byte/error oracle, while trace-scratch ownership is Rust-only
evidence: no parity row, fixture-manifest row, diagnostic origin, new test
function, or coverage-only hook was added. Exact-head managed Pillow parity run
`73efdb2e-d5e5-45e3-92df-4211dad892f3` passed 1,445/1,445 checks, and exact-head
feature-matrix run `4ba0d011-cd39-427b-8368-f7db6477131a` passed all configured
native/WASI lanes with the capability agreement marker and no `lock-wait`
match. Both managed runs have `coverage_ingest.status=not_configured`; they are
test-result evidence, not Coverage MCP metrics.

The lossless WebP VP8L cache-transform output-scratch reuse slice is
implemented at production and Rust test/runtime revision
`d7a43f6314b2570baefbc048d0ef532395154f3e`, following nested metadata output
scratch reuse at `2e272b2405ec108fb2b531df07665f0e81c2f1f8`. Cache-bit candidate
transforms now retain a reusable transformed-token buffer alongside the
existing color-cache table; each trial swaps its output with the current best
candidate, returning the replaced vector to scratch while keeping only the
selected token vector independently owned. Cache-bit ordering, checkpoint
behavior, encoded bytes, errors, and sink output remain unchanged. The existing
WebP encode matrix (28/13/47 rows), full fixture matrix, all 45 feature-gated
Rust contracts, full all-feature suite, strict Clippy, rustfmt, and all
configured native/WASI feature-matrix lanes passed locally. Clean
`fixture-benchmark@3` observations at this revision passed the Pillow-parity
workload in 0.948031 s wall / 2.776938 user s / 0.193214 sys s /
250,478,592-byte peak RSS, and the separate Rust-only feature-gate workload in
1.590595 s wall / 2.264828 user s / 0.096149 sys s /
150,044,672-byte peak RSS. The native release `rlib` was 7,979,320 bytes and
the `wasm32-unknown-unknown` determinism artifact was 24,848,101 bytes. These
are host/cache/toolchain observations, not comparative or universal
performance claims; peak RSS is a direct-child POSIX observation. Pillow
remains the byte/error oracle, while transformed-token ownership is Rust-only
evidence: no parity row, fixture-manifest row, diagnostic origin, new test
function, or coverage-only hook was added. Exact-head managed Pillow parity run
`74d6b9ee-d771-4ad3-99c4-27a17c9512f7` passed 1,445/1,445 checks, and exact-head
feature-matrix run `64fb001e-366c-43d9-8dc4-7ac507e945ce` passed all configured
native/WASI lanes with the capability agreement marker and no `lock-wait`
match. Both managed runs have `coverage_ingest.status=not_configured`; they are
test-result evidence, not Coverage MCP metrics.

The lossless WebP VP8L nested metadata output-scratch reuse slice is
implemented at production and Rust test/runtime revision
`2e272b2405ec108fb2b531df07665f0e81c2f1f8`, following nested metadata-stream
scratch reuse at `e9aabbc0cc1f4cd208f1b63be74b065809d1f5d7`. The configured
image-stream helper now carries a separate output scratch buffer; nested
metadata candidate trials reuse losing suffix storage and return the winning
suffix capacity to that buffer after delivery. Candidate selection, checkpoint
ordering, encoded bytes, errors, and sink output remain unchanged. The existing
WebP encode matrix (28/13/47 rows), full fixture matrix, all 45 feature-gated
Rust contracts, full all-feature suite, strict Clippy, rustfmt, and all
configured native/WASI feature-matrix lanes passed locally. Clean
`fixture-benchmark@3` observations at this revision passed the Pillow-parity
workload in 1.051700 s wall / 2.796538 user s / 0.217463 sys s /
246,251,520-byte peak RSS, and the separate Rust-only feature-gate workload in
1.809399 s wall / 2.292459 user s / 0.280494 sys s /
176,734,208-byte peak RSS. The native release `rlib` was 7,977,792 bytes and
the `wasm32-unknown-unknown` determinism artifact was 24,845,082 bytes. These
are host/cache/toolchain observations, not comparative or universal
performance claims; peak RSS is a direct-child POSIX observation. Pillow
remains the byte/error oracle, while output-scratch ownership is Rust-only
evidence: no parity row, fixture-manifest row, diagnostic origin, new test
function, or coverage-only hook was added. Exact-head managed Pillow parity run
`dedb5e2e-8124-4aa2-8e9c-c7db7e998db8` passed 1,445/1,445 checks, and exact-head
feature-matrix run `db97b171-6f32-477c-84c1-335a1f323098` passed all configured
native/WASI lanes with the capability agreement marker and no `lock-wait`
match. Both managed runs have `coverage_ingest.status=not_configured`; they are
test-result evidence, not Coverage MCP metrics.

The lossless WebP VP8L nested metadata-stream scratch-reuse slice is
implemented at production and Rust test/runtime revision
`e9aabbc0cc1f4cd208f1b63be74b065809d1f5d7`, following Huffman node/merge
scratch reuse at `a15a4c5840d51cd7bc451846ee0ff9d4aad144f7`. The configured
image-stream helper now accepts retained token-stream scratch; multi-group
token streams keep an optional boxed child scratch for the sampled metadata
image across outer candidate trials, while the metadata stream disables further
recursion. Candidate suffix ownership, parent-writer prefix state, checkpoint
ordering, encoded bytes, errors, and sink output remain unchanged. The existing
WebP encode matrix (28/13/47 rows), full fixture matrix, all 45 feature-gated
Rust contracts, full all-feature suite, strict Clippy, rustfmt, and all
configured native/WASI feature-matrix lanes passed locally. Clean
`fixture-benchmark@3` observations at this revision passed the Pillow-parity
workload in 0.948142 s wall / 2.796201 user s / 0.193921 sys s /
246,923,264-byte peak RSS, and the separate Rust-only feature-gate workload in
1.608785 s wall / 2.269095 user s / 0.131215 sys s /
147,193,856-byte peak RSS. The native release `rlib` was 7,978,808 bytes and
the `wasm32-unknown-unknown` determinism artifact was 24,842,828 bytes. These
are host/cache/toolchain observations, not comparative or universal
performance claims; peak RSS is a direct-child POSIX observation. Pillow
remains the byte/error oracle, while nested-scratch ownership is Rust-only
evidence: no parity row, fixture-manifest row, diagnostic origin, new test
function, or coverage-only hook was added. Exact-head managed Pillow parity run
`69d78c49-273a-4ddf-9dc3-29129e71a3cd` passed 1,445/1,445 checks, and exact-head
feature-matrix run `72f7f069-aaf1-4ff5-9d12-ede748ac7085` passed all configured
native/WASI lanes with the capability agreement marker and no `lock-wait`
match. Both managed runs have `coverage_ingest.status=not_configured`; they are
test-result evidence, not Coverage MCP metrics.

The lossless WebP VP8L Huffman node/merge scratch-reuse slice is implemented
at production and Rust test/runtime revision
`a15a4c5840d51cd7bc451846ee0ff9d4aad144f7`, following meta-pixel scratch reuse
at `6e243f7e92becc664cf3d17e68fcecf25a873863`. Huffman construction retains the
leaf-node vector and token-aware merge-sort buffer across sequential tree
builds; recursive boxed nodes remain per-tree owned and the traversal stack
remains local. Ordering, checkpoint behavior, tree selection, encoded bytes,
errors, and sink output remain unchanged. The existing WebP encode matrix
(28/13/47 rows), full fixture matrix, all 45 feature-gated Rust contracts, full
all-feature suite, strict Clippy, rustfmt, and all configured native/WASI
feature-matrix lanes passed locally. Clean `fixture-benchmark@3` observations
at this revision passed the Pillow-parity workload in 1.727992 s wall /
3.779852 user s / 0.347120 sys s / 288,571,392-byte peak RSS, and the separate
Rust-only feature-gate workload in 2.832432 s wall / 3.168662 user s /
0.185820 sys s / 238,272,512-byte peak RSS. The native release `rlib` was
7,981,064 bytes and the `wasm32-unknown-unknown` determinism artifact was
24,831,896 bytes. These are host/cache/toolchain observations, not comparative
or universal performance claims; peak RSS is a direct-child POSIX observation.
Pillow remains the byte/error oracle, while node/merge vector ownership is
Rust-only evidence: no parity row, fixture-manifest row, diagnostic origin,
new test function, or coverage-only hook was added. Exact-head managed Pillow
parity run `9ebf2cee-a617-46b9-a7a1-3173cb959602` passed 1,445/1,445 checks,
and exact-head feature-matrix run `15175037-4f06-4e3e-b463-17499189d740` passed
all configured native/WASI lanes with the capability agreement marker and no
`lock-wait` match. Both managed runs have
`coverage_ingest.status=not_configured`; they are test-result evidence, not
Coverage MCP metrics.

The lossless WebP VP8L meta-pixel scratch-reuse slice is implemented at
production and Rust test/runtime revision
`6e243f7e92becc664cf3d17e68fcecf25a873863`, following Huffman-RLE mask scratch
reuse at `058e6b7dc89dd59b96f3d06343d9e296af7006b0`. Multi-group token streams
now clear and refill one meta-pixel materialization buffer across candidate
trials; the recursive meta-stream write consumes it before the next candidate,
so metadata grouping, checkpoint behavior, encoded bytes, errors, and sink
output remain unchanged. The existing WebP encode matrix (28/13/47 rows), full
fixture matrix, all 45 feature-gated Rust contracts, full all-feature suite,
strict Clippy, rustfmt, and all configured native/WASI feature-matrix lanes
passed locally. Clean `fixture-benchmark@3` observations at this revision
passed the Pillow-parity workload in 0.942339 s wall / 2.799787 user s /
0.184515 sys s / 253,739,008-byte peak RSS, and the separate Rust-only
feature-gate workload in 1.740961 s wall / 2.320372 user s / 0.212541 sys s /
169,181,184-byte peak RSS. The native release `rlib` was 7,973,968 bytes and
the `wasm32-unknown-unknown` determinism artifact was 24,793,428 bytes. These
are host/cache/toolchain observations, not comparative or universal
performance claims; peak RSS is a direct-child POSIX observation. Pillow
remains the byte/error oracle, while meta-pixel buffer ownership is Rust-only
evidence: no parity row, fixture-manifest row, diagnostic origin, new test
function, or coverage-only hook was added. Exact-head managed Pillow parity run
`9e87d06e-9bba-4e14-9b4c-e4378ea3d492` passed 1,445/1,445 checks, and exact-head
feature-matrix run `a40d1a78-d1ae-437d-bd56-db097b81f0f4` passed all configured
native/WASI lanes with the capability agreement marker and no `lock-wait`
match. Both managed runs have `coverage_ingest.status=not_configured`; they are
test-result evidence, not Coverage MCP metrics.

The lossless WebP VP8L Huffman-RLE mask-scratch reuse slice is implemented at
production and Rust test/runtime revision
`058e6b7dc89dd59b96f3d06343d9e296af7006b0`, following Huffman symbol-array
reuse at `401e9ab847eb717dd29515ccd5c8c8fe1f9cb621`. Huffman-RLE preparation
now resizes and clears one boolean good-mask buffer across sequential channel
and histogram-group tree builds; token-aware and no-token RLE decisions,
checkpoint behavior, tree selection, encoded bytes, errors, and sink output
remain unchanged. The existing WebP encode matrix (28/13/47 rows), full
fixture matrix, all 45 feature-gated Rust contracts, full all-feature suite,
strict Clippy, rustfmt, and all configured native/WASI feature-matrix lanes
passed locally. Clean `fixture-benchmark@3` observations at this revision
passed the Pillow-parity workload in 0.940543 s wall / 2.819080 user s /
0.183555 sys s / 246,939,648-byte peak RSS, and the separate Rust-only
feature-gate workload in 1.594874 s wall / 2.266260 user s / 0.104678 sys s /
178,700,288-byte peak RSS. The native release `rlib` was 7,967,040 bytes and
the `wasm32-unknown-unknown` determinism artifact was 24,800,227 bytes. These
are host/cache/toolchain observations, not comparative or universal
performance claims; peak RSS is a direct-child POSIX observation. Pillow
remains the byte/error oracle, while mask ownership is Rust-only evidence:
no parity row, fixture-manifest row, diagnostic origin, new test function, or
coverage-only hook was added. Exact-head managed Pillow parity run
`36fac1e0-ecb9-45d3-ab03-dd4cf3ad1a1f` passed 1,445/1,445 checks, and exact-head
feature-matrix run `2bfd3ae0-7b9d-49be-95fd-d74f3cd6cd86` passed all configured
native/WASI lanes with the capability agreement marker and no `lock-wait`
match. Both managed runs have `coverage_ingest.status=not_configured`; they are
test-result evidence, not Coverage MCP metrics.

The lossless WebP VP8L Huffman symbol-array reuse slice is implemented at
production and Rust test/runtime revision
`401e9ab847eb717dd29515ccd5c8c8fe1f9cb621`, following optimized-frequency
scratch reuse at `6042b77c5c568968295bae030335cb6d9cabb417`. Simple-tree symbol
discovery stores at most three indices in a fixed array instead of a heap
vector; token-aware and no-token scans preserve their early-stop behavior and
checkpoint schedule. Tree selection, encoded bytes, errors, and sink output
remain unchanged. The existing WebP encode matrix (28/13/47 rows), full
fixture matrix, all 45 feature-gated Rust contracts, full all-feature suite,
strict Clippy, rustfmt, and all 33 native/WASI feature-matrix lanes passed
locally. Clean `fixture-benchmark@3` observations at this revision passed the
Pillow-parity workload (0.931031 s wall, 248,987,648-byte peak RSS) and the
separate Rust-only feature-gate workload (1.580142 s wall, 150,962,176-byte
peak RSS); these are host/cache/toolchain observations, not a comparative or
universal performance claim. Pillow remains the byte/error oracle, while the
fixed symbol storage is Rust-only evidence: no parity row, fixture-manifest
row, diagnostic origin, new test function, or coverage-only hook was added.
Managed checkout validation passed Pillow parity run
`5bd106cf-fe10-421b-834e-9897d855cf83` and feature-matrix run
`8b18d725-8b61-423b-9417-8ae43b6c3aec`, but both recorded pre-commit HEAD
`458865de920e80f81b6ac7cc89ef1c6806ab94d2`; no exact-head managed parity,
feature-matrix, or Coverage MCP coverage rerun is claimed for this revision.

The preceding lossless WebP VP8L optimized-frequency scratch-reuse slice is
implemented at production and Rust test/runtime revision
`6042b77c5c568968295bae030335cb6d9cabb417`, following Huffman-token scratch
reuse at `b770e3c4238194fa0c65f1490c20d0e8e14380d2`. Huffman tree construction
reuses one optimized-frequency buffer across sequential channel and
histogram-group trees, copying each frequency slice into retained storage
before the existing RLE optimization. Ordinary and token-aware tree
construction, checkpoint sites, encoded bytes, errors, and sink output remain
unchanged; the no-token path remains free of optional polling. The existing
WebP encode matrix (28/13/47 rows), full fixture matrix, all 45 feature-gated
Rust contracts, full all-feature suite, strict Clippy, rustfmt, and all 33
native/WASI feature-matrix lanes passed locally. Clean `fixture-benchmark@3`
observations at this revision passed the Pillow-parity workload (0.950112 s
wall, 252,411,904-byte peak RSS) and the separate Rust-only feature-gate
workload (1.687539 s wall, 199,180,288-byte peak RSS); these are
host/cache/toolchain observations, not a comparative or universal performance
claim. Pillow remains the byte/error oracle, while optimized-frequency buffer
ownership is Rust-only evidence: no parity row, fixture-manifest row,
diagnostic origin, new test function, or coverage-only hook was added. Managed
checkout validation passed Pillow parity run
`46e13ce3-09ac-411e-b5ba-4b0dd186b123` and feature-matrix run
`673e9ccf-c495-460b-a4a1-31aad144de0a`, but both recorded pre-commit HEAD
`2cfcdb49814321d0d251cc40072d2ba0bf583d15`; no exact-head managed parity,
feature-matrix, or Coverage MCP coverage rerun is claimed for this revision.

The preceding lossless WebP VP8L Huffman-token scratch-reuse slice is implemented at
production and Rust test/runtime revision
`b770e3c4238194fa0c65f1490c20d0e8e14380d2`, following `GroupCodes` buffer reuse
at `cc00fe4f4e67e40bb9570dedac8d4b185745202f`. Huffman tree writing reuses one
compressed code-length token buffer across sequential channel and
histogram-group trees; each tree consumes it before the next tree clears and
refills it. Ordinary and token-aware tree construction, checkpoint sites,
encoded bytes, errors, and sink output remain unchanged. The existing WebP
encode matrix (28/13/47 rows), full fixture matrix, all 45 feature-gated Rust
contracts, full all-feature suite, strict Clippy, rustfmt, and all 33 native/WASI
feature-matrix lanes passed locally. Clean `fixture-benchmark@3` observations
at this revision passed the Pillow-parity workload (0.940096 s wall,
257,966,080-byte peak RSS) and the separate Rust-only feature-gate workload
(1.610291 s wall, 177,471,488-byte peak RSS); these are host/cache/toolchain
observations, not a comparative or universal performance claim. Pillow remains
the byte/error oracle, while token-buffer ownership is Rust-only evidence: no
parity row, fixture-manifest row, diagnostic origin, new test function, or
coverage-only hook was added. Managed checkout validation passed Pillow parity
run `299bfbff-2784-4b62-8d7a-e2974d6082b2` and feature-matrix run
`074d3e14-4899-4b6c-9e2f-f4c3f4b00318`, but both recorded pre-commit HEAD
`c48b3bcbf49937314b5f23e93b4326c64a3d3105`; no exact-head managed parity,
feature-matrix, or Coverage MCP coverage rerun is claimed for this revision.

The preceding lossless WebP VP8L `GroupCodes` buffer-reuse slice is implemented at
production and Rust test/runtime revision
`cc00fe4f4e67e40bb9570dedac8d4b185745202f`, following trace-cache reuse at
`5d386f0e8d0c4f8780cc59cf3080f9107c0d66c2`. Candidate trials retain their
per-group Huffman length/code arrays in bounded scratch, resize and reset them
in place, and keep each group live through token-reference emission. Ordinary
and token-aware group construction, checkpoint sites, encoded bytes, errors,
and sink output remain unchanged. The existing WebP encode matrix (28/13/47
rows), full fixture matrix, all 45 feature-gated Rust contracts, full
all-feature suite, strict Clippy, rustfmt, and all 33 native/WASI
feature-matrix lanes passed locally. Clean `fixture-benchmark@3` observations
at this revision passed the Pillow-parity workload (0.946648 s wall,
252,985,344-byte peak RSS) and the separate Rust-only feature-gate workload
(1.726289 s wall, 209,485,824-byte peak RSS); these are host/cache/toolchain
observations, not a comparative or universal performance claim. Pillow remains
the byte/error oracle, while GroupCodes ownership is Rust-only evidence: no
parity row, fixture-manifest row, diagnostic origin, new test function, or
coverage-only hook was added. Managed checkout validation passed Pillow parity
run `3c240d18-2cf7-4a21-9918-d7b87e877766` and feature-matrix run
`0dae6dcc-fef9-4d12-a583-ab1e45228243`, but both recorded pre-commit HEAD
`3a31d87be7a8a0d03ef0179db0cea2805a413a4b`; no exact-head managed parity,
feature-matrix, or Coverage MCP coverage rerun is claimed for this revision.

The lossless WebP VP8L trace-cache reuse slice is implemented at production and
Rust test/runtime revision
`5d386f0e8d0c4f8780cc59cf3080f9107c0d66c2`, following cache-transform scratch
reuse at `ecc5ac4c95a608f3c709fb0de98a89c3f131df59`. The dynamic-programming
color cache is reset and reused for token replay after path reconstruction,
removing the second cache-table allocation while leaving the winning token
output independently owned. Ordinary and token-aware trace ordering,
checkpoint sites, encoded bytes, errors, and sink output remain unchanged. The
existing WebP encode matrix (28/13/47 rows), full fixture matrix, all 45
feature-gated Rust contracts, full all-feature suite, strict Clippy, rustfmt,
and all 33 native/WASI feature-matrix lanes passed locally. Clean
`fixture-benchmark@3` observations at this revision passed the Pillow-parity
workload (0.934377 s wall) and the separate Rust-only feature-gate workload
(1.682465 s wall); these are host/cache/toolchain observations, not a
comparative or universal performance claim. Pillow remains the byte/error
oracle, while cache ownership is Rust-only evidence: no parity row,
fixture-manifest row, diagnostic origin, new test function, or coverage-only
hook was added. No managed parity, feature-matrix, or Coverage MCP rerun is
claimed at this revision.

The lossless WebP VP8L cache-transform scratch slice is implemented at
production and Rust test/runtime revision
`ecc5ac4c95a608f3c709fb0de98a89c3f131df59`, following candidate-estimate
scratch reuse at `56efb2215f9f37d412368f43109cd9ebab3bd87e`. Sequential
cache-bit candidate trials now reuse a bounded zeroed cache table through
`CacheTransformScratch`; its capacity grows only when a larger cache-bit trial
requires it, and each trial clears the existing storage instead of allocating a
new table. Candidate token vectors remain independently owned because the
winning trial must survive. Ordinary and token-aware cache transformation,
checkpoint sites, cost decisions, encoded bytes, errors, and sink output remain
unchanged. The existing WebP encode matrix (28/13/47 rows), full fixture
matrix, all 45 feature-gated Rust contracts, full all-feature suite, strict
Clippy, rustfmt, and all 33 native/WASI feature-matrix lanes passed locally.
Clean `fixture-benchmark@3` observations at this revision passed the
Pillow-parity workload (1.034803 s wall) and the separate Rust-only feature-gate
workload (1.606970 s wall); these are host/cache/toolchain observations, not a
comparative or universal performance claim. Pillow remains the byte/error
oracle, while this scratch ownership is Rust-only evidence: no parity row,
fixture-manifest row, diagnostic origin, new test function, or coverage-only
hook was added. No managed parity, feature-matrix, or Coverage MCP rerun is
claimed at this revision.

The lossless WebP VP8L candidate-estimate scratch slice is implemented at
production and Rust test/runtime revision
`56efb2215f9f37d412368f43109cd9ebab3bd87e`, following the CostModel
population-buffer reuse at `b9aff15d42432e01f1120f1b7fd9f731ed86101e`.
Sequential cache-bit candidate trials now reuse a bounded green histogram
vector through `CostEstimateScratch`; its capacity grows only when a larger
cache-bit estimate requires it, and each trial clears the existing storage
instead of allocating a new green vector. Ordinary and token-aware estimator
ordering, checkpoint sites, cost decisions, encoded bytes, errors, and sink
output remain unchanged. The existing WebP encode matrix (28/13/47 rows), full
fixture matrix, all 45 feature-gated Rust contracts, full all-feature suite,
strict Clippy, rustfmt, and all 33 native/WASI feature-matrix lanes passed
locally. Clean `fixture-benchmark@3` observations at this revision passed the
Pillow-parity workload (1.218692 s wall) and the separate Rust-only feature-gate
workload (2.966705 s wall); these are host/cache/toolchain observations, not a
comparative or universal performance claim. Pillow remains the byte/error
oracle, while this scratch ownership is Rust-only evidence: no parity row,
fixture-manifest row, diagnostic origin, new test function, or coverage-only
hook was added. No managed parity, feature-matrix, or Coverage MCP rerun is
claimed at this revision.

The lossless WebP VP8L CostModel population-buffer reuse slice is implemented
at production and Rust test/runtime revision
`b9aff15d42432e01f1120f1b7fd9f731ed86101e`, following the interval-split
scratch reuse at `e9ee33d589f76f7f4c392d4ae29811db3a7e203f`. The fixed-alphabet
population histograms now transform their existing vectors in place instead of
allocating temporary `Vec` values for the 256- and 40-symbol cost arrays before
conversion. Ordinary and token-aware cost-model ordering, checkpoint sites,
cost decisions, encoded bytes, errors, and sink output remain unchanged. The
existing WebP encode matrix (28/13/47 rows), full fixture matrix, all 45
feature-gated Rust contracts, full all-feature suite, strict Clippy, rustfmt,
and all 33 native/WASI feature-matrix lanes passed locally. Clean
`fixture-benchmark@3` observations at this revision passed both the
Pillow-parity workload (1.215618 s wall) and the separate Rust-only
feature-gate workload (3.054082 s wall); these are host/cache/toolchain
observations, not a comparative or universal performance claim. Pillow remains
the byte/error oracle, while this buffer ownership is Rust-only evidence: no
parity row, fixture-manifest row, diagnostic origin, new test function, or
coverage-only hook was added. No managed parity, feature-matrix, or Coverage
MCP rerun is claimed at this revision.

The lossless WebP VP8L CostManager interval-split scratch slice is implemented
at production and Rust test/runtime revision
`e9ee33d589f76f7f4c392d4ae29811db3a7e203f`, following the interval-state reuse
at `f974c84d8f04114d24a3914a3517b601645ac4b5`. Boundary, addition, overlap,
rebuild, and merge vectors are retained as bounded manager scratch instead of
being allocated for each split/rebuild call. Ordinary and token-aware interval
ordering, checkpoint sites, cost decisions, encoded bytes, errors, and sink
output remain unchanged. The existing WebP encode matrix (28/13/47 rows), full
fixture matrix, all 45 feature-gated Rust contracts, full all-feature suite,
strict Clippy, rustfmt, and all 33 native/WASI feature-matrix lanes passed
locally. Pillow remains the byte/error oracle, while this internal scratch
ownership is Rust-only evidence: no parity row, fixture-manifest row,
diagnostic origin, new test function, or coverage-only hook was added. No
managed parity, feature-matrix, or Coverage MCP rerun is claimed at this
revision.

The lossless WebP VP8L CostManager interval-state allocation slice is
implemented at production and Rust test/runtime revision
`f974c84d8f04114d24a3914a3517b601645ac4b5`, following the preceding
`63dd8b7ebaa7d5c36699d5b9c3278ed32e9253ff` update-state change. Interval
updates no longer materialize a temporary applicable-interval vector, cleanup
compacts the existing interval vector in place, and push paths borrow the
immutable length-interval table instead of cloning it per call. Ordinary and
token-aware cost decisions, checkpoint ordering, encoded bytes, errors, and
sink output remain unchanged. The existing WebP encode matrix (28/13/47 rows),
complete 28-function fixture matrix, all 45 feature-gated Rust contracts, full
all-feature suite, strict Clippy, rustfmt, and all 33 native/WASI feature-matrix
lanes passed locally. Pillow remains the byte/error oracle, while these internal
allocation choices are Rust-only evidence: no parity row, fixture-manifest row,
diagnostic origin, new test function, or coverage-only hook was added. No
managed parity, feature-matrix, or Coverage MCP rerun is claimed at this
revision.

The TIFF multi-page sink page-base planning slice is implemented at production
and Rust test/runtime revision
`f13a5aa3b99fed752875f67c9a73c27b4f97a538`. Sink delivery now derives each
16-byte-aligned page base from the running delivered position while relocating
pages, removing the page-count-sized `Vec<usize>` without changing next-IFD
links, relocated offsets, alignment, overflow checks, encoded bytes, sink
segment boundaries, cancellation, or output-policy behavior. The existing 57
TIFF encode Pillow rows, complete 28-function fixture matrix, all 45
feature-gated Rust contracts, full all-feature suite, strict Clippy, rustfmt,
and all 33 native/WASI feature-matrix lanes passed locally. Pillow remains the
byte/error oracle, while this temporary bookkeeping choice is Rust-only
evidence: no parity row, fixture-manifest row, diagnostic origin, new test
function, or coverage-only hook was added. No managed parity, feature-matrix,
or Coverage MCP rerun is claimed at this revision.

The PNG all-level repeated-row Deflate allocation slice is implemented at
production and Rust test/runtime revision
`6e96b2c7f5587543b840bfde78ef0f2a239c1f3c`. PNG now passes the filtered-row
length and height directly to the stored-block and zlib-ng compressor paths for
compression levels 0 through 9 instead of allocating a duplicate row-length
vector. Ordinary and token-aware paths replay the same input-call boundaries,
matcher behavior, checkpoint cadence, compressed bytes, errors, and sink
output. The existing 83 PNG encode Pillow rows, complete 28-function fixture
matrix, all 45 feature-gated Rust contracts, full all-feature suite, strict
Clippy, rustfmt, and all 33 native/WASI feature-matrix lanes passed locally.
Pillow remains the byte/error oracle, while this allocation choice is Rust-only
evidence: no parity row, fixture-manifest row, diagnostic origin, new test
function, or coverage-only hook was added. No managed parity, feature-matrix,
or Coverage MCP rerun is claimed at this revision.

The preceding PNG level-six repeated-row Deflate allocation slice was implemented at
production and Rust test/runtime revision
`6b6ff5c4c1a4d5998ee4c6c9fe2ff438ed8d77df`. PNG’s default level-six path now
passes the filtered-row length and height directly to the zlib-ng tokenizer
instead of allocating a duplicate row-length vector; non-level-six paths keep
their existing input-chunk slice because their distinct compressor strategies
still consume that representation. The ordinary and token-aware level-six
paths replay the same input-row boundaries, matcher behavior, checkpoint
cadence, compressed bytes, errors, and sink output. The existing 83 PNG encode
Pillow rows, complete 28-function fixture matrix, all 45 feature-gated Rust
contracts, full all-feature suite, strict Clippy, rustfmt, and all 24
native/WASI feature-matrix lanes passed locally. Pillow remains the byte/error
oracle, while this allocation choice is Rust-only evidence: no parity row,
fixture-manifest row, diagnostic origin, new test function, or coverage-only
hook was added. No managed parity, feature-matrix, or Coverage MCP rerun is
claimed at this revision.

The TIFF repeated-row Deflate allocation slice is implemented at production
and Rust test/runtime revision
`4866fdb1d35a57a1c1f7edf4326bcebbcff0fe51`. TIFF pages now pass their row
length and height directly to the level-six zlib-ng path instead of allocating
a duplicate `Vec<usize>` containing the same row length once per row. The
specialized no-token and token-aware tokenizers replay the same row-boundary
calls, matcher behavior, checkpoint cadence, compressed bytes, error behavior,
and sink output; only the temporary row-length vector is removed. The existing
57 TIFF encode Pillow rows, complete 28-function fixture matrix, all 45
feature-gated Rust contracts, full all-feature suite, strict Clippy, rustfmt,
and all 24 native/WASI feature-matrix lanes passed locally. Pillow remains the
byte/error oracle, while allocation ownership is Rust-only evidence: no parity
row, fixture-manifest row, diagnostic origin, new test function, or
coverage-only hook was added. No managed parity, feature-matrix, or Coverage
MCP rerun is claimed at this revision.

The latest lossless WebP VP8L Huffman-RLE reverse-tail scan slice is implemented
at production and Rust test/runtime revision
`8b52b7180df0118ed9e427b5df01b906bbe32eaf` through the existing
`encode_work_budget_is_a_non_parity_result_contract`. Token-aware Huffman-RLE
preparation now scans the fixed code-length alphabet from its tail toward the
last nonzero slot and polls after each 64 scanned entries; the no-token path
retains the original tight `rposition` search. The deterministic 256×3 RGB
sparse-tail probe remains byte-identical under the ordinary and `u64::MAX`
policies, while the finite policy rejects both whole-buffer and caller-owned
sink paths at `maximum: 6,710`, `observed: 6,711`; the sink sentinel
`[0xB2]` remains untouched. This is Rust-only work-control and sink evidence:
Pillow has no caller token, work budget, or caller-owned sink, so no parity row,
fixture-manifest row, diagnostic origin, new test function, or coverage-only
hook was added. The focused and full all-feature suites, strict Clippy,
rustfmt, and all 24 native/WASI feature-matrix lanes passed locally. No managed
parity, feature-matrix, or Coverage MCP rerun is claimed at this revision.

The TIFF sequence length-planning slice is implemented at production revision
`59e4c4fa7c33e047fbb802d7722058e71a6263f1`. Still and sink sequence encoders
now derive the final aligned length directly from the already-owned encoded
pages instead of collecting a duplicate `Vec<usize>` of page lengths. Page
alignment, offset relocation, overflow behavior, encoded bytes, cancellation
checkpoints, and sink delivery are unchanged. All 57 TIFF encode parity rows,
the complete 28-function fixture matrix, all 45 feature-gated Rust contracts,
full all-feature tests, strict Clippy, and the native/WASM feature matrix
passed locally. Pillow remains the exact byte/error oracle; length bookkeeping
is a Rust implementation boundary with no Pillow allocation contract. No new
fixture, test function, diagnostic origin, or coverage-only hook was added. No
managed parity, feature-matrix, or Coverage MCP rerun is claimed for this
revision.

The GIF indexed frame-diff state slice is implemented at production revision
`84a18ee1be94fcc4de1064f92c53303ea3950bcc`. GIF output assembly now retains
the previous frame's palette and indices, compares palette entries directly,
and masks unchanged current indices without materializing a full RGB copy for
each frame. Frame coalescing, transparency decisions, encoded bytes, error
behavior, and cancellation checkpoints are unchanged. All 41 GIF encode parity
rows across 10 matrix functions, the complete 28-function fixture matrix, all
45 feature-gated Rust contracts, full all-feature tests, strict Clippy, and
the native/WASM feature matrix passed locally. Pillow remains the exact
byte/error oracle; retained indexed diff state is a Rust implementation
boundary with no Pillow allocation contract. No new fixture, test function,
diagnostic origin, or coverage-only hook was added. No managed parity,
feature-matrix, or Coverage MCP rerun is claimed for this revision.

The ICO BMP-payload ownership slice is implemented at production revision
`2347ae0ee31d9ab592d9eefbea8ed3f2e0b9b4b3`. BMP-backed ICO entries now append
converted BGR/BGRA rows directly into the pre-sized DIB output instead of
materializing a separate converted-pixel buffer and copying it into that
output. Row cancellation polls, DIB bytes, directory lengths, and sink output
are unchanged. All 20 ICO encode parity rows, the complete 28-function fixture
matrix, all 45 feature-gated Rust contracts, full all-feature tests, strict
Clippy, and the native/WASM feature matrix passed locally. Pillow remains the
exact byte/error oracle; payload ownership is a Rust implementation boundary
with no Pillow allocation contract. No new fixture, test function, diagnostic
origin, or coverage-only hook was added. No managed parity, feature-matrix, or
Coverage MCP rerun is claimed for this revision.

The BMP row-scratch ownership slice is implemented at production revision
`dd99a47d5342f7c4e7d50b09f98cdcbb8b41e812`. One-bit, indexed, RGB, and RGBA
BMP row assembly now reuses one scratch `Vec` per encoder invocation instead
of allocating a fresh row buffer for every emitted row. The writer consumes
each row synchronously before the next row is prepared, so encoded bytes,
error behavior, cancellation checkpoints, and sink output are unchanged. All
25 BMP encode parity rows, the complete 28-function fixture matrix, all 45
feature-gated Rust contracts, full all-feature tests, strict Clippy, and the
native/WASM feature matrix passed locally. Pillow remains the exact byte/error
oracle; scratch ownership is a Rust implementation boundary with no Pillow
allocation contract. No new fixture, test function, diagnostic origin, or
coverage-only hook was added. No managed parity, feature-matrix, or Coverage
MCP rerun is claimed for this revision.

The JPEG grayscale source-ownership slice is implemented at production
revision `5f1a7e61db30663022d4d28cc63dc2ec271e1de3`. Grayscale JPEG encoding
now borrows the immutable source luminance pixels through `Cow` while retaining
the existing row-level cancellation polls; RGB JPEG encoding still owns the
YCbCr conversion planes it must materialize. Encoded bytes, error behavior, and
work-budget checkpoints are unchanged. All 47 JPEG encode parity rows, the
complete 28-function fixture matrix, all 45 feature-gated Rust contracts, full
all-feature tests, strict Clippy, and the native/WASM feature matrix passed
locally. Pillow remains the exact byte/error oracle; source ownership is a
Rust implementation boundary with no Pillow allocation contract. No new
fixture, test function, diagnostic origin, or coverage-only hook was added. No
managed parity, feature-matrix, or Coverage MCP rerun is claimed for this
revision.

The TIFF conditional source-ownership slice is implemented at production
revision `b14aa2d89d5e24c87f1f2693a8b0886f3440e206`. TIFF now borrows the
immutable source raster for raw, PackBits, and non-predictive LZW/Deflate
paths. It creates the mutable owned working copy only when horizontal
prediction is selected with LZW or Deflate, the combinations that actually
rewrite samples before compression. Compressed outputs remain owned result
buffers, and encoded bytes plus explicit token checkpoints are unchanged. All
57 TIFF encode parity rows, the complete 28-function fixture matrix, all 45
feature-gated Rust contracts, and full all-feature tests passed locally. Pillow
remains the exact byte/error oracle; conditional ownership is a Rust
implementation boundary with no Pillow allocation contract. No new fixture,
test function, diagnostic origin, or coverage-only hook was added. No managed
parity, feature-matrix, or Coverage MCP rerun is claimed for this revision.

The PNG source-pixel ownership slice is implemented at production revision
`7a75acb33cbce80984dcf1dadd63d498b5f551e3`. L1, P8, L8, La8, RGB8, and RGBA8
encoding now borrows the immutable decoded pixel buffer through `Cow`; only
L16 performs the required little-endian-to-big-endian owned conversion. The
filter, compression, chunk, and policy stages are otherwise unchanged, so
there is no source-raster clone before filtering. All 83 PNG encode parity
rows, the complete 28-function fixture matrix, all 45 feature-gated Rust
contracts, and full all-feature tests passed locally. Pillow remains the exact
byte/error oracle; source ownership is a Rust implementation boundary with no
Pillow allocation contract. No new fixture, test function, diagnostic origin,
or coverage-only hook was added. No managed parity, feature-matrix, or Coverage
MCP rerun is claimed for this revision.

The JPEG entropy output-buffer ownership slice is implemented at production
revision `5929982b72e1edca9cce7cd82658b6f66ba29c89`. Baseline and progressive
entropy writers now take ownership of the already-built JPEG output buffer;
restart markers remain in that buffer, and checkpoint observations use the
same per-entropy-segment lengths as the former resettable staging writers.
Progressive scans likewise return the same buffer after each scan. This
removes the separate entropy staging buffers and their final copies without
changing markers, encoded bytes, or work-budget values. The 47 JPEG encode
parity rows, complete 28-function fixture matrix, all 45 feature-gated Rust
contracts, and full all-feature tests passed locally. Pillow remains the exact
encoded-byte/error oracle; buffer ownership and typed work budgets are
Rust-only contracts. No new fixture, test function, diagnostic origin, or
coverage-only hook was added. No managed parity, feature-matrix, or Coverage
MCP rerun is claimed for this revision.

The GIF sequence frame-ownership slice is implemented at production revision
`2f4b2afd58d813083d878bce2b6f1cea8968799a`. After the prepared frames have
been checked for transparency, the encoder consumes them during emission:
the first prepared frame is moved into the frame loop, later frames are moved
from the iterator, and only the global palette is retained for comparison.
This removes the retained prepared-frame collection, per-frame clones, and the
two full first-frame raster copies without changing palette decisions, encoded
bytes, or explicit token checkpoints. The existing 10 GIF encode matrix tests,
the complete 28-function Pillow fixture matrix, all 45 feature-gated Rust
contracts, and full all-feature tests passed locally. Pillow rows remain the
exact byte/error regression oracle; frame ownership is a Rust implementation
boundary with no Pillow allocation contract. No new fixture, test function,
diagnostic origin, or coverage-only hook was added. No managed parity,
feature-matrix, or Coverage MCP rerun is claimed for this revision.

The WebP animation assembly ownership slice is implemented at production
revision `228e419a0168ab083770c1fa009cf5c83d1711f3`. Each completed frame now
remains in its existing encoded buffer until the canvas alpha state is known;
the final ANMF prefix is stack-backed and its nested VP8/VP8L chunks are
written directly into the final RIFF buffer. This removes the temporary copied
chunk buffer and temporary ANMF payload, along with their staging copies. The
existing `test_encode_matrix_webp_animation` Pillow row and the complete
28-function fixture matrix preserve exact encoded bytes; the ample-token
sequence path is byte-identical as well. Token-aware animation assembly now
polls the remaining final-output chunk copy, while the removed staging copies
no longer create intermediate cancellation checkpoints. This is a
Pillow-observable byte regression gate plus a Rust-only ownership/cancellation
implementation boundary; Pillow has no allocation or caller-token contract.
No new fixture, test function, diagnostic origin, or coverage-only hook was
added. No managed parity, feature-matrix, or Coverage MCP rerun is claimed for
this revision.

The shared PNG/TIFF zlib-ng output-buffer ownership slice is implemented at
production and Rust test/runtime revision
`ea95e30e9a1538aaf316fd65b4c30e7a2f2c1e33`. Every no-token and token-aware
zlib level now starts its bit writer with the two-byte zlib header and returns
that writer-owned buffer directly before appending Adler-32, eliminating the
intermediate bitstream-to-output copy. This changes transient allocation and
copy behavior only; encoded bytes and checkpoint counts remain unchanged. The
existing `encode_work_budget_is_a_non_parity_result_contract` proves ample
byte identity for the PNG levels and retains the existing Rust-only policy
boundaries. The 28-function Pillow fixture matrix, all 45 feature-gated Rust
contracts, full all-feature tests, strict Clippy, and native/WASM feature
matrix passed locally; no parity row, fixture-manifest row, diagnostic origin,
new test function, or coverage-only hook was added. These are separate
implementation and Rust-only evidence: Pillow parity remains the regression
oracle for observable bytes/errors, while Pillow has no caller budget, sink,
or allocation contract. No managed parity, feature-matrix, or Coverage MCP
rerun is claimed for this revision.

The latest lossless WebP VP8L Huffman-tree leaf-census/materialization/depth slice is
implemented at production and Rust test/runtime revision
`a5ac1a14d7ad8f88c9ac60a0da73a94474708cb1` through the existing
`encode_work_budget_is_a_non_parity_result_contract`. Token-aware fixed-
alphabet scans now poll after each 64 code-length slots while counting active
symbols, materializing leaf nodes, and checking the resulting maximum depth;
the no-token path retains the original iterator construction. The generated
128×128 RGB probe rejects at
`maximum: 145,330`, `observed: 145,331` for the whole-buffer API and at
`maximum: 145,335`, `observed: 145,336` for the caller-owned sink. The
depth-scan endpoint rejects before structural delivery and retains sentinel
`[0xF1]`; this is work-budget evidence, not short-write/rollback evidence. The
slice changes no encoded bytes under the
ordinary or ample policy and adds no Pillow parity row, fixture-manifest row,
diagnostic origin, new test function, or coverage-only hook. No managed parity,
feature-matrix, or Coverage MCP rerun is claimed at this revision.

The current AVIF grid-provenance portion of implementation/runtime revision
`79d53951ba83b700f2647d5912718c634cecd417` uses the existing
`source_alpha_matches_the_container_contract` feature-gated contract now also
checks the committed `grid.avif` fixture's `AvifGridProperties`: version `0`,
raw flags `0`, `2` rows, `1` column, and an `80 × 80` output canvas. Portable
inspection checks the descriptor in native and WASI lanes; native still and
sequence-frame checks confirm the same source descriptor. The ordered `dimg`
IDs and bounded alpha/`iref` relationships remain separate fields. This is
source-provenance/specification evidence, not Pillow parity: the existing test
function was extended without a new fixture-manifest row, diagnostic origin,
or coverage-only hook, because Pillow exposes no equivalent grid topology
field.

The carried-forward WebP work-control portion of implementation/runtime
revision `79d53951ba83b700f2647d5912718c634cecd417` uses the existing
`encode_work_budget_is_a_non_parity_result_contract` now includes the lossless
VP8L RGB/RGBA source-pixel materialization checkpoint. The token-aware path
polls after each 1,024 source pixels; the no-token path keeps its original tight
maps and byte behavior. The same 64×64 RGB lossless WebP fixture rejects at
`maximum: 2`, `observed: 3`, and leaves the caller-owned sink sentinel `[0xC4]`
untouched. The later image-palette construction boundary is
`maximum: 6`, `observed: 7`, with `[0xBA]` untouched after four earlier
conversion intervals; RGBA hidden-RGB cleanup is `18/19` with `[0xB7]`, palette
lookup is `9,820/9,821` with `[0xA9]`, and palette-mode packing is `5,205/5,206`;
the ordered unique-color palette-drain boundary added at
`3dc95ea179b4be2c664ec2402ca0c8635e463e7f` is `18/19` with `[0xC7]`
untouched after its fourth 1,024-color checkpoint;
the token-aware cost-manager table initialization now leaves only `[0xC3]`
before that bounded sink rejection. The token-aware backward-reference result
backfill also polls every 256 backfilled entries, so a constant 1×512 RGB
probe rejects at `maximum: 2,516`, `observed: 2,517`; the existing sink path
retains the validated `RIFF`/`WEBP` prefix after its later checkpoint. This is
Rust-only work-control evidence, not a Pillow-observable result. Downstream exact boundaries were
recalibrated for the four conversion intervals: entropy analysis `23/24`,
histogram population `62/63`, combined entropy cost `80/81`, histogram merge
`8,258/8,259`, cost estimate `14,092/14,093`, Huffman-RLE `828/829` (or
`827/828` for the sink), grayscale preparation `195/196`, Huffman frequency
`44,001/44,002` (or `44,000/44,001` for the sink), code-length emission
`144,869/144,870`, Huffman-RLE token materialization `2,424/2,425` (or
`2,423/2,424` for the sink), and cache population `136,928/136,929`.

The current lossy WebP VP8 padded-plane slice extends the same
`encode_work_budget_is_a_non_parity_result_contract`: token-aware Y/U/V
edge-replication polls after each 1,024 padded items when dimensions require
padding, while aligned planes take a direct clone and the no-token path
retains the original tight helper and byte behavior. A 17×17 RGB probe reaches
the first shared padded-plane checkpoint and rejects at `maximum: 2`,
`observed: 3`; the direct-sink path reports the same typed work-budget result
and leaves sentinel `[0xA9]` untouched. This is Rust-only caller-budget
evidence because Pillow has no caller token, typed work-budget result, or
caller-owned sink/rollback contract, so it adds no parity row,
fixture-manifest row, diagnostic origin, new test function, or coverage-only
hook.

The current lossy WebP VP8 segment-assignment slice extends the same existing
contract: the analysis pass now polls its separate macroblock rewrite after
each 1,024 items, while the no-token path retains the original tight rewrite.
The aligned 512×512 feature-gate probe therefore reaches analysis at
`maximum: 326`, `observed: 327`, then segment assignment at
`maximum: 328`, `observed: 329` in both whole-buffer and direct-sink paths; the
direct-sink segment-assignment sentinel `[0xA7]` remains untouched. The same
aligned probe exercises the direct-clone padding fast path, which avoids
walking/polling edge replication when no padding is needed. Pillow has no
caller token, typed work-budget result, or sink/rollback contract, so this is
Rust-only evidence with no parity row, fixture-manifest row, diagnostic origin,
new test function, or coverage-only hook.

The latest lossy WebP VP8 analysis-histogram slice is implemented at
`a698bc5ec019d94131ae681ce87ee6d656f9d700` through that same existing
`encode_work_budget_is_a_non_parity_result_contract`. Token-aware histogram
construction now polls after each 64 completed 4×4 blocks, while the no-token
path retains the original tight transform loop. The committed
`tests/fixtures/input/images/webp/lossy_checker_17x19_q1_m0.webp` fixture
preserves exact bytes under the ample policy; its bounded whole-buffer and
direct-sink calls reject at `maximum: 8`, `observed: 9`, with sink sentinel
`[0xB6]` untouched. Pillow has no caller token, typed work-budget result,
caller-owned sink, or rollback contract, so this is Rust-only evidence with no
parity row, fixture-manifest row, diagnostic origin, new test function, or
coverage-only hook.

The current lossy WebP VP8 segment-clustering slice was introduced at
`6207722d23e014ed4fda9e2045499500d59b3c7c` and revalidated at
`a698bc5ec019d94131ae681ce87ee6d656f9d700`. Token-aware segment clustering
polls after each 64 alpha-domain values, including a trailing partial chunk,
while the no-token path retains the original byte-preserving algorithm. The
same committed fixture rejects at `maximum: 9`, `observed: 10` in both
whole-buffer and direct-sink calls, with sink sentinel `[0xB5]` untouched.
This remains Rust-only evidence with no parity row, fixture-manifest row,
diagnostic origin, new test function, or coverage-only hook.

The latest lossless WebP VP8L Huffman run-scan slice is implemented at
`51c6f7effe8a12649b19cff9fb276476be7232df` through the same existing
`encode_work_budget_is_a_non_parity_result_contract`. The token-aware
code-length run scan now polls whenever it crosses a 64-symbol boundary,
including before a long equal-length run finishes; the no-token path returns
to the original tight helper. The existing deterministic feature-gate probe
reaches this interior path while retaining the established Huffman-RLE
rejections at `maximum: 828`, `observed: 829` for the whole-buffer path and
`maximum: 827`, `observed: 828` for the caller-owned sink, with `[0xB1]`
untouched. Pillow has no caller token, typed work-budget result, caller-owned
sink, or rollback contract, so this is Rust-only evidence with no parity row,
fixture-manifest row, diagnostic origin, new test function, or coverage-only
hook.

The latest lossless VP8L Huffman-RLE fill-materialization slice is implemented
at production and test/runtime revision
`646ed73413a574368bfd01172fcd46c60622046f` through the same existing
`encode_work_budget_is_a_non_parity_result_contract`. Token-aware long-run
marking and normalized-count fills now poll after each 64 code-length values,
while the no-token helper retains its bulk fills. The existing caller-built
128×4 RGB palette probe proves `2,423/2,424` whole-buffer and `2,422/2,423`
caller-owned-sink rejection with `[0xC8]` untouched. This is Rust-only
work-control evidence: Pillow has no caller token, typed work-budget result,
caller-owned sink, or rollback contract, so no parity row, fixture-manifest
entry, diagnostic origin, new test function, or coverage-only hook was added.

The following lossless VP8L Huffman-RLE token-materialization slice is
implemented at production and test/runtime revision
`b78d0ffedc3bb193624eb11fd12d68378713489e` through the same existing
`encode_work_budget_is_a_non_parity_result_contract`. Token-aware code-length
RLE expansion now polls after each 16 emitted compressed tokens, while the
no-token helper retains its original tight construction path. The existing
caller-built 128×4 RGB palette probe proves `2,424/2,425` whole-buffer and
`2,423/2,424` caller-owned-sink rejection with `[0xC9]` untouched. This is
Rust-only work-control evidence: Pillow has no caller token, typed work-budget
result, caller-owned sink, or rollback contract, so no parity row,
fixture-manifest entry, diagnostic origin, new test function, or coverage-only
hook was added. No managed parity, feature-matrix, or Coverage MCP rerun is
claimed at this revision.

The latest lossy WebP VP8 boolean-output flush slice is implemented at
`2945ad28fde44976f33459c7664482f9c61a2b70` through the same existing
`encode_work_budget_is_a_non_parity_result_contract`. Token-aware boolean
flushes now drain pending delayed `0xff` output runs in 1,024-byte chunks,
charging the existing output accounting after each chunk and final byte before
returning; the no-token path retains the original flush helper. This is
Rust-only interruption evidence: Pillow has no caller token, typed work-budget
result, caller-owned sink, or rollback contract, so there is no parity row,
fixture-manifest row, diagnostic origin, new test function, or coverage-only
hook.

Historical exact-head managed validation for the preceding WebP work-control revision passed Pillow parity run
`b62b000d-ff77-4ead-9297-b8a87b69dca7` with 1,445/1,445 checks in 2,338 ms.
Feature-matrix run `9e62cd19-0cb2-4fb9-95a4-8818dd1f2eaa` passed all configured
lanes in 74,279 ms with `cache=cold`, `lanes=6`, `test_threads=2`,
`build_jobs=2`, `debug=0`, and `verbose=0`; its retained log says every native
and `wasm32-wasip1` lane agrees and has no `lock-wait` match. Nightly LLVM run
`174827b8-3e88-4885-8709-eabebc67a7c6` passed 85/85 tests in 83,593 ms and
ingested snapshot `c5bb524f-600d-42ba-9143-e16d2a47b0d0`, reporting
54,149/54,871 lines, 7,667/7,854 branches, 3,077/3,155 functions, and
83,579/85,188 regions. Compared with the preceding implementation snapshot
`74b84527-6b5c-4bd6-8c28-24c4f2ac07da`, covered/source totals changed by
`+34/+33` lines, `+14/+14` branches, `+0/+0` functions, and `+54/+54`
regions. In `src/codecs/webp/encode/vp8/bool_enc.rs`, coverage is 150/150
lines, 33/34 branches, 10/10 functions, and 216/221 regions: all lines and
functions are exercised, while the bounded query retains partial branch/region
records from generic token-aware instantiations without adding synthetic
coverage. The known LLVM JSON segment-normalization warning remains; the
aggregate shortfall is 722 lines, 187 branches, 78 functions, and 1,609
regions.

Test-runtime acceptance record: warm feature-matrix fanout (`8a54909`)

The harness-only revision `8a549091c618c7282c43b8566da79d6f592d4bae`
retains the same 33 native, `wasm32-unknown-unknown`, and
`wasm32-wasip1` lanes, 991/991 assertions, capability-table agreement, and
unknown-target library lint scope. For retained warm roots, the default
fanout now admits up to two single-worker lanes per logical CPU, capped at 24;
`MATRIX_JOBS`, `MATRIX_TEST_THREADS`, and `MATRIX_BUILD_JOBS` remain explicit
overrides. This changes validation scheduling only, not production code,
Pillow fixtures, parity rows, or coverage origins.

Exact managed feature-matrix run
`4dd23596-58c7-49ab-b92d-bb5600c06a4b` passed all 33 configured lanes in
31,992 ms with `cache=warm`, `lanes=24`, `test_threads=1`, `build_jobs=1`,
`debug=0`, and `verbose=0`; its retained log contains the
`capability tables OK: every native and wasm32-wasip1 lane agrees` marker and
no `lock-wait` match. On the same unchanged local host, the former 12-lane
default took 17.05 s and the 24-lane run took 14.86 s. These are
cache- and runner-sensitive observations, not universal benchmarks and not
the revision-bound allocation/peak-memory evidence still required by QA-010
and QA-030.

Exact-head Pillow parity run `f811133a-b09d-4273-9423-5804cbf60987` passed
1,445/1,445 checks in 1,401 ms. Nightly LLVM run
`69fc0cfe-4bc8-413f-b3ab-2214cafb7b51` passed 85/85 tests in 65,242 ms and
ingested snapshot `0e5bcd27-f18c-4b11-81fb-5ff6613b3f54`, retaining
54,149/54,871 lines, 7,667/7,854 branches, 3,077/3,155 functions, and
83,579/85,188 regions. Compared with the preceding snapshot
`c5bb524f-600d-42ba-9143-e16d2a47b0d0`, covered/source deltas are zero because
the slice changes only the test harness; the existing LLVM
segment-normalization warning remains. These execution, target, and coverage
records remain separate from Pillow parity, and no synthetic coverage test was
added.

Current WebP no-token hot-path acceptance record

The implementation revision
`863b68844fa871500bf7c88b29de77f76c24b258` removes two unconditional
`check_token(None)` calls from the ordinary no-token VP8L sampling-compaction
and lossy RGBA alpha-palette packing loops. Token-aware branches and their
documented checkpoint cadence are unchanged. This is a production-path
overhead correction, not a new work-boundary contract: it changes no
Pillow-visible bytes or errors and adds no fixture, parity row, diagnostic
origin, coverage-only hook, or unit test. The focused Rust-only
`encode_work_budget_is_a_non_parity_result_contract` passed 1/1 locally in
4,280 ms.

Exact-head Pillow parity run
`f62f657b-1c02-4f8a-a1ca-13914aa39bdd` passed 1,445/1,445 checks in 1,393 ms.
The source-changing feature-matrix run
`3b71a559-834a-4b4b-a3be-d285baae833a` passed all 33 configured lanes in
40,485 ms with `cache=cold`, `lanes=6`, `test_threads=2`, `build_jobs=2`,
`debug=0`, and `verbose=0`; its retained log contains the capability-table
agreement marker and no `lock-wait` match. Nightly LLVM run
`aead67da-dff1-43a4-aa3c-8cd4dd1dad4f` passed 85/85 tests in 62,290 ms and
ingested snapshot `95f8cdcd-961b-4656-b14b-ab58aa8fbb6b`, retaining
54,143/54,865 lines, 7,663/7,850 branches, 3,077/3,155 functions, and
83,567/85,174 regions. Compared with snapshot
`0e5bcd27-f18c-4b11-81fb-5ff6613b3f54`, covered/source deltas are
`-6/-6` lines, `-4/-4` branches, `+0/+0` functions, and `-12/-14` regions;
the denominator changes are the removed no-op polling code, not suppressed
coverage. The known LLVM segment-normalization warning remains, and the
aggregate shortfall is 722 lines, 187 branches, 78 functions, and 1,607
regions. These execution, target, and coverage records remain separate from
Pillow parity, and no synthetic coverage test was added.

Current partial structural-write acceptance record

Test revision `d5f7e416b30862819dbddb38f8b6027cc4219076` extends the existing
Rust-only sink contracts without changing production encoding or the Pillow
oracle. `output_sinks_receive_the_exact_encoded_bytes` now drives a destination
that accepts all but one byte of the second structural segment and then
returns an error through the PNG, BMP, ICO, WebP, JPEG, GIF, and native AVIF
still paths. `tiff_capability_and_destination_failures_are_structured` applies
the same prefix assertion to TIFF still output with both Raw and Deflate
compression options and to TIFF sequence output. Each call must normalize the
rejection to `ImageError::OutputWrite`, retain the selected format and
corresponding `StillEncode` or `SequenceEncode` stage, make exactly two writes,
and leave the delivered bytes as
an exact prefix of the whole-buffer result. The pre-existing cross-codec
contract retains the no-`flush` assertion for the broader still/sequence
writer set. These assertions prove observable prefix delivery; they do not
claim rollback, recovery, or partial-container cleanup.

This is not a Pillow parity fixture or row. Pillow has no caller-owned
`OutputSink`, no partial-write failure interface, and no equivalent destination
state to compare, so the evidence remains in the existing
`tests/feature_gate_tests.rs` Rust-only contracts. No diagnostic origin,
coverage-only hook, or synthetic unit test was added; unchanged coverage is
incidental to the real destination behavior.

Exact-head managed validation for this test revision recorded Pillow parity
run `7b21a875-5c2e-493f-b3cb-f98a96927b6d` at 1,445/1,445 checks in 914 ms.
Feature-matrix run `d757e2c9-ac63-43df-9fcb-892a22910e57` passed all 33
configured lanes in 51,641 ms with `cache=cold`, `lanes=6`,
`test_threads=2`, `build_jobs=2`, `debug=0`, and `verbose=0`; its retained
log contains the capability-table agreement marker and no `lock-wait` match.
Nightly LLVM run `a2d550c1-66ea-41ac-93f8-5bebac67530f` passed 85/85 tests in
64,321 ms and ingested snapshot
`48ef5dc3-f331-483d-92bf-4508c82f0102`, retaining 54,143/54,865 lines,
7,663/7,850 branches, 3,077/3,155 functions, and 83,567/85,174 regions.
Compared with the preceding implementation snapshot
`95f8cdcd-961b-4656-b14b-ab58aa8fbb6b`, all covered/source deltas are zero.
The known LLVM JSON segment-normalization warning and the aggregate shortfall
of 722 lines, 187 branches, 78 functions, and 1,607 regions remain. The
focused local `output_sinks_receive_the_exact_encoded_bytes` and
`tiff_capability_and_destination_failures_are_structured` contracts each
passed 1/1; the
all-target, all-feature Clippy gate also passed with warnings denied.

Current WebP sampling-witness runtime acceptance record

Test revision `d42d32f6b61ec97473230025dd4064cbd60f245e` compacts the existing
Rust-only VP8L meta-histogram sampling witness from a 16,384x16 RGBA probe to
12,288x16. The deterministic probe still keeps two real meta-histogram groups,
retains 1,025 equal tile symbols before the distinct tail, and reaches the
same interior comparison after the first 1,024 symbols. The generated pixel
allocation and tile count are each reduced by 25%; the recalibrated exact
work-budget boundary is `maximum: 600,000`, `observed: 600,001` for both
whole-buffer and caller-owned-sink calls, with sentinel `[0xB7]` untouched.

This remains a Rust-only feature-gate witness, not a Pillow parity fixture or
row. Pillow has no caller token, typed work-budget result, caller-owned sink,
or rollback contract, so the witness stays in the existing
`encode_work_budget_is_a_non_parity_result_contract`; no new test function,
fixture-manifest row, diagnostic origin, synthetic unit test, or
coverage-only hook was added. The implementation revision and accepted
coverage snapshot are unchanged.

Exact-head managed feature-matrix run
`7bae6546-8023-4d10-9b1f-fc182e3e1a50` passed all 33 configured lanes in
14,731 ms with `cache=warm`, `lanes=24`, `test_threads=1`, `build_jobs=1`,
`debug=0`, and `verbose=0`; its retained log contains the capability-table
agreement marker and no `lock-wait` match. The preceding exact-head warm run
`8459ba78-b482-41f6-9436-ac3d81670a34` took 16,775 ms with the same scheduler,
an observed reduction of 2,044 ms (12.2%) on this host. These are
cache- and runner-sensitive observations, not the revision-bound benchmark,
allocation, or peak-memory evidence still required by QA-010 and QA-030.

Exact-head Pillow parity run `995d9dcf-14c4-4566-a463-40b5b7cc573d` passed
1,445/1,445 checks in 642 ms. The parity surface and coverage totals remain
unchanged.

Current WebP predictor row-copy acceptance record

Implementation revision `446a4a723b8a5ed066b20b0086669cb927ec92b4` makes the
token-aware VP8L predictor tile scan copy each image-width source row in
1,024-pixel chunks and poll after each completed chunk. The ordinary no-token
branch keeps its original bulk `copy_from_slice`, so Pillow-visible bytes and
the no-token hot path remain unchanged. The existing
`encode_work_budget_is_a_non_parity_result_contract` reaches this row-copy
boundary at `maximum: 3,675`, `observed: 3,676` in both whole-buffer and
caller-owned-sink calls; the sink sentinel remains `[0xAA]`.

This is Rust-only interruption evidence, not a Pillow parity fixture or row.
Pillow has no caller token, typed work-budget result, caller-owned sink, or
rollback contract, so the existing feature-gate contract remains the correct
home. No new test function, fixture-manifest row, diagnostic origin,
synthetic unit test, or coverage-only hook was added.

Exact-head managed feature-matrix run
`e536c225-71a7-4e56-80e8-f75ecc259b1e` passed all 33 configured lanes in
30,606 ms with `cache=cold`, `lanes=6`, `test_threads=2`, `build_jobs=2`,
`debug=0`, and `verbose=0`; its retained log contains the capability-table
agreement marker and no `lock-wait` match. Exact-head Pillow parity run
`cc4ba163-57eb-4bfa-a1f9-6f9915872a20` passed 1,445/1,445 checks in 606 ms.

Nightly LLVM run `8c53809f-62dd-40a2-9c64-a2e57031d396` passed 85/85 tests in
49,992 ms and ingested snapshot
`9cbeb2a1-b603-407f-9f4d-93d65cb73061`, retaining 54,152/54,878 lines,
7,666/7,856 branches, 3,077/3,155 functions, and 83,583/85,197 regions.
Compared with the preceding accepted snapshot
`48ef5dc3-f331-483d-92bf-4508c82f0102`, covered/source deltas are `+9/+13`
lines, `+3/+6` branches, `+0/+0` functions, and `+16/+23` regions. The
predictor source file is 320/320 lines, 62/62 branches, 23/23 functions, and
594/599 regions covered. The known LLVM segment-normalization warning remains;
the current aggregate shortfall is 726 lines, 190 branches, 78 functions,
and 1,614 regions. The parity, feature-matrix, and coverage records remain
separate evidence systems.

Current WebP source-preparation acceptance record

Implementation revision `0f9a59b55a15ca4899aead8b2fa2ff9b97f27ef6` adds
token-aware checkpoints to WebP L1/P8/L8/La8/CMYK source-mode preparation and
RGBA alpha/RGB extraction. The token-aware branches poll after each 1,024
source pixels; the no-token maps and iterators retain their original tight
paths and byte behavior. The existing
`encode_work_budget_is_a_non_parity_result_contract` reaches the first L8
source-preparation boundary with a generated 1,024×1 probe at
`maximum: 3`, `observed: 4` in both whole-buffer and caller-owned-sink calls;
the sink sentinel remains `[0xCA]`.

This is Rust-only interruption evidence, not a Pillow parity fixture or row.
Pillow has no caller token, typed work-budget result, caller-owned sink, or
rollback contract, so the witness remains in the existing feature-gated
contract. No new test function, fixture-manifest row, diagnostic origin,
synthetic unit test, or coverage-only hook was added.

Exact-head managed feature-matrix run
`f19bbffa-e118-4d90-8bd4-baa0f1d204d7` passed all 33 configured lanes in
30,317 ms with `cache=cold`, `lanes=6`, `test_threads=2`, `build_jobs=2`,
`debug=0`, and `verbose=0`; its retained log contains the capability-table
agreement marker and no `lock-wait` match. Exact-head Pillow parity run
`3ad9c7f6-1d86-4c18-a712-19711dc6e12c` passed 1,445/1,445 checks in 5,987 ms.

Nightly LLVM run `eece1b04-2370-4d9b-a268-24a64cf4ef5d` passed 85/85 tests in
51,686 ms and ingested snapshot
`36e1f4f0-be46-48c1-81da-3d3fb9680562`, retaining 54,217/55,037 lines,
7,681/7,880 branches, 3,080/3,160 functions, and 83,672/85,494 regions.
Compared with the preceding accepted snapshot
`48ef5dc3-f331-483d-92bf-4508c82f0102`, covered/source deltas are
`+74/+172` lines, `+18/+30` branches, `+3/+5` functions, and `+105/+320`
regions. The known LLVM JSON segment-normalization warning remains; the
current aggregate shortfall is 820 lines, 199 branches, 80 functions, and
1,822 regions. In `src/codecs/webp/encode/mod.rs`, coverage is 617/740 lines,
84/98 branches, 47/59 functions, and 1,008/1,272 regions. The parity,
feature-matrix, and coverage records remain separate evidence systems.

Current lossless WebP VP8L RIFF container-copy acceptance record

Implementation revision `f5eacca47a32d9ad0208700b2656ffc6b4d79a8e` keeps the
ordinary no-token native VP8L RIFF frame copy as one bulk append, while the
token-aware path copies the complete frame payload in 1,024-byte chunks and
polls after each complete chunk. The existing
`encode_work_budget_is_a_non_parity_result_contract` uses its deterministic
128×128 patterned RGB `output_lossless_image`, whose encoded output is exactly
1,764 bytes. The pre-fix bulk-copy control completed at `maximum: 99,250`; the
chunked path rejects the same whole-buffer call at `maximum: 99,250`,
`observed: 99,251`. Its direct-sink witness rejects at `maximum: 99,249`,
`observed: 99,250`, before delivery, with sentinel `[0xAB]` untouched.

This is Rust-only interruption evidence, not a Pillow parity fixture or row.
Pillow has no caller token, typed work-budget result, caller-owned sink, or
rollback contract, so no new parity row, fixture-manifest row, diagnostic
origin, synthetic unit test, new test function, or coverage-only hook was
added.

Local focused `encode_work_budget_is_a_non_parity_result_contract` passed 1/1
in approximately 2.41 s; `cargo check --all-features --all-targets --locked`,
strict Clippy, rustfmt check, and `git diff --check` passed.

Exact-head managed feature-matrix run
`8bacf2b5-1338-4b2d-bcd5-379e277e86da` passed all 33 configured lanes in
52,811 ms with `cache=cold`, `lanes=6`, `test_threads=2`, `build_jobs=2`,
`debug=0`, and `verbose=0`; exact-head Pillow parity run
`3212f4d1-5897-4b02-9843-5d04d2e347bb` passed 1,445/1,445 checks in 1,467 ms.

Nightly LLVM run `f3edb17a-24ee-4a34-8565-f088ec36cc97` passed 85/85 tests
in 62,723 ms and ingested snapshot
`3527ccf6-8bbc-405c-9bef-421166b98599`, retaining 54,304/55,119 lines,
7,711/7,906 branches, 3,082/3,162 functions, and 83,818/85,644 regions.
Compared with the preceding accepted snapshot
`a9f63171-3e46-4a28-a0cf-e46f3b983be8`, covered/source deltas are `+8/+6`
lines, `+1/+0` branches, `+0/+0` functions, and `+5/+5` regions. The known
LLVM JSON segment-normalization warning remains; current aggregate shortfall
is 815 lines, 195 branches, 80 functions, and 1,826 regions. Parity,
feature, coverage, and non-Pillow-origin records remain separate.

Current lossy WebP VP8/ALPH RIFF container-copy acceptance record

Implementation revision `62ccc3800be8960af5c738e0fd5015f77ba92115` keeps the
ordinary no-token native VP8 and extended ALPH/VP8 container payload copies as
bulk appends, while the token-aware path copies those payloads in 1,024-byte
chunks and polls after each complete chunk. The existing
`encode_work_budget_is_a_non_parity_result_contract` uses the deterministic
128×128 patterned RGB `assembly_image` with a 4,096-byte ICC payload; its
native VP8 payload is 10,605 bytes and the metadata-bearing output is exactly
14,748 bytes. The pre-fix native VP8 bulk-copy control completed at
`maximum: 889,806`; the chunked whole-buffer call rejects at
`maximum: 889,806`, `observed: 889,807`. The direct-sink witness rejects at
`maximum: 889,796`, `observed: 889,797`, with sentinel `[0xB7]` untouched.

This is Rust-only interruption evidence, not a Pillow parity fixture or row.
Pillow has no caller token, typed work-budget result, caller-owned sink, or
rollback contract, so no new parity row, fixture-manifest row, diagnostic
origin, synthetic unit test, new test function, or coverage-only hook was
added.

Local focused `encode_work_budget_is_a_non_parity_result_contract` passed 1/1
in approximately 2.43 s; `cargo check --all-features --all-targets --locked`,
strict Clippy, rustfmt, `git diff --check`, and the claim-ledger,
coverage-origin, and diagnostic-provenance verifiers passed.

Exact-head managed feature-matrix run
`2515e677-dd0d-419c-a15c-303ef0f89500` passed all 33 configured lanes in
59,414 ms with `cache=cold`, `lanes=6`, `test_threads=2`, `build_jobs=2`,
`debug=0`, and `verbose=0`; its retained log reported the feature-matrix
settings and passing lanes without a `lock-wait` match. Exact-head Pillow
parity run `cb43336c-721c-423d-a2dd-8aa209a8de60` passed 1,445/1,445 checks
in 1,979 ms.

Nightly LLVM run `5d3138e6-7d15-4454-a687-29e5662cb9e9` passed 85/85 tests in
58,811 ms and ingested snapshot
`c2c0d660-6dc9-4b07-ae1e-2eda1b201d53`, retaining 54,331/55,146 lines,
7,715/7,910 branches, 3,083/3,163 functions, and 83,850/85,682 regions.
Compared with the preceding accepted snapshot
`3527ccf6-8bbc-405c-9bef-421166b98599`, covered/source deltas are `+27/+27`
lines, `+4/+4` branches, `+1/+1` functions, and `+32/+38` regions. The known
LLVM JSON segment-normalization warning remains; current aggregate shortfall
is 815 lines, 195 branches, 80 functions, and 1,832 regions. Parity, feature,
coverage, and non-Pillow-origin records remain separate.

Current lossless WebP VP8L candidate-trial suffix-copy acceptance record

Implementation revision `487348d01389eb8d100b8a668c9921d97634c022` adds the
VP8L candidate-trial suffix copy. VP8L candidate trials already
reuse the emitted prefix and retain only each candidate suffix. The token-aware
winner-selection path now copies that suffix in 1,024-byte chunks, while the
ordinary no-token path keeps one bulk suffix copy. The existing
`encode_work_budget_is_a_non_parity_result_contract` reuses its deterministic
128×128 LCG lossless probe, whose complete output is exactly 49,236 bytes.
The pre-fix bulk-suffix control completes at `139,125`; the chunked path
rejects at `maximum: 139,172`, `observed: 139,173`, proving 48 new complete
copy intervals rather than a final-check artifact. The direct-sink witness
rejects at `maximum: 139,171`, `observed: 139,172` before delivery, with
sentinel `[0xDB]` untouched.

This is Rust-only interruption evidence, not a Pillow parity fixture or row.
Pillow has no caller token, typed work-budget result, caller-owned sink, or
rollback contract, so no new parity row, fixture-manifest row, diagnostic
origin, synthetic unit test, new test function, or coverage-only hook was
added; the existing feature-gated contract remains the correct home.

Focused `encode_work_budget_is_a_non_parity_result_contract` passed 1/1 in
approximately 2.36 s after the change. Exact-head managed evidence on the
committed revision is recorded by feature-matrix run
`2d14425b-9a6a-4ff3-9577-6b65dc444bc3` (all 33 configured lanes passed in
47,038 ms; `cache=cold`, `lanes=6`, `test_threads=2`, `build_jobs=2`,
`debug=0`, `verbose=0`, with no retained `lock-wait` match), Pillow parity
run `c7e67804-f563-43a4-8814-8cf8b5e88319` (1,445/1,445 in 808 ms), and
nightly LLVM run `96813ff1-b9e7-45ac-945c-2c0318bc8538` (85/85 in 59,866 ms).
The latter ingested snapshot `026d33d8-47e7-4d36-99a1-08757710f186`, with
54,334/55,146 lines, 7,716/7,910 branches, 3,083/3,163 functions, and
83,854/85,684 regions. Compared with the preceding accepted snapshot
`c2c0d660-6dc9-4b07-ae1e-2eda1b201d53`, covered/source deltas are `+3/+0`
lines, `+1/+0` branches, `+0/+0` functions, and `+4/+2` regions; the
aggregate shortfall is 812 lines, 194 branches, 80 functions, and 1,830
regions. The known LLVM JSON segment-normalization warning remains.

The harness-only follow-up is implemented at revision
`3519b21c2ac3dd0cbd70207cfa0a2669f31300b5`. Its feature-matrix cache
signature still includes every tracked and non-ignored source/test input, but
now batches the checksum work instead of spawning one process per file. This
changes only cache-classification overhead; the 33 lanes, 991 assertions,
capability-table agreement, and all parity/Rust evidence origins remain
unchanged. Exact-head managed feature-matrix run
`01d0ff0d-c663-4a08-8882-06916f2f05e9` passed all 33 configured lanes in
6,072 ms with `cache=warm`, `lanes=24`, `test_threads=1`, `build_jobs=1`,
`debug=0`, and `verbose=0`; its retained log contains the native/WASI
capability agreement marker and no `lock-wait` match. On the same local host,
the warm repeat fell from 12.5 s before this change to 5.82 s after it. These
are cache- and runner-sensitive observations, not universal benchmarks. No
Rust source changed, so the accepted LLVM snapshot and Pillow-parity result
remain the preceding records above.

Earlier WebP container/metadata assembly acceptance record

Implementation revision `51bc2cc5ef5fc2d2329e6d6f7ccac41b088fe5c2` keeps the
ordinary no-token WebP sequence/metadata assembly copies as bulk appends,
while the token-aware path copies caller-sized metadata payloads, encoded
chunks, and ANMF frame payloads in 1,024-byte chunks and polls after each
complete chunk. The existing `encode_work_budget_is_a_non_parity_result_contract`
uses a deterministic 128×128 patterned RGB image with a 4,096-byte ICC
payload; the metadata-bearing output is exactly 14,748 bytes. An ample budget
preserves exact bytes. The pre-fix bulk-copy control completed at the exact
threshold `889,783`; the chunked path rejects at `maximum: 889,795`,
`observed: 889,796` in both whole-buffer and caller-owned-sink paths, with
sentinel `[0xB7]` untouched.

This is Rust-only interruption evidence, not a Pillow parity fixture or row.
Pillow has no caller token, typed work-budget result, caller-owned sink, or
rollback contract, so no new parity row, fixture-manifest row, diagnostic
origin, synthetic unit test, new test function, or coverage-only hook was
added.

Local focused `encode_work_budget_is_a_non_parity_result_contract` passed 1/1
in approximately 2.47 s; `cargo check --all-features --all-targets --locked`,
strict Clippy, rustfmt check, and `git diff --check` passed.

Exact-head managed feature-matrix run
`01190d09-b63f-4238-b384-374bc480c814` passed all 33 configured lanes in
65,433 ms with `cache=cold`, `lanes=6`, `test_threads=2`, `build_jobs=2`,
`debug=0`, and `verbose=0`; its retained log contains the capability-table
agreement marker and no `lock-wait` match. Exact-head Pillow parity run
`76f40704-a48d-47c9-971e-daa19b4e2f86` passed 1,445/1,445 checks in 1,471 ms.

Nightly LLVM run `214d548c-c893-4005-a087-17516c6f6f83` passed 85/85 tests
in 64,548 ms and ingested snapshot
`a9f63171-3e46-4a28-a0cf-e46f3b983be8`, retaining 54,296/55,113 lines,
7,710/7,906 branches, 3,082/3,162 functions, and 83,813/85,639 regions.
Compared with preceding accepted snapshot
`29e913f2-b48a-4789-8a10-ba344b8323f4`, covered/source deltas are `+31/+23`
lines, `+8/+4` branches, `+1/+1` functions, and `+48/+49` regions. Current
`src/codecs/webp/encode/mod.rs` is 640/763 lines, 88/102 branches,
48/60 functions, and 1,050/1,321 regions. Current aggregate shortfall:
817 lines, 196 branches, 80 functions, and 1,826 regions. The known LLVM
JSON segment-normalization warning remains. Parity, feature, coverage, and
non-Pillow-origin records remain separate.

Current WebP alpha-stream buffer-copy acceptance record

Implementation revision `8d07256f934bf7cf8c09962ace3b29cdbd9b9215` keeps the
ordinary no-token lossy WebP alpha-stream copies as one bulk append, while the
token-aware path copies both the compressed VP8L candidate and raw alpha plane
in 1,024-byte chunks and polls after each complete chunk. The existing
`encode_work_budget_is_a_non_parity_result_contract` uses its 64×64
`alpha_image` with default lossy WebP options: the first full raw-copy
interval rejects at `maximum: 1,485`, `observed: 1,486` in both whole-buffer
and caller-owned-sink paths, with sentinel `[0xB6]` untouched. A stronger
complete-call witness rejects at `maximum: 176,838`, `observed: 176,839` in
both paths; the pre-fix bulk-copy control completed at the old exact budget
of 176,838, which proves the new rejection is caused by the chunked copy
boundary rather than the existing final check. A short final chunk remains
covered by the existing post-copy poll.

This is Rust-only interruption evidence, not a Pillow parity fixture or row.
Pillow has no caller token, typed work-budget result, caller-owned sink, or
rollback contract, so no new parity row, fixture-manifest row, diagnostic
origin, synthetic unit test, new test function, or coverage-only hook was
added; the existing feature-gated contract remains the correct home.

Exact-head managed feature-matrix run
`eb84e5dd-aee7-488a-9189-5826c6668dbf` passed all configured lanes in 54,535 ms
with `cache=cold`, `lanes=6`, `test_threads=2`, `build_jobs=2`, `debug=0`,
and `verbose=0`; its retained log contains the capability-table agreement
marker and no `lock-wait` match. Exact-head Pillow parity run
`f03501ea-fd36-4e42-8f58-eb128e678c9a` passed 1,445/1,445 checks in 1,363 ms.

Nightly LLVM run `d878a20d-c26c-4371-84de-82ffa2b69952` passed 85/85 tests in
62,533 ms and ingested snapshot
`29e913f2-b48a-4789-8a10-ba344b8323f4`, retaining 54,265/55,090 lines,
7,702/7,902 branches, 3,081/3,161 functions, and 83,765/85,590 regions.
Compared with the preceding accepted snapshot
`c05b6728-0313-4a05-89e2-291fb61e283b`, covered/source deltas are `+15/+15`
lines, `+4/+4` branches, `+1/+1` functions, and `+25/+24` regions. The
current `src/codecs/webp/native/encoder.rs` file is 2,039/2,122 lines,
450/478 branches, 94/94 functions, and 3,031/3,287 regions; the histogram
file remains 782/782 lines, 172/172 branches, 39/39 functions, and
1,184/1,210 regions. The known LLVM JSON segment-normalization warning
remains; the current aggregate shortfall is 825 lines, 200 branches,
80 functions, and 1,825 regions. The parity, feature-matrix, and coverage
records remain separate evidence systems.

Historical acceptance record: WebP histogram collection

Implementation revision `336e61988c1873f32d70626e9f1fba608e7c84a6` adds a
token-aware checkpoint to lossless VP8L's populated tile-histogram filter and
clone loop after each 64 histograms. The ordinary no-token path retains its
original iterator. The existing `encode_work_budget_is_a_non_parity_result_contract`
uses the 64×64 `lossless_image` probe, which has exactly 64 histogram tiles;
whole-buffer and direct-sink calls reject at `maximum: 15,198`,
`observed: 15,199`, with sentinel `[0xC8]` untouched.

This is Rust-only interruption evidence, not a Pillow parity fixture or row.
Pillow has no caller token, typed work-budget result, caller-owned sink, or
rollback contract, so the existing feature-gated contract remains the correct
home. No new test function, fixture-manifest row, diagnostic origin,
synthetic unit test, or coverage-only hook was added.

Current WebP meta-pixel materialization acceptance record

Implementation revision `c9acf5d219e82c9c1077c3ec3e0c5df345ee28c5` adds a
token-aware checkpoint to lossless VP8L meta-pixel materialization after
`optimize_sampling`, polling after each 1,024 retained histogram symbols. The
ordinary no-token symbols-to-meta-pixels map retains its original iterator and
collection. The existing `encode_work_budget_is_a_non_parity_result_contract`
uses the 512×512 `cache_probe_image`, which produces 4,096 retained symbols;
the whole-buffer path rejects at `maximum: 132,284`, `observed: 132,285`, and
the direct-sink path rejects at `maximum: 132,283`, `observed: 132,284`, with
sentinel `[0xB4]` untouched.

This is Rust-only interruption evidence, not a Pillow parity fixture or row.
Pillow has no caller token, typed work-budget result, caller-owned sink, or
rollback contract, so no new parity row, fixture-manifest row, diagnostic
origin, synthetic unit test, new test function, or coverage-only hook was
added.

Exact-head managed feature-matrix run
`5f4d9367-1ffd-487d-ae69-d97d236c3709` passed all 33 configured lanes in
62,249 ms with `cache=cold`, `lanes=6`, `test_threads=2`, `build_jobs=2`,
`debug=0`, and `verbose=0`; its retained log contains the capability-table
agreement marker and no `lock-wait` match. Exact-head Pillow parity run
`d4f38bd8-b3f7-4ab6-8ffe-eb595b96c260` passed 1,445/1,445 checks in 1,541 ms.

Nightly LLVM run `35d447c9-9c6b-4621-9bb8-d83312b1fe8e` passed 85/85 tests in
67,126 ms and ingested snapshot
`c05b6728-0313-4a05-89e2-291fb61e283b`, retaining 54,250/55,075 lines,
7,698/7,898 branches, 3,080/3,160 functions, and 83,740/85,566 regions.
Compared with the preceding accepted snapshot
`4abde553-9342-4f9a-9602-4d2a80243b30`, covered/source deltas are `+7/+11`
lines, `+3/+4` branches, `+0/+0` functions, and `+19/+21` regions. The
current `src/codecs/webp/native/encoder.rs` file is 2,024/2,107 lines,
446/474 branches, 93/93 functions, and 3,008/3,263 regions; the histogram
file remains 782/782 lines, 172/172 branches, 39/39 functions, and
1,184/1,210 regions. The known LLVM JSON segment-normalization warning
remains; the current aggregate shortfall is 825 lines, 200 branches,
80 functions, and 1,826 regions. The parity, feature-matrix, and coverage
records remain separate evidence systems.

Current WebP palette-drain acceptance record

Implementation revision `3dc95ea179b4be2c664ec2402ca0c8635e463e7f` adds a
token-aware checkpoint to lossless VP8L's ordered unique-color palette drain
after each 1,024 colors. The ordinary no-token `BTreeSet` drain remains the
existing tight iterator. The same existing
`encode_work_budget_is_a_non_parity_result_contract` uses the 64×64
`lossless_image` probe, which reaches the fourth drain checkpoint; whole-buffer
and direct-sink calls reject at `maximum: 18`, `observed: 19`, with sentinel
`[0xC7]` untouched.

This is Rust-only interruption evidence, not a Pillow parity fixture or row.
Pillow has no caller token, typed work-budget result, caller-owned sink, or
rollback contract, so no new parity row, fixture-manifest row, diagnostic
origin, synthetic unit test, new test function, or coverage-only hook was
added.

Exact-head managed feature-matrix run
`47b16e65-d4bf-414f-9cff-58c6172c3f15` passed all 33 configured lanes in
57,196 ms with `cache=cold`, `lanes=6`, `test_threads=2`, `build_jobs=2`,
`debug=0`, and `verbose=0`; its retained log contains the capability-table
agreement marker and no `lock-wait` match. Exact-head Pillow parity run
`af6b89cc-3e27-480f-9384-fba0238f83c6` passed 1,445/1,445 checks in 1,480 ms.

Nightly LLVM run `a3428d4d-cda7-49cc-86c3-97e77b65e3f4` passed 85/85 tests in
63,570 ms and ingested snapshot
`4abde553-9342-4f9a-9602-4d2a80243b30`, retaining 54,243/55,064 lines,
7,695/7,894 branches, 3,080/3,160 functions, and 83,721/85,545 regions.
Compared with the preceding accepted snapshot
`c48bc742-ea7b-4f91-963e-74f60edfadac`, covered/source deltas are `+9/+9`
lines, `+2/+2` branches, `+0/+0` functions, and `+15/+17` regions. The
current `src/codecs/webp/native/encoder.rs` file is 2,017/2,096 lines,
443/470 branches, 93/93 functions, and 2,989/3,242 regions; the histogram
file remains 782/782 lines, 172/172 branches, 39/39 functions, and
1,184/1,210 regions. The known LLVM JSON segment-normalization warning
remains; the current aggregate shortfall is 821 lines, 199 branches,
80 functions, and 1,824 regions. The parity, feature-matrix, and coverage
records remain separate evidence systems.

Current WebP histogram row-transition acceptance record

Implementation revision `2c4d994227deb9cdc06c44228d39a6d6689d1bbf` adds a
token-aware checkpoint inside lossless VP8L histogram clustering when one Copy
token advances across image rows. The token-aware path polls after each 256
row transitions; the ordinary no-token loop remains unchanged. The existing
`encode_work_budget_is_a_non_parity_result_contract` uses a constant 1×512 RGB
`cluster_row_image`: ample-budget encoding preserves exact bytes, while the
whole-buffer and direct-sink paths reject at `maximum: 2,516`,
`observed: 2,517`, with sentinel `[0xC9]` untouched.

This is Rust-only interruption evidence, not a Pillow parity fixture or row.
Pillow has no caller token, typed work-budget result, caller-owned sink, or
rollback contract, so the existing feature-gated contract remains the correct
home. No new test function, fixture-manifest row, diagnostic origin,
synthetic unit test, or coverage-only hook was added.

Exact-head managed feature-matrix run
`5c987f41-e8fb-472e-8328-8e87b8fd47b2` passed all 33 configured lanes in
31,727 ms with `cache=cold`, `lanes=6`, `test_threads=2`, `build_jobs=2`,
`debug=0`, and `verbose=0`; its retained log had no `lock-wait` match.
Exact-head Pillow parity run `3ce3e54c-83b1-49e9-9b3d-badd42630672` passed
1,445/1,445 checks in 5,455 ms with zero skips.

Nightly LLVM run `bf6de8cf-e416-438b-a4a3-f1bb0f7d7f48` passed 85/85 tests in
65,624 ms and ingested snapshot
`66e5687a-1cda-4c61-bf1b-8e83f9c12146`, retaining 54,225/55,046 lines,
7,687/7,886 branches, 3,080/3,160 functions, and 83,686/85,508 regions.
Compared with the preceding accepted snapshot
`48ef5dc3-f331-483d-92bf-4508c82f0102`, covered/source deltas are
`+82/+181` lines, `+24/+36` branches, `+3/+5` functions, and `+119/+334`
regions. The known LLVM JSON segment-normalization warning remains; the
current aggregate shortfall is 821 lines, 199 branches, 80 functions, and
1,822 regions. The histogram source file is 773/773 lines, 166/166 branches,
39/39 functions, and 1,164/1,190 regions. The parity, feature-matrix, and
coverage records remain separate evidence systems.

The finer lossy WebP VP8 mode-selection, transform, trellis, distortion, and
residual-cost slice is implemented in
`2f957016e8b52d1e76a4de3a04fa54e88f1f6dd8` through the same existing
`encode_work_budget_is_a_non_parity_result_contract`. Token-aware frame
selection retains the outer checkpoint after each 64 completed macroblocks for
intra16/chroma and completed-decision work, and now also polls after each
intra4 candidate-trial stage, each forward- and inverse-transform row/column
subpass, each non-trellis quantization coefficient, each method-6
trellis-quantization coefficient candidate and path-reconstruction node,
each squared-error pixel, each spectral-distortion weighted-transform
row/column pass, each residual-cost coefficient, candidate, and completed luma
4×4 block. The committed fixture
`tests/fixtures/input/images/webp/lossy_checker_17x19_q1_m0.webp` is small
enough to avoid the outer 64-macroblock boundary. Its method-0 witness reaches
the first squared-error pixel in the initial distortion-only intra4 candidate
after the segment-clustering boundary: whole-buffer and direct-sink calls
reject at exactly `maximum: 12`, `observed: 13`, with sentinel `[0xAE]`
untouched.
The same fixture is reused with method 2 to skip that intentional
distortion-only preselection and exercise the interiors: ample-budget encoding
preserves exact bytes; the first forward-transform row rejects at
`maximum: 6`, `observed: 7` with sink sentinel `[0xAD]`, the first non-trellis
quantization coefficient at `maximum: 14`, `observed: 15` with `[0xAF]`, and
the first inverse-transform column at `maximum: 30`, `observed: 31` with
`[0xB0]`. The same method-2 fixture now also exercises the previously coarse
interiors: the first squared-error pixel rejects at `maximum: 38`,
`observed: 39` with `[0xB2]`, the first spectral-distortion weighted-transform
row at `maximum: 55`, `observed: 56` with `[0xB3]`, and the first residual-cost
coefficient at `maximum: 72`, `observed: 73` with `[0xB4]`. Method 6 reuses the
same fixture: its ample-budget encoding preserves exact bytes, and the first
trellis coefficient candidate rejects at `maximum: 23,442`, `observed: 23,443`
with sink sentinel `[0xB1]`. All boundaries are asserted through whole-buffer
and direct-sink calls. The no-token path keeps its original tight
selection/transform/cost loop; only the token path enters these interior
helpers. Pillow exposes neither a caller token nor a typed work-budget result,
caller-owned sink, or rollback contract; this is Rust-only
evidence with no parity row, fixture-manifest row, diagnostic origin, new test
function, or coverage-only hook.

The feature-matrix runtime follow-up is committed at
`f1de82ef6d5cde827daf6f5fa195d938a9abe67b`. Its isolated native and
`wasm32-wasip1` test lanes now default to `MATRIX_TEST_OPT_LEVEL=2`, matching
the repository's regular Cargo test profile; the override remains available
for compile-heavy environments. The existing native AVIF structural-sink
contract also pins `sequence_time=1` in its caller-built options so repeated
sequence encodes compare identical BMFF timestamps rather than wall-clock
metadata. This is harness/input determinism only: production codec behavior,
Pillow manifest rows, parity fixtures, and coverage origins are unchanged.
The exact-revision managed feature-matrix run
`db5b85d9-8189-4589-8354-eb9d45365bf8` passed all 33 configured lanes in
8,806 ms with `cache=warm`, `lanes=12`, `test_threads=1`, `build_jobs=1`,
`debug=0`, and `verbose=0`; its log records the native/WASI capability
agreement marker and no `lock-wait` match. The parent level-1 run
`1ddb3b47-4876-4914-be5e-d4c215c8f4ef` took 11,478 ms on the same host; the
first level-2 run rebuilt isolated artifacts in 25,946 ms. These are
cache- and runner-sensitive observations, not universal speed claims.

The current stale-cache classification follow-up is committed at
`8c531be322c6234b4694e9353164587f8c79b4ba`. The harness fingerprints
build/test inputs before choosing the retained-root scheduler, so source
changes use compile-oriented fanout while unchanged revisions retain the
warm scheduler. Source-changing managed run
`236ec73c-0f8d-4f1b-8187-6d0204dd0938` passed all 33 configured lanes in
17,110 ms with `cache=cold`, `lanes=6`, `test_threads=2`, `build_jobs=2`,
`debug=0`, and `verbose=0`; unchanged-revision run
`077d58d7-b211-4e4a-8c18-3f6539b56eb0` passed in 15,424 ms with
`cache=warm`, `lanes=12`, `test_threads=1`, `build_jobs=1`, `debug=0`, and
`verbose=0`. Both retained the native/WASI capability agreement marker. The
earlier 65,185 ms run was a stale-cache-classification observation, not a
persistent test-body regression. These remain cache- and runner-sensitive
observations; production codec behavior, Pillow manifest rows, parity
fixtures, and coverage origins are unchanged.

Latest exact-head managed validation for implementation/coverage revision
`51c6f7effe8a12649b19cff9fb276476be7232df` passed Pillow parity run
`bcc9ec15-5205-4609-9baa-9977c2dce73f` with 1,445/1,445 checks in 1,925 ms;
the Huffman run-scan checkpoint is Rust-only, so the Pillow oracle surface
remains unchanged. Feature-matrix run
`fd4f69b3-dd5a-45ae-9100-a293c2097c3f` passed all configured lanes in 88,028 ms
with `cache=cold`, `lanes=6`, `test_threads=2`, `build_jobs=2`, `debug=0`, and
`verbose=0`; its retained log contains the native/WASI capability agreement
marker and no `lock-wait` match. Nightly LLVM run
`037c9de9-0936-46db-875d-9e1d697bcc5e` passed 85/85 tests in 91,110 ms and
ingested snapshot `74b84527-6b5c-4bd6-8c28-24c4f2ac07da`, reporting
54,115/54,838 lines, 7,653/7,840 branches, 3,077/3,155 functions, and
83,525/85,134 regions. Compared with the preceding accepted snapshot
`8cc6bdf3-7a44-4681-a731-ad1fb50f949b`, covered/source totals changed by
`+2/+2` lines, `+2/+2` branches, `+0/+0` functions, and `+4/+5` regions.
The known LLVM JSON segment-normalization warning remains; the aggregate
shortfall is 723 lines, 187 branches, 78 functions, and 1,609 regions. In
`src/codecs/webp/native/encoder.rs`, coverage is 2,016/2,093 lines,
447/472 branches, 93/93 functions, and 2,990/3,239 regions; the new
run-scan loop and 64-symbol boundary branch are covered. Existing unrelated
gaps remain named by the bounded coverage query; no synthetic or
coverage-only test was added.

Prior exact-head managed validation for implementation/coverage revision
`6207722d23e014ed4fda9e2045499500d59b3c7c` passed Pillow parity run
`2eee7cb3-54e8-43c1-877e-729ced549cdb` with 1,445/1,445 checks in 4,165 ms;
the segment-clustering work is Rust-only, so the Pillow oracle surface remains
unchanged. Feature-matrix run `d3ce5ad1-06de-426f-9663-9335b7bdd583` passed
all 33 configured lanes in 50,364 ms with `cache=cold`, `lanes=6`,
`test_threads=2`, `build_jobs=2`, `debug=0`, and `verbose=0`; its retained log
has the native/WASI capability agreement marker and no `lock-wait` match.
Nightly LLVM run `1b56c2c7-5541-476b-a9cd-53eb804890c8` passed 85/85 tests in
58,069 ms and ingested snapshot
`4d159e5e-c0b7-42e6-bf10-cff246a448b2`, reporting 54,094/54,817 lines,
7,649/7,836 branches, 3,075/3,153 functions, and 83,501/85,105 regions.
Compared with the preceding accepted snapshot
`74d32c64-7231-448b-9a23-b8fabc4f70c2`, covered/source totals changed by
`+13/+14` lines, `+3/+4` branches, `+2/+2` functions, and `+20/+21`
regions. The known LLVM JSON segment-normalization warning remains; the
aggregate shortfall is 723 lines, 187 branches, 78 functions, and 1,604
regions. In `src/codecs/webp/encode/vp8/analysis.rs`, coverage is 521/522
lines, 41/42 branches, 31/31 functions, and 851/852 regions; line 367 has a
partial branch and line 369 is uncovered for the trailing partial-chunk path.
That named aggregate gap is retained without a synthetic or coverage-only
test.

Prior exact-head managed validation for implementation/coverage revision
`2f957016e8b52d1e76a4de3a04fa54e88f1f6dd8` passed Pillow parity run
`0913abdc-b1f7-4a37-b697-d7d35d29139b` with 1,445/1,445 checks in 3,728 ms;
the new Rust-only work-control evidence therefore leaves the Pillow oracle
surface unchanged. Feature-matrix run
`9ba9bd95-5b7f-49ca-89c5-1a127657ea1c` passed all configured lanes in
65,185 ms with `cache=warm`, `lanes=12`, `test_threads=1`, `build_jobs=1`,
`debug=0`, and `verbose=0`; its retained log records the native/WASI
capability agreement marker and no `lock-wait` match.
The later harness revision `8c531be322c6234b4694e9353164587f8c79b4ba`
passed the same 33 lanes in 17,110 ms during the source-changing run and
15,424 ms on the unchanged warm rerun above.

Nightly LLVM run
`afea191b-ffaa-48eb-89d5-41c94592ea6a` passed 85/85 tests in 79,822 ms and
ingested snapshot `74d32c64-7231-448b-9a23-b8fabc4f70c2`, reporting
54,081/54,803 lines, 7,646/7,832 branches, 3,073/3,151 functions, and
83,481/85,084 regions. Compared with the preceding accepted snapshot, the
covered/source totals changed by +182/+183 lines, +24/+24 branches, +14/+14
functions, and +299/+308 regions. The known LLVM JSON segment-normalization
warning remains; the strict aggregate shortfall is 722 lines, 186 branches,
78 functions, and 1,603 regions. These are implementation, target-matrix, and
Pillow-oracle records with separate evidence ownership.

The lossy WebP VP8 coefficient-statistics slice extends the same existing
`encode_work_budget_is_a_non_parity_result_contract`: token-aware statistics
collection polls after each 1,024 selected macroblocks, while the no-token path
retains its original tight traversal. A deterministic 512×512 RGB probe
preserves exact bytes under the ample policy and rejects at `maximum: 712`,
`observed: 713` in both whole-buffer and direct-sink paths; the sink sentinel
`[0xAA]` remains untouched. This is Rust-only caller-budget evidence because
Pillow has no caller token, typed work-budget result, or sink/rollback
contract, so it adds no parity row, fixture-manifest row, diagnostic origin,
new test function, or coverage-only hook.

The lossy WebP VP8 filter-edge adjustment slice also extends this existing
contract: method 3 enables the adjustment path, whose token-aware scan polls
after each 1,024 selected macroblocks. The 512×512 RGB feature-gate probe
rejects at `maximum: 719`, `observed: 720` in both whole-buffer and direct-sink
paths, leaving sentinel `[0xD3]` untouched; the no-token path retains its
original tight adjustment pass. Pillow exposes no caller token, typed
work-budget result, or sink/rollback contract, so this adds no parity row,
fixture-manifest row, diagnostic origin, new test function, or coverage-only
hook.

The first-partition WebP VP8 segment-probability prepass now polls after each
1,024 selected macroblocks while collecting the four segment counts. The same
existing feature-gated contract reaches this Rust-only boundary at
`maximum: 332`, `observed: 333` in both whole-buffer and direct-sink paths;
the sink sentinel `[0xAE]` remains untouched. The no-token path retains its
original tight count pass. Pillow exposes no caller token, typed work-budget
result, or sink/rollback contract, so this adds no parity row, fixture-manifest
row, diagnostic origin, new test function, or coverage-only hook.

The token-aware VP8L meta-histogram sampling path now polls row/column
comparisons and symbol compaction after each 1,024 symbols. The existing
16,384×16 RGBA feature-gate probe reaches 1,537 equal tile symbols across
adjacent rows and rejects at `maximum: 967,091`, `observed: 967,092` in both
whole-buffer and caller-owned-sink paths; the sink remains `[0xB7]` untouched.
No-token sampling loops retain their original iterators. Pillow has no caller
token, typed work-budget result, or caller-owned sink/rollback contract, so
this adds no parity row, fixture-manifest row, diagnostic origin, new test
function, or coverage-only hook.

The token-aware VP8L Huffman-node ordering path now uses a stable bottom-up
merge sort and polls after each 64 comparisons; the no-token path retains the
original stable sort. The existing 128-entry palette feature-gate fixture
rejects at `maximum: 2,412`, `observed: 2,413` in both whole-buffer and
caller-owned-sink paths before structural delivery; the sink sentinel `[0xC6]`
remains untouched. Pillow has no caller token, typed work-budget result, or
caller-owned sink/rollback contract, so this adds no parity row,
fixture-manifest row, diagnostic origin, new test function, or coverage-only
hook.

The predictor mode-application path now also checkpoints its pre-transform
source snapshot copy after each 1,024 pixels; its no-token path retains the
original bulk clone. The existing predictor-transform probe exercises this
token-aware path before the later transform boundary. This is the same
Rust-only caller-budget contract, not a Pillow-observable result. The
token-aware VP8L backward-reference cost manager also initializes its
pixel-sized cost/length tables after each 1,024 entries; its capacity
reservations retain the no-recoverable-OOM policy. The existing 1,024-pixel
palette-mode sink probe now rejects at `maximum: 5,205`, `observed: 5,206`
before structural delivery and leaves `[0xC3]` untouched.

These are Rust-only work-control results. Pillow cannot exercise a caller token,
typed work-budget result, or caller-owned sink/rollback contract, and it has no
equivalent AVIF grid-topology field. The revision therefore adds no Pillow
parity row, fixture-manifest row, diagnostic origin, new test function, or
coverage-only hook. Pillow parity run
`1eeb8129-ad85-4bfd-8512-ac6c1ef83659` passed 1,445/1,445 checks with zero
skips in 1,598 ms. Feature-matrix run
`e10497b4-59f4-4f59-9241-5754b61c6c2d` passed all 33 configured lanes in
48,013 ms, retained `cache=warm lanes=12 test_threads=1 build_jobs=1 debug=0
verbose=0`, ended with `capability tables OK: every native and wasm32-wasip1
lane agrees`, and had no targeted lock-wait/build-directory/package-cache
matches.

Managed LLVM coverage run `25d73b60-ec57-4691-835a-1fb2c0400e14` passed 85/85
tests in 68,038 ms and ingested snapshot
`2ed526ed-3c23-442f-b7e8-b4209956f8f8`: 53,665/54,326 lines, 7,627/7,790
branches, 3,028/3,106 functions, and 82,985/84,457 regions. Compared with
preceding accepted snapshot `f655e807-2c09-4fe6-80a1-f885717c5e51`, covered/source
totals changed by `+15/+15` lines, `+2/+2` branches, `+2/+2` functions,
and `+21/+18` regions. The known LLVM segment-normalization warning remains;
the strict aggregate shortfall is 661 lines, 163 branches, 78 functions, and
1,472 regions. Coverage is implementation evidence, not Pillow parity; the
AVIF grid provenance and WebP work-control contracts are covered by
feature-gate execution, and no coverage-only test was added. Managed durations
remain cache- and runner-sensitive.

## Historical acceptance record: superseded WebP work-control revisions

For implementation/runtime revision
`dd1f8be02234d89d49f79c23aacf569768ad1b8e`, the current work-budget contract
is the existing feature-gated assertion updated by the lossless VP8L palette
index-lookup checkpoint slice committed at
`dd1f8be02234d89d49f79c23aacf569768ad1b8e` and the batched Huffman
code-length-emission slice committed at
`84a9abbd8fca78fc468e3e46be8baa5ca37e005f5`; the preceding work-budget
contract remains the feature-gated assertion from
`52623efa026c775b2d1c5157e10cf485e5fca789`; the candidate-trial prefix-reuse
optimization is committed at `3e139ae7fc5bc1bfaeb3440c4112394cb33eeff3`; the
entropy-analysis checkpoint slice is committed at
`1a8cae394ad0265e4f0a3bf84511b80e7e2a7842`; the entropy-bin clustering
pre-pass checkpoint slice is committed at
`4eae86493bad9016611648a498a81a79f90f5551`; the lossless VP8L palette
sign/nearest-delta scan implementation is committed at
`c36e2472d0366bddd42c55e6ec20d282f8abe068`, with public short-palette fixture
coverage completed at `4a81e987bfac8c9893e9131a772a3eb0cebc63f8`;
runtime harness commit `5af768432579730f01e6af0bf595ac4f02a371df` remains the
active harness revision. The fixture manifest and managed commands report:

| Metric | Count |
| --- | ---: |
| Active fixture rows | 1,417 |
| Decode/inspect/verify fixture rows | 1,024 |
| Encode fixture rows | 393 |
| Planned or unwired fixture rows | 0 |
| Managed Pillow parity checks | 1,445 (1,417 rows + 28 worker functions) |
| Feature-gate assertions per native/WASI lane | 45 |
| Feature-matrix checks | 991 |
| Formats | 8 |

The fixture-row count, managed Pillow parity count, and non-Pillow feature-gate
count are separate evidence surfaces; worker functions do not add fixtures or
Pillow assertions, and feature-gate assertions do not belong to the oracle
matrix.

For the current implementation and test/runtime revision, token-aware WebP
backward-reference tracing now checkpoints the non-saturated interval
split/merge comparisons after each 1,024 interval-work entries, the saturated
cost-interval fallback, and its long length-interval enumeration after each
1,024 entries, while the ordinary no-token path retains its original tight
loops. Token-aware repeated-run hash-chain insertion and long backward-reference
result backfills also charge after each 256 entries; their no-token paths remain
tight. Lossless VP8L entropy-mode analysis
now charges after each 64 symbols while scanning fixed-alphabet histogram costs.
The token-aware VP8L traced backward-reference dynamic-programming pass, path
reconstruction, and token replay now poll after each 256 consumed pixels; its
no-token calls are const-specialized to retain the original 1,024-pixel cadence.
The same Rust-only work-budget contract proves whole-buffer `maximum: 52,493`,
`observed: 52,494` and caller-sink `maximum: 52,492`, `observed: 52,493` on the
patterned 128x128 RGB probe, with sink sentinel `[0xDA]` untouched, followed by
the replay boundaries `52,500/52,501` whole-buffer and `52,499/52,500`
caller-sink with `[0xD9]` untouched. Pillow exposes none of the caller token,
typed work-budget result, caller-owned sink, or rollback semantics, so this adds
no parity row, fixture, diagnostic origin, or coverage-only hook.
The token-aware VP8L token-stream reference walk now polls after each 256
consumed pixels, including every boundary crossed by one Copy token; its no-token
reference loop retains the original tight path. The existing 1×512 constant RGB
probe rejects whole-buffer and caller-owned-sink paths at `maximum: 2,457`,
`observed: 2,458`, with `[0xDC]` untouched. This is Rust-only work-budget
evidence: Pillow has no caller token, typed result, caller-owned sink, or
rollback equivalent, so it adds no parity row, fixture, diagnostic origin, or
coverage-only hook.
Predictor mode application now copies each wide pre-transform source row in
completed 1,024-pixel chunks and polls after each completed chunk; the no-token
path retains its original bulk row copy. The same existing
`encode_work_budget_is_a_non_parity_result_contract` uses a caller-built
4,096×1 RGB probe and rejects whole-buffer and caller-owned-sink paths at
`maximum: 10,728`, `observed: 10,729`, with `[0xDB]` untouched. This remains
Rust-only work-budget evidence because Pillow has no caller token, typed
result, caller-owned sink, or rollback equivalent; it adds no parity row,
fixture, diagnostic origin, new test function, or coverage-only hook.
The token-aware lossless VP8L backward-reference hash-chain candidate scan now
charges after each 64 completed candidate trials across the pass; its no-token
candidate loop retains the original tight path. The existing Rust-only
work-budget contract proves `maximum: 16,254`, `observed: 16,255` in both
whole-buffer and caller-owned-sink paths for the deterministic 160×160
repeated-row RGB probe, with `[0xD6]` untouched.
The token-aware palette-mode VP8L box-chain candidate scan now charges after
each 64 completed low-distance candidate offsets across the pass; its no-token
box-chain loop retains the original tight path. The existing Rust-only
work-budget contract proves `maximum: 600`, `observed: 601` in both paths for
the deterministic 64×64 sixteen-color RGB probe, with `[0xD7]` untouched.
Histogram clustering now charges in both its min/max and bin-assignment
pre-passes after each 64 tile histograms; its ordinary no-token path keeps the
existing algorithm and data.
The existing Rust-only work-budget contract exercises that entropy-analysis and
histogram-clustering pre-pass boundary, histogram population, the RGB-equal grayscale preparation checkpoint
after each 1,024 pixels, the backward-reference length-cost table and equal-cost
interval setup after each 1,024 entries, the token-aware cost-manager
interval-update and cleanup scans after each 256 cumulative interval entries,
the VP8L copy-token cache population and traced copy-token replay scans after
each 256 pixels, the Huffman-tree simple-tree symbol-discovery scan after each
64 code-length slots, Huffman RLE token materialization after each 16 emitted
compressed code-length tokens, the code-length-token frequency scan, the
trailing zero-repeat-token trim scan, and Huffman code-length emission after
each 16 compressed token entries through deterministic feature-gated probes;
these add no Pillow parity row, fixture,
diagnostic origin, or coverage-only hook because Pillow has no caller token,
work-budget result, or caller-owned sink. The VP8L candidate-trial writer now copies the
already-emitted prefix once and retains only each trial suffix, removing the
repeated prefix copy/allocation without changing selected bytes or adding a
new public work-budget result. The entropy-analysis probe proves
`maximum: 19`, `observed: 20` with `[0xAD]` untouched; histogram population
proves `maximum: 58`, `observed: 59` with `[0xB8]`, and the combined
entropy-cost boundary is `maximum: 76`, `observed: 77` with `[0xAE]`.
The histogram-merge boundary is `maximum: 8,254`, `observed: 8,255` with
`[0xAF]` untouched, and the cost-estimate boundary is `maximum: 14,088`,
`observed: 14,089` with `[0xB0]` untouched. Huffman-RLE preparation proves
`maximum: 812`, `observed: 813` for the whole-buffer path and `maximum: 811`,
`observed: 812` with `[0xB1]` untouched for the caller-owned sink. The
grayscale preparation boundary is `maximum: 179`, `observed: 180` in both
paths with `[0xB2]` untouched. The histogram-clustering min/max and
bin-assignment pre-pass boundary is `maximum: 5,309`, `observed: 5,310` with
`[0xB9]` untouched. Huffman-tree frequency remains
`maximum: 43,985`, `observed: 43,986` for the whole-buffer path and
`maximum: 43,984`, `observed: 43,985` with `[0xB3]` untouched for the sink.
The batched code-length-emission contract first proves normal/ample-budget
fixture-byte identity, then rejects both whole-buffer and caller-owned-sink
paths at `maximum: 144,853`, `observed: 144,854`; the sink retains
`[0xB6, 0x52, 0x49, 0x46, 0x46, 0x58, 0xC0, 0x00, 0x00, 0x57, 0x45, 0x42,
0x50]`. The old late cost-manager and trailing-trim maxima were tied to the
previous per-token polling schedule and are superseded by the batched emission
poll. The implementation checkpoints remain current behavior, but those stale
exact thresholds are not claimed. Pillow has no caller token, work-budget
result, or sink contract, so this change adds no parity row, fixture-manifest
row, diagnostic origin, new test function, or coverage-only hook. The cache
probe now rejects both paths at `maximum: 136,672`, `observed: 136,673`; its
sink retains `[0xB5, 0x52, 0x49, 0x46, 0x46, 0x9C, 0x04, 0x00, 0x00, 0x57,
0x45, 0x42, 0x50]`. The lossless VP8L palette ordering path keeps its no-token helper
byte-preserving and charges token-aware sign collection and nearest-delta
suffix scans after each 64 palette entries or candidate values. Palette-index
packing keeps its no-token linear lookup byte-preserving and charges the
token-aware lookup after each 64 palette candidates; the deterministic 128×128
RGB probe rejects both whole-buffer and caller-owned-sink paths at
`maximum: 9,804`, `observed: 9,805`, with `[0xA9]` untouched. Managed Pillow
parity run `ffb55fc7-ad94-4d26-b8b3-bcf7a3cf6eaa` passed 1,445/1,445 checks
with zero skips in 1,002 ms. Feature-matrix run
`5e02f54d-a300-4cbd-aaec-8847ec7435ef` passed all 33 configured lanes in
39,841 ms, with `cache=warm lanes=12 test_threads=1 build_jobs=1 debug=0
verbose=0` and the terminal capability agreement; targeted searches returned
no lock-wait, build-directory, or package-cache match. Managed LLVM coverage run
`36d06757-30af-4342-9394-51b3b1f07aa2` passed 85/85 tests in 62,524 ms and
ingested snapshot `d197d0bb-8fab-45d2-b140-45e6db91511a`:
53,219/53,833 lines, 7,532/7,682 branches, 2,998/3,074 functions, and
82,345/83,711 regions. Compared with the preceding accepted snapshot
`3e0b3832-a8b1-4743-bbd7-fcbf06f2137e`, covered/source totals changed by
`+16/+26/+5/+6/+1/+1/+27/+50` for covered/source lines, branches, functions,
and regions. The changed native WebP encoder reports 1,850/1,900 lines,
397/412 branches, 88/88 functions, and 2,707/2,894 regions. The known LLVM
JSON segment-normalization warning remains. The strict aggregate shortfall is
614 lines, 150 branches, 76 functions, and 1,366 regions; coverage is
implementation evidence, not Pillow parity, and no coverage-only test was
added. The palette-index lookup checkpoint adds no Pillow parity row,
fixture-manifest row, diagnostic origin, or new test function because Pillow
has no caller token, work-budget result, or sink contract. Managed durations
remain cache- and runner-sensitive.

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

The same-revision managed run passed all 991 checks in 31,864 ms and recorded
`cache=warm lanes=24 test_threads=1 build_jobs=1 debug=0 verbose=0`;
these are cache- and runner-sensitive execution observations, not a universal
benchmark and not the revision-bound allocation/peak-memory evidence still
required by QA-010 and QA-030. The 131,072-bit contract uses a 768×768 RGB
probe, the 262,144-bit contract uses a 1,024×1,024 high-entropy RGB probe, and
both coefficient-only 524,288-bit and 1,048,576-bit contracts reuse an 832×832
deterministic RGB checkerboard probe; the focused combined contract completed
in 3.12 s in this workspace after removing the separate 1,920×1,920 witness.
These are targeted boundary witnesses, not general benchmarks or claims of
universal codec speedup. The managed durations remain cache- and
runner-sensitive observations.

Historical acceptance context: WebP VP8L 262,144-bit checkpoint

The lossless WebP VP8L work-control slice is implemented at
`cc765b33d2b2846b7f17171292660cc275fb431b`. The existing
`encode_work_budget_is_a_non_parity_result_contract` now extends the
deterministic high-entropy RGB probes through the 262,144-logical-coded-bit
checkpoint. The 128×128 probe proves exact 32,768-bit and 65,536-bit
whole-buffer/direct-sink maximum/observed pairs of `9,287/9,288`,
`9,286/9,287`, `9,288/9,289`, and `9,287/9,288`; the 256×256 probe proves the
131,072-bit pairs `41,439/41,440` and `41,438/41,439`, while the 656×656 probe
proves the 262,144-bit pairs `262,143/262,144` and `262,142/262,143`.
Caller-owned sink sentinels `[0xA2]`, `[0xA1]`, `[0xA0]`, and `[0x9F]` remain
untouched. The probes remain generated from fixed LCGs rather than added as
fixtures. This is Rust-only
resource-contract evidence: Pillow has no caller work budget, typed work-unit
result, or caller-owned sink, so the change adds no parity row, parity fixture,
diagnostic origin, new test function, or coverage-only hook; the unchanged
Pillow run is regression evidence only.

The preceding AVIF item-relationship slice is implemented across
`489351caa15dbdbba7e9c7d41b01a87aebfd457b` and the coverage-fixture
completeness fix `a09bda379ced7abc8b88ba09982de3a4d012ce91`. The existing
`source_alpha_matches_the_container_contract` feature-gated contract now
asserts that `grid.avif` retains the ordered non-alpha `dimg` edges `1`→`2`
and `1`→`3` through inspection, still decode, and sequence-frame decode, while
direct alpha `auxl` edges remain in the dedicated alpha relationship fields.
It uses existing fixtures and an existing test function: no parity row,
fixture, diagnostic origin, or coverage-only hook was added. Pillow has no
source-descriptor or AVIF item-graph result field, so this is Rust-only source
provenance evidence and the unchanged Pillow result remains outer-output
regression evidence.

The preceding WebP VP8L work-control slice is implemented at
`54de3e3f8ded6c889b59416727285297016a891e`; it remains a Rust-only contract
for the same caller-budget/sink reason and is retained below as historical
acceptance context. The preceding GIF high-color nearest-palette work-control slice is implemented at
`6d851a1ca259598c3fa0056c0e3b25f7073cea51`. The token-aware path reuses
candidate and merge scratch buffers, charges stable candidate ordering and the
bounded nearest-candidate scan after each 1,024 work items, and leaves the
legacy no-token byte path unchanged. The existing high-color contract proves
ample-budget byte identity and typed `EncodeWorkUnits` rejection at the new
nearest-palette boundary (`2,048/2,049` whole-buffer and `2,047/2,048`
direct-sink), with untouched sink prefixes. This is Rust-only work-control
evidence: Pillow has no caller token, work-budget result, or caller-owned sink,
so it adds no parity row, fixture, diagnostic origin, new test function, or
coverage-only hook.

The test-only runtime follow-up is committed at
`a819abb48cd6878ec4ae6c4a41e42a038b81a105`. The existing
`incremental_decode_tracks_truncation_progress_per_format` contract still
sweeps every byte boundary and compares `decode_prefix` with legacy `decode`;
it now uses the valid 343-byte `miniswhite_8bit.tiff` and 294-byte
`portable_probe_gray_128.avif` fixtures instead of the 16,506-byte TIFF and
3,077-byte AVIF fixtures. This removes accidental repeated full-raster decode
cost without reducing the per-format boundary assertions. In the same local
workspace the all-feature feature-gate body fell from 3.23 s to 0.79 s, and the
warm managed feature matrix fell from 6,065 ms at the preceding test revision
to 3,231 ms here. These are local execution observations, not universal
benchmarks. The change is Rust-only test-harness evidence: it adds no Pillow
parity row, parity-manifest fixture, diagnostic origin, new test function, or
coverage-only hook; its current coverage delta is recorded above and does not
turn the Rust-only work-control contract into Pillow parity.

The historical WebP VP8L work-control slice before the current 262,144-bit
extension is implemented at
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

The preceding Rust implementation revision also closes API-003. `decode_with_format` and
`decode_with_format_and_policy` validate the complete signature against the
caller-selected `ImageFormat`, preserve the encoded-input limit before
selection, and then use the ordinary feature, inspection, policy, and codec
path. Existing fixture-selected rows in the feature-gate contract compare both
explicit entry points with auto-detecting decode, assert staged mismatch and
unknown-signature errors, retain disabled-feature and WASM AVIF outcomes, and
prove an encoded-byte policy rejection before dispatch. This is a Rust API
contract: Pillow has no caller-supplied format-hint operation, so it adds no
parity row, no new fixture, no diagnostic-origin assignment, and no new test
function.

The preceding Rust implementation revision also closes API-012 and API-013. `EncodedImage` now has an
independent lazy sequence cache, so repeated owned-source sequence materialization
does not repeat codec work or collapse an animated source to its first frame.
`EncodedImageDecodeState` and the still/sequence state accessors distinguish
not-attempted, succeeded, and failed caches. Unlimited compatibility failures are
retained; limited sequence-policy failures use the policy-aware selected-format
path and do not poison the unlimited cache. The existing source-bound fixture
contract proves full sequence ordering, clone-visible cache state, separate
still/sequence state, and policy-failure isolation. This is a Rust-only
source/cache contract: no
Pillow caller API changes, no parity row or fixture, no diagnostic origin, no new
test function, and no coverage-only hook.

The preceding Rust implementation revision also advances API-045. Owned and borrowed source-bound
still and sequence decode now reuse the format validated during construction,
so they skip a second signature-detection scan while preserving the root
auto-detecting APIs, policy checks, and codec parsing behavior. Verification and
codec-specific container parsing remain independent; retaining a parsed
header/index is still a future optimization that requires a proof for every
codec. This is a Rust-only dispatch optimization: no Pillow caller API changes,
parity row or fixture, diagnostic origin, new test function, or coverage-only
hook.

The source memory contract is a retained-payload model rather than an allocator
benchmark: an owned source retains one shared encoded-byte snapshot, inspected
metadata, and each successful still/sequence result independently; clones add
no buffer copy; verification does not populate either cache; and a borrowed
view owns neither its input nor a persistent cache. Codec parser, decompressor,
and temporary materialization allocations are outside that accounting, so no
Pillow-parity row or synthetic coverage hook is appropriate. Optional eviction,
cached verification, and revision-bound allocator/peak measurements remain
open under API-014 and QA-030.

The preceding Rust implementation revision also closes API-006. `DecodedImage::try_new`,
`try_with_mode`, and `try_with_palette` provide checked zero-copy construction
while retaining `new`, `with_mode`, and `with_palette` as explicitly unchecked
compatibility builders. The existing feature-gate contract proves valid RGB and
indexed construction, palette validation, vector pointer identity, dimensions,
and color/mode error classification. This is a Rust-only defensive-model
contract: no Pillow caller API changes, no parity row or fixture, no diagnostic
origin, no new test function, and no coverage-only hook.

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

The claim ledger (`tests/fixtures/claim_ledger.json`) pins the current
revision-bound tuple: implementation revision
`487348d01389eb8d100b8a668c9921d97634c022`, Pillow manifest SHA-256,
generated-matrix SHA-256, the Coverage MCP run/snapshot identifiers, every
fixture-manifest SHA-256, the VP8L property-map SHA-256
`78a0410d2c7e050e9a5746c3c423d0e70d3f7871735897221765c920cb2096d5`, and
the inspector SHA-256
`833f0926c1a931a24087ae8dea3d199f11e6c236c50f90c97ae657aac40af541`.
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

The non-primary AVIF item-color contract is also separate from Pillow parity.
The existing `alpha.avif` fixture is mutated only in memory to add the primary
item's typed `colr`/`nclx` property association to auxiliary item 2. The
contract asserts source-local item identity and exact CICP values on inspect,
still decode, and fallback-sequence decode, preserves the primary
`SourceColor`, and proves decoded pixels are unchanged. Pillow's observable
schema has no item-level color declaration, so this is Rust
source-provenance/specification evidence: it adds no parity row or new fixture
file,
diagnostic origin, new test function, or coverage-only hook. Raw non-primary
`prof`/`rICC` profiles are retained separately through
`SourceDescriptor::avif_item_icc_profiles()`; other item color/property forms
remain open.

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

The current AVIF unknown-item-property slice is also separate from Pillow
parity. The existing `source_alpha_matches_the_container_contract` test
mutates `alpha.avif` only in memory to associate a `zzzz` property with item 2,
then checks the exact item ID, four-byte kind, and payload on inspection, still
decode, and sequence-frame decode while proving decoded pixels are unchanged.
Pillow has no item-level property result, so this is Rust
source-provenance/specification evidence: no parity row, fixture file,
diagnostic origin, new test function, or coverage-only hook was added.

The current AVIF plane-declaration slice is also separate from Pillow parity.
The existing `source_alpha_matches_the_container_contract` test asserts the
real `alpha.avif` and `grid.avif` fixture declarations on inspection, still
decode, and sequence-frame decode, then mutates `ispe` and `pixi` associations
only in memory to prove duplicate declarations are malformed. Pillow has no
item-level plane declaration field, so this is Rust source-provenance evidence:
no parity row, fixture file, diagnostic origin, new test function, or
coverage-only hook was added. Aggregate LLVM execution remains implementation
coverage, not Pillow-parity coverage.

The current AVIF codec-declaration slice is also separate from Pillow parity.
The existing `source_alpha_matches_the_container_contract` test asserts the
real `alpha.avif` and `grid.avif` `av1C` payloads, item IDs, declared depth,
and chroma position on inspection, still decode, and sequence-frame decode,
then mutates an `av1C` association only in memory to prove duplicate
declarations are malformed. Pillow has no item-level codec-configuration
field, so this is Rust source-provenance evidence: no parity row, fixture file,
diagnostic origin, new test function, or coverage-only hook was added.
The exact raw payload is retained alongside the typed fields; aggregate LLVM
execution remains implementation coverage, not Pillow-parity coverage.

Historical acceptance for production implementation revision
`9e2ffcc5b190c4044c08b0496bafe30b918561f8` and test/runtime revision
`15965fbda46db35dc4b9f547d757ee9c6ac20ec0`: managed Pillow parity run
`9cd5adbc-b0c2-4edd-96b8-1539f3374182` passed 1,445/1,445 checks in 936 ms;
feature-matrix run `50e0cb86-c8fe-49b4-b43b-e529e349b552` passed with
`cache=cold`, `lanes=6`, `test_threads=2`, `build_jobs=2`, `debug=0`, and
`verbose=0`, and its retained log ended with `capability tables OK: every
native and wasm32-wasip1 lane agrees` with no `lock-wait` match. Nightly LLVM
run `d91b95a2-ff4c-4fce-b11c-ce2d19ab392c` passed 85/85 tests in 47,468 ms and
ingested snapshot `2ff3c38e-1d61-4aa0-98e9-d444d67cb809`: 54,750/55,600
lines, 7,794/7,998 branches, 3,110/3,201 functions, and 84,415/86,308
regions. The known LLVM JSON segment-normalization warning remains. These
coverage numbers are Rust implementation evidence, not Pillow parity, and no
coverage-only test was used.

The historical test-runtime-only follow-up removed one redundant ample-budget
comparison from the existing Rust-only work-budget contract and replaces its
late VP8L bitstream probe with a deterministic 248x248 high-entropy probe.
The compact probe still reaches the 262,144-, 524,288-, and 1,048,576-bit
boundaries with the same whole-buffer/sink observations: 262,602/262,603 and
262,601/262,602; 328,138/328,139 and 328,137/328,138; and 459,210/459,211
and 459,209/459,210. Existing rejection and untouched-sentinel assertions
remain. No production codec behavior, Pillow parity row or fixture,
diagnostic origin, new test function, or coverage-only hook changed.

The clean schema-@2 local workload protocol at this revision passed all four
workloads: the Pillow parity fixture suite took 0.973 s wall time (reported
`real=0.96`) and the separate Rust-only feature-gate suite took 1.683 s wall
time (reported `real=1.68`); the native release and wasm32-unknown-unknown
compile workloads also passed. These are fixed-host/cache observations, not
universal speed claims. The Rust-only contract and benchmark remain separate
from Pillow-oracle evidence.

Historical acceptance record: WebP VP8L 2,097,152-bit work-budget boundary

The next lossless WebP VP8L logical-bitstream checkpoint is implemented at
`5aa0d77b37a5d81e1149e5169915ce21c59b6454`. The existing
`encode_work_budget_is_a_non_parity_result_contract` uses a deterministic
296x296 high-entropy RGB probe and proves exact whole-buffer/direct-sink
rejection at the 2,097,152-bit boundary: maximum/observed
`648,911/648,912` and `648,910/648,911`, with sentinel `[0x9C]` untouched.
The no-token bit-writer path is unchanged. This is Rust-only work-control
evidence: Pillow has no caller token, typed work-budget result, caller-owned
sink, or rollback equivalent. No Pillow parity row, parity fixture, diagnostic
origin, new test function, or coverage-only hook was added.

Local focused and full all-feature tests passed, as did strict all-target
Clippy and rustfmt. Exact-head managed Pillow parity run
`a4839050-7de4-4e3c-8a60-102c75d789f4` passed 1,445/1,445 checks in 640 ms;
feature-matrix run `f3c50fd6-d3ed-4cbd-8a09-9d405d5a88f8` passed in 52,030 ms
with `cache=cold`, `lanes=6`, `test_threads=2`, `build_jobs=2`, `debug=0`,
and `verbose=0`, ending with native/WASI capability agreement and no
`lock-wait` match. Nightly LLVM run
`1ee268ab-2660-4094-b7d2-504846bec32f` passed 85/85 tests in 64,265 ms and
ingested snapshot `9427f8d2-a9e8-4698-92d1-b0c06f0f855e`: 54,755/55,605
lines, 7,796/8,000 branches, 3,110/3,201 functions, and 84,422/86,315
regions. The known LLVM JSON segment-normalization warning remains. The
schema-@2 local benchmark separately measured the Pillow parity fixture suite
at 1.075 s wall time and the Rust-only feature-gate suite at 1.874 s; these
are fixed-host observations, not universal performance claims.

Historical acceptance record: WebP work-budget witness runtime

The test-only runtime slice is implemented at
`35cf266552fa4cfaaef1e231bb01bead1c00d99b`; production behavior remains at
`5aa0d77b37a5d81e1149e5169915ce21c59b6454`. The existing
`encode_work_budget_is_a_non_parity_result_contract` keeps the exact VP8
262,144-bit whole-buffer/direct-sink pairs `66,879/66,880` and
`66,878/66,879`, but replaces its 1,024×1,024 high-entropy witness with an
81×81 witness. It keeps the exact VP8 coefficient 524,288-bit pairs
`187,405/187,406` and `187,404/187,405`, plus the 1,048,576-bit pairs
`318,670/318,671` and `318,669/318,670`, while replacing the shared 832×832
checkerboard with a 129×129 checkerboard. The 80×80 and 128×128 candidates
were rejected because they no longer reached the respective downstream
coefficient boundary; the accepted smaller witnesses preserve the prior
counter and sentinel assertions.

This is Rust-only test-runtime evidence: Pillow exposes no caller token,
typed work-budget result, caller-owned sink, or rollback equivalent. No
production codec behavior, Pillow parity row or fixture, diagnostic origin,
new test function, or coverage-only hook changed. Local focused/all-feature
tests, strict Clippy, and formatting passed. Exact-head managed Pillow parity
run `d058105b-71d1-4fc5-9bc5-7ea473edbb7c` passed 1,445/1,445 checks in 860 ms.
Feature-matrix run `4f8bbb4f-210a-4b34-91e8-33c1e7589d84` passed all 991/991
checks in 35,692 ms with `cache=cold`, `lanes=6`, `test_threads=2`,
`build_jobs=2`, `debug=0`, and `verbose=0`; its retained log contains the
native/WASI capability agreement marker and no `lock-wait` match. Nightly LLVM
run `4b74e8de-14b9-4438-9a17-7471b12b3ad4` passed 85/85 tests in 58,885 ms and
ingested snapshot `d4a74b4d-3804-4224-b120-430af2cde3ec`: 54,755/55,605
lines, 7,796/8,000 branches, 3,110/3,201 functions, and 84,422/86,315
regions. These coverage totals are Rust implementation evidence, not Pillow
parity, and no coverage-only test was used.

The clean schema-@2 all-workload observation reported 1.39 s parity and
2.01 s Rust-only feature-gate wall time; a warm parity/non-parity repeat
reported 1.08 s and 1.57 s. Native release and wasm32-unknown-unknown compile
workloads also passed. These are fixed-host/cache observations, not universal
speed claims; the Rust-only runtime measurement remains separate from the
Pillow-oracle result.

Historical acceptance record: direct-child peak-RSS benchmark measurement

The benchmark-protocol slice is implemented at
`4415a84463103d3d0916821a3ed8637b832442d6`; production behavior remains at
`5aa0d77b37a5d81e1149e5169915ce21c59b6454`, and the Rust-only feature-gate
contract remains at test/runtime revision
`35cf266552fa4cfaaef1e231bb01bead1c00d99b`. Schema-`@3` of
`scripts/benchmark_fixture_workloads.py` uses POSIX `wait4` instead of relying
on `/usr/bin/time -l`, which can be denied while querying macOS sysctls. It
records direct-child CPU time and peak RSS per selected workload while keeping
Pillow parity and Rust-only workload provenance in separate records.

The clean revision-bound run passed all four workloads. The Pillow parity
fixture suite reported 1.089135 s wall, 2.860993 user s, 0.222043 sys s, and
251,002,880 bytes peak RSS. The separate Rust-only feature-gate suite reported
1.588432 s wall, 2.140313 user s, 0.100214 sys s, and 151,781,376 bytes peak
RSS. The native release `rlib` was 7,970,888 bytes and the
`wasm32-unknown-unknown` determinism artifact was 25,080,461 bytes. Peak RSS
is a direct-child POSIX observation, not a universal process-tree, allocator,
or memory claim.

This changes measurement tooling only: no production codec behavior, Pillow
parity row or fixture, diagnostic origin, new test function, or coverage-only
hook changed. Allocation counts, retained encoded/decoded cache bytes,
caller-buffer reuse, peak stack, and WASM runtime time/memory remain open under
QA-010/QA-030.

Historical acceptance record: WebP VP8L palette-mode box-chain candidate offsets

The token-aware lossless WebP VP8L palette-mode box-chain scan is implemented
at production and test/runtime revision
`b29a8f3ef9c34063d1311aaebced5f61423d1818`. The existing
`encode_work_budget_is_a_non_parity_result_contract` now counts completed
low-distance candidate offsets across the box-chain pass and polls after each
64 offsets in the token path; the no-token box-chain loop remains a separate
tight path. Its deterministic 64×64 sixteen-color RGB probe preserves ordinary
bytes under an ample budget and rejects at `maximum: 600`, `observed: 601` in
both the whole-buffer and caller-owned-sink paths, with sink sentinel `[0xD7]`
untouched. The boundary is a real palette-mode box-chain work dimension, not a
synthetic coverage hook or a Pillow-parity claim.

Pillow exposes no caller token, typed work-budget result, caller-owned sink, or
rollback equivalent, so this is Rust-only resource-contract evidence. It adds
no Pillow parity row, fixture-manifest row, diagnostic origin, new test
function, or coverage-only hook. Local focused/full all-feature tests, strict
all-target Clippy, and rustfmt passed. Exact-head managed Pillow parity run
`527419c8-2963-4436-902c-e00e89d740c0` passed 1,445/1,445 checks in 1,020 ms.
Feature-matrix run `29aac451-8657-4382-b82e-8899a602c4eb` passed all configured
native/WASI lanes in 58,639 ms with `cache=cold`, `lanes=6`, `test_threads=2`,
`build_jobs=2`, `debug=0`, and `verbose=0`; its retained log contains the
native/WASI capability agreement marker and no `lock-wait` match. Nightly LLVM
run `a73364f0-0f4c-47ed-9fa8-beb218ebb88a` passed 85/85 tests in 69,182 ms and
ingested snapshot `003d69d6-820f-4bcc-a28f-af6be3327207`: 54,838/55,682 lines,
7,832/8,038 branches, 3,112/3,203 functions, and 84,546/86,427 regions. These
are Rust implementation/coverage records, not Pillow-parity coverage; the
known LLVM JSON segment-normalization warning remains. Remaining progress
semantics, interruption inside one candidate or other codec unit, transient
allocation accounting, and short-write/rollback cleanup remain open.

Historical acceptance record: uncapped encode-work policy token fast path

The production/test/runtime slice is implemented at
`8361ceb5c1b69c75ea6555a01c9908fe5b37ac78`. `work_budget_token` now treats
`EncodePolicy::max_work_units() == Some(u64::MAX)` as observationally uncapped:
it preserves the token-aware codec and cancellation path while reusing the
source token (or a plain uncapped token) instead of allocating and mutating a
redundant work-budget cell at every checkpoint. Finite work budgets retain the
same budget token, counter, and typed `EncodeWorkUnits` rejection semantics.
This is a Rust runtime/control optimization; it changes no encoded bytes,
finite-budget boundary, Pillow oracle result, parity row, fixture, diagnostic
origin, test function, or coverage-only hook. The existing work-budget
contract already exercises both source-less and caller-token policy paths with
`u64::MAX` ample policies, while finite exact-boundary witnesses remain in
the same feature-gated test.

Local focused/full all-feature tests, strict all-target Clippy, and rustfmt
passed. Exact-head managed Pillow parity run
`c12585d0-52f8-42cb-ba1e-0f0400756aa7` passed 1,445/1,445 checks in 643 ms.
Feature-matrix run `7e39a32b-e939-4105-99f9-b3a82403ba24` passed all configured
native/WASI lanes in 30,968 ms with `cache=cold`, `lanes=6`, `test_threads=2`,
`build_jobs=2`, `debug=0`, and `verbose=0`; its retained log contains the
native/WASI capability agreement marker and no `lock-wait` match. Nightly LLVM
run `12ff4489-ccb8-4b5a-9e5a-f04716ab535b` passed 85/85 tests in 56,905 ms and
ingested snapshot `58a71bc8-925c-4589-9bc5-6b2a92b83f87`: 54,842/55,686 lines,
7,834/8,040 branches, 3,112/3,203 functions, and 84,552/86,433 regions. The
known LLVM JSON segment-normalization warning remains; the aggregate shortfall
is 844 lines, 206 branches, 91 functions, and 1,881 regions. These are Rust
implementation/coverage records, not Pillow-parity coverage.

Historical acceptance record: compact VP8 coefficient-statistics test witness

The test/runtime-only slice is implemented at
`25cee2bb82e43d56cbd6f0b0fd5238d6818f7ff0`; production behavior remains at
`8361ceb5c1b69c75ea6555a01c9908fe5b37ac78`. The existing
`encode_work_budget_is_a_non_parity_result_contract` replaces only its lossy
VP8 coefficient-statistics probe with a 432×432 constant RGB image, the
smallest square public probe that still reaches 27×27 = 729 macroblocks and
the exact `maximum: 712`, `observed: 713` boundary. Whole-buffer and direct-sink
rejections remain asserted, and sink sentinel `[0xAA]` remains untouched.
This reduces test allocation/work while preserving the existing contract and
the no-token production path.

This is Rust-only test-runtime evidence: Pillow exposes no caller token, typed
work-budget result, caller-owned sink, or rollback equivalent. No production
codec behavior, Pillow parity row or fixture, diagnostic origin, new test
function, or coverage-only hook changed. Local focused/full all-feature tests,
strict all-target Clippy, and rustfmt passed. The clean schema-@3 benchmark at
this revision reported 1.185317 s wall / 2.937481 user s / 0.287744 sys s /
257,703,936-byte peak RSS for the Pillow parity fixture suite, and 1.664819 s
wall / 2.211355 user s / 0.139264 sys s / 179,666,944-byte peak RSS for the
separate Rust-only feature-gate suite. Peak RSS is a direct-child POSIX
observation, not a universal process-tree or memory claim.

Exact-head managed Pillow parity run
`da8e5704-a08c-4f09-a641-2a26b13bdf1f` passed 1,445/1,445 checks in 794 ms.
Feature-matrix run `01a5599c-73a8-446a-b14c-9d86394799f1` passed all configured
native/WASI lanes in 32,903 ms; its retained log has the native/WASI capability
agreement marker and no `lock-wait` match. Nightly LLVM run
`3f6c0c42-b74e-409d-8699-503e286eae59` passed 85/85 tests in 53,549 ms and
ingested snapshot `c1e2648d-61b8-4015-b110-173966ae6ac5`: 54,842/55,686 lines,
7,834/8,040 branches, 3,112/3,203 functions, and 84,552/86,433 regions. These
are Rust implementation/coverage records, not Pillow-parity coverage; the
known LLVM JSON segment-normalization warning remains. The aggregate shortfall
is 844 lines, 206 branches, 91 functions, and 1,881 regions.

Current acceptance record: WebP VP8L candidate-result token pool reuse

The production and Rust test/runtime slice is implemented at
`aa65af084624175a0279f42ffe904107e921db8b`, following the preceding
`630baeace17edb64bdc3dc7c5f3e95ea1130baa4` Huffman-tree arena implementation.
Selected VP8L candidate token vectors now return to a
bounded two-vector pool after each image-stream trial; one retained vector can
seed the next cache-selection pass, while active candidates remain owned until
their trial completes. Candidate ordering, encoded bytes, errors, and sink
output remain unchanged. Existing WebP fixture rows (28/13/47), the full
fixture matrix, all 45 feature-gated Rust contracts, strict Clippy, and the
clean benchmark protocol provide the regression evidence. The clean warm-2
benchmark passed the Pillow-parity workload in 1.018434 s wall / 2.930880 user
s / 0.272201 sys s / 295,944,192-byte peak RSS and the separate Rust-only
feature-gate workload in 1.651498 s wall / 2.334869 user s / 0.137705 sys s /
218,955,776-byte peak RSS. The native release `rlib` was 7,998,080 bytes and
the `wasm32-unknown-unknown` determinism artifact was 24,767,822 bytes. These
are host/cache/toolchain observations, not comparative or universal
performance claims. Pillow cannot observe allocator ownership, so the existing
Pillow fixture rows provide byte/error regression only; candidate-result pool
ownership is Rust-only evidence. No parity row, fixture-manifest entry,
diagnostic origin, new test function, or coverage-only hook was added.
Exact-head managed Pillow parity run
`8e5ab7d3-525f-4859-b134-e9dbf771f487` passed 1,445/1,445 checks in 655 ms.
Exact-head feature-matrix run `ba2ad67d-6658-48b2-900d-6d6d4d53fa7a` passed
all configured native/WASI lanes in 37,046 ms; its retained log has the
capability agreement marker and no `lock-wait` match. Both managed runs have no
configured coverage ingestion, so no Coverage MCP metric is claimed.

Current acceptance record: WebP VP8L Huffman-tree arena reuse

The production and Rust test/runtime slice is implemented at
`630baeace17edb64bdc3dc7c5f3e95ea1130baa4`, following the preceding
`4c76598e9bb71133e626f42bfb94bcf1544bfa84` backward-reference scratch reuse.
VP8L Huffman-tree construction now retains a compact index arena per token
stream instead of allocating boxed child nodes for every merge. The
cancellation-aware stable ordering path copies weighted node indices rather
than deep-cloning subtrees; arena contents reset before each tree and capacity
survives sequential trees and image streams. Tree ordering, code lengths,
encoded bytes, errors, and sink output remain unchanged. Existing WebP fixture
rows (28/13/47), the full fixture matrix, all 45 feature-gated Rust contracts,
strict Clippy, and the clean benchmark protocol provide the regression
evidence. The clean warm-2 benchmark passed the Pillow-parity workload in
0.999031 s wall / 2.941257 user s / 0.260066 sys s /
292,683,776-byte peak RSS and the separate Rust-only feature-gate workload in
1.666358 s wall / 2.367853 user s / 0.148051 sys s /
231,948,288-byte peak RSS. The native release `rlib` was 7,997,080 bytes and
the `wasm32-unknown-unknown` determinism artifact was 24,763,448 bytes. These
are host/cache/toolchain observations, not comparative or universal
performance claims. Pillow cannot observe allocator ownership, so the existing
Pillow fixture rows provide byte/error regression only; Huffman-tree arena
ownership is Rust-only evidence. No parity row, fixture-manifest entry,
diagnostic origin, new test function, or coverage-only hook was added.
Exact-head managed Pillow parity run
`1e6b0e82-67b8-4955-b6a9-7c90f0e43c89` passed 1,445/1,445 checks in 1,228 ms.
Exact-head feature-matrix run `6e884473-afe3-4603-97da-c5d423aadc86` passed
all configured native/WASI lanes in 59,890 ms; its retained log has the
capability agreement marker and no `lock-wait` match. Both managed runs have no
configured coverage ingestion, so no Coverage MCP metric is claimed.

Current acceptance record: WebP VP8L backward-reference scratch reuse

The production and Rust test/runtime slice is implemented at
`4c76598e9bb71133e626f42bfb94bcf1544bfa84`, following the preceding
`36ce85ba244d7195baef8d5fea7adcdd3cbcc613` histogram-clustering scratch reuse.
Each token stream now retains one candidate workspace: the hash-chain result
table, 18-bit hash-head table, box-chain run counts, source-token buffer,
cost-estimate storage, cache-transform storage, and trace storage reset their
logical contents before reuse across sequential image streams. Selected
candidate token vectors remain independently owned, and nested metadata streams
retain their own workspace. Candidate ordering, encoded bytes, errors, and sink
output remain unchanged. Existing WebP fixture rows (28/13/47), the full fixture
matrix, all 45 feature-gated Rust contracts, strict Clippy, and the clean
benchmark protocol provide the regression evidence. The clean warm-2 benchmark
passed the Pillow-parity workload in 0.929043 s wall / 2.791228 user s /
0.169988 sys s / 251,412,480-byte peak RSS and the separate Rust-only
feature-gate workload in 1.569235 s wall / 2.246073 user s /
0.097882 sys s / 155,566,080-byte peak RSS. The native release `rlib` was
8,006,736 bytes and the `wasm32-unknown-unknown` determinism artifact was
24,815,459 bytes. These are host/cache/toolchain observations, not comparative
or universal performance claims. Pillow cannot observe allocator ownership, so
the existing Pillow fixture rows provide byte/error regression only;
backward-reference scratch ownership is Rust-only evidence. No parity row,
fixture-manifest entry, diagnostic origin, new test function, or coverage-only
hook was added. Exact-head managed Pillow parity run
`cb42a1d3-2a09-4baa-a75e-3614bd30dfab` passed 1,445/1,445 checks in 619 ms.
Exact-head feature-matrix run `465b92d3-87b1-4c70-8cd8-e71cd8d16568` passed all
configured native/WASI lanes in 24,514 ms; its retained log has the capability
agreement marker and no `lock-wait` match. Both managed runs have no configured
coverage ingestion, so no Coverage MCP metric is claimed.

Current acceptance record: WebP VP8L histogram clustering scratch reuse

The production and Rust test/runtime slice is implemented at
`36ce85ba244d7195baef8d5fea7adcdd3cbcc613`, following the preceding
`87a42863ca46c2539aff75d18b85a669f7dac88b` image-stream scratch reuse. Token
streams retain one `HistogramScratch` workspace per image stream; original tile
histograms, cluster copies, the symbol map, and remapped group histograms reset
their logical contents and are reused across sequential candidate streams, while
cache-dependent population lengths resize before each use. Nested metadata
streams retain their own workspace, so histogram state never crosses an active
stream boundary. Clustering order, encoded bytes, errors, and sink output remain
unchanged. Existing WebP fixture rows (28/13/47), the full fixture matrix, all
45 feature-gated Rust contracts, strict Clippy, and the clean benchmark protocol
provide the regression evidence. The clean warm-2 benchmark passed the
Pillow-parity workload in 0.933144 s wall / 2.815010 user s /
0.180552 sys s / 248,283,136-byte peak RSS and the separate Rust-only
feature-gate workload in 1.598661 s wall / 2.270305 user s /
0.112728 sys s / 163,905,536-byte peak RSS. The native release `rlib` was
7,993,648 bytes and the `wasm32-unknown-unknown` determinism artifact was
24,803,389 bytes. These are host/cache/toolchain observations, not comparative
or universal performance claims. Pillow cannot observe allocator ownership, so
the existing Pillow fixture rows provide byte/error regression only; histogram
scratch ownership is Rust-only evidence. No parity row, fixture-manifest entry,
diagnostic origin, new test function, or coverage-only hook was added. Exact-head
managed Pillow parity run `ffc34aad-6216-4b75-8baf-f907746ca9da` passed
1,445/1,445 checks in 602 ms. Exact-head feature-matrix run
`42deab70-5f43-4cdd-a6d2-a54be3923c50` passed all configured native/WASI lanes
in 23,394 ms; its retained log has the capability agreement marker and no
`lock-wait` match. Both managed runs have no configured coverage ingestion, so
no Coverage MCP metric is claimed.

Current acceptance record: WebP VP8L image-stream scratch reuse

The production and Rust test/runtime slice is implemented at
`87a42863ca46c2539aff75d18b85a669f7dac88b`, following the preceding
`8a5a1e5aef3fc44e7cb2a9d956e6395c4389d5a7` hash-chain result storage. Frame,
palette, and alpha substreams now share one bounded image-stream scratch object
per encoder invocation; trial-output and token-stream buffers retain capacity
across sequential streams, including nested metadata streams, while each stream
resets its logical contents before writing. Stream boundaries, candidate
ordering, encoded bytes, errors, and sink output remain unchanged. Existing
WebP fixture rows (28/13/47), the full fixture matrix, all 45 feature-gated Rust
contracts, strict Clippy, and the clean benchmark protocol provide the
regression evidence. The clean warm-2 benchmark passed the Pillow-parity
workload in 0.926429 s wall / 2.784499 user s / 0.184165 sys s /
255,311,872-byte peak RSS and the separate Rust-only feature-gate workload in
1.574040 s wall / 2.264394 user s / 0.094817 sys s /
170,229,760-byte peak RSS. The native release `rlib` was 7,995,656 bytes and
the `wasm32-unknown-unknown` determinism artifact was 24,832,078 bytes. These
are host/cache/toolchain observations, not comparative or universal performance
claims. Pillow remains the byte/error oracle; image-stream scratch ownership is
Rust-only evidence. No parity row, fixture-manifest entry, diagnostic origin,
new test function, or coverage-only hook was added. Exact-head managed Pillow
parity run `57ca3c13-4763-4f32-a13c-d5513772742d` passed 1,445/1,445 checks in
568 ms. Exact-head feature-matrix run
`906235c8-99d0-488a-905a-2f6a7903e151` passed all configured native/WASI lanes
in 26,613 ms; its retained log has the capability agreement marker and no
`lock-wait` match. Both managed runs have no configured coverage ingestion, so
no Coverage MCP metric is claimed.

Current acceptance record: WebP VP8L hash-chain result storage

The production and Rust test/runtime slice is implemented at
`8a5a1e5aef3fc44e7cb2a9d956e6395c4389d5a7`, following the preceding
`26e39ed56ba25159bea3d35cd5cc8045ee3acd06` Huffman traversal fixed-stack
storage. Hash-chain construction now uses the final distance/length result
table as temporary predecessor-link storage during descending best-match
materialization. Each link points to an earlier position, so finalized entries
can be overwritten without affecting later traversal; the result table,
candidate ordering, checkpoint behavior, encoded bytes, errors, and sink output
remain unchanged. Existing WebP fixture rows (28/13/47), the full fixture
matrix, all 45 feature-gated Rust contracts, strict Clippy, and the clean
benchmark protocol provide the regression evidence. The clean warm-2 benchmark
passed the Pillow-parity workload in 0.935276 s wall / 2.795164 user s /
0.192718 sys s / 256,262,144-byte peak RSS and the separate Rust-only
feature-gate workload in 1.784594 s wall / 2.307308 user s / 0.253276 sys s /
170,393,600-byte peak RSS. The native release `rlib` was 7,997,000 bytes and
the `wasm32-unknown-unknown` determinism artifact was 24,833,297 bytes. These
are host/cache/toolchain observations, not comparative or universal performance
claims. Pillow remains the byte/error oracle; hash-chain storage ownership is
Rust-only evidence. No parity row, fixture-manifest entry, diagnostic origin,
new test function, or coverage-only hook was added. Exact-head managed Pillow
parity run `051353b1-c718-46d8-8d44-550a6e9fc52a` passed 1,445/1,445 checks in
624 ms. Exact-head feature-matrix run
`23a172a8-648c-493a-b066-66a1e72ceaed` passed all configured native/WASI lanes
in 14,791 ms; its retained log has the capability agreement marker and no
`lock-wait` match. Both managed runs have no configured coverage ingestion, so
no Coverage MCP metric is claimed.

Current acceptance record: WebP VP8L Huffman traversal fixed-stack storage

The production and Rust test/runtime slice is implemented at
`26e39ed56ba25159bea3d35cd5cc8045ee3acd06`, following the preceding
`da2b9489fc3ac1ffcf94de5f4a685705d80d8702` box-chain storage reuse. Huffman
tree depth traversal now uses a bounded fixed stack sized for the largest VP8L
alphabet instead of allocating a temporary heap vector for each tree. Tree
shape, code lengths, checkpoint behavior, encoded bytes, errors, and sink output
remain unchanged. Existing WebP fixture rows (28/13/47), the full fixture
matrix, all 45 feature-gated Rust contracts, strict Clippy, and the clean
benchmark protocol provide the regression evidence. The clean warm benchmark
passed the Pillow-parity workload in 1.088299 s wall / 3.084727 user s /
0.286170 sys s / 284,033,024-byte peak RSS and the separate Rust-only
feature-gate workload in 1.613453 s wall / 2.308328 user s / 0.107688 sys s /
193,626,112-byte peak RSS. The native release `rlib` was 7,996,704 bytes and
the `wasm32-unknown-unknown` determinism artifact was 24,839,363 bytes. These
are host/cache/toolchain observations, not comparative or universal performance
claims. Pillow remains the byte/error oracle; fixed-stack ownership is Rust-only
evidence. No parity row, fixture-manifest entry, diagnostic origin, new test
function, or coverage-only hook was added. Exact-head managed Pillow parity run
`9d57dbfd-8f64-4a43-911f-994fbad04fce` passed 1,445/1,445 checks in 583 ms.
Exact-head feature-matrix run
`84746d1c-b3cc-4a10-a659-7dad38e728f4` passed all configured native/WASI lanes
in 30,514 ms; its retained log has the capability agreement marker and no
`lock-wait` match. Both managed runs have no configured coverage ingestion, so
no Coverage MCP metric is claimed.

Current acceptance record: WebP VP8L box-chain storage reuse

The production and Rust test/runtime slice is implemented at
`da2b9489fc3ac1ffcf94de5f4a685705d80d8702`, following the preceding
`3c6638abe1e32d33f4cfa8fc00d4fbba3bef4a32` candidate-source token scratch reuse.
The optional low-distance box-chain pass now repopulates the existing primary
hash-chain storage in place after the primary candidate has been consumed,
avoiding a second pixel-sized `(distance, length)` result vector. Existing WebP
fixture rows (28/13/47), the full fixture matrix, all 45 feature-gated Rust
contracts, strict Clippy, and the clean benchmark protocol provide the
regression evidence. The clean warm benchmark passed the Pillow-parity workload
in 0.929563 s wall / 2.758877 user s / 0.163403 sys s /
251,461,632-byte peak RSS and the separate Rust-only feature-gate workload in
1.578949 s wall / 2.245215 user s / 0.093334 sys s /
175,734,784-byte peak RSS. The native release `rlib` was 7,989,624 bytes and
the `wasm32-unknown-unknown` determinism artifact was 24,850,890 bytes. These
are host/cache/toolchain observations, not comparative or universal performance
claims. Pillow remains the byte/error oracle; box-chain storage ownership is
Rust-only evidence. No parity row, fixture-manifest entry, diagnostic origin,
new test function, or coverage-only hook was added. Exact-head managed Pillow
parity run `5e03a0c6-ae7b-49dc-8f06-aab4b6545ec8` passed 1,445/1,445 checks in
623 ms. Exact-head feature-matrix run
`0eae8aea-3241-4fbc-9293-80e07d6ed1fd` passed all configured native/WASI lanes
in 14,955 ms; its retained log has the capability agreement marker and no
`lock-wait` match. Both managed runs have no configured coverage ingestion, so
no Coverage MCP metric is claimed.

Current acceptance record: WebP VP8L candidate-source token scratch reuse

The production and Rust test/runtime slice is implemented at
`3c6638abe1e32d33f4cfa8fc00d4fbba3bef4a32`, following the preceding
`98f1e5e8b154cab176e227e41f7b0bde83d52f7b` trace CostModel histogram reuse.
Candidate construction now reuses one source-token buffer across the
sequential LZ77, RLE, and optional low-distance box-chain candidates. Cache-bit
selection still reads each source independently, and selected candidate vectors
remain independently owned. Existing WebP fixture rows (28/13/47), the full
fixture matrix, all 45 feature-gated Rust contracts, strict Clippy, and the
clean benchmark protocol provide the regression evidence. The clean warm
benchmark passed the Pillow-parity workload in 1.071854 s wall /
3.069491 user s / 0.283397 sys s / 292,110,336-byte peak RSS and the separate
Rust-only feature-gate workload in 1.694006 s wall / 2.401989 user s /
0.159787 sys s / 206,192,640-byte peak RSS. The native release `rlib` was
7,990,432 bytes and the `wasm32-unknown-unknown` determinism artifact was
24,854,581 bytes. These are host/cache/toolchain observations, not comparative
or universal performance claims. Pillow remains the byte/error oracle;
source-token buffer ownership is Rust-only evidence. No parity row,
fixture-manifest entry, diagnostic origin, new test function, or coverage-only
hook was added. Exact-head managed Pillow parity run
`23c83e83-43a6-4b9f-9527-a3dfbb599d9a` passed 1,445/1,445 checks in 1,769 ms.
Exact-head feature-matrix run
`e447924d-e4b3-43a0-8fb8-022278e16a44` passed all configured native/WASI lanes
in 19,249 ms; its retained log has the capability agreement marker and no
`lock-wait` match. Both managed runs have no configured coverage ingestion, so
no Coverage MCP metric is claimed.

Current acceptance record: WebP VP8L trace CostModel histogram reuse

The production and Rust test/runtime slice is implemented at
`98f1e5e8b154cab176e227e41f7b0bde83d52f7b`, following the preceding
`fc56627eb07deb931da462c077ec81dab9c6e702` trace CostManager buffer reuse.
Trace cost-model construction now retains and resets the green histogram and
fixed channel/distance histograms across sequential trace attempts. The
population-cost transformation still runs in place with the same token-aware
checkpoints, and the no-token path remains direct. Existing WebP fixture rows
(28/13/47), the full fixture matrix, all 45 feature-gated Rust contracts,
strict Clippy, and the clean benchmark protocol provide the regression
evidence. The clean warm benchmark passed the Pillow-parity workload in
0.930263 s wall / 2.780541 user s / 0.183454 sys s / 238,485,504-byte peak RSS
and the separate Rust-only feature-gate workload in 1.605898 s wall /
2.267140 user s / 0.116672 sys s / 174,587,904-byte peak RSS. The native
release `rlib` was 7,997,768 bytes and the `wasm32-unknown-unknown` determinism
artifact was 24,860,447 bytes. These are host/cache/toolchain observations, not
comparative or universal performance claims. Pillow remains the byte/error
oracle; CostModel histogram ownership is Rust-only evidence. No parity row,
fixture-manifest entry, diagnostic origin, new test function, or coverage-only
hook was added. Exact-head managed Pillow parity run
`b458bb94-f780-4df6-9782-c45134425418` passed 1,445/1,445 checks in 707 ms.
Exact-head feature-matrix run
`2e8db749-bdf5-4020-b420-869feee0c76f` passed all configured native/WASI lanes
in 41,897 ms; its retained log has the capability agreement marker and no
`lock-wait` match. Both managed runs have no configured coverage ingestion, so
no Coverage MCP metric is claimed.

Current acceptance record: WebP VP8L trace CostManager buffer reuse

The production and Rust test/runtime slice is implemented at
`fc56627eb07deb931da462c077ec81dab9c6e702`, following the preceding
`a7538a957a04efed5950b7ea16ff98b42ebff7da` trace path/output scratch reuse.
Trace setup now retains the CostManager pixel-cost and path-length tables,
match-length cost and equal-cost interval tables, active interval state, and
interval split/rebuild scratch across sequential trace attempts. Each attempt
resets candidate-specific values and preserves the token-aware initialization
checkpoints; the no-token path remains tight. Existing WebP fixture rows
(28/13/47), the full fixture matrix, all 45 feature-gated Rust contracts,
strict Clippy, and the clean benchmark protocol provide the regression
evidence. The clean warm benchmark at final checkout
`f83435351aadf13e0b320dd7a42f830d52c84895` passed the Pillow-parity workload in
1.153682 s wall / 3.423246 user s / 0.247221 sys s / 293,060,608-byte peak RSS
and the separate Rust-only feature-gate workload in 1.782899 s wall /
2.567871 user s / 0.150656 sys s / 231,702,528-byte peak RSS. The native
release `rlib` was 7,996,744 bytes and the `wasm32-unknown-unknown` determinism
artifact was 24,857,623 bytes. These are host/cache/toolchain observations, not
comparative or universal performance claims. Pillow remains the byte/error oracle;
CostManager scratch ownership is Rust-only evidence. No parity row,
fixture-manifest entry, diagnostic origin, new test function, or coverage-only
hook was added. Exact-head managed Pillow parity run
`5db3e841-8bc3-4288-8e5c-ab2160394d33` passed 1,445/1,445 checks in 607 ms.
Exact-head feature-matrix run
`a130342c-215b-4493-b53b-11d93a8ee540` passed all configured native/WASI lanes
in 15,311 ms; its retained log has the capability agreement marker and no
`lock-wait` match. Both managed runs have no configured coverage ingestion, so
no Coverage MCP metric is claimed.

Current acceptance record: WebP VP8L trace path/output scratch reuse

The production and Rust test/runtime slice is implemented at
`a7538a957a04efed5950b7ea16ff98b42ebff7da`, following the preceding
`d7a43f6314b2570baefbc048d0ef532395154f3e` cache-transform output-scratch
reuse. Trace-back candidate improvement now retains the dynamic-programming
cache, path-length reconstruction buffer, and transformed-token output buffer
across sequential trace attempts. A selected trace keeps its token vector
independently owned; a rejected trace or replaced candidate returns its vector
to scratch. Trace ordering, checkpoint behavior, encoded bytes, errors, and
sink output remain unchanged. Existing WebP fixture rows (28/13/47), the full
fixture matrix, all 45 feature-gated Rust contracts, strict Clippy, and the
clean benchmark protocol provide the regression evidence. The clean benchmark
passed the Pillow-parity workload in 1.040943 s wall / 2.796161 user s /
0.205729 sys s / 250,462,208-byte peak RSS and the separate Rust-only
feature-gate workload in 1.591423 s wall / 2.260406 user s / 0.113248 sys s /
165,330,944-byte peak RSS. The native release `rlib` was 7,983,152 bytes and
the `wasm32-unknown-unknown` determinism artifact was 24,866,097 bytes. These
are host/cache/toolchain observations, not comparative or universal
performance claims. Pillow remains the byte/error oracle; trace-scratch
ownership is Rust-only evidence. No parity row, fixture-manifest entry,
diagnostic origin, new test function, or coverage-only hook was added.
Exact-head managed Pillow parity run
`73efdb2e-d5e5-45e3-92df-4211dad892f3` passed 1,445/1,445 checks in 655 ms.
Exact-head feature-matrix run
`4ba0d011-cd39-427b-8368-f7db6477131a` passed all configured native/WASI lanes
in 22,875 ms; its retained log has the capability agreement marker and no
`lock-wait` match. Both managed runs have no configured coverage ingestion, so
no Coverage MCP metric is claimed.

Current acceptance record: WebP VP8L cache-transform output-scratch reuse

The production and Rust test/runtime slice is implemented at
`d7a43f6314b2570baefbc048d0ef532395154f3e`, following the preceding
`2e272b2405ec108fb2b531df07665f0e81c2f1f8` nested metadata output-scratch
reuse. Cache-bit candidate transforms now retain a reusable transformed-token
buffer alongside the existing color-cache table; each trial swaps its output
with the current best candidate, returning the replaced vector to scratch
while keeping only the selected token vector independently owned. Cache-bit
ordering, checkpoint behavior, encoded bytes, errors, and sink output remain
unchanged. Existing WebP fixture rows (28/13/47), the full fixture matrix, all
45 feature-gated Rust contracts, strict Clippy, and the clean benchmark
protocol provide the regression evidence. The clean benchmark passed the
Pillow-parity workload in 0.948031 s wall / 2.776938 user s / 0.193214 sys s /
250,478,592-byte peak RSS and the separate Rust-only feature-gate workload in
1.590595 s wall / 2.264828 user s / 0.096149 sys s /
150,044,672-byte peak RSS. The native release `rlib` was 7,979,320 bytes and
the `wasm32-unknown-unknown` determinism artifact was 24,848,101 bytes. These
are host/cache/toolchain observations, not comparative or universal
performance claims. Pillow remains the byte/error oracle; transformed-token
ownership is Rust-only evidence. No parity row, fixture-manifest entry,
diagnostic origin, new test function, or coverage-only hook was added.
Exact-head managed Pillow parity run
`74d6b9ee-d771-4ad3-99c4-27a17c9512f7` passed 1,445/1,445 checks in 579 ms.
Exact-head feature-matrix run
`64fb001e-366c-43d9-8dc4-7ac507e945ce` passed all configured native/WASI lanes
in 21,842 ms; its retained log has the capability agreement marker and no
`lock-wait` match. Both managed runs have no configured coverage ingestion, so
no Coverage MCP metric is claimed.

Current acceptance record: WebP VP8L nested metadata output-scratch reuse

The production and Rust test/runtime slice is implemented at
`2e272b2405ec108fb2b531df07665f0e81c2f1f8`, following the preceding
`e9aabbc0cc1f4cd208f1b63be74b065809d1f5d7` nested metadata-stream scratch
reuse. The configured image-stream helper now carries a separate output
scratch buffer; nested metadata candidate trials reuse losing suffix storage
and return the winning suffix capacity to that buffer after delivery. Candidate
selection, checkpoint ordering, encoded bytes, errors, and sink output remain
unchanged. Existing WebP fixture rows (28/13/47), the full fixture matrix, all
45 feature-gated Rust contracts, strict Clippy, and the clean benchmark
protocol provide the regression evidence. The clean benchmark passed the
Pillow-parity workload in 1.051700 s wall / 2.796538 user s / 0.217463 sys s /
246,251,520-byte peak RSS and the separate Rust-only feature-gate workload in
1.809399 s wall / 2.292459 user s / 0.280494 sys s /
176,734,208-byte peak RSS. The native release `rlib` was 7,977,792 bytes and
the `wasm32-unknown-unknown` determinism artifact was 24,845,082 bytes. These
are host/cache/toolchain observations, not comparative or universal
performance claims. Pillow remains the byte/error oracle; output-scratch
ownership is Rust-only evidence. No parity row, fixture-manifest entry,
diagnostic origin, new test function, or coverage-only hook was added.
Exact-head managed Pillow parity run
`dedb5e2e-8124-4aa2-8e9c-c7db7e998db8` passed 1,445/1,445 checks in 648 ms.
Exact-head feature-matrix run
`db97b171-6f32-477c-84c1-335a1f323098` passed all configured native/WASI lanes
in 21,489 ms; its retained log has the capability agreement marker and no
`lock-wait` match. Both managed runs have no configured coverage ingestion, so
no Coverage MCP metric is claimed.

Current acceptance record: WebP VP8L nested metadata-stream scratch reuse

The production and Rust test/runtime slice is implemented at
`e9aabbc0cc1f4cd208f1b63be74b065809d1f5d7`, following the preceding
`a15a4c5840d51cd7bc451846ee0ff9d4aad144f7` Huffman node/merge scratch reuse.
The configured image-stream helper now accepts retained token-stream scratch;
multi-group token streams keep an optional boxed child scratch for the sampled
metadata image across outer candidate trials, while the metadata stream
disables further recursion. Candidate suffix ownership, parent-writer prefix
state, checkpoint ordering, encoded bytes, errors, and sink output remain
unchanged. Existing WebP fixture rows (28/13/47), the full fixture matrix, all
45 feature-gated Rust contracts, strict Clippy, and the clean benchmark
protocol provide the regression evidence. The clean benchmark passed the
Pillow-parity workload in 0.948142 s wall / 2.796201 user s / 0.193921 sys s /
246,923,264-byte peak RSS and the separate Rust-only feature-gate workload in
1.608785 s wall / 2.269095 user s / 0.131215 sys s /
147,193,856-byte peak RSS. The native release `rlib` was 7,978,808 bytes and
the `wasm32-unknown-unknown` determinism artifact was 24,842,828 bytes. These
are host/cache/toolchain observations, not comparative or universal
performance claims. Pillow remains the byte/error oracle; nested-scratch
ownership is Rust-only evidence. No parity row, fixture-manifest entry,
diagnostic origin, new test function, or coverage-only hook was added.
Exact-head managed Pillow parity run
`69d78c49-273a-4ddf-9dc3-29129e71a3cd` passed 1,445/1,445 checks in 605 ms.
Exact-head feature-matrix run
`72f7f069-aaf1-4ff5-9d12-ede748ac7085` passed all configured native/WASI lanes
in 27,884 ms; its retained log has the capability agreement marker and no
`lock-wait` match. Both managed runs have no configured coverage ingestion, so
no Coverage MCP metric is claimed.

Current acceptance record: WebP VP8L Huffman node/merge scratch reuse

The production and Rust test/runtime slice is implemented at
`a15a4c5840d51cd7bc451846ee0ff9d4aad144f7`, following the preceding
`6e243f7e92becc664cf3d17e68fcecf25a873863` meta-pixel scratch reuse. Huffman
construction retains the leaf-node vector and token-aware merge-sort buffer
across sequential tree builds; recursive boxed nodes remain per-tree owned
and the traversal stack remains local. Ordering, checkpoint behavior, tree
selection, encoded bytes, errors, and sink output remain unchanged. Existing
WebP fixture rows (28/13/47), the full fixture matrix, all 45 feature-gated
Rust contracts, strict Clippy, and the clean benchmark protocol provide the
regression evidence. The clean benchmark passed the Pillow-parity workload in
1.727992 s wall / 3.779852 user s / 0.347120 sys s /
288,571,392-byte peak RSS and the separate Rust-only feature-gate workload in
2.832432 s wall / 3.168662 user s / 0.185820 sys s /
238,272,512-byte peak RSS. The native release `rlib` was 7,981,064 bytes and
the `wasm32-unknown-unknown` determinism artifact was 24,831,896 bytes. These
are host/cache/toolchain observations, not comparative or universal
performance claims. Pillow remains the byte/error oracle; node/merge vector
ownership is Rust-only evidence. No parity row, fixture-manifest entry,
diagnostic origin, new test function, or coverage-only hook was added.
Exact-head managed Pillow parity run
`9ebf2cee-a617-46b9-a7a1-3173cb959602` passed 1,445/1,445 checks in 1,162 ms.
Exact-head feature-matrix run
`15175037-4f06-4e3e-b463-17499189d740` passed all configured native/WASI lanes
in 43,478 ms; its retained log has the capability agreement marker and no
`lock-wait` match. Both managed runs have no configured coverage ingestion, so
no Coverage MCP metric is claimed.

Current acceptance record: WebP VP8L meta-pixel scratch reuse

The production and Rust test/runtime slice is implemented at
`6e243f7e92becc664cf3d17e68fcecf25a873863`, following the preceding
`058e6b7dc89dd59b96f3d06343d9e296af7006b0` Huffman-RLE mask scratch reuse.
Multi-group token streams clear and refill one meta-pixel materialization
buffer across candidate trials; the recursive meta-stream write consumes it
before the next candidate, so metadata grouping, checkpoint behavior, encoded
bytes, errors, and sink output remain unchanged. Existing WebP fixture rows
(28/13/47), the full fixture matrix, all 45 feature-gated Rust contracts,
strict Clippy, and the clean benchmark protocol provide the regression
evidence. The clean benchmark passed the Pillow-parity workload in 0.942339 s
wall / 2.799787 user s / 0.184515 sys s / 253,739,008-byte peak RSS and the
separate Rust-only feature-gate workload in 1.740961 s wall / 2.320372 user s /
0.212541 sys s / 169,181,184-byte peak RSS. The native release `rlib` was
7,973,968 bytes and the `wasm32-unknown-unknown` determinism artifact was
24,793,428 bytes. These are host/cache/toolchain observations, not comparative
or universal performance claims. Pillow remains the byte/error oracle;
meta-pixel buffer ownership is Rust-only evidence. No parity row,
fixture-manifest entry, diagnostic origin, new test function, or coverage-only
hook was added. Exact-head managed Pillow parity run
`9e87d06e-9bba-4e14-9b4c-e4378ea3d492` passed 1,445/1,445 checks in 725 ms.
Exact-head feature-matrix run `a40d1a78-d1ae-437d-bd56-db097b81f0f4` passed
all configured native/WASI lanes in 22,001 ms; its retained log has the
capability agreement marker and no `lock-wait` match. Both managed runs have
no configured coverage ingestion, so no Coverage MCP metric is claimed.

Current acceptance record: WebP VP8L Huffman-RLE mask scratch reuse

The production and Rust test/runtime slice is implemented at
`058e6b7dc89dd59b96f3d06343d9e296af7006b0`, following the preceding
`401e9ab847eb717dd29515ccd5c8c8fe1f9cb621` Huffman symbol-array reuse.
Huffman-RLE preparation resizes and clears one boolean good-mask buffer across
sequential channel and histogram-group tree builds; token-aware and no-token
RLE decisions, checkpoint behavior, tree selection, encoded bytes, errors, and
sink output remain unchanged. Existing WebP fixture rows (28/13/47), the full
fixture matrix, all 45 feature-gated Rust contracts, strict Clippy, and the
clean benchmark protocol provide the regression evidence. The clean benchmark
passed the Pillow-parity workload in 0.940543 s wall / 2.819080 user s /
0.183555 sys s / 246,939,648-byte peak RSS and the separate Rust-only
feature-gate workload in 1.594874 s wall / 2.266260 user s / 0.104678 sys s /
178,700,288-byte peak RSS. The native release `rlib` was 7,967,040 bytes and
the `wasm32-unknown-unknown` determinism artifact was 24,800,227 bytes. These
are host/cache/toolchain observations, not comparative or universal
performance claims. Pillow remains the byte/error oracle; mask ownership is
Rust-only evidence. No parity row, fixture-manifest entry, diagnostic origin,
new test function, or coverage-only hook was added. Exact-head managed Pillow
parity run `36fac1e0-ecb9-45d3-ab03-dd4cf3ad1a1f` passed 1,445/1,445 checks in
1,182 ms. Exact-head feature-matrix run
`2bfd3ae0-7b9d-49be-95fd-d74f3cd6cd86` passed all configured native/WASI lanes
in 25,201 ms; its retained log has the capability agreement marker and no
`lock-wait` match. Both managed runs have no configured coverage ingestion, so
no Coverage MCP metric is claimed.

Current acceptance record: WebP VP8L Huffman symbol-array reuse

The production and Rust test/runtime slice is implemented at
`401e9ab847eb717dd29515ccd5c8c8fe1f9cb621`, following the preceding
`6042b77c5c568968295bae030335cb6d9cabb417` optimized-frequency scratch reuse.
Simple-tree symbol discovery stores at most three indices in a fixed array
instead of a heap vector; token-aware and no-token scans preserve their
early-stop behavior and checkpoint schedule. Tree selection, encoded bytes,
errors, and sink output remain unchanged. Existing WebP fixture rows (28/13/47),
the full fixture matrix, all 45 feature-gated Rust contracts, strict Clippy,
and the clean benchmark protocol provide the regression evidence. The clean
benchmark passed the Pillow-parity workload in 0.931031 s wall and the separate
Rust-only feature-gate workload in 1.580142 s wall; these are host/cache/toolchain
observations, not a comparative or universal performance claim. Pillow remains
the byte/error oracle; fixed symbol storage is Rust-only evidence. No parity
row, fixture-manifest entry, diagnostic origin, new test function, or
coverage-only hook was added. Managed checkout validation passed, but its ledger
HEAD predates this commit; no exact-head managed parity, feature-matrix, or
Coverage MCP rerun is claimed at this revision.

Current acceptance record: WebP VP8L optimized-frequency scratch reuse

The production and Rust test/runtime slice is implemented at
`6042b77c5c568968295bae030335cb6d9cabb417`, following the preceding
`b770e3c4238194fa0c65f1490c20d0e8e14380d2` Huffman-token scratch reuse.
Huffman tree construction reuses one optimized-frequency buffer across
sequential channel and histogram-group trees, copying each frequency slice
into retained storage before the existing RLE optimization. Ordinary and
token-aware tree construction, checkpoint sites, encoded bytes, errors, and
sink output remain unchanged. Existing WebP fixture rows (28/13/47), the full
fixture matrix, all 45 feature-gated Rust contracts, strict Clippy, and the
clean benchmark protocol provide the regression evidence. The clean benchmark
passed the Pillow-parity workload in 0.950112 s wall and the separate Rust-only
feature-gate workload in 1.687539 s wall; these are host/cache/toolchain
observations, not a comparative or universal performance claim. Pillow remains
the byte/error oracle; optimized-frequency buffer ownership is Rust-only
evidence. No parity row, fixture-manifest entry, diagnostic origin, new test
function, or coverage-only hook was added. Managed checkout validation passed,
but its ledger HEAD predates this commit; no exact-head managed parity,
feature-matrix, or Coverage MCP rerun is claimed at this revision.

Current acceptance record: WebP VP8L Huffman-token scratch reuse

The production and Rust test/runtime slice is implemented at
`b770e3c4238194fa0c65f1490c20d0e8e14380d2`, following the preceding
`cc00fe4f4e67e40bb9570dedac8d4b185745202f` `GroupCodes` buffer reuse. Huffman
tree writing reuses one compressed code-length token buffer across sequential
channel and histogram-group trees; each tree consumes the buffer before the
next tree clears and refills it. Ordinary and token-aware tree construction,
checkpoint sites, encoded bytes, errors, and sink output remain unchanged.
Existing WebP fixture rows (28/13/47), the full fixture matrix, all 45
feature-gated Rust contracts, strict Clippy, and the clean benchmark protocol
provide the regression evidence. The clean benchmark passed the Pillow-parity
workload in 0.940096 s wall and the separate Rust-only feature-gate workload in
1.610291 s wall; these are host/cache/toolchain observations, not a comparative
or universal performance claim. Pillow remains the byte/error oracle; token
buffer ownership is Rust-only evidence. No parity row, fixture-manifest entry,
diagnostic origin, new test function, or coverage-only hook was added. Managed
checkout validation passed, but its ledger HEAD predates this commit; no
exact-head managed parity, feature-matrix, or Coverage MCP rerun is claimed at
this revision.

Current acceptance record: WebP VP8L `GroupCodes` buffer reuse

The production and Rust test/runtime slice is implemented at
`cc00fe4f4e67e40bb9570dedac8d4b185745202f`, following the preceding
`5d386f0e8d0c4f8780cc59cf3080f9107c0d66c2` trace-cache reuse. Candidate trials
retain each `GroupCodes` object in bounded scratch; its five channel length/code
arrays resize and reset in place, while the active groups remain owned until
all token references have been emitted. Ordinary and token-aware group
construction, checkpoint sites, encoded bytes, errors, and sink output remain
unchanged. Existing WebP fixture rows (28/13/47), the full fixture matrix, all
45 feature-gated Rust contracts, strict Clippy, and the clean benchmark
protocol provide the regression evidence. The clean benchmark passed the
Pillow-parity workload in 0.946648 s wall and the separate Rust-only
feature-gate workload in 1.726289 s wall; these are host/cache/toolchain
observations, not a comparative or universal performance claim. Pillow remains
the byte/error oracle; GroupCodes ownership is Rust-only evidence. No parity
row, fixture-manifest entry, diagnostic origin, new test function, or
coverage-only hook was added. Managed checkout validation passed, but its ledger
HEAD predates this commit; no exact-head managed parity, feature-matrix, or
Coverage MCP coverage rerun is claimed at this revision.

Current acceptance record: WebP VP8L trace-cache reuse

The production and Rust test/runtime slice is implemented at
`5d386f0e8d0c4f8780cc59cf3080f9107c0d66c2`, following the preceding
`ecc5ac4c95a608f3c709fb0de98a89c3f131df59` cache-transform scratch reuse.
The dynamic-programming color cache is reset and reused for token replay after
path reconstruction, removing the second cache-table allocation while leaving
the winning token output independently owned. Ordinary and token-aware trace
ordering, checkpoint sites, encoded bytes, errors, and sink output remain
unchanged. Existing WebP fixture rows (28/13/47), the full fixture matrix, all
45 feature-gated Rust contracts, strict Clippy, and the clean benchmark
protocol provide the regression evidence. Pillow remains the byte/error oracle;
cache ownership is Rust-only evidence. No parity row, fixture-manifest entry,
diagnostic origin, new test function, or coverage-only hook was added, and no
managed parity, feature-matrix, or Coverage MCP rerun is claimed at this
revision.

Current acceptance record: WebP VP8L cache-transform scratch reuse

The production and Rust test/runtime slice is implemented at
`ecc5ac4c95a608f3c709fb0de98a89c3f131df59`, following the preceding
`56efb2215f9f37d412368f43109cd9ebab3bd87e` candidate-estimate scratch reuse.
Sequential cache-bit candidate trials now reuse a bounded zeroed cache table
through `CacheTransformScratch` instead of allocating a new table per trial;
candidate token vectors remain independently owned for the winning trial.
Ordinary and token-aware cache transformation, checkpoint sites, cost decisions,
encoded bytes, errors, and sink output remain unchanged. Existing WebP fixture
rows (28/13/47), the full fixture matrix, all 45 feature-gated Rust contracts,
strict Clippy, and the clean benchmark protocol provide the regression evidence.
Pillow remains the byte/error oracle; scratch ownership is Rust-only evidence.
No parity row, fixture-manifest entry, diagnostic origin, new test function, or
coverage-only hook was added, and no managed parity, feature-matrix, or Coverage
MCP rerun is claimed at this revision.

Current acceptance record: WebP VP8L candidate-estimate scratch reuse

The production and Rust test/runtime slice is implemented at
`56efb2215f9f37d412368f43109cd9ebab3bd87e`, following the preceding
`b9aff15d42432e01f1120f1b7fd9f731ed86101e` CostModel population-buffer reuse.
Sequential cache-bit candidate trials now reuse a bounded green histogram
vector through `CostEstimateScratch` instead of allocating a new green vector
for each estimate. Ordinary and token-aware estimator ordering, checkpoint
sites, cost decisions, encoded bytes, errors, and sink output remain unchanged.
Existing WebP fixture rows (28/13/47), the full fixture matrix, all 45
feature-gated Rust contracts, strict Clippy, and the clean benchmark protocol
provide the regression evidence. Pillow remains the byte/error oracle; scratch
ownership is Rust-only evidence. No parity row, fixture-manifest entry,
diagnostic origin, new test function, or coverage-only hook was added, and no
managed parity, feature-matrix, or Coverage MCP rerun is claimed at this
revision.

Current acceptance record: WebP VP8L CostModel population-buffer reuse

The production and Rust test/runtime slice is implemented at
`b9aff15d42432e01f1120f1b7fd9f731ed86101e`, following the preceding
`e9ee33d589f76f7f4c392d4ae29811db3a7e203f` interval-split scratch reuse.
Fixed-alphabet cost-model population histograms now transform their existing
vectors in place instead of allocating temporary `Vec` values before moving
them into the fixed cost arrays. Ordinary and token-aware cost-model ordering,
checkpoint sites, cost decisions, encoded bytes, errors, and sink output remain
unchanged. Existing WebP fixture rows (28/13/47), the full fixture matrix, all
45 feature-gated Rust contracts, strict Clippy, and the clean benchmark
protocol provide the regression evidence. Pillow remains the byte/error oracle;
buffer ownership is Rust-only evidence. No parity row, fixture-manifest entry,
diagnostic origin, new test function, or coverage-only hook was added, and no
managed parity, feature-matrix, or Coverage MCP rerun is claimed at this
revision.

Current acceptance record: WebP VP8L CostManager interval-split scratch reuse

The production and Rust test/runtime slice is implemented at
`e9ee33d589f76f7f4c392d4ae29811db3a7e203f`, following the preceding
`f974c84d8f04114d24a3914a3517b601645ac4b5` interval-state reuse. Boundary,
addition, overlap, rebuild, and merge vectors are retained as bounded manager
scratch instead of being allocated for each split/rebuild call. Ordinary and
token-aware interval ordering, checkpoint sites, cost decisions, encoded bytes,
errors, and sink output remain unchanged. The existing WebP encode rows
(28/13/47), full fixture matrix, all 45 feature-gated Rust contracts, full
all-feature suite, strict Clippy, rustfmt, and all 33 native/WASI
feature-matrix lanes provide the regression evidence. Pillow remains the
byte/error oracle, while scratch ownership is Rust-only evidence; no parity
row, fixture-manifest entry, diagnostic origin, new test function, or
coverage-only hook was added. No managed parity, feature-matrix, or Coverage
MCP rerun is claimed at this revision.

Current acceptance record: WebP VP8L CostManager interval-state reuse

The production and Rust test/runtime slice is implemented at
`f974c84d8f04114d24a3914a3517b601645ac4b5`, following the preceding
`63dd8b7ebaa7d5c36699d5b9c3278ed32e9253ff` update-state change. Lossless VP8L
CostManager updates no longer allocate a temporary applicable-interval vector;
token-aware cleanup compacts the existing interval vector, and push paths borrow
the immutable length-interval table instead of cloning it per call. Ordinary
and token-aware cost decisions, checkpoint ordering, encoded bytes, errors, and
sink output remain unchanged. The existing WebP encode rows (28/13/47), full
fixture matrix, and 45 feature-gated Rust contracts provide the regression
evidence; allocation ownership is Rust-only because Pillow exposes no caller
budget or allocation contract. No new parity row, fixture-manifest entry,
diagnostic origin, test function, or coverage-only hook was added. No managed
parity, feature-matrix, or Coverage MCP rerun is claimed at this revision.

Current acceptance record: TIFF multi-page sink page-base planning

The production and Rust test/runtime slice is implemented at
`f13a5aa3b99fed752875f67c9a73c27b4f97a538`. Multi-page TIFF sink delivery now
derives each aligned page base from the running delivered position while
relocating pages, removing the temporary page-count-sized base vector. Next-IFD
links, relocated offsets, alignment, overflow checks, encoded bytes, sink
segment boundaries, cancellation, and output-policy behavior remain unchanged.
The existing 57 TIFF Pillow rows and complete fixture matrix provide the
byte/error regression evidence; temporary bookkeeping is Rust-only because
Pillow exposes no allocation or caller-sink contract. All 45 feature-gated Rust
contracts, the full all-feature suite, strict Clippy, rustfmt, and all 33
native/WASI feature-matrix lanes passed locally. No new parity row,
fixture-manifest entry, diagnostic origin, test function, or coverage-only hook
was added. No managed parity, feature-matrix, or Coverage MCP rerun is claimed
at this revision.

Current acceptance record: PNG all-level repeated-row Deflate input planning

The production and Rust test/runtime slice is implemented at
`6e96b2c7f5587543b840bfde78ef0f2a239c1f3c`. PNG no longer builds a temporary
`Vec<usize>` of repeated filtered-row lengths for compression levels 0 through
9; the stored-block and zlib-ng strategies receive the row length and height
directly and preserve the same input-call boundaries in ordinary and
token-aware paths. Encoded bytes, work-budget observations, errors, and sink
delivery remain unchanged. The existing 83 PNG Pillow rows and complete
fixture matrix are the observable byte/error regression evidence; allocation
ownership is a Rust-only implementation boundary because Pillow exposes no
allocation or caller-budget contract. No new parity row, fixture-manifest
entry, diagnostic origin, test function, or coverage-only hook was added. No
managed parity, feature-matrix, or Coverage MCP rerun is claimed at this
revision.

Historical acceptance record: PNG level-six repeated-row Deflate input planning

The production and Rust test/runtime slice is implemented at
`6b6ff5c4c1a4d5998ee4c6c9fe2ff438ed8d77df`. PNG’s default level-six Deflate
path no longer builds a temporary row-length vector; it receives the repeated
filtered-row length and height directly and preserves the same input-call
boundaries in ordinary and token-aware paths. Non-level-six paths retain their
existing chunk-slice representation. Encoded bytes, work-budget observations,
errors, and sink delivery remain unchanged. The existing 83 PNG Pillow rows
and complete fixture matrix are the observable byte/error regression evidence;
the allocation change is a Rust-only implementation boundary because Pillow
exposes no allocation or caller-budget contract. No new parity row,
fixture-manifest entry, diagnostic origin, test function, or coverage-only hook
was added. No managed parity, feature-matrix, or Coverage MCP rerun is claimed
at this revision.

Current acceptance record: TIFF repeated-row Deflate input planning

The production and Rust test/runtime slice is implemented at
`4866fdb1d35a57a1c1f7edf4326bcebbcff0fe51`. TIFF Deflate pages no longer build
a temporary `Vec<usize>` for repeated row lengths; the level-six matcher now
receives the row length and row count directly and replays the same input-call
boundaries in both ordinary and token-aware paths. Encoded bytes, matcher
boundaries, work-budget observations, errors, and sink delivery remain
unchanged. The existing 57 TIFF Pillow rows and the complete fixture matrix
are the observable byte/error regression evidence; the allocation change is a
Rust-only implementation boundary because Pillow exposes no allocation or
caller-budget contract. No new parity row, fixture-manifest entry, diagnostic
origin, test function, or coverage-only hook was added. No managed parity,
feature-matrix, or Coverage MCP rerun is claimed at this revision.

Current acceptance record: WebP VP8L Huffman-RLE reverse-tail scan

The production and Rust test/runtime slice is implemented at
`8b52b7180df0118ed9e427b5df01b906bbe32eaf`. Token-aware Huffman-RLE
preparation scans the fixed code-length alphabet backward to find its last
nonzero slot and charges each 64 scanned entries; the no-token path keeps its
original `rposition` fast path. The existing
`encode_work_budget_is_a_non_parity_result_contract` proves ordinary/ample
byte identity and exact whole-buffer and caller-owned-sink rejection at
`6,710/6,711` on the deterministic sparse-tail probe, with `[0xB2]` untouched
in the sink. Pillow cannot observe the caller token, typed work-budget result,
or caller-owned sink, so this is Rust-only evidence with no parity row,
fixture-manifest entry, diagnostic origin, new test function, or coverage-only
hook. No managed parity, feature-matrix, or Coverage MCP rerun is claimed at
this revision.

Current acceptance record: WebP VP8L Huffman-tree leaf census, materialization, and depth scan

The production and Rust test/runtime slice is implemented at
`a5ac1a14d7ad8f88c9ac60a0da73a94474708cb1`. Token-aware fixed-alphabet
Huffman-tree scans now charge each 64 code-length slots while counting active
symbols, constructing leaf nodes, and checking the resulting maximum depth;
the ordinary no-token path retains the original iterator construction and byte
behavior. The existing
`encode_work_budget_is_a_non_parity_result_contract` proves whole-buffer
`145,330/145,331` and caller-owned-sink `145,335/145,336` rejection on the
generated 128×128 RGB probe. The depth-scan endpoint retains the sentinel
before structural delivery, so this endpoint is not a rollback claim. Pillow has no
caller token, typed work-budget result, or caller-owned sink, so this remains
Rust-only evidence with no parity row, fixture-manifest entry, diagnostic
origin, new test function, or coverage-only hook. No managed parity,
feature-matrix, or Coverage MCP rerun is claimed at this revision.

Current acceptance record: Rust-only work-budget witness runtime cutoff

The existing `run_work_budget_pair` helper is tuned at test/runtime revision
`af98b51bd145ea022687d12ca0ae23abc85334a7`: probes below 64×64 remain
sequential because thread setup costs more than their paired encodes, while
larger independent whole-buffer and caller-owned-sink probes overlap on native
test lanes; WASM remains sequential. This changes test scheduling only. It
adds no production codec behavior, Pillow parity row, fixture-manifest row,
diagnostic origin, new test function, or coverage-only hook. The clean
schema-`@3` benchmark at this revision passed the Pillow parity fixture suite
at 1.090497 s wall / 2.850926 s user / 0.234184 s sys / 248,414,208-byte
direct-child peak RSS, and the separate Rust-only feature-gate suite at
1.690155 s wall / 2.297903 s user / 0.185056 s sys / 185,434,112-byte
direct-child peak RSS. These are host/cache/toolchain observations, not
universal speed or memory claims; managed parity, feature-matrix, and Coverage
MCP reruns remain unclaimed for the production revision `b78d0ff`.

Current acceptance record: WebP VP8L Huffman-RLE token-materialization checkpoint

The production/test/runtime slice is implemented at
`b78d0ffedc3bb193624eb11fd12d68378713489e`. Token-aware code-length Huffman
RLE expansion now materializes compressed tokens through a 16-token checkpoint
boundary; the no-token helper retains the original tight construction path and
ordinary/ample-policy encoded bytes remain unchanged. The existing
`encode_work_budget_is_a_non_parity_result_contract` proves exact whole-buffer
`2,424/2,425` and caller-owned-sink `2,423/2,424` rejections with sentinel
`[0xC9]` untouched. This is Rust-only work-control evidence, not Pillow parity:
Pillow has no caller token, typed work-budget result, caller-owned sink, or
rollback equivalent, so the slice adds no parity row, fixture-manifest row,
diagnostic origin, new test function, or coverage-only hook. No managed parity,
feature-matrix, or Coverage MCP rerun is claimed at this revision.

Current acceptance record: WebP VP8 analysis-buffer reuse and budget-pair runtime tuning

The production/test/runtime slice is implemented at
`9266e4f26749870c1dd680b08598ed6d378ef1c3`. Lossy VP8 analysis now reuses
fixed 16x16 and 8x8 block, prediction, and edge buffers across macroblocks
instead of allocating vectors for each bounded analysis block; the block
geometry and checkpoint semantics are unchanged. The existing
`run_work_budget_pair` helper now keeps compact witnesses sequential, avoiding
thread setup overhead, while retaining native overlap for the large whole-
buffer/direct-sink comparisons; WASM remains sequential. The no-token path
still uses the same analysis algorithm without cancellation checks, and
ordinary and ample-policy encoded bytes remain unchanged.

This is Rust implementation and Rust-only test-runtime evidence. Pillow has no
caller token, typed work-budget result, caller-owned sink, or rollback
equivalent, so the slice adds no Pillow parity row, fixture-manifest row,
diagnostic origin, new test function, or coverage-only hook. The focused and
full 45-test feature-gate suite, 28-function Pillow parity matrix,
all-feature tests/doctests, strict all-target Clippy, and rustfmt passed. The
clean schema-`@3` Rust-only feature-gate workload at this revision measured
1.746872 s wall / 2.315945 user s / 0.188449 sys s / 147,226,624-byte
direct-child peak RSS. This is a host/cache/toolchain observation, not a
universal speed or memory claim; allocator counts, retained-cache size,
caller-buffer reuse, stack depth, and WASM runtime measurements remain open.

Current acceptance record: VP8L candidate-prefix retention and suffix allocation recycling, predictor row-copy, entropy-analysis pixel, traced replay, token-stream, and Huffman-RLE fill checkpoints

The production trace slice is implemented at
`9275f4e6caa394c88fda815543a29411c737f96d`, with the verified Rust witness in
test/runtime revision `9275f4e6caa394c88fda815543a29411c737f96d`. Token-aware VP8L backward-reference dynamic
program tracing, path reconstruction, and token replay now checkpoint every
256 consumed pixels, while the const-specialized no-token path retains its
1,024-pixel cadence. The existing
`encode_work_budget_is_a_non_parity_result_contract` proves the earlier DP
boundaries `52,493/52,494` whole-buffer and `52,492/52,493` direct-sink with
`[0xDA]`, then the new replay boundaries `52,500/52,501` whole-buffer and
`52,499/52,500` direct-sink with `[0xD9]`. An ample token-aware encode remains
byte-identical to the ordinary encode. Native and `wasm32-wasip1` all-feature
feature-gate runs each passed all 45 tests, strict all-target Clippy and
rustfmt passed, and the clean schema-`@3` benchmark passed both suites at this
revision. This is Rust-only work-control evidence: Pillow has no caller token,
typed work-budget result, caller-owned sink, or rollback equivalent, so the
slice adds no parity row, fixture, diagnostic origin, new test function, or
coverage-only hook. The same existing 1x512 constant RGB probe proves the new
token-stream boundary at `maximum: 2,457`, `observed: 2,458` in both
whole-buffer and caller-owned-sink paths, with `[0xDC]` untouched; a single copy
token crosses both 256-pixel boundaries and the no-token reference loop remains
tight.
Predictor mode application now copies each wide pre-transform source row in
completed 1,024-pixel chunks and polls after each completed chunk; its no-token
path keeps the original bulk row copy. The same existing feature-gate contract
uses a caller-built 4,096×1 RGB probe and proves whole-buffer and caller-owned
sink rejection at `maximum: 10,728`, `observed: 10,729`, with `[0xDB]`
untouched. This is Rust-only work-control evidence: Pillow has no caller token,
typed work-budget result, caller-owned sink, or rollback equivalent, so it adds
no parity row, fixture, diagnostic origin, new test function, or coverage-only
hook.

The entropy-analysis follow-up is implemented at
`9c0ff979abd1f26b71e2d3b297fc163d16921d3a`, with its verified Rust witness in
the same test/runtime revision. It adds a post-scan checkpoint to the
token-aware VP8L entropy-mode pixel histogram pass after each completed
1,024-pixel chunk on rows wider than 1,024 pixels. The existing feature-gated
contract uses a deterministic caller-built 4,096×1 RGB probe and proves exact
whole-buffer and caller-owned-sink rejection at `maximum: 21`,
`observed: 22`, with `[0xAF]` untouched; an ample token-aware encode remains
byte-identical to the ordinary encode. Narrower rows retain their existing
row-start bounds, and the no-token traversal remains direct. This is
Rust-only work-control evidence: Pillow has no caller token, typed work-budget
result, caller-owned sink, or rollback equivalent, so no parity row, fixture,
diagnostic origin, new test function, or coverage-only hook was added.

The current VP8L candidate-trial assembly optimization is implemented at
`fe77d46c239da119e36942d5523255c47b8e06c8`, following the prefix-retention
change at `fa7b86abdc0ff91c870516d7a51ad986ff4d64bf`. Candidate trials leave
the already-emitted prefix in the parent writer, retain only each trial's
suffix, and recycle losing or replaced winning suffix allocations as scratch.
This removes redundant prefix clone/re-copy work and fresh per-trial suffix
allocations while preserving ordinary and token-aware output/checkpoint
behavior. The existing 28-row Pillow parity suite and 45-test Rust-only
feature-gate suite pass; this is an implementation optimization, not a
Pillow-visible result or new work-budget boundary, so no new parity fixture,
feature-gate test function, diagnostic origin, or coverage-only hook was added.

The latest lossless VP8L Huffman-RLE fill-materialization checkpoint is
implemented at production and test/runtime revision
`646ed73413a574368bfd01172fcd46c60622046f` through the same existing
`encode_work_budget_is_a_non_parity_result_contract`. Token-aware long-run
marking and normalized-count fills now poll after each 64 code-length values,
while the no-token helper retains its bulk fills. The existing caller-built
128×4 RGB palette probe proves `2,423/2,424` whole-buffer and `2,422/2,423`
caller-owned-sink rejection with `[0xC8]` untouched. This is Rust-only
work-control evidence: Pillow has no caller token, typed work-budget result,
caller-owned sink, or rollback contract, so no parity row, fixture-manifest
entry, diagnostic origin, new test function, or coverage-only hook was added.

A clean schema-`@3` benchmark at this revision reported 1.327542 s wall /
3.260114 user s / 0.223734 sys s / 289,112,064-byte peak RSS for the Pillow
parity fixture suite, 2.409336 s wall / 2.985661 user s / 0.192187 sys s /
245,104,640-byte peak RSS for the separate Rust-only feature-gate suite,
11.894242 s wall for the native release build with a 7,993,312-byte `rlib`,
and 5.589502 s wall for the `wasm32-unknown-unknown` determinism compile with
a 25,083,139-byte artifact. These are direct-child POSIX observations from
schema `@3`, not universal process-tree, allocator, or speed claims; repeated
allowed-dirty local observations are not release evidence. No managed parity,
feature-matrix, or Coverage MCP rerun is recorded for this revision.

Previous acceptance record: compact VP8 work-budget witnesses

The production checkpoint is implemented at
`bb48d168f94bedd8c2f9caf873e5a42d54690c47`, with its test/runtime witness at
`841ecbdba75a96f68ec23cdf6e0f7d4599786a9f`. The existing
`encode_work_budget_is_a_non_parity_result_contract` keeps its exact typed
boundary assertions and the compact preparation probes; its actual VP8
coefficient ladder now uses one deterministic quality-100 609×625 high-entropy
RGB probe. The exact endpoint whole-buffer/sink maximum/observed pairs are
`7,861,562/7,861,563` and `8,386,322/8,386,323` at 2,097,152 bits; sentinel
`[0xDB]` remains untouched. The probe traverses the intermediate coefficient
intervals while the test asserts only the new endpoint, avoiding four
redundant whole-buffer/sink re-encodes. The earlier 129×129 checkerboard
thresholds were global work-budget observations, not proof that the
coefficient checkpoint had been reached, and are superseded by this
stage-local witness.

The VP8L bitstream ladder now asserts exact whole-buffer/direct-sink endpoints
at 8, 32, 128, 512, 2,048, and 8,192 logical bits. The nested 16, 64, 256,
1,024, and 4,096-bit intervals are still traversed by those endpoint calls;
removing their intermediate whole-buffer/sink re-encodes saves 12 redundant
encodes while preserving the endpoint sentinels. The endpoint maximum/observed
pairs are 200/201 and 199/200, 206/207 and 205/206, 54,502/54,503 and
54,501/54,502, 54,940/54,941 and 54,939/54,940, 56,560/56,561 and
56,559/56,560, and 58,098/58,099 and 58,097/58,098 respectively. This
remains Rust-only test-runtime evidence because Pillow has no caller token,
typed work-budget result, caller-owned sink, or rollback equivalent.

The test-runtime follow-up at `841ecbdba75a96f68ec23cdf6e0f7d4599786a9f`
executes the two expensive 2,097,152-bit whole-buffer/direct-sink comparisons
concurrently on native test lanes while retaining the same exact errors and
untouched sentinels; the WASM path remains sequential. The 131,072-bit VP8L
witness is reduced from 256x256 to a verified 64x64 probe without changing its
41,542/41,543 whole-buffer or 41,541/41,542 sink boundary counts. This is
Rust-only harness evidence and adds no parity row, fixture, diagnostic origin,
new test function, or coverage-only hook.

This is Rust-only test-runtime evidence: Pillow exposes no caller token, typed
work-budget result, caller-owned sink, or rollback equivalent. No production
codec behavior, Pillow parity row or fixture, diagnostic origin, new test
function, or coverage-only hook changed. Local focused/full all-feature tests,
strict all-target Clippy, and rustfmt passed. The clean schema-@3 benchmark at
test/runtime revision `841ecbdba75a96f68ec23cdf6e0f7d4599786a9f` reported
1.089032 s wall / 2.883983 user s / 0.228330 sys s /
253,329,408-byte peak RSS for the Pillow parity fixture suite, and 1.366795 s
wall / 2.042505 user s / 0.116231 sys s / 169,902,080-byte peak RSS for the
separate Rust-only feature-gate suite. The native release `rlib` was 7,981,040
bytes and the `wasm32-unknown-unknown` determinism artifact was 25,091,745
bytes. Peak RSS is a direct-child POSIX observation, not a universal
process-tree or memory claim; benchmark resource dimensions not measured
remain open.

Exact-head managed Pillow parity run
`0121c773-64b8-4c09-b46e-8df639b046a4` passed 1,445/1,445 checks in 739 ms.
Feature-matrix run `2d1f5d78-dd74-4fe1-882d-ae4aa946b6a9` passed all configured
native/WASI lanes in 34,306 ms; its retained log has the native/WASI capability
agreement marker and no `lock-wait` match. Nightly LLVM run
`afa2a5ab-c5a2-4be8-80c6-bd535440eafd` passed 85/85 tests in 57,076 ms and
ingested snapshot `208b22e7-5a8c-4884-8fd5-856293c45d01`: 54,883/55,691 lines,
7,855/8,042 branches, 3,112/3,203 functions, and 84,607/86,439 regions. These
are Rust implementation/coverage records, not Pillow-parity coverage; the
known LLVM JSON segment-normalization warning remains. The aggregate shortfall
is 808 lines, 187 branches, 91 functions, and 1,832 regions.

Correction and supersession note: the historical VP8 records below that report
832×832 or 129×129 checkerboard probes and the old `187,405/187,406`,
`318,670/318,671` boundary pairs are retained for chronology only. Those
thresholds were global work-budget observations; they did not prove that the
coefficient checkpoint had been reached. The current
`bb48d168f94bedd8c2f9caf873e5a42d54690c47` record above is
the authoritative stage-local VP8 coefficient witness, and no historical
checkerboard values should be used as current acceptance evidence.

Historical acceptance record: WebP VP8L hash-chain candidate trials

The token-aware lossless WebP VP8L backward-reference hash-chain scan is
implemented at production and test/runtime revision
`e3e39ff687aba7b589b74188f2592b1bf3839306`. The existing
`encode_work_budget_is_a_non_parity_result_contract` now counts completed
chain-candidate trials across the pass and polls after each 64 trials in the
token path; the no-token candidate loop remains a separate tight path. Its
deterministic 160×160 repeated-row RGB probe preserves ordinary bytes under an
ample budget and rejects at `maximum: 16,254`, `observed: 16,255` in both the
whole-buffer and caller-owned-sink paths, with sink sentinel `[0xD6]` untouched.
The boundary is a real interior hash-chain work dimension, not a synthetic
coverage hook or a Pillow-parity claim.

Pillow exposes no caller token, typed work-budget result, caller-owned sink, or
rollback equivalent, so this is Rust-only resource-contract evidence. It adds
no Pillow parity row, fixture-manifest row, diagnostic origin, new test
function, or coverage-only hook. Local focused/full all-feature tests, strict
all-target Clippy, and rustfmt passed. Exact-head managed Pillow parity run
`4ade50dc-898b-4171-bdb1-251ec41aa0d9` passed 1,445/1,445 checks in 9,831 ms.
Feature-matrix run `b12125c4-d185-41c4-b952-53828f6448a7` passed all
configured native/WASI lanes in 60,951 ms with `cache=cold`, `lanes=6`,
`test_threads=2`, `build_jobs=2`, `debug=0`, and `verbose=0`; its retained log
contains the native/WASI capability agreement marker and no `lock-wait` match.
Nightly LLVM run `a33150ab-c337-4325-be49-687741d091b3` passed 85/85 tests in
70,946 ms and ingested snapshot `a225b727-7b8d-4783-b03b-1a109d831b2d`:
54,785/55,635 lines, 7,812/8,016 branches, 3,111/3,202 functions, and
84,476/86,361 regions. These are Rust implementation/coverage records, not
Pillow-parity coverage; the known LLVM JSON segment-normalization warning
remains. Remaining progress semantics, interruption inside one candidate or
other codec unit, transient allocation accounting, and short-write/rollback
cleanup remain open.

Historical acceptance record: cross-codec flush rejection

The Rust-only sink contract is implemented at test/runtime revision
`163520b4ab06b9f4b15c2a6e8bdc12e9a29c4d39`; production behavior remains at
`5aa0d77b37a5d81e1149e5169915ce21c59b6454`. The existing
`partial_structural_sink_write_preserves_prefix_across_available_encoders`
feature-gate test now also sends complete still and supported multi-frame
sequence deliveries to a sink whose `flush` rejects after all bytes were
accepted. Every available path must return one typed `OutputWrite` with the
selected format and encode stage, attempt `flush` exactly once, and preserve
the exact bytes already delivered. Partial second-write behavior remains
separately asserted with an observable prefix and no flush attempt.

This is Rust-only destination evidence: Pillow has no caller-owned sink,
flush hook, typed output-write cause, or rollback equivalent. No production
codec behavior, Pillow parity row or fixture, diagnostic origin, new test
function, or coverage-only hook changed. Exact-head managed Pillow parity run
`72051344-2014-4a9d-9fb8-e5e5a56a1f73` passed 1,445/1,445; feature-matrix run
`18bb761a-7ade-4a78-8680-1b55562053ac` passed with
`cache=warm`, `lanes=24`, `test_threads=1`, `build_jobs=1`, `debug=0`, and
`verbose=0`, ending with the native/WASI capability agreement marker and no
`lock-wait` match. Nightly LLVM run
`ed54deb4-2f1f-45fb-9e45-204cb7ba8621` passed 85/85 tests in 70,409 ms and
ingested snapshot `1c76bb32-2fcf-4b82-a50c-1cf3409e9a0c`: 54,756/55,605
lines, 7,797/8,000 branches, 3,110/3,201 functions, and 84,427/86,315
regions. These coverage totals are Rust implementation evidence, not Pillow
parity; the known LLVM JSON segment-normalization warning remains. Rollback,
short-write recovery beyond the tested prefix, and partial-container cleanup
remain open.

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

## Historical acceptance record: superseded implementation slices

Historical acceptance record: WebP VP8L entropy-mode analysis, entropy-bin histogram
clustering pre-passes, histogram population, combined entropy-cost, merge,
backward-reference cost-manager setup and
interval-update/cleanup, non-saturated
interval split/merge, saturated fallback,
and cost, Huffman RLE, canonical-code,
Huffman-tree simple-tree symbol-discovery, token-frequency, trailing-token-trim,
and code-length-emission checkpoints, plus RGB-equal grayscale-preparation
checkpoints, candidate-trial prefix reuse, lossy WebP RGBA alpha-palette
source-collection, index-packing, and candidate-scan checkpoints, and lossless WebP
VP8L source-pixel materialization, image-palette source scans, ordered
unique-color palette drains, palette-mode index-packing, palette-index lookup,
palette sign, nearest-delta candidate-scan, and RGBA
hidden-RGB cleanup checkpoints, plus compile-only matrix runtime

The token-aware VP8L entropy-mode analysis now charges cooperative checkpoints
after each 64 symbols while scanning fixed-alphabet histogram costs. The
token-aware VP8L histogram analysis path likewise charges after each 64 symbols
while scanning histogram populations,
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
each 1,024 source pixels; the no-token path retains its bulk BTreeSet collection
and byte output. The deterministic 16×64 RGBA fixture in this same existing
feature-gate contract proves exact whole-buffer and direct-sink rejection at
`maximum: 5`, `observed: 6`, with the sink sentinel `[0xC1]` untouched. Pillow
has no caller token, work-budget result, or caller-owned sink, so this is Rust-only evidence with no parity row,
fixture-manifest row, diagnostic origin, new test function, or coverage-only
hook. The implementation is committed at
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
caller-owned-sink rejection at `maximum: 2`, `observed: 3`, with sentinel
`[0xBA]` untouched. Pillow has no caller token, work-budget result, or
sink-rollback contract, so this remains Rust-only evidence with no parity row,
fixture-manifest row, diagnostic origin, new test function, or coverage-only
hook. The implementation is committed at
`53886bfdc7ea4eee996f5a892e1742a8acd91a9b`. The lossless VP8L palette
ordering path keeps the no-token helper byte-preserving and charges token-aware
sign collection and nearest-delta suffix scans after each 64 palette entries or
candidate values. Palette-index packing keeps its no-token linear lookup
byte-preserving and charges token-aware lookup scans after each 64 palette
candidates. The deterministic 128-entry 128×4 RGB fixture proves exact
whole-buffer and caller-owned-sink rejection at `maximum: 3,000`,
`observed: 3,001`, with sentinel `[0xA7]`; monotone, mixed short, and
transparent-zero public palette fixtures cover the early-return and rotation
branches. The deterministic 128×128 RGB lookup probe proves whole-buffer and
caller-owned-sink rejection at `maximum: 9,804`, `observed: 9,805`, with
sentinel `[0xA9]` untouched. Pillow has no caller token, work-budget result, or sink contract, so
this remains Rust-only evidence with no parity row, fixture-manifest row,
diagnostic origin, new test function, or coverage-only hook. The same existing
contract now uses a deterministic 128×8 RGB fixture built from the existing
128-entry palette to reach lossless VP8L palette-mode index packing. It proves
ample-budget byte identity, then exact whole-buffer and caller-owned-sink
rejection at `maximum: 5,204`, `observed: 5,205`; the sink preserves the
delivered prefix `[0xC3, 0x52, 0x49, 0x46, 0x46, 0xEA, 0x03, 0x00, 0x00, 0x57,
0x45, 0x42, 0x50]`. The token-aware packing path polls after each 1,024 source
pixels, while the no-token linear packing loop remains byte preserving. Pillow
has no caller token, typed work-budget result, or caller-owned sink, so this is
Rust-only evidence with no parity row, fixture-manifest row, diagnostic origin,
new test function, or coverage-only hook. The implementation is committed at
`589186a6e3f0a1f8fd47ca84dcc73133620ed9fa`. Candidate
scoring and fixed-alphabet Huffman cost paths now charge after each 1,024 tokens
and each 64-symbol population scan. Huffman RLE preparation, including the
reverse-tail fixed-alphabet scan, and in-run code-length scans charge after
each 64 source symbols, while compressed
Huffman-token materialization charges after each 16 emitted tokens.
Canonical-code assignment and compressed Huffman-token generation charge after
each 64 code-length symbols. Huffman-tree simple-tree symbol discovery now charges after
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
`84a9abbd8fca78fc468e3e46be8baa5ca37e005f5`; an earlier production slice is
committed at revision
`b1fafe4bacd60628b2385e14a843bb6bf827c1e2`; the current contract and backward
cost-manager setup are committed at `0675baea3b97104d68636e8fe363ed61ba625c01`
(following `063f00e145aff455c30656b3559c8881b8e51a6f`), and the saturated
cost-interval implementation is committed at
`b153381bd9657b1f9da3707ca1d6f015ab174042`; the non-saturated interval
split/merge implementation is committed at
`2dd22a3f8f535563ae5db4f80c55829ddcf2c94f`; the current cost-manager
interval-update/cleanup checkpoint slice is committed at
`52623efa026c775b2d1c5157e10cf485e5fca789`; the candidate-trial prefix-reuse
optimization is committed at `3e139ae7fc5bc1bfaeb3440c4112394cb33eeff3`; the
entropy-analysis checkpoint slice is committed at
`1a8cae394ad0265e4f0a3bf84511b80e7e2a7842`. The existing
entropy-bin clustering pre-pass checkpoint slice is committed at
`4eae86493bad9016611648a498a81a79f90f5551`; the alpha-palette candidate scan
slice is committed at `1b87a06bf0b8c866bd843df3ecb8c63e447f475c`; the lossless
VP8L palette sign/nearest-delta scan implementation is committed at
`c36e2472d0366bddd42c55e6ec20d282f8abe068`, with public short-palette fixture
coverage completed at `4a81e987bfac8c9893e9131a772a3eb0cebc63f8`. The existing
`encode_work_budget_is_a_non_parity_result_contract` uses deterministic RGB
probes and proves exact whole-buffer and caller-owned-sink rejection at the
entropy-analysis boundary `maximum: 19`, `observed: 20` with sentinel `[0xAD]`,
the histogram-population boundary `maximum: 58`, `observed: 59` with `[0xB8]`,
the combined entropy-cost boundary `maximum: 76`, `observed: 77` with `[0xAE]`,
the histogram-merge boundary `maximum: 8,254`, `observed: 8,255` with `[0xAF]`
untouched, and the cost-estimate boundary `maximum: 14,088`, `observed: 14,089`
with `[0xB0]` untouched, plus exact Huffman-RLE preparation boundaries at
`maximum: 812`, `observed: 813` for the whole-buffer return path and
`maximum: 811`, `observed: 812` with `[0xB1]` untouched for the caller-owned
sink. The same
existing contract now uses a deterministic 128×128 RGBA grayscale probe to
prove the preparation checkpoint boundary at `maximum: 179`, `observed: 180`
in both whole-buffer and caller-owned-sink paths, with `[0xB2]` untouched.
The same contract proves the histogram-clustering min/max and bin-assignment
pre-pass boundary at `maximum: 5,309`, `observed: 5,310` with `[0xB9]`
untouched. The Huffman-tree frequency boundary remains
`maximum: 43,985`, `observed: 43,986` for the whole-buffer return path and
`maximum: 43,984`, `observed: 43,985` with `[0xB3]` untouched for the
caller-owned sink. The batched code-length-emission contract first proves
normal/ample-budget fixture-byte identity, then rejects both whole-buffer and
caller-owned-sink paths at `maximum: 144,853`, `observed: 144,854`; the new
pre-output palette checkpoint leaves sentinel `[0xB6]` untouched. The old late
cost-manager and trailing-trim maxima were tied to
the previous per-token polling schedule and are superseded by the batched
emission poll. The implementation checkpoints remain current behavior, but
those stale exact thresholds are not claimed. Pillow has no caller token,
work-budget result, or sink contract, so this change adds no parity row,
fixture-manifest row, diagnostic origin, new test function, or coverage-only
hook. The cache probe now rejects both paths at `maximum: 136,672`,
`observed: 136,673`; the new pre-output palette checkpoint leaves sentinel
`[0xB5]` untouched.
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
rejection at `maximum: 2`, `observed: 3`, with sentinel `[0xB7]` untouched.
The token-aware VP8L cleanup polls after each 1,024 scanned pixels; the
ordinary no-token path retains its bulk loop. Pillow has no caller token,
work-budget result, or sink-rollback contract, so this remains Rust-only
evidence with no parity row, fixture-manifest row, diagnostic origin, new test
function, or coverage-only hook. The implementation is committed at
`464126042af49a945a63a505cb1675ebe703a904`.
This closes the next causal interior checkpoint in the current WebP work-control
slice. It is Rust-only work-control evidence: Pillow has no caller token,
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
ms and its retained log ended with the terminal capability agreement; targeted
searches returned no lock-wait, build-directory, or package-cache matches.
Managed LLVM coverage run `d47dff4a-7ff6-4add-9277-e0d8f2b14f52` passed 85/85
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
configured lanes in 42,579 ms; its retained log records `cache=warm lanes=24
test_threads=1 build_jobs=1 debug=0 verbose=0`, ends with `capability tables OK:
every native and wasm32-wasip1 lane agrees`, and has no targeted lock-wait,
build-directory, or package-cache matches. Coverage MCP run
`7e2fe608-90a3-4095-94e4-3be2328abe2a` passed 85/85 tests in 77,812 ms and
ingested snapshot `2e17df97-e1e8-4e01-8486-b8fbbcc54aff`:
52,165/52,719 lines, 7,217/7,338 branches, 2,960/3,036 functions, and
80,696/81,909 regions. Compared with the preceding accepted snapshot
`f8af387e-26a2-43fc-bdc7-65a8b098705f`, covered totals increased by 8 lines,
4 branches, 0 functions, and 11 regions; source totals grew by 10 lines,
4 branches, 0 functions, and 12 regions. The changed partition file reported
536/541 lines, 88/88 branches, 30/30 functions, and 774/817 regions; the
changed coefficient file reported 416/425 lines, 66/66 branches, 21/21
functions, and 579/620 regions. The known LLVM JSON segment-normalization
warning remained; the strict aggregate shortfall was 554 lines, 121 branches,
76 functions, and 1,213 regions. These were implementation/coverage records
separate from Pillow parity, and no coverage-only test was used.

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
discarded 2,048×1,024 exploratory allocation. Two new whole-buffer/sink boundary
pairs are covered inside the existing feature-gated contract; no new test
function, parity row, fixture file, diagnostic origin, or coverage-only hook was
added. Pillow has no
caller token, work-budget result, or caller-owned sink, so these are Rust-only
resource-contract results and the unchanged parity run is regression evidence.

The focused contract and the full local all-feature test set passed; the
45-test feature-gate target finished in 2.83 seconds, and strict all-target
Clippy, rustfmt, and repository provenance gates passed. Managed Pillow parity
run `80f593b8-68ef-42b9-a3ec-ecb8e9025d62` passed 1,445/1,445 checks with zero
failures or skips in 4,247 ms. Feature-matrix run
`32b78172-49dc-42d9-9c7e-f30fb3626047` passed all 991/991 checks across 24/24
configured lanes in 37,095 ms; its retained log records `cache=warm lanes=24
test_threads=1 build_jobs=1 debug=0 verbose=0`, ends with `capability tables OK:
every native and wasm32-wasip1 lane agrees`, and has no targeted lock-wait,
build-directory, or package-cache matches. These timings are observed
cache-/runner-sensitive evidence, not a universal codec-speed claim.

Coverage MCP run `23feb620-aac5-4be4-b7bf-58de1fa9642d` passed 85/85 tests in
69,289 ms and ingested snapshot `f8af387e-26a2-43fc-bdc7-65a8b098705f`:
52,157/52,709 lines, 7,213/7,334 branches, 2,960/3,036 functions, and
80,685/81,897 regions. Compared with the preceding accepted snapshot
`bb395085-1498-442a-9fae-ada84a71f90e`, covered totals increased by 14 lines,
5 branches, 0 functions, and 18 regions; source totals grew by 10 lines,
4 branches, 0 functions, and 12 regions. The changed partition file reports
532/536 lines, 86/86 branches, 30/30 functions, and 768/811 regions; the
changed coefficient file reports 412/420 lines, 64/64 branches, 21/21
functions, and 574/614 regions, with remaining uncovered lines limited to
defensive/error-propagation alternatives. The known LLVM JSON
segment-normalization warning remains; the strict aggregate shortfall is 552
lines, 121 branches, 76 functions, and 1,212 regions. Coverage is
implementation evidence, not Pillow parity, and no coverage-only test was used.

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
54,369 ms and ingested snapshot `60efcff6-ade0-465f-a1fc-6a08f8dd655f`:
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

Historical acceptance record: AVIF non-primary typed CICP

The non-primary AVIF typed-CICP slice is implemented at
`1451217f6344c71141518b28d724b9455b7c0a87`. The existing
`source_alpha_matches_the_container_contract` feature-gated integration test
mutates the committed `alpha.avif` bytes only in memory to associate the
primary typed `colr`/`nclx` property with auxiliary item 2. It asserts the
source-local item ID and exact CICP values on inspect, still decode, and
fallback-sequence decode, preserves the primary `SourceColor`, and proves
decoded pixels are unchanged. No new test function, fixture file, parity row,
diagnostic origin, or coverage-only hook was added. Pillow has no item-level
source-color result, so this is Rust source-provenance/specification evidence;
non-primary ICC profiles and other item color/property forms remain open.

The focused contract passed, and the local all-feature test suite passed all
82 tests; strict all-target Clippy and rustfmt also passed. Managed Pillow
parity run `55d0f139-9806-4563-b74a-04693622b2f2` passed 1,445/1,445 checks in
2,367 ms with zero failures or skips. Feature-matrix run
`3317cea7-75ab-4c01-b665-520c832aa6c0` passed all 991/991 checks in 80,775 ms,
ended with `capability tables OK: every native and wasm32-wasip1 lane agrees`,
and had no `lock-wait`, `build-directory`, or `package-cache` log matches.

Coverage MCP run `58d84cda-d167-4641-9f14-8f81ba5c533e` passed 85/85 tests in
90,128 ms and ingested snapshot `80bdf23c-8b1c-4459-ae66-fd0b789d3eb7`:
52,025/52,576 lines, 7,188/7,310 branches, 2,948/3,022 functions, and
80,512/81,726 regions. Compared with the preceding accepted snapshot
`5afb834b-bdb7-4f52-a29e-da99b9af4103`, covered totals increased by 95 lines,
7 branches, 14 functions, and 119 regions; source totals grew by 95 lines,
8 branches, 14 functions, and 119 regions. The changed AVIF files report:
`container.rs` 2,530/2,534 lines, 406/410 branches, 158/158 functions, and
3,782/3,788 regions; `samples.rs` 3,712/3,714 lines, 545/546 branches,
180/181 functions, and 4,820/4,826 regions; `decode.rs` and `av1/mod.rs` are
fully covered at 566/566 and 152/152 lines respectively. `types/mod.rs` is
1,474/1,483 lines, 118/124 branches, 139/142 functions, and 1,478/1,488
regions; its named partial `SourceDescriptor::is_empty` chain includes the
new item-color field. The current named AVIF defensive gaps remain the
duplicate transform branches at `container.rs:1079-1080`, `:1085-1086`, and
`:1091-1092`, the duplicate
alpha-association branch at `container.rs:1188-1189`, the empty-grid branch at
`samples.rs:1235-1236`, and the pre-existing enum-doc gaps at
`types/mod.rs:962-976`. The known LLVM JSON segment-normalization warning
remains; the strict aggregate shortfall is 551 lines, 122 branches, 74
functions, and 1,214 regions. These are Rust implementation/coverage metrics,
not Pillow-oracle parity metrics, and no coverage-only test was used to hide
them.

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

The original cross-codec partial structural sink-write slice was implemented at
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

The native AVIF auxiliary-alpha provenance correction is implemented at
`bf9dda0de0ce8214cf525ccdba395fa99246d8a6`. AVIF inspection now reports an
alpha item as `SourceAlpha::Auxiliary`, and native still and sequence decoded
images retain the same source descriptor. This identifies samples carried by a
separate auxiliary image; it does not alter the normalized decoded RGBA bytes.
The feature-gated integration contract
`source_alpha_matches_the_container_contract` uses the committed
`tests/fixtures/input/images/avif/alpha.avif` fixture and asserts inspect,
still-decode, and sequence-decode provenance. It is not a unit test, adds no
`#[cfg(coverage)]` hook, and adds no Pillow-parity row. Pillow parity cannot
cover this field because its result schema has no source descriptor or
auxiliary-item provenance; the unchanged parity run below is regression
evidence for Pillow-observable output only.

Managed Pillow parity run `002ee279-806e-4de5-acb9-3485f009c2a1` passed
1,445/1,445 checks with zero failures or skips in 41,696 ms. Feature-matrix run
`b204b2a7-d4f4-470c-b6aa-3698ff3a97d1` passed 991/991 checks in 7,053 ms; its
retained log contains the capability marker `capability tables OK: every native
and wasm32-wasip1 lane agrees`. Coverage MCP run
`712ff626-3b86-4cb5-aaf2-14b554761541` passed 85/85 tests in 49,016 ms and
ingested snapshot `8e804ce4-ac81-4386-a283-c77a12dec7c5`, reporting
50,813/51,279 lines, 7,012/7,096 branches, 2,828/2,897 functions, and
78,967/80,004 regions. Compared with snapshot
`dcaab996-685d-4470-ae30-a8d96790261f`, coverage has no line delta and no
changed-to-uncovered lines; it adds two covered/total branches and five
covered/total regions. `src/codecs/avif/container.rs` is 2,392/2,392 lines,
374/374 branches, 137/137 functions, and 3,593/3,593 regions; the changed
`src/codecs/avif/decode.rs` remains 455/455 lines, 40/40 branches, 26/26
functions, and 622/622 regions. The LLVM JSON segment-normalization warning
remains. The static coverage-origin verifier still reports 219 exact guards
with no Pillow-parity origin. These source-provenance and aggregate coverage
records remain separate from Pillow parity; no coverage-only test was added.

The AVIF premultiplied-alpha relationship slice is implemented at
`2d4b9f622923255617eac62669d32d489ead90c5`. `SourceDescriptor` now retains
bounded source-local `prem` `iref` edges through
`avif_premultiplied_relationships()` on native inspection, still decode, and
sequence-frame decode, while `SourceAlpha::Auxiliary` continues to identify
the separate alpha item. The existing
`source_alpha_matches_the_container_contract` extends its committed
`alpha.avif` fixture contract with an in-memory `prem` child witness, asserts
the generic and filtered relationship views, and proves decoded normalized
bytes are unchanged. This is Rust source-provenance evidence: Pillow has no
source descriptor or item-relationship result field, so it adds no parity
row, fixture, diagnostic origin, new test function, or coverage-only hook.

Managed Pillow parity run `723e15eb-58e2-417f-9cc1-52c77f458fb4` passed
1,445/1,445 checks with zero failures or skips in 74,171 ms. Clean-revision
feature-matrix run `aace6bcd-f981-479a-97e9-1f6a03cc96ed` passed 991/991
checks in 37,636 ms and retained `capability tables OK: every native and
wasm32-wasip1 lane agrees`; targeted lock-wait/build-directory/package-cache
searches were empty. Coverage MCP run
`c5bcc4ae-1a8b-45cf-83f6-ce410acb8020` passed 85/85 tests in 108,208 ms and
ingested snapshot `5afb834b-bdb7-4f52-a29e-da99b9af4103`: 51,930/52,481
lines, 7,181/7,302 branches, 2,934/3,008 functions, and 80,393/81,607
regions. Compared with snapshot `c5b5dedb-0685-4222-9eee-89dbf6c0a55c`,
covered totals increased by 75 lines, 7 branches, 8 functions, and 96
regions; source totals grew by 75 lines, 8 branches, 8 functions, and 96
regions. The line-only comparison reports four displaced defensive records at
`src/codecs/avif/container.rs:1075`, `:1081`, `:1087`, and `:1167` after the
source-descriptor insertion; they remain visible rather than hidden. The LLVM
JSON segment-normalization warning remains, and the strict aggregate shortfall
is 551 lines, 121 branches, 74 functions, and 1,214 regions. The
coverage-origin verifier still accounts for all 219 exact guards without
assigning any to Pillow parity.

Remaining AVIF metadata work is non-alpha auxiliary relationships and
properties, item identity/plane-range/quality details, grid topology,
non-primary color forms beyond typed CICP, and invisible RGB semantics. The feature-matrix serial-tail overlap is implemented
at `da3dfbe43c90320c6cbf92ac7bcfea6bec71c1fe`: the two
`wasm32-unknown-unknown` `feature_gate_tests --no-run` checks now run in their
matching `none` and `avif` lanes, and the all-feature
`wasm32-wasip1` determinism compile/run runs in the `all` lane. The command
still covers all 33 target/feature lanes, with 45 feature-gate assertions in
each native and WASI runtime lane (990 total), plus the determinism test and
the capability-table no-drift check. In a
controlled fresh-root local comparison using `MATRIX_JOBS=6`,
`MATRIX_TEST_THREADS=2`, and `MATRIX_BUILD_JOBS=2`, the pre-change harness at
`842f8edbc2325022108e7fd494b2ec6b7f11c69d` took 81.21 seconds and the new
harness took 69.61 seconds. Managed run
`1569fabd-04b5-483a-b971-20fd3e8aca76` passed 991/991 in 3,353 ms and retained
`capability tables OK: every native and wasm32-wasip1 lane agrees`, with no
`lock-wait` match. This is a cache- and runner-sensitive harness measurement,
not a universal speedup claim; it changes no production profile, fixture,
parity row, diagnostic origin, or coverage scope.

The finer lossy WebP/VP8 coefficient logical-bitstream checkpoint slice is
implemented at `e3f8d4bfb1f5687d0f5322519b776740748a82fc`. Token-aware residual
encoding now charges after each 1,024 logical coefficient bit crossings while
retaining the 16,384-boolean coefficient-bit and 1,024-byte emitted-output
intervals; the no-token path remains a monomorphized no-op controller. The
Rust-only `encode_work_budget_is_a_non_parity_result_contract` uses the same
512x512 RGB probe to prove ample-budget byte identity and whole-buffer and
direct-sink rejection at the first logical coefficient checkpoint
(`maximum: 361`, `observed: 362`), with both bounded sinks untouched. Pillow
has no caller token, work-budget result, or caller-owned sink, so this adds no
parity row, fixture, diagnostic origin, or coverage-only hook.

Managed Pillow parity run `f54e8e8f-5413-425e-b8f2-288f99c45688` passed
1,445/1,445 checks with zero failures or skips in 50,344 ms. Feature-matrix run
`76b54e3e-b339-456e-88a6-c7ed7e3968f1` passed 991/991 checks in 38,706 ms; its
retained log contains the capability marker
`capability tables OK: every native and wasm32-wasip1 lane agrees` and no
`lock-wait` match. Coverage MCP run
`7e76aa2c-6bd3-4a24-8029-45fbb6a2b333` passed 85/85 tests in 62,893 ms and
ingested snapshot `4cc74646-c229-4ab5-92ec-a511434a893a`, reporting
50,815/51,279 lines, 7,012/7,096 branches, 2,828/2,897 functions, and
78,970/80,004 regions. Compared with snapshot
`8e804ce4-ac81-4386-a283-c77a12dec7c5`, coverage adds two covered lines and
three covered regions, with no changed-to-uncovered lines, branch delta, or
function delta. The VP8 residual file reports 339/349 lines, 38/38 branches,
21/21 functions, and 493/537 regions; its retained line gaps are recorded,
not hidden with a coverage-only test. The LLVM JSON segment-normalization
warning remains. These are Rust-only implementation and target records
separate from Pillow parity.

The finer lossless WebP/VP8L logical-bitstream checkpoint slice is implemented
at `d8abbfb228e53dc704cae8571959e594486fd60c`. Token-aware VP8L bit writing
now charges after each 512-bit logical-bitstream interval while retaining the 1,024-byte
emitted-output interval; compression-search trials preserve their checkpoint
state when the shortest candidate is selected, and the no-token path remains a
monomorphized no-op controller. The existing Rust-only
`encode_work_budget_is_a_non_parity_result_contract` uses the patterned 128x128
RGB probe to prove ample-budget byte identity and whole-buffer/direct-sink
rejection at the first logical checkpoint (`maximum: 54,823`, `observed:
54,824`), with both bounded sinks untouched. Pillow has no caller token,
work-budget result, or caller-owned sink, so this adds no parity row, fixture,
diagnostic origin, or coverage-only hook.

Managed Pillow parity run `e7fcfaba-c7e0-4c0b-910c-b9b5ed4081f0` passed
1,445/1,445 checks with zero failures or skips in 56,180 ms. Feature-matrix
run `13582bc0-0266-41ed-957b-651696df49a3` passed 991/991 checks in 64,693 ms;
its retained log contains the capability marker
`capability tables OK: every native and wasm32-wasip1 lane agrees` and no
`lock-wait` match. Coverage MCP run
`d9b2ba47-76b3-45dc-a1c7-091e532153fb` passed 85/85 tests in 90,751 ms and
ingested snapshot `09bee72c-c5cf-4c21-ac25-80fda41c1622`, reporting
50,815/51,279 lines, 7,012/7,096 branches, 2,828/2,897 functions, and
78,970/80,004 regions. Compared with snapshot
`4cc74646-c229-4ab5-92ec-a511434a893a`, coverage has zero line, branch,
function, or region delta and no changed-to-uncovered lines. The VP8L encoder
file reports 1,467/1,477 lines, 226/226 branches, 77/77 functions, and
2,127/2,228 regions; retained gaps are recorded rather than hidden. The LLVM
JSON segment-normalization warning remains. These are Rust-only implementation
and target records separate from Pillow parity.

The finer lossy WebP/VP8 first-partition logical-checkpoint slice is implemented
at `2af1eed8a117995b6965fde7461480d6586960b1`. Token-aware first-partition
boolean coding now charges after each 512 logical coded bits while retaining the
16,384-boolean first-partition boundary; the existing 1,024-bit logical
coefficient, 16,384-boolean coefficient, and 1,024-byte boolean-bitstream
output intervals remain unchanged. The no-token path remains a monomorphized
no-op controller. The Rust-only
`encode_work_budget_is_a_non_parity_result_contract` uses the patterned 896x512
RGB probe to prove whole-buffer and direct-sink rejection at the 512-bit
logical boundary (`maximum: 593`, `observed: 594`), retains the neighboring
fine probe (`maximum: 580`, `observed: 581`), and proves the independent
16,384-boolean first-partition boundary (`maximum: 613`, `observed: 614`) in
both paths. The existing boolean-output probes remain `maximum: 589`,
`observed: 590` for whole-buffer and `maximum: 588`, `observed: 589` for the
direct sink; a later `maximum: 700`, `observed: 701` probe reaches the
coefficient partition after first-partition completion. Every bounded sink
remains untouched. Pillow has no caller token, work-budget result, or
caller-owned sink, so this is Rust-only resource-contract evidence with no
parity row, fixture, diagnostic origin, or coverage-only hook.

Managed Pillow parity run `805d09c5-9d04-45d9-afe9-d5e80629380c` passed
1,445/1,445 checks with zero failures or skips in 48,956 ms. Feature-matrix
run `31a5f5f0-a665-4d55-bcab-8ad166cf5eae` passed 991/991 checks in 51,687 ms;
its retained log contains `capability tables OK: every native and wasm32-wasip1
lane agrees` and has no `lock-wait` match. Coverage MCP run
`9da0601f-f376-4acf-9d7a-6c5bf88b6781` passed 85/85 tests in 95,449 ms and
ingested snapshot `bb9c8a0b-8d68-4b33-bfbc-0eea51aedb75`, reporting
50,816/51,279 lines, 7,012/7,096 branches, 2,828/2,897 functions, and
78,975/80,004 regions. Compared with snapshot
`09bee72c-c5cf-4c21-ac25-80fda41c1622`, coverage adds one covered line and
five covered regions, with zero branch or function delta. The line-only
regression view still reports one displaced line:
`src/codecs/webp/encode/vp8/residual.rs:220` changed from one hit to zero;
this is recorded rather than hidden. The VP8 residual file reports 339/349
lines, 38/38 branches, 21/21 functions, and 494/537 regions. The LLVM JSON
segment-normalization warning remains. These are Rust-only implementation and
target records separate from Pillow parity.

Historical acceptance record: grid-derived AVIF item provenance and warm matrix fanout

The bounded AVIF source-provenance slice is implemented at
`c8c18221d1d3126ac320cfc9a097386ddd007289` and its ordered primary-grid item
list plus existing feature-gated fixture contract were completed at
`fdd7afe988cf9a6b57de9bb69a98cc7dc8d690ca`. Coverage compilation completeness
for the existing structural-state initializers was fixed at
`8607dca5cf813448a8f95bbe62c6e5c07733ecef`.
The committed `grid.avif` fixture has primary item `1`, derived color items
`2` and `3`, and alpha auxiliary items `5` and `6` targeting `2` and `3`.
`SourceDescriptor::avif_auxiliary_relationships()` retains those exact
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

Remaining AVIF categories are non-alpha and richer auxiliary graphs, grid
topology/composition, gain maps/depth/thumbnails/supplementary content,
premultiplication and plane/range/quality semantics, `iloc` extent variants,
content selection, invisible RGB, and fragmented-track/edit-list behavior.

Historical acceptance record: direct AVIF auxiliary relationship

The direct AVIF auxiliary-alpha relationship slice is implemented at
`fcff8dd9e9bebf22da8b7ee3dd3e93ae13798018` and finalized with the
assertion-only contract checkpoint `4c61ad60eab2be62dcad80f8f4b95550cae2688c`.
`SourceDescriptor::avif_auxiliary_relationship()` retains the direct
source-local `auxl` relationship from auxiliary item `2` to primary item `1`
in the committed `alpha.avif` fixture. The relationship is present on
inspection, still decode, and every sequence frame; it records provenance and
does not transform decoded pixels. Non-alpha auxiliary properties,
derived/grid/track relationships, plane range/quality, premultiplication, and
invisible-RGB semantics remain open.

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
are cache- and runner-sensitive execution measurements, not universal
benchmarks. Coverage MCP run `3c34f53c-72d8-4240-8ebf-6595f24c7b8d` passed
85/85 tests in 49,923 ms and ingested snapshot
`92f6ba37-f4eb-4ee8-aeb3-88e94856501a`: 51,105/51,579 lines, 7,043/7,130
branches, 2,856/2,925 functions, and 79,321/80,378 regions. The only
changed-to-uncovered line against the prior accepted snapshot is the defensive
duplicate-alpha-association branch at `src/codecs/avif/container.rs:1102`; the
shortfall is recorded rather than hidden.

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

The same runtime-first slice keeps feature-matrix lanes isolated, avoids the
shared Cargo lock, and propagates native/WASI child failures instead of masking
them behind capability-table output. Warm retained roots on the measured
12-logical-CPU host now use one Cargo build worker per lane; explicit
overrides remain available. The exact-head managed matrix passed 991/991 in
46,976 ms, down from the preceding 52,870 ms run at the same scope, and its
retained log ends with the native/WASI capability agreement marker with no
build-directory or package-cache lock-wait match. These are execution
measurements, not controlled universal benchmarks.

Managed Pillow parity run `95fa9817-5693-4a82-9188-3e2de83af18f` passed
1,445/1,445 checks with zero failures or skips in 45,497 ms. Feature-matrix run
`204d59f1-a261-4152-871a-035ead6b464b` passed 991/991 checks in 52,870 ms; its
retained log contains `capability tables OK: every native and wasm32-wasip1
lane agrees` and has no build-directory or package-cache lock-wait match.
Coverage MCP run `4898fcc9-4d09-4d37-b6d8-77cd6cafcd98` passed 87/87 tests in
82,830 ms and ingested snapshot `5a8b1512-2377-4d21-8951-dd1430d2b653`,
reporting 51,010/51,483 lines, 7,031/7,116 branches, 2,846/2,915 functions,
and 79,219/80,272 regions. Compared with baseline snapshot
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

The current managed Pillow parity run passed 1,445/1,445 checks in 48,062 ms;
the feature matrix passed all configured native, `wasm32-unknown-unknown`, and
`wasm32-wasip1` lanes in 91,794 ms and ended with
`capability tables OK: every native and wasm32-wasip1 lane agrees`. Coverage
MCP run `4b01d7a9-abda-4f47-9cff-373376da2cfa` passed 85/85 tests in 244,572 ms
and ingested snapshot `57a4ea82-7122-4e45-8b78-2626fa033bf2`, reporting
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

The current managed Pillow parity run passed 1,445/1,445 checks in 1,281 ms;
the feature matrix passed all configured native, `wasm32-unknown-unknown`, and
`wasm32-wasip1` lanes in 51,535 ms and ended with
`capability tables OK: every native and wasm32-wasip1 lane agrees`. Coverage
MCP run `7a938bac-8dff-4ba9-96f4-dea15dda6ebe` passed 85/85 tests in 123,268 ms
and ingested snapshot `d3036cb7-1ea5-4fce-8ec2-abaf17950c32`, reporting
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
`c1e765f6-4636-41a0-9771-961f004a7731` had one native-AVIF sink byte-identity
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

The managed Pillow parity run `f3077d75-2370-48cc-9845-fdd9cfa6f698`
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

The VP8L traced backward-reference dynamic-programming pass is now a documented
checkpoint: token-aware calls poll every 256 processed pixels and the
const-specialized no-token path retains its 1,024-pixel cadence. Remaining work
is finer WebP bitstream and other interior work beyond the
current 8-bit/16-bit/32-bit/64-bit/128-bit/256-bit/512-bit/1,024-bit/2,048-bit/4,096-bit/8,192-bit first-partition, 8-bit/16-bit/32-bit/64-bit/128-bit/256-bit/512-bit/1,024-bit/2,048-bit/4,096-bit/8,192-bit coefficient, 8-bit/16-bit/32-bit/64-bit/128-bit/256-bit/512-bit/1,024-bit/2,048-bit/4,096-bit/8,192-bit/16,384-bit VP8L
bitstream, the 64-symbol VP8L histogram/cost/merge/backward-reference scans,
64-code-length-symbol Huffman RLE preparation, 16-emitted-token Huffman-RLE
materialization, and canonical-code assignment,
64-candidate-node Huffman-tree insertion scans, 16-compressed-token-entry
Huffman-tree frequency scans, and 1,024-pixel RGBA cleanup
checkpoints, JPEG interior work beyond
the current 1,024-pixel RGB-to-YCbCr and chroma-downsample output, completed 8x8 JPEG
forward-DCT/quantization-block, optimized baseline Huffman frequency gathering,
progressive scan block slots, event-frequency items, and coefficient traversal
items, and 1,024-byte entropy intervals, other codec
interior and transient-allocation boundaries,
short-write/rollback semantics, and the other roadmap categories below.

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
| VP8L property map | `tests/fixtures/webp_vp8l_property_map.json`; `scripts/inspect_webp_vp8l_structure.py` and `scripts/verify_webp_vp8l_property_map.py` | Named active WebP fixtures plus independently parsed VP8L structural facts and malformed parser code/phase/bit-offset witnesses, with their Pillow outer-result origin and current hashes | Proof that Pillow itself selected any internal VP8L state named by a candidate fixture |
| Fixture benchmark protocol | `scripts/benchmark_fixture_workloads.py` schema-`@3` | Clean-revision workload timings with a fixed four-worker test-harness budget, manifest/matrix hashes, native release/WASM compile artifact sizes, and direct-child POSIX CPU/peak-RSS observations with parity and Rust-only provenance kept separate | Universal performance or process-tree memory claims, allocator counts, retained-cache size, stack, caller-buffer-reuse, or WASM-runtime claims |

The aggregate line, branch, function, and region totals must therefore never
be described as "Pillow parity coverage". A defensive contract may contribute
executed lines to the aggregate report without becoming a generated parity
row, and a private coverage model may exercise an unreachable state without
being user-facing behavior.

### VP8L property-to-fixture map

`tests/fixtures/webp_vp8l_property_map.json` is the compact property map for
the active lossless WebP corpus. It is pinned to implementation revision
`487348d01389eb8d100b8a668c9921d97634c022`, manifest SHA-256
`bffa47f55b0a4ef2d64979392410e7544617fcebdedcd4086cd76532a4c936e3`, and
generated-matrix SHA-256
`b087396b064ed216a03ed789d9a6171d1f97ec99491f2f90f0c134bce29bf510`.
The map also pins the independent inspector SHA-256
`833f0926c1a931a24087ae8dea3d199f11e6c236c50f90c97ae657aac40af541`.
`python3 scripts/verify_webp_vp8l_property_map.py` currently verifies 14
properties, 68 named witnesses, 79 distinct active WebP rows, 46 successful
structural witnesses, and 40 malformed parser witnesses.

The frame-header, color-indexing size-band, subtract-green, color-transform,
meta-Huffman, entropy-image, successful cache-boundary, simple-Huffman-tree,
full-Huffman-tree, distance-mapping, and three malformed-form properties
are
`witnessed` only at their explicitly listed scopes: the color-indexing rows
remain Pillow-origin outer-result fixtures while the independent inspector
proves the transform and table-size fields, the subtract-green encode artifact
is parsed independently, the color-transform rows prove the selected block
sizes, the meta-Huffman rows prove the selected one- and two-group forms, the
entropy-image rows prove the selected 2×1 and 24×24 dimensions, and the cache
rows prove the selected 1- and 10-bit widths, and the full-tree rows prove the
two listed high-entropy forms, the simple-tree rows prove the two listed
successful forms, the distance rows prove the two listed mappings, and the
malformed groups prove their rejection code/phase/offset records. The independent
parser now sees predictor mode values 0–13, including mode 4 from the existing
Pillow encode artifact; the predictor-mode property remains the one broad
`candidate` at the full-category level because its source-pattern and
transform-combination claims are wider. Its named rows are Pillow-origin
outer-result fixtures, while the 46 successful structural
witnesses independently establish only
selected transform, meta-Huffman, color-cache, Huffman-tree, distance, and
entropy-image facts. The 40 malformed witnesses independently check rejection
code, parser phase, and bit offset (including Pillow-tolerated malformed
streams that the inspector accepts); those fields are specification evidence,
never Pillow diagnostics. The map adds no synthetic parity row,
`cfg(coverage)` hook, or Rust unit test. The remaining WEP-022 work is to expand the successful
structural witnesses to every claimed combination without changing the Pillow
parity claim. The verifier requires every minimal witness named by a
`witnessed` property to have a matching successful structural or malformed-parser
witness before that status can be promoted.

### Revision-bound fixture benchmark protocol

`scripts/benchmark_fixture_workloads.py` is the executable protocol for the
remaining QA-010/QA-030 measurement work. It refuses a dirty worktree by
default and emits the current commit, host/toolchain identities, manifest and
generated-matrix hashes, active-row summary, and one record per selected
workload:

```bash
python3 scripts/benchmark_fixture_workloads.py --output /tmp/image-star-benchmark.json
```

The `pillow_parity_fixture_suite` record runs only the generated
`coverage_matrix_tests` workload. The separate
`rust_non_parity_feature_gate_suite` record runs the existing
`feature_gate_tests` contract, whose cancellation, work-budget, sink, policy,
and structured-diagnostic fields have no Pillow result field. Release-library
and `wasm32-unknown-unknown` compile workloads record artifact sizes. These
are revision-bound observations for a fixed host/cache/toolchain, not universal
benchmarks. On POSIX, schema-`@3` collects `reported_user_seconds`,
`reported_sys_seconds`, and `peak_resident_bytes` with `peak_resident_source` set
to `posix_wait4_direct_child`. The peak value is a direct-child observation and
must not be read as a universal process-tree or allocator measurement; non-POSIX
hosts report it as unavailable. Allocation counts, retained encoded/decoded cache
bytes, caller-buffer reuse, peak stack, and WASM runtime time/memory remain
unmeasured until dedicated collectors exist.

The schema-`@3` workload fixes both Cargo test targets at
`--test-threads=4`. This bounded parallel budget represents normal local
execution without allowing host CPU count to change the measured command. The
parity harness also shares immutable, fixture-derived source sequences across
its partitioned test functions; mutable sequence-option cases clone that
cache. Neither scheduling change changes the 1,024 decode/393 encode fixture
denominator, adds a parity row, or turns the Rust-only feature-gate contract
into Pillow evidence.

A clean local run at test/runtime revision
`1964d6752a140e24bb1af86a6342d5abbd1f72de` passed all four workloads on the
arm64 macOS host with the pinned Rust 1.96.1 toolchain. Its observations were
2.424575 s wall / 3.078958 user / 0.635682 sys / 256,999,424-byte peak RSS for
the Pillow parity suite; 1.429546 s wall / 2.108534 user / 0.123127
sys / 191,971,328-byte peak RSS for the separate Rust-only feature-gate suite;
7,981,248 bytes for the native release `rlib`; and 25,178,233 bytes for the
`wasm32-unknown-unknown` determinism test artifact. The two peak values are
direct-child POSIX observations. These values are a revision-bound execution
record, not a universal benchmark or a parity claim for the Rust-only workload.

### Feature and target matrix

The feature script checks:

- no features;
- each codec feature independently;
- default features;
- all features;
- native feature-gate tests for every feature lane;
- matching `wasm32-unknown-unknown` library Clippy and rustdoc builds;
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
are present, the scheduler treats the root as warm: it allows one independent
lane per logical CPU, capped at 24, uses one Cargo compiler worker per lane, and
derives the test-harness budget as `floor(logical_cpus / MATRIX_JOBS)`, with a
minimum of one and a maximum of eight workers per lane. On the measured 12-CPU
host, the warm default is therefore 24 lanes and one test worker per lane.
`MATRIX_JOBS`, `MATRIX_TEST_THREADS`, and `MATRIX_BUILD_JOBS` can override the
derived values for a constrained or unusually large CI runner. This bounds
aggregate process, compiler, and test-thread fan-out without dropping any lane
or assertion while avoiding a cold-build fan-out on disposable roots.

Successful lanes report one compact status line by default. Their complete
logs remain in the run-scoped directory so the capability-table no-drift check
can consume the emitted rows, and a failed lane always replays its full log.
Set `MATRIX_VERBOSE=1` to replay every successful lane log as well. Keeping
successful compiler, test, and WASI output out of the parent process reduces
test-run I/O without changing any lane, assertion, or retained evidence.

The unknown-target Clippy lanes lint the library surface only: native and WASI
lanes already compile and execute the complete feature-gated integration target
for every feature selection, and the unknown-target `none` and `avif` lanes
retain explicit test-compilation checks. This avoids rebuilding the same
integration targets in all eleven compile-only lanes without reducing target,
feature, or capability-table coverage.

The matrix defaults `MATRIX_TEST_OPT_LEVEL` to `2`, matching the repository's
regular `opt-level = 2` test profile. Callers may override this value when
compile time matters more than runtime.

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
a coverage hook. It currently accounts for 222 exact `#[cfg(coverage)]` guards
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
