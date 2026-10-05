# Versioned compatibility evidence

Fixture declarations, executed comparisons, source coverage, and release
publication have different identities and denominators.
The full-coverage section records the last full local report; later entries
retain the report-specific observations made when each entry was recorded.

## Local artifact availability and exporter investigation — 2026-10-05

An owner-confirmed routine cleanup removed the local `target/` directories,
including the raw profiles, compiled objects and receipts cited below. Their
recorded digests remain historical identities; unavailable artifacts have not
been recreated or represented as surviving originals. Fresh measurements are
required for further comparisons. New essential receipts are being retained
outside build directories.

Before cleanup, a native exporter investigation found positive raw-profile
counts for 64 API functions reported as uncovered in the last full JSON.
Reordering the identical complete object set changed the exported function
coverage. The table below preserves the original exporter output, but these
mapping losses prevent interpreting its changes as missing execution or a
test regression. The diagnosis retained after cleanup is a context
reconstruction, and a native repair must be validated with fresh artifacts.
The repair must prefer real mappings over unused mappings, including real
functions with zero executions, while retaining genuinely unused functions
and every reported source. No corrected coverage result is claimed here.

## Last full all-feature coverage — 2026-10-05

The last recorded full local report was
`target/release-evidence/coverage-avif-derived-skip-refs-v2-20261005.json`, SHA-256
`32b12a55dddb5a2488f8c09bfb67a02feb42dab40fa0ac21697c0b151983e0e3`.
`make coverage` uses `nightly-2026-07-16`, an empty `RUSTC_WRAPPER`, two build
jobs, `CARGO_INCREMENTAL=0` and a fresh
`target/llvm-cov-avif-derived-skip-refs-v2-full-20261005` target.

| Metric | Covered / total | Percent |
| --- | ---: | ---: |
| Lines | 88,832 / 135,656 | 65.483281% |
| Branches | 17,359 / 31,196 | 55.644954% |
| Functions | 4,921 / 8,682 | 56.680488% |
| Regions | 135,986 / 210,661 | 64.552053% |

All alpha floors pass. The separate strict verifier exits 1: all four 100%
requirements remain unmet. All seven test binaries pass 3, 4, 1, 57, 7, 1 and
68 tests, with zero failures or ignored tests. The current ordinary inventory
contains 2,070 rows: 1,607 active decode, 431 active encode and 32 planned
encode. AVIF has 525 decode rows, JPEG 216 and TIFF 200. All 25 target-only
fault contracts retain oracle status `not_applicable` and separate results.
The malformed-input ledger remains at 562 entries.

The incremental and full campaigns preserve all 5,190 measured
source/configuration/script/fixture hashes: 5,189 repository files and one
ignored ambient Finder file. The repository-local count includes the ignored
`.cargo/config.toml`; 5,188 measured files are tracked in Git. The local
configuration sets `CARGO_INCREMENTAL=0` for child commands and selects a
compiler wrapper. The campaign explicitly sets the parent Cargo environment
to `CARGO_INCREMENTAL=0` and overrides the wrapper with an empty
`RUSTC_WRAPPER`. Both ignored files retain
separately verified hashes. The full campaign has its own 113 instrumented
artifacts and seven fresh profiles, unchanged across JSON and LCOV exports.
The selected campaign's 63 artifacts and final profiles remain unchanged.
Different test scopes use separately bound objects; no cross-scope object
identity is claimed. The original full source receipt retains precommit base
revision `61f4dfdc145f2a35526588667368d5873df07280` and SHA-256
`dc6f39d09a0a1424857fd8c72ebb435bd084f6870cd47a752d8cdbf9549cf582`.
Human evidence documentation is outside that measured inventory.

All eight formatting/strict-Clippy gates pass, including coverage-nightly,
JPEG benchmark and SSE2/AVX2 cross-compilation. Their v2 receipt has SHA-256
`7ecdc21d94aa08a4c3f9ba76c947fdc2c3eed1e1607d9fbf2823c94aeafb892e`.
All 33 feature lanes pass: 11 native runtime, 11 WASI runtime and 11 browser
compile/strict-Clippy/rustdoc lanes. The v2 feature receipt has SHA-256
`5b5451ff599288230aeaae13b032bde06b0f5d7d6fea48065417d98d695da871`.
These cross-target checks provide compilation evidence, without matching x86
runtime or throughput measurements for this change. The origin registry
retains 140 guards across 20 files. This change adds no private unit tests,
unsafe code, lint exceptions, coverage exclusions or ignored cases.

### Complete AVIF disabled-skip reference parity — 2026-10-05

Three complete ordinary lossless animations exercise I420, I422 and I444
sampling with five 64×64 frames and fixed 32×32 blocks. At the observed inter
context, the header retains a derived skip-reference pair while skip mode is
disabled. The reconstruction gate now permits that unused pair, retaining
the existing exclusion of enabled skip mode and all reference, quantizer,
geometry and safety checks. The change removes one context predicate and
adds no pixel allocation, raster copy or block-loop operation. It has no
measured speed claim. Execution evidence covers bounded reconstruction for
these three complete inputs.

| Ordinary input | Bytes | Complete input SHA-256 |
| --- | ---: | --- |
| `animated_lossless_inter_420_derived_skip_refs_b32x32_64x64` | 6,130 | `50d8bdd33c5a5e958d5991d25767748088ba3354d35d4b809d52e84a45713c15` |
| `animated_lossless_inter_i422_derived_skip_refs_b32x32_64x64` | 7,530 | `0eae260be09a74b98b1f525261a237ef3783105870de44224de91dd65ee5d168` |
| `animated_lossless_inter_i444_derived_skip_refs_b32x32_64x64` | 10,135 | `3769c145bb4d97f357a8b3c4660fb8a2f976c682643304e64833d06515901009` |

The maintained `scripts/generate_avif_derived_skip_reference_fixtures.py`
runs through the normal AVIF asset hook. Recipes retain stimulus and complete
input hashes; fresh pinned Pillow 12.2.0 owns expected pixel and lifecycle
observations. Two separate output-directory generations and `--check`
reproduce all three input files. The generator receipt has SHA-256
`a618607002514ab86a69a686d24ecb6030c5c952678296526341600f9fb8d305`.
The normal native loop producer independently observes all five displayed
frames, exact pixels, 100 ms timing and loop zero. Its index grows from 88
to 91 cases and 267 to 285 artifacts. All previous cases and artifact bytes
remain exact. The new index SHA-256 is
`38e8bf690190b189c26fb93876ef7c7b47b67ca57fa4c99c49b819b0ef58f13d`.

Canonical regeneration preserves all 522 prior AVIF ordinary outcomes and
every other format/encode block. Only the native index digest changes in the
63 existing AVIF sequence provenance fields in each of the matrix and output
JSON. All 25 fault contracts and historical managed-ledger measurements,
run identities and measured-input hashes remain exact. An isolated public
probe also matches seven complete ordinary inputs, 29 sequence frames and
seven first images against fresh Pillow observations; its receipt has
SHA-256 `9774993f56582350a1c7ce44e02295a14087d47f97d9cf069477223d1ee56a23`.
Independent source review finds no required repairs and binds the retained
skip-mode exclusion, all 18 new first-image/sequence references, old outcomes
and native artifacts. Its receipt has SHA-256
`c8e17b450b2ba14ec348b861c1ac9ad5461babb3605475f18102637e437ee504`.

The first full attempt fails on the existing native observer's stale
267-artifact assertion. The repair updates that observer to 285 artifacts,
appends the three case registrations, shifts five origin line coordinates by
one and refreshes the ledger's current origin digest. A subsequent manual
formatting attempt also fails `rustfmt`; its unsuccessful evidence is kept.
The authoritative `cargo fmt` output is used by every successful v2 campaign.
The original incremental/feature successes, failed full attempt and formatting
failures remain separate from the passing v2 receipts.

#### Incremental measurements and coverage limits

On the fixed v2 source, the baseline executes 522 prior ordinary AVIF rows
and 24 applicable AVIF fault contracts, reported separately. Three subsequent
runs execute one new ordinary input each through the existing matrix runner.
All four runs pass, resetting raw profiles between runs and preserving all
63 actual instrumented objects. The incremental receipt has SHA-256
`301313e5e2c007b55b3fdb88420d94e4abafd83e19a40649a73c3f8f3efc1609`.
The read-only producer audit binds original report hashes, 110 measured source
files, all 5,190 frozen files, actual objects and passing selected-row logs.
Its SHA-256 is
`d67650d5986a300b9038b0a18c77b0129a8bec462009fc5c37fd65e444b00104`.

Coverage MCP verifies the LCOV line-coordinate union
**43,973 → 44,032 / 131,155**, with marginal gains of 58, one and zero.
The 58 coordinates are in existing code: 40 forwarding-wrapper lines in
`block.rs` and 18 compound-reconstruction lines in `entropy.rs`. The I422-only
gain is a short-circuit right-hand evaluation in
`bounded_reference_mode_supported`; it does not show enabled skip mode.
The zero-gain I444 case retains independent parity value. Native LF/LH totals
and the full report's aggregate metrics retain their separate denominators.
JSON region and branch comparisons are `incomparable`: region summary/detail
covered counts disagree, and native branch detail loses four identities.
No JSON producer context, replacement counters or synthetic identities are
introduced. The verbatim query receipt has SHA-256
`0dee6bf2dc3a8842f191176cc4ddfed258f9018aae122e132168b9ba47263f53`.

All four native full covered counts are lower than the preceding TIFF report.
Source, fixture inventory and build identity differ, so these reports do not
provide an exact full-suite regression comparison. The 59 selected line gains
are not a claim that 59 preceding full-suite gaps disappeared. The current
full report retains 46,824 missing lines, 13,837 missing branch outcomes,
3,761 missing functions and 74,675 missing regions. Proofs are under
`target/release-evidence/avif-derived-skip-v2-61f4dfdc-20261005/`, with original
reports and receipts beside it; canonical producer evidence is under
`target/release-evidence/avif-derived-skip-canonical-61f4dfdc-20261005/`.

## Preceding TIFF associated-alpha full campaign — 2026-10-05

The preceding TIFF full local report is
`target/release-evidence/coverage-tiff-associated-alpha-20261005.json`, SHA-256
`97b19bdff5d96774f0b4499a7177bbc25d0746eb51477cda829f201d44056e87`.
`make coverage` uses `nightly-2026-07-16`, an empty `RUSTC_WRAPPER`, two build
jobs and the dedicated `target/llvm-cov-tiff-associated-alpha-20261005` target.

| Metric | Covered / total | Percent |
| --- | ---: | ---: |
| Lines | 89,044 / 135,657 | 65.639075% |
| Branches | 17,363 / 31,216 | 55.622117% |
| Functions | 4,984 / 8,682 | 57.406128% |
| Regions | 136,223 / 210,663 | 64.663942% |

All alpha floors pass. The separate 100% verifier exits 1; all four completion
requirements remain unmet. All seven test binaries pass 3, 4, 1, 57, 7, 1 and
68 tests, with zero failures or ignored tests. The ordinary inventory has
2,067 rows: 1,604 active decode, 431 active encode and 32 planned encode.
TIFF has 200 decode rows, JPEG 216 and AVIF 522. The 25 target-only fault
contracts remain separately reported with oracle status `not_applicable`.

All 5,094 source/configuration/script/fixture hashes remain exact across the
fixed-source incremental and full campaigns: 5,093 repository files and one
ignored ambient Finder file. The original full source receipt preserves its
precommit base revision `3489d205f3acd4698d4c336eadfd1e756e40ff7a`; receipt
SHA-256 is `009eff76925febd98014667645a7fae0233f87eaa02020fde320d48d56999d32`.
Human evidence documentation is outside that inventory. Native full aggregate
totals and selected-report coordinate totals remain distinct.

All eight formatting/strict-Clippy gates pass, including coverage-nightly,
JPEG benchmark and SSE2/AVX2 cross-compilation. All 33 feature lanes pass with
all 5,004 captured file hashes unchanged: 11 native runtime, 11 WASI runtime
and 11 browser-target compile/Clippy/rustdoc lanes. The feature receipt has
SHA-256 `b8773060f79d62247a44b732187028308116bbee26c47677a238778d41c910ba`.
The origin inventory remains 140 guards across 20 files. Private unit tests,
unsafe code, coverage exclusions and ignored cases are absent from this change.

### Complete TIFF associated-alpha parity — 2026-10-05

Twenty-two complete ordinary inputs cover contiguous/separate sample planes,
multiple strips, clipped edge tiles, raw/Deflate/LZW/PackBits compression and
per-plane horizontal prediction. The maintained
`scripts/generate_tiff_associated_alpha_fixtures.py` is called by the normal
TIFF asset hook. It includes nonzero hidden RGB at alpha zero, stored colors
above alpha, alpha 1/2/17/254, opaque samples and payloads crossing cancellation
checkpoints. Two output-directory generations and `--check` reproduce every
input byte. All 180 previously tracked TIFF inputs and 178 prior decode rows,
59 encode rows, encoded reference JSON, other-format rows and 25 faults remain
exact. Inputs contain storage samples and selectors; the live oracle owns all
normalized expected pixels and failures.
The final 22-input regeneration receipt has SHA-256
`183af3b4daf63b2158bd59f8ea44dce8bc526c94a8b388c0ae5e39267adb6407`.

Pinned Pillow 12.2.0 independently establishes inspection/verification success
for all 22 inputs, 18 exact pixel successes and four materialization failures.
The raw separate importer accepts three declared RGB planes with the alpha
plane omitted, retaining colors with zero alpha. Declaring the first alpha
block instead fails with `ValueError: unknown raw mode for given image mode`.
These boundary inputs retain every payload and byte-count byte; only the
offset count changes. They are ordinary parity cases. Their malformed-ledger
specification status is `ambiguous`, because the complete TIFF storage is valid
and the reference's raw band importer lacks that decoder. The ledger preserves
558 prior classes and adds four, for 562.

Associated RGBA normalization now runs in place: transparent pixels clear,
opaque pixels retain their channels, and intermediate alpha uses truncated
`channel * 255 / alpha`, clipped to 255. The source descriptor still records
`SourceAlpha::Premultiplied`. Raw separate planes without an alpha block retain
their reference-defined transfer samples. A shared pixel helper and 1,024-byte
cancellation chunks add no pixel allocation or raster copy. One local Clippy
expectation documents the exact arithmetic invariant: byte multiplication is
at most 65,025 and the division arm's alpha is 1..=254. It excludes no source
from coverage. This correction has no throughput claim.

The preserved old binary mismatches all 18 initial inputs: 16 wrong normalized
rasters and two incorrectly successful raw planar decodes. Two further first
alpha-block inputs also expose the error; two omitted-plane controls already
match and preserve that leniency. The fresh public probe matches all 22 final
cases, with all inspection/verification boundaries retained. The canonical
matrix supplies wrapper, cancellation, metadata, sequence and structured-error
checks. The focused green receipt has SHA-256
`cdaa78609447990e5f58d2a92592c96257668cdcdd6adb3b3cd35360ee510e7b`.
The primary behavior is Pillow's `Unpack.c` RGBa unpacker and `TiffDecode.c`
separate-plane completion, pinned to tag 12.2.0 and preserved in ignored proof.
The tag resolves to commit `3c41c095064200a02672d89cc5ff629eaf4b0d4f`;
the saved commit-addressed source text is provenance and was neither compiled
nor executed.

#### Incremental measurements and producer receipts

Changed source is measured afresh: 178 prior ordinary TIFF rows, then batches
of eight contiguous, eight compressed separate and six raw boundary rows.
Every run passes with all 387 instrumented artifact hashes fixed. Raw profiles
are reset independently and fresh nonempty profiles are required. Initial
Cargo execution including compilation takes 124.120 seconds; later batches
take 0.843–1.003 seconds under concurrent checks. These are workflow timings.

Audited optional producer sidecars bind original report hashes, all 110 measured
source files, the frozen build identity and passing canonical row logs.
Coverage MCP verifies region union **6,116 → 6,153 / 210,643**: gains are 28,
one and eight. The separately audited LCOV line union is verified
**3,967 → 3,999 / 131,156**. These selected denominators differ from full
aggregate totals and do not establish a full-suite regression comparison.
JSON line and branch unions return `incomparable`; branch detail contains
31,210 coordinate arms against 31,216 native arms. Those responses are kept
without replacing native totals or inventing missing branch identities.

The full run still has 46,613 missing lines, 13,853 missing branch outcomes,
3,698 missing functions and 74,440 missing regions. In particular, selected
gains include newly implemented conversion behavior and are not a claim that
37 historical full-suite gaps disappeared. Historical managed ledger metrics,
run/snapshot IDs and measured-input hashes remain unchanged. Evidence is under
`target/release-evidence/tiff-planar-alpha-3489d205-20261005/`, with reports and
feature receipts beside it. Wider ExtraSamples-zero and LA layout differences
from the exploratory 128-input family remain unimplemented ordinary gaps.

## Preceding JPEG scan-declaration full campaign — 2026-10-05

The preceding JPEG full local report is
`target/release-evidence/coverage-jpeg-sos-20261005.json`, generated by
`make coverage` with `nightly-2026-07-16`, an empty `RUSTC_WRAPPER`, two
Cargo build jobs and the dedicated `target/llvm-cov-jpeg-sos-20261005` target.
Its SHA-256 is
`5b6ca500bae7c9d3d2add1b9145c7ccf15afbc7879056e43853388a7cff67b0d`.

| Metric | Covered / total | Percent |
| --- | ---: | ---: |
| Lines | 88,995 / 135,609 | 65.626175% |
| Branches | 17,343 / 31,196 | 55.593666% |
| Functions | 4,983 / 8,681 | 57.401221% |
| Regions | 136,168 / 210,608 | 64.654714% |

All alpha floors pass. The separate strict verifier exits 1 because all four
100% requirements remain unmet. All seven test binaries pass 3, 4, 1, 57, 7,
1 and 68 tests, with no failures or ignored tests. Its report-era ordinary inventory
contains 2,045 rows: 1,582 active decode, 431 active encode and 32 planned
encode rows. JPEG has 216 ordinary decode rows and AVIF retains 522. All 25
target-only fault contracts remain separately counted, with their reference
status `not_applicable`.

All 5,053 recorded source/configuration/script/fixture hashes are unchanged
between the fresh incremental and full campaigns. This inventory contains
5,052 repository files and one ignored ambient `tests/fixtures/.DS_Store`.
The original source receipt retains precommit base revision
`455d66aa3a8aa7e2bf6ac1da8c75198c8d96f316`; its SHA-256 is
`fa9e7c99f5d287bd28509077d2c6bd57608c0657e2f98b343b580aeeb5492977`.
Human evidence documentation is outside that measured inventory.

All eight formatting/strict-Clippy gates pass, including debug, release,
coverage-nightly, JPEG benchmark and SSE2/AVX2 cross-target checks. All 33
feature lanes pass with 4,959 recorded file hashes and capability artifacts
unchanged. Native/WASI lanes execute tests; browser WASM lanes compile and
run strict Clippy/rustdoc. Cross-target SIMD checks establish compilation and
lint evidence, without x86 throughput measurements for this change. The
origin registry retains 140 guards across 20 files. No private unit tests,
unsafe code, source exclusions, ignored cases or lint exceptions are added.

Coverage MCP reads the full report with source unverified and tests unknown.
It groups 74,440 missing regions into 6,647 missing-region groups, with AV1
inter-translation instantiations still the largest displayed groups. Missing
counts also include 46,614 lines, 13,853 branch outcomes and 3,698 functions.
Local command/hash receipts do not create a managed MCP source/build binding
or named-test attribution. Historical managed ledger metrics and identities
remain unchanged; current integrity hashes and inventory counts are separate.

### Complete JPEG scan-length and component-count parity — 2026-10-05

Eleven complete public inputs preserve the source entropy, scan order and
final EOI bytes, changing only the selected SOS length and, where declared,
component count. The maintained `generate_jpeg_sos_edge_fixtures.py` uses the
normal JPEG asset hook, pinned source/output hashes, bounded shared marker
walking and fresh pinned Pillow lifecycle calls. Two separate output-directory
runs and `--check` reproduce all eleven bytes exactly; all 216 prior JPEG
input files remain unchanged. Inputs contain no Rust outcomes.

| Ordinary case | Bytes | Complete input SHA-256 |
| --- | ---: | --- |
| `error_sos_length_0` | 4,564 | `729419da5fe9727a0f6d751a6865c55e44bcd95094bf153c06cc7e382ce2e4d0` |
| `error_sos_length_1` | 4,564 | `a744fc417d07b52a16f2dd1f3ac0f7e784803c64b88423c2342f07c1d1ff7223` |
| `error_sos_length_2` | 4,564 | `066f03bcef810cb346ff6f55ffdb1cbab465b375d1832e894a542d2706062929` |
| `error_sos_length_11` | 4,564 | `5f235865d0e748c1f67a12d63ac883a43c73eea66cd255ae44f7e47a1534e960` |
| `error_sos_length_13` | 4,564 | `121633f20c3b9b7e4fc33e80ea817fbf6d7e4cab6a6eab2d4ab791cad029ed3d` |
| `error_sos_length_65535` | 4,564 | `63c54401762bd11614feb80554c736d1d8c80ba8be003c9d570224deefd47edb` |
| `error_sos_zero_components_length_6` | 4,564 | `538b1e21c0c658ef42b5d053ff6c379c0f7ea9a5f40ee2f53140ae3a4f8754fa` |
| `error_sos_five_components_length_16` | 4,564 | `e192d29496bf30ab3f4ea34e2f8dc5fb3608ad66942475153695dc10b5d2c393` |
| `error_sos_255_components_length_516` | 4,564 | `fffe6aba1f8e5ed5d232db2bc6eded2ddaaf20c4b4b13c11d1577cf6dbd3e4b8` |
| `error_progressive_second_sos_length_7` | 3,939 | `eb1f33e9234cb7b8ab4ebabfb199c3eb255107a67b3e9ce468eaf81b07c0298e` |
| `error_baseline_multiscan_second_sos_length_7` | 649 | `d5dd86cf6d57ea40449b16e137de0a9403512c831f5d4bcee6c995ea8ee6c232` |

Pinned Pillow 12.2.0 with bundled libjpeg-turbo 3.1.4.1 detects every input as
JPEG. Ten inputs open, inspect and verify with the unchanged source metadata,
then both pixel workflows fail with `builtins.OSError` and
`broken data stream when reading image file`. The UINT16_MAX declaration
instead fails at open with `Truncated File Read`. Three complete valid source
controls and additional valid one-/four-component controls retain exact
Pillow pixels.

The old target incorrectly materialized all six length mutations and both
later-scan mutations. Lengths 0/1 also incorrectly failed inspection and source
opening for verification. The three count mutations already failed as malformed
input; they now validate the reference's scan-shape boundary before component
reads. They are regression cases, not additional claimed old mismatches.

Inspection now stops at SOS lengths 0/1 after the bounded length read, matching
Pillow's open behavior. Normal payload bounds, truncated length fields and
other markers retain their existing handling. Pixel parsing reads the length
and count, preserves the existing zero-component error, then requires at most
four components and exactly `2 * count + 6` bytes before allocation or table
reads. The saturated u16 arithmetic is exact for every byte-sized count
(maximum 516); strict Clippy passes without a new allowance. The check runs
once per scan and adds no pixel-loop work or copies. This is a compatibility
and validation fix, without a measured throughput claim.

The exact primary source is tag 3.1.4.1, commit
`9217719d3a58633923b096af4c1d50d304768a64`. Stored tag and commit-addressed
`jdmarker.c` bytes both have SHA-256
`1dd922aff8f92ece4f8f2b11eea08cd49b2e232de797ee98db9e331f6d0e3296`;
its `get_sos` reads length and count before validating the shape. Independent
review reproduced the complete inputs and verified all entropy/tails without
calling the repository generator or either codec.

The preserved red probe matches the root dependency lock, including wide 1.7.0.
The fresh green probe matches all eleven ordinary outcomes, preserves exact
Debug/display diagnostics, stages, offsets and identities of all seventeen
prior SOS rows, and preserves complete metadata/pixels for five normal
controls. Static JPEG sequence failures retain their existing `StillDecode`
stage. Initial observer field/lock/tooling failures and the first green
observer's executable-mode failure remain unsuccessful separate evidence;
no result from them becomes a passing receipt. The final green receipt is
`public-green-receipt-v2.json`, SHA-256
`13a8f62b4ed4346a1178bcbbdd8d16a343e7347f2adf3c6198dbb214469d2d79`.
Canonical Pillow regeneration preserves all 205 old JPEG decode rows, 81 encode
rows, encoded/raw bytes, every other format row and all 25 fault rows. The
malformed ledger preserves 547 prior classes and adds eleven, for 558 total.

#### Incremental execution and Coverage MCP limits

Changed source requires a fresh baseline. Twelve selections use the same
finished source and identical hashes for all 387 compiled artifacts: the
previous 205 ordinary JPEG rows plus its one existing target-only allocation
fault, followed by the eleven ordinary cases individually. Every selection
passes. The fault remains `oracle_status: not_applicable`; no new injected
failure is needed for input-reachable scan validation.

Each selection resets only raw profiles, uses `--no-clean`, and requires fresh
nonempty profiles before reporting. The first selection takes 95.456 seconds
including the build; subsequent Cargo executions take 0.789–0.828 seconds.
These are workflow timings with concurrent checks, not codec throughput.

Coverage MCP and the independent LCOV union agree on 5,630 to 5,634 covered
line coordinates out of 131,108. The first length-zero case adds four; the
other ten retain distinct boundary/later-scan regression value with zero
additional line-coordinate gain. This selected coordinate denominator differs
from the full aggregate line denominator above. No earlier-source report or
selected-test absence is used to claim a regression.

Both LCOV and LLVM JSON branch-union requests return `incomparable` because
normalized baseline detail does not match reported branch totals. Those
responses are preserved; no aggregate branch union is claimed. Bounded raw
parser/header observations and the full four-metric report remain separate
from this provider limit. The original line query was limited, with source
unverified and tests unknown. A subsequent read-only audit adapted the immutable
producer receipts into optional sidecars before the TIFF source change.
Coverage MCP then verified **9,155 → 9,157 / 210,588 regions** for all eleven
JPEG cases, with no report or source mutation. Branch union remained
incomparable: 31,190 coordinate arms versus 31,196 native arms. This historical
fixed-source result and its producer audit are preserved in the JPEG bundle;
it is not a comparison against the later TIFF source.

The ignored evidence bundle is
`target/release-evidence/jpeg-sos-455d66aa-20261005/`, with the original failed
attempts, pinned primary source/inputs, generator and independent reviews,
red/green outcomes, preservation/accounting receipts and strict checks.
Incremental/full LLVM reports, source receipts, MCP responses and the 33-lane
feature receipts are beside it in `target/release-evidence/`.

### Rejected JPEG packed quantizer correction deletion (2026-10-05)

A separate worktree at `78fc8c8c24126b6c095f3746569dba0bfaf94921`
tested removing the ceiling-reciprocal correction from the private packed
four-block JPEG quantizer. An independent review checked all 13 byte-origin
callers, the clamped quantization tables, pinned `wide 1.7.0` arithmetic, and
the reciprocal bound. The arbitrary-i32 quantizer retained its correction.
The candidate passed strict Clippy and formatting, 81/81 public JPEG encode
rows (379 calls, zero panics), 205/205 decode rows, and all 20 complete-byte
baseline/candidate output comparisons before each timing campaign.

The fixed `paired-five-rounds` and `paired-five-rounds-reversed` campaigns
(local sessions 53108 and 36102, both exit 0) each completed 40 whole-public-call
workloads across 20 configurations and five rounds, with opposite starting
order and the unchanged TurboJPEG 3.2.0 oracle following each Rust variant.
Each campaign saved 800 raw records. Source, binary, and oracle-library hashes
were unchanged. The Apple M3 Pro ARM64 host used macOS 15.7.7 and Rust 1.96.1
with LLVM 22.1.2; the Rust builds used ordinary release settings and the C
harness used `-O3` with TurboJPEG SIMD enabled.

Ratios below are candidate/baseline median paired times, first/reversed
campaign. Values above 1 mean slower complete calls.

| Encode workload | Raw ratio | Oracle-normalized ratio |
| --- | --- | --- |
| RGB 128x128 Q10 4:2:0 | 1.0727 / 1.0729 | 1.0727 / 1.0725 |
| RGB 512x512 Q10 4:2:0 | 1.0685 / 1.0825 | 1.0729 / 1.0886 |
| RGB 1024x1024 Q85 4:2:0 | 1.0290 / 1.0433 | 1.0323 / 1.0411 |
| RGB 128x128 Q85 4:2:2 | 1.0572 / 1.0597 | 1.0552 / 1.0324 |
| Grayscale 128x128 Q85 | 1.0504 / 1.0628 | 1.0550 / 1.0586 |
| Grayscale 512x512 Q85 | 1.0628 / 1.0749 | 1.0610 / 1.0728 |

Of the 17 configurations admitted to the changed encoder path, 15 had slower
raw medians in both campaigns and 14 had slower normalized medians in both.
None improved both raw and normalized medians in both campaigns. CMYK128's
small raw improvement reversed after normalization; RGB512 Q85 changed
direction between campaigns. Three unaffected encode configurations and all
20 decode workloads remain separately reported controls, not a combined score.

No repository Cargo, test or profiling jobs overlapped the timing window. The
first-start process snapshot showed unrelated Python jobs and macOS media
analysis consuming CPU. Those
heavy jobs were absent from the reverse-start snapshot, which retained smaller
background activity. Some controls also varied, including progressive decode;
these measurements do not establish a physically idle host or attribute every
timing change to the quantizer. Repeated material encoder penalties and the
absence of a repeatable acceptable benefit justified rejection. Static ARM64
instruction deletion did not establish lower runtime cost. No numeric SSE2,
AVX2, or world-fastest result is claimed, and the candidate was not applied to
main.

The ignored evidence bundle is
`target/release-evidence/jpeg-packed-quantizer-78fc8c8c-20261005/`: both raw
campaigns and summaries, `comparison.json`/CSV, exact JPEGs, source snapshots,
patch, binaries, build/check/range-review receipts, hardware/load snapshots,
and the rejection and independent measurement review. The baseline encoder
SHA-256 was
`5451cb6570679fe9ea13b385900e5270581220e35cf1993c09eeb0b1a799afab`;
candidate encoder
`df73cfae0d0accc85e54a2e2d11412915d26a54d4778b38153797e59356ae7a7`;
candidate patch
`85548d5ab3a7ee901d022b86a5ccf0e6acb25e2e0646ef934c1c28629a10c5e1`.

## Preceding AVIF container and multi-tile full campaign — 2026-10-05

This preceding full local report is
`target/release-evidence/coverage-avif-multitile-container-v2-20261005.json`,
generated by `make coverage` with `nightly-2026-07-16`, an empty
`RUSTC_WRAPPER`, two Cargo build jobs and the dedicated
`target/llvm-cov-multitile-container-v2-20261005` coverage target. Its SHA-256 is
`027665b7e2cb4e54182b7929ecd568ff83941b839f3708d29277747c4720103e`.

| Metric | Covered / total | Percent |
| --- | ---: | ---: |
| Lines | 88,986 / 135,603 | 65.622442% |
| Branches | 17,338 / 31,192 | 55.584765% |
| Functions | 4,983 / 8,681 | 57.401221% |
| Regions | 136,157 / 210,598 | 64.652561% |

All alpha floors pass. The separate strict verifier exits 1 because all four
100% requirements remain unmet. All seven test binaries pass 3, 4, 1, 57, 7,
1 and 68 tests, with no failures or ignored tests. They exercise 522 ordinary
AVIF decode rows, 25 separate target-only fault contracts and 88 native loop
witnesses. At that report’s source, the ordinary inventory contains 2,034 rows: 1,571 active
decode, 431 active encode and 32 planned encoder rows.

All 5,041 recorded local source/configuration/script/fixture hashes remain
unchanged between the fresh incremental and full campaigns. That inventory
contains 5,040 files committed in the checkpoint and one ignored ambient
`tests/fixtures/.DS_Store`; it is not a count of newly created source files.
The original local receipt preserves its precommit base revision. A separate
Git binding checks the measured files against committed blobs without
rewriting that receipt. Documentation changed after execution is outside this
measured source inventory.

All eight formatting/strict-Clippy gates pass, including debug, release,
coverage-nightly, JPEG benchmark, SSE2 and AVX2 cross-target checks. All 33
feature lanes pass with the 4,940 recorded local file hashes unchanged and
capability artifacts unchanged. Native/WASI lanes run tests; browser WASM
lanes compile and run strict Clippy/rustdoc. Cross-target SIMD checks establish
compilation/lint evidence. They supply no x86 throughput measurement. The
coverage origin registry has 140 guards across 20 files, with no source
exclusions. No new private unit tests, unsafe code, ignored cases or lint
exceptions are introduced.

Coverage MCP reads this full report with source unverified and tests unknown.
It groups 74,441 missing regions into 6,647 missing-region groups; these are
not counts of wholly uncovered functions. Missing counts also include 46,617
lines, 13,854 branch outcomes and 3,698 functions. The largest displayed groups
remain AV1 inter-translation instantiations. Local source/command/hash receipts
do not create a managed MCP source/build binding or named-test attribution.

### Complete AVIF container and multi-tile sequence cases — 2026-10-05

Thirteen new ordinary cases give 522 AVIF decode rows and 2,034 ordinary rows
(1,571 active decode, 431 active encode, 32 planned encode). The 25 target-only
fault contracts remain separate. These are current inventory/reference counts;
execution and coverage claims require completed campaign receipts.

The maintained container-edge and multi-tile-motion generators use the normal
AVIF asset hook, valid bounded BMFF boxes and byte-identical repeated inputs.
Pinned live Pillow 12.2.0, with libavif 1.4.1/dav1d 1.5.3/libaom 3.13.2,
supplies all public outcomes and raw pixels. No Rust outputs become ordinary
inputs or references. Format detection succeeds for every input.

| Ordinary case | Bytes | Complete input SHA-256 | Live Pillow outcome |
| --- | ---: | --- | --- |
| `animated_tkhd_version_zero` | 1,223 | `eb7b131fb8288fbe8e67daf9e41e7f80bdf3f923d474177fac84a0b58550b674` | Open, inspect, verify, decode and sequence decode succeed; all five frames, metadata and timing match the source. |
| `animated_tkhd_version_zero_unknown_duration` | 1,223 | `ca1834317cd4b5b0db00130370cf3db3c94426ff7479c6414c4da2cabfef2a09` | Open, inspect, verify, decode and sequence decode succeed; all five frames, metadata and timing match the source. |
| `ipma_optional_zero_index` | 5,907 | `590514b40d0df05335861bf4345eb99068dc1264d38715f23f8e931c5376aaf3` | Open, inspect, verify, decode and sequence decode succeed with unchanged still pixels and metadata. |
| `error_ipma_essential_zero_index` | 5,907 | `b4f6f62f44d85c84a27bd98c1ad9529453c7ce31fa88602fc88eae0641b06c80` | Open fails with `PIL.UnidentifiedImageError`; inspect, verify, decode and sequence decode report malformed input. |
| `error_ipma_property_index_out_of_bounds` | 5,907 | `ca46ccb69a865f237092cf3ed7fb77862fa3faff112d311f860ecfb91eec77dd` | Open fails with `PIL.UnidentifiedImageError`; inspect, verify, decode and sequence decode report malformed input. |
| `iloc_file_data_reference_1` | 3,077 | `d4b06f63b3bd3ce2bd409930b7c2892236000ff4583e70e771f4a21573dbb6af` | Open, inspect, verify, decode and sequence decode succeed with unchanged still pixels and metadata. |
| `iloc_file_data_reference_65535` | 3,077 | `a9d263c7af3c3d1219cf2187bef5916ace90bf1018dfb6fa58650f99f51be0f7` | Open, inspect, verify, decode and sequence decode succeed with unchanged still pixels and metadata. |
| `iloc_idat_data_reference_1` | 5,906 | `b0689513b0693085ef11397fa967bcc56741ae79b80d8a81d4ef424ce561bea2` | Open, inspect, verify, decode and sequence decode succeed with unchanged still pixels and metadata. |
| `iloc_idat_data_reference_65535` | 5,906 | `756ea5b7a21c472cee85913fbb64260fccd5267f98900dd2b815760738f07eb4` | Open, inspect, verify, decode and sequence decode succeed with unchanged still pixels and metadata. |
| `animated_motion_multitile` | 1,264 | `c248f019a008e9dea4425e1d5f44d1418b44929604312d95a69a0a3ac5d4709f` | Open, inspect, verify, decode and sequence decode succeed; four exact RGB 256×128 frames, each lasting 100 ms. |
| `animated_motion_multitile_split_groups` | 1,280 | `eb19bcebc0698dfa3149c76fb6828705b22e37f45e213b70541a61b3abd20b89` | Open, inspect, verify, decode and sequence decode succeed; four exact RGB 256×128 frames, each lasting 100 ms. |
| `animated_lossless_motion_multitile` | 1,327 | `24612186624c6b470a50a549d876469c8265976633afee87fa030d910cd0532b` | Open, inspect, verify, decode and sequence decode succeed; four exact RGB 256×128 frames, each lasting 100 ms. |
| `animated_lossless_motion_multitile_split_groups` | 1,342 | `04b61cf8b3e9887ee651cec5dc055b5a102198de9f69e2ad64b4e183639cf596` | Open, inspect, verify, decode and sequence decode succeed; four exact RGB 256×128 frames, each lasting 100 ms. |

The two association errors retain `cannot identify image file <bytes>`.
The essential-zero input asset is `ipma_essential_zero_index.avif`.
Track mutations narrow tkhd version 1 to version 0 and rebase the file extent
and chunk offset by twelve bytes, preserving other fields and media bytes.
Both versions, including the `UINT32_MAX` duration sentinel, retain five
frames and native one-play repetition. Association/data-reference mutations
retain every media byte. Split-group variants preserve each encoded tile and
parsed frame-header field, repacking complete sample extents and track tables.

#### Ordinary compatibility fixes

The Q100 pair exposed a real lossless sequence gap. Before the fix, public
still decoding succeeded while both sequence decodes returned
Unsupported/SequenceDecode/NotImplemented with
`decode sequence: AVIF sequence validation failed: AVIF sequence rendering has no completed color surface`.
Pillow decoded four RGB 256×128 frames. Source inspection traced the gate to
comparing a full 256-pixel reference width with a checked 128-pixel tile width;
the public probe alone did not locate the failing sample.

The generic lossless inter gate now admits validated, unreduced full references
shared by checked tiles and preserves the previous admitted-width alternative.
`ScaleFactors` still uses full current/reference dimensions; `scaled == false`
requires exact dimensional equality. Depth/layout, surface, tile bounds and
tool checks remain; local reconstruction/global prediction coordinates are
unchanged. Fixed public sequence decodes of both Q100 variants match all four
Pillow frames exactly, and still pixels equal frame zero. The Q80 source/split
pair retains the same complete parity. This proves the bounded witnesses, not
every possible multi-tile block grammar.

The four nonzero data-reference files exposed another ordinary gap: Pillow
accepts 1 and 65,535 for construction method 0/file and 1/idat; the preceding
Rust probe rejected all four at inspection, still decode and sequence decode.
Pinned libavif `read.c` reads the field without storing/using it; `idatStored`
selects local bytes. Both Rust parsers and the independent full sample inspector
now consume the fallible u16 without rejecting its value. Reserved bits,
supported methods, checked arithmetic and existing file/idat bounds remain.
The field cannot select an external resource. Fixed public still/sequence
pixels match the live Pillow witnesses without a fault label or private
comparator.

#### Native provenance and preservation

Six new native observations cover both tkhd variants and all four motion
inputs. All 82 prior record dictionaries and native artifact records/bytes
remain exact. The 88-record bundle has 80 accepted complete decodes and eight
malformed inputs; 267 artifacts total 9,816,480 bytes, including 9,293,948
Pillow frame bytes. Additional collections bind their actual compiler and
observer-binary identities, preserving original metadata. Native repetition
is 0 for both tkhd variants (one total play), and -1 for the motion inputs
(infinite). The native index SHA-256 is
`38b94714d241f811741a4116bb80cbb9e1479feb8360da99a001bad37037b159`.

Canonical generation preserves all 509 prior AVIF operation observations,
other-format observations and AVIF encode rows. Only
`sequence.loop_evidence.index_sha256` refreshes in 57 output rows and their
57 matrix rows. Existing native identities, input hashes, loop counts, pixels,
durations, errors and flags stay unchanged. All prior 24 fault rows and
1,876 raw files retain exact values/bytes. The 37 new ordinary canonical raws
match stored live Pillow bytes in full. Another 32 new files are six native
witness inputs plus 26 Pillow frames. Only two association-error classes
extend malformed accounting: all 545 prior class dictionaries remain exact,
giving 547 classes. Later count/integrity updates are separately recorded
accounting metadata.

#### Capacity-growth-only allocation contract

`avif_retained_temporal_sample_reservation_failure` is a separate target-only
case using ordinary `animated_motion_multitile_split_groups`. Its
`av1.frame.retained_temporal_sample_reservation` hook consumes only when a
partial group's nonempty sample count exceeds spare vector capacity. Empty
or already-fitting groups cannot consume it. `usize::MAX` additional
nonzero-sized samples causes an actual deterministic `try_reserve` capacity
failure without host OOM. Normal builds retain the actual sample request,
existing error closure and ordering before pending-state publication.

The generic shared runner requires AVIF Dimensions at SequenceDecode and
`decode sequence: AVIF sequence validation failed: unable to reserve retained AV1 temporal-MV samples`.
Existing thread-local scoping consumes once and restores state on return or
unwind. A successful retry must equal a fresh uninjected Rust sequence decode;
its ordinary source independently checks exact Pillow frames. Injection/retry
has `oracle_status: not_applicable`, not Pillow allocation attribution. The
new hook is `defensive_model`; current origins total 140 guards across 20 files.

#### Identity and claim limits

Clean pinned libavif commit is
`6543b22b5bc706c53f038a16fe515f921556d9b3`; `read.c` SHA-256 is
`d2e4062f7030ffa5843a3c9cc4984e382a07782ce2241d8b5354fda888266e85`.
The Pillow handshake checks wheel/runtime pins. Public-probe receipts retain
actual source/input hashes and locked dependency identities. Independent
canonical/source audits are retained under
`target/release-evidence/avif-multitile-current-78fc8c8c-20261005/`; generator,
native and before/after proofs are under the companion
`next-public-gaps-78fc8c8c-20261005/` and
`avif-multitile-inter-candidate-78fc8c8c-20261005/` directories.

The existing native parity test now pins 267 artifacts and all 88 names,
including the six new matrix source mappings. Its four existing test functions
and preceding helper/assertion bodies are preserved. Fresh ignored native
collections have their own build identity and ordering; publishing additions
preserves the accumulated canonical index's record order and provenance.

#### Fresh incremental campaign and failed-run preservation

The first 15 selected runs pass on their recorded source. The subsequent full
run fails only the old 235-artifact assertion in the existing native parity
test. Its failed terminal receipt and log remain preserved; no successful full
report is attributed to that run. The existing test's six new witness names
and source mappings are added with the 267-artifact assertion. Source changed,
so a fresh baseline and all 14 additions are remeasured in the separate `v2`
campaign; reports from the two source inventories are not combined.

The fresh baseline executes 509 preceding ordinary AVIF rows plus 24 preceding
faults. Thirteen ordinary selections and one target-only fault then run
separately. All 15 selections pass. Each case resets only profiles with
`cargo +nightly-2026-07-16 llvm-cov clean --profraw-only`; the normal combined
LCOV invocation uses `--no-clean`, followed by a separate JSON report. The
receipt requires fresh nonempty profiles and keeps all 387 compiled-file hashes
identical between selections. Profile absence cannot reuse stale profdata.
The complete campaign takes 106.450857 seconds including its first build;
subsequent selected Cargo calls take 0.809–0.866 seconds on this host. These
are workflow timings, with no codec throughput claim.

The independent LCOV union and all 14 Coverage MCP incremental comparisons
agree: 45,247→45,290 observed lines out of the fixed 131,102 selected-report
coordinate denominator. Marginal gains in campaign order are
16, 3, 0, 0, 10, 1, 6, 1, 1, 0, 0, 0, 0 and 5 lines. The two Q100 sources and
four data-reference inputs remain regression witnesses despite zero marginal
lines after earlier selections. This selected denominator remains distinct
from the full report's aggregate line total.

The retained allocation-error closure at `frame.rs:1598–1602` has three LLVM
regions, each with baseline count zero, selected fault count one and full-suite
count one. The fault's structured error and retry assertions pass separately
from its ordinary source's exact Pillow parity. Selected-report absences are
not regressions. MCP retains limited coordinate evidence with source unverified
and tests unknown. The 100% and fastest-codec goals remain open.

The `avif-multitile-container-v2-*` local receipts, immutable full source receipt,
independent coverage audit and `coverage-mcp-multitile-v2-*` summaries retain
these results. The current malformed and claim-integrity accounting is updated
before the fresh campaigns, preserving all historical managed measurement IDs,
counts, inputs and preceding report observations.

### Preceding full AVIF location and temporal coverage — 2026-10-05

The preceding full local report is
`target/release-evidence/coverage-avif-iloc-temporal-20261005.json`, generated by
`RUSTC_WRAPPER= CARGO_BUILD_JOBS=2 COVERAGE_REPORT=target/release-evidence/coverage-avif-iloc-temporal-20261005.json make coverage`
with `nightly-2026-07-16`. It records 88,946/135,595 lines (65.596814%),
17,313/31,190 branches (55.508176%), 4,982/8,681 functions (57.389702%), and
136,103/210,590 regions (64.629375%). All alpha floors pass. The strict 100%
verifier exits with status 1 with all four requirements still unmet; that goal remains
open. The report SHA-256 is
`1de66283960b30d4afbf90213e54a0a111663a48a5d59ca37a341fe7a9de0f7f`.

All seven test binaries pass: 3, 4, 1, 57, 7, 1 and 68 tests, with no failures
or ignored tests. The matrix harness covers 509 ordinary AVIF decode rows,
24 separate target-only fault contracts and the native repetition bundle's
82 cases. The ordinary decode/encode inventory contains 2,021 rows: 1,558
active decode rows, 431 active encode rows and 32 planned encoder rows.
All 505 preceding AVIF observations, other-format observations and 23 preceding
fault rows remain unchanged.

The full local receipt records unchanged hashes for 4,957 local files:
4,956 tracked source, configuration, script and fixture files,
plus one ignored ambient `tests/fixtures/.DS_Store` file. The expanded inventory
includes 109 existing committed scripts and five new fixture files; it does
not mean that 114 source files were created. The full receipt separately records
the generated `malformed_ledger.json` and its `claim_ledger.json` integrity
refresh after the incremental campaign. All executable source, input/reference
and matrix hashes remain identical between those campaigns.

Relative to the preceding full report, covered counts increase by 36 lines,
eight branch outcomes, four functions and 42 regions. Source totals increase
by 30 lines, eight branches, two functions and 40 regions. Those changes span
the generic inspection/decode staging fix and the new coverage hook; they are
descriptive counts for different source. They remain separate from the 14 new
line observations isolated by the same-source incremental campaign below.
No source exclusions are used.

`make fmt`, `make lint` and `make lint-x86-simd` pass all eight recorded
formatting/strict-lint gates with identical hashes for 153 Rust/configuration
files. The SSE2 and AVX2 gates are cross-target compile/lint checks. All 33
feature lanes pass with all 4,855 recorded local hashes unchanged, including
4,698 codec fixture/observation files and the separate ambient Finder file.
Native and WASI lanes execute runtime checks; browser WASM lanes compile and
run strict Clippy/rustdoc checks. Capability artifacts remain unchanged.
The two accounting-file updates made after these campaigns are recorded
separately. The exact `cfg(coverage)` inventory is 139 guards across 20 files.
No new private unit tests, unsafe code, ignored cases or lint exceptions are
introduced.

Coverage MCP reads the full report with source unverified and test attribution
unknown. It groups the 74,487 repository-wide missing regions into 6,648
missing-region groups; these are not counts of wholly uncovered functions.
There are also 46,649 missing lines, 13,877 missing branch outcomes and 3,699
missing functions. The largest displayed groups remain repeated AV1
inter-translation instantiations. An AVIF display filter retains the global
totals. Local command/hash receipts do not create a managed MCP source/build
binding or named-test attribution.

### Complete AVIF location cases and error staging — 2026-10-05

Four ordinary cases extend the existing indexed-idat input. All are complete
files with valid enclosing BMFF box sizes. The maintained idat generator and
normal AVIF asset hook construct their inputs; pinned live Pillow 12.2.0
produces the operation observations and raw reference. Its runtime is libavif
1.4.1, dav1d 1.5.3 and libaom 3.13.2. Repeated generation is byte-identical.

| Ordinary case | Bytes | Complete input SHA-256 | Live Pillow outcome |
| --- | ---: | --- | --- |
| `iloc_idat_split_extents` | 5,918 | `b92d22f904a48d380a99f7b91ff78aa73ab4fc73e9d2f8b71697d843881aa1bd` | Open, inspect, verify, decode and sequence decode succeed with the original pixels and frame metadata. |
| `error_iloc_truncated_extent_index` | 5,896 | `62f5d6dd3b3cfb368030407977a2af434659c178c1412844781e43c03e25d8ed` | Open fails with `PIL.UnidentifiedImageError`; inspect, verify, decode and sequence decode report malformed input. |
| `error_iloc_index_size_three` | 5,906 | `53be83151f9031fbc56c0f3bc91fd50f5c2b7b3376ac760fb662b458eadc572b` | Open fails with `PIL.UnidentifiedImageError`; inspect, verify, decode and sequence decode report malformed input. |
| `error_iloc_idat_extent_out_of_bounds` | 5,906 | `1e0f61d11eb4cf291f4af89d3636ff874f517358e2eecb20d8b92d1a4a05e46b` | Open, inspect and verify succeed; decode and sequence decode fail with `builtins.SyntaxError`. |

Format detection succeeds for all four. The two open failures retain the
normalized Pillow message `cannot identify image file <bytes>`. The overrun
retains the exact decode message
`Failed to decode frame 0: BMFF parsing failed`. Its successful header is RGB,
128×128, one frame, so an inspection failure would violate the public contract.

The split input retains the nonzero base offset of eight. Its two zero-index
extents use relative offsets eight and 1,405, with lengths 1,397 and 1,398;
they concatenate to the original 2,795 AV1 bytes. The truncated-index case ends
its bounded `iloc` after two of the four declared index bytes. The width case
declares an unsupported three-byte index. The overrun changes only the extent
length from 2,795 to 2,796: its relative end exceeds the 2,811-byte idat payload
by one while remaining inside the outer file. Adjacent bytes cannot satisfy
that item's source bound. All `ftyp`, original unreferenced `mdat`, non-`iloc`
metadata children and idat/AV1 bytes remain unchanged. The AV1 payload SHA-256
is `be0bf650b8612533577e4b47a60989bb3f092cfb256b81f3bd84e3b5ce8ea199`.
The split row compares all 49,152 decoded bytes with the independent reference.

The generic Rust inspection path now validates container/configuration metadata
while retaining declared idat spans with checked arithmetic. Decode extraction
checks every item's file/idat bound before using encoded payloads. Grid-descriptor and
retained opaque-metadata reads remain bounded to the selected source; opaque
extents are checked before reserving their declared size. File-backed items
and sequence sample-table spans retain their existing bounds checks. This
preserves Pillow's lazy item-payload failure stage without permitting a read
outside idat or adding a case-specific comparator.

Independent bit-depth enrichment now uses `inspect_color_configurations` to
read bounded item properties or track sample descriptions without reading item
extents. The full sample inspector remains strict and rejects the overrun with
`item extent exceeds source bytes`. All 29 preceding inspector function bodies
and all 470 successful preceding inspection-depth values are unchanged.
`generate_decode_refs.py` catches only public Pillow open failures when
recording inspection errors; an independent enrichment failure now aborts
generation instead of being mislabeled as a Pillow outcome.

During ordinary input/reference generation, 4,690 of 4,694 preceding fixture
files stay byte-identical. The ordinary generator changes only the three AVIF
aggregate JSON/index files; the concurrently changed origin registry belongs
to the separate fault work. Four complete inputs and one raw reference are
added, with no prior files deleted. All 505 preceding AVIF row dictionaries,
other-format matrix rows and 23 preceding fault rows are unchanged. Later
count/integrity updates are accounting metadata, separately recorded.

### Nonempty temporal-sample allocation contract — 2026-10-05

The target-only row `avif_tile_group_temporal_sample_reservation_failure`
reuses the ordinary `animated_motion_chroma` input. Its coverage-only point,
`av1.frame.tile_group_temporal_sample_reservation`, is consumed only when a
reconstructed tile has nonempty temporal-MV samples. Empty intra-frame work
cannot consume it. A real `try_reserve(usize::MAX)` fails deterministically;
the production request remains the actual sample count, with the same existing
Dimensions error closure.

The shared public runner requires AVIF SequenceDecode and the exact message
`decode sequence: AVIF sequence validation failed: unable to allocate AV1 tile-group temporal-MV samples`.
It then checks a successful uninjected retry against a fresh Rust sequence
decode, including all returned content. The ordinary source row separately
supplies independent Pillow sequence/pixel parity. One-shot fault state is
cleared before the retry. This row remains in the shared matrix's
`oracle_status: not_applicable` lane; all 24 fault contracts are separate from
the 2,021 ordinary rows. Ordinary image bytes cannot deterministically force
this allocation failure.

### Same-source incremental observations and accounting — 2026-10-05

The six selected runs first execute all 505 preceding AVIF rows plus 23
preceding fault contracts, then each of the four new ordinary rows and the new
fault row separately. All selections pass. All 4,957 recorded local hashes
remain unchanged throughout execution and the subsequent MCP comparisons.
The independently checked LCOV coordinate union is
45,228→45,230→45,233→45,234→45,237→45,242 observed lines out of 131,094:
gains of two, three, one, three and five lines, in the table order above followed
by the temporal-sample fault. No coordinates are added or removed. This fixed
selected-report denominator differs from the full report's 135,595 lines.

The truncated-index selection observes the error-return region at
`samples.rs:1122` in both inspection/decode instantiations: preceding counts
zero/zero become four/two. The temporal-sample fault observes the existing
allocation-error closure at `frame.rs:2941–2943` once, from zero. Those exact
regions are also observed in the full report. MCP's raw LCOV/LLVM results retain
source unverified and tests unknown. These coordinate observations do not
establish full-suite regressions, managed source binding or named-test
attribution.

The preliminary `make verify` catches the stale generated malformed ledger.
After all incremental tests and MCP comparisons finish, the maintained
generator adds exactly the three ordinary AVIF error classes, increasing the
ledger from 542 to 545 classes while preserving all preceding class values.
Only that ledger and its current claim-ledger integrity digest change between
the incremental and full receipts. Rust runtime tests do not read the malformed
ledger; executable source, inputs, references and the shared matrix stay
unchanged. Historical managed run/snapshot identities, metrics and measured
inputs are preserved. The later verification passes. Receipts are retained in
`target/release-evidence/avif-iloc-edge-cases-d9efb879-20261005/`, the
`avif-iloc-temporal-*` campaign paths and the corresponding `coverage-mcp-iloc-*`
summaries.

### Rejected aligned JPEG 4:2:2 conversion fusion — 2026-10-05

An isolated experiment based on `d9efb879` fused RGB conversion with h2v1
chroma downsampling for no-token Cs422 inputs whose width is divisible by 16.
It removed two full-size chroma allocations while preserving saturation before
averaging. Both frozen variants passed 205 JPEG decode and 81 JPEG encode public
matrix rows, strict Clippy and all 20 exact JPEG benchmark-encoded-byte comparisons.

The append-based variant took 1.0191× the baseline's time on the affected RGB 128
Q85 Cs422 encode workload. A direct-store refinement measured 0.9958× and 0.9927×
in opposite-start five-round campaigns, while its oracle-normalized direction
crossed (1.0029× / 0.9974×). Normal RGB 512 Q10 Cs420 encodes measured 1.0148× / 1.0089×
and Q85 Cs420 encodes 1.0039× / 1.0139×, with higher normalized medians in both runs.
Every complete 40-workload campaign and raw sample remains in the ignored
`target/release-evidence/jpeg-next-performance-d9efb879-20261005/` evidence.

Both variants were rejected and reverted. No JPEG optimization or speed claim
was retained; these local ARM64 results do not establish x86 performance.

## Preceding indexed-idat, color CDEF and H4 full campaign — 2026-10-05

The preceding full local report is
`target/release-evidence/coverage-avif-indexed-idat-cdef-20261005.json`, generated by
`RUSTC_WRAPPER= CARGO_BUILD_JOBS=2 COVERAGE_REPORT=target/release-evidence/coverage-avif-indexed-idat-cdef-20261005.json make coverage`
with `nightly-2026-07-16`. It records 88,910/135,565 lines (65.584775%),
17,305/31,182 branches (55.496761%), 4,978/8,679 functions (57.356838%), and
136,061/210,550 regions (64.621705%). Every alpha floor passes. The strict
verifier still fails all four 100% requirements; that goal remains open.

All seven test binaries pass, including 57 coverage-matrix harness tests,
505 AVIF decode rows, 23 separate target-only fault contracts, the native
repetition bundle's 82 cases, seven decode-policy tests and 68 feature-gate
tests. The ordinary matrix contains 2,017 rows. All preceding Pillow case
observations and 21 preceding fault rows remain unchanged. The local full
receipt records unchanged hashes for 4,843 local files, including 4,842 tracked
source, configuration and fixture files plus one ignored Finder metadata file.
The report SHA-256 is
`6a4d058c0d3a2c6dad13eea36f71da69bfcce4bcf69b9151f2b54c9bbb95db0e`.

Relative to the preceding full report, covered counts increase by 18 lines,
six branch outcomes, two functions and 24 regions. Source totals decrease by
362 lines, 14 branches, 13 functions and 470 regions across the H4 cleanup and
new coverage hooks. These source-total changes remain separate from the
eight additional line observations isolated by the same-source incremental
campaign below. No source exclusions are used.

`make fmt`, `make lint`, `make lint-x86-simd`, `make verify` and all 33 native,
WASI and WASM feature lanes pass. Strict lint retains identical hashes for
153 Rust/configuration files; the feature campaign retains all 1,334 input
hashes without capability-artifact drift. The two new defensive fault points
increase the exact `cfg(coverage)` inventory from 136 to 138 guards across
20 Rust files. Existing guard origins are retained. No new unit tests,
unsafe code, ignored cases or lint exceptions are introduced.

Coverage MCP reads the full report as source-unverified with test attribution
unknown. It retains 74,489 missing regions and 13,877 missing branch outcomes.
The largest groups remain repeated AV1 inter-translation instantiations; the
retained luma-only H4 fallback still has 24 unobserved branch outcomes.
Local command/hash receipts do not create managed MCP evidence. This
checkpoint makes no measured performance or fastest-codec claim.

### Indexed AVIF item, color CDEF faults and luma-only H4 — 2026-10-05

The ordinary parity row `iloc_idat_indexed_extent` adds a four-byte zero extent
index to the pinned method-one idat input. Its version-one `iloc` declares
`index_size = 4`; construction method one leaves that index unused. Base and
extent offsets remain eight bytes each, and all 2,795 AV1 bytes, idat payload,
`ftyp`, original `mdat` and non-`iloc` metadata children remain unchanged.
Bounded box reconstruction adds four bytes to `iloc` and `meta`. The complete
5,906-byte input has SHA-256
`d90e970b5b9570c5091df64871d58a8f500c72df294c456037fad90cc1d92aac`.

Live pinned Pillow 12.2.0 with libavif 1.4.1, dav1d 1.5.3 and libaom 3.13.2
accepts the indexed input with exactly the source image's pixels, metadata and
frame observations. The maintained generator now verifies both idat variants
before writing either and retains its original returned path. The normal AVIF
asset hook and independent reference generator use the same workflow. Both
repeated generation runs and `generate_decode_refs.py --format avif` pass.
During input/reference generation, 4,689 of 4,692 preceding fixture files
remain byte-identical; only the three aggregate AVIF JSON/index files change,
and the new complete input and 49,152-byte raw reference are added. Subsequent
current-count and integrity-hash updates affect only accounting metadata.
Historical coverage metrics, measured inputs and managed identities remain
unchanged.

Two new target-only rows,
`avif_assembled_cdef_region_map_reservation_failure` and
`avif_assembled_cdef_active_map_reservation_failure`, reuse the active
`multitile_color_split_groups` parity input. Ordinary image bytes cannot
reliably force either allocation failure. Each coverage-only point requests
`usize::MAX` capacity at one fallible vector reservation, takes the existing
structured AVIF still-decode Dimensions error, and then checks a successful
uninjected retry against a fresh Rust decode. The existing ordinary source row
separately supplies independent Pillow pixel parity. Error messages and
production reservation counts remain unchanged; restoring the normal aliases
reconstructs `frame.rs` byte-for-byte to the preceding production source.
These rows stay in the shared matrix's separate `oracle_status: not_applicable`
lane and do not increase Pillow parity totals.

The former `decode_following_vertical_without_chroma` has one caller: the
third child of the closed H4 compositor, always 16x4 with no Full-resolution
edges. Its locally owned state remains 4:2:0, while this child parses and
publishes Monochrome syntax/output because it owns only luma samples.
The new private `decode_following_horizontal_four_without_chroma` keeps sample
checks, geometry consumption, quantization/CDEF bookkeeping, error ordering,
palette rejection, the active H4 reconstruction and byte-identical output
contexts. Four disconnected private helpers and unused edge preparation are
removed after independent caller/reference review. All other surviving
function bodies are byte-identical apart from the sole compositor call.
`block.rs` shrinks by 422 lines, with final SHA-256
`0f18d590882c560b9770503795b3a0c3520dd03c1fbd554bf0ef7930767abb00`.
The shared reconstruction, complete partition decoder and checked H4 fallback
remain intact.

The incremental campaign separately runs all 504 preceding AVIF rows plus
21 preceding fault contracts, then the new parity row and each new fault row.
Every selection passes with no failures or ignored tests. All 4,843 recorded
local file hashes stay unchanged within the campaign. Coverage MCP's LCOV union
estimates 45,192→45,194→45,197→45,200 observed lines out of 131,064: gains of
two, three and three lines, respectively, with zero added or removed
coordinates. Successful extent-index reads at `container.rs:659` and
`samples.rs:1110` are observed; their error-return outcomes remain missing.
The color CDEF allocation error closures at `frame.rs:1993` and `:2010` are
observed in their selected reports. This subset denominator differs from the
full report; the union is limited coordinate evidence and does not establish
full-suite regressions or verified named-test attribution.

The preliminary verifier catches stale current provenance counts in the
roadmap/documentation. They are synchronized to 138 before the successful
final verification. The local full receipt explicitly records the later
claim-ledger integrity refresh; all Rust and test input/reference hashes
remain identical to the incremental campaign. Independent source, input,
registry and numerical reviews have no actionable findings. Proofs and
receipts are retained under
`target/release-evidence/avif-h4-no-chroma-specialization-0899215a/`,
`target/release-evidence/avif-idat-indexed-20261005/` and the corresponding
`avif-indexed-idat-cdef-*` campaign paths.

### Preceding AVIF idat and following-chroma H4 campaign — 2026-10-05

The preceding full local report is
`target/release-evidence/coverage-avif-h4-idat-20261005.json`, generated by
`RUSTC_WRAPPER= CARGO_BUILD_JOBS=2 COVERAGE_REPORT=target/release-evidence/coverage-avif-h4-idat-20261005.json make coverage`
with `nightly-2026-07-16`. It records 88,892/135,927 lines (65.396867%),
17,299/31,196 branches (55.452622%), 4,976/8,692 functions (57.248044%), and
136,037/211,020 regions (64.466401%). All alpha floors pass; the strict report
verifier still fails all four 100% requirements, so that goal remains open.

Every test binary passed, including all 57 coverage-matrix harness tests,
504 AVIF decode rows, the native repetition bundle's 82 cases, all 21
target-only fault contracts, seven decode-policy tests, all 68 feature-gate
tests, determinism, animation, and runtime suites. The ordinary decode/encode
inventory now contains 2,016 rows; previous Pillow observations are unchanged.
The local source receipt confirms unchanged Rust, Cargo and input hashes
throughout this full campaign.

Relative to the preceding full report, covered counts increase by 15 lines,
four branch outcomes and 26 regions, with the function numerator unchanged.
The same-source incremental comparison below isolates the new idat row's
line observations. Source totals decrease by 6,732 lines, 1,038 branches,
388 functions and 9,914 regions after the H4 specialization supported by
caller proof and removal of its disconnected code. These source-total changes are kept
separate from executed coverage; no source exclusions are used.

Coverage MCP reads the report as source-unverified with test attribution
unknown. It retains 74,983 missing regions and 13,897 missing branch outcomes.
The largest groups remain repeated AV1 inter-translation instantiations.
The retained following-H4 fallback still has uncovered outcomes; its
reachability and checked fallback remain intact.

`make fmt`, `make lint`, `make lint-x86-simd`, `make verify`, and all 33 native,
WASI, and WASM feature lanes pass. Lint and feature receipts confirm unchanged
source/input hashes. The origin inventory remains 136 exact `cfg(coverage)`
guards across 20 Rust files. No new Rust unit tests, unsafe code, source
exclusions, ignored cases or lint exceptions are introduced.

### AVIF idat item and following-H4 specialization — 2026-10-05

The ordinary parity row `iloc_idat_primary_item` relocates the pinned baseline's
primary AV1 item into a complete `meta/idat` box. Its `iloc` version-one entry
uses construction method one, data-reference index zero, and nonzero base and
extent offsets of eight bytes each. Both container inspection and sample
extraction must resolve the item relative to the `idat` payload. The sixteen
padding bytes precede the unchanged 2,795-byte AV1 payload; the original
`mdat`, `ftyp`, and non-`iloc` metadata children remain byte-identical. The
5,902-byte complete fixture has SHA-256
`cc020ae1e91a2cad8c4f66b10165702bf11abc39a462a06b9665664f3c63c337`.

Live pinned Pillow 12.2.0 with libavif 1.4.1, dav1d 1.5.3, and libaom 3.13.2
accepts the relocated item with the same complete image and frame observations
as the source. The maintained reference generator produces exact pixels and
operation contracts independently of Rust. Generate or verify the input with
`target/oracle-staging/pillow122/bin/python scripts/generate_avif_idat_fixture.py`;
the normal AVIF asset hook invokes it too. Repeated generation preserves the
same bytes and refuses to replace a differing fixture. The input/reference
generation step leaves all 4,687 preceding nonaggregate fixture files
byte-identical, with previous JSON case observations and ordinary matrix rows
preserved. Subsequent count and integrity-hash synchronization changes only
accounting metadata; historical measured inputs and managed coverage identities
remain unchanged. The AVIF decode inventory grows from 503 to 504; all 21
target-only fault rows retain their separate `oracle_status: not_applicable`
lane.

The H4 refactor uses the following caller invariants. Both calls to the former `decode_following_vertical` belong to one
locally owned `Lossy420Decoder::with_qindex` state, which fixes sampling to
4:2:0. Each receives 16x4 dimensions and no Full-resolution edges. Pending
geometry is consumed by each successful syntax attempt, so both following
leaves have parent `Horizontal16x4` geometry. Encoded TX8x4 or TX4x4 child splits
do not change that parent. The callback can access only the range decoder.
The private entry is now `decode_following_horizontal_four_chroma`.

Sample-depth and geometry checks, quantization/CDEF bookkeeping, syntax/error
sequencing, palette rejection before reconstruction, the active H4
reconstruction, and visible output/context updates are retained. The complete
partition decoder, normalized Full/high-depth path and H4 compositor remain.
The source proof disconnects 63 private helpers together with their unused
aliases, constants, neighbor type and twelve unread fields. Four already
disabled private tests used only removed helpers; they are retired. A mixed
edge test retains its live right-edge assertion. No public parity case is
removed. `block.rs` shrinks by 8,365 lines; the final source SHA-256 is
`c389592f0ba11d479ceb3ff0277b6148d7a3b94093ff7d0449828b0362f586c7`.
No coverage exclusions, unsafe code, ignored cases or new lint exceptions are
introduced.

Structured caller/reference proofs, preserved baseline/candidate source,
lint and feature receipts with source hashes, and the live input/reference receipts
are retained under `target/release-evidence/avif-h4-following-specialization-3e4ab357/`,
`target/release-evidence/avif-idat-primary-20261005/`, and the corresponding
`avif-h4-idat-*` campaign paths.

The incremental campaign runs all 503 preceding AVIF rows and the new row
separately on identical source; both selections pass. Its local receipt confirms
unchanged Rust, Cargo and input hashes. Coverage MCP's LCOV union estimates
43,755→43,770 observed lines out of 131,419, a fifteen-line gain
(0.011414 percentage points), with zero added or removed coordinates. The new
row observes 14,754 lines; 14,739 overlap the preceding subset. The previously
missing Idat selection at `container.rs:642` and relative-span calculation at
`samples.rs:1124` are observed in its selected LLVM report. This subset
denominator differs from the full campaign. MCP still reports source/build
identity unverified and test attribution unknown; the local command/hash
receipt does not create managed evidence or establish full-suite regressions.

### Preceding Full-chroma H4 cleanup campaign — 2026-10-05

The preceding full local report is
`target/release-evidence/coverage-avif-h16x4-cleanup-20261005.json`, generated by
`RUSTC_WRAPPER= CARGO_BUILD_JOBS=2 COVERAGE_REPORT=target/release-evidence/coverage-avif-h16x4-cleanup-20261005.json make coverage`
with `nightly-2026-07-16`. It records 88,877/142,659 lines (62.300311%),
17,295/32,234 branches (53.654526%), 4,976/9,080 functions (54.801762%), and
136,011/220,934 regions (61.561824%). All alpha floors pass; the strict report
verifier still fails all four 100% requirements, so that goal remains open.

Every test binary passed, including all 57 coverage-matrix harness tests,
503 AVIF decode rows, the native repetition bundle's 82 cases, all 21
target-only fault contracts, seven decode-policy tests, all 68 feature-gate
tests, determinism, animation, and runtime suites. The 2,015 ordinary
decode/encode rows and their Pillow references are unchanged.

The covered numerators are identical to the preceding full campaign. The
caller-proven dead-code cleanup removes 457 reported lines, 84 branches,
15 functions, and 713 regions from the source totals; no coverage exclusion
or synthetic case produces this change. Coverage MCP reads the report as
source-unverified with test attribution unknown. Its totals retain 84,923
missing regions and 14,939 missing branch outcomes. Uncovered paths in the
retained vertical fallback remain open.

`make fmt`, `make lint`, `make lint-x86-simd`, `make verify`, and all 33 native,
WASI, and WASM feature lanes pass. The origin inventory remains 136 exact
`cfg(coverage)` guards across 20 Rust files. No Rust unit tests, unsafe code,
source exclusions, or new lint exceptions are introduced by the cleanup.

### Preceding CDEF fault-contract campaign — 2026-10-05

The preceding full local report is
`target/release-evidence/coverage-cdef-region-map-20261005.json`, generated by
`RUSTC_WRAPPER= COVERAGE_REPORT=target/release-evidence/coverage-cdef-region-map-20261005.json make coverage`
with `nightly-2026-07-16`. It records 88,877/143,116 lines (62.101372%),
17,295/32,318 branches (53.515069%), 4,976/9,095 functions (54.711380%), and
136,011/221,647 regions (61.363790%). All alpha floors pass; the strict report
verifier still fails all four 100% requirements, so that goal remains open.

Every test binary passed, including all 57 coverage-matrix harness tests,
503 AVIF decode rows, the native repetition bundle's 82 cases, all 21
target-only fault contracts, seven decode-policy tests, all 68 feature-gate
tests, determinism, animation, and runtime suites. The 2,015 ordinary
decode/encode rows and their Pillow references are unchanged by this slice.
Fault results remain a separate `oracle_status: not_applicable` lane.

Coverage MCP reads this full report as source-unverified with test attribution
unknown. It reports 85,636 missing regions and 15,023 missing branch outcomes.
The monochrome CDEF region-map allocation error closure is now covered.
The same-source incremental line comparison below isolates the new case's
coordinate observations; full-report denominator changes are reported intact.

`make fmt`, `make lint`, `make lint-x86-simd`, `make verify`, and all 33 native,
WASI, and WASM feature lanes pass. The origin inventory records 136 exact
`cfg(coverage)` guards across 20 Rust files. No source exclusions, Rust unit
tests, unsafe code, or lint exceptions were added in this slice.

### Preceding AVIF edge campaign — 2026-10-05

The preceding full local report is
`target/release-evidence/coverage-avif-edges-20261005.json`, generated by
`RUSTC_WRAPPER= COVERAGE_REPORT=target/release-evidence/coverage-avif-edges-20261005.json make coverage`
with `nightly-2026-07-16`. It records 88,864/143,107 lines (62.096194%),
17,293/32,316 branches (53.512192%), 4,975/9,095 functions (54.700385%), and
136,002/221,642 regions (61.361114%). All alpha floors pass (59%, 46%, 52%,
and 58% respectively). The strict report verifier still fails all four 100%
requirements; that goal remains open.

Every test binary passed, including all 57 coverage-matrix harness tests,
503/503 AVIF decode rows, the native repetition bundle's 82 complete cases,
all twenty target-only fault contracts, seven decode-policy tests, determinism,
all 68 feature-gate tests, animation, and runtime suites. The matrix contains
2,015 decode/encode rows. The unchanged fault contracts remain a separate
lane with `oracle_status: not_applicable`, outside Pillow parity counts.
The initial full run caught the native repetition harness's stale 232-artifact
inventory; its exact inventory now includes the new sequence and all 235
artifact hashes, and the repeated full campaign passes.

Coverage MCP reads the final report as source-unverified with test attribution
unknown. It reports 85,640 missing regions and 15,023 missing branch outcomes.
The largest groups remain AV1 inter-translation and vertical decode
instantiations in `block.rs`. Repeated generic instantiations dominate; these
coordinates are leads for source review, not direct fixture prescriptions.
The final same-source incremental comparison is recorded below. Source and
harness edits changed full-report denominators, so differences from the
preceding full report are not an isolated execution gain.

`make lint`, `make lint-x86-simd`, `make fmt`, `make verify`, and all 33 native,
WASI, and WASM feature lanes pass. The origin inventory remains 135 exact
`cfg(coverage)` guards across 20 Rust files, with no source exclusions.
No new Rust unit tests, unsafe code, or lint exceptions are introduced.

### Preceding complete-source campaign — 2026-10-05

The preceding full local report is
`target/release-evidence/coverage-current-source-accounting-20261005.json`,
generated by
`RUSTC_WRAPPER= COVERAGE_REPORT=target/release-evidence/coverage-current-source-accounting-20261005.json make coverage`
with `nightly-2026-07-16`. It records 88,864/143,127 lines (62.087517%),
17,279/32,344 branches (53.422582%), 4,974/9,095 functions (54.689390%), and
136,005/221,676 regions (61.353056%). The project alpha floors pass (59% lines,
46% branches, 52% functions, 58% regions), while the separate 100% requirement
still fails and remains open.

All test binaries in the campaign passed, including all 57 coverage-matrix
harness tests, seven decode-policy tests, determinism, all 68 feature-gate
tests, animation, and runtime suites. The shared matrix contains 1,995
decode/encode rows and twenty target-only AVIF/JPEG fault contracts in a
separate lane with `oracle_status: not_applicable`; all twenty contracts
passed. The full AVIF decode matrix passed 483/483, including both lossy 4:2:0
four-Square8 parity cases, the skipped color/alpha CDEF case, and the 33×17
I422 super-resolution case's Pillow odd-width right-edge chroma behavior.

Coverage MCP reads the full LLVM report as source-unverified with test
attribution unknown because it carries no source/build receipts. Its
region and branch totals report 85,671 missing regions and 15,065 missing
branch observations. The largest groups are AV1 inter-translation and vertical
decode instantiations in `block.rs`; repeated generic instantiations dominate
the result, so the coordinates are leads for source review rather than a direct
case prescription. The targeted branch scan still reports missing outcomes in
`Lossy420Decoder::decode_following_vertical` (`block.rs:67556`); neither lossy
Square8 parity row adds observations there. The `frame.rs` branch scan also
reports 35 missing outcomes in `assemble_monochrome_tiles`, including
inconsistent or absent per-tile filter metadata paths. MCP does not attribute
observations to tests. The full report includes all twenty fault-contract
rows; source/build receipt-bound attribution remains unavailable.

The current coverage-origin inventory records 135 exact `cfg(coverage)`
guards across 20 Rust files. All source `coverage(off)` attributes have been
removed. Rust's strict Clippy policy, coverage-hook Clippy configuration, and
x86 SSE2/AVX2 lint targets pass without new lint exceptions. Target fault
injection remains compiled only for coverage builds.

### AVIF configuration precedence and film-grain edges — 2026-10-05

Twenty new complete-file manifest cases use live pinned Pillow 12.2.0,
libavif 1.4.1, dav1d 1.5.3, and libaom 3.13.2. Fourteen succeed completely,
five fail during opening or first-frame decoding, and one succeeds on frame
zero before rejecting an unlisted grain reference in frame one. The ordinary
public runner executes all twenty successfully after the fixes. These are
Pillow parity inputs; the twenty target-only fault contracts remain a
separate unchanged lane.

The configuration generator preserves BMFF lengths, item extents, and AV1
media bytes while changing redundant declarations or a later duplicate
property. Seven cases establish that decoded sequence fields take precedence
over differing profile, level, tier, monochrome, subsampling, and chroma
position declarations. Two depth disagreements between the selected `pixi`
and `av1C` properties require opening errors. Two later duplicate `pixi`
properties retain the first property's precedence. Two matching ten/twelve-bit
container declarations retain the actual eight-bit sequence pixels, and one
accepted twelve-bit declaration has `highBitdepth` clear. Declared depth
metadata remains independently identified; decoded storage and conversion
use the validated AV1 sequence depth.

Before fixes, the original nine configuration cases failed, both duplicate
property cases failed, both declared-versus-sequence depth cases failed, and
the tolerated twelve-bit flag case failed. Rust now validates the selected
first `pixi` against the cached `av1C` depth and follows libavif's twelve-bit
flag precedence. The redundant configuration/sequence equality check is
removed. Bounded container framing, actual AV1 syntax, repeated sequence
consistency, sample bounds, and checked plane/conversion validation remain.
No additional pixel allocation or repeated configuration payload parse is
introduced.

The grain generator adds exact I422 pixels, Y-point-count 15 and UV-count 11
rejections, unequal I420 U/V presence, an unlisted reference rejection, and
valid reuse of an inactive reference. The last case reproduced a sequence
decode failure before the fix. A listed, existing reference with inactive
grain now inherits inactivity; empty or unlisted references still fail.
`Option<FilmGrain>::None` retains its documented meaning of `apply_grain=false`.
The independent header inspector follows the same native semantics. The
refreshed native repetition bundle contains 82 cases; all 81 preceding case
records are unchanged, with one new inactive-reference sequence observation.

Reproduce inputs with
`target/oracle-staging/pillow122/bin/python scripts/generate_avif_config_disagreement_fixtures.py`
and
`target/oracle-staging/pillow122/bin/python scripts/generate_avif_filmgrain_edge_fixtures.py`.
The existing asset-generation hook invokes both. Exact outputs and operation
contracts are generated through `scripts/generate_decode_refs.py --format avif`;
no expected pixels or failures are copied from Rust.

The incremental campaign executes the preceding 483 AVIF rows and the twenty
new rows in separate runs on identical final source, features, and LLVM flags.
Both subsets pass and their local receipt confirms unchanged Rust inputs and
matrix hashes throughout. Coverage MCP's LCOV coordinate union reports
43,734→43,749 observed lines out of 138,301: fifteen additional lines, with no
source coordinates added or removed. The twenty-row subset observes 22,283
lines; 22,268 were already observed by the old AVIF subset. The I422 grain
helper has seven missing regions in the baseline and none in the new subset.
This subset denominator differs from the full all-test campaign.

Coverage MCP labels both raw reports source-unverified and test attribution
unknown. The local receipt supports this same-source comparison but does not
turn it into managed MCP evidence or a full-suite regression result. JSON
branch/region coordinates are useful for review; no numeric incremental branch
or region union is claimed. Reports are `avif-edges-baseline-20261005.lcov` and
`avif-edges-selected-20261005.lcov`, with companion JSON reports,
`avif-edges-receipt-20261005.json`, and
`coverage-mcp-avif-edges-final-20261005.json`, under `target/release-evidence/`.

Local red/green logs: `config-disagreement-red-20261005.log`,
`avif-15-red-20261005.log`, `duplicate-pixi-red-20261005.log`,
`sequence-depth-red-20261005.log`, `depth-flags-red-20261005.log`, and
`avif-20-green-20261005.log`, all under `target/release-evidence/`.

### Rejected JPEG sparse chroma masks — 2026-10-05

A separate worktree at `d5ddebbe83d067be16f49eb13c4f29603faa55d5`
implemented fused chroma nonzero masks and a separate sparse Cb/Cr entropy
loop for complete I420 MCUs without low-quality trimming. The prototype is
rejected and reverted; the main encoder remains unchanged. Its exact source
and patch are preserved locally for review of the rejected prototype.

The ARM64 macOS 15.7.7 campaign used rustc 1.96.1, ordinary Cargo release
builds, and the pinned TurboJPEG 3.2.0 oracle. No LTO, target-cpu, PGO, or
TurboJPEG SIMD disabling was added. Single-threaded complete public calls use
the existing twenty configurations, forty encode/decode workloads, and Rust
harness's 100-call warmup; per-invocation iteration counts and timings are
retained. Five rounds alternate baseline/candidate order, with the unchanged
oracle following each variant. All 800 raw records retain commands, timing,
output hashes, order, and background-load observations. Source, binary, and
oracle hashes remained unchanged. CPU brand/model queries were unavailable;
recorded background load was nonzero, so this is a scoped host observation.

All seven admitted encode configurations regress in every paired round.
The table gives medians of paired candidate/baseline time ratios; values above
one are slower. Oracle normalization retains the same regression decision.

| Encode workload | Candidate / baseline | Oracle-normalized ratio |
| --- | ---: | ---: |
| 32×32 Q85 I420 | 1.1585 | 1.1587 |
| 128×128 Q50 I420 | 1.0494 | 1.0413 |
| 128×128 Q85 I420 | 1.1222 | 1.1243 |
| 128×128 Q100 I420 | 1.2138 | 1.1838 |
| 512×512 Q85 I420 | 1.2125 | 1.2083 |
| 1024×1024 Q85 I420 | 1.2366 | 1.2138 |
| 128×128 Q85 I420, restart interval 4 | 1.1085 | 1.1325 |

Controls varied: odd 63×65 encoding was 0.9805, I444 encoding 1.0008,
512×512 Q10 encoding 1.0927, and 1024×1024 decoding 0.9928. These controls
are retained alongside the regressions. Mask-construction overhead and lost
six-chain instruction overlap are plausible causes, without stage-level
proof. No encoder, decoder, SSE2, AVX2, or universal speedup is claimed.

Correctness still passed: emitted JPEG bytes matched the baseline for all
twenty configurations; the existing public manifest passed 81/81 encode rows
(379 calls, zero panics) and 205/205 decode rows; strict all-feature/all-target
Clippy passed.

The preserved candidate source SHA-256 is
`283be8d25af57b67396f3a08742586532c539593cbb5095cd6b0dcc78e912321`;
patch SHA-256 is
`b541f59801c0a84d7ae4510ab292943a28779fc3046661896b9543a247558556`.
Local evidence is under `target/release-evidence/jpeg-sparse-chroma-d5ddebbe/`:
`rejection.json`, `candidate-source.rs`, `candidate.patch`, build/check logs,
and `paired-five-rounds/{metadata.json,raw.jsonl,summary.json,summary.csv,exact-encode-equality.json}`.
These local artifacts are not shipped release benchmark results. The next
candidate must preserve instruction overlap or identify a different measured
cost, then pass the same correctness and complete-call measurement gates.

### Rejected JPEG bounded entropy arithmetic — 2026-10-05

An isolated candidate based on `e070f9cef79005c5b832ebb78db0fee3eb192cf6`
replaced three saturating operations with locally bounded arithmetic: available
reservoir space, the post-capacity-check word count, and the AC zero run.
The source-bound ARM64 object showed saturation selects for these expressions,
but did not quantify their share of elapsed time. Six-chain scheduling,
quantization, storage, and output contracts were unchanged.

Two whole-call campaigns each completed forty workloads and five balanced
rounds, with the second reversing the starting order: 1,600 raw records in
total. Both retained exact equality for all twenty encoded benchmark JPEGs.
Strict Clippy and the public Pillow manifest passed 81/81 encode rows and
205/205 decode rows. The ordinary release builds used Rust 1.96.1 on macOS
15.7.7 ARM64 and the pinned TurboJPEG 3.2.0 timing oracle, with single-threaded
operation loops. CPU brand/model and memory queries were unavailable; nonzero
background load is recorded in each campaign. Inputs, source,
executables, and oracle hashes stayed unchanged during each campaign.

Each table value is the median of five paired round ratios. Candidate/baseline
ratios below one mean lower elapsed time. Normalized ratios also divide each
target measurement by its following oracle measurement.

| RGB I420 encode workload | First ratio | Reversed ratio | Normalized first / reversed |
| --- | ---: | ---: | ---: |
| 63×65 Q85 | 1.0092 | 1.0122 | 1.0067 / 1.0095 |
| 128×128 Q100 | 1.0214 | 1.0135 | 1.0198 / 1.0337 |
| 512×512 Q85 | 1.0169 | 1.0048 | 1.0122 / 0.9981 |
| 1024×1024 Q85 | 1.0167 | 1.0131 | 0.9955 / 1.0129 |
| 512×512 Q10 | 0.9852 | 0.9479 | 0.9839 / 0.9511 |

Regular RGB inputs repeatedly regressed despite gains on some low-quality and
CMYK inputs. Decode controls also varied. The combined candidate is rejected;
these results do not isolate an individual expression's cost or establish an
SSE2/AVX2 result. The encoder was restored byte for byte, and the clean managed
worktree was archived after independent preservation checks. No candidate
source reached main.

The retained bundle `target/release-evidence/jpeg-bounded-entropy-e070f9ce/`
contains both executables, source and patch snapshots, harness files, inputs,
both complete campaigns, `comparison.json`, `comparison.csv`, `rejection.json`,
and `preservation.json`. Candidate source SHA-256:
`71942749a3545ed9323573abc8a60c9c9a59ca6a9234f8e992798ed3a375c36f`;
restored encoder SHA-256:
`5451cb6570679fe9ea13b385900e5270581220e35cf1993c09eeb0b1a799afab`.

### SSE2/AVX2 runtime evidence and JPEG profile — 2026-10-05

At pushed revision `d5ddebbe83d067be16f49eb13c4f29603faa55d5`,
[CI run 37263569622](https://github.com/appunni-m/image-slash-star/actions/runs/37263569622)
completed successfully. Both SSE2 and AVX2 public JPEG/AVIF encode/decode
parity lanes passed with branch coverage; aggregate coverage floors, strict
lint, feature/target checks, and dependency checks also passed. The x86
coverage artifact retains eight reports. Their numerical totals have not been
inspected locally, so this establishes runtime parity and passed floors.

[Benchmark run 37263569657](https://github.com/appunni-m/image-slash-star/actions/runs/37263569657)
also passed five rounds each of SSE2 and AVX2 on the same Ubuntu host, covering
20 configurations and 40 workloads. All 40 comparisons retained equal output
lengths and hashes. Downloading its timing artifact requires authenticated
access (HTTP 401), so no relative speed or speedup is claimed from this run.

A source-bound ARM64 profile at that revision sampled the complete 1024×1024
RGB Q85 I420 encode call for 12 seconds. Its receipt records source hashes,
binary SHA-256, build flags, and the owned process; the source remained
unchanged through sampling. Streaming encoding represented 84.56% of observed
top-of-stack samples, RGB conversion 9.11%, and external FDCT 6.20%. Exact
binary/object/DWARF matching maps representative streaming addresses to Cb AC
coefficient reads, zero-run handling, and the bit reservoir. Collapsed samples
prevent assigning a precise entropy fraction. Sampling is not a timing
benchmark.

The profile motivated collecting sparse chroma AC masks during quantization
and enumerating their zigzag positions. The experiment above rejects that
prototype after complete-call measurement. Any future implementation must
preserve ZRL/EOB, DC prediction, byte stuffing, and restart semantics, then
pass exact public parity and balanced complete-call measurements. Dense-image
mask overhead is part of the measured rejection above; no optimization is
retained from this profile. Halving the measured conversion bucket would permit approximately
1.048× overall speedup, while the sparse-scan candidate's affected fraction is
unknown.

Local receipts: `target/release-evidence/jpeg-profile-d5ddebbe/receipt.json`,
`analysis.md`, `analysis-offset-mapping.json`, and `analysis-ci-terminal.json`;
remote benchmark metadata: `target/release-evidence/simd-remote-d5ddebbe/`.

### Unreachable AVIF Full-chroma Horizontal16x4 cleanup — 2026-10-05

Both current callers of `Lossy420Decoder::decode_following_vertical` belong to
the H4 composer. Each receives a local `with_qindex` state initialized to
`Subsampled420`, dimensions `(16, 4)`, and absent full-edge state. Intervening
methods do not change sampling; the callback receives only the range decoder.
The sole Full-sampling setter, `decode_origin_full`, is outside this local path.
The Full-chroma `Horizontal16x4` predicate therefore cannot succeed for any
current caller, independently of coverage observations.

The removed cluster consists of that reconstruction arm,
`reconstruct_following_lossy_full_vertical_16x4_leaf`,
`reconstruct_lossy_full_16x4_chroma_normalized_target`, and their exclusively
owned `right_edge_4_for_16x4` accessor. It removes 580 source lines and three
separator blanks. Exact comparison against
`f46efa4b3eee82b9c33e5cd3fe1911a7ad2d16f5` confirms only those four ranges were
deleted: every retained byte is unchanged. The live 4:2:0 arm, entire vertical
decoder, H4 composer, and their safety checks remain. This proof does not
justify deleting the complete fallback method.

The source proof is retained in
`target/release-evidence/avif-vertical-h16x4-cleanup-f46efa4b/proof.json`.
No cfg guard or source coverage exclusion changed.
The full all-feature campaign, strict debug/release/coverage and x86 SIMD
Clippy gates, formatting, static verification, and all 33 feature lanes pass.
Ordinary Pillow rows and target-only fault rows are unchanged. This source
cleanup is not a measured AVIF throughput improvement.

### Complete source accounting and unreachable Square8 cleanup — 2026-10-05

The preceding full report still contained 42 source `coverage(off)` attributes.
This cleanup removes all 42, including defensive production checks, and the
unused nightly coverage-attribute feature gates. `make verify` now rejects
source coverage exclusions in Rust files under `src` and `tests`, preventing
those attributes from silently shrinking later denominators.

Source review proves the older lossy four-Square8 fallback is unreachable:
public root contexts retain level zero or one; its exact frame class is absent
from `closed_class` admission, and the alternative vertical-pair admission
requires context level three. The complete frame decoder already handles the
two public Square8 inputs. The obsolete predicate, dispatch branch, and
composer are deleted, removing 208 lines and one coverage guard. Shared
vertical helpers and the H4 callers remain.

The subsequent full campaign passes all public parity and twenty target-only
fault-contract rows. `make lint`, `make lint-x86-simd`, and all 33 feature/target
lanes also pass. Coverage is rebaselined at the four totals above; source
accounting and deletion changed the denominators, so their difference from the
previous report is not an incremental execution gain. The 100% target remains
open. These edits introduce no codec optimization or new runtime speed claim.

The x86 CI benchmark lane also gains an explicit Linux POSIX feature macro
for its strict C11 timing harness. The native strict C build and a timing smoke
invocation pass on ARM64 macOS; Linux compilation and SSE2/AVX2 runtime results
remain pending the pushed workflow. The fixture encoder patch retains its
semantics after whitespace normalization; reverse application checks pass
against the pinned patched libavif tree.

Report: `target/release-evidence/coverage-current-source-accounting-20261005.json`.

### AVIF skipped CDEF color/alpha public parity — 2026-10-05

`decode:avif:multitile_skipped_cdef_color_alpha` is a complete 128×64 RGBA
input with two tile columns in each color and monochrome-alpha item. The
independent libaom default-CDF writer selects unpartitioned 64×64 DC blocks
with skipped transforms. Frame headers retain enabled CDEF syntax with zero
strengths, while skipped blocks omit the region index. Pinned Pillow 12.2.0
accepts the file and supplies exact pixels; the public matrix runner passes
all declared operations. No target fault injection or Rust implementation
change is involved.

The new one-case selection passes against the same 481-case AVIF baseline
used for the gradient below. Coverage MCP's incremental LCOV line comparison,
including the gradient as a previously accepted batch, observes 20 newly
covered coordinates and 11,454 overlapping coordinates. The estimated gain is
0.014490 percentage points over the 138,027-line selected-report denominator.
The gains include the inactive-block and absent-region-index skips in both
color and monochrome tile assembly (`frame.rs:2063,2099,2411,2462`).
Missing source/build receipts and test attribution limit this to coordinate
evidence, without a full-suite regression claim. This ordinary-input case
belongs to Pillow parity; the twenty typed target-only fault contracts remain
a separate outcome lane.

The subsequent full campaign passes all tests and records 18 additional
covered lines, 24 branches, zero functions, and 24 regions with unchanged
denominators relative to the preceding full report. MCP's full-report branch
comparison remains limited by unverified source/build identity; its `frame.rs`
filter observes seven newly covered outcomes, including all four targeted
assembly skips. The current report records both outcomes at each target:
color active-map 256/4,352, color region-index 68/4, monochrome active-map
256/1,536, and monochrome region-index 24/4. These are report counts, not
receipt-bound per-test attribution. `make fmt`, `make verify`, `make lint`,
and `make lint-x86-simd` also pass. The native fixture writer now compiles with
`-Wall -Wextra -Werror`; both generator profiles reproduce their pinned hashes.
The SSE2/AVX2 gates are cross-compilation checks on this ARM64 host, with no
new runtime speed measurement. The fixture additions change no codec hot path.

Reports:

- `target/release-evidence/coverage-avif-existing-minus-square8-gradient-20261005.info`
- `target/release-evidence/coverage-avif-square8-gradient-new-20261005.info`
- `target/release-evidence/coverage-avif-skipped-cdef-color-alpha-new-20261005.info`
- `target/release-evidence/coverage-current-skipped-cdef-20261005.json`

### AVIF lossy Square8 gradient public parity — 2026-10-05

`decode:avif:portable_lossy_420_square8_gradient` adds a 16×16 full-range
quality-99 RGB gradient with four terminal Square8 leaves and disabled
screen-content tools. The pinned libavif 1.4.1/libaom 3.13.2 generator produces
identical repeated encodes; pinned Pillow 12.2.0 supplies the exact RGB
reference. The public manifest runner passes all declared operations.

The focused LLVM report records three complete lossy-partition decodes,
twelve streamed intra leaf decodes, and nine following-leaf decodes. It records
zero hits in `decode_four_lossy_420_leaves` and
`Lossy420Decoder::decode_following_vertical`. The complete frame decoder handles
this input before the legacy fallback. Source review also finds that the
fallback's `closed_class` admission omits the lossy square-split predicate;
the public root context has level zero or one, while its vertical-pair
alternative requires level three. This case therefore protects the complete
frame's public pixels without providing a witness for the older square helper.

Both the 481-case existing AVIF selection and the new one-case selection pass.
Coverage MCP cannot union these LLVM JSON details with the full report because
the detailed observations do not reconcile with the reported totals. Matching
LCOV selections instead provide an unverified incremental line estimate:
zero newly covered lines, with all 12,177 new-selection line coordinates
already covered by the 138,027-line existing-selection inventory. The case is
retained for its distinct public gradient regression contract. Missing
source/build receipts and test attribution limit the comparison to coordinate
evidence; it makes no full-suite regression claim.

Reports:

- `target/release-evidence/coverage-avif-square8-gradient-new-20261005.json`
- `target/release-evidence/coverage-avif-existing-minus-square8-gradient-20261005.info`
- `target/release-evidence/coverage-avif-square8-gradient-new-20261005.info`

### AVIF monochrome CDEF region-map reservation fault contract — 2026-10-05

`fault-contract:avif:avif_monochrome_cdef_region_map_reservation_failure`
reuses the existing `decode:avif:multitile_monochrome_split_groups` complete
128×64 Pillow-success input, SHA-256
`57490ef5f80626a3218b96cae9f3e9601ff9db685fc608cd02ad39213638b24c`.
The ordinary input requests two CDEF region slots. It cannot deterministically
choose allocator failure, and Pillow cannot receive the target's injection.

The coverage-only point makes the existing `try_reserve_exact` request
`usize::MAX`, causing its real capacity failure before resizing the map.
The runner requires the public AVIF `StillDecode` dimensions error and exact
message `decode: AVIF AV1 validation failed: unable to allocate assembled monochrome CDEF map`,
then requires a successful retry equal to a fresh uninjected decode. The row
uses the existing `decode_error_then_retry_succeeds` contract and
structured diagnostics requirement, with `oracle_status: not_applicable`.
Production builds keep the original reservation count and error path.

The twenty existing contracts and the new one-case selection each pass.
Their local receipt confirms unchanged source and Cargo inputs between runs.
Coverage MCP's matching LCOV comparison observes five newly covered lines,
30,769→30,774/138,310, a 0.003615 percentage-point incremental estimate.
The selected LLVM region scan finds no remaining gap in the allocation error
closure. MCP still labels source/build identity unverified and test attribution
unknown; the coordinate union does not establish a full-suite coverage result.
The added hook brings the origin inventory to 136 exact `cfg(coverage)` guards
across 20 Rust files, without source exclusions.

Reports:

- `target/release-evidence/cdef-region-map-baseline-twenty-20261005.lcov`
- `target/release-evidence/cdef-region-map-selected-one-20261005.lcov`
- `target/release-evidence/cdef-region-map-selected-one-20261005.json`
- `target/release-evidence/cdef-region-map-incremental-receipt-20261005.json`
- `target/release-evidence/coverage-mcp-cdef-region-map-incremental-20261005.json`

### AVIF monochrome CDEF active-map reservation fault contract — 2026-10-05

`fault-contract:avif:avif_monochrome_cdef_active_map_reservation_failure`
reuses the active `decode:avif:multitile_monochrome_split_groups` Pillow
stimulus, which enables CDEF for a split monochrome frame. Its coverage-only
failpoint targets the later `cdef_active.try_reserve_exact` allocation, after
the CDEF region map and loop-filter metadata reservations. The public AVIF
`StillDecode` dimensions error is checked, then a fresh retry must match an
uninjected decode. The row uses `oracle_status: not_applicable`; the injected
failure remains outside Pillow parity counts.

The new fault case and the nineteen-case baseline each passed through
`test_fault_contract_matrix`. Coverage MCP's LCOV incremental line comparison
observed five newly covered lines in `frame.rs` and estimated a 0.003622
percentage-point gain over the 138,027-line selected-report denominator.
MCP marks the estimate limited because source/build receipts and test
attribution are missing. The branch comparison is incomparable because exact
branch detail is unavailable, so this is selected-run coordinate evidence,
not a full-suite coverage claim.

Reports:

- `target/release-evidence/coverage-fault-contracts-existing-nineteen-minus-cdef-20261005.info`
- `target/release-evidence/coverage-fault-contract-monochrome-cdef-active-map-new-20261005.info`
- `target/release-evidence/coverage-current-fault-contracts-20261005-cdef-active-map.json`

### AVIF grid and monochrome assembly fault contracts — 2026-10-05

Two target-only rows use existing active Pillow parity inputs as reproducible
stimulus. `fault-contract:avif:avif_grid_cell_reservation_failure` uses
`decode:avif:grid`, an 80×80 color grid with per-cell alpha. Grid assembly now
uses fallible `try_reserve` instead of `Vec::with_capacity`, preserving the
normal reservation size while returning the public AVIF `StillDecode`
dimensions error on allocation failure. Its coverage-only failpoint forces
that error, and the runner confirms a fresh retry matches an uninjected decode.

`fault-contract:avif:avif_monochrome_loop_filter_metadata_reservation_failure`
uses `decode:avif:multitile_monochrome_split_groups`. It forces the assembled
monochrome loop-filter metadata reservation to fail, checks the public
`StillDecode` dimensions error, and verifies retry output against a fresh
decode. Both rows use `oracle_status: not_applicable`; their injected outcomes
remain separate from Pillow parity counts.

The 17-case existing-fault baseline and the two-case new selection each passed
through `test_fault_contract_matrix`. Coverage MCP's LCOV incremental line
comparison observed 923 new line coordinates, a 0.668744 percentage-point
gain over the 138,020-line selected-report denominator; 15,044 coordinates
were already covered. MCP marks this estimate limited because the reports
have no source/build receipts or test attribution. Its branch comparison was
incomparable because exact branch detail was unavailable, so these figures
are selected-run coordinate evidence, not full-suite coverage claims.

Reports:

- `target/release-evidence/coverage-fault-contracts-seventeen-baseline-20261005.info`
- `target/release-evidence/coverage-fault-contracts-two-new-20261005.info`

### AV1 projected temporal-field reservation fault contract — 2026-10-05

The target-only row
`fault-contract:avif:avif_projected_temporal_field_reservation_failure` reuses
`decode:avif:animated_motion_chroma`, whose inter frames enable reference-frame
MV projection. Its `cfg(coverage)` failpoint forces
`ProjectedTemporalField::new`'s reservation to fail and asserts the exact AVIF
`SequenceDecode` dimensions diagnostic, then verifies a fresh retry matches an
uninjected sequence decode. The new contract passed 1/1; the existing sixteen
contracts passed 16/16 in the selected baseline. Its oracle is
`not_applicable`, so it remains outside Pillow parity totals.

Coverage MCP compared the selected LCOV reports in incremental line mode: four
newly observed coordinates (0.002898 percentage points), with 19,478 already
covered. In the sixteen-case baseline, the allocation-error mapping at
`motion.rs:2999` was missing; it is observed by the new contract. MCP marks the
estimate limited because source/build receipts and test attribution are
unavailable.

Reports:

- `target/release-evidence/coverage-fault-contracts-existing-sixteen-20261005.info`
- `target/release-evidence/coverage-fault-contract-projected-field-new-20261005.info`
- `target/release-evidence/coverage-full-active-goal-20261005-projected-field.json`

The public row
`decode:avif:coverage_lossy_420_square8_four_leaves_01` pins a lossy 4:2:0
16×16 AVIF with a four-Square8 AV1 partition trace and exact Pillow RGB output.
It passed 1/1, and its same-target baseline comparison against the other 480
active AVIF decode rows found zero newly covered lines. The frame's
`allow_screen_content_tools` flag fails the specialized closed-frame gate, so
this row is retained as a public parity regression only; it is not claimed as
coverage for `decode_four_lossy_420_leaves` or `decode_following_vertical`.
MCP's selected report remains source-unverified and test attribution unknown.

Reports:

- `target/release-evidence/coverage-avif-existing-480-20261005.info`
- `target/release-evidence/coverage-avif-lossy-square8-new-20261005.info`

### AV1 tile assembly and partition-node reservation fault contracts — 2026-10-05

Two target-only cases exercise deterministic AV1 allocation failures through
the shared fault-contract runner. The assembled loop-filter metadata contract
reuses `decode:avif:multitile_color_split_groups`; the partition-node contract
reuses `decode:avif:baseline`. Each case checks the public AVIF `StillDecode`
dimensions error for its named allocation failure, then verifies that a fresh
decode retry matches the clean result. Both use
`oracle_status: not_applicable` and remain excluded from Pillow parity counts.

The selected runner passed both new contracts (2/2); a same-source selected
baseline run passed the other fourteen contracts (14/14). Coverage MCP compared
their LCOV reports in incremental line mode: 7 newly observed line coordinates,
14,410 already covered, and a limited 0.005072 percentage-point estimate. MCP
marks the comparison limited because source/build receipts and test
attribution are unavailable. Its selected report has no remaining line gaps in
`PartitionWalker::push`, and no region gap at the assembled metadata error
mapping or branch gap at its injection check. The rest of
`assemble_color_tiles` retains unrelated uncovered regions.

Reports:

- `target/release-evidence/coverage-fault-contracts-existing-fourteen-20261005.info`
- `target/release-evidence/coverage-fault-contracts-new-two-20261005.info`
- `target/release-evidence/coverage-fault-contract-assembly-partition-20261005-confirmed.json`

### AV1 tile block metadata reservation fault contract — 2026-10-05

The target-only row
`fault-contract:avif:avif_tile_block_metadata_reservation_failure` reuses the
active `decode:avif:baseline` input. Its coverage-only failpoint forces the
per-tile decoded-block metadata reservation to fail and checks the public AVIF
`StillDecode` dimensions error, then verifies that an uninjected retry matches
a clean decode. The selected contract passed 1/1; all twelve earlier fault
contracts also passed in their selected batch, and the full run passed all
thirteen. Its oracle status is `not_applicable`, so it remains separate from
Pillow parity counts.

Coverage MCP's same-target LCOV incremental comparison starts from the
56-test coverage-matrix baseline without its fault-contract runner, then unions
the twelve existing fault contracts and this new row. It reports four newly
observed line coordinates (0.002899 percentage points) in `tile_state.rs`.
MCP marks the result limited because the reports have no source/build receipts
or test attribution. The full all-feature report has no remaining region gap
at the new reservation error path.

### AV1 chroma tile-cell reservation fault contract — 2026-10-05

The target-only row
`fault-contract:avif:avif_chroma_tile_cell_reservation_failure` reuses the
active `decode:avif:baseline` input. Its coverage-only failpoint runs after the
luma cell allocation succeeds and forces chroma-cell reservation to fail. The
contract checks the distinct public AVIF `StillDecode` dimensions diagnostic,
then verifies an uninjected retry matches a clean decode. The selected case
passed 1/1; its oracle status is `not_applicable` and it does not contribute to
Pillow parity counts.

The same-target LCOV incremental comparison adds this row after the thirteen
existing fault contracts and reports three newly observed line coordinates
(0.002174 percentage points) in `tile_state.rs`. MCP marks the comparison
limited because the local reports have no source/build receipts or test
attribution. The full report has no missing branch at the injection check and
no remaining region at the public error mapping.

### AV1 film-grain parity and restoration fault contracts — 2026-10-05

The public parity row `decode:avif:animated_filmgrain_reference_reuse_i444_64x64`
uses a two-frame full-resolution I444 sequence. The first key frame refreshes
film-grain parameters into reference slot 0; the second displayed inter frame
sets `update=false`, reuses slot 0, and supplies a new seed. Pinned Pillow
12.2.0/libavif 1.4.1 decodes both frames. The selected Rust parity run passed
1/1, including exact per-frame pixel references.

The public row
`decode:avif:portable_lossless_filmgrain_420_chroma_from_luma_64x64` adds a
lossless I420 key frame with `chroma_scaling_from_luma=true`, non-empty Y
scaling points, and empty U/V point arrays. The generator preserves the tile
payload and item extent, validates the rewritten syntax with the independent
OBU inspector, and pins the exact Pillow RGB output. Its selected Rust parity
run passed 1/1. Coverage MCP reports no remaining branch gap at the
`read_film_grain` chroma-scaling gate (`frame.rs:4277`) in the full AVIF matrix.

The public row
`decode:avif:portable_lossless_filmgrain_420_zero_y_points_64x64` exercises a
valid 8-bit I420 key frame whose film-grain syntax has `num_y_points == 0` and
CFL disabled. AV1 infers empty U/V point tables and omits luma AR coefficients
for this 4:2:0 syntax. The deterministic generator preserves the tile payload
and item extent, validates the rewritten OBU independently, and pins Pillow's
exact 12,288-byte RGB output
(`5d35ab50438b9f1536ef7de3cbca8ac3d01360efa9d77446dfe0d69786d2fb62`).
The selected public parity row passed 1/1. The decoder now accepts this legal
case, retains a zero luma-grain LUT for active chroma consumers, and skips Y
plane writes when no Y points are present. With no active chroma points, it
returns after geometry and parameter validation without allocating grain LUTs.

This case exposed an admission gap: lossless non-super-resolution inter
reconstruction rejected all film-grain frames before producing a color
surface. The decoder now admits only bounded full-resolution I444 grain for
that lossless profile. Grain remains display-only and retained reference
samples remain ungrained. The bounded geometry and depth checks match the
existing display materializer.

The target-only row
`fault-contract:avif:avif_sgr_intermediate_reservation_failure` injects a
deterministic reservation failure in SGR intermediate allocation while
decoding the active
`decode:avif:animated_lossy_inter_420_superres_sgr_8bit_160x56` input. It
asserts the public allocation error classification and retry behavior; its
oracle status is `not_applicable`. The selected new fault case passed 1/1.
The target-only row
`fault-contract:avif:avif_restoration_stripe_scratch_reservation_failure` uses
the active `decode:avif:high_bitdepth` sequence. Independent OBU inspection
confirms a 64×64, non-super-resolution, single-tile frame with restoration
enabled, which reaches striped restoration. The reservation hook forces the
scratch reservation to fail; ordinary allocation failure now reports the
typed `Dimensions` resource error rather than malformed input. The contract
asserts the exact `SequenceDecode` diagnostic and verifies retry against a
fresh sequence decode. Its oracle status is `not_applicable`, and the selected
fault case passed 1/1.

The target-only row
`fault-contract:avif:avif_tile_cell_reservation_failure` uses the active
`decode:avif:baseline` input. A `cfg(coverage)` failpoint forces the AV1 tile
cell reserve to fail; the contract checks the exact public `Dimensions`
diagnostic at `StillDecode`, then confirms a fresh retry matches a clean
decode. Its oracle status is `not_applicable`, and the selected contract
passed 1/1. The coverage-origin inventory now records 128 exact guards across
20 Rust files; the new reservation hook is classified as `defensive_model`.

The focused film-grain parity rows passed 3/3; all twelve target-only fault
contracts passed 12/12 in the full all-feature campaign. The complete AVIF
decode matrix passed 480/480, and the AVIF edit-list repetition witness passed
after its artifact index was updated to include the new verified loop sample.
The current full report is
`target/release-evidence/coverage-zero-y-filmgrain-tile-cell-20261005.json`.

Coverage MCP `scope: incremental`, metric `lines`, reports five newly observed
coordinates for the zero-Y parity row (0.003624 percentage points) and three
for the tile-cell fault contract (0.002174 percentage points). The comparisons
use the same-source selected reports
`target/release-evidence/coverage-baseline-zero-y-filmgrain-final-20261005.info`,
`target/release-evidence/coverage-filmgrain-zero-y-i420-final-20261005.info`,
`target/release-evidence/coverage-baseline-fault-contract-tile-cell-20261005.info`,
and
`target/release-evidence/coverage-fault-contract-tile-cell-final-20261005.info`.
MCP marks them source-unverified with unknown test attribution, so both gains
are limited coordinate estimates. Branch-detail unions are incomparable.
Invalid reference and point syntax remains outside the positive Pillow parity
lane; the twelve remaining `read_film_grain` branch observations need separate
input-reachability review. The current full report also leaves normal SGR
regions uncovered, including 36 in `sgr_intermediates` and 23 in
`restore_sgr_plane`; the striped restoration contract covers its allocation
failure outcome only. The earlier film-grain, SGR, and stripe reports are
preserved as historical measurements. Earlier reports:
`target/release-evidence/coverage-baseline-filmgrain-existing-20261005.json`,
`target/release-evidence/coverage-filmgrain-reference-reuse-20261005.json`,
`target/release-evidence/coverage-filmgrain-cfl-i420-20261005.json`,
`target/release-evidence/coverage-baseline-faults-existing-20261005.json`, and
`target/release-evidence/coverage-fault-contract-sgr-intermediate-20261005.json`.

A prior full all-feature report and its dated observations are retained below
for historical context.

A later `coverage_matrix_tests` integration-target run completed 56/56 tests on
the 1,981-row matrix. Its parity-target report is
`target/release-evidence/coverage-parity-matrix-final-20261004.json`: 85,100 /
142,526 lines (59.7084%), 16,699 / 32,240 branches (51.7959%), 4,652 / 8,990
functions (51.7464%), and 130,211 / 220,521 regions (59.0470%). This report is
separate from the broader all-feature report above.

### AVIF display-plane allocation fault contract — 2026-10-04

`fault-contract:avif:avif_display_plane_copy_allocation_failure` injects a
one-shot allocation failure immediately before AV1 display-plane copy
reservation while decoding the existing `decode:avif:baseline` input. It
asserts the public `Dimensions` / AVIF / still-decode error and a successful
retry. The case is indexed in the shared matrix with a not-applicable oracle;
it does not add to Pillow parity counts. The selected
all-feature coverage run passed 1/1 and wrote
`target/release-evidence/coverage-fault-contract-avif-20261004.json` (1,972 /
32,244 branches for that selected run). Coverage MCP reports no remaining
branch gap at `src/codecs/avif/av1/surface.rs:174`.

For Coverage MCP incremental comparison, a same-source baseline was captured
with `test_fault_contract_matrix` skipped; its remaining 56 integration tests
passed. The compare call is `incomparable`: LLVM detailed observations do not
reconcile with report totals, and these reports have no source/build receipts
or test attribution. No marginal coverage gain is claimed from that comparison.

`fault-contract:avif:avif_sequence_frames_reservation_failure` drives the
public sequence decoder with the active `decode:avif:animated_motion_chroma`
fixture (32×32, four frames). The coverage-only failpoint asks
`Vec::try_reserve` for an impossible capacity, which deterministically reaches
the existing reservation-error conversion without allocating; the public call
must return `Dimensions` at `SequenceDecode`, and retry must return all four
frames. The selected all-feature run passed both fault contracts (2/2 cases).
Against the same-source 56-test parity baseline at
`target/release-evidence/coverage-parity-baseline-postfmt-20261004.info`, the
selected combined fault report
`target/release-evidence/coverage-fault-contracts-postfmt-20261004.info` adds 23
line coordinates (0.016688 percentage points). Coverage MCP reports no
remaining branch outcomes at `src/codecs/avif/decode.rs:231` or `:241`, the
failpoint and reservation-error conversion, nor at
`src/codecs/avif/av1/surface.rs:174` for the existing still-image fault.
Branch-level incremental union is incomparable because LLVM detail does not
reconcile with aggregate totals. The reports have no source/build receipts or
test attribution, so the line result is a limited estimate rather than a
verified causal claim.

### AV1 split monochrome tile reservation fault contract — 2026-10-04

`fault-contract:avif:avif_monochrome_tile_reservation_failure` uses the active
`decode:avif:multitile_monochrome_split_groups` Pillow parity input. Its first
tile group exercises the existing incremental monochrome tile-state reserve.
The coverage-only fault requests an impossible vector capacity, which reaches
the existing `Dimensions` error conversion without allocating. The selected
target-only contract checks `StillDecode` error classification and that a
fresh retry returns the complete decoded image. The injected result remains
`oracle_status: not_applicable` and outside Pillow parity totals.
The focused five-case fault-contract lane passed 5/5 contracts, and its
instrumented JSON report is
`target/release-evidence/coverage-fault-contracts-five-20261004.json`.
Coverage MCP reports no remaining branch outcomes at the new failpoint on
`src/codecs/avif/av1/frame.rs:1577`. The report has no source/build receipts or
test attribution, so this is limited coverage-coordinate evidence.

For the new case alone, a same-source 56-test parity baseline skipped
`test_fault_contract_matrix`; those tests passed 56/56. Coverage MCP's
incremental comparison against
`target/release-evidence/coverage-parity-baseline-monochrome-fault-20261004.info`
estimates 21 newly observed line coordinates, or 0.015235 percentage points.
The selected report is
`target/release-evidence/coverage-fault-contract-monochrome-tile-20261004.info`.
It remains a limited estimate because source/build identity and test
attribution are unavailable.

### AV1 super-resolution and JPEG multi-scan allocation fault contracts — 2026-10-04

`fault-contract:avif:avif_superresolution_positions_reservation_failure` uses
the active `decode:avif:animated_lossy_inter_420_superres_sgr_8bit_160x56`
sequence. The inter frame reaches AV1 super-resolution after the first frame
has been decoded. A coverage-only one-shot fault requests `usize::MAX`
positions from the existing `try_reserve_exact` call, producing its normal
`Dimensions` / AVIF / `SequenceDecode` error without attempting a large
allocation. Retry must return the matrix canvas and frame count, and its full
result must equal a fresh uninjected decode.

`fault-contract:jpeg:jpeg_multiscan_coefficient_reservation_failure` uses the
active `decode:jpeg:baseline_444_multiscan_duplicate_component` input to enter
multi-scan coefficient retention. Its coverage-only fault forces the actual
`try_reserve_exact` capacity-overflow path; the public result must be
`Dimensions` / JPEG / `StillDecode`, and retry must equal a fresh uninjected
decode. Both cases use `oracle_status: not_applicable` and remain outside
Pillow parity totals.

The focused all-feature baseline passed 56/56 integration tests with the fault
runner skipped. The selected four-case fault lane then passed 4/4 contracts
(the two existing AVIF contracts plus these additions). Its LCOV reports are
`target/release-evidence/coverage-fault-parity-baseline-20261004.info` and
`target/release-evidence/coverage-fault-contracts-new-cases-20261004.info`.
Coverage MCP's incremental line comparison is limited because the reports
have no source/build receipts or test attribution: it estimates 29 newly
observed line coordinates, or 0.021040 percentage points. The path-filtered
observations include the JPEG coefficient reservation and AV1 position
reservation error paths. Branch comparison is incomparable because the
available detail does not reconcile with report totals; no branch gain or
regression claim is made.

### AV1 super-resolution output-plane reservation fault contract — 2026-10-04

`fault-contract:avif:avif_superresolution_plane_reservation_failure` reuses
the active 160×56 AV1 super-resolution sequence after the positions vector has
been built. A coverage-only fault requests `usize::MAX` output samples from
the existing plane `try_reserve_exact`; the public sequence result must be
`Dimensions` / AVIF / `SequenceDecode`, and retry must equal a fresh clean
decode. The fault is recorded with `oracle_status: not_applicable` and does
not contribute to Pillow parity counts.

The final-source baseline selected 472 existing AVIF decode rows and five
prior fault contracts; all 57 coverage-matrix harness tests passed. The
selected sixth-contract run also passed all 57 tests. Coverage MCP reports the
sample-reservation error region at `resize.rs:398` as missing in the baseline
and absent from the selected-run gap list. Its incremental comparison is
incomparable because LLVM detail does not reconcile with report totals. Reports
are
`target/release-evidence/coverage-avif-superres-faults-postfmt-baseline-20261004.json`
and
`target/release-evidence/coverage-fault-avif-superres-plane-postfmt-new-20261004.json`.
They lack source/build receipts and test attribution, so the coordinate change
does not establish a numeric marginal gain or a verified regression result.

### AV1 temporal-motion-field reservation fault contract — 2026-10-05

`fault-contract:avif:avif_temporal_motion_field_reservation_failure` injects a
capacity-overflow failure at the existing AV1 temporal-motion-field reservation
while decoding the active `decode:avif:baseline` Pillow input. The selected
contract checks the public AVIF `Dimensions` / `StillDecode` error and its full
diagnostic (`decode: AVIF AV1 validation failed: unable to allocate AV1 temporal
motion field`), then requires a retry to equal a fresh uninjected decode. The
injected outcome is indexed as `oracle_status: not_applicable` and does not
contribute to Pillow parity counts.

The six existing fault contracts passed in the selected baseline run, and the
new contract passed in its one-row run. Coverage MCP showed the allocation
error region as missing in the earlier AVIF report at `motion.rs:3113`; it is
absent from the selected run's gap list at `motion.rs:3122`. The region-level
incremental comparison against the six-case run estimates six newly observed
regions (0.002720 percentage points). This evidence is limited: reports have
no source/build receipts or test attribution. Line comparison is incomparable,
and branch detail does not reconcile with report totals. The selected reports
are
`target/release-evidence/coverage-fault-contracts-six-before-temporal-motion-20261005.json`
and
`target/release-evidence/coverage-fault-contract-avif-temporal-motion-field-v2-20261005.json`.

### AV1 split color-tile reservation fault contract — 2026-10-05

`fault-contract:avif:avif_color_tile_reservation_failure` uses the active
`decode:avif:multitile_color_split_groups` Pillow parity input. The dedicated
`scripts/generate_avif_color_tile_split_fixture.py` derives it from the pinned
`multitile.avif`, preserving frame-header and tile payload bytes while placing
the two color tiles in separate tile-group OBUs. It checks the split ranges
`(0, 0)` and `(1, 1)`, then requires Pillow 12.2.0 to return the same format,
mode, size, metadata, and pixels as the source. The generated input hash is
`654ef88dd8ebc71a554c03f721ad97581fcf2d2df0d825020d267939eb8f5e34`; its
98,304-byte RGB reference hashes to
`8fddfd016bbc17e2f00a5154ee6e50c8f1d9d6a8254279ddf57be490ecdf9e44`.

The selected Pillow parity row passed. The target-only coverage case injects a
one-shot failure at the existing incremental color-tile reservation and checks
the public `Dimensions` / AVIF / `StillDecode` error, including the diagnostic
`decode: AVIF AV1 validation failed: unable to reserve reconstructed AV1 tile
state`; retry must equal a fresh uninjected decode. Its oracle remains
`not_applicable`, outside the Pillow parity result count. The focused fault run
passed 1/1 and wrote
`target/release-evidence/coverage-fault-contract-color-tile-20261005.json`.

Coverage MCP reports the reservation-error closure at
`src/codecs/avif/av1/frame.rs:1574` as missing in the same-source AVIF parity
baseline, and absent from the selected fault report. The JSON region comparison
is incomparable because its detailed LLVM observations do not reconcile with
the reported totals. An LCOV incremental line comparison against
`target/release-evidence/coverage-avif-color-tile-parity-baseline-20261005.info`
reports 29 newly observed line coordinates (0.021037 percentage points); the
selected report is
`target/release-evidence/coverage-fault-contract-color-tile-final-20261005.info`.
The LCOV branch comparison remains incomparable because branch detail does not
reconcile with totals. Neither report has source/build receipts or test
attribution, so the line result is an unverified coordinate estimate, not a
causal coverage claim.

The full `scripts/generate_test_assets.py --format avif` command currently
stops earlier at its ICC fixture hash assertion. The dedicated split fixture
generator and Pillow reference generation completed independently.

### AV1 SGR restoration allocation classification fault contract — 2026-10-05

`fault-contract:avif:avif_sgr_restoration_output_reservation_failure` reuses
the active `decode:avif:animated_lossy_inter_420_superres_sgr_8bit_160x56`
sequence input to inject a deterministic failure at SGR output-plane
reservation. It protects a distinct public diagnostic: allocation failures
must remain `Dimensions` errors, not be mislabeled as malformed AV1 input.
The runner checks the full `SequenceDecode` diagnostic and that retry matches
a fresh uninjected sequence decode. The matching AVIF source row remains the
Pillow parity case; the injected result is `oracle_status: not_applicable`.

The focused target-only run selected, executed, and passed all nine fault
contracts declared in the matrix at that point. Its report is
`target/release-evidence/coverage-fault-contracts-nine-20261005.json` and
records 4,878/32,254 branches for this selected run. Coverage MCP found no
missing branch observation at the SGR allocation error conversion in
`restoration.rs:688`. The report has no source/build receipt or test
attribution, so its coverage coordinates are limited evidence; the runner
output is the evidence that the nine selected contracts passed.

### AV1 equal-width super-resolution parity — 2026-10-04

The active `decode:avif:superres_equal_width_16x16` case uses a pinned lossy
16×16 I420 still whose AV1 sequence enables super-resolution and whose frame
header signals denominator 9 while coded and upscaled widths both remain 16.
The fixture generator checks that syntax, repeats the encode, and verifies the
pinned Pillow 12.2.0 RGB pixels (768 bytes; SHA-256
`d5b4e270a08e4f03c6de84f4488420658a3ab62b00a954aeca14d50f58bb0eef`). The
encoded asset SHA-256 is
`2de74d720f8863be43049a3df776337c1cde494122a57093ffad532cea19a004`.

The selected parity row passed within the 57-test coverage-matrix harness. In
the final-source 472-row AVIF baseline, Coverage MCP lists the true arm at
`resize.rs:208` as missing; in the one-row measurement, that arm is absent from
the gap list while the false arm remains missing. The incremental comparison
is incomparable because LLVM detail does not reconcile with report totals.
Reports are
`target/release-evidence/coverage-avif-superres-faults-postfmt-baseline-20261004.json`
and
`target/release-evidence/coverage-avif-superres-equal-width-postfmt-new-20261004.json`.
The reports lack source/build receipts and test attribution, so this confirms
the selected coordinate change without a numeric marginal percentage claim.

### AV1 equal-width I444 super-resolution parity — 2026-10-05

The active `decode:avif:superres_equal_width_i444_16x16` row adds the 8-bit
4:4:4 path to the existing 4:2:0 equal-width case. The deterministic Y4M
source is pinned at
`0ed9bb725d8f4eb3ae33998b97f064ef9486683156befa28e3c572f652d42788`; the
fixture generator repeats the pinned encode and checks AV1 I444 and
equal-coded/upscaled-width syntax. The fixture hash is
`cd02b86f213b96f9d58bec3781f8855f325aa7b4c867a6fbf76f5802886420a4`. Its
768-byte Pillow RGB reference hashes to
`9cebc72915c26c647a920455c639b606976a1bbc0221ff9bf10cc0c9441864f9`.

The selected public parity run passed 1/1. Coverage MCP branch gaps at
`resize.rs:210` and `:215` now list only the `true` arms, so this I444 row
observes the previously missing `false` arms. The JSON incremental comparison
is incomparable because LLVM detail does not reconcile with the baseline
totals. The LCOV line union estimates 113 newly observed coordinates
(0.081973 percentage points) against
`target/release-evidence/coverage-avif-color-tile-parity-baseline-20261005.info`.
Both reports lack source/build receipts and test attribution, so this is a
limited coordinate estimate rather than a verified causal gain. The selected
reports are
`target/release-evidence/coverage-avif-i444-equal-width-selected-20261005.json`
and
`target/release-evidence/coverage-avif-i444-equal-width-selected-20261005.info`.

### JPEG optimized Huffman length-limit parity — 2026-10-05

The active `encode:jpeg:enc_optimize_huffman_length_limit` row encodes a
deterministic 2504×288 L8 PNG at quality 1 with optimized baseline Huffman
tables. Its balanced frequencies across seventeen AC symbols and EOB force
the JPEG 16-bit code-length limiter to rebalance the generated table. The
integer-basis fixture generator pins the source pixels and PNG hashes to
`8c5f7f43d4a78548fe6c8054b4c6a74279da5d46afe367adbd95a1ec8da92a9c` and
`21d7fefde68d0e4c9ee627c2222831fce271230e3365a8a921393bf37b554338`.
Pillow 12.2.0 with JPEG 6.2 returns the 8,167-byte output pinned at
`4f8d2339e8eb0053fc2190cbbdd7e10a99f74e4139edd7be16805a669db7d668`; public
Rust encode bytes match exactly, and decoded L8 pixels match the Pillow image.

The selected public parity run passed 1/1. Coverage MCP reports no missing
regions, branches, or lines in `jpeg::encode::huffman::optimal_table` for this
selected report. Incremental LCOV comparison is incomparable because the
measurement includes coordinates outside the older baseline inventory; the
JSON detail comparison is also incomparable. Reports lack source/build
receipts and test attribution, so no aggregate percentage gain is claimed.
The selected reports are
`target/release-evidence/coverage-jpeg-huffman-length-limiter-selected-20261005.json`
and
`target/release-evidence/coverage-jpeg-huffman-length-limiter-selected-20261005.info`.

### Target-only fault-contract lane — 2026-10-05

At this report snapshot, the shared matrix had nine active AVIF/JPEG fault
contracts, kept separate from Pillow parity with `oracle_status: not_applicable`. An earlier selected
lane run, recorded before the SGR restoration contract was added, executed
eight contracts and passed all eight public error and retry checks. Coverage
MCP reported no remaining branch gaps at the AVIF color-tile reservation fault
point in `frame.rs:1574` or its reservation error conversion at `:1577`. That
selected report records 4,876/32,252 branches; it is not full-suite coverage.
It has no source/build receipts or test attribution, so those totals are
limited selected-run evidence. Against
`target/release-evidence/coverage-avif-color-tile-parity-baseline-20261005.info`,
the incremental LCOV comparison estimated 1,366 newly observed line
coordinates (0.990925 percentage points). This remains an unverified estimate,
not a receipt-bound causal claim or full-suite gain. The historical reports
are `target/release-evidence/coverage-fault-contracts-eight-agent-20261005.json`
and `target/release-evidence/coverage-fault-contracts-eight-agent-20261005.info`.

That nine-case lane subsequently passed 9/9, including the SGR
restoration allocation-classification contract. Its report is
`target/release-evidence/coverage-fault-contracts-nine-20261005.json` and
records 4,878/32,254 branches. Coverage MCP reports no remaining branch gap at
the SGR allocation error conversion in `restoration.rs:688`. This selected
report also has no source/build receipts or test attribution; the runner output
establishes the nine passing contracts, while its coverage coordinates remain
limited evidence.

### Progressive CMYK without APP14 parity — 2026-10-04

The active `progressive_cmyk_no_app14` row strips the single Adobe APP14
segment from the existing progressive CMYK input. The 11,859-byte fixture has
SHA-256
`abe44e8567c1cbf463ba0e99cd8d96598fad6de8291e8723f3abd00d82ce769c`; Pillow
12.2.0 returns 65,536 CMYK bytes with SHA-256
`ff97c9d811b513694fa089a582cdf540e4f17c26bfa087cd9df458dd0f610385`. The
initial selected parity run found all channels complemented: the progressive
path inverted CMYK samples only when APP14 existed, unlike the baseline path
and Pillow's four-component output convention. The decoder now applies the
same inversion for all four-component progressive JPEGs and no longer stores
an unused APP14 transform byte. The targeted parity run passed for the new
case and the existing baseline and progressive APP14 CMYK neighbors. Its
matrix case is included in the same-source parity baseline; it therefore adds
no further marginal line coverage in a separate selected-row comparison.

### AV1 duplicate tile-group-start parity — 2026-10-04

The active `error_tile_group_start_order` AVIF row changes the second split
tile-group header from tile 1 to a duplicate tile 0, with zero alignment
padding. The 316-byte fixture hashes to
`68781acc246e2946c962c33990c53b50ed308a06fcdec6e421a38dbf353de4cf`.
Pillow reports `RuntimeError: Failed to decode frame 0: Decoding of color
planes failed`; the selected Rust/Pillow parity row passes 1/1 on the local
Pillow 11.3.0 host (the repository pins 12.2.0). The isolated report
`target/release-evidence/coverage-avif-tile-order-final.json` and the refreshed
parity-target report cover the missing true arm at
`src/codecs/avif/av1/frame.rs:1482`. Coverage MCP's selected-test comparison
observes one new coordinate, but remains limited without source/build receipts.
The deeper `accept_tile_group` ordering guard at line 1545 is rejected earlier
by `read_tile_group`, so this row claims only the public path it reaches.

### GIF metadata-limited decode parity — 2026-10-04

The active `palette_absent` case also exercises the public metadata-limited
decode path on its three Pillow-valid GIF inputs, with a 13,772-byte cap above
their metadata extents. The normal decode assertion remains active, and the
policy decode must return exact reference pixels. Its selected matrix row passes
1/1; the isolated report and Coverage MCP comparison observe the false outcome
at `src/codecs/gif/decode.rs:369`. The full comparison also observes the true
outcome at line 400 for the local-table variants. Pillow pixel references and GIF
assets were unchanged. The host Pillow version is 11.3.0 while the repository
pins 12.2.0, so this Rust-only policy-field change did not regenerate oracle
outputs.

### Progressive JPEG restart-overrun parity — 2026-10-04

The active decode row `progressive_restart_overrun` mutates the second
progressive scan in `progressive_restart.jpg` by appending sequence-correct
RST7/RST0 markers at its entropy boundary, before the following DHT and DRI
markers. Its 5,655-byte input hashes to
`509d436c87d82c880128aeffe1b41a39819dc7030141964bd302a6e4039f3c86`; pinned
Pillow 12.2.0 returns the same 128×128 RGB pixels as the unmutated source, with
reference SHA-256
`e92abdc1f9d14fd2491920de98bdcee028788715bebc6ba5d21b20d89b59fd76`. The
selected public parity row passes 1/1. Its isolated LLVM report records 4 true
and 5,632 false outcomes at
`src/codecs/jpeg/decode/progressive.rs:832`, exercising the extra empty segment
past the 256-MCU scan. Coverage MCP has no source/build receipt or test
attribution for this selected report; the named-row result and branch counts
are local execution evidence.

### JPEG low-quality 4:2:0 edge parity — 2026-10-04

### Monochrome AV1 film-grain parity — 2026-10-04

The active decode row `portable_lossless_filmgrain_monochrome_64x64` is a
6,210-byte 64×64 8-bit all-lossless monochrome AV1 primary still with frame
film grain. Its AVIF input SHA-256 is
`0bc3fe81f320d7f55853d53ec7b7fa20f099bf8af7e5e7ccbaa69556ccd980d4`; Pillow
12.2.0's 12,288-byte RGB reference hashes to
`853bfb557b4ab4960d708f4ecfeda145ed9feab8c987214d82ee6a20665e93f4`. The
selected public parity row passes. The decoder now admits monochrome primary
sequences that declare film-grain support because selected-display
materialization applies the frame's grain before checking the complete plane.
The auxiliary-alpha admission rule remains unchanged.

Coverage MCP compares eight newly observed branch coordinates in
`film_grain::apply_monochrome` with the earlier full report; eight validation
arms remain unobserved. Both reports lack source/build receipts and test
attribution, so this is limited coordinate evidence, not a verified causal
claim.

### JPEG low-quality 4:2:0 edge parity — 2026-10-04

The active encode row `enc_sub_420_q10_odd_streaming_edge` combines quality 10,
4:2:0 sampling, and a 33×33 source, exercising partial right and bottom MCU
edges on the low-quality streaming path. Its existing source asset hashes to
`afdec16d964c39b1d8f8329f72d798669ce507bf0904f96388fd273ddf65cc45`; Pillow
12.2.0's 33×33 RGB reference is 3,267 bytes with SHA-256
`9ee98db427d2e8ac625f6eb0e09e79f96ae13ff7c975297d537fe0e1725c27e4`, and the
encoded JPEG reference is 777 bytes with SHA-256
`db364d028ea02c8c5e3548e22df6c1d4a74466e9afb06cf952bfe0e47e71419e`. The
selected public matrix row passes. Its selected LLVM report records the true
arm of `present[0]` at `jpeg/encode/mod.rs:3925`; the remaining listed arms are
structural states, and the full coverage totals remain unchanged. MCP reports
no source/build receipt and unknown test attribution, so the branch observation
is coordinate evidence rather than a receipt-verified causal claim.

### GIF reserved disposal values — 2026-10-04

The active rows `enc_animated_disposal_8_reserved_bits` and
`enc_animated_disposal_255_u8_limit` exercise GIF sequence encoding with the
full public `u8` disposal range. Pillow 12.2.0 shifts the supplied value into
the graphic-control packed byte: disposal 8 produces `0x20` / `0x21`, while
255 produces `0xfc` / `0xfd`. The exact two-frame Pillow outputs hash to
`8a8954d61effd22db2292078adda561beb35102d50c36d2cd438094787afc190` and
`727785b857f0bbb52cddea5cbbad4a14b8721f68914eb31cc557b92344a69305`.
Both selected public parity rows pass. Coverage MCP returns no missing branch
group for `disposal_code`; its report has no source/build receipt and unknown
test attribution, so this is coordinate evidence rather than a verified
causal claim.

### Public parity rows added — 2026-10-04

The active AVIF rows `error_sequence_frame_id_width_exceeds_16` and
`error_sequence_identity_color_matrix_profile2_8bit` mutate the pinned
error-resilient animation's sequence header. The first changes its three-bit
additional frame-ID length from 0 to 2, declaring a 17-bit frame ID. The
1,084-byte input hashes to
`6970dc8e41a8861097dff0fbd8577b84dde72ec2018d3f2f75169bf7d78a123e`. Pillow
12.2.0 opens and verifies the container, then reports
`RuntimeError: Failed to decode frame 0: Decoding of color planes failed` on
load. The selected public decode row passes. The selected LLVM branch report
observes the true arm of the 16-bit guard at
`src/codecs/avif/av1/sequence.rs:266`, which the preceding full Coverage MCP
report listed as missing. Coverage reports lack source/build receipts and test
attribution, and the selected comparison is not aggregate-comparable, so this
is coordinate-level evidence. The second changes the same sequence from Main
profile to profile 2 while retaining 8-bit depth, then sets the CICP matrix to
the identity value. Its input hash is
`478d360b9c47d12f32c1a2f6ee9da0a483f7b6ac87b1ea93deda0e786dea07f6`; Pillow
12.2.0 opens and verifies it before reporting the same decode error. Its
selected branch report enters identity-matrix parsing, but raw counts at
`sequence.rs:354` show its `profile != 1` true arm and both nested
`profile == 2 && bit_depth == 12` outcomes remain unobserved. This malformed
row therefore does not close the profile/depth guard.

The active rows `baseline_reserved_ac_zero_size_eob` and
`baseline_multiscan_reserved_ac_zero_size_eob` cover Pillow's handling of a
reserved zero-size AC symbol (0x10) as end-of-block. Their 137-byte grayscale
and 165-byte 4:4:4 multiscan inputs hash to
`278b5b7d9a5d56799646149baec5c89be8d1f1bc1d39c74f0ab02b16a53c774d` and
`8aa96a1a0a6e8edd37fafbe7be424c3b3691deb16923a469c46718730a10e7bc`.
Pillow 12.2.0 yields 8×8 grayscale and RGB images respectively, all pixels
128; both selected public decode rows pass exact parity. The grayscale row
reaches the single-scan fast path, and the three-scan RGB row dispatches through
the scalar baseline block decoder. Zero-size EOB leaves the DC-only output
optimization available.

The active JPEG rows `baseline_420_restart_row_aligned` and
`baseline_420_restart_overrun` add an exact DRI=8 4:2:0 control and a
Pillow-tolerated surplus-restart mutation. The 5,954-byte control hashes to
`a0ea540bcedbb4fdb195055eee29aa0e3f5b9a856e6bea195002be0d8f7d5f4c`; the
5,958-byte candidate hashes to
`d52d934b119504b84ee1697bec2ce78077788e3a98ff8073a371a2a42b26715c`. Pillow
12.2.0 loads and verifies both as 128×128 RGB with the same pixel SHA-256
`e92abdc1f9d14fd2491920de98bdcee028788715bebc6ba5d21b20d89b59fd76`. The
candidate inserts `FF D0 FF D1` before EOI. With 64 MCUs and DRI 8, the decoder
expects eight entropy segments; the mutation supplies ten, reaching the
direct-path segment-count fallback at
`src/codecs/jpeg/decode/decode.rs:1180`. The focused public row passes 1/1.
Coverage MCP's refreshed full report has no missing branch group at that
location. MCP still lacks source/build receipts and test attribution, so this
is coordinate evidence rather than a receipt-verified causal claim.

The active JPEG row `baseline_444_single_component_scan_cb` retains only the
first Cb scan from `baseline_444_three_sequential_scans` and appends EOI. The
640-byte input hashes to
`6ab4a1f95019f910ed48b81d5889ff9a464904c9ba158232ab290e41dfe16231`; Pillow
12.2.0 opens, verifies, and decodes it as 8×8 RGB with pixel SHA-256
`3100a6a318864cd8fff4192c9acc15b51f2219a0195043761c7a38caf5b3963a`. This
single-scan route reaches the `scan_components.len() != 3` fallback guard in
`src/codecs/jpeg/decode/decode.rs:1590`, then decodes through the general
checked path. The public matrix test and complete `make coverage` campaign
pass. Coverage MCP no longer lists that outcome as missing, but cannot attribute
the hit to a test without source/build receipts. No codec or SIMD implementation
changed in this fixture batch.

The active JPEG row `baseline_cmyk_single_component_scan` retains only the
first component scan from `baseline_cmyk_13x9.jpg`. Its 13×9 CMYK input hashes
to `81109c230ad68dc6ddfccb25a441f9c0ceb2b5d336a3e948b2496628849c1c7f`, and
Pillow 12.2.0's decoded CMYK pixels hash to
`1ca1b4e0d3309f8b0e4d84ddc4fd20c8fc98827a62e0b4d3713ef3017f52f7b4`. The
focused public JPEG matrix passes 202/202 rows. This input reaches the
single-component fallback in `src/codecs/jpeg/decode/decode.rs:963`; the
coverage campaign below measures it in the current report.

The active AVIF row `animated_lossy_inter_420_superres_sgr_8bit_160x56`
extends the 10-bit super-resolution/restoration witness to 8-bit I420. Its
frame 1 uses largest-transform mode, 16×16 partitions, super-resolution from
128×56 to 160×56, and SGR restoration on all three planes. The 1,612-byte
timestamp-normalized input hashes to
`c294163610d4a45852fe374e0345c878979bb81e5ea94596960ef64411180fd7`.
Pillow 12.2.0 RGB frame hashes are
`69db76fd576323ba0a068072e8473ed16740c868e8c8c7199063056f97b4f9ed` and
`0bb7e3278f7cad0535a87e433d644434074b207396bf668ab3f5007dfca98061`; the
selected public sequence decode matches them byte-for-byte. The fixture uses
the pinned encoder compatibility patch documented in
`scripts/avif_fixture_oracle/README.md`.

The active AVIF rows `error_av1c_invalid_marker` and
`error_av1c_twelve_bit_without_high_bit_depth` each mutate one field in the
3,077-byte baseline file. Their input SHA-256 values are
`8e183203e849b4a7fd01bf5662b585a42b745b01b18a62a5c11cb64d550dbae9` and
`68e3963bfc3437156e871b5b48e279e1450fa8d67f22b03644925190b8b03695`. Pillow
12.2.0 reports `PIL.UnidentifiedImageError`; the Rust parity contract reports
`malformed`. The AVIF decode matrix passes 442/442 cases, and Coverage MCP now
returns no missing branch observations at `samples.rs:724` and `samples.rs:732`.

The active AVIF row `error_animated_duplicate_sample_size` retypes the animated
fixture's `stss` box as a second `stsz`, places it before `stco`, and makes the
following chunk-offset table incomplete. Its input SHA-256 is
`99400ac1612db40de4af63b07e7511483381590689be3cc0ed1607df63e44959`.
Pillow reports `PIL.UnidentifiedImageError`, and the Rust parity contract
reports `malformed`. The AVIF decode matrix passes 443/443 cases. Coverage MCP
reports no missing branch observation at `samples.rs:2189`; the report still
lacks a source/build receipt, so this remains coordinate-level evidence.

The active AVIF row `animated_lossy_inter_420_superres_sgr_10bit_160x56` adds a
full-range two-frame 10-bit I420 sequence. Its inter frame is coded at
128×56, super-resolves to 160×56, and uses SGR restoration on all three planes;
the bitstream uses the supported largest-transform mode. The 1,366-byte
timestamp-normalized input hashes to
`016e4a8433002b60899744fba6f26a7c10af82c192f65b4c5c19233b15c3cb11`. Both
public Rust decode APIs match Pillow 12.2.0 byte-for-byte: frame hashes are
`69db76fd576323ba0a068072e8473ed16740c868e8c8c7199063056f97b4f9ed` and
`199a031bed1879efe5b17d90bacd44226ec2c05da120f4e981326695e6d493d0`, each
with a 33 ms duration. The selected manifest parity row passes 1/1. Coverage
MCP's full-report coordinate comparison shows this case alongside other
all-feature work exercising 61 additional outcomes in the high-depth
super-resolution/restoration admission function; the function now has 65
unobserved branch outcomes. MCP reports no source/build receipt or test
attribution, so the comparison is not a verified causal claim.

The active AVIF row `animated_lossy_inter_420_superres_select_10bit_160x56`
adds a companion full-range 10-bit I420 sequence. Its inter frame uses
TX_MODE_SELECT with the same 128×56 to 160×56 super-resolution geometry and no
active restoration. The 1,311-byte timestamp-normalized input hashes to
`04e31a3ac36c25ef77061a2ed09b79fd8fb885a5ff14d6ab3a82e2d32ad3b02e`. Public
Rust decode and sequence decode match Pillow 12.2.0 byte-for-byte; the RGB
frame hashes are `69db76fd576323ba0a068072e8473ed16740c868e8c8c7199063056f97b4f9ed`
and `7aa2455358aa4a1d51b3a4a263a425229b1be524efd68979a26dad18c8250fef`, both
with a 33 ms duration. The focused row passes 1/1, and native repetition is
confirmed as infinite. Coverage MCP's selected comparison against the SGR row
shows seven newly observed coordinates in
`complete_high_depth_inter_reconstruction_context`; the all-feature report
also adds one covered branch relative to the prior full report. Those reports
lack source/build receipts and test attribution, so these remain coordinate
signals rather than verified attribution.

The active ICO row `error_short_header_metadata_preflight` uses a four-byte
recognized signature without the remaining directory header. Its input SHA-256
is `6b1e73a0094b7b812d3b9e22cffb4f8239319847522c4fa103753b6950020f93`; the
manifest also compares `decode_with_policy` with a zero metadata-byte limit.
Pillow reports `PIL.UnidentifiedImageError`, and the Rust parity contract
reports `malformed`. The ICO decode matrix passes 57/57 cases, and Coverage MCP
returns no missing branch observation at `ico/decode.rs:123`.

The active WebP encode row `enc_lossless_sampling_row_compaction_checkpoint`
uses a deterministic 512×1,024 RGB input whose two VP8L histogram bands compact
to multiple rows. The token-aware encode shard passes 47/47 cases, including
byte-for-byte Pillow parity for the new row. The input SHA-256 is
`36d8d0a1048480054a0d99cb503cab17e299d0f6fa1f9afdbee81e1568e43478`; its
395,738-byte Pillow WebP output has SHA-256
`5d216ba74978fb381c518359f11db60861b8ce6a25ab2c30edd4bdcd8e1ed012`. The
selected Coverage MCP report has no missing branch at
`webp/native/encoder.rs:1454`, the nonzero-row token checkpoint. Its report
still lacks a source/build receipt, so this is coordinate-level coverage
evidence.

### Token-aware VP8L sampling checkpoint parity — 2026-10-04

The active WebP encode row `enc_lossless_sampling_equal_rows_checkpoint` uses
an 8,192×16 RGB fixture with 511 unique colors. Repeated histogram
distributions across tile rows reach the token-aware row and column sampling
checkpoints without selecting the palette transform. The selected public row
passes Pillow byte parity, including the token-aware Rust output. The input
SHA-256 is
`cda7a74a4e7b6e8414374a82307919b65426a6c24b252055e6f89c0cc230650a`; its
134,180-byte Pillow WebP output has SHA-256
`3da23b0660aba09c176a17b552913c14e33f2ac943a265582bdbfb5a0529b92f`.
Coverage MCP reports no missing branch outcomes at
`webp/native/encoder.rs:1378` and `:1418`, the 1,024-comparison row and column
checkpoints. The incremental LCOV estimate reports four newly covered lines
and a 0.002902 percentage-point gain against the pre-row baseline. That
increment is coordinate evidence only because source/build and test receipts
are unavailable; branch-level incremental unions remain incomparable.

The current matrix contains 2,070 total rows: 1,607 decode / inspect /
verify rows and 463 encode rows. Of those, 1,607 decode rows and 431 encode
rows are active; 0 decode rows and 32 encode rows are planned. The 25
fault contracts are tracked separately from the Pillow parity totals. Full
and selected MCP reports lack source/build receipts and test attribution, so
MCP locations are coordinate evidence rather than verified named-test
attribution.

### Current AVIF location generator observations — 2026-10-05

The current AVIF location inputs come from
`scripts/generate_avif_idat_fixture.py` through the normal asset hook. They
exercise a truncated extent index, an unsupported three-byte index width,
assembly from two indexed idat extents, and an extent ending one byte beyond
idat. Pinned Pillow 12.2.0 rejects the first two at open, preserves the split
item's complete image result, and accepts inspection and verification of the
overrun before frame decoding rejects it as malformed. The canonical reference
generator reads configuration depth through the bounded configuration-only
reader in `scripts/inspect_avif_bitstreams.py`; its full sample inspector keeps
extent validation. Independent enrichment failures abort generation. These
ordinary cases remain separate from
the 24 target-only fault contracts.

### Token-aware PNG reserved-distance parity — 2026-10-04

The active row `error_token_aware_reserved_deflate_distance` reuses the pinned
`zlib_reserved_distance_symbol.png` input through the public
`decode_with_token` API and checks the same Pillow 12.2.0 malformed-decode
contract as ordinary decode. Its 67-byte fixture has SHA-256
`222fcf6a5fc0b121582aa779cd86a7e77e35948b779e71151313d9ced571b7b1`; the
selected public parity row passes 1/1.

The selected LLVM report records one hit on the reserved-distance true arm at
`src/codecs/compression/deflate.rs:539`, which the earlier full report lists as
uncovered. The other outcome remains uncovered in this one-row selection. The
full-versus-selected MCP comparison is incomparable because baseline branch
detail is incomplete; both reports lack source/build receipts and test
attribution, so this is coordinate-level evidence only. No codec or SIMD
implementation changed.

### AVIF profile-2 12-bit identity-CICP parity — 2026-10-04

The active row `profile2_identity_12bit_444` uses a reproducible 16×16 full-range
4:4:4 AVIF with profile 2, 12-bit samples, and CICP 1/13/0. Its 1,188-byte
fixture SHA-256 is
`1ce2aae5539074e3a65092f185c4db34537462d637859d4fd85449bb15557931`; the
768-byte Pillow RGB8 reference SHA-256 is
`45114693c3cde135ae810d49fbfb8bcf54204da4b9bf11fa3094b5e177686343`. The
selected public parity row passes 1/1. The decoder maps AV1's Y/Cb/Cr identity
planes to G/B/R and truncates validated 12-bit samples to RGB8; existing YUV
conversion kernels are unchanged.

The fresh selected LLVM report records the true outcomes of all three guards at
`src/codecs/avif/av1/sequence.rs:354` (three hits each). The separately selected
8-bit malformed identity-CICP row records the false depth outcome (three hits),
so the two public rows exercise both depth outcomes. Coverage MCP reports the
opposite outcomes missing in their respective single-row selections. The
`profile != 1` false arm and `profile == 2` false arm remain unobserved. MCP
reports lack source/build receipts and test attribution; its selected-test
comparison is limited coordinate evidence, not a verified full-suite gain.

### VP8 loop-filter mode-delta parity — 2026-10-04

The active WebP row `lossy_vp8_mode_filter_delta` uses a deterministic
128×128 RGB keyframe with a +5 VP8 loop-filter mode delta. The 3,312-byte
fixture SHA-256 is
`86b97b8489b9ec2318858bef5d91be138e1222398dd2b8ffbde6dded0e598b55`; its
Pillow RGB pixel SHA-256 is
`ce195285d0ae374bc2ae5538da590658fbe26846b27cf2b0d2300979319f4c99`. The
selected public decode parity row passes 1/1. Its fixture generator applies the
encoder-only patch to a temporary copy of pinned libwebp 1.6.0 and checks the
Pillow output hashes.

The selected LLVM report records true hits at
`src/codecs/webp/native/vp8.rs:1158` (3), `:1283` (3), and `:1913` (192); the
mode check at `:1915` observes both outcomes (177 true, 15 false). Coverage MCP
confirms the opposite outcomes at 1158, 1283, and 1913 remain unobserved in this
selected-only report. The earlier full report listed these true outcomes as
missing, but neither report has source/build receipts or test attribution, so
the comparison remains coordinate-level evidence.

## WebP malformed chunk extents and GIF89a exact metadata budget — 2026-10-04

The active WebP row `error_truncated_chunk_header_metadata_preflight` retains
the valid VP8X and EXIF chunks from `exif.webp`, removes its VP8L image chunk,
and adds one byte inside the corrected RIFF extent. Pillow rejects the
resulting malformed file; the Rust row exercises the metadata preflight path
with a zero-byte metadata limit. The full parity matrix passes, and Coverage
MCP no longer lists the short-chunk-header branch at
`src/codecs/webp/decode.rs:203`. The report is source-unverified and its test
attribution is unknown, so this is coordinate evidence.

The active WebP row `error_chunk_exceeds_riff_metadata_preflight` uses the same
extended metadata container with the EXIF chunk length raised past the RIFF
extent. Pillow and the Rust public operations reject it, and the zero-byte
metadata-limit call reaches the chunk-extent error at
`src/codecs/webp/decode.rs:223`. Coverage MCP no longer lists either line 203
or line 223. Its two remaining WebP metadata branches, at lines 185 and 186,
reject bad RIFF/WEBP signatures before public metadata scanning can be entered.

The active GIF row `gif89a_exact_metadata_limit` checks that the existing
GIF89a `lzw_kwkwk.gif` fixture decodes at its exact 39-byte metadata limit.
Its Pillow pixel parity passes. This case adds a metadata-policy regression
check but does not change aggregate coverage; the remaining reported GIF
signature arm corresponds to invalid signatures, which public format
detection rejects before metadata scanning.

An earlier local full report recorded 86,813/142,471 lines (60.9338%),
16,768/32,250 branches (51.9938%), 4,849/8,990 functions (53.9377%), and
132,773/220,396 regions (60.2429%). The GIF exact-budget case adds a parity
regression check without changing that report's aggregate coverage. The report
and MCP view lack source/build receipts and test attribution, so their source
locations are coordinate evidence rather than verified causal attribution.

The active AVIF row `iref_future_version` adds an empty version-2 `iref`
FullBox and preserves exact Pillow 12.2.0 metadata and pixels. Its selected
public Rust parity row passes.

The active JPEG row `baseline_444_multiscan_final_empty_entropy` derives an
8×8 baseline 4:4:4 image with an empty final component scan immediately before
EOI. Pinned Pillow 12.2.0 decodes the fixture and the selected Rust parity row
passes. The refreshed local LLVM report with both rows records
86,819/142,504 lines (60.9239%), 16,751/32,252 branches (51.9379%),
4,847/8,990 functions (53.9155%), and 132,753/220,401 regions (60.2325%).
Coverage MCP no longer reports missing branch observations at
`avif/samples.rs:982` or `jpeg/decode/decode.rs:2473`. The local report has no
source/build receipt or test attribution, so these locations are coordinate
evidence rather than verified causal claims.

MCP still reports the true outcome for the right-hand comparison at
`tiff/decode.rs:1872` and `:1965`. The active invalid-first-code TIFF row makes
the left side of `code >= CLEAR || output.len() >= expected` true, so the
right side is short-circuited. Public TIFF inputs keep `expected` positive and
the decoder stops when it reaches that size; no public Pillow-parity input can
take the right-hand true outcome. This is not a missing public fixture.

The active WebP row `enc_lossy_method_6_token_checkpoint` exercises the
token-aware lossy VP8 method-6 trellis path on `lossless.webp`. Its 128×128 RGB
input SHA-256 is
`20eb48b4ef2a42d4842684930c2023f7d14ac343b825d19a185e8dda5b25f8b2`; its
49,152-byte Pillow RGB reference hashes to
`2c759de6a1ae9096f129744a6c04d24f1a611bcd8841b26810cc1c583b6a1525`, and its
3,570-byte Pillow WebP output hashes to
`ba524d40418e1bd254e437cb4cf51d98e10d31df9fb5a163938840b9e3fddf4f`. The
token-aware WebP encode shard passes 47/47 cases, including byte-for-byte
parity for this row. Coverage MCP's focused report no longer lists a missing
group at `webp/encode/vp8/quant.rs:238`, and the raw LLVM export records 10,240
executions of the token-specific quantizer specialization. The aggregate
branch percentage did not change; the report has no source/build receipt or
test attribution, so these are coordinate and specialization observations,
not receipt-verified causal attribution.

A parity-only coverage-matrix run passed 38 public parity-target tests with 18
internal/admin tests filtered out. Its selected report,
`target/release-evidence/coverage-public-parity-20261004-final6.json`, records
83,386/142,489 lines (58.5210%), 16,208/32,252 branches (50.2542%),
4,554/8,987 functions (50.6732%), and 127,597/220,381 regions (57.8984%).
Compared with final5, raw covered totals rise by 50 lines, 12 branches, six
functions, and 81 regions. Coverage MCP's selected comparison observes six
new `parse_pasp` branch coordinates: the valid false outcomes in both parser
implementations. Across the three TIFF
increments, Coverage MCP observes the zero next-IFD arm at `decode.rs:46`, three
`metadata_bytes` outcomes, and the `inspect_basic` arm at `inspect.rs:47`.
Current filtered gap queries list no missing TIFF outcomes in
`metadata_bytes`, `decode_page`, or `inspect_inner`. MCP marks the comparisons
limited because source/build receipts are unavailable; they are coordinate
evidence rather than verified causal or regression claims. The incremental
comparison to final5 is limited because source/build identity is unverified.
These selected-target totals describe the public parity test target and do not
replace the full release coverage campaign.

The active AVIF row `primary_item_pasp_4x3` adds the exact 342-byte input
`primary_item_irot_pasp_4x3.avif` (SHA-256
`2acc2e59572718a5fd1247aef7d18ab7666943533f068c221117df05378c4564`). It
associates a valid 4:3 `pasp` property with the existing 270-degree primary-item
rotation fixture; the property and association add 17 metadata bytes, and the
`iloc` extent is relocated from byte 285 to 302. Pillow 12.2.0 verifies and
loads it as 2×3 RGB with the same info and all 18 pixel bytes as the source.
The focused public AVIF parity target passes 1/1. Coverage MCP reports
`target/release-evidence/coverage-avif-pasp-20261004.json` and
`target/release-evidence/coverage-public-parity-20261004-final6.json` leave only
the three invalid-value arms in each `container::parse_pasp` and
`samples::parse_pasp` unobserved; the valid false/success outcomes are present.
Both reports lack source/build receipts and test attribution, so this is
coordinate evidence, not a verified suite-wide coverage gain.

The active AVIF row `clap_zero_width_ignored` adds an essential clean-aperture
property with a zero width numerator to the 3,077-byte baseline AVIF. The
3,118-byte input hashes to
`ce95ee92b5498964ff6e3e0bd383608c559fafcafe7bd110e5f2101d10734572`;
Pillow 12.2.0/libavif 1.4.1 verifies and decodes it as 128×128 RGB with the
unchanged pixel SHA-256
`f1a2555b1c61036af2bd1d3d125a6a1343993873d9e5d58e3caee79989095dcf`. Rust's
inspector and decoder now preserve this property as opaque metadata and do not
apply a crop, matching Pillow.

The active AVIF row `clap_zero_width_denominator_ignored` adds an essential
clean-aperture property with a nonzero width numerator and a zero width
denominator to the same 3,077-byte baseline. The 3,118-byte input hashes to
`8641c0eff26bfdf83f998e2a13737eeea8dbff30d04c818109a358d3a267b4e1`.
Pinned Pillow 12.2.0/libavif 1.4.1 verifies and decodes it as 128×128 RGB with
the unchanged pixel SHA-256
`f1a2555b1c61036af2bd1d3d125a6a1343993873d9e5d58e3caee79989095dcf`.

Six further active rows exercise the other ignored clean-aperture cases. The
3,118-byte zero-height-numerator, zero-height-denominator, zero-horizontal-
offset-denominator, and zero-vertical-offset-denominator inputs hash to
`ec0031d6d620d303a05937051504d621498dfe73920c51fb072e03c7746fbe2c`,
`1004a811270a222f0072637fafc2b1004f25084a6f8f1b34976696595fefefab`,
`9d67878e798b93a6fe551fdd492701a501d4744d847f26c670e5c793c9a367c3`, and
`662fca402bf3a78537aaaadbe101c5ceaee736a064ddcaa46ff5931ec17971c2`.
The trailing-payload rows contain one and 16 bytes and hash to
`ece2fc8f98cc42f31a8806690ac415460d5eadf96c93492ba2d710c0b2e22bde` and
`93038a983d242ee2204d866b63a1811573acc7f8e38699595d328ccdf49cf2d7`.
Pinned Pillow 12.2.0/libavif 1.4.1 verifies and decodes all six rows as
128×128 RGB with the unchanged pixel SHA-256 above. Both AVIF parsers now
preserve these unusable clean-aperture declarations as opaque metadata. The
full AVIF matrix passes 467/467, and Coverage MCP reports no missing branch
group in either `parse_clap`; its source identity and test attribution remain
unverified.

The active GIF row `lzw_long_expansion_token_checkpoint` is a 1,024×515 Pillow
12.2.0 palette image. Its 1,909-byte input hashes to
`60bfab0d96d4fcc742246b440f337bc183e1e1ead86dfa02a8fb4013f07e571a`; its
527,360-byte decoded reference has SHA-256
`8245292ed38da63c136ed67020067c8dad124da8cadacdb8cad7489f914fa01b`. Its
valid LZW phrase reaches the cancellable expansion checkpoint, and the new GIF
fixture passes exact public pixel parity. A separate
`error_lzw_end_only_token_checkpoint` row checks the immediate-end-code error
through the token-aware public decoder; it was split out of the aggregate
malformed group so the token-aware assertion applies only to the relevant
input. Two more dedicated error rows cover an invalid first LZW code and a
future dictionary reference. The selected four-row GIF shard passes 4/4. The
full selected Coverage MCP report returns no missing branch groups for
`decode_lzw_with_token` or `append_code_with_token` in
`src/codecs/gif/decode.rs`; this remains coordinate evidence.

Four existing Pillow rows now also exercise public `decode_with_policy` at the
measured metadata boundary: BMP `depth_24` at 54 bytes, ICO `single_icon` at 22,
WebP `icc_profile` at 69, and TIFF `tiled_missing_byte_counts` at 146. The
focused BMP/ICO/WebP selector passes 3/3 and the TIFF selector passes 1/1; all
four retain exact Pillow pixel references. Against the prior selected report,
MCP's filtered gap listings now show one missing BMP `metadata_bytes` outcome
instead of two, no ICO `metadata_bytes` group instead of one, six WebP
`metadata_bytes` outcomes instead of 19, and three TIFF outcomes instead of 10.
The TIFF check exercises the unequal offset/count length path at
`tiff/decode.rs:150`. The cycle and empty-directory policy rows exercise the
remaining TIFF `metadata_bytes` exits; the final selected report has no missing
branch groups for that function. MCP's aggregate incremental comparison remains
incomparable and its reports lack source/build receipts and test attribution,
so these are coordinate-level observations rather than verified test
attribution.

The TIFF `cyclic_ifd_leniency` row exercises metadata policy on the
self-referential `cyclic_ifd.tiff` with its measured inclusive limit of 140
bytes. A dedicated `error_empty_ifd_chain_metadata_policy` row exercises
`empty_ifd_chain.tiff` with a zero limit and retains Pillow's malformed error.
Together they cover the cycle stop, matching strip offset/count arrays, and
no-directory error path. The final selected report has no remaining TIFF
`metadata_bytes` branch gaps; source/build identity is unverified.

The TIFF `single_page` row requests page index 1 from `single.tiff`. Pinned
Pillow 12.2.0 reports `builtins.EOFError`, and the public owned and borrowed
`decode_frame` calls are pinned to Rust's `parameter` error. This reaches the
zero next-IFD termination arm in `tiff/decode.rs:46`; the preceding selected
report listed that arm as missing. The final parity-only run passed with the
frame request included, and Coverage MCP no longer lists that arm. The report
has no source/build receipt or test attribution. The row also exercises public
`inspect_basic`; the parity harness checks that its complete page count matches
the full inspection and Pillow reference, covering the `basic` path in
`tiff/inspect.rs:47`.

The active TIFF row `error_lzw_invalid_first_token_checkpoint` checks the
invalid-first-code error through the token-aware public decoder as well as the
ordinary decode path. It is retained for that API parity contract, not claimed
as a new branch gain. A fresh one-row LLVM report,
`target/release-evidence/coverage-tiff-invalid-first-token-selected.json`, records
the invalid `code >= CLEAR` arm four times in ordinary decode and twice in
token-aware decode at `tiff/decode.rs:1872` and `:1965`. The RHS
`output.len() >= expected` is short-circuited and has no counter in this
selection. Coverage MCP reports the missing RHS outcomes in the one-row view;
this does not identify a missing public input, because the first code is already
invalid and a valid first code starts with zero output against a positive
expected length. The same fresh report lacks source/build receipts and test
attribution.

The final5 TIFF branch query has four remaining outcomes: the invalid-signature
arm in `parse_header` at `decode.rs:1005`, the output-full RHS in ordinary and
token-aware LZW at `decode.rs:1872` and `:1965`, and the empty-input arm in
`encode_lzw` at `encode.rs:723`. Public format dispatch rejects signatures
before `parse_header`; TIFF validation requires positive decoded dimensions and
skips zero-row compressed strips before LZW; and public encoder validation
passes nonempty pixel data to `encode_lzw`. These defensive arms have no known
public Pillow-compatible parity witness under the current APIs. This is a
source-level reachability assessment; the MCP report lacks source/build receipts
and test attribution.

### Public parity rows added — 2026-10-03

The active JPEG row `baseline_444_restart_marker_fill` adds three legal `0xFF`
fill bytes before the restart markers of a Pillow 12.2.0 baseline 4:4:4 JPEG.
Its input is 2,593 bytes (SHA-256
`c5369d29b879ca0d40ac01c927c4a5d845d3fc9f59ae07ccb7cb5947b0e5fee2`); its
2,835-byte Pillow RGB reference has SHA-256
`0276fe6a4734a34cee3009e2c6aca9c36323785712622759f3a8b9e835453895`. The
selected public parity run passes. The pre-row full report listed the true arm
of the repeated-fill loop at `jpeg/decode/decode.rs:2484`; the selected report
observes it, and the refreshed full report has no gap at that location.

The active AVIF row `no_intrabc_repeated_patch_monochrome_256` pairs the
existing monochrome IntraBC witness with an all-lossless frame that disables
IntraBC. Its input SHA-256 is
`34321df1afb253b459f20169ddc4d3b588e517b4832c2c2460d6900ca647c7a3`; its
196,608-byte Pillow RGB reference has SHA-256
`ed729898cd63496b4a1aa628686ebff2dd7a0f383dfdae854f7cd77dae54ce22`, and the
selected public parity row passes. Coverage MCP still reports the false arm of
`!header.superres_enabled` at `av1/frame.rs:3346`; this row tests the valid
IntraBC-disabled bitstream state and is not credited with closing that
superres branch. Selected and full reports have no source/build receipts and
unknown test attribution, so the branch observations are coordinate evidence.

The selected and refreshed full reports show no remaining branch gap at
`jpeg/decode/decode.rs:2484`, `av1/frame.rs:3647`, or
`webp/native/decoder.rs:226`. MCP incremental comparison is unavailable because
the full report's detailed baseline branch inventory is incomplete; no
four-metric union or receipt-verified causal attribution is claimed. The full
report still has 15,726 unobserved branch outcomes, so the 100% goal remains
open.

The existing WebP rows `enc_lossless_indexed` and
`enc_lossless_indexed_alpha` now also request public `encode_with_token` parity.
The 9,190-byte opaque Pillow reference has SHA-256
`fc35d688c00c25e149daee2fc8716ca3e6fcd671a67430db0169671c0dc39b34`; the
2,022-byte alpha reference has SHA-256
`5e8dfcc6d3fab8655405b166a951cbb1885400c7b6966945e112cc0473d0c339`. Both
token-aware calls match those exact bytes, and the indexed-alpha call also
retains its 65,536-byte RGBA reference (SHA-256
`f622a7e1dbe429f8414c6081c3d79c93573e37b18538dc828c497f13fa8b3659`). The
selected MCP report and refreshed full report no longer list either outcome of
`has_alpha` in the token-aware `expand_indexed` path at
`webp/encode/mod.rs:663`. MCP source/build receipts and test attribution are
unavailable, so this is coordinate evidence.

The active WebP error row `error_truncated_riff_extent_metadata_preflight`
adds a reproducible 16-byte RIFF prefix declaring 32 container bytes while
providing only the 16-byte header. Pillow 12.2.0 detects WebP and rejects the
input with `OSError: could not create decoder object`; the selected public
parity row passes. The matrix also repeats public still decode with a zero-byte
metadata limit and checks the resulting error kind and format against the same
Pillow error contract. The full Coverage MCP report no longer
lists a missing branch at `webp/decode.rs:193`, the declared-extent guard. MCP
source/build receipts and test attribution remain unavailable, so the
coordinate mapping is not receipt-verified.

The WebP decode row `lossy_vp8_segmentation_feature_data_disabled_16x16`
(input SHA-256
`b1fce88cb549bb4429cada0271752681530e8a86bc1c76256c57e581cf7c93ad`) matches
its pinned Pillow RGB reference and passes selected public parity. A Coverage
MCP comparison with the saved pre-row report observes the false arm at
`src/codecs/webp/native/vp8.rs:1177`; the refreshed filtered query has no missing
branch there. The reports lack source/build receipts and test attribution, so
this is coordinate evidence, not receipt-verified causal attribution.

The GIF encode row `enc_rgb_balanced_checkerboard_token_split` uses source
SHA-256 `d5045ccb8a0f05d35a07e300642c7501d4349af24aee2157b0b6886a0e3a32d4`
and matches the 78-byte Pillow GIF reference with SHA-256
`edcba5284f8c6f34ffb4de0f7c6012a90b7a650c3c693ac97efe6ad176c74f6d`. Its
selected public parity passes. The full comparison shows no new branch
coordinate: the false arm for `split > 0` at
`src/codecs/gif/encode.rs:1832` remains missing, while this input exercises the
false outcome of the later equality operand. The normal 22-row color
quantization partition also leaves that arm unobserved. Retain this row for
public token-encoding parity, not as a coverage gain; MCP source/build identity
is unverified.

The additional GIF row `enc_rgb_257_color_low_green_dominant` exercises a
skewed 257-color RGB distribution (source SHA-256
`221a0b156a315d1113a1d491c7fe497b21253a21ff17ad28c9ac7dd6f3c7c0e7`) and
matches the 1,124-byte Pillow GIF reference with SHA-256
`f63195630ba5ccc8c680101828adceb54944bfa35dd235d48ae7a7921cf070cd`. Its
selected public token-encode row passes. The full branch comparison shows zero
new coordinates, and the refreshed query still reports the false arm of the
`split < sorted.len()` condition at `src/codecs/gif/encode.rs:1814`. The selected
input reaches the final axis entry but exits through the median-threshold
`break`; treating the condition's false arm as unreachable for valid public
inputs is a source inference, not receipt-verified Coverage MCP evidence. Keep
the row for its exact palette-encoding parity value, not as a coverage gain.

The AVIF decode row `animated_lossless_inter_420_clipped_b32x32_10bit_28x28`
uses input SHA-256
`f9d829f20968483dbd078a47f136b9c413205ce78d423fb0b5f1bdb2d2c0cce4`. Pinned
Pillow 12.2.0 returns two 28×28 RGB frames at 100 ms each, with hashes
`925479a7e1eb832f5ce02848b2fb4df84769979f770e056ac398bf856cdcca1f` and
`e8e893f5c8a17b1d22653f7d92adfed2021901ab37d9bcbc72debccfba3645bd`; the
selected public parity row passes. The selected report still shows missing
outcomes in `decode_inter_transform_size` at `av1/entropy.rs:6927` and `:6938`,
so this row is retained for high-bit-depth clipped-frame parity rather than as
a coverage gain. The report has no source/build receipt or test attribution.

Three WebP encode rows cover compressed, raw, and equal-sized alpha candidates.
`enc_lossy_rgba_alpha_compressed` (source SHA-256
`bdf14b68c3d95d39080728931fee6002ddb27624e816df41a1120ba497ee2a81`) matches
the 140-byte Pillow output SHA-256
`9528b8edc7bbf5d2010bcaa1492f07feeef7d9004073c4ef3fdf8127f5a5a307`.
`enc_lossy_rgba_alpha_raw` (source SHA-256
`e1105dfac182e64e01ef20cd87bbe44ffa4a187666161e1d54858afaf947ed36`) matches
the 4,210-byte output SHA-256
`c3fed186ee97cc2418fd1815880646118e73fdaa3fed896bf977118972b13e4d`.
`enc_lossy_rgba_alpha_size_tie` uses a 9×1 constant-alpha RGBA source (source
SHA-256 `6e7e517108ef0e25c4f6c4257318ebf06f0abb2bea133804ea2417883e586d14`)
and matches Pillow's 100-byte output SHA-256
`f99e6b76bea04b1263728c6f4c6d275db57a7e9e1636887f6da8df0f1d84b594` through
both ordinary and token-aware public encode calls. The exact tie exposed a
parity bug: Pillow keeps the compressed candidate on equal lengths. Both Rust
paths now choose raw only when it is strictly shorter, retaining the existing
compressed buffer on a tie. The selected row passes with five public encode
calls and no panics. Coverage MCP reports both outcomes of the ordinary
Noop-writer size decision covered at `webp/native/encoder.rs:2750`; its
token-specialized sibling arms remain unobserved in that block because the
specialization receives `Some(token)`. MCP source/build receipts and test
attribution are unavailable, so the coverage mapping remains coordinate
evidence.

The AVIF matrix now includes two size-zero box cases. `size_zero_top_level_free`
appends a size-zero top-level `free` box and payload to the baseline (input
SHA-256 `0390a339020a54aa4ddb7600b988407c10efdc21ec0bdff762a793b7a1217e6c`).
Pinned Pillow 12.2.0 returns the same 128×128 RGB pixels as the baseline: 49,152
bytes with SHA-256
`f1a2555b1c61036af2bd1d3d125a6a1343993873d9e5d58e3caee79989095dcf`. The
`error_meta_nested_zero_size_ipma` case sets the final nested `ipma` box size
to zero (input SHA-256
`01ef1212f06626031f796c5268e49065a340ab0308a0cc7db4a0fdbafcfdbc65`); Pillow
rejects it as malformed, and its active public row passes. A Coverage MCP
coordinate comparison with the pre-case full report shows one additional
observation at `samples::next_box` (`samples.rs:264`), and the refreshed
filtered query reports no remaining branch gap there. The comparison has no
source/build receipt or test attribution, so it cannot bind that observation to
these rows. `avif/container.rs:449` still has an unobserved nested-size
rejection arm: public validation rejects the nested box in `samples::validated`
before reaching the duplicate container parser.
The `error_meta_duplicate_primary_item` AVIF row changes the baseline's nested
`iloc` FourCC to a second `pitm` without moving box data (input SHA-256
`3badc9ce02273d7ac0210aa6bcc3634e75cfed2531b30427bf721620452ab2c3`). Pillow
rejects the malformed meta box and the selected public row passes. Compared
with the full report captured before this row, Coverage MCP reports one new
observation at `samples::parse_meta` (`samples.rs:471`); the refreshed filtered
query has no gap there. The report comparison is coordinate-only because it
has no source/build receipt or test attribution.
The `error_meta_nonzero_version` AVIF row changes the top-level `meta` FullBox
version from zero to one (input SHA-256
`81cbc07cf4cad9653c58c47c7b88fcd3c323d7f749e2c47cbb39995a3a411b9b`). Pinned
Pillow rejects it as malformed, and the selected public AVIF row passes. The
current filtered Coverage MCP query reports no remaining branch gap at
`samples.rs:449`; source/build receipts and test attribution are unavailable.
Two more AVIF error rows relabel a later `meta` child to exercise duplicate
tables. `error_meta_duplicate_item_info` changes `iprp` to a second `iinf`
(input SHA-256
`51045cb251dc8cbbdfa245be7893f8fa54735e16414a71cb77abd0708e4f7aec`), and
`error_meta_duplicate_item_location` changes `iinf` to a second `iloc` (input
SHA-256 `8139b4019d1bdaad53622eaa137d0b8987b29b06a1d360df2c1dc0218d546f76`).
Pinned Pillow rejects both, and both selected public AVIF rows pass.
Compared with the saved report before these rows, Coverage MCP shows two new
coordinates at `samples::parse_meta` (`samples.rs:478` and `:499`); the filtered
queries have no remaining gaps there. The comparison has no source/build
receipts or test attribution.
The JPEG row `baseline_444_multiscan_duplicate_dqt_after_snapshot` duplicates
the 69-byte DQT marker between the first two component scans (input SHA-256
`a73504634df562811870132182af22730318fa2f4416347770499aa412c71e56`). Pillow
12.2.0 returns the 8×8 RGB reference (192 bytes, SHA-256
`983025493b251f11b379bbbf15a92cde4e5b9177e63626ff035e0049cc6779b4`), and the
selected public JPEG row passes. Coverage MCP reports one new coordinate at
`jpeg/decode/parser.rs:431`, with no remaining filtered gap. This comparison
also lacks source/build receipts and test attribution.
The JPEG row `baseline_444_multiscan_duplicate_dht_after_snapshot` repeats the
first DHT marker twice between the first two component scans (input SHA-256
`60ce19025b6c5e78ff18659ffea26dc97a0c628bb29dbc9c78c4d653c8c708f6`). Pillow
12.2.0 returns the same 8×8 RGB pixels as the three-scan source (192 bytes,
SHA-256 `0d4dccc69dc0ee4ae956be8c624186ddb2b5f3d411671361056c51a7ea7f9e66`),
and the selected public JPEG row passes. Comparison with the saved report before
this row shows three new branch coordinates at `jpeg/decode/parser.rs:442`;
the current filtered query has no gap there. This remains coordinate-only
evidence without source/build receipts or test attribution.
The TIFF parity matrix adds `tiled_gray2_deflate_short_tile` (input SHA-256
`1fb7f7d7d6a281cb667c365fe097bc868d841ec246783bd0b5014f4417127d8`) and
`tiled_palette4_oob_tile` (input SHA-256
`175f8b4e1c2df6e59d8a758892b8430ddc03cf23c24e26db752faff754435cb6`). The
first has a valid Deflate stream that expands to a truncated packed tile; the
second has a tile offset outside the file. Pinned Pillow opens both headers and
rejects pixel loading with decoder/truncation errors, and both active public Rust
decode rows pass the expected malformed-input parity. The full all-feature
coverage campaign passes with these rows in the matrix.

The `tiled_deflate_no_predictor` row adds a 128×128 RGB TIFF using Deflate tiles
without a Predictor tag (input SHA-256
`fddee4f848bfcde2bf9f8064016278ba0d8a755d7f0fc3598c756755e2399f73`). Pinned
Pillow 12.2.0 produces the exact 49,152-byte RGB reference with SHA-256
`8a1d6fcc36f5b5e70fbf949d4e612630a1931279383e7c21db27cd3cbad98131`; its active
public matrix row passes. The full local LLVM report records 64 false-arm hits at
`decode_ifd:454`, and the filtered Coverage MCP query no longer lists that
predictor branch.

The packed-tile helpers no longer repeat bounds and sample-width checks already
guaranteed by their callers; checked bit-offset arithmetic remains. Strict Clippy
and the full all-feature suite pass. Removing the two duplicated layout guards
reduced the local branch inventory from 32,264 to 32,254 arcs. Filtered MCP
queries no longer report gaps at `decode.rs:290`, `:454`, or `:1141`; they still
show one header-magic error arm and two LZW output-length guard outcomes. The
active `error_bad_ifd_lzw_invalid_first` row emits code 258 and exercises the
separate `code >= CLEAR` arm in both ordinary and token-aware public decode paths.
The remaining `output.len() >= expected` operands protect an internal state
which public TIFF dimensions and capped output writes do not allow. The header
magic arm is also unreachable through normal public dispatch: `detect_format`
recognizes only signatures whose magic `parse_header` considers registered.
These MCP locations remain coordinate evidence because source/build receipts
and test attribution are unavailable.

On this report, the filtered branch query
shows no missing branch groups in
`src/codecs/jpeg/encode/encode_baseline_422_block_row_streaming`. The selected
`enc_sub_422_streaming_exif_restart_rows` public parity row passes and its
selected report observes the true EXIF arm at `jpeg/encode/mod.rs:1958`; the
opposite arm remains unobserved in that selection. The report lacks
source/build receipts, so it does not bind the observed arm to a source
revision or prove test attribution.

The JPEG decode matrix now has 161 active rows. The new
`baseline_cmyk_aligned_entropy_truncated_tail_64` case uses a 32×8 MCU-aligned
CMYK image with 64 entropy bytes removed before EOI (input SHA-256
`51f4f76d8a6144572eb0880c5722f290840db23c6fa5a83148ac1a1947d7445c`). Its
exact 1,024-byte Pillow output has SHA-256
`89b4bd202579d6c4c3497dc124d1d45795ffb3dd74bb25a024a40d66f094c718`. Public
parity passes, and the selected report observes the incomplete-data arm at
`jpeg/decode/decode.rs:916`; the full Coverage MCP query lists no remaining gap
there. MCP source/build receipts are absent and test attribution is unknown,
so those findings remain coordinate evidence.

The `progressive_dqt_redefined_between_scans` JPEG row inserts an identical
DQT segment after the first DC scan of the 128×128 progressive fixture. The
5,579-byte input has SHA-256
`353d4f1b20a5b73e473deef0d401dd91a9d2973c6065b0dfddd005edf21785da`; its exact
49,152-byte Pillow output has SHA-256
`e92abdc1f9d14fd2491920de98bdcee028788715bebc6ba5d21b20d89b59fd76`. The
public JPEG decode row passes. Local full LLVM coverage now records three hits
for the previously unseen `!progressive` false outcome at
`jpeg/decode/parser.rs:431`; the filtered MCP query no longer lists that span,
though a later conjunction outcome on the same line remains uncovered. MCP
source/build receipts are absent and test attribution is unknown.

The `animated_lossless_inter_420_b8x16` AVIF row adds a two-frame 8×16 I420
sequence whose public decode selects a lossless B8×16 inter transform. Its
1,152-byte input SHA-256 is
`9e3fbac5e42c61413ec8c1f7f4f72bc1338fb7cfd21587e30826855732e369c0`; exact
384-byte frame references have SHA-256
`201fc960015c0e0974cb9b6aab74c9147694ddfd7876da531b696fc3639257e6` and
`aebfeaa0e20ab2e7aad44ca321ed0bdebaea085045a02954e55eb63505342659`. Both
frames have 100 ms durations, and the pinned native loop oracle records
infinite repetition. The selected public parity run and refreshed full local
report hit the `lossless_rect` and exact B8×16 outcomes at
`av1/block.rs:56172`. Coverage MCP still lists sibling outcomes and other
decoder instantiations at that line; source/build receipt and named test
attribution are unavailable.

The active `enc_animated_rgba_transparency_composite` row decodes a two-page
RGBA TIFF (input SHA-256
`445c160373a2391d3e090179cf9e5b3959382c1753b9ed762b0a653ec81bf5e6`) and
encodes both frames through the public GIF sequence API. Its 90-byte Pillow
reference has SHA-256
`e6fbf77ce827bd01c711a70ea9b48468b3ddf10dcca7bc9635e96eceeee63a7f`. The
focused GIF matrix test passes, and the fresh filtered Coverage MCP result no
longer lists the false arc in `composite_image` at `encode.rs:710`. Coverage
MCP's report remains coordinate-only because source/build receipts are absent
and test attribution is unknown.

The active `enc_error_oversized_dimensions` BMP row supplies a valid packed L1
image of width 2,147,483,648 (268,435,456 bytes) through the public encoder
matrix. Pillow 12.2.0 raises `OverflowError: signed integer is greater than
maximum`; the focused BMP matrix and full coverage campaign pass. The fresh
filtered Coverage MCP query no longer lists the width-failure arm in
`bmp_file_fits`. The height-failure arm remains unobserved: a valid 1×2,147,483,648
packed L1 image requires a 2 GiB pixel buffer, so the matrix retains the
existing Pillow error row without constructing that buffer. Coverage MCP has
no source/build receipt and unknown test attribution, so these locations are
coordinate evidence.

The active `enc_sub_420_q10_small_image_threshold` JPEG row encodes the
existing 17×17 RGB input (source SHA-256
`cb1632ddf154ea044863205d6b6f6e8249c70791a044debdc2b80b8fae92622e`) with
quality 10, baseline 4:2:0 subsampling, and optimization disabled. Pillow 12.2.0
produces a 690-byte reference (SHA-256
`ff93ff388fa786b630cff562f4052d31de74e9f4d2cdd076b1377f4cf83a8cba`). The
focused public matrix case passes exact byte parity. Its selected report is
`target/release-evidence/coverage-jpeg-q10-sub420-small-20261003.json`; the
full-report query at `jpeg/encode/mod.rs:713` now has no missing branch group.
The selected report observes the small-image false arm, but the MCP source and
test receipts remain unverified/unknown.

The packed FDCT encoder now multiplies all four rounded coefficient numerators
by their shared reciprocal with safe `u32x4::widening_mul`. Both the packed and
scalar paths apply the exact possible one-step correction by subtracting the
boolean as 0 or 1. The full public JPEG encode matrix passes exact Pillow byte
parity. Coverage MCP's filtered full-report queries list no missing branch
groups at `jpeg/encode/mod.rs:3042` or `:3212`; the report still has no
source/build receipt and unknown test attribution. Three five-round public
benchmark runs measured the 512×512 RGB quality-10 4:2:0 case 7.6–8.1% faster
than the scalar baseline, with identical encoded bytes and hashes. The
full-matrix geometric mean stayed within 1% of baseline, so this is a targeted
low-quality workload gain rather than a general JPEG speedup. Artifacts are in
`target/benchmarks/jpeg/arm64-goal-quantize-simd-{baseline-20261003-b,candidate-20261003-a,candidate-20261003-b,branchless-20261003-c}/`.

Coverage MCP still lists the true checkpoint arms at
`src/codecs/webp/native/encoder.rs:1378` and `:1418` as missing in this full
report. The experimental large lossless input did not observe either arm and
failed exact Pillow parity, so it is not a retained matrix case.
The preceding full report's filtered queries found no remaining branch gaps at
`container.rs:608` or `:622`, or at `jpeg/encode/mod.rs:2201`, `:2241`, and
`:2283`. An earlier filtered query on
`coverage-after-gif-parity-20261003.json` reported no remaining branch gaps in
the WebP grayscale/alpha scan. The grayscale RGBA input has 1,024 opaque
pixels before varying alpha, so its token-aware scan crosses one checkpoint
before selecting the per-tile predictor; alpha classification now shares that
grayscale traversal. Two 32×32 partial-alpha RGBA rows exercise the red/green
and green/blue early exits. Both match Pillow byte-for-byte at 942 and 954
bytes, respectively, with ordinary and token-aware public encoding. The opaque
grayscale shortcut remains available for fully opaque input.

The active `decode:gif:valid_frame_missing_trailer` row exercises Pillow's
acceptance of a complete one-frame GIF that ends at a top-level block boundary
without a trailer. Its ordinary and metadata-limited public decode paths match
Pillow's 1×1 palette image. The focused GIF matrix passes 80/80, strict lint
passes, and the full coverage run above passes. Coverage MCP's filtered branch
query reports no missing branch groups in `src/codecs/gif/inspect.rs`; the
report lacks source/build receipts, so this is coordinate evidence only.

Coverage MCP also lists AV1 `decode_following_vertical` at `block.rs:67554`,
but the `full_strict` outcome is unreachable through today's public decoder:
all four callers use `Lossy420Decoder` constructors fixed to 4:2:0. The true
arm at `block.rs:21111` is ordinary nonzero-plane decoding, already represented
by active I444 residual rows. Neither coordinate provides a distinct public
parity case; the report remains unverified coordinate evidence.
## Selected parity and coverage — 2026-10-03

The JPEG public decode matrix passes 174/174 active rows, including valid cases
where Cb and Cr use different sampling factors. Those cases exposed a panic in
the generic reconstruction path: it cropped Cr using Cb's plane dimensions.
Reconstruction now crops and upsamples each chroma plane from its own sampling
factors. The existing baseline 4:2:0, 4:2:2, 4:4:4, grayscale, and CMYK direct
decoders still return before this generic path.

The four selected `baseline_420_chroma_component_sampling_fallback` rows pass
4/4. Their LLVM report is
`target/release-evidence/coverage-jpeg-chroma-component-sampling-all-variants-20261003.json`.
Coverage MCP's older full report showed missing true arms for the Cb/Cr
horizontal and vertical sampling guards at `decode.rs:1170-1173`; the selected
report now observes those true arms. The false arm at line 1173 remains
uncovered in that selected run. This is coordinate evidence from reports with
no source/build receipts; MCP lists test attribution as unknown.

The selected 4:2:2 sampling, CMYK sampling, and restart rows pass 7/7. Their
LLVM report is
`target/release-evidence/coverage-jpeg-sampling-fallback-guards-20261003.json`.
Coverage MCP shows the selected 4:2:2 cases reach the component sampling
fallback guards at `decode.rs:1430-1433` and the CMYK cases reach the
non-1x1 sampling guard at lines 962-965. The report does not observe every
opposite arm in this selection; source/build receipts are absent and MCP lists
test attribution as unknown.

The AVIF `ipma_wide_property_associations` row passes its selected public parity
case (1/1). The selected LLVM report is
`target/release-evidence/coverage-avif-wide-associations-20261003.json`.
Coverage MCP reports 2,142/32,164 branches observed in that selected test and
leaves the false arm of `parse_ipma` at `container.rs:1077` uncovered. The
earlier full report listed the true arm at that coordinate as missing. This is
consistent with the selected row exercising the wide-association arm. Both
reports lack source/build receipts and MCP lists test attribution as unknown;
the incremental comparison is incomparable, so no combined coverage gain is
claimed.

The active `compression_lzw_lzw_long_phrase_checkpoint_1024` TIFF row adds a
Pillow-exact 525,825-pixel grayscale stream whose growing LZW phrase reaches
1,025 bytes and crosses the token-aware output checkpoint. Its input SHA-256 is
`0e85b9b6a8f064755ce9f1934d838c5d64c2e8fb5b453a2fe072ba347f2f0`; the Pillow
pixel reference SHA-256 is
`e32ad4269a71ed42a3d18578f1e3cbd8ec3a0b026b8215287483b0d47fa9e7bd`. The
selected TIFF matrix suite and full coverage run pass. Coverage MCP's filtered
query reports no remaining branch group in `append_lzw_with_token`; its
coordinate comparison observes the checkpoint's true arm once. The report has
no source/build receipt and test attribution is unknown, so this remains local
coordinate evidence rather than a managed Coverage MCP claim.

Coverage MCP still reports eight uncovered branch observations in WebP's main
`optimize_sampling` and one in its cross-color sampler, plus two observations
each in GIF's `split_median_box` and token-aware variant. A 16,320×576
token-aware WebP candidate reached 641 row comparisons but still missed the
main sampler's row-checkpoint true arm and column pass; it encoded to 222,328
Rust bytes versus 222,334 Pillow bytes, so it remains outside the parity
manifest. This report has no source or build receipt and test attribution is
unknown, so Coverage MCP comparisons are source-coordinate diagnostics rather
than verified coverage gains. For the GIF misses, a 2×1 two-color Pillow
control reaches the equal-value fallback, but public-path branch counters
confirm that the initial split loop's exhausted condition and the fallback's
`split > 0` false arm remain unobserved. With positive, nonsaturated per-color
counts, the midpoint loop breaks on the last color and the fallback stops at a
prior distinct color; no public fixture witness for either false arm was found.

The same full report has a PNG chunk-iterator stop observation. Its raw LLVM
branch table identifies the missing arm as `Chunks::failed`; the position-at-EOF
arm is already exercised by the active missing-IEND input. Every public chunk
consumer propagates a chunk error immediately, so none requests another item
after `failed` becomes true. TIFF's packed-tile extent checks follow validated
tile geometry and bounded source rows. BMP signature guards and ICO size guards
are preempted by public dispatch/source validation; reaching the BMP file-size
limit with a valid image would require impractically large pixel storage. These
remain defensive branches without valid public Pillow parity witnesses, rather
than reasons to add private coverage-only tests.

Coverage MCP's TIFF LZW gap at `decode_lzw:1888` is the right side of the
`||` check (`output.len() >= expected`). The active
`error_bad_ifd_lzw_invalid_first` row passes public parity but returns on the
left side (`code >= CLEAR`) before evaluating it. Successful output writes
return as soon as they reach `expected`, so the right-side true outcome remains
a defensive state without a valid public input witness.

The new `tiled_rgb1_packed_multisample_rejected` row preserves Pillow's
malformed-error parity for packed 1-bit RGB tiles. It does not reach the
`decode_ifd` tile-sample guard at `decode.rs:289-291`: `layout_and_palette`
rejects RGB below 8 bits earlier, and every currently supported packed layout
uses one stored sample. Its selected LLVM report records no branches at that
guard, so the row is an error-parity edge rather than a coverage gain there. A
fresh selected run passes 1/1 and is recorded at
`target/release-evidence/coverage-tiff-packed-tile-control-20261003.json`;
Coverage MCP likewise reports no observations at that guard or in
`copy_packed_tile_into`/`copy_packed_tile_row`. The report is selected-test
evidence with no source/build receipt and unknown test attribution.

Coverage MCP's remaining JPEG encoder groups include zero-dimension and
unsupported-sampling guards in `downsample_without_checkpoint`,
`downsample_two_to_one`, and `downsample_with_checkpoint`. Public image
validation rejects zero dimensions, and `JpegSubsampling` exposes only 4:4:4,
4:2:2, and 4:2:0; no-token 4:2:0 uses a separate pre-downsampled path. The
`encode_six_coefficient_major_edge_raw_blocks` helper also receives chroma
presence flags set to true at both public call sites, while its absent-luma
edge makes the uncovered `present[3]` outcome unreachable. Existing exact
parity rows cover the supported sampling and clipped-edge paths.

The JPEG direct-decoder bailout audit found a valid baseline multi-scan edge:
Pillow accepts a 4:4:4 image that encodes Cb, Y, and Cr in separate
non-interleaved sequential scans. The active
`baseline_444_three_sequential_scans` fixture is generated by splitting a
single 8×8 interleaved Pillow JPEG. The resulting file has SHA-256
`b9e3e277c0d05b2df169804236d5f9b72d3a1c8beda33e845f9c6ffd436f424b`, and its
decoded pixels match the original Pillow JPEG exactly (pixel SHA-256
`0d4dccc69dc0ee4ae956be8c624186ddb2b5f3d411671361056c51a7ea7f9e66`). The
active `baseline_444_three_scans_dqt_redefined` variant inserts a new
8-bit Cb quantization table between the first and second scans. Pillow's
decoded pixels differ from the original when the final table changes (input
SHA-256 `74d02103e151831846019cbcf009f2d17912988045e8d82e850828e18d1724f7`,
pixel SHA-256
`983025493b251f11b379bbbf15a92cde4e5b9177e63626ff035e0049cc6779b4`), so
each component latches its quantization table at its first scan; scans retain
their SOS Huffman and restart settings. The decoder handles non-interleaved
block geometry and resets DC predictors at scan and restart boundaries.
Single-scan baseline decoding keeps its existing dispatch and fast paths, and
the parser defers per-scan table copies until a later scan or table update
requires them.

Coverage MCP surfaced three additional Pillow-tolerated SOF0 cases. The
`baseline_444_multiscan_ss_nonzero` row sets Ss to 1 in the first sequential
scan; Pillow returns the same pixels as the ordinary three-scan input (input
SHA-256 `7a36ce48ba3d15bcefb15ad70b1a5e3f0c70523c84d2cdadade2ed6531e0094d`, pixel SHA-256
`0d4dccc69dc0ee4ae956be8c624186ddb2b5f3d411671361056c51a7ea7f9e66`).
`baseline_444_multiscan_duplicate_component` changes the final scan's component
ID from Cr to the already-seen Cb (input SHA-256
`76b4d1d36238ff9f92d63a44bb6dc1bcde040e99e0217f15b1c96f4a624668cc`, pixel
SHA-256 `2b73ccd3b62ddd2c645e63cd0d65ed016fc25a461d82e161514ac0d48fb2fb52`).
Pillow maps that scan back to Cb and leaves the omitted Cr plane neutral. Its
coefficient buffer retains earlier AC values where a repeated sequential scan
encodes zeros, so the Rust path stores coefficient history only for components
that repeat and merges the later block coefficients. This follows libjpeg-turbo's
multiscan coefficient buffers and Huffman decoder ([`jdcoefct.c`](https://github.com/libjpeg-turbo/libjpeg-turbo/blob/main/src/jdcoefct.c),
[`jdhuff.c`](https://github.com/libjpeg-turbo/libjpeg-turbo/blob/main/src/jdhuff.c)).
The `baseline_444_multiscan_missing_component` row omits the final Cr scan and
leaves its neutral plane initialized (input SHA-256
`d91a46924a2310aae6a9b7c16fd7f091eaf21bff8a8b74dc99c388d1983e6a8a`, pixel
SHA-256 `669b0f32cd296cd0742d7e9b8e9c25cf7a4f114b6a5788f83af5ed0e89f409b7`).
The `baseline_444_multiscan_empty_entropy` row removes the first component
scan's entropy while retaining its SOS; Pillow accepts the file and leaves that
component neutral (input SHA-256
`f8bba1d8f8e8f631f5487349550f64a3bd3438a9789ed293619d7e2553be6192`, pixel
SHA-256 `1b343e02a958d0137fd239618dd0332265457fc011296714fe1a42cf0e35d49d`).
`baseline_444_multiscan_interleaved_restart` combines Y and Cb in one scan,
leaves Cr in a singleton scan, and places a restart marker between its two
MCUs (input SHA-256
`ddbcf29eb1549aae0356661ff2b879b7b2b23b7aae924adbb424af22246e20d7`, pixel
SHA-256 `f83545d43c6939ec393b6b8310959b6174fd764b08a12fc22d908408a7e6a43e`).
The `baseline_444_multiscan_restart_overrun` variant adds two trailing restart
markers; the first creates an empty segment after the final MCU. Pillow accepts
it and returns the same pixels (input SHA-256
`1aac34523a7e6491744764d84c9c0de21ef9a308d532924e121c7c36229defe6`, pixel
SHA-256 `f83545d43c6939ec393b6b8310959b6174fd764b08a12fc22d908408a7e6a43e`).
`baseline_444_multiscan_duplicate_component_dc_only` repeats Cb with blocks
containing only DC coefficients, reaching the repeated-component DC-only path
(input SHA-256
`3a316bfffa1834975e775d7a4dd62f0766919eef85bdd5a44f0401f12f17f429`, pixel
SHA-256 `fa7b78cc215df21d7ce54d8c3c6637c326dab95c10fbc12263101365973f4268`).
All seven tolerance cases match Pillow. The JPEG decode matrix passes 188/188.
The active `entropy_all_ones_huffman` row is a grayscale baseline JPEG whose
SOS prefix is followed by two stuffed all-ones entropy bytes and EOI. Pillow
12.2.0 decodes it; the input SHA-256 is
`bef219c962289debcec499023a86acf162a5e51c0ccd78309162f845a2544456`, and the
exact 128×128 L8 reference SHA-256 is
`f9db8abaac585e058a892932da246a2cd05aec42b862dd931ffee9891577c4de`. The
fixture reaches the fast decoder's overlong-code sentinel with length 17.
Coverage MCP no longer lists the overlong-length guards in either the scalar
or fast decoder in the current full report. The active
`empty_dc_huffman_table` row starts from `sampling_3x1.jpg` and replaces DC0's
DHT entry with a zero-symbol table. Pillow 12.2.0 decodes the resulting 617-byte
input (SHA-256
`c28fa453e06196681ad2815bfe6dd320ee2239e19d9efba3999d0bdc9c5c8d14`) to a
24×8 RGB image (pixel SHA-256
`65f0bcf872b036301186859680a42be71467f6fd402c47ab2cc20c68e10dee44`). Rust
now follows IJG's fake-zero recovery for scalar and fast overlong codes while
preserving the error for its internal invalid-table sentinel. The empty-table
fixture reaches scalar `decode_slow` at length 17. Remaining filtered gaps are
fast `ensure` true at `huffman.rs:373`, fast `values.is_empty()` true at line
390, scalar `ensure` true at line 329, the invalid synthetic sentinel at line
349, and the missing-symbol arm at line 360. MCP reports source/build identity
as unverified and test attribution as unknown.
Coverage MCP reported remaining unobserved branches in
`reconstruct_baseline_multi_scan`; its local report has no source/build receipt
and unknown test attribution, so it is coordinate-level guidance rather than a
managed coverage claim. A header-only SOS component-ID swap remains rejected by
Pillow as a broken data stream; no parity case is claimed for that mutation.
The final full `make coverage` run passes all tests and repository floors:
86,169/142,520 lines (60.4610%), 16,492/32,268 branches (51.1095%),
4,810/8,989 functions (53.5098%), and 131,806/220,404 regions (59.8020%).
Coverage MCP now reports one branch at `decode.rs:2268`: an empty extracted
segment set with nonempty scan bounds. The JPEG parser ends each entropy range
at the first non-stuffed, non-restart marker, so this error arm appears
unreachable through manifest-visible inputs. The LLVM report has no source or
test receipt; MCP marks both as unverified/unknown.

Coverage MCP also pointed to a valid baseline JPEG with a DQT marker after its
only SOS entropy scan and before EOI. Pillow 12.2.0 accepts the full 8×8 4:4:4
input and decodes it identically to the source without the trailing marker
(input SHA-256
`1b4519e48e1ce711cd1cbb44c3173ef3c97f222b8aa1686f9437df5e5f906372`, pixel
SHA-256 `7382706a9ecb0f6acae26e7c1c9a9be0e9471233c1a0c0da48d08a458115be2f`).
The public `baseline_444_single_scan_trailing_dqt` row exercises this case.
Single-scan reconstruction now stops entropy extraction at the scan boundary
and uses the quantization-table snapshot captured before the marker. Pillow
also accepts an empty trailing DQT (input SHA-256
`bedd3492ec593158787385c58ec04055ecedd0beb334d1dbcae55716cb3ed9a3`) and
marker fill before a nonempty trailing DQT (input SHA-256
`6f4e22755f714d4fe3dab67e8d9e0154abc33cd80714ba4cd7ba4792abfa18e4`); both
produce the same pixel hash above. The public
`baseline_444_single_scan_empty_trailing_dqt` and
`baseline_444_single_scan_dqt_marker_fill` rows cover those forms. The code
keeps the existing single-scan dispatch and fast paths. The focused public rows
and full JPEG decode matrix pass; these are previously unsupported valid
inputs.

Coverage MCP identified the false arm of the second comparison at
`src/codecs/jpeg/decode/huffman.rs:410`: a valid AC DHT has the standard luma
code counts but a different symbol order. The public
`baseline_444_unused_custom_ac_table` fixture inserts an unused AC table 2 into
the complete `baseline_444.jpg` input, swapping the first two symbols assigned
to one code length. Its generator pins the source and generated-input SHA-256
values and verifies Pillow 12.2.0's decoded RGB SHA-256. Pillow accepts the
9,329-byte input as 128×128 RGB and preserves the baseline pixels exactly;
Rust matches all 49,152 Pillow bytes. The selected public matrix row passes
1/1. Its LLVM branch report changes the targeted arm from zero observations
for `decode:jpeg:subsampling_444` to four observations for the new row. The
Coverage MCP selected-test comparison reports the line-410 arm as newly
observed, but remains limited because source/build receipts are absent. The
selected report is `target/jpeg-coverage-probe/coverage-public-row.json`; this
is coordinate-level evidence, not a full-suite coverage claim.

## JPEG direct-safe SOF component-count invariant — 2026-10-04

Coverage MCP also showed missing true arms for `components.len() != N` in the
grayscale, CMYK, 4:2:0, 4:2:2, and 4:4:4 baseline direct-safe paths. Those arms
are unreachable from public JPEG inputs: `parse_sof0` builds the component
vector from the SOF count, the parser derives `num_components` from that vector,
and its sole `JpegInfo` construction stores both values together. The five
redundant hot-path checks were removed and the invariant is documented on
`JpegInfo`; no private unit tests or coverage exclusions were added. The public
JPEG decode matrix passes 203/203 rows. The selected LLVM report is
`target/release-evidence/coverage-jpeg-direct-safe-guards-20261004.json`; MCP
has no source/build receipts, so it is row-local evidence and does not support
a verified aggregate coverage-gain claim.

Coverage MCP also lists repeated AV1 specialization observations in
`decode_inter_translation_impl` and `decode_following_vertical`; specialization
counts are not distinct source branches. A selected run of the clipped
`animated_lossless_inter_420_clipped_b32x32_64x60` row passes public parity and
reaches the true outcome at lines 54836–54838 in one of eight compiled
`decode_inter_translation_impl` specializations. Its report has no source/build
receipts, so retain this as coordinate evidence.

The 32×32 `animated_lossless_inter_420_b32x32_10bit` row records no branch
counts at lines 54836–54845 across eight compiled specializations. A separate
selected run of
`animated_lossless_inter_420_b32x32_10bit_64x64` passes public parity and
records six false outcomes at `block.rs:54838` (`tools.sample_depth ==
SampleDepth::EIGHT`) in one specialization, with the preceding 32×32
`LosslessGrid` match reached. This existing full-size case already witnesses
the sample-depth false arm. The 28×28 10-bit clipped fixture uses B8×8 leaves;
high-depth clipped B32 geometry is rejected by the current planner before
this block evaluator, so it does not need a new fixture to cover the false arm.
The selected 64×64 coverage report is
`target/release-evidence/coverage-avif-10bit-64x64-target-20261004.json`.
Coverage MCP still lists other missing specialization observations, and its
incremental comparison with the full baseline is incomparable because exact
baseline branch detail is incomplete; this does not claim a marginal full-suite
coverage increase.

`decode_following_vertical` has no observed calls in this report; its
production caller requires a lossy 4:2:0 16×16 split in a closed frame
context. A 16×16 q76 noisy-quadrant AVIF probe decoded in Pillow but returned
`NotImplemented` from the Rust public decoder, so it was not added. No valid
Pillow-parity witness for this remaining gap was established. The reports have
no source/build receipts; Coverage MCP marks source unverified and test
attribution unknown, so these are source-coordinate diagnostics only.

Coverage MCP reports no remaining branch gap in `decode_portable` at lines 492,
496, 535, 555, 588, and 606 after adding 10-bit alpha 4:2:0, 4:2:2, and 4:4:4
public parity rows. It reports no remaining gap at
`webp::inspect::validate_animation_frame` line 311 or in the token-aware GIF
coalescing group. These are source-coordinate diagnostics; the report has no
source/build receipt and marks test status unknown.

Seven public limited-range monochrome parity rows now cover RGB and auxiliary
alpha output at 8, 10, and 12 bits, plus BT.2020/PQ CICP. Their selected rows
match exact Pillow output and the full AVIF decode matrix passes. The CICP row
shows monochrome admission no longer needs a 1/13/6 color declaration. Coverage
MCP reports no branch gaps in `limited_luma_to_u8`; remaining AVIF groups are
plane-shape guards at `decode_portable` lines 457–463 and
`decode_monochrome_portable` lines 676–684, plus the false
`subsampling_x/subsampling_y` check at line 410 of
`monochrome_primary_sequence_supported`. The shape guards follow validated
plane assembly, and AV1 monochrome sequences have fixed 4:0:0 subsampling, so
these arms have no known public encoded-input witness. The report is
unverified for source/build identity and test attribution.

The active `enc_rgba_octree` and `enc_rgba_octree_sorted` GIF rows now also
call the public `encode_with_token` API on their existing RGBA inputs. Each
token-aware result is compared byte-for-byte with both the exact Pillow
reference and ordinary Rust encoding. Both selected parity rows passed, and
Coverage MCP reports no remaining branch gaps in
`gif::encode::apple_qsort_buckets_with_token` in the refreshed full report.

The active `animated_lossless_inter_420_clipped_b32x32_185x64` AVIF case adds
a 185×64 two-frame, lossless 4:2:0 B32 edge input. Its final block has 25
visible columns; the input is pinned by SHA-256
`701d247ebbe2666b1f28f447fa1a086573e7dc6d88b0b29405f9f7a6f9460b59`, and its
two Pillow RGB frames are pinned by SHA-256
`ac76c404cf3001f4e087307516e596223e3e5f1c26d7eaf8860bc710b88761a1` and
`336f948700529f81199b9d8fb0adcc8da79dcc00b06ffffa5b66d5dee255199e`. The
selected parity row and full AVIF matrix pass. This edge does not make the
AV1 partial-grid false arm observable: the relevant decoded extent is MI
padded to a multiple of four.

The active `animated_lossless_inter_420_clipped_b32x32_64x60` AVIF row adds a
two-frame bottom-edge crop case with exact Pillow frame parity. Its selected
LLVM run reports `selected=1 active=1` and passes the row. The report records 11
calls to `inter_lossless_grid_clipped_b32_geometry_supported`; one B32x32 call
passes the 32-pixel width check and takes the 28-pixel visible-height arm.
`decode_inter_transform_size` runs 11 times, but the later clipped-plan arm at
line 6927 remains unobserved in this selected run. Coverage MCP marks the
report's source unverified and test attribution unknown, so these local
counters are scoped diagnostics, not managed coverage attribution.

Two active TIFF parity inputs set the separate-planar RGB tile width or height
to zero. Pillow opens and verifies both headers, then raises
`ValueError: tile cannot extend outside image` while decoding pixels. Their
input SHA-256 values are `f8265499d2ee3d4e63e9525a76e70aa88cd9840891267c47aaa840928056617a`
and `1dc16a0bd0c6b6574ae38c9ed163bf931e343736e41a41f2d81a24331bd08cc0`. The
public parity run matches all four lifecycle results. These inputs cover both
zero-dimension exits in `verify_separate_planar_offset_count`; the remaining
mismatched-tag operand was removed because the preceding directory check
rejects that state before this helper runs. Coverage MCP's filtered query now
returns no branch gaps for the helper, with the same source/build and test
attribution caveat noted above.

The JPEG encode matrix now has 74 passing active rows. The new
`enc_sub_444_unaligned_subthreshold` and
`enc_sub_444_aligned_subthreshold` rows pin exact Pillow bytes for 4:4:4
inputs below the row-streaming size threshold; together they exercise the
three-component independent-entropy predicate and the dimension-alignment
alternative. The token-aware GIF LZW and BMP raw-pixel paths also run through
public parity: GIF passes 79/79 decode rows and BMP passes 106/106. The
selected report `target/release-evidence/coverage-public-parity-edges-20261003.json`
records the JPEG three-component arm and both token-aware BMP read paths. Its
aggregate covers only the four selected rows, and Coverage MCP reports no
source/build receipt and unknown test attribution. A seven-row token-aware GIF
selection passes 7/7, covering the existing patterned frame, four LZW boundary
cases, and invalid minimum code sizes on both sides of the accepted range. Its
report is
`target/release-evidence/coverage-public-gif-token-lzw-edges-20261003.json`;
Coverage MCP reports no remaining branch observations at the invalid-size
guard in `gif/decode.rs:556` or the phrase traversal in
`append_code_with_token` at line 695 for this selection. The full GIF decode
matrix passes 79/79. This report also has unverified source/build identity and
unknown test attribution, so it supports a selected-path observation rather
than a full-suite coverage claim.

The VP8L structural inspector now follows nested `VP8L` chunks in `ANMF`
frames when no top-level bitstream exists. The property map records the nested
16×16 frame header and color-indexing transform for
`pillow_tolerated_malformed_animated_vp8l_anmf_width_mismatch`; the malformed
outer width remains covered by Pillow parity, not by the VP8L parser.

## Public parity coverage edges — 2026-10-02

Active Pillow parity cases target codec edges surfaced by Coverage MCP.
The WebP row `error_malformed_container_animated_frame_bottom_outside`
complements the existing horizontal frame-overflow case by moving the ANMF top
offset down two pixels. Pillow 12.2.0 reports
`OSError: could not create decoder object`, and the selected public parity row
passes. The isolated LLVM report
`target/release-evidence/webp-frame-bottom-outside-isolated.json` records four
false outcomes for the horizontal operand and four true outcomes for the
vertical operand at `webp/inspect.rs:311`. This is selected local evidence only;
it has no managed Coverage MCP source/build receipt or test attribution, so it
does not establish full-suite coverage.

The `fdat_without_actl_before_idat` PNG case uses a valid-CRC `fdAT` chunk
before `IDAT` with no `acTL`; Pillow rejects it during inspection, and Rust now
rejects it in the independent PNG inspector as well as the decoder. Its input
is pinned by SHA-256
`6d1a1d4a5fb1e4185b39fe44bd13e489d9a58680b3a5f7490ffcad4854148c4d`. The
`apng_truncated_fdat_before_idat` case puts a three-byte `fdAT` after valid
`acTL` and `fcTL` chunks but before `IDAT`; Pillow raises
`ValueError: APNG contains truncated fDAT chunk`. Its CRC-correct fixture is
pinned by SHA-256
`4f34e6caa28789baa349f5b3ada75f0ad7aae29f617717da9dffd6f906d2312b`. The
PNG parity matrix passes all 175 active rows, including both malformed cases
and the existing valid sequence-only APNG case.

An earlier all-feature `make coverage` run passed every suite and repository
floor with
`target/release-evidence/coverage-full-webp-tiff-jpeg-20261002.json`:
85,519/142,085 lines (60.1886%), 16,254/32,178 branches (50.5128%),
4,788/8,972 functions (53.3660%), and 130,592/219,584 regions (59.4725%).
The PNG parity harness decodes each successful row through `decode_with_token`
and compares its exact pixels with Pillow. The existing WebP `enc_exif`,
`enc_xmp`, `enc_icc`, and `enc_lossy_alpha_exif` cases now also run through
`encode_with_token`; the harness checks exact bytes against Pillow and the
ordinary Rust encoder. The WebP decode parity lane passes all 230 active rows.
Four complete VP8X mismatch files exercise width and height rejection for both
VP8 and VP8L, including `inspect_basic`. The three new files derive from
valid generated WebP inputs by changing only one 24-bit VP8X canvas dimension;
Pillow reports `OSError: could not create decoder object`, and Rust reports a
structured malformed-image error. The VP8L decoder now checks its signature,
dimensions, and version in the frame header and returns typed errors rather
than panicking on malformed input. These checks run once before pixel
reconstruction; the pixel loops and SIMD kernels are unchanged. Coverage MCP's
filtered branch queries no longer list missing branches in
`inspect_extended_basic` or `LosslessDecoder::read_frame_header`. Other groups
remain in WebP inspection and lossless decoding. It still
reports the false arm of `encoded.len() >= 30` at
`src/codecs/webp/encode/mod.rs:912`; valid public output already contains the
12-byte RIFF header and 18-byte VP8X chunk, so that shorter-buffer arm is
unreachable there. The AVIF query still reports missing branch observations at
`src/codecs/avif/av1/block.rs:54833`; the 10-bit fixture validates its sample
but does not complete sequence surface reconstruction. These Coverage MCP
queries are diagnostic: the full report has no source/build receipt and MCP
reports test attribution as unknown. The 100% coverage goal remains open.

The new `lossless_copy_out_of_bounds` WebP decode row reaches the lossless
copy-length bounds rejection: a singleton VP8L copy asks for 25 pixels when
only 16 remain. Pillow reports `OSError: failed to read next frame`; the Rust
decoder matches the malformed-input outcome. Its 8×4 fixture is pinned by
SHA-256 `abb0a3d2b630b23fc5bdd476bb59e9b7790a461ae60522ddc282fd53b27ef075`.
`enc_wide_scanline_checkpoint` uses a 1,025×1 RGB BMP row to reach the
post-pixel checkpoint at index 1,024 in `bmp::encode::write_rows`; its exact
3,130-byte BMP output is pinned in `tests/fixtures/outputs/jsons/Encode.bmp.json`.
`enc_lossless_palette4_box_chain` uses a 4×4,100 four-color WebP source whose
packed palette stream reaches the 4,095-sample box-chain match boundary; its
exact 62-byte output is pinned in `tests/fixtures/outputs/jsons/Encode.webp.json`.
`enc_sequence_raw_final_aligned` uses two 4×3 RGB TIFF pages and produces an
exact 352-byte encoded sequence ending on a 16-byte boundary.
`enc_animated_reserved_disposal` re-encodes a two-frame GIF with disposal
values 4 and 2; its exact 13,328-byte Pillow output exercises the GIF writer's
valid `FrameDisposal::Reserved(4)` conversion.
`enc_sub_420_aligned_full_mcu_rows` encodes a 32×32 RGB source through the
complete 4:2:0 MCU-row converter; its exact 1,264-byte Pillow output is pinned
with SHA-256 `644ec00d071d47f21042a0548a7c2d89387c43039be6af375f7b087ef7c0f3a0`.
`lossy_vp8_delta_segment_values` decodes a four-segment VP8 frame whose
quantizer and filter strengths use delta syntax. Stock libwebp always emits
absolute segment values, so `scripts/build_webp_delta_cwebp.py` patches a
temporary copy of pinned libwebp 1.6.0 and
`scripts/generate_test_assets.py` verifies the resulting stream against its
byte and decoded-pixel hashes. Pillow 12.2 decodes the delta and stock
absolute-mode streams to the same 49,152 RGB bytes. The input PPM is pinned by
SHA-256 `9efd98332fada058f0ec106ac5c714b7898d640379b3739a22a1185397604421`,
the WebP asset by `9f6a1b274d9faf180fd024b962b068d6c4db5fe29e619868e7e4f9e91f12adbb`,
and decoded RGB by
`6b2c202c55d0bf70d9f32b74c1b8705232ae41e0ed24203ea2b1b138dfd914fe`.
The `portable_lossless_monochrome_alpha_17x17` AVIF row adds a 17×17 lossless
monochrome color item with an auxiliary alpha plane. Its 900-byte input is
pinned by SHA-256
`573db25044c9dcbb03f00d623d20a30639d3334ba35feae6d57f115e208741bd`; Pillow
returns exact RGBA pixels in 1,156 bytes with SHA-256
`d0201688c025cda34f274e3187240f35090e3366cb8883d3a3f04c45d03e0319`. The
case targets the `has_alpha` path in `decode_monochrome_portable`; its primary
and auxiliary AV1 items are both monochrome. The AVIF decode parity matrix
passes all 402 active rows. Coverage MCP no longer lists the true arm at
`src/codecs/avif/decode.rs:699` in the refreshed full run; source identity is
unverified and test attribution is unknown, so this is diagnostic coverage
evidence alongside the local parity result.

The `bmp_8bit_default_palette` ICO row sets the embedded 8-bit DIB's
`colors_used` field to zero while retaining its full palette. Its 1,374-byte
input is pinned by SHA-256
`4915bb486fdd9b56d0bb200d43dfeb80efe82762a6e02485390b22cabc7f22ad`; Pillow
returns 16×16 RGBA pixels in 1,024 bytes with SHA-256
`d3b82ecad923e39c5007fdae2b42acb59a6dd9d36419a4e1329d53859b824ff5`. The ICO
decode parity matrix passes all 55 active rows. Coverage MCP's refreshed full
report no longer lists the true arm at `src/codecs/ico/decode.rs:465`; source
identity is unverified and test attribution is unknown, so this is diagnostic
coverage evidence alongside the local parity result.

The `enc_cmyk_small_unaligned_fallback` JPEG encode row uses the existing
13×9 CMYK source, below the streaming threshold and outside its alignment
constraints. It exercises the baseline generic encode path and matches Pillow
12.2.0 byte-for-byte: 634 bytes, SHA-256
`0d8885a6009b46e1f8f066fe90abadf5dac3153dcd11c5c63a8f62c15bf9b03a`.

The `enc_cmyk_aligned_subthreshold` 32×8 row takes the aligned baseline
streaming path with only 256 pixels, below the 1,024-pixel threshold; its exact
759-byte Pillow output is pinned by SHA-256
`226d495545b556a72558ba6f9b768ef382372b2729eaa6481410313426c18fba`. The
`enc_cmyk_aligned_width_partial_height` 32×9 row reaches the aligned-width,
partial-height fallback; its exact 809-byte output is pinned by SHA-256
`aa5e354fe0ae1c8d072642ea27501f72252648fc1bb52a03bf3ff61d1730709c`. Together
with the 13×9 unaligned row, these three cases cover both alignment outcomes
inside the baseline CMYK dispatch expression.

The `enc_cmyk_optimized` and `enc_grayscale_optimized` rows take the generic
optimized entropy paths; Pillow's exact outputs are 10,325 bytes (SHA-256
`7bb93dc5c122a85aa555e03c9912da5f22d8f5377226d7cb16ceb9af84070d4e`) and
3,185 bytes (SHA-256
`5612f6c2f016150cde09d9d35f336c8cf9115178772bff3d143a64804908b9ff`),
respectively. The JPEG encode parity matrix passes with all five rows active.
Coverage MCP no longer lists branch gaps at `src/codecs/jpeg/encode/mod.rs`
lines 607, 608, or 631 in
`target/release-evidence/coverage-full-jpeg-optimized-paths-20261002.json`.
That full report passes every suite and repository floor: 85,184/142,042 lines
(59.97%), 16,037/32,156 branches (49.87%), 4,780/8,972 functions (53.28%), and
130,166/219,556 regions (59.29%). Its source/build identity is unverified and
test attribution unknown, so the branch results are diagnostic evidence beside
the local parity results.

The `error_malformed_structure_apng_fdat_payload_before_idat` PNG row places
frame-data payload after an `fdAT` sequence number before the default image's
`IDAT`. Pillow raises `OSError: broken data stream when reading image file`
when decoding and `SyntaxError: broken PNG file (chunk b'\x00\x00\x00\x01')`
when verifying. Its input is pinned by SHA-256
`07d84a0ef147069484e49bdb3a63aca3a28d5e863d6af033708c3b186434d783`. Parity
exposed a static PNG decoder path that previously ignored this invalid APNG
state; the decoder now validates the sequence-only exception and rejects early
frame payload before IDAT.

The `compression_deflate_deflate_predictor_checkpoint_343x2` TIFF row uses
343×2 RGB pixels with Deflate and horizontal Predictor 2. A 1,029-byte scanline
reaches the predictor's 1,024-byte cancellation checkpoint. The 194-byte input
is pinned by SHA-256
`1a0c5c7ce1a80fa071619c138e8702854ece6d7101fcd16cb93db4b8de899cdf`; Pillow's
2,058-byte RGB reference has SHA-256
`ab69d4afd329ce8d28339663557fdc4a3c66844a0ddde05f87bdf61f1b75574f`.

Two AVIF malformed rows cover required `meta` child validation:
`error_meta_first_child_not_hdlr` changes the first child box away from `hdlr`,
and `error_meta_hdlr_non_pict` declares a non-picture handler. Pillow raises
`UnidentifiedImageError` for both. Their input SHA-256 values are
`21062fa34dab7a259cc7aa47f7960a5c2a0b1b39d28396f361c46c5e4a754a9c` and
`39d963ae504465535d09d7341de78c1788a752cc528ff5d888a873a5478e85e4`.

`error_meta_missing_primary_item` and `error_meta_missing_item_info` retag the
respective required `pitm` and `iinf` children as `free`. Pillow raises
`RuntimeError: Failed to decode image: Missing or empty image item` for both;
their input SHA-256 values are
`e7eb26add7703e8973c243e22f7b9225ead5292e8e1f051a09ada938824c0ffa` and
`8551acfec0234028ec989a52f1ceea4ff5666a3bc929e00a5a4946f902ae6dc6`.
`error_meta_missing_item_properties` retags the required `iprp` child as
`free`; Pillow raises `UnidentifiedImageError`, and the input SHA-256 is
`5339cf9635130eee4dcfd7801dd25563b591332b243b137b077a67576ee69436`.
`unknown_extended_size_uuid_box` appends a valid top-level extended-size UUID
box to the 64×64 I444 fixture. Pillow returns the same 12,288 RGB bytes as the
base image; its input SHA-256 is
`270ba9556bd21bd85c56b5f7609a5d48e9a255ffc3ec4d1b1fa4c1885a70847d` and its
pixel SHA-256 is
`5e74b862ca69314d8ae9a3aa4b8f3af5651dfbd238ce922a7973c193e338f64a`.

`error_bad_ifd_zero_tile_height` sets TIFF `TileLength` to zero while retaining
a nonzero width. Pillow opens and verifies the header, then raises
`ValueError: tile cannot extend outside image` on decode; the input SHA-256 is
`6c83b3e663f810d18db3d9a6a623eeac6ee7f21407d9dd22ad7084f0f626e208`.
`enc_grayscale_small_fallback` encodes a 13×9 grayscale JPEG source below the
streaming threshold and outside its alignment constraints. Its exact 422-byte
Pillow output is pinned by SHA-256
`05fae511efea0f239c6d7c59ee24d0e52f993f6d6581ee842b2397ec06cb3442`.
`enc_grayscale_aligned_width_partial_height` and
`enc_grayscale_aligned_subthreshold` encode 32×1 and 32×8 grayscale sources.
Both stay below the 1,024-pixel threshold with aligned width; their heights
take the false and true outcomes of the 8-pixel alignment check. Pillow's
exact outputs are 349 bytes (SHA-256
`1467cdfb18e2f59ce73814ebfeb2882f6bef77c97990b4b16c3f4182e217b241`) and
465 bytes (SHA-256
`29c71e3206d0eeebe5428504e2432d5810631508a82d761f7fd3db9fbab3ffec`).
`enc_sub_422_aligned_width_partial_height` and
`enc_sub_422_aligned_subthreshold` encode 64×1 and 64×8 RGB sources with
4:2:2 subsampling. They stay below the 1,024-pixel threshold with aligned
width; their heights take the false and true outcomes of the 8-pixel alignment
check. Pillow's exact outputs are 691 bytes (SHA-256
`4afe5fc617557765f13a6806683d757d6bdab6815240556c4192f4f357dc7ea5`) and
1,026 bytes (SHA-256
`3c6dcb8056b151eb832db29ec8a337e4edaf76a423ba0aefdf4573466d557a01`).
The JPEG encode parity lane passes all 72 active rows.

The `palette_absent_animated_no_palette_second_frame_no_local_table` GIF row
removes the second frame's local color table while retaining its two-frame
palette-less structure. Pillow returns an 8×8 luminance image (64 bytes) with
SHA-256 `c2a74daea21f6caad6b7794dcfd121cbaa0bddc3407ce5f5fdc1914bd5e7ff90`;
the input SHA-256 is
`440eb36917bab98cd88a53534d87aadb611865999898bc9bbc3503a62a2c9654`.

The `error_malformed_short_dib_payload_below_eight` ICO row contains a
seven-byte embedded DIB payload, exercising the short-payload rejection before
the DIB header can be read. Pillow raises `OSError: Truncated File Read`; the
input SHA-256 is
`585f1feefb2f364c4ecb0b50ac8e3ff414771fde088ec23fe39b44e652caa981`.

The `incomplete_image_data_no_iend` PNG row contains a valid IHDR and IDAT
chunk, but its truncated zlib stream does not provide the complete image and
the file ends without IEND. Pillow raises `OSError: image file is truncated`
when loading it (and `OSError: truncated PNG file` during `verify`). Its
1×2 fixture is pinned by SHA-256
`b4c5b83118403fbff8bfa3fe0d4d85f094a79e21a8421de87bd7695c9b2e56e7`. The PNG
decode parity matrix passes with the row active, and Coverage MCP no longer
lists the no-IEND `NeedMore` mapping at `src/codecs/png/decode.rs:678` in the
refreshed full report. Source identity and test attribution remain unverified.

The public parity matrix compares every eligible successful non-direct-still
row through its shape-matched sink API: stills through `encode_to_sink`, and
animations plus all TIFF cases through `encode_sequence_to_sink`. Each sink
result must match the same pinned Pillow bytes and the regular encoder result.
At the time of the selected LLVM report below, the public matrix had 1,334
active decode rows across eight format tests and 386 active encode rows across
19 `coverage_matrix_tests` functions. All 27 public decode/encode matrix tests
passed. The selected LLVM report,
`target/release-evidence/coverage-public-parity-matrix-vp8-delta-20261002.json`,
records 14,277/32,072 branches (44.52%) for that earlier matrix. It predates
the TIFF rows recorded below, is not a full-suite measurement, and Coverage MCP
marks source identity unverified and test attribution unknown. Coverage MCP
could not calculate an incremental union with the earlier report because exact
baseline branch detail is incomplete, so no marginal percentage gain is
claimed.

Coverage MCP lists no remaining branch gaps at WebP
`Vp8Decoder::read_quantization_indices` line 1119 after the delta-mode row was
added; the row-only report covers the `true` outcome, while the full matrix
report covers both outcomes. It also lists no remaining gaps for PNG
`validate_color_chunk_structure`, TIFF `decode_packbits`, or JPEG
`convert_rgb_420_mcu_row`, including its aligned fast path. The earlier
edge-case report still predates the delta-mode row. The targeted report
`target/release-evidence/coverage-webp-copy-overrun-row-20261002.json` records
the `<4>` lossless decoder taking the true bounds-rejection outcome at
`lossless.rs:766`; its filtered MCP query no longer lists that arm as a gap.
Coverage MCP marks these reports' source/build identity unverified and test
attribution unknown, so only the observed locations and aggregates are
reported. The latest full local `make coverage` campaign passes all 56
`coverage_matrix_tests`, including all eight decode lanes and the 402/402 AVIF
parity rows. Its report,
`target/release-evidence/coverage-full-avif-jpeg-alignment-edges-20261002.json`,
records 85,328/142,054 lines (60.0673%), 16,076/32,164 branches (49.9813%),
4,785/8,972 functions (53.3326%), and 130,382/219,569 regions (59.3809%).
Coverage MCP reports no remaining branch group at
`src/codecs/avif/container.rs:441` or `:444`,
`src/codecs/avif/samples.rs:513`, `src/codecs/jpeg/encode/mod.rs:632` or `:654`,
or `src/codecs/tiff/decode.rs:265`. The report has no source/build receipt and
test status is unknown, so those coordinates are diagnostic evidence alongside
the passing public parity lanes. The 100% coverage goal remains open.

## Packed low-bit TIFF tiles — 2026-10-02

Three active Pillow parity rows now cover 1-bit black-is-zero grayscale,
Deflate-compressed 2-bit grayscale, and 4-bit palette tiles. Each uses a
spec-compliant 16×16 tile over a 17×13 image, so the decoder reconstructs both
partial right and bottom edges. Pillow 12.2.0 supplies the exact references.
Three more rows use a Pillow-accepted 9-pixel tile width over a 17×1 image,
covering unaligned second-tile offsets for 1-bit, 2-bit, and 4-bit samples.
Those nonstandard widths record Pillow compatibility, not TIFF-spec
conformance.
Byte-aligned sample placement stays on a bulk byte-copy path; packed samples
with unusual bit offsets use a checked merge path that preserves neighboring
samples in the destination byte. No private unit tests were added.

The complete TIFF decode matrix now has 167 active rows. The broader 100%
coverage goal remains open. The refreshed local full LLVM run at
`target/release-evidence/coverage-full-png-incomplete-no-iend-20261002.json`
passes repository floors: 85,181/142,042 lines (59.97%), 16,027/32,156
branches (49.84%), 4,780/8,972 functions (53.28%), and 130,159/219,556 regions
(59.28%). Coverage MCP no longer lists the unaligned-copy arm at
`src/codecs/tiff/decode.rs:1199` as a gap; it still lists packed-row bounds
checks at line 1193 and the defensive sample-width guard at line 1122. MCP
marks report source identity unverified and test attribution unknown, so these
branch locations remain diagnostic observations rather than source-bound
coverage claims. The older full report below predates these rows and the
packed-tile decoder.

## Separate-planar TIFF parity — 2026-10-02

Manifest parity now covers PlanarConfiguration 2 RGB strips and tiles, including
raw data, Deflate with and without horizontal prediction, partial right/bottom
tiles, missing final strips and tiles that Pillow zero-fills, one-sample
grayscale, and 1:1 YCbCr strips and tiles. The YCbCr cases preserve Pillow's raw
component triplets in RGB mode. Malformed rows cover excess strip/tile offsets,
out-of-bounds strip/tile payloads, zero rows per strip, invalid planar value 3,
and missing, empty, zero, or mismatched compressed byte counts. Pillow accepts
inspection for invalid planar value 3 but fails at pixel load; Rust matches that
lazy failure. Pillow also rejects extra separate-planar tile offsets during
open, while a short final raw tile remains a successful zero-filled decode.

At that measurement, the TIFF decode matrix passed all 161 active rows. The full
`make coverage` campaign passes all suites and repository floors with
`target/release-evidence/coverage-full-planar-tiles-20261002.json`: 84,990/141,861
lines (59.91%), 15,994/32,130 branches (49.78%), 4,768/8,959 functions
(53.22%), and 129,857/219,262 regions (59.22%). Coverage MCP returns no
remaining branch gaps for `decode_separate_planar_tiles`; report source identity
is unverified and test attribution unknown, so its location results are
diagnostic rather than source-bound claims. The current matrix has 1,367 active
decode rows and 392 active encode rows at that point. Strict debug, release,
coverage-configured, x86-64 SSE2-baseline, and AVX2 Clippy checks pass. The x86
SIMD commands are compile checks and do not measure runtime performance.

`pillow_tolerated_post_idat_color_chunks` appends PLTE and tRNS chunks after
the 1×1 RGB PNG's IDAT data; Pillow and the Rust decoder retain the same exact
three pixel bytes. The existing `size_1x1` row also exercises
`decode_with_policy` at its exact 57-byte non-pixel metadata budget. The public
`apng_animated` row decodes with a 169-byte metadata budget and compares every
frame with its pinned Pillow reference; a 168-byte budget returns
`LimitExceeded(MetadataBytes)` with an observed extent of 169 bytes. The final
matrix report above includes these rows. Its Coverage MCP source and test
attribution remain unverified.

`apng_orphan_fdat_missing_control` keeps the orphaned `fdAT` sequence number
valid, so `decode_sequence` reaches the missing-`fcTL` outcome at
`src/codecs/png/decode.rs:1007`. Pillow 12.2 decodes the default image to the
exact pinned 49,152 RGB bytes, then reports `OSError: Truncated File Read` when
animation playback seeks the orphaned frame data. The paired
`apng_orphan_fdat_invalid_sequence` case retains the original bad sequence
number and checks Pillow's `SyntaxError: APNG contains frame sequence errors`
in ordinary decode; it covers the sequence-consumption error at
`src/codecs/png/decode.rs:568`. The short-`fdAT` row continues to verify
truncated-chunk handling before either sequence outcome.

`apng_empty_fdat_before_idat` adds a sequence-only `fdAT` before `IDAT` while
preserving Pillow's exact default-frame pixels and both decoded frames. Rust
verification rejects the misplaced chunk; ordinary decode retains the default
image, and sequence decode consumes the sequence number while ignoring the
empty frame-data payload. The follow-up PNG-only public parity run is
`target/release-evidence/coverage-png-apng-orphan-sequence-parity-20261002.json`;
its `test_decode_matrix_png` integration test passes. Coverage MCP reports no
remaining branch observations at `decode.rs:562` or `decode.rs:568`. The
`parse_apng` length check at `decode.rs:1003` still lacks its non-empty
pre-IDAT `fdAT` outcome; moving compressed frame data before `IDAT` caused
46,906 of 49,152 decoded bytes to differ, so that case cannot be added as a
pixel-parity witness. This selected-test report has an unverified source/build
identity and unknown test attribution; its global branch denominator is not a
whole-suite measurement.

Coverage MCP also found the unobserved COREHEADER depth-selection arm at
`src/codecs/bmp/inspect.rs:66`. The new `error_unsupported_core_depth` row uses a
complete 1×1 OS/2 COREHEADER with unsupported 2-bit samples, a four-entry
palette, and a padded row; Pillow reports `Unsupported BMP pixel depth (2)`.
The selected public parity run now observes both outcomes at that branch.

The TIFF decode matrix now invokes both public `decode_with_token` and
`decode_prefix` for each active TIFF input. Successful token results are
checked against the Pillow pixel reference; terminal errors retain the Pillow
contract, and resumable results must match the prefix API's exact retry
minimum. This keeps the ordinary `decode` route as the terminal Pillow
error-parity assertion. It also exposed two Deflate `NeedMore` results inside
strips whose byte counts already bound the compressed input. Those errors are
now terminalized in the TIFF Deflate path, while genuinely truncated TIFF
structures and strip spans retain resumable status.

Public boundary-sized TIFF cases cover token-aware sample-conversion
checkpoints: the existing 128×64 WhiteIsZero bilevel image contains exactly
1,024 packed bytes; new 32×32 WhiteIsZero Gray8 and Gray2 images reach their
1,024-sample checkpoints; and a new 32×32 YCbCr image reaches its
1,024-byte checkpoint. Coverage MCP reports no remaining branch outcomes in
`convert_pixels_with_token`. The new `lzw_extra_zero_output_strip` fixture
contains one LZW strip that decodes to `A` plus an extra strip whose geometry
requires zero output; pinned Pillow returns the exact one-byte image. The
decoder now skips unused compressed strips after checking their declared byte
range, while retaining the Pillow error for uncompressed extra strips. The
LZW first-code output-length guard at `decode.rs:1377` remains unobserved by
public inputs because zero-row compressed strips are now skipped before
inflation.

The full all-format public decode matrix passes all eight integration tests.
Its report,
`target/release-evidence/coverage-public-decode-matrix-parity-final-20261002.json`,
records 11,386/32,252 branches (35.30%) for this selected decode run. Coverage
MCP reports both outcomes covered at BMP `inspect.rs:66` and no remaining
outcomes at TIFF `decode.rs:490` for the zero-row compressed-strip path,
`decode.rs:599` for bounded Deflate handling, or in `convert_pixels_with_token`.
The report has no source/build receipt and unknown test attribution; its
incremental comparison with the earlier selected report is unavailable
because that baseline lacks complete branch detail. These measurements do not
replace the separate full-suite coverage snapshot or claim 100% coverage.

The [AV1 encoder entropy corpus](../tests/fixtures/outputs/av1_encoder/index.json)
retains eleven complete files from standalone pinned libavif/libaom builds, with
Pillow/dav1d decoded pixels. It contains thirteen independently inspected tile
payloads: monochrome, 4:2:0/4:2:2/4:4:4, alpha, two-column tiling, lossless,
and 8/10/12-bit samples. Original and instrumented builds each encode every
file twice with identical results. All thirteen tile payloads bind uniquely to
finished native writers; unmodified libaom replay matches every logical state,
CDF update and final byte. This is native encoder evidence, not Pillow `save()`
parity or a Rust execution result.

The 83 hashed artifacts total 1,618,610 bytes. Gzip traces retain expanded
lengths and hashes; prepared planes, replay inputs, syntax reports, complete
files, pixels, instrumentation patch and build identities remain separate.
Committed tiles exhibit 292 normalization flushes and 103 carry changes,
including a two-byte carry. Separately labelled native models cover alphabets
2–16, probability endpoints, CDF plateaus, frozen adaptation, counter
thresholds and empty/short termination. They are not full-file syntax claims.
The generator is `scripts/generate_av1_encoder_refs.py`; its C observers under
`scripts/av1_encoder_oracle/` are development tools, excluded from the crate.
The Rust entropy/tile writer, image analysis and full AV1 compressor remain
unimplemented. The 16x16 10-bit I420 case also supplies the active public AVIF
decoder parity row `high_bitdepth_still_10bit_420_lossy_16x16` in the
[coverage matrix](../tests/fixtures/coverage_matrix.json), with generator and
fixture provenance in the [AVIF fixture README](../tests/fixtures/input/images/avif/README.md).
Its selected Pillow 12.2.0 check passed for 16x16 RGB8 output
([reference bytes](../tests/fixtures/outputs/raws/Decode.avif_high_bitdepth_still_10bit_420_lossy_16x16_avif.bin), 768 bytes,
`55820e29e26b25634c402e57e8b743bb9560666861f5ea07c14a607f31eea38f`). A
same-source AVIF selected-test Coverage MCP comparison reports 73 newly
observed branch coordinates, including seven outcomes in
`complete_high_depth_420_reconstruction_context`. Its source/build receipts are
unverified, so this is limited selected-test coordinate evidence and does not
claim a full-suite coverage-percentage change. The active matrix row is decoder
evidence; it does not promote a Rust AV1 encoder roadmap item.

The 2026-09-17 private still-container candidate has a
[28-file native mux corpus](../tests/fixtures/outputs/avif_mux/index.json).
Pinned Pillow/libavif/libaom observations repeat exactly, including complete
encoded bytes and decoded pixels. Sixteen files correspond to registered
planned still rows; twelve supplemental files cover orientation, combined
alpha/metadata and actual media reuse within and across item boundaries.
The corpus has 105 hashed artifacts totaling 314,731 bytes and retains clean
libavif source, native binary, generator, source image and manifest identities.

The Rust writer consumes semantic descriptors and encoded/metadata payloads;
expected serialized boxes and output offsets are excluded from its inputs.
Deferred tests compare complete containers, output limits and separate model
error/interruption states. The source-derived 10/12-bit descriptor handling
has no native encode witness yet; all 28 files are 8-bit. AV1 compression and
metadata preparation remain unimplemented, so public encoding is still
unavailable and no matrix row is promoted. Rust behavioral execution and
managed coverage remain deferred; historical measurements below are unchanged.
All 16 strict compile configurations pass across native and both WASM targets,
with AVIF-only/all features and ordinary/coverage modes, plus native default
and no-codec lanes. Warnings-as-errors rustdoc, formatting and static checks
pass. The coverage-origin manifest now classifies 111 exact guards across 16
files; these are provenance records, not coverage exclusions or a new
measurement.

## Historical release 0.1.2

Version 0.1.2 passed [main CI](https://github.com/appunni-m/image-slash-star/actions/runs/35007807946)
and [release CI](https://github.com/appunni-m/image-slash-star/actions/runs/35010129246)
at `70190214a0711223302c76ab58e76c097288d80b`. The published crate matches the
GitHub checksum manifest, with SHA-256
`e53037e57d0c5cae052ba94851c8cf72a80b9dfe195cd21b166506ab7bdbeb3b`.

The 2026-09-15 all-feature release coverage run recorded 95,473/161,451 lines,
14,912/32,258 branches, 4,859/9,244 functions, and 140,609/241,503 regions.
The release floors are respectively 59%, 46%, 52%, and 58%. The complete target
retains 100%; a floor pass is not complete coverage. All executed comparisons
remain mandatory.

## Maintained fixture matrix

The retained matrix contains 1,798 total rows: 1,370 decode /
inspect / verify rows and 428 encode rows. Of those, 1,370 decode rows and 396 encode rows are
active; 32 encode rows remain planned. Planned rows stay outside executed
parity numerators. Active rows have operation-specific outcomes, including
not-applicable results.

[Generated capabilities](capabilities.md) distinguish declarations and actual
native/all-feature fixture observations. A WASM cross-compile does not extend
that pixel-evidence scope.

## Decoder validation — 2026-09-20

The full public matrix passes all 46 tests, including 343 AVIF rows and exact
native comparisons of complete animations. All native, WASI and browser-WASM
feature lanes pass. High-depth, HDR and animation additions are on `main`,
awaiting the next release; the 32 AVIF encoder rows remain planned.

A fresh local `make coverage` run passes every executed test and the unchanged
release floors: 99,349/163,373 lines (60.8112%), 15,524/32,418 branches
(47.8870%), 5,052/9,372 functions (53.9053%), and 145,689/243,793 regions
(59.7593%). This is LLVM evidence, not a new managed Coverage MCP snapshot.
The changed-line review observes 274/294 executable lines in the seven changed
AV1 modules. Unobserved lines include defensive failures and the Wiener stripe
branch; this is not complete line or branch coverage.

The local reports live under `target/release-evidence/`: `coverage.json`,
`coverage.lcov`, and `avif-fix-coverage.json`. The last receipt retains source
and input hashes and the exact uncovered line list. CI produces a fresh report
for each pushed commit and retains it as the `llvm-coverage` artifact.

## Codec parity and coverage — 2026-10-03

At that point, the matrix contained 1,927 total rows: 1,474 decode /
inspect / verify rows and 453 encode rows. Of those, 1,474 decode rows are active;
421 encode rows are active and 32 are explicitly planned. It includes 440 active
AVIF decode rows. Twelve new error rows pass the selected public matrix runs:
`short_marker_missing_eoi` (JPEG, input SHA-256
`869e30d73346ca502d81c1a04881b3ca8cbb64d980cb73b73cf856d54a8bdcbc`),
`error_ispe_nonzero_version` (AVIF, `9caaefec8072bd826607867f9d80c8ce2a10d67f1638bf8c1ac382055d75a791`),
`error_ispe_zero_width` (AVIF, `3524b652351b5a668a7a91a7dcd10a376205d5771858d11e073f61bdf96de3da`),
`error_iloc_version_3` (AVIF, `9ce231f76eac5fb15f9da9d12cc34b6e6a59d7eeee8bcad4101a451ee9648dd2`),
`error_sequence_zero_timing_rate` (AVIF, `055dea40a69ca9eb7a60a499524a7e563990bccb708f6a9992c1691f304f15e2`),
`error_inconsistent_operating_point_idc` (AVIF, `3c81896058305429697219be55ddcaf4fcf5293aca48570c1e7e22f6ebcd0a07`),
`error_sequence_zero_time_scale` (AVIF, `92d7882e439adf1430d60460d488d0df2b7feb9f6ba3bb7931898c4f70f25ac0`),
`error_inconsistent_operating_point_idc_low_byte` (AVIF, `d49e5403f421c1f71140c7442ba4493e51317bd1c9776d0aaf19069e9002fd71`),
`error_ispe_zero_height` (AVIF, `e93421df7d5d3564f7d297c4deb09a765be702e16aa86de41c3705974d059024`),
`error_vp8_color_space_invalid` (WebP,
`ce6b3a9b8e69194d02cdc0739b9db9cdc668554abc0fe29e28fa902e49eb6e45`),
`error_malformed_container_animated_all_frame_chunks_unrecognized` (WebP,
`cde9d77559e01ef3324fb73f6541c4cd6257b8563595a4ca397fc0ab52f69886`), and
`error_truncated_riff_extent_metadata_preflight` (WebP,
`e84c137505358f37c528aa349b7541971603dae587e5dbfd15e67792ce72da09`).
The pinned Pillow 12.2.0 oracle rejects each case under the matrix's malformed
decode contract. The selected rows and full coverage campaign pass. Coverage
MCP reports nine newly observed branch coordinates at AVIF `samples.rs:656`,
`:661`, `:1030`, `sequence.rs:179`, and `:223`, JPEG `parser.rs:609`, and WebP
`vp8.rs:1268`; a filtered follow-up reports no missing branch groups at the
targeted AVIF height, timing, and IDC operands. The comparison has no
source/build receipts and test attribution is unknown, so these are coordinate
differences rather than verified causal gains or regressions. The active
`uniform_tile_rows_monochrome_256` AVIF row uses a 344-byte constant grayscale
still with one tile column and four uniform tile rows (input SHA-256
`782568b37a63b44e632aee68896bcee3c59c5492081c7ac2ac6a31fc0ca1df37`). Its
Pillow reference is 196,608 RGB bytes with SHA-256
`d5039cbfb3fb9a915614802a721cba155442563d36a6bbf73e582e65f5290890`; the
selected public parity row passes. The pre-row full report showed the true arm
of `read_tiling` at `av1/frame.rs:3647` missing, and the selected report leaves
only the false outcome of the tile-row bit read. The active WebP row
`error_malformed_container_animated_all_frame_chunks_unrecognized` changes both
nested VP8L frame FourCCs in a valid two-frame lossless animation to `JUNK`
while preserving RIFF lengths (input SHA-256
`cde9d77559e01ef3324fb73f6541c4cd6257b8563595a4ca397fc0ab52f69886`). Pillow
reports `could not create decoder object`; its selected public matrix row passes
the malformed WebP contract. The selected report covers the previously missing
true arm of `WebPDecoder::new` at `webp/native/decoder.rs:226`. Coverage MCP
reports missing source/build receipts and unknown test attribution for these
selected runs, so the arm mappings remain coordinate evidence. The new
`animated_lossless_inter_420_b32x32_10bit_64x64` AVIF sequence adds a
`animated_lossless_inter_420_b32x32_10bit_64x64` AVIF sequence adds a
deterministic 64×64 10-bit 4:2:0 inter-frame case while preserving the prior
32×32 fixture. Its pinned dav1d trace confirms four 32×32 leaves in the second
frame; the selected public parity run passed and checked exact Pillow frame
bytes. The selected LLVM report records three hits on the previously missing
false arm in one `decode_inter_translation_impl` specialization at
`src/codecs/avif/av1/block.rs:54836`. The fresh full report adds one aggregate
branch observation. Coverage MCP comparison remains limited because these
reports have no source/build receipts and MCP cannot attribute tests. The active
`enc_lossless_sampling_multi_group_checkpoint` WebP
row encodes an 8192×16 RGB PNG with lossless quality 80, method 4, and a
cancellation token. Its deterministic 32×1 metadata map alternates two retained
Huffman histogram groups. Pinned Pillow 12.2.0 produces the committed 98,388-byte
reference (SHA-256
`cc92a5c28e780be64ef34a104ddeeb92e76c967f8e3f8f1a16fb42a01ca907a4`), and the
focused selected row and full encode matrix pass. The current full local report
still lists the true checkpoint arms at `encoder.rs:1378` and `:1418`; the
filtered query at `:1421` has no missing branch group. Because the report lacks
source/build receipts and test attribution, this is a fixture-parity result plus
coordinate coverage evidence, not source-bound proof that those checkpoint arms
were executed. The active `enc_animated_non_centisecond_delays` GIF row uses
two WebP frames with 17 ms and 33 ms source durations. Rust now stores their GIF
delays as 1 and 3 centiseconds, matching Pillow's 10 ms and 30 ms output; the
129-byte encoded reference SHA-256 is
`52f23eb3646581d4982bed87c11bf63315ddf2e3a4cdf5d73a5b0b80c44e8509`. The
`enc_animated_large_fraction_delay` row sets the first duration to the unreduced
`u64::MAX / u64::MAX` rational (one second), exercises GIF's wide scaling path,
and matches Pillow's `[100, 3]` centisecond delays. Its 129-byte reference SHA-256
is `df760f2efb3378fd2eb3c46660ebaf5399b11086838ec28f99b5faf5e9a15792`. The
focused GIF encode parity suite and full coverage run pass. Coverage MCP's
filtered `gif_delay` query now leaves only the zero-denominator arm at line 869;
the wide-scaling arm is no longer missing. The local report has no source/build
receipt and test attribution is unknown, so this is coordinate evidence. A
zero-denominator value is not a valid Pillow frame duration and has no public
Pillow-backed fixture witness. The new `primary_item_irot` row decodes the independently
generated 325-byte mux fixture with an associated 270-degree primary-item
rotation. Its Pillow 12.2.0 output is 2×3 RGB (18 bytes,
`efbe5247163f007591d148c7975ad74429fd2282feb0bdec402838155763da38`), and the
Rust decoder reports the matching counterclockwise 270-degree transform. The
Seven `portable_lossless_monochrome_limited_*` rows cover limited-range
monochrome RGB and RGBA at 8, 10, and 12 bits, plus a BT.2020/PQ CICP case.
Their selected public parity
rows pass against exact Pillow bytes; the preceding all-feature AVIF decode lane
included all 415 active rows. Coverage MCP reports no
remaining branch gap at `container.rs:1367` in
`coverage-avif-primary-irot-current.json`; that selected report has no
source/build receipt and labels test status unknown, so this is bounded path
evidence only.

The new active `ipma_version_1_wide_item_ids` still row widens the primary
item ID in AVIF's `ipma` FullBox from 16 to 32 bits and adjusts the following
absolute `iloc` extent. The 3,079-byte fixture is generated from Pillow's
baseline still and hashes to
`90ac8f0230ddede24a72b73fb120bd46680962d4c0a7facab31e77b088cd82c5`. Pillow
12.2.0 verifies and decodes it to the same 128×128 RGB pixels, metadata, and
info as the baseline. The focused public AVIF decode matrix passes 416/416
rows. The full local `make coverage` campaign passes every executed test and
reports 85,737/142,131 lines (60.3225%), 16,350/32,170 branches (50.8237%),
4,797/8,975 functions (53.4485%), and 131,045/219,720 regions (59.6418%);
all four release floors pass and the 100% target remains open. Its LLVM report
is `target/release-evidence/coverage-full-ipma-v1-20261003.json`. Coverage MCP's
coordinate comparison lists two newly observed branch arms in `parse_ipma`,
the version-zero arms in `container.rs` and `samples.rs`. That report has no
source/build receipt and MCP reports test attribution as unknown, so the
comparison is coordinate evidence; the local matrix and full coverage commands
themselves passed.

The active `iloc_version_2_wide_item_ids` AVIF row derives from
`baseline.avif`, widening the `iloc` item count and ID to 32 bits, adding
construction method zero, and relocating the absolute extent after the
six-byte box growth. The generated input SHA-256 is
`39a494da43edb3e25dcc60c1c51c987452c66a2590e5ac9563d416d922199d87`. Pinned
Pillow 12.2.0 accepts it and produces the same 128×128 RGB pixels and info as
the baseline (49,152 bytes; output SHA-256
`f1a2555b1c61036af2bd1d3d125a6a1343993873d9e5d58e3caee79989095dcf`). The
selected public row passes, and the full AVIF matrix passes 418/418 rows. Its
selected LLVM report records four visits to the version-2 path at
`container.rs:608`; the full report has no remaining branches at lines 608 or
622. Coverage MCP reports no gaps at those coordinates, but has no source/build
receipt and test attribution is unknown, so that result is coordinate evidence.

The active `size_1x1_local_palette_only` GIF row moves the exact six-byte
palette from the Logical Screen Descriptor's global table into the image's
local table, leaving no global table. Its 37-byte input SHA-256 is
`2d029f561c59b4bf8c23b46e1acf61a144e16d7ec46901e0c428914801363146`. Pillow
12.2.0 returns the one-byte P image `00` and the six-byte RGB palette with SHA-256
`b0f66adc83641586656866813fd9dd0b8ebb63796075661ba45d1aa8089e1d44`; the
focused public parity row passes detect, inspect, verify, decode, and sequence
comparisons. Pillow omits `info["background"]` for this input, so the matrix
records the Logical Screen Descriptor's palette index with
`specification_reference` provenance. Metadata-byte scanning runs only when a
caller sets a `DecodePolicy` metadata limit, so the parity-only row does not
claim coverage for that policy path.

The ordinary and token-aware GIF LZW paths reject zero-sized frames before
decoding and return as soon as the output reaches its expected length. This
makes the first-code output-full guards and end-code exact-length success arm
unreachable; those redundant conditions are removed. All 79 active
GIF parity rows pass. The refreshed Coverage MCP GIF query no longer lists
`decode_lzw`; it still lists the metadata scanner's global-table false arm at
line 366 and local-table true arm at line 393. That scanner only runs when a
caller sets a metadata-byte policy limit. The new full report has no
source/build receipt, so these filtered results are source-coordinate leads.

The active `high_bitdepth_still_10bit_420_alpha_lossless_16x16` AVIF row
exercises the public 10-bit 4:2:0 color path with an auxiliary 10-bit alpha
plane. Its 1,653-byte input SHA-256 is
`8c3cb86055e1003d91897d986995dd2ee030d0278b82d3d7c9ddc783189eb76f`; pinned
Pillow returns 16×16 RGBA8 (1,024 bytes) with SHA-256
`a3eec8f3c1e9539ccee71ec2e9d514a46d20d0b93a7b8e5d562379ef4a794dd9`. The
selected public parity row passes. Its isolated branch report records the
10-bit alpha path at `decode_portable` lines 492, 535, and 555. The refreshed
full report shows no remaining branch gap at those locations.

Two more active 10-bit alpha stills exercise the AV1 4:2:2 and 4:4:4 decode
paths. The all-lossless 16x16 4:2:2 input hash is
`eef1ffae7961354032dad642c1ebfc146cbcb28f7147c4dce6d9b8a4de17d392`; Pillow
RGBA8 output hash is
`b7731cca14b3da564883533a5cfd6ddc75aba189b60dbe7795de93711237a457`. The
16x16 lossy 4:4:4 input hash is
`468bd5af2f96693582d6618572dc353cac4ee16719cebebb8e20ff7e81049548`; Pillow
RGBA8 output hash is
`02e871ddbaa576836e9d486cf36be09efce4f2fef164c7f66af2a579dfb1e67a`. Both
selected public parity rows pass. Coverage MCP's selected report reaches the
10-bit 4:2:2 sampler path at lines 588 and 606 and the full-resolution alpha
conversion at line 496. That report has no source/build receipt and marks test
status unknown; the local selected matrix command supplies the pass result.

The active `portable_lossless_monochrome_1x1` row exercises a
1×1 8-bit lossless monochrome still. Pillow returns RGB `7f7f7f`; the selected
public parity test passes, and the Rust reconstruction crops its coded block
to the visible canvas. The row's pinned input SHA-256 is
`6c4212de07ead445c0b468c39b77f099cc8555e99edd6460806407d7536be305`.
The WebP decode lane passes all 230 active rows. Two generated 1×1 VP8L cases
exercise transformed opaque RGB reconstruction and singleton RGBA color-cache
insertion. Their Pillow outputs are RGB `0a141e` and RGBA `0a141e80`. Coverage
MCP reports no remaining branch group at `webp/native/lossless.rs:763`; it still
reports the alpha-used false arm at line 155.

Two JPEG fixtures truncate 64 entropy bytes before EOI in baseline 4:2:2 and
4:4:4 inputs; both selected public parity rows pass. Coverage MCP reports no
remaining branch group at the direct-row fallback locations
`jpeg/decode/decode.rs:1504` and `:1697` in the latest full report.

The metadata-policy manifest adds public TIFF cases for a cyclic IFD, empty
tile byte counts, and an empty IFD chain. The policy test checks successful
metadata accounting and malformed-scan propagation through inspect, still
decode, and sequence decode. Coverage MCP reports no remaining branch groups
at `tiff/decode.rs:133`, `:150`, or `:160`.

These filtered MCP observations have no source/build receipt or per-test
attribution; they are bounded path evidence. The full-report comparison lists
seven newly observed branch coordinates at these TIFF, JPEG, and WebP paths,
but MCP marks the result limited because source/build identity is unverified.
The full local report still passes every repository coverage floor.

The active GIF row `enc_rgb_high_color_token_checkpoint` adds a 33×33 RGB PNG
with 1,089 distinct colors, forcing the token-aware median-cut quantizer across
its 1,024-pixel checkpoints in all three mapping passes. The source PNG SHA-256
is `a4c488ae9683a7ae782017bf8fc334d67ec43e0f0430a0c2442cfa426dcc82e6`; its
Pillow GIF reference is 1,862 bytes with SHA-256
`7bbedc5d1161b6b8b00a41b9b5f4dc52e7cc2d33a4cc63b30320876a4165e43f`. The
selected public parity row passed byte-for-byte comparison against Pillow and
ordinary Rust encoding. Its isolated LLVM run records one checkpoint hit and
1,087 non-hits at each pass; the fresh full report records no remaining branch
gap for `quantize_rgb_nearest` in Coverage MCP. Both reports lack source/build
receipts and test attribution, so local test results and counters are scoped
evidence rather than managed coverage attribution.

The active GIF row `enc_rgba_high_color_token_checkpoint` exercises transparent
pixel normalization in token-aware RGBA quantization with 1,088 opaque pixels
and one fully transparent pixel. Its 33×33 PNG SHA-256 is
`1939c2816f1f56c21c3e672d03df8341a6dcf16181cbaa8035b93ad0e3ae9896`; the
1,936-byte Pillow GIF reference has SHA-256
`ee584ccbbe4b55eb06a9e2d3ad52ae037d74f2d3ad545cf0e954ef555e8c4e34`. The
selected public parity test passes exact Pillow byte parity. Its isolated LLVM
run records the normalization checkpoint and both alpha outcomes; the full
report has no remaining branch group for `quantize_rgba` in Coverage MCP. Those
reports also lack source/build receipts and test attribution.

The active GIF row `enc_animated_rgb_full_palette_token` uses a lossless,
opaque two-frame WebP animation with all 256 grayscale values in each 16×16
frame; the second frame swaps the first and last pixels. The WebP SHA-256 is
`eda7f7272532e97d362a46a905d3f80dc857c5a5602232dde366f885d9838946`. The row
requests GIF local color tables and the public token-aware sequence encoder;
its Pillow GIF reference is 2,963 bytes with SHA-256
`881dfb2f5a8b81575a20ac40350ccb6124bf3ef95df6b9b15c9a9cc5cb98c6f4`. The
selected parity row passes for both ordinary and token-aware sequence encoding.
Its isolated LLVM run records three visits to the false arm at
`gif/encode.rs:472`, where the full 256-entry palette prevents adding a
transparency slot. The report has no source/build receipt or test attribution.

The WebP lane's public basic-inspection
parity assertion now covers VP8 canvas width and height mismatches and VP8L
canvas width and height mismatches. The generated VP8L files are complete
`VP8X, VP8L, EXIF` RIFF images derived from `exif.webp`; their Pillow outcome is
`OSError: could not create decoder object`. The VP8 width fixture is pinned by
SHA-256 `9213d677987c5dd684e0d07d09efcb3922a624b8188706f2879b3dbfe385d7ea`,
the VP8 height fixture by
`8d9ebf1b4656176dae2627363d7f73cc99a27b4fdcfc74f14db917def973bf14`, the
VP8L width fixture by
`51127d31ba0f68d73000ffea0a8d962cc5408db4dea2df6188a70302e7b10fa4`, and the
VP8L height fixture by
`79a1c86ebd90824274d792f602d980e6dc1c01bb83a969ceb4e1ffb2b057c3a5`. Two
more full-file mutations start from the valid two-frame
`animated_sequence_rgba_keyframes.webp`: one clears the first nested VP8L
signature byte, and one sets its unsupported version bit while preserving
dimensions. Pillow returns the same decoder-object error for both. The public
decode matrix passes both rows across detection, inspection, verification,
still decode, and sequence decode; their input SHA-256 digests are
`a40cd40a7f492abe7c086323aefd25edd04e73291618154285e83b85d3ca4a4c` and
`6e51fc2edee8208762d8faa8e292993f165d715a3ba2e7a5cdaf9e629c2f75c8`.
Coverage MCP's full report filtered to WebP inspection and VP8L lossless
decoding contains no missing group in `inspect_extended_basic` or
`LosslessDecoder::read_frame_header`, though other WebP groups remain.
Source/build identity and test attribution are unverified in that report, so
the query is diagnostic evidence alongside the local parity pass.
The new `portable_i444_quality100_64x64` case checks a 64×64 8-bit 4:4:4
quality-100 frame with fixed 64-pixel partitions. The selected public
matrix row passes exact Pillow parity for all 12,288 RGB bytes; its input SHA-256
is `5ceb66b47bda43bec5c421222eeb3ef858c62702bc41833986fdf678f6867737` and its
output SHA-256 is `5e74b862ca69314d8ae9a3aa4b8f3af5651dfbd238ce922a7973c193e338f64a`.
The new `animated_lossy_global_halfblend_spatial_i444_b256x256`
row pairs identity and RotZoom global references in compound spatial neighbors.
Its four complete RGB frames and 100ms timing match Pillow, and the independent
loop bundle records an infinite-play sequence. Selected LLVM coverage records
five visits to each outcome at `motion.rs:1707`; the full report records 30/10.
Coverage MCP returns no missing branch group at that coordinate. These local
reports have no source/build receipt and list test status as unknown; the public
matrix row and full campaign pass. The new `coverage_h16x4_third_leaf_no_chroma_tx8x4_01` row pins
a 16×16 4:2:0 H4 with two TX8×4 luma residuals in its third no-chroma leaf
against Pillow output and passes the matrix. Coverage MCP still shows 26 missing
branch observations in
`decode_following_vertical_without_chroma`: production decodes this input with
the complete partition walker, so the row guards that public reconstruction
path without covering the narrower first-partition fallback. Its report has no
source/build or test receipt, so those locations remain unverified.
Three further AVIF parity rows add a 64x64 TX64-root split and two opposite
512x128 projected-motion window misses. All eleven decoded frames and native
loop observations match their pinned references. Coverage MCP reports no
missing branch group at `entropy.rs:8000` for the TX64 child offsets or at
`motion.rs:3351` for the temporal x-window checks. These reports have no
source/build receipt and label test status unknown; the successful full local
coverage run supplies execution evidence.
One more public sequence adds a textured 184×64 all-lossless 4:2:0 input with
fixed 32×32 AV1 partitions. Its final-frame edge block at pixel `(160,0)` has
a 24×32 visible extent and selects the clipped-lossless plan. The input hash is
`0a8c935fe67694fe576e7f21064eec4c428cffc2a05a3ff2d0be33e401a20c1d`; both
Pillow RGB frame hashes are retained in the AVIF loop oracle and exact public
sequence parity passes. The selected LLVM report observes the true outcome at
`entropy.rs:6925`; Coverage MCP reports the false outcome missing in that
single-row report. Its source/build receipt is unverified and test status is
unknown. The complete coverage run passes, with branch totals unchanged.
Two public parity rows exercise the AVIF ICC `prof` and `rICC`
properties; Pillow 12.2.0 accepts both fixtures with matching RGB pixels and
preserved profile bytes. Two further rows assert associated BT.2020/D65 `mdcv`
fields against the property definition while Pillow verifies identical pixels
and profile preservation, including when one bounded trailing metadata byte is
present. A new two-frame 60×64 lossless I420 sequence pins a B32×32 leaf at
pixel `(32,32)` with 28×32 visible pixels at the right edge; both complete
frames match Pillow exactly. New JPEG parity cases cover progressive
AC-first run overshoot, AC-refinement EOB runs and scan tails, repeated sparse refinement, a malformed
DC Huffman category-16 symbol, and out-of-range spectral starts and ends
rejected by Pillow during pixel loading, an oracle-accepted out-of-range DC
coefficient case matching signed JCOEF narrowing, plus two DC-only progressive cases
that exercise smoothing with ordinary and Pillow-tolerated zero quantization
tables. The matrix also includes a 33×33 baseline 4:2:0 encode with partial
MCU edges. Five additional encoder rows exercise the fused 4:2:0
batch path at 16×2, 16×1, 17×17, and 17×2 sizes, plus the generic 4:2:2 RGB
batch tail at 17×17. Four more JPEG fixtures cover partial baseline grayscale
and CMYK blocks, plus progressive 4:2:2 scans at 17×13 and 1×8. The odd 4:2:2
case exposed scan iteration over padded storage blocks; progressive scans now
use the logical component block extent while retaining the padded buffer
stride. Successful cases compare exact pixels or bytes against
Pillow. At that time, the all-feature JPEG decode lane passed all 154 active rows. The new
`baseline_ac_pair_tail` fixture exercises a baseline AC run whose first
coefficient position exceeds 63; its exact Pillow pixels pass the public
matrix. The bounded overrun path consumes the symbol amplitude and writes to
coefficient 63, matching libjpeg-turbo's padded zigzag behavior for corrupted
streams. Coverage MCP's query for `decode.rs:145` has no remaining missing
branch group in the full JPEG lane report. A selected incremental branch union
was incomparable because its exports lacked complete branch detail; the
parallel region comparison found 10 newly observed regions in
`decode_block_fast` against the other 153 JPEG decode rows. The reports are
`target/release-evidence/coverage-jpeg-baseline-excluding-ac-pair-tail.json`
and `target/release-evidence/coverage-jpeg-ac-pair-tail-selected.json`; the
full-lane report is `target/release-evidence/coverage-jpeg-ac-pair-tail-final.json`.
These reports
have no source/build receipts, so MCP marks their test status unknown; local
selected and full JPEG decode runs passed.

Three more Pillow-generated baseline JPEG rows exercise public restart-marker
fallbacks: 4:2:0 with DRI 3, which is not aligned to its eight-MCU row, and 4:2:2
and 4:4:4 with DRI 1. The fixture generator uses Pillow's
`restart_marker_blocks` option; the full 128×128 sampling grids avoid partial
MCU edges. All 157 active JPEG decode rows pass exact pixel parity. Coverage MCP
reports no remaining branch gaps at `decode.rs:1083`, `1330`, or `1494` in
`coverage-jpeg-restart-fallbacks-current.json`. That selected report has no
source/build receipt and marks test status unknown, so the result is bounded
path evidence; the local test output confirms 157/157.

A fresh selected all-feature run passes all 60 active JPEG encode rows,
including progressive CMYK and the CMYK EXIF cases below.

The public `enc_cmyk_progressive` row closes a Pillow-supported combination
that previously returned an unsupported error. The general progressive scan
script now covers each component of non-YCbCr images, and the optimized CMYK
row-streaming path remains restricted to baseline output. The exact Pillow
reference is 10,364 bytes with SHA-256
`53d7b0a56e5156b5b9a7ea14ef13fe5fc0fe88274b1d86efa5e6668fa1592814`; all 60
active JPEG encode rows pass. Coverage MCP reports no remaining branch-gap group
in `default_progression_script` for
`coverage-jpeg-progressive-cmyk-current.json`. Its source/build receipt is
unverified and test status unknown, so this does not claim a managed coverage
delta. The baseline CMYK streaming path remains available unchanged for
non-progressive output.

JPEG parity now also covers baseline CMYK streaming with valid and oversized
EXIF, restart-marker emission, the 65,536-MCU restart-interval clamp, and restart
rows above Pillow's signed limit, alongside the existing row without EXIF. The
full all-feature JPEG encode lane passes all 60 active rows. Coverage MCP reports
no missing branch at `restart_interval_from_rows`, and no missing region at the
APP1 error-propagation site on line 4813, the restart-marker emission site on
line 4866, or in the CMYK streaming function. A 4,096-row setting clamps to DRI
65,535; a setting of 2,147,483,648 omits DRI, matching Pillow. Pillow rejects the
65,534-byte EXIF payload with `ValueError: EXIF data is too long`, matching the
Rust parameter error. The current report has no source/build receipt and MCP
marks test status unknown; the local test output confirms 60/60. MCP findings are
limited to this JPEG lane and do not establish a full-repository coverage change.
At that point, the active JPEG decode inventory had grown to 158 rows. Its new
`baseline_cmyk_entropy_truncated_tail_64` case truncates 64 bytes of baseline CMYK
entropy while retaining EOI. Pillow returns the exact 13×9 CMYK output (468 bytes);
the direct-decoder fallback at `decode.rs:1065` is covered by the public row.
Coverage MCP returns no missing branch group at that coordinate in the current
full report, which has no source/build receipt and reports test status unknown.

The JPEG decode inventory now has 159 active rows. The new
`baseline_cmyk_height_partial_aligned_width` fixture is 8×9 CMYK: its width is
MCU-aligned while its final MCU row is partial. Pillow's exact 288-byte output
has SHA-256 `ac10bdd2898d8df3462b1594b15ade2e2a429a61ca2908a31102c64f27b579b9`.
Exact parity passes, and both outcomes of the AArch64 height-alignment check at
`decode.rs:1023` appear in the full local report. Coverage MCP returns no
remaining gap at that line; the LLVM report has no source/build receipt and MCP
labels test status unknown.

The `idct_high_horizontal_frequencies` row uses a restart-separated, fully
valid baseline grayscale JPEG. Six blocks place AC coefficients at horizontal
frequencies 3 through 7 and at vertical frequency 1. Its exact Pillow pixel
reference passes the public decode matrix. The selected LLVM report observes
both outcomes at each scalar IDCT row-shortcut predicate on lines 191–195;
Coverage MCP returns no remaining gap for those selected coordinates. The
local report has no source/build receipt, so this is selected-path evidence and
does not establish a complete-suite coverage delta by itself.

The 2026-10-01 all-feature `make coverage` run passes every executed test and
the release floors: 106,527/162,369 lines (65.607967%), 17,345/32,236 branches
(53.806304%), 5,341/9,371 functions (56.994985%), and 156,010/242,524 regions
(64.327654%). Its LLVM report is `target/release-evidence/coverage.json`, measured
on macOS ARM64 with `nightly-2026-07-16` against the current working tree. It
has no source/build receipt: Coverage MCP reports source identity unverified
and test status unknown, so its totals cannot establish a source-bound delta or
per-row coverage gain. Coverage MCP groups the remaining branch observations in
the AV1 lossless partial-grid predicate at `block.rs:54830-54831`; the targeted
`decode_following_vertical_without_chroma` query still returns 26 observations.
A selected 2026-10-04 run of the public 64×64 10-bit fixed-32-partition row
records six false outcomes at the current `block.rs:54838` sample-depth check,
so that outcome is reachable through a valid full-size `LosslessGrid`. This
selected witness does not identify the older report's remaining observations
at lines 54830–54831; both reports lack source/build receipts, so their line
coordinates cannot be treated as source-bound branch identities. The 100% goal
remains open, with 55,842 lines, 14,892 branches, 4,030 functions, and 86,514
regions uncovered.

While validating the three-frame I444 parity row, a DAV1D trace exposed a
transposed compound-reference CDF lookup. Rust indexed the AV1 `[group][context]`
tables as `[context][group]`, selecting `Last3` where the stream selected `Last`.
The decoder now indexes group then context; the selected animated I444 parity
row and the full test campaign pass. Coverage MCP reports 207 newly observed
branch coordinates against the prior report, but source/build receipts are
missing, so MCP marks that comparison limited rather than a verified gain.
Its highest missing group remains
`Lossy420Decoder::decode_inter_translation_impl`, with 1,842 repeated
coordinates across monomorphizations, not 1,842 distinct source branches. The
100% goal remains open.

Two odd-sized baseline color JPEG rows now cover right and bottom edges in
separate sampling modes: 13×9 4:4:4 and 17×9 4:2:2. Both compare exact decoded
pixels with the pinned Pillow oracle. The 4:4:4 row also drives a short RGB
conversion tail through the direct row kernel, extending coverage beyond the
existing odd-sized progressive 4:2:2 case.
The AVIF additions include four-frame 4:2:2 and 4:4:4 sequences. The 4:2:2
case moves a small block across horizontal chroma boundaries; the 4:4:4 case
translates a periodic full-frame chroma pattern. A three-frame 4:2:0 sequence
with forced 4x4 inter partitions and a small luma/chroma change exercises the
subsampled block-width ownership edge. Exact Pillow frame references and pinned
libavif loop witnesses pass the public sequence checks.

A 96×96 untransformed VP8L lossless case repeats a four-pixel RGB phrase at
short distances. Its exact Pillow reference passes public decode parity. The
isolated and full local branch reports observe the short non-overlapping copy
arm; Coverage MCP reports new branch coordinates for this input, with source
and build identity unverified.

A deterministic 128×64 VP8L stream exercises color-cache lookahead after a
cached color, a new literal, and a repeated cached color. The exact RGB output
matches Pillow, and the isolated public report observes both outcomes at the
three lookahead branches in `lossless.rs:828-830`; Coverage MCP reports no
remaining gap for those selected coordinates, but source/build receipts are
unverified. The new active row is `lossless_color_cache_lookahead`.

The public GIF encode matrix now includes a one-frame sequence with exactly one
total play. Its Pillow byte reference is unchanged because no GIF loop extension
is required. The selected parity row reaches the `Finite { total_plays: 1 }`
mapping in `encode.rs:1734`; the selected report has no remaining gap at that
coordinate, with source/build identity unverified.

Three complete 3×1, 4-bpp ICO fixtures pin Pillow's grayscale-palette rules:
identity grayscale becomes raw luminance bytes, non-identity grayscale remains
indexed, and a black/white two-entry palette uses the one-bit interpretation.
All three decode, inspect, and verify through the public manifest parity path.
The identity fixture's input SHA-256 is
`de23f5d6d7ec9047cc6103e2991db5b62fde8cbd9b3d95e078ccf3960d1f5b29`, and its
Pillow RGBA reference SHA-256 is
`45add7ffebacd45767e5608c10f43c1aab79268a2645bfe9f5ab759656af4c22`. The
selected LLVM report observes both outcomes at the odd-nibble guard; Coverage
MCP reports no remaining gap there, with source/build identity unverified.

A new two-frame 16×16 4:2:0 AVIF sequence moves a 4×4 patch by one pixel.
Both frames match Pillow exactly. Its inter frame is all-lossless and selects
the specialized `LosslessB16I420` reconstruction path; a branch-instrumented
public decode observed the previously missing false arm at
`decode_inter_transform_size`'s small-block eligibility check.

A four-frame 256×256 4:2:0 AVIF sequence with moving inter-frame content covers
the large-block side of that eligibility check. Its frames match Pillow and
pinned native loop witnesses. The row also exposed a decoder-state bug: a
skipped transform block inside a non-skipped coding block must propagate its
own `txb_skipped` state to the wide-luma terminal decoder. Passing the enclosing
block state could consume coefficient symbols for an empty transform and
overread the tile; the corrected path passes the per-transform state.

A new 16×16 lossy WebP fixture uses luma-balanced red and green 4×4 tiles.
It reaches the VP8 Y2 residual path with no coefficients, so both neighboring
macroblocks receive a zero Y2 complexity context. The exact Pillow RGB output
matches the public Rust decoder, and targeted LLVM evidence observes the
previously missing false arm at `read_residual_data`.

A 32×32 four-quadrant lossy WebP row extends the public VP8 chroma decode
corpus with a 2×2 macroblock layout and passes exact Pillow output parity. Its
selected run reaches the production chroma DC prediction path. The saved full
LLVM report `coverage-after-adam7-gray16.json` already records hits for that
arm, and the MCP incremental comparison is incomparable, so no coverage gain
is claimed; the row is retained as a decode regression case.

Three further 32×32 lossy WebP rows target VP8 chroma prediction with mixed
neighboring blocks, a horizontal color gradient, and a vertical color
gradient. All three pass exact Pillow RGB parity. Their isolated reports reach
the production `Vp8Decoder<Take<&mut Cursor<&[u8]>>>` specialization's
TrueMotion, Vertical, and Horizontal predictor arms. The aggregate coverage
counts did not increase because the existing full matrix already covered those
arms; the cases add focused regression inputs, not a claimed coverage gain.
Coverage MCP has no source/build receipt for these local reports, so its source
identity is unverified and selected-test status is unknown.

The malformed WebP row
`error_malformed_container_alpha_uncompressed_truncated_payload` starts from
`alpha_uncompressed.webp`, shortens the raw ALPH plane from 4,096 samples to
one, and updates both chunk and RIFF lengths so the container stays complete.
Pinned Pillow 12.2.0 opens and structurally verifies it, then reports
`OSError: failed to read next frame` during load. The public Rust parity row
passes with the matching malformed error category. It exercises the
`Take<&mut Cursor<&[u8]>>` production specialization at
`read_alpha_chunk`'s `read_exact` in `extended.rs:442`; the prior full report
listed that exact region as missing. The current full report and selected
report show it executed. The row-only LLVM report is
`target/release-evidence/coverage-webp-alpha-truncated.json`. MCP comparison
remains coordinate evidence only because its source/build identity is
unverified and test status is unknown.

The companion malformed row
`error_malformed_container_alpha_lossy_gradient_empty_compressed_payload`
keeps the compressed ALPH header and intact lossy VP8 image while removing the
compressed alpha bitstream, then repairs the chunk padding and RIFF length.
Its input SHA-256 is
`dbacedb09b0aec8375255d8d5b509fde9063b018b3497764769e8ea5aa93ba52`.
Pinned Pillow 12.2.0 opens it as 64×64 RGBA and raises
`OSError: failed to read next frame` on load. Its public parity row passes with
the malformed category, and a selected LLVM report records the production
`Take<&mut Cursor<&[u8]>>` specialization without a remaining gap. Comparing
the old and refreshed full reports observes one new region at
`read_alpha_chunk`'s lossless decoder call in `extended.rs:433`; that MCP result
is limited coordinate evidence because source/build receipts are absent.
The selected report is
`target/release-evidence/coverage-webp-alpha-lossy-truncated.json`.

A further malformed WebP row,
`error_lossless_vp8l_short_backreference_truncated_tail`, starts from the active
96×96 RGB `lossless_short_backreference.webp` stream and removes its final two
VP8L payload bytes while repairing both the chunk and RIFF sizes. The complete
13,924-byte container has SHA-256
`6c5dac54aae71b6c8e086096acfd00e4370689f955aa66bbe0227eb06198dd83`.
Pinned Pillow 12.2.0 opens it as RGB and raises `OSError: failed to read next
frame` on pixel load; the public Rust parity row passes. The full coverage
totals did not change. MCP's full-report comparison identifies one newly
observed region in the direct RGB VP8L pixel path at `lossless.rs:754:72`, but
the report has no source/build receipt and the result remains coordinate-only.
The row is retained as a malformed-input regression for a valid header and
short-copy stream that fails during pixel reconstruction, not as an aggregate
coverage increase. Its full report is
`target/release-evidence/coverage-after-webp-avif-shortbackref.json`.

The AVIF compound-reference investigation found a CDF table-axis error that
could panic on a valid context-2 stream. The correction indexes each CDF by
branch group then context. A deterministic full-file AVIF mutation preserves
the compound-reference prefix and damages only a later coded sample; Pillow
and Rust both reject sequence decoding as malformed, while the first image
still matches Pillow exactly. An isolated public `decode_sequence` trace stops
at the corrected backward-reference CDF lookup with group 0/context 2; the old
transposed access would index beyond that table's two group rows. The selected
parity row then returns its structured malformed result. This access occurs in
an earlier AV1 sample than the mutation at byte 16098, so it proves the public
prefix exercises the fix but does not attribute the eventual sequence error to
the changed byte or frame. The valid DIFFWTD stream still reaches other AV1
decoder limitations, so it is not counted as a successful parity case.

An odd-sized 9×9 4-bit grayscale PNG uses Adam7 with all seven passes carrying
samples. Exact Pillow L8 parity exercises packed-sample reconstruction in the
interlaced pass callback, complementing the existing non-interlaced low-depth
fixtures.

A three-page RGB TIFF sequence now uses 9×7, 9×7, and 5×11 pages. The public
sequence decoder matches Pillow for all three frames, and the encoder matches
Pillow's complete byte stream. This extends IFD traversal and inter-page
alignment coverage through a third page with a different extent. The two new
rows add regression coverage but leave the full LLVM totals unchanged; the
Coverage MCP incremental comparison is incomparable because the prior report
does not retain compatible branch detail.

A deterministic 128×64 monochrome AVIF with two tiles now passes exact Pillow
RGB pixel parity in both one-group and split-group forms. The split case sends
tile 0 and tile 1 in separate tile-group OBUs and exercises the accumulated
pending-plus-trailing tile path. A new two-tile AVIF with a monochrome alpha
item and CDEF disabled reaches the CDEF-off reconstruction branch; the complete
RGBA output matches Pillow byte-for-byte.
The new row `multitile_monochrome_alpha_zero_loop_filter` keeps that two-tile
monochrome alpha layout but makes its alpha frame all-lossless, with zero Y/U/V
loop-filter levels. Its generator checks both tile indices, CDEF, lossless state,
and filter levels before accepting the fixture. Exact Pillow RGBA parity passes;
the selected LLVM report observes the missing false arm at
`assemble_monochrome_tiles` in `frame.rs:2246`. That report has no source/build
receipt, so the location remains coordinate evidence. The selected report is
`target/release-evidence/coverage-avif-monochrome-alpha-zero-loop-filter.json`.

A sparse grayscale progressive JPEG pins a 64-ZRL AC-refinement scan with
coefficient correction bits and passes exact Pillow L8 pixel parity. A second
8×8 grayscale row, `progressive_ac_refinement_refine_zrl`, includes refinement
symbols with `r=15, size=0` and also matches Pillow exactly. Its selected
nightly LLVM report records 8 true and 16 false executions at
`ac_refine_block`'s `r != 15` branch; Coverage MCP lists no remaining gap for
the production `FastBitReader` specialization. The full ARM64 report still
lists the generic `BitReader` monomorphization, which this target does not use
for production JPEG decode. The isolated report has no source/build receipt,
so its branch location is coordinate evidence; the local public parity run
passed. This edge case adds regression value without increasing the aggregate
branch count.

An earlier full all-feature `make coverage` run passes every executed test and
remains above all project floors: 100,225/163,401 lines (61.3368%),
15,892/32,398 branches (49.0524%), 5,085/9,372 functions (54.2574%), and
146,750/243,772 regions (60.1997%). The local LLVM report is
`target/release-evidence/coverage-goal-lossless-inter-420-final.json`; it is
not a managed Coverage MCP snapshot. It was measured on the macOS ARM64
working tree on 2026-09-30 with `nightly-2026-07-16`, all Cargo features, and
the complete `make coverage` test campaign; it is not bound to a clean commit
or a source/build receipt. Compared with the prior full local report
`target/release-evidence/coverage-goal-cdef-disabled.json`, the measured totals
show +171 covered lines, +70 branches, +11 functions, and +203 regions. The
new lossless inter-frame AVIF parity row passes the full Pillow pixel check and
observes the eligible lossless transform-size branch in the public decoder.
Coverage MCP's report-coordinate comparison lists 173 newly observed branch
locations across monomorphizations; its reports lack source/build receipts and
test status, so the comparison is limited and does not establish verified
attribution or regressions. The local full campaign itself reports all tests
passing. The 100% target remains open.

The preceding all-feature `make coverage` run passed 118 library unit tests, 3
binary tests, 4 animation-loop tests, 1 capability test, 59 coverage-matrix
tests, 7 decode-policy tests, 1 determinism test, and 68 feature-gate tests.
It reports 106,798/165,795 lines (64.4157%), 16,435/32,508 branches
(50.5568%), 5,578/9,617 functions (58.0015%), and 156,910/247,495 regions
(63.3993%). The local LLVM report is
`target/release-evidence/coverage-goal-jpeg-jcoef-dc-shortcut-final.json`,
measured on macOS ARM64 with `nightly-2026-07-16` and all features. Unit tests
were included after enabling the existing library test target. Coverage MCP
queries on this report no longer list branch gaps for JPEG `range_limit`,
`ycc_to_rgb_batch`, `reconstruct_baseline_444_direct_safe`, or
`jpeg_idct_islow`. The largest remaining group is AV1
`decode_inter_translation_impl`: MCP repeats 4,063 observations at the same
source spans across specialized monomorphizations, not 4,063 distinct source
statements. MCP marks source as unverified and test status unknown, so these
are report-coordinate findings; the local Cargo campaign itself passed. All
four floors pass, while 100% remains open.

An earlier all-feature `make coverage` run passes 119 library unit tests, 3
binary tests, 4 animation-loop tests, 1 capability test, 59 coverage-matrix
tests, 7 decode-policy tests, 1 determinism test, and 68 feature-gate tests.
It reports 106,929/165,923 lines (64.4450%), 16,441/32,508 branches
(50.5752%), 5,580/9,619 functions (58.0102%), and 156,956/247,537 regions
(63.4071%). The local LLVM report is
`target/release-evidence/coverage-goal-av1-has-chroma-final.json`, measured on
macOS ARM64 with `nightly-2026-07-16` and all features. The new table-driven
AV1 test covers monochrome, 4:4:4, 4:2:2, and 4:2:0 ownership at odd/even
origins and one-MI versus larger block axes. Coverage MCP has no remaining
branch gaps for `inter_block_has_chroma`; the rule is `#[inline]` in release
builds. Its next AV1 branch group is `lossy_i422_narrow_mode0`, the
4:2:2-only 4×8/4×16 mode-0 specialization. That group has 1,916 repeated
observations across specialized monomorphizations; they are not distinct
source statements. The full report improves by 131 covered lines, 6 branches,
2 functions, and 46 regions over the preceding full report. MCP reports source
as unverified and test status as unknown, so its gap listings are
report-coordinate findings; the local Cargo campaign itself passed. All four
floors pass, while 100% remains open.

An earlier selected LLVM run of the 59-test `coverage_matrix_tests`
integration target records 97,978/163,390 lines (59.9657%), 15,457/32,404
branches (47.7009%), 4,867/9,369 functions (51.9479%), and 143,525/243,760
regions (58.8796%). It clears the line, branch, and region floors and is five
functions short of the 52% function floor. Every successful JPEG decode input
row also passes through the public token-aware decoder and matches its Pillow
pixel reference. Every successful JPEG encode row passes through both ordinary
and token-aware encoding, and both byte streams match the committed Pillow
output exactly. Every successful JPEG encode roundtrip also passes its encoded
output through the token-aware decoder and compares its pixels with the Pillow
reference, including the 1,024-MCU checkpoint in the 2048×1024 grayscale case.
Coverage MCP reports that checkpoint branch observed. Its selected report
also listed the false arm of the AArch64 dequantization-overflow fallback.
Source review shows full-file parsing bounds progressive coefficients to i16
and DQT entries to u16; the maximum product, 2,147,450,880, fits in i32. That
fallback protects internal states outside the parser path and has no valid
full-file JPEG parity witness. The selected report has no branch-gap groups for
`baseline_frequencies_with_checkpoint`, `append_ac_refine_events`, or
`encode_baseline_420_mcu_pair`.
The MCP report has no source/build receipt and marks test status unknown, so
these are report-coordinate observations; the local Cargo command itself
reported 59/59 passing. A Coverage MCP selected comparison against the previous
selected report shows seven newly observed branch observations, including both
`find_entropy_end` arms, the generic CMYK branch in `reconstruct_image`, and
PNG opaque-chunk/reserved-bit paths. The source/build receipts are unavailable,
so these are coordinate-only comparisons. The selected target does not describe
whole-workspace coverage. The 100% target remains open.

The selected JPEG additions cover baseline CMYK reconstruction with restart
markers and two missing-EOI tails: a dangling `0xFF` and a stuffed `0xFF00` at
EOF. Pillow error contracts remain exact, and the selected comparison observes
both `find_entropy_end` arms plus the generic four-component branch.

The same selected run adds Pillow-parity PNG cases for duplicate gAMA, sRGB,
cHRM, and iCCP chunks; invalid and short sRGB payloads; malformed short cHRM;
an empty-key iCCP chunk; and valid, invalid, and edge compressed text metadata.
It also covers a Pillow-tolerated unknown ancillary chunk with a lowercase
reserved bit in static PNG and APNG inputs, preserving exact pixels and frames.
`text_chunks.png` now contains tEXt, zTXt, and iTXt. All active PNG decode rows
pass exact pixel comparison. Typed source-color assertions cover valid values
and duplicate chunks where Pillow and Rust expose the same declared color.
Malformed short sRGB and empty-key iCCP rows assert pixels only because their
Pillow metadata differs from the decoder's spec-aware typed `SourceColor`.
Coverage MCP's selected comparison reports 10 newly observed branch
observations in `retain_color_chunk`, with three false arms still uncovered;
its gap query lists no remaining branch group for
`invalid_compressed_metadata_identity`. The remaining color-chunk arms require
short gAMA data, an iCCP keyword without a NUL terminator, or an iCCP method
without profile bytes. Pinned Pillow rejects those images, so they cannot be
added as parity fixtures. The coverage report lacks source/build receipts and
MCP reports test status unknown; the local run passed 59/59. This remains
selected-target evidence, not whole-workspace coverage.

Coverage MCP reports no remaining branch-gap groups in
`encode_baseline_420_mcu_pair` after the edge row and removal of redundant
first-column checks implied by the MCU loops. The all-feature run's gap query
listed 12 remaining AC-refinement branch observations for `BitReader`; the
`FastBitReader` group had disappeared after the repeated sparse-refinement row
and once-per-scan bound check. AV1 `decode_inter_translation_impl` groups
remained the largest branch gap in that run. After adding the AC-first narrow-tail fixture and matching
libjpeg-turbo's padded-table write behavior, Coverage MCP no longer lists the
`FastBitReader` AC-first or AC-refinement gap groups. The malformed
category-16 fixture also closes the fast-reader `dc_first_block` error group.
The selected ARM report still shows generic `BitReader` groups for those
functions; the public JPEG path selects `FastBitReader`. This is selected-run
evidence, not a full-suite result. The new `Ss=64` and `Se=64` JPEG rows
reach both scan-range error paths. The larger 128×128 JPEG scan-tail row adds
multi-block parity coverage. MCP reports have no source/build receipts, so
their comparisons and gap listings are report-scoped evidence, not standalone
source-provenance claims.

The selected AVIF report passed all active AVIF matrix rows. Coverage MCP's
query at `block.rs:55046` dropped from ten to nine missing specialization
groups, and the previously unobserved block-width branch recorded both sides
in the selected AVIF lane. The local report is
`target/release-evidence/coverage-parity-avif-current.json`; MCP reports lack
source/build receipts and report test status as unknown, so this remains scoped
coverage evidence rather than a full-suite claim.

The row-selected JPEG encoder report exercised the new micro-image cases.
Coverage MCP compares the five-case report with the earlier two-case batch
report and reports five newly observed branch arms: the aligned/even-height
decision, the 4:2:0 row remainder and even-height fallback, and both arms of
the generic RGB conversion loop. Gap queries show no remaining observations at
those locations; the fused helper's two zero-dimension guard arms remain.
Public zero-sized JPEG inputs fail validation before this helper. These local
LLVM reports have no source/build receipts and MCP test status is unknown, so
their branch comparison is limited selected-run evidence, not an aggregate
coverage or regression claim. Direct all-feature parity runs passed both JPEG
decode and encode matrix tests. Reports are under
`target/release-evidence/coverage-parity-jpeg-kernel-edges-final.json` and
`target/release-evidence/coverage-parity-jpeg-420-batch-cases.json`.

The selected decode report's largest group is AV1
`decode_inter_translation_impl`. Raw LLVM data shows ten monomorphized closure
instantiations at the reported location, with execution recorded in two for
this run. Coverage MCP's 1,928 observations therefore span unvisited specialized
paths; they are not 1,928 distinct source statements. A useful next parity pass
needs to identify the missing supported AV1 syntax paths before selecting or
generating fixtures.

Two local release A/B runs per source version exercised the public JPEG
benchmark matrix on an Apple M3 Pro, with three alternating rounds per run.
Across 63×65, 127×129, 128², 512², and 1024² RGB 4:2:0 encodes, the median of
the run medians was 5.3–7.8% lower after the MCU-check cleanup; Rust output
lengths and hashes matched before and after in all five cases. One baseline run
started with a 1-minute load average of 10.7, so these are directional local
measurements rather than a cross-machine or release speed claim.

A separate release profile used the public Rust runner for a 1024×1024 RGB
4:2:0 encode (2,000 measured iterations after 100 warmups) on the same M3 Pro.
The median was 6.806 ms and P95 was 7.200 ms. A five-second `sample` capture
recorded 3,866 stacks; `rgb_to_ycbcr_420_packet` appeared in 340 stacks (8.8%)
and the four-block FDCT in 221 (5.7%). This is a single-workload diagnostic
that points to color conversion for follow-up profiling. It is not a paired
speedup measurement, and it provides no SSE2 or AVX2 runtime evidence.

A same-checkout A/B isolated `#[inline(always)]` on
`rgb_to_ycbcr_420_packet`. Seven alternating release-runner pairs on the M3 Pro
measured 128², 512², and 1024² RGB 4:2:0 encodes. The candidate medians were
2.61%, 0.34%, and 3.18% lower, respectively; every encoded length and hash
matched. These small local gains support retaining the hint. The run does not
measure SSE2 or AVX2 execution. Raw paired results are in the ignored local
artifact `target/benchmarks/jpeg/goal-continuation-2026-09-29/packet-inline-controlled-ab.json`.

A follow-up experiment replaced four overlapping 8-pixel conversions in each
16-pixel RGB row with three 16-byte loads and mask-based channel deinterleaving.
The public JPEG encode parity lane passed and all 20 encoded outputs matched the
stored baseline hashes and lengths. However, a five-round full JPEG matrix on
the M3 Pro measured an encode median ratio of 1.0031 and a geometric-mean ratio
of 1.0067 versus baseline; RGB 512² and 1024² 4:2:0 encodes regressed 5.15% and
3.69%. The packet-body change was reverted. This result is directional ARM64
evidence only, not an SSE2 or AVX2 runtime measurement. The local candidate
report is `target/benchmarks/jpeg/goal-20261001-arm64-3load-candidate`.

A bounded public-path CMYK experiment streams each decoded component block into
its final channel for aligned baseline 4:4:4 images on AArch64. Five
alternating release rounds reduced median latency by 21.13% at 512×512
(6.187 ms to 4.880 ms) and 18.24% at 128×128 (0.254292 ms to 0.207917 ms);
output hashes matched in every round. The helper keeps the existing packet path
for partial edges and restart scans, and is not compiled for x86, where the
existing SIMD path is unchanged. The selected Pillow decode matrix passed all
three active CMYK cases, including the aligned fast-path case, odd partial
blocks, and restart rows. `cargo fmt --check`, strict release library Clippy,
and `git diff --check` passed. Round data and the exact candidate patch are in
`target/jpeg-cmyk-fusion-experiment/`; these are local Apple M3 Pro measurements,
not SSE2/AVX2 runtime evidence.

The preceding full local `make coverage` campaign on 2026-09-30 passed every
executed test and the unchanged release floors: 100,431/163,617 lines
(61.3818%), 15,947/32,444 branches (49.1524%), 5,097/9,395 functions
(54.2523%), and 147,132/244,178 regions (60.2560%). Its LLVM report is
`target/release-evidence/coverage.json`; it was measured on macOS ARM64 with
`nightly-2026-07-16` and is not bound to a clean commit. The fresh Coverage MCP
gap query still lists AV1 `decode_inter_translation_impl` as the largest
branch group. MCP source/build receipts are unavailable and test status is
unknown, so these locations are report-scoped observations rather than verified
test attribution or a managed snapshot.

The AV1 gap query at `block.rs:55105` returns repeated
`decode_inter_translation_impl` observations across specializations. The
apparent lossy I422 narrow path requires transform mode 0 and a non-lossless
segment. Valid frame parsing sets mode 0 only when `all_lossless`, and derives
that flag as the conjunction of every segment's lossless state, so no valid
frame can satisfy the helper's `!segment_lossless && transform_mode == 0`
predicate. This is an unreachable implementation branch, not a missing public
fixture; no parity row should be invented to exercise it. A separate reachable
candidate is diff-weighted compound prediction at `block.rs:57292` and
`:57310`. No candidate is accepted without independent syntax inspection and
exact Pillow/native frame parity.

An earlier full `make coverage` run passes all executed tests and project
floors: 101,330/163,618 lines (61.9308%), 16,104/32,444 branches (49.6363%),
5,124/9,395 functions (54.5396%), and 148,460/244,179 regions (60.7997%).
This run includes the Adam7 4-bit grayscale PNG and large-motion 4:2:0 AVIF
parity rows. The AVIF sequence row exposed and now covers propagation of the
per-transform skip flag to the wide-luma terminal decoder; all active AVIF
decode rows pass. The 59-test integration target, 68 feature-gate tests,
7 decode-policy tests, 4 animation-loop tests, 3 binary tests, and the
determinism test pass. The local report is
`target/release-evidence/coverage.json`, from macOS ARM64 with
`nightly-2026-07-16`; it is not bound to a clean commit. Coverage MCP reports
16,340 uncovered branch observations and keeps AV1
`decode_inter_translation_impl` as the largest group, with repeated hits
across monomorphizations. It reports no remaining gap at the large-block
eligibility check in `motion.rs:2278`. MCP source/build receipts are absent and
test status is unknown, so these are report-coordinate findings; the local
full campaign supplies the passing test evidence. The 100% target remains open.

The preceding full `make coverage` campaign on 2026-09-30 passes all executed
tests and the release floors: 104,438/163,657 lines (63.8152%),
16,770/32,444 branches (51.6891%), 5,251/9,398 functions (55.8736%), and
152,993/244,210 regions (62.6481%). The local report is
`target/release-evidence/coverage.json`, measured on macOS ARM64 with
`nightly-2026-07-16`; it is not bound to a clean commit. Coverage MCP reports
15,674 missing branch observations, with the largest group at AV1
`decode_inter_translation_impl` (`block.rs:55105`); repeated specialization
records are observations, not distinct source branches. At the WebP Y2
`read_residual_data` false edge, LLVM records 57 executions in a covered
specialization and a duplicate zero-count generic record, while Coverage MCP
still lists missing branch records at that coordinate. Keep that edge marked
ambiguous in MCP rather than calling its gap closed. MCP reports source/build
identity as unverified and test status as unknown, so its results are
report-coordinate findings; the local full campaign supplies the passing test
evidence. `make lint`, `make lint-x86-simd`, `make test-feature-matrix`, and
`make verify` also pass. SSE2/AVX2 checks validate formatting and compilation;
they do not establish runtime speedups. The 100% coverage and fastest-decoder
goals remain open.

A Coverage MCP query against that earlier report found no supportable new
non-AV1 parity fixture. Its JPEG `ac_refine_block<BitReader>` gaps are for the
x86 decoder specialization; this ARM64 run uses `FastBitReader`, whose public
progressive ZRL cases are already in the parity corpus. The reported
`progressive_dequantize_block` fallback only follows a DC-times-quantizer
overflow, which valid parser bounds prevent. These are target-specific or
unreachable observations, not reasons to add duplicate or invalid fixtures.

The AV1 `decode_following_vertical` gap was checked with selected public parity
rows on 2026-09-30. `coverage_h4_horizontal_bands` and
`coverage_h16x4_predictor_adst_dct_01` both pass the matrix parity test, but a
fresh branch-instrumented run of the latter records zero calls to
`Lossy420Decoder::decode_following_vertical`. The public decoder therefore
does not exercise this bounded first-leaf helper for those H4 streams, despite
their encoded topology. Source inspection shows its only callers are the
bounded 4:2:0 split helpers. Those callers fix chroma sampling to 4:2:0 and pass
no full-resolution edges, so the method's Full-only entry checks cannot be
covered by a valid public fixture on the current dispatch path. Coverage MCP
also reports both outcomes missing for the `full_strict` condition at
`block.rs:67502`: the 4:2:0 leaf helpers hardcode `Subsampled420`, and their
closed-frame callers require 8-bit input. The generic decoder's Full/high-depth
following path uses `decode_following_from_edges` instead. Keep this gap open
until a supported public path can exercise it or the obsolete path is removed
with a behavior-preserving refactor; do not add a parity fixture that merely
matches the topology label. Selected reports are
`target/release-evidence/coverage-h4-horizontal-bands.json` and
`target/release-evidence/coverage-h16x4-predictor.json`; Coverage MCP marks their
source identity unverified and test status unknown.

A fresh 2026-10-01 Coverage MCP read of `target/release-evidence/coverage.json`
reports 1,512 missing-branch groups and 14,972 missing branch observations.
The `decode_following_vertical` group has 342 repeated observations across
monomorphizations; filtering to `block.rs:67501` isolates the two missing
outcomes of its `full_strict` condition. Fresh selected parity profiles for
the H4/H16 fixtures and the 16x16 4:2:0 SPLIT fixtures all passed, yet recorded
zero executions across the method and its 102 closure records. This confirms
that their encoded partition labels do not reach this helper on the public
dispatch path. Coverage MCP still marks report source identity unverified and
test status unknown, so these are coordinate-only observations, not a verified
coverage gain. No duplicate fixture was added.

On 2026-09-30, a portable `u32x4::widening_mul` experiment for four-lane JPEG
reciprocal quantization preserved Rust output lengths and hashes across all 40
rows of the public JPEG benchmark matrix. Two five-round full-matrix repeats
per implementation, run in opposite order on the aarch64-apple-darwin host,
showed the candidate's 1024×1024 RGB 4:2:0 encode median 9.63% and 14.02%
slower than baseline; the median slowdown across the 20 encode workloads was
10.69% and 15.37%. The candidate was removed. Results are local directional
evidence only and do not measure SSE2 or AVX2 runtime performance. Raw reports
are under the ignored local artifacts
`target/benchmarks/jpeg/goal-quantizer-{baseline,candidate,candidate2,baseline2}-2026-09-30/`.

A subsequent all-feature `make coverage` campaign on 2026-09-30 passes every
executed test and the release floors: 104,471/163,650 lines (63.8381%),
16,807/32,432 branches (51.8223%), 5,251/9,398 functions (55.8736%), and
153,027/244,201 regions (62.6644%). The local report is
`target/release-evidence/coverage.json`, measured on macOS ARM64 with
`nightly-2026-07-16`; it is not bound to a clean commit or a managed Coverage
MCP receipt. The 100% target remains open.

The WebP matrix adds a 32×16 public parity row with a flat macroblock beside a
deterministic high-frequency macroblock. Its exact Pillow output passes the
selected decoder matrix test. The selected LLVM report
`target/release-evidence/coverage-webp-macroblock-candidate.json` invokes the
`Cursor<&[u8]>` specialization of `Vp8Decoder::read_residual_data` and records
both outcomes at the plane-selection and luma complexity-update branches.
Separate selected runs of the existing public rows
`lossy_vp8_lossy_macroblock_residual_context_edges_32x16_q75_m4` (input
SHA-256 `9bbe53e9597854a1f5412ab40b1ee9f54bcf56f6c8513d254d69dee85fa74abb`)
and `lossy_vp8_lossy_y2_empty_ac_16x16_q90_m4` (input SHA-256
`e5e6cc56cf73aded4521a16de4ef0d08ff4066db75cdcd5527b15e6210d4ea98`) both
pass exact Pillow pixel parity. Their row-only LLVM reports record `n=true`
and `n=false`, respectively, at `src/codecs/webp/native/vp8.rs:1583-1584` in
the production `Vp8Decoder<Take<&mut Cursor>>` specialization. The reports
have SHA-256 `4723f808dd1331cf4b0cd92639aaab47958c5f0dee08a0ef356da1dacd146109`
and `7c0769e6f8fa66ab048c442e6552b865f128bfabd60d670e2590d4430064f1b9`.
The aggregate MCP miss is in the separate `Vp8Decoder<Cursor<&[u8]>>`
specialization; MCP source/build identity is unverified and selected-test
status is unknown, so this is coordinate guidance rather than a source-bound
coverage claim. No private-only coverage test was added.

A 128×64 lossless RGB WebP row, `lossless_color_cache_lookahead`, covers the
VP8L color-cache lookahead branches through the public decoder. Its deterministic
512-color input has SHA-256
`920481d0986bf9034730ebdd4678c65540c985dc00c25c0cc293d45167b3e541`; the
Pillow 12.2.0 RGB8 reference has SHA-256
`d35118946b17473c290fc38351acb637a8887d4248de5052a3e6e2f305ddd35c`. The
independent VP8L inspector finds one untransformed stream, cache width 10, and
6,252 cache lookups. The selected all-feature matrix row passes exact Pillow
parity. Its isolated local LLVM report records both outcomes at
`lossless.rs:828` (10,572/93), `:829` (10,113/459), and `:830` (8,091/2,022),
true/false respectively. This is selected-row evidence, not a refreshed
Coverage MCP measurement or complete coverage claim.

The same change simplifies AV1 lossless inter-transform selection to use its
prevalidated geometry predicate directly, removing a repeated exact-geometry
check and duplicate block/layout guards. The matrix adds a two-frame 8-bit
monochrome lossless inter sequence with a translated patch. Its generator
disables warped motion to stay within the decoder's admitted reconstruction
profile; exact Pillow frame/timing parity and pinned native loop evidence pass.
Coverage MCP reports no missing branch observation at
`decode_inter_transform_size` line 6950; the true arm at line 6951 remains
missing at that checkpoint. The report's source/build identity is unverified
and test status is unknown, so this is coordinate evidence rather than an
attributed MCP gain.
Strict Clippy now denies `missing_errors_doc`; the eight
previously undocumented public `Result` APIs have `# Errors` sections, and
`make lint` passes. This lint and documentation add no runtime work. No SSE2 or
AVX2 runtime benchmark was available on this ARM64 host, so no x86 speedup is
claimed.

The 8-bit color lossless inter-transform gap has public parity fixtures:
`animated_lossless_inter_420_b32x32.avif` uses a translated 4x4 patch inside an
exact-visible 32x32 block and is pinned to input SHA-256
`65e8617044f7f12db081f65276235296f8dc208a006451210ebb9603f27f20a7`; the new
`animated_lossy_split_inter_420_b16x16.avif` exercises a lossy B16x16 transform
split and is pinned to input SHA-256
`9eb50f5a45dc2eb549c62ac9bcb67abd931a977076685bcf3c0f69e6827ce323`.
`animated_lossy_wide_monochrome_b128x128.avif` is a four-frame 8-bit
monochrome sequence with three lossy B128x128 inter blocks using switchable
transform splits; it is pinned to input SHA-256
`eb8dda5000882ffd03c182a817e08fc944ef110afa1e78aaf1678bef5efc65bf`. All
three fixtures match Pillow frame pixels exactly, and native sequence evidence
records each displayed frame.

The three-frame `animated_lossy_b16_mixed_topology_inter_420_b16x16` sequence
adds an inter B16x16 whose top-right TX8 child splits while the other three
children stay unsplit. The deterministic Pillow 12.2.0/libavif 1.4.1 generator
is `scripts/generate_avif_b16_mixed_topology_fixture.py`; its 1,186-byte input
has SHA-256
`3264482e2a50d80bd39be178b843fdfe9997e947d54b4466c745751837821d74`. The exact
Pillow/Rust frame hashes are
`7f3e5e4e65eca4390e9242558012bc9bdad133d7ac9f6aed53fa156a2288f73b` (frames 0
and 1) and
`e195092d99a776d7638ff75a30825447f6dffd85a04fa845750617ce04a98a79` (frame 2).
The selected public row passed 1/1 under branch-enabled isolated coverage in
`target/release-evidence/coverage-avif-b16-mixed-topology-row-20261004.json`.
Its `entropy.rs:7778` branch counts are true=1, false=0, proving the
`any(child_split)` true arm; at line 7775 they are true=0, false=1, leaving the
all-children-split true arm unobserved. The line 7778 false arm is also still
unobserved. Coverage MCP comparison to the older full AVIF report is limited
and shows zero aligned coordinate gains because the receipts and inventories
differ, so this selected-row observation is not an aggregate coverage gain.

The 10-bit public row `animated_lossless_inter_420_b32x32_10bit` now materializes
its sequence instead of returning the former Rust-only `Unsupported` result.
Its header permits warped motion, but the checked-in 32×32 stream has one
top-left B32×32 leaf and no overlappable neighbor; the frame flag therefore adds
no local-warp mode syntax for this input. The high-depth admission predicate no
longer rejects that capability flag, while per-block mode and geometry checks
remain in `decode_inter_leaf`. The existing row passes exact Pillow parity for
both RGB frames and sequence timing/loop metadata; no new fixture or unit test
was added.

The 2026-10-02 macOS ARM64 all-feature, branch-enabled `make coverage` run passes
all executed tests and the repository floors: 85,551/142,082 lines (60.2124%),
16,248/32,170 branches (50.5067%), 4,791/8,972 functions (53.3995%), and
130,676/219,579 regions (59.5121%). Its report is
`target/release-evidence/coverage-full-avif-high-depth-parity-20261002.json`.
Coverage MCP still reports unvisited branches in
`decode_inter_transform_size`; its source/build receipt is absent, so its
comparison is coordinate guidance and does not establish a verified coverage
delta. The 100% target remains open.

The public row `animated_opidc_0x101` transforms the pinned animated AVIF
fixture (input SHA-256
`25b79a856ea2767e02e5a509f72e7af3f9563202301779be65b0745724ab23be`) to use
operating-point IDC `0x101` and explicit temporal-0/spatial-0 extension headers
for its seven layer-specific OBUs. Its five 150×150 RGB frames match Pillow
byte for byte, and the native loop witness confirms all five displays. The
selected public parity command reports `1/1` active rows passed; the native
repetition witness test also passes. Before this row, Coverage MCP listed the
false arm at `src/codecs/avif/av1/frame.rs:952` as missing. After the full
coverage run, its line-filtered branch query returns no missing group at 952;
the function still has gaps at lines 947 and 964. MCP marks source/build
identity unverified and test status unknown, so this is coordinate guidance;
the local instrumented campaign is the executed test evidence.

A fresh all-feature `make coverage` run passes every executed test and the
release floors: 104,619/163,698 lines (63.9098%), 16,854/32,444 branches
(51.9480%), 5,262/9,405 functions (55.9490%), and 153,241/244,291 regions
(62.7289%). Compared with the preceding full report, it observes four
additional lines, four branches, and four regions; function coverage is
unchanged. The new 32×32 lossy I444 AVIF parity row exercises mode-2 and positive
qindex outcomes before its geometry excludes the separate 64-axis split path.
The full report still has 247 uncovered branch observations in
`decode_inter_transform_size`. Coverage MCP marks source/build identity
unverified and test status unknown, so its gap coordinates are guidance. The
report SHA-256 is
`79cc1056db899cd493dea18492556626d95c8e91bab4da1250ddf96d77d58149`.
Raw LLVM counters observe six true and 24 false executions at
`src/codecs/avif/av1/block.rs:65010` in a wide-chunk decoder specialization,
and the true arm at `block.rs:55789`
was reached three times in the lossy decoder specialization. Coverage MCP still
lists misses in other monomorphizations. The largest general AV1 gap remains at
`block.rs:55105`, which prior parsed-state analysis found unreachable for
legal lossy input. JPEG progressive gaps belong to the non-active `BitReader`
monomorphization on this ARM64 host; the active public `FastBitReader` path is
covered, and parsed JPEG state cannot reach the reported dequantization
overflow fallback. MCP reports no remaining ICO decode branch group. The WebP
cost/quant interpretation in that report depended on private coverage-hook
monomorphizations that have since been removed; those old coordinates are not
current evidence. The current public parity report above still lists WebP VP8
segment-delta and VP8L bounds outcomes. Coverage MCP marks source/build identity
`unverified` and test status `unknown`, so the results are coordinate guidance
only. The 100% target remains open.

ICO adds four 3×1 public 4-bpp parity rows for the black-and-white white-bit
arm, two distinct two-entry palette predicate failures, and an out-of-range
palette reference. Pillow decodes the missing palette index as black; the Rust
decoder now uses a fixed 16-entry table with black-filled unused entries. That
removes the separate palette-reference scan while preserving the existing
short grayscale-palette errors. The selected rows pass 4/4, and Coverage MCP
reports no missing decode branch in `src/codecs/ico/decode.rs` for the full
report.

JPEG adds a minimal public parity fixture,
`progressive_ac_refinement_eobrun_correction_bit`, for a 16×8 two-block
grayscale progressive stream. Its repeated 8×8 source pattern is pinned to
input SHA-256 `d1823abbbd3f9b78b440eb25abbb38b73db7dc30be1457d013131d481038c3eb`;
the selected public decode row matches Pillow pixels exactly. The isolated
instrumented run reaches the AC-refinement EOBRUN correction arm through
`FastBitReader`. The full local report remains unchanged because the remaining
true arm is the separate `BitReader` instantiation used on x86, which this
ARM64 host does not execute. The existing x86 CI lane runs the public JPEG
matrix and retains branch reports; no x86 runtime result is claimed here.

A fresh all-feature `make coverage` campaign on 2026-09-30 passes every
executed tests and the release floors: 104,452/163,691 lines (63.8105%),
16,752/32,454 branches (51.6177%), 5,254/9,404 functions (55.8698%), and
153,135/244,275 regions (62.6896%). It used `nightly-2026-07-16` on macOS
ARM64. The report is local to the dirty worktree and contains no revision or
build receipt. Coverage MCP therefore labels its source identity unverified
and test identity unknown; the successful `make coverage` exit is the local
test-run evidence. These floor passes do not meet the separate 100% target.
The current report leaves 59,239 lines, 15,702 branch observations, 4,150
functions, and 91,140 regions uncovered. `make fmt`, `make lint`,
`make lint-x86-simd`, and `make test-feature-matrix` also pass. The matrix
passes native and WASI runtime lanes plus browser-WASM compile/documentation
lanes; its capability tables agree. SSE2-baseline and AVX2 x86 builds pass
strict Clippy, which validates compilation but is not an x86 runtime benchmark
or speed claim.

Two new public parity rows extend the corpus. JPEG
`progressive_ac_refinement_zrl_symbol` exercises a ZRL across coefficients
1–16 before introducing coefficient 17; its selected row matches Pillow and
the instrumented ARM64 run covers both outcomes in the `FastBitReader`
specialization. Coverage MCP still reports the portable `BitReader` false
arm at `progressive.rs:209`, which the ARM64 public matrix does not exercise. AVIF
`animated_lossy_wide_i444_b128x128` adds exact two-frame parity for a lossy
4:4:4 sequence with 128×128 superblocks. Its selected run does not satisfy the
size predicate at `block.rs:65010`, so it does not cover the specialized wide
chunk branch. The remaining `decode_following_vertical` observations are in a
diagnostic fallback that the current public 4:2:0 decode path cannot reach;
no synthetic coverage-only input was added.

On 2026-10-01, the active public row
`progressive_ac_refinement_zrl_duplicate_tail` passed exact Pillow pixel parity
(1/1). Its repeated AC-refinement scan is the targeted edge case for the
correction-bit no-op at `progressive.rs:226`. Coverage MCP reports no remaining
branch group there for the ARM64 `FastBitReader` specialization; the listed
missing outcomes belong to the portable `BitReader` monomorphization, which
this target's public matrix does not exercise. The selected-format report has
no source/build receipt, so the mapping is local coordinate evidence rather
than a verified full-suite comparison. This active row already covers the
reachable path, so no additional fixture was justified. x86 runtime parity and
performance still require matching hardware.

Coverage MCP reports no missing branch observations in JPEG
`ycc_to_rgb_batch` or AV1 `inter_intra_context` in this report. The public
`baseline_444_partial_blocks` JPEG row exercises the scalar RGB tail. The new
deterministic 17×17 two-frame 4:2:0 sequence has exact Pillow references for
both frames. Differential decoding exposed and fixed two sequence-path
defects: one-intra/two-neighbor inter-intra context selection, which previously
overread tile padding, and final-column odd-width chroma upsampling, which
blended an out-of-edge sample. Both frame hashes match Pillow after the fixes.

Coverage MCP on the current local report lists 191 missing branch observations
in `decode_inter_transform_size`. Its current full report observes the
`transform_mode == 0` arm and the TX64 `offset_x == 0` decision; clipped-geometry
and high-depth guards remain open. Pinned AOM searches still find input
geometries that the safe Rust sequence decoder rejects, so those candidates
remain outside the Pillow parity corpus. The largest
AV1 groups remain the I444 split predicate at `block.rs:54939` and the vertical
I422 TX4 chroma grid at `block.rs:54973`; observations repeat across decoder
monomorphizations. The new I444 parity row covers the predicate true arm for
some instantiations, while the separate terminal body and other
monomorphizations remain open. The existing active public row
`animated_motion_chroma_422` reaches the vertical TX4 predicate's true arm in
one `decode_inter_translation` specialization; its row-filtered LLVM report
records 3 true and 0 false evaluations with exact four-frame Pillow parity.
Other decoder monomorphizations remain uncovered, so no duplicate I422 fixture
was added.

Coverage MCP also reports 114 missing branch observations in
`high_depth_lossy_i444_superres_restoration_supported`, beginning at
`entropy.rs:13514`. The existing 8-bit I444 animated parity fixtures exercise
the generic I444 operand, but not the high-depth operand. The pinned
Pillow/libavif AOM encoder rejects a 10-bit animated probe, and Pillow's AVIF
save interface does not expose a bit-depth option, so there is no reproducible
oracle-backed public candidate to add yet.
At the time of this historical report, an 8-bit I420 super-resolution/
restoration fixture was blocked because stock pinned libavif rejected the
required encoder controls. On 2026-10-04, a reproducible fixture-only patch
under `scripts/avif_fixture_oracle/` exposed those controls and enabled a
public parity case. The active case uses largest-transform mode and 16x16
partitions; its two exact Pillow frame hashes are recorded in
`tests/fixtures/input/images/avif/README.md`. This does not close the separate
high-depth I444 restoration gap.
Coverage MCP reports four missing branch observations in the defensive
empty/zero-size guard at `decode.rs:1049` inside the 4:2:0 sampler. No public
AVIF fixture has been identified that reaches this helper with an empty plane
or zero geometry; the gap remains open instead of adding a private-only test.
It also reports repeated observations around `decode_following_vertical` at
`block.rs:67502`. Source review found its four direct callers inside two
bounded 4:2:0 helpers. Both constructors hardcode 4:2:0, and their closed-frame
callers require 8-bit input; generic Full/high-depth following blocks use a
separate decoder path. The `full_strict` branch is therefore not reachable
through today's public decoder call graph. These MCP coordinates are
unverified because the local report has no source/build receipt. The 100%
target remains open.

### Previous full-suite snapshot before parity-only cleanup — 2026-10-02

The last recorded all-feature `make coverage` run before removing private
coverage sweeps passed its then-current tests and release floors: 108,537/162,403
lines (66.8319%), 17,646/32,238 branches (54.7366%), 5,456/9,374 functions
(58.2035%), and 159,061/242,607 regions (65.5632%). It ran with
`nightly-2026-07-16` on macOS ARM64 and wrote
`target/release-evidence/coverage.json`. This report predates the current source
and fixture set and is historical, not a measurement of the current tree.
Coverage MCP reports its source identity as unverified and test status as
unknown because the report has no source/build receipt. Its branch observations
cannot be attributed to current public parity rows. The selected public matrix
measurement above is current but does not establish full-suite floors or 100%
coverage.

That earlier MCP report listed a missing outcome in an AV1 decoder
specialization. Its coordinate (`block.rs:21108`) predates source cleanup. The
format rule still requires `delta_q_present = 0` when `CodedLossless = 1`, so a
conforming parity input cannot exercise the opposite state
([AV1 bitstream semantics](https://github.com/AOMediaCodec/av1-spec/blob/master/07.bitstream.semantics.md#codedlossless-semantics)).
No synthetic case was added to claim that unreachable state.

The active `portable_lossless_filmgrain_i444_64x64` row exercises a 64×64
8-bit all-lossless I444 AV1 still with film grain. The full-resolution I444
admission uses the existing bounded grain-dimension check, then the public
decode path applies the grain kernel. The row is one of 393 active AVIF decode
cases and passes the current public matrix with exact Pillow parity. Its RGB
reference is 12,288 bytes with SHA-256
`44f85ee642e036ae8646b40b2a71643f1e74a27d2a5a6af719c938d8aba2dceb`, and its
AVIF source SHA-256 is
`5e40ca71068fdd232d11f356103f53c1fdd7b94356fa91b0004d7307de4cbd25`. The
historical full coverage run recorded 859 more covered lines, 110 more
branches, 64 more functions, and 1,335 more regions than its predecessor. The
report has no source/build receipt, so that coordinate change is not a
test-attributed gain.

Progressive JPEG reconstruction now selects the existing safe fixed-point
`wide` IDCT on x86_64 as well as AArch64. The x86_64 baseline uses SSE2; the
AVX2 configuration is a separate compile-time target-feature build. The
all-feature JPEG decode matrix passed all 163 active rows on this ARM64 host,
which exercises the AArch64 path only. `make lint-x86-simd` passed strict
Clippy for x86_64 SSE2 and AVX2 builds. CI runs the public JPEG decode and
encode parity matrices with branch reports on both x86 configurations, and the
benchmark workflow measures both variants on the same AVX2-capable host. No
x86 runtime parity or speedup result is claimed until those artifacts are
available.

A separate three-round, 40-workload pre-candidate ARM64 JPEG diagnostic
compared the then-current build with TurboJPEG. RGB 4:2:0 encode medians were 1.82× slower at 512×512
quality 85 and 1.89× slower at 1024×1024 quality 85; their output lengths and
hashes matched. The run is a baseline, not an A/B candidate comparison or an
x86 measurement. Its artifacts are local under
`target/benchmarks/jpeg/goal-baseline-20261002/`; the results identified the
RGB 4:2:0 encode path for further full-workload profiling.

A 3-second macOS `sample` capture during 2,000 public 1024×1024 quality-85
RGB 4:2:0 encodes observed 2,320 stack samples: 1,953 ended in
`encode_baseline_420_mcu_row_streaming`, 215 in aligned RGB conversion, and 148
in packed FDCT. The optimized runner reported a 6.569 ms median and 7.004 ms
p95, with the same 793,139-byte output and hash `d748543a168ce6aa`. The row
pipeline inlines its transform, quantization, and entropy work, so this sample
does not separate those costs enough to justify a stage-specific SIMD change.
The run and sample are under
`target/benchmarks/jpeg/goal-encode-rgb1024-q85-arm64-debuginfo-20261002.*`;
they are profiling evidence, not a controlled performance comparison.

The aligned RGB 4:2:0 MCU-row fast path reuses the existing packet conversion
kernel and keeps the generic edge-replication path for partial dimensions. Its
public JPEG encode parity matrix passes all 61 active rows. A 32×33 RGB case
reaches the streaming converter with aligned width and odd height; Coverage MCP
no longer reports missing branches in this converter. The unreachable padded
width comparison was removed because `ceil(width / 16) * 16` equals `width`
whenever the preceding width-alignment condition is true. A balanced local
baseline/candidate/candidate/baseline comparison, with five rounds per build
and the full 40-workload matrix on the same ARM64 Mac15,7, measured a 2.13%
lower geometric-mean latency across encode workloads and a 1.65% lower mean
across the 512×512 quality-10/85 and 1024×1024 quality-85 RGB 4:2:0 cases.
Decoder latency changed by -0.10%; input and Rust output length/hash matched
for all 40 workloads. The measurements are directional ARM64 evidence, not an
x86 SIMD result or a cross-machine performance claim. Raw runs are under
`target/benchmarks/jpeg/goal-aligned-ab-{b1,c1,c2,b2}-20261002/`.

The JPEG quantizer now omits its upward quotient correction: prepared JPEG
divisors are 8–2040 and use `ceil(2^32 / divisor)`, so multiply-high cannot
underestimate the quotient. The single downward correction remains, and the
public JPEG encode parity matrix passes all 61 active rows. Ten alternating
500-encode captures of the public 1024×1024 RGB quality-85 4:2:0 workload on an
Apple M3 Pro measured a 1.14% lower median of run medians (6.271 ms candidate,
6.343 ms baseline); six of ten paired runs were faster. Output size and FNV-1a
hash were identical at 793,139 bytes and `d748543a168ce6aa`. This is a small,
directional ARM64 result; raw records are in
`target/benchmarks/jpeg/goal-quantizer-20261002/`, and no x86 runtime speedup is
claimed.

Four exact public AVIF sequences now select non-wedge inter-intra mask modes
0, 1, 2, and 3. The mode-3 case has a pinned dav1d syntax trace for the
final-frame B16x16 at `(16,16)`, matches all three Pillow RGB frames and
100 ms durations, and is retained in the 60-case, 167-artifact AVIF loop
bundle. The current full LLVM report observes both outcomes of
`mode == 2` at `mc.rs:778` 768 times each, and Coverage MCP reports no missing
branch at that line. Six missing observations remain in
`fill_inter_intra_mask`'s invalid-mode and invalid-geometry guards; valid AV1
syntax cannot produce those states, so they remain uncovered without
private-only tests.

The new active `animated_lossy_wide_i444_mode2_unsplit_b128x128` case is a
two-frame 256×256 I444 sequence encoded with fixed 128×128 partitions. Its
mode-2 unsplit inter leaf is the public input needed to take the previously
unobserved `topology.is_none()` arm at `block.rs:63734`. The selected public
row passes exact Pillow RGB parity; frame hashes are
`e9a4e243aaa2832b61a0f8d362ee44e06d67acf653cf0e687c0d3e33099d8e` and
`cd57a87fb5d4d0d896d8dd50b80583ca80780eee160f5d59cd4699283937c4dc`, each
with a 100 ms duration. Its complete-file loop observation is infinite.
The selected LLVM report records one true and three false evaluations at
that branch. The full report now records two true and 60 false evaluations in
the public `decode_inter_leaf` specialization (the preceding full report had
zero true and 54 false); other monomorphizations still have uncovered
observations there. Coverage MCP marks the report source unverified and test
status unknown because no source/build receipt accompanies the local report.

The companion `animated_lossy_wide_i444_mode2_split32_b128x128` fixture uses a
flat gray key frame followed by a row-alternating 32-pixel sinusoid. Its
1,739-byte input hashes to
`f48a6d235c7ab245e8d57aedcc6135b897cea1bcbc579c3d630aa7c20e1a61aa`, and both
Rust-decoded frames match the pinned Pillow bytes. The selected public row
passes 1/1; its branch report reaches the true `all_roots_split &&
all_children_unsplit` `LossyWideMode2Split32` arm at `entropy.rs:7109` and the
true `all_roots_split && all_children_split` `LossyWideMode2Deep16` arm at
`entropy.rs:7115`. Coverage MCP's selected-test comparison reports six newly
observed branch coordinates in `entropy.rs`, including both arms, but marks
the comparison limited because source/build identity and test receipts are
unavailable. The fresh full AVIF decode-matrix report passes 466/466 and
records 9,528/32,200 branch
observations; this selected AVIF report does not represent full-project
coverage. Reports are
`target/release-evidence/coverage-avif-mode2-split32-row-20261004.json` and
`target/release-evidence/coverage-avif-goal-full-20261004-rev2.json`.

The mode-zero condition in `decode_inter_transform_size` is covered, but its
nested `clipped_lossless_grid` false arm at `entropy.rs:6927` remains missing:
the fresh full report records six true and zero false outcomes. The public
17×17 clipped-grid row now changes its bottom-right pixel in frame two and
passes exact Pillow parity. Its selected LLVM report exits through the earlier
complete-grid guard at line 6881 and records neither outcome at line 6927, so
it is retained as an edge-pixel parity regression case rather than coverage
evidence for that arm. The selected batch of ten lossless inter rows records
two true and zero false outcomes there. Full aggregate coverage remains
unchanged at lines 85,551/142,082, branches 16,248/32,170, functions
4,791/8,972 and regions 130,676/219,579. Coverage MCP reports the full-report
comparison as limited because source/build receipts are absent. The reports
are `target/release-evidence/coverage-avif-clipped17-edge-20261002.json`,
`target/release-evidence/coverage-avif-lossless-inter-batch-20261002.json` and
`target/release-evidence/coverage-full-bottom-b32-speed8-edge-20261002.json`.
Pinned AOM searches found inputs for some clipped B32x16/B64x128 geometries
that Rust rejects; no rejected or mismatching file was promoted to the public
parity corpus.

The public `interlace_adam7_gray_16bit` PNG row adds a 9×9 grayscale fixture
with 16-bit samples across all seven Adam7 passes. The selected PNG coverage
report has no remaining region observations at `decode.rs:1654–1655`. A full
comparison against the preserved pre-row report lists seven newly observed
coordinates in that span, with limited status because source/build identity is
unverified. Full-suite totals are unchanged, and the incremental comparison
could not combine the selected PNG and full-suite report inventories. No
aggregate coverage increase is claimed from these reports.

In this snapshot, `decode_inter_translation_impl` still has its largest grouped
branch observations at `block.rs:54939` and `block.rs:54973`, corresponding to
the small I444 chroma-split predicate and vertical I422 TX4 chroma-grid path.
Coverage MCP repeats observations across decoder monomorphizations, so group
counts are not unique source branches. The deterministic public AVIF input
`animated_lossy_inter_i444_split_b16x32_mode2` now passes exact two-frame Pillow
pixel parity. Its SHA-256 is
`ad7ce564a11440b91237e0dfffedbde1053bc663ae0e9bf0894f87e05e669bd0`. The
public decoder trace selects `SplitB16x32Deep` with transform mode 2 and the
LLVM report records the true arm of the small I444 predicate at `block.rs:54939`.
This input does not reach the later small-chroma terminal body; other decoder
monomorphizations remain uncovered. A Coverage MCP coordinate comparison
against the saved pre-rerun report observes 45 additional AV1 block branch
observations, including 27 in `decode_inter_translation_impl`; source/build
receipts are absent, so MCP does not verify attribution. A row-filtered run of
the existing public `animated_motion_chroma_422` case
records 3 true and 0 false evaluations at `block.rs:54973` in one decoder
specialization. Its active input SHA-256 is
`5a83530dcc60f75f0bec7d36cb4db67ae7258b2acc481cbb08cc0987247ea66a`; the
focused report SHA-256 is
`efe7104321bb07819efe9a9d17194236102e0122594e145aee3056adf76e89e7`. This
confirms the geometry is publicly reachable, while the remaining repeated MCP
observations belong to other monomorphizations. The lossy mode-zero plan
families formerly reported as narrow I422 mode-zero cases were removed
after source and AV1 syntax analysis showed that they required both a lossy
segment and frame transform mode 0. AV1 uses ONLY_4X4 only for coded-lossless
frames ([AV1 specification](https://aomediacodec.github.io/av1-spec/)), while
the parser sets mode 0 only when every segment is lossless. The legitimate
lossless plans and mode-zero parsing remain.

A temporary public parity profile of staged 16×16 and 32×16 I444 candidates
matched their Pillow frame pixels and durations, but neither reached the
plane-nonzero terminal fallback at `block.rs:58149`; the LLVM report records
zero executions for both arms across the eight emitted decoder
specializations. A staged 16×32 candidate failed frame-1 pixel parity and was
excluded. The candidates did not justify another matrix row; the 100% target
and the terminal-body gap remain open.

The thin-64 lossy qindex guard at `block.rs:55045` remains without a public
parity row. A 128-case staging sweep used the pinned Pillow 12.2.0, libavif
1.4.1, dav1d 1.5.3, and AOM 3.13.2 stack across exact 16×64 and 64×16
canvases, monochrome and 4:2:0, four content patterns, quality 75 and 50, and
partition settings from 8/16 minimum to 64/128 maximum. The sweep index is
`target/oracle-staging/thin64-lossy-search/quality-partition-sweep-index.json`;
none of its candidates selected a thin-64 inter block. The closest
mode-2/non-lossless candidates with positive qindex matched all Pillow frames
exactly but selected 16×16, 16×32, or 32×16 inter leaves. This search does not
prove the decoder path unreachable for every valid AV1 bitstream, so no
coverage-only or synthetic test was added and the gap remains open.

A separate 36-case qindex-zero sweep over 16×64 and 64×16 4:2:0/monochrome
sequences also produced no thin-64 inter leaf: every quality-100 segment was
lossless with `only_4x4`, while a quality-99 control used inter qindexes 2 and
4. The pinned three-frame qmatrix-at-qindex-zero input is
`target/oracle-staging/thin64-zero-qindex/candidates/16x64_420_bands_q100_min64_max64.avif`
(SHA-256 `da19be87bf0952334f6d613e5ad1b15381ecc04f41c3e952dacde25f9cd288ac`).
A two-frame derivative at
`target/oracle-staging/thin64-zero-qindex/two-frame-qmatrix/16x64_420_bands_2frame_q100_qm15.avif`
(SHA-256 `80b7e4469118c276fb0478e4f97ea0b319d1b3feb88c9f70d9eb22f3b5cc2f53`)
uses the same first two AV1 sample payloads and isolates the qmatrix inter
frame. Both kept `segment_lossless=true` and passed independent Pillow/dav1d
decoding; the three-frame input also passed Rust still decode. Its first shown
key frame reconstructed, but the second
shown all-lossless I420 inter frame was rejected by the initial reconstruction
admission gate because `complete_lossless_inter_color_reconstruction_context`
requires `!quantization.using_matrix`. The qmatrix-off control reconstructed
that frame, then its third frame was rejected because the same profile excludes
`skip_mode_references`. The AV1 specification assigns lossless segments
`SegQMLevel=15` and applies matrix scaling only below level 15
([AV1 specification, §§5.9 and 7.12.3](https://aomediacodec.github.io/av1-spec/av1-spec.pdf));
this looks like a future profile-expansion candidate. However, the repo's
capability contract declares AVIF `sequence_decode` not implemented, so the
probe does not justify a public parity row or changing that decoder gate under
the current scope. `deltaq-mode=1` is suppressed at qindex zero by the encoder
and its three-frame parity control still selected 16×16/16×32 inter leaves.
No bitstream surgery or coverage-only fixture was added.

Earlier coverage measurements reported the true arm of the mode-zero fallback
in `decode_inter_transform_size` at `entropy.rs:6888`. It can be reached only
when `lossless_grid_geometry` is false, because the preceding geometry path
returns a lossless plan or an unsupported result. The public 16×16 lossless
inter row and staging-only 17×17, 23×21, and 33×33 lossless edge probes took
the geometry path; the odd-size probes passed exact public parity in shadow
harnesses but did not cover the fallback. They do not prove that every valid
lossless syntax/geometry is covered, so this guard remains open. Three
additional monochrome lossless sequences—7×9, 65×67 translated noise, and a
65×67 edge-residual variant—also passed exact public pixel, timing, and loop
parity in staging. Their isolated reports still record 0/0 at line 6888; the
lossless-grid guard at line 6799 was true for every inter-transform call
(true/false counts 1/0, 72/0, and 16/0). These candidates therefore took the
complete lossless-grid plan before the fallback. Candidate files and reports
remain under ignored `target/oracle-staging/mode0-public-{7x9,65x67,edge-65x67}/`;
they do not prove every valid lossless geometry is covered, and no new row was
added. The compound-reference CDF lookup now follows the declared
`[group][context]` table order, including the backward table's two group rows;
its parity correction passes the complete matrix and coverage campaign.

The existing public row `animated_odd_dimensions_inter_intra_boundary_420`
also passes an isolated all-feature parity run on the ARM64 host. Its selected
LLVM report leaves both outcomes of `transform_mode == 0` at
`entropy.rs:6888` unobserved, while the current full report still lists the
true arm as missing. The row therefore does not reach this fallback, and no
duplicate fixture was added. The selected report
`target/release-evidence/av1-odd-interintra-mode0-arm64.json` has no
source/build receipt; Coverage MCP marks its source unverified and test status
unknown.

Two additional active AVIF parity cases close decoder divergences found while
examining public edge inputs. The 20x20 all-lossless I444 sequence exposed an
incorrect B8x8 inter-intra size group; it now matches dav1d's group 1 and
passes exact two-frame Pillow parity at clipped edges. The three-frame lossy
I444 sequence exposed single-reference motion-vector collection from the
matching lane of a compound neighbor, plus a mixed B16x32 split that was
incorrectly rejected whenever OBMC was present. The decoder now selects the
first matching neighbor lane without allocation, accepts the existing
OBMC-capable topology reconstruction path, and uses AV1's 8x8 paired-MI
metadata rule in the OBMC syntax precheck. The normative check examines the
right/lower metadata owner in each 4x4 pair ([AV1 specification, §7.10.3](https://aomediacodec.github.io/av1-spec/av1-spec.pdf)). Its three
frames match Pillow exactly. The loop bundle now contains 49 complete
observations: 41 accepted inputs and eight malformed inputs, with native
repetition and complete frame hashes retained.
Coverage MCP's remaining true arm in `has_overlappable_neighbor` at
`entropy.rs:5608` is defensive: its public caller invokes the helper only when
both block axes are at least two MIs. No private-only test was added for this
unreachable guard.

A four-frame 256×256 4:4:4 affine-motion AVIF row exercises `RotZoom` global
models on a compound-reference path. Its frame-3 mismatch traced to a second
application of `compound_mode_context`: `find_reference_mvs` had already
reduced the reference stack to the final 0–7 CDF context. The decoder now uses
that context directly. All four Rust frames match the pinned Pillow RGB
references byte for byte at 90, 110, 130, and 150 ms, and the native libavif
loop witness confirms infinite repetition and repeat equality. This fixture
also exercises compound spatial-neighbor and affine-motion state through the
public decode path.

An earlier local Coverage MCP query on `target/release-evidence/coverage.json`,
before the all-feature snapshot above, reported 17,239/32,240 branch
observations covered, with 15,001 missing across 1,513 groups. Its largest
`decode_inter_translation_impl` groups reported 1,870 repeated misses at
`block.rs:54830–54832`; those counts span decoder monomorphizations and are not
unique source branches. Coverage MCP marked that report's source unverified and
test identity unknown because it had no source/build receipt, so the counters
were coordinate diagnostics rather than attributed test coverage. An earlier
coordinate comparison reported 84 newly observed coordinates against
`target/release-evidence/coverage-after-compound-reference-cdf.json`, but that
baseline file is no longer present in the workspace and the reports lacked
receipts. The earlier local test run passed all 59 coverage-matrix tests.

The active `intrabc_spatial_neighbor_monochrome_256` fixture now witnesses the
IntraBC spatial-neighbor path. Its pinned Pillow 12.2.0/libavif 1.4.1/AOM 3.13.2
input is a deterministic 256×256 monochrome lossless AVIF with neighboring
8×8 IntraBC leaves (input SHA-256
`c77c448e3668e90a4de1228c8b44a54fd47caae8e7538e314a1597dcc961795f`). Public
`decode` and `decode_sequence` both match all 196,608 Pillow RGB bytes (SHA-256
`ed729898cd63496b4a1aa628686ebff2dd7a0f383dfdae854f7cd77dae54ce22`). The
fixture exposed that AV1 infers both subsampling flags as true for monochrome;
the bounded profile gate now checks those inferred values, and the fallback
motion-vector row test is tile-relative. A DAV1D trace confirms the second leaf
uses its IntraBC spatial neighbor. The full-report Coverage MCP query still
lists three missing observations in `add_spatial_candidate` at `motion.rs:1675`,
`1678`, and `1707`; this fixture does not claim to cover every outcome there.
That report has no source/build receipt and labels test status unknown, so the
remaining locations are diagnostic rather than verified per-row attribution.
No coverage-only test was added.

The targeted temporal-projection candidate is already exercised by the active
`animated.avif` parity row (SHA-256
`2f8683d21725261f37f86e115f0c212cc52d0fefd3a2ddfcc4fa648c1859906d`).
Independent OBU inspection confirms order hints and reference-frame MVs are
enabled, including an inter frame whose future ALT slot differs from GOLDEN.
The selected public-row LLVM profile invokes `load_projected_temporal_field`
five times and records both outcomes for the ALT/GOLDEN hint comparison (2/3)
and positive temporal-distance selector (4/11); the existing row already pins
five complete Pillow RGB displays and sequence metadata. The selected report
has no source/build receipt, so these are diagnostic counters rather than
Coverage MCP attribution. No duplicate parity row was added.

Boundary-focused monochrome probes also passed public pixel and duration
parity at 69×73 with 128×128 superblocks, 4×4, 5×8, 8×5, and 5×5. Their
isolated profiles took the complete lossless-grid path for every inter
transform call and recorded no execution at `entropy.rs:6842`. The candidates
remain under ignored `target/oracle-staging/mode0-edge-non4/`; no parity row
was added because none reached the fallback.

Two additional staging-only 64×64 monochrome lossless inter-sequences matched
Pillow's decoded RGB frames and frame durations; the second frame in each
sequence was inter-coded. Their isolated LLVM profiles recorded no executions
of the large-transform helper or either outcome at `block.rs:21105`. Pillow's
AVIF info does not expose the loop field, so the Rust `loop=0` result has no
independent Pillow assertion. These candidates do not justify a public parity
row, and no row was added.

A later bounded probe exercised four deterministic 64×64 monochrome lossless
images and four RGBA images with monochrome lossless alpha items. All eight
public decodes matched Pillow exactly, while isolated LLVM profiles still
recorded 0/0 at line 21105 and did hit a separate monochrome plane-geometry
branch. The target function's production caller is a fallback reached only
when the complete streamed monochrome validator declines; these probes
followed that complete validator instead. Encoder-option attempts did not find
a valid candidate that passes the fallback's narrower admission checks. The
route remains open for a bitstream where the first validator declines and the
fallback accepts; no coverage-only input or parity row was added.

The selected public rows `animated_lossless_inter_420_b16x16` and
`animated_lossy_split_inter_420_b16x16` passed their Pillow parity checks but
did not exercise the luma-extension condition at `block.rs:48287`. Coverage
MCP reports both branch outcomes missing in the selected report
`target/release-evidence/coverage-goal-vertical420-probe.json`; the full and
other selected AVIF profiles also show no observations for the helper's
extension/fallback arms. No new row was added from these cases.

Coverage MCP also identifies a WebP `Vp8Decoder<Cursor<&[u8]>>` specialization
gap at `vp8.rs:1566`. Production decode call sites pass bounded readers
(`Take`); the direct `Cursor` specialization is instantiated only by private
decoder tests. The selected public `lossy_vp8_lossy_macroblock_residual_context_edges_32x16_q75_m4`
parity row exercised the `Take` specialization six times and did not call the
direct specialization. Since a new public row follows the same production
reader path, it cannot cover this gap; no parity row was added for it.

The new public encode row `enc_lossy_noise` re-encodes the 64×64 high-entropy
`lossless_noise.webp` source as lossy WebP through both ordinary encoding and
`encode_with_token`. The 2,888-byte Pillow 12.2.0 output matches exactly
(SHA-256 `4989ffb95ab4b7bd4f1d5feb61d80d9fe256278a1e8d90070835db628dbb3912`).
Its pre-cleanup selected LLVM report, `coverage-webp-token-noise-selected.json`,
shows the `TokenPartitionCheckpoint::flush_with_checkpoint` `output.last_mut()`
`Some(previous)` arm 15 times in `encode_bool` and once in the final flush at
`bool_enc.rs:169`. A selected comparison against the q50 token-aware row reports
three newly observed branch outcomes at lines 168, 169, and 173. The quality,
alpha, palette, grayscale, CMYK, and predictor probes did not hit this carry
path. The earlier full profile's eight `None` outcomes came from the
coverage-only hook's empty output vectors, which have since been removed. This
public parity row supplies the complementary prior-byte carry case. Its old
branch coordinates and the 17,536/32,238 aggregate belong to a pre-cleanup
report; they are not current full-suite coverage. Coverage MCP reports no
source/build receipt and unknown test status, so the selected comparison is
limited coordinate evidence rather than a managed coverage delta.

### Public parity rows added — 2026-10-01

Three active decode rows add exact edge-case parity from Coverage MCP findings.
`unknown_ancillary_after_idat` inserts a valid `abCd` ancillary chunk after PNG
image data; Pillow 12.2.0 and Rust return identical 128×128 RGB pixels
(49,152 bytes, SHA-256
`8a1d6fcc36f5b5e70fbf949d4e612630a1931279383e7c21db27cd3cbad98131`).
`bmp_1bit_default_palette` sets ICO's embedded 1-bit DIB `colors_used` field to
zero and matches Pillow's 16×16 RGBA output (1,024 bytes, SHA-256
`1a9eee34c466acd78abb5ad2db2467d3fae0931df64b402f1eaf35a65463b842`).

`compression_packbits_packbits_zero_strip_byte_count` sets TIFF tag 279
(`StripByteCounts`) to zero. Pillow still returns the exact 128×128 RGB pixels
above. The public parity test exposed Rust rejecting this input as a truncated
PackBits stream; compressed strips with a zero byte count now derive their
bounded payload span from the next strip or file boundary, while positive
declared counts keep their existing path. The all-feature TIFF decode matrix
passes 129/129 rows.

The PNG, TIFF, and ICO focused decode matrices pass 163/163, 129/129, and 54/54
rows respectively. Coverage MCP reports no remaining branch gaps at
`png/decode.rs:347`, `tiff/decode.rs:1180`, or `ico/decode.rs:699` in their
selected reports. Those reports have no source/build receipts and test status
is unknown, so these are bounded path observations and do not update the
source-bound full-repository coverage totals.

### Selected parity coverage probes — 2026-10-03

The existing public GIF row `enc_rgba_mixed` now also invokes
`encode_with_token` and compares exact Pillow bytes and ordinary/token-aware
output. Its selected Coverage MCP report no longer lists the token-present arm
in `compact_rgba_palette`. That 4×2 input is too short to reach the remap
checkpoint at 1,024 pixels, so a separate public row
`enc_rgba_mixed_token_compact_checkpoint` uses a 33×33 PNG cycling the same
eight-value mixed-alpha pattern. The selected matrix run reports exactly one active
row passed, and Coverage MCP's selected comparison observes the true
checkpoint arm at `encode.rs:2027`. The two reports have no source/build
receipts and unknown test attribution; this is local coordinate evidence, not
a full-suite coverage claim. The remaining `has_holes` true arm has no public
witness under the current FASTOCTREE palette ordering: used indices form a
dense prefix, with unused padding at the end.

The public row `pillow_tolerated_malformed_missing_iend` verifies that PNG
decode succeeds while Pillow-style verification reports a truncated-file
error. Its selected run exposed a coverage-matrix lifecycle assertion that
always required `EncodedImage::verify()` to succeed for the first selected row
of a format. The parity harness now checks the source verification result
against that row's recorded Pillow status and error contract. The selected PNG
row passes 1/1. Its LLVM report records `Chunks::next`'s clean-EOF condition
true 7 times and false 12 times; the other true arm on the same source line,
`self.failed`, remains unobserved. Coverage MCP groups those two short-circuit
branches at one line and still lists a true-arm gap. Chunk errors propagate to
the current iterator consumers, so a later `next()` call after `self.failed`
has no known public parity input. The selected report has no source/build
receipt and unknown test attribution; these are local path observations, not a
full-suite coverage claim.

The selected BMP rows `error_not_bmp` and `error_related_os2_signatures` both
pass public error parity, but their selected reports do not enter
`bmp::decode()`'s signature branch at `decode.rs:363`: format detection rejects
those signatures before dispatch. Adding another row with the same dispatch
path would not cover that decoder guard. Their reports likewise have no
source/build receipts or test attribution.

The active WebP row
`pillow_tolerated_malformed_animated_alpha_anmf_width_mismatch` keeps the
nested VP8 and alpha streams at 64×64 while declaring the ANMF frame width as
16,385. Pillow 12.2.0 decodes two 64×64 frames; the previous ALPH path
trusted the declared width and returned `ImageTooLarge`. The decoder now reads
the bounded VP8 bitstream first and uses its validated dimensions for alpha
expansion and compositing, retaining the 16,384 limit on the decoded frame. The
selected public parity row passes 1/1. Its report
`target/release-evidence/coverage-webp-alpha-frame-too-large-20261003-after.json`
still lists the guard's `false` arm as unobserved at `decoder.rs:705`; Coverage
MCP has no source/build receipt and unknown test attribution, so this is not a
verified full-suite coverage gain. Incremental comparison with
`coverage-full-gif-lzw-invariants-20261003.json` is incomparable because its
baseline branch detail is incomplete; no marginal-coverage union is claimed.

The active JPEG row `baseline_420_singleton_chroma_width_fallback` uses a 1×1
baseline JPEG whose Cb and Cr sampling factors differ. Pinned Pillow 12.2
decodes the pixel as RGB `(128, 128, 128)`, and the selected public parity row
passes 1/1 without a decoder change. Its LLVM report
`target/release-evidence/coverage-jpeg-singleton-h2v2-20261003.json` shows the
previously missing false arms at `upsample.rs:85` and `:105` hit; the opposite
true arms remain missing. Coverage MCP reports no source/build receipt and
unknown test attribution, so this records the selected input's paths rather
than a suite-wide gain.

The active WebP row
`pillow_tolerated_malformed_animated_vp8l_anmf_width_mismatch` declares an
ANMF width of 16,385 around a 16×16 VP8L stream. Pillow 12.2 decodes two
64×64 RGBA frames. The decoder now reads the bounded VP8L header and uses its
intrinsic dimensions before allocating and compositing the frame. The selected
row passes 1/1. The active rows `error_animated_vp8l_short_header` and
`error_animated_vp8l_short_header_oversized_width` cover four-byte nested
VP8L payloads with normal and oversized ANMF declarations. Pillow 12.2 rejects
both with `OSError: could not create decoder object`; both selected public
parity rows pass 1/1, including the oversized-frame guard's `ImageTooLarge`
path. The WebP decode matrix passes 237/237. Rows
`error_animated_vp8l_frame_outside_right` and
`error_animated_vp8l_frame_outside_bottom` shift a valid 64×64 nested frame two
pixels beyond each canvas edge. Pillow 12.2 rejects both; the selected public
parity rows pass 2/2 and exercise both short-circuit arms of the frame-bounds
guard. The complete decoder matrix ran eight public format tests and passed
1,419/1,419 active decode rows. Coverage MCP report
`target/release-evidence/coverage-public-decode-matrix-20261003-verified.json`
records 11,766/32,174 crate-wide branch counters for that decode-only run.
The report has no source/build receipt and unknown test attribution, so use it
as a path inventory rather than a full-suite coverage measure. Coverage MCP no
longer lists gaps in the new WebP VP8L header-size or canvas-bounds paths, or
in the JPEG singleton-width upsampler. Its remaining WebP `read_frame` gap at
`decoder.rs:734` is the VP8 dimension guard; valid VP8 widths and heights are
14-bit values and cannot exceed its 16,384 limit.

The latest public parity batch adds three malformed AVIF meta cases: duplicate
`iprp` (`9a245809b8f3a78aec952c15ee9703973a5d407bbaffe17580c4b6b3aca26ee8`),
duplicate `iref`
(`ec6f361b37447d4abeb047508863c59e60d3aa73721adb4b5a76e7e159295f3e`), and
empty `idat` (`281c5cf87f6d34f77635924a13384d107111870db7600e10636e4a2735d2c1c7`).
It also adds JPEG `progressive_com_tail_junk`
(`70dd4ed893d23e30d3e5c27ef1ac3f55e6a1a98e3aad166610db48754ee43ed2`), which
opens and verifies in Pillow but fails during load as a truncated progressive
stream. All four selected public parity rows pass. The full `make coverage` run
passes every executed test and the local floors. Coverage MCP reports seven new
branch coordinates compared with the saved pre-batch report: five under
`samples::parse_meta` and two in JPEG `find_next_marker`. The comparison remains
limited because source/build identity and test attribution are unverified; these
are coordinate changes, not source-bound coverage claims. The 100% goal remains
open.

## Clipped AV1 lossless inter-grid parity — 2026-10-04

The public AVIF matrix now includes clipped B32 lossless inter-grid witnesses
for I444 right and bottom edges, I422 right-edge cases, and I422/I444 dual-edge cases. The new
`animated_lossless_inter_i444_clipped_b32x32_64x56.avif` has SHA-256
`9004ec840b2a413c0e15294f52c723ebe5c3689cbe3decbd4afd5575cad78e23`; its
bottom B32 block exposes 32x24 pixels, and its two decoded RGB frames match
Pillow exactly. The selected public row and the complete AVIF native-loop
parity test pass. The 52x64 I422 case also passes exact sequence parity with a
20x32 clipped right block. The two 52x60 dual-edge cases pass exact sequence
parity with 20x28 luma visibility; I422 chroma visibility is 10x28. For odd-width I422 at 49x64, key-frame pixels match
Pillow and sequence decoding keeps an explicit `Unsupported` expectation
because the final chroma sample geometry is not parity-safe. The AVIF matrix
now has 454 active decode rows; the full fixture matrix has 1,950 rows.

`make coverage` passed all executed tests and the local floors. Its LLVM report
records 86,804/142,471 lines (60.9275%), 16,766/32,250 branches (51.9876%),
4,849/8,990 functions (53.9377%), and 132,767/220,396 regions (60.2402%).
Coverage MCP identified six missing branch outcomes in
`inter_lossless_grid_clipped_b32_geometry_supported` near
`src/codecs/avif/av1/entropy.rs:5395-5397`. The report has no source/build
receipt and MCP reports `source=unverified`, `tests=unknown`, and no build or
revision ID, so these coordinates do not establish named-test attribution or
a verified coverage delta. Those false arms are unreachable for valid B32 leaves: tree traversal skips
zero-area blocks, and a visible extent over 32 contradicts the coded block
size. Malformed inputs may exercise upstream rejection, but defensive-only
states do not become Pillow parity fixtures. Strict SSE2-baseline and AVX2 Clippy builds pass,
but they are compile checks, not runtime speed measurements. The 100% goal
remains open.

The refreshed MCP branch report's largest group is the `lossless_partial_grid`
predicate at `block.rs:54838-54847`. Source review found seven zero-hit
callback-specialization clones; the remaining live clone already observes both
outcomes of the clipped-versus-full extent predicate. Existing active parity
rows cover admitted clipped B32 grids across I420, I422, and I444. The rejected
zero-hit clones and the depth/alignment rejection arms do not provide a new
valid Pillow-parity input, so no redundant AVIF row was added. A bounded probe
for the 64-axis lossy transform mapping at `block.rs:57818` and its topology
variant at `block.rs:57852` also produced no bitstream that both selected the
missing plan and passed public parity. MCP still has no source/build receipts;
these are coordinates, not verified test attribution.

## PNG post-IDAT ancillary CRC parity — 2026-10-04

The public PNG matrix adds `pillow_tolerated_post_idat_ancillary_bad_crc`, an
unknown ancillary chunk after IDAT with one corrupted CRC byte. Pillow 12.2.0
still decodes the original RGB pixels (SHA-256
`8a1d6fcc36f5b5e70fbf949d4e612630a1931279383e7c21db27cd3cbad98131`), while
`verify()` reports `SyntaxError: broken PNG file (bad header checksum in
b'abCd')`. The selected public decode-matrix test passes. Coverage MCP could
not union its selected report with the saved full baseline: branch totals differ
(32,250 versus 32,324), and both reports lack source and build receipts, so no
verified coverage gain is claimed.

## GIF89a metadata-limit parity — 2026-10-04

The active `palette_index_leniency` row also decodes its 35-byte GIF89a input
with a public metadata limit of exactly 33 bytes; its two compressed image-data
bytes are excluded from the metadata extent. The targeted matrix row and full
coverage campaign pass, with exact Pillow pixel parity. Coverage MCP now leaves
only the malformed-signature outcome at `src/codecs/gif/decode.rs:361`; public
format detection rejects that signature before metadata preflight, so it does
not justify another parity fixture. The report remains unbound to source/build
receipts and has unknown test attribution.

## Small JPEG chroma and WebP palette encode parity — 2026-10-04

Two active public encode rows add exact Pillow references using existing source
assets: `enc_subsample_422_tiny_edge` encodes the 1x8 RGB JPEG as baseline 4:2:2
at quality 85, and `enc_lossless_palette16` losslessly re-encodes the 64x64 RGB
WebP fixture with exactly 16 colors. They cover small-image chroma edge
replication and the WebP 5–16-color packed-palette case. The selected public
matrix passes 2/2; the full all-feature coverage campaign also passes all 56
matrix tests, 7 decode-policy tests, and 68 feature-gate tests.

Coverage MCP still reports the defensive invalid-geometry arm at
`src/codecs/jpeg/encode/mod.rs:1507`; validated public dimensions cannot reach
that arm. The earlier full report for this review recorded 86,804/142,471 lines,
16,766/32,250 branches, 4,849/8,990 functions, and 132,767/220,396 regions; it
predates the latest full report at the top of this file. Neither report carries
source/build receipts for MCP attribution. The 100% coverage goal remains open.

## Historical claim ledger

The following source-bound baseline was measured on 2026-08-27. Its historical
percentages must not be relabeled as current-release coverage. The validator
checks the historical manifest/matrix bytes at the measured Git revision,
Coverage MCP identities, and separately checks current fixture integrity. The public rendering is centralized here
so copied prose cannot drift across guides.

<!-- current-claim-ledger:begin -->
Current claim-ledger baseline (not current `HEAD`):
- Measured revision: `93ec80ec99c42671dce6cf70694bce27ad8a2ef4`.
- Coverage MCP run: `ec4c4bbd-dbda-4e49-8109-d7da07722dc0`; snapshot: `7665cda3-f4a7-4568-b871-a9d34afaa92c`.
- Coverage: 100,389/110,015 lines (91.2503%), 12,861/14,246 branches (90.2780%), 5,125/5,794 functions (88.4536%), and 150,221/166,375 regions (90.2906%).
- Measured manifest SHA-256: `c1a1cccd485d066ffbe206a6e1577a1788aff8d4f288e4e8f8a933fa3c62ae7b`; measured matrix SHA-256: `f26151b3811aaab58556da422f476b714b5fac5925ff5b97807904096b4d2d58`.
- Current fixture integrity only: manifest `bb691b5538bf06dec81825e73a2c339ac96d3c65ebb103e1f86aa3d2ac50ab72`; matrix `c6a918a9011752acd1c11347ca87618f47ac2c7047348287f397c5ba4a4592b9`.
<!-- current-claim-ledger:end -->

The larger current source denominator and the historical source denominator
are not interchangeable. Selected incremental runs cannot substitute for a full
fresh report. Keep the original raw reports and context rather than attaching
old identities to new measurements.

The ledger's fixture-manifest hashes track current-file integrity. Updating
those hashes does not extend the historical coverage run to new code, guards,
or unexecuted regressions. The 2026-09-17 AVIF temporal-candidate repair remains
unverified by Rust execution, as recorded in the roadmap.

The 2026-09-17 coverage-fixture maintenance passes strict all-feature Clippy
with `--cfg coverage` on native and `wasm32-unknown-unknown` targets. Native
coverage builds also pass with no codecs, default codecs, and each codec
individually. Ordinary native Clippy, both WASM library feature matrices, and
all-feature rustdoc pass. Fixture setup retains explicit failure assertions,
checked bounds, and the original malformed inputs; feature-specific exercises
follow the same feature gates as their implementation. The Rust behavioral
test suite and coverage measurements remain deferred. The documentation
checker inadvertently executed the README quickstart during this maintenance;
that example passed, but is not evidence of roadmap or parity completion. The
historical coverage totals and open capability statuses remain unchanged.

The 2026-09-17 HDR color candidate has independent full-file native evidence:
dav1d and libavif agree on 240,000 YUV bytes, while pinned scalar libyuv,
libavif and Pillow agree on 120,000 RGB bytes. The deterministic observation
is recorded in the [HDR oracle index](../tests/fixtures/outputs/avif_hdr_color/hdr/index.json).
This witnesses the native conversion of 10-bit full-range I444 CICP 9/16/9
without alpha. The safe-Rust converter and its full-plane/tail regressions are
implemented; their behavioral execution and managed coverage are deferred.
Strict Clippy passes for native, `wasm32-unknown-unknown` and `wasm32-wasip1`
with AVIF-only and all features, both ordinary and coverage configurations.
Native and coverage-enabled unknown-WASM checks include all targets; WASI
checks compile the library. Warnings-as-errors rustdoc, formatting and static
provenance/roadmap checks also pass.
The HDR public row remains planned and historical coverage is unchanged.

The 2026-09-17 high-depth color candidate adds the exact 12-bit limited-range
I422 CICP 2/2/2 declaration with auxiliary alpha from `10bit.avif`. Independent
scalar dav1d and libavif agree on 81,920 color-plane bytes and 40,960 alpha-plane
bytes across all five frames. Pinned scalar libyuv, libavif and Pillow agree on
81,920 RGBA bytes, with repeatable native planes, pixels and timing. Source,
compiler, default I601 build flags and artifact hashes are retained in the
[native index](../tests/fixtures/outputs/avif_sequence_color/high_bitdepth/index.json).
Partial-alpha pixels witness straight output; zero-alpha pixels are absent.

The shared declaration selector and Rust color conversion are implemented,
with deferred regressions for every native frame, 25 full-row slices and
separate internal admission/sample-buffer checks. Strict Clippy passes all
12 native/WASM, AVIF-only/all-feature, ordinary/coverage lanes used for the
HDR candidate, and warnings-as-errors rustdoc, formatting and static checks
pass. This is compile-only Rust verification. Public reconstruction, sequence
presentation, Rust pixel execution and managed coverage remain unverified;
the existing sequence gates, planned matrix rows and historical coverage
totals are retained.

The 2026-09-17 sequence presentation candidate processes matching color and
alpha samples with persistent reference state, converts each complete display
before advancing, and publishes frames only after all samples succeed. It
reserves later output transfer bytes before reconstruction and requires the
inspected frame count, output mode and actual frame geometry to agree. Missing
reconstruction surfaces remain capability gaps; malformed tile envelopes and
empty reference slots retain precedence. That revision retained the frame-ID
sequence presentation gate; the later candidate below removes it.

The [sequence native index](../tests/fixtures/outputs/av1_sequence/animated/index.json)
retains five complete RGB displays, six decoded frames, two hidden frames and
one show-existing display from the unchanged `animated.avif`. Unmodified and
instrumented pinned scalar dav1d agree on 168,750 YUV bytes; repeated native
libavif/Pillow observations agree on 337,500 RGB bytes. Native durations are
exactly 1/30 second; Pillow reports rounded 33 ms durations.
Independent libavif repetition observations distinguish this
one-play animation from the infinitely repeating high-depth fixture. Pillow
omits its loop field for both. The [bundle notes](../tests/fixtures/outputs/av1_sequence/README.md)
retain the native metadata origin and required matrix reconciliation.

Deferred regressions compare every public display, first-image pixels,
timing, native repetition and output-budget boundaries for both fixtures.
Internal defensive cases remain Rust model assertions. All 12 strict Clippy
lanes described above, warnings-as-errors rustdoc, formatting and static
provenance/roadmap checks pass. Rust behavioral execution and managed coverage
remain deferred; no row or finding is promoted. Output transfer limits do not
bound retained AV1 references or scratch memory, and variable frame geometry
and primary-item/track declaration disagreements remain unsupported.

The later 2026-09-17 frame-ID candidate corrects interleaved reference index and
delta parsing in Rust and the syntax inspector. Seven pinned native reads
establish the ordering and values independently of either implementation.
The [frame-ID oracle](../tests/fixtures/outputs/av1_sequence/error_resilient/index.json)
retains both native RGB displays, native YUV noninterference, exact timing and
repetition, a full-file 32767-to-0 wraparound success and a full-file reference
delta rejection. The latter preserves the first image in both native/Pillow
observations and Rust's deferred regression. Native CLI diagnostics and actual
frame outputs establish rejection even though that CLI returns exit code 0.
Strict Clippy passes in all 12 native/WASM, AVIF-only/all-feature,
ordinary/coverage lanes. Warnings-as-errors rustdoc, formatting, artifact
hashes and static provenance/roadmap checks also pass. These are compile-only
and static Rust checks; the native observer runs execute only pinned oracles.

The frame-ID-only presentation gate is removed, while repeated-current-ID
validation retains precedence over reference-delta failures. Behavioral Rust
execution and managed coverage remain deferred. Short-signaling fallback,
stale-reference behavior, broader reconstruction and native-versus-Pillow loop
reference reconciliation remain unfinished. The bundle's README separates
these limitations from the bounded evidence; no matrix row or finding closes.

The 2026-09-17 short-reference candidate removes the no-future rejection and
corrects the Python inspector's maximum-distance tie handling. The shared
production helper preserves native fallback order, duplicate anchors and
repeated earliest-slot reuse. Its normal parser wrapper still requires all
eight reference headers.

The [short-reference oracle](../tests/fixtures/outputs/av1_short_references/index.json)
contains six complete AVIF mutations with independently repeated native traces,
4,608 YUV bytes and 9,216 RGB bytes. Unmodified and instrumented dav1d agree
byte for byte. Pillow/libavif observations repeat with exact native timing,
and the corrected inspector agrees on all selected indices. Equal reference
pixels make index comparisons necessary; the deferred coverage test invokes
the same production selection helper. Invalid adapter arguments are separately
identified as internal model assertions. No coverage exclusions were added.

All 12 strict native/WASM, AVIF-only/all-feature, ordinary/coverage compile
lanes pass. Rust behavior and managed coverage remain deferred. This corpus
does not establish every mixed-distance history or complete sequence/resource
parity; planned rows, finding counts and historical coverage remain unchanged.

The 2026-09-17 repetition candidate aligns the container parser with native
edit-list handling and replaces the matrix's absent-Pillow-loop inference with
explicit native provenance. The [loop index](../tests/fixtures/outputs/avif_loops/index.json)
retains 28 complete inputs, 41 hashed artifacts and 420,956 RGB/RGBA bytes.
Twenty accepted observations preserve all frames; eight malformed files fail
native parsing and Pillow opening. All observations repeat identically.

The native signed-count boundary preserves 2,147,483,648 total plays as finite,
then normalizes larger values to infinite. Absent edit lists stay unspecified;
nonrepeating lists stop after flags. Repeating lists validate count, version
and nonzero segment duration but ignore media fields. Color repetition wins
over a different alpha value, while malformed alpha metadata still rejects.

The generator and Rust matrix harness bind loop evidence to the complete
input, case, native source, index hash and normalized value. The Python static
validator rejects a valid witness substituted from a different file. Existing
row statuses and deferred Rust capability contracts remain unchanged pending
the final behavioral campaign. All 16 strict native/WASM compile configurations,
warnings-as-errors rustdoc and static provenance checks pass. The guard inventory
remains 540 across 89 files. No current-source coverage claim is made.

The later 2026-09-17 display-retention candidate replaces the per-sample vector
of shown completions with one selected completion. Online maximum selection
preserves temporal-unit/spatial/temporal precedence and latest-tie behavior.
Losing candidates still commit their reference/CDF/current-ID state. A hidden
sample cannot reuse an earlier display as its current presentation. Failed
flushes preserve the last completed-frame commit, not a whole-sample rollback.

Two deferred internal ownership regressions use `Weak` references to check
immediate release of losing/superseded surfaces and survival of independently
referenced surfaces. They also cover selection axes, exact ties, a winning
missing-surface gap, hidden-unit filtering, pending frames and counter overflow.
Existing complete native animation/high-depth/frame-ID witnesses retain pixel
authority. No input acceptance boundary or oracle output is changed. Rust
behavior and managed coverage remain deferred; aggregate memory limits and
peak-allocation measurements remain unfinished.
All 12 strict native/WASM, AVIF-only/all-feature, ordinary/coverage compile
configurations pass, along with rustdoc, formatting and static provenance gates.
The coverage-origin inventory records 122 exact guards across 18 Rust files.

## Diagnostic provenance

The separate defensive-model contract has 61 diagnostic cases: 38 use committed bytes that also have a Pillow parity row;
23 cases construct runtime mutations. Diagnostic identities and messages remain
Rust model observations, not Pillow parity fields. The provenance verifier checks
the original input hashes and supporting baseline route for every case.

## Contract catalog: behavior Pillow cannot prove

This is the separate Rust-only list. “Cannot prove” means Pillow may have a
similar idea internally, but it cannot return this crate's exact field, token,
target, sink, or typed result for comparison.

The bounded v1 map is machine-checked by
`make verify` against
`tests/fixtures/unreachable_contract_manifest.json`. `covered` means that the
manifest names an existing fixture-backed integration contract or fixture
verifier; `planned` means that no such contract is claimed yet. The map is an
evidence index, not a claim that every legal format state is implemented.

The verifier also parses this ten-row table. It requires each row's status and
Pillow-parity column to match the manifest, every covered row to name the exact
manifest evidence paths, the release-package row to name its exact fixture
verifier, and the planned allocation/stack/coverage row to say that no
category-specific evidence is claimed while naming its bounded context paths.
This is documentation-integrity evidence only: it does not promote the planned
category to covered and does not add any Rust-only result to Pillow parity.

The separate `fault_contract_cases` index maps target-only injected failures
to their structured-diagnostics requirement and shared matrix selector. The
AVIF and JPEG fault cases reuse active Pillow parity inputs as reproducible
stimulus, while each injected result is recorded with
`verification: fault-contract` and `oracle_status: not_applicable`. The runner
reports this lane separately from Pillow decode/encode counts.

| Map ID | Rust-only contract | Why Pillow cannot prove it | v1 status | Separate evidence | Pillow parity |
| --- | --- | --- | --- | --- | --- |
| `decode-encode-policy-limits` | `DecodePolicy` and `EncodePolicy` limits | Pillow does not expose this crate's pre-detection, canvas, metadata, decoded-byte, encoded-output, or work-budget result with the same boundary/error fields | `covered` | Manifest evidence: `tests/decode_policy_tests.rs`, `tests/feature_gate_tests.rs` | `excluded` |
| `cancellation-work-budgets` | Cancellation and work budgets | Pillow has no caller-owned `CancellationToken`, checkpoint budget, or `EncodeWorkUnits` result | `covered` | Manifest evidence: `tests/feature_gate_tests.rs` | `excluded` |
| `output-sink-delivery` | `OutputSink` delivery | Pillow does not accept this crate's dependency-free sink, expose delivered prefixes, flush failures, or rollback semantics | `covered` | Manifest evidence: `tests/feature_gate_tests.rs` | `excluded` |
| `caller-owned-destination-buffers` | Caller-owned destination buffers | Pillow does not expose `decode_into` capacity, short-destination rejection, or no-partial-write guarantees | `covered` | Manifest evidence: `tests/feature_gate_tests.rs` | `excluded` |
| `source-provenance` | Source provenance | `SourceDescriptor`, FileTypeBox facts, AVIF item/property identity, raw source relationships, and declared-versus-confirmed fields are not Pillow result fields | `covered` | Manifest evidence: `tests/feature_gate_tests.rs` | `excluded` |
| `structured-diagnostics` | Structured diagnostics | Rust diagnostic kind, offset, consumed extent, recovery status, and provenance are not Pillow's ordinary return shape | `covered` | Manifest evidence: `tests/feature_gate_tests.rs`, `scripts/verify_diagnostic_provenance.py`; target-only fault cases: `fault-contract:avif:avif_display_plane_copy_allocation_failure`, `fault-contract:avif:avif_temporal_motion_field_reservation_failure`, `fault-contract:avif:avif_sequence_frames_reservation_failure`, `fault-contract:avif:avif_superresolution_positions_reservation_failure`, `fault-contract:avif:avif_superresolution_plane_reservation_failure`, `fault-contract:avif:avif_sgr_restoration_output_reservation_failure`, `fault-contract:avif:avif_sgr_intermediate_reservation_failure`, `fault-contract:avif:avif_restoration_stripe_scratch_reservation_failure`, `fault-contract:jpeg:jpeg_multiscan_coefficient_reservation_failure`, `fault-contract:avif:avif_monochrome_tile_reservation_failure`, `fault-contract:avif:avif_color_tile_reservation_failure`, `fault-contract:avif:avif_tile_cell_reservation_failure`, `fault-contract:avif:avif_tile_block_metadata_reservation_failure`, `fault-contract:avif:avif_chroma_tile_cell_reservation_failure`, `fault-contract:avif:avif_assembled_loop_filter_metadata_reservation_failure`, `fault-contract:avif:avif_partition_node_reservation_failure`, `fault-contract:avif:avif_projected_temporal_field_reservation_failure`, `fault-contract:avif:avif_grid_cell_reservation_failure`, `fault-contract:avif:avif_monochrome_loop_filter_metadata_reservation_failure`, `fault-contract:avif:avif_monochrome_cdef_region_map_reservation_failure`, `fault-contract:avif:avif_monochrome_cdef_active_map_reservation_failure` | `excluded` |
| `feature-target-capability` | Feature and target capability | Pillow does not model this crate's Cargo feature-disabled errors or native versus `wasm32-wasip1` capability table | `covered` | Manifest evidence: `tests/feature_gate_tests.rs`, `tests/capability_table.rs` | `excluded` |
| `cache-concurrency-api-lifecycle` | Cache/concurrency/API lifecycle | Pillow does not expose `EncodedImage` lazy-cache states, Rust clone sharing, bounded native concurrent verification, or this crate's frame/page lifecycle | `covered` | Manifest evidence: `tests/feature_gate_tests.rs` | `excluded` |
| `release-package-surface` | Release package surface | Pillow cannot inspect this crate's Cargo archive, included source/legal files, or deliberate exclusion of parity fixtures and repository-only integration targets | `covered` | Manifest evidence: `tests/fixtures/package_surface_manifest.json`, `scripts/verify_package_surface.py` | `excluded` |
| `allocation-stack-coverage-models` | Allocation/stack/coverage models | Pillow cannot witness Rust allocator checkpoints, stack measurements, or private defensive branches | `planned` | No category-specific evidence is claimed. Planned context: `scripts/benchmark_fixture_workloads.py`, `scripts/verify_coverage_origins.py`, `tests/fixtures/coverage_origin_manifest.json` | `excluded` |

Rust-only results do not belong in Pillow parity rows. The shared matrix has a
separate `fault_contracts` lane for deterministic target-internal failures that
ordinary inputs cannot trigger. Each such row is typed as `fault-contract`,
uses `oracle_status: not_applicable`, and shares the matrix selector and
case-index machinery without contributing to parity counts. Its input may be
an existing Pillow parity fixture; that keeps the stimulus reproducible but
does not turn the injected result into Pillow parity.
