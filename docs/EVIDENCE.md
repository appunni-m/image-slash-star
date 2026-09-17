# Versioned compatibility evidence

Fixture declarations, executed comparisons, source coverage, and release
publication have different identities and denominators.

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

The retained matrix contains 1,567 total rows: 1,170 decode /
inspect / verify rows and 397 encode rows. Of those, 1,167 decode rows
and 365 encode rows are active. Planned rows stay outside executed
parity numerators. Active rows have operation-specific outcomes, including
not-applicable results.

[Generated capabilities](capabilities.md) distinguish declarations and actual
native/all-feature fixture observations. A WASM cross-compile does not extend
that pixel-evidence scope.

## Historical claim ledger

The following source-bound baseline was measured on 2026-08-27. Its historical
percentages must not be relabeled as current-release coverage. The validator
checks the original revision, manifest/matrix hashes, Coverage MCP identities,
and all referenced fixture contracts. The public rendering is centralized here
so copied prose cannot drift across guides.

<!-- current-claim-ledger:begin -->
Current claim-ledger baseline (not current `HEAD`):
- Measured revision: `93ec80ec99c42671dce6cf70694bce27ad8a2ef4`.
- Coverage MCP run: `ec4c4bbd-dbda-4e49-8109-d7da07722dc0`; snapshot: `7665cda3-f4a7-4568-b871-a9d34afaa92c`.
- Coverage: 100,389/110,015 lines (91.2503%), 12,861/14,246 branches (90.2780%), 5,125/5,794 functions (88.4536%), and 150,221/166,375 regions (90.2906%).
- Manifest SHA-256: `72cba218c984eb7179d5efc984b0836f72610e22a8bcc49d979651c46e4478d2`; generated matrix SHA-256: `002f1a6293a0913d6a010f325db64a82258d5b5f7ae8e778e37b008af22ecc71`.
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

| Map ID | Rust-only contract | Why Pillow cannot prove it | v1 status | Separate evidence | Pillow parity |
| --- | --- | --- | --- | --- | --- |
| `decode-encode-policy-limits` | `DecodePolicy` and `EncodePolicy` limits | Pillow does not expose this crate's pre-detection, canvas, metadata, decoded-byte, encoded-output, or work-budget result with the same boundary/error fields | `covered` | Manifest evidence: `tests/decode_policy_tests.rs`, `tests/feature_gate_tests.rs` | `excluded` |
| `cancellation-work-budgets` | Cancellation and work budgets | Pillow has no caller-owned `CancellationToken`, checkpoint budget, or `EncodeWorkUnits` result | `covered` | Manifest evidence: `tests/feature_gate_tests.rs` | `excluded` |
| `output-sink-delivery` | `OutputSink` delivery | Pillow does not accept this crate's dependency-free sink, expose delivered prefixes, flush failures, or rollback semantics | `covered` | Manifest evidence: `tests/feature_gate_tests.rs` | `excluded` |
| `caller-owned-destination-buffers` | Caller-owned destination buffers | Pillow does not expose `decode_into` capacity, short-destination rejection, or no-partial-write guarantees | `covered` | Manifest evidence: `tests/feature_gate_tests.rs` | `excluded` |
| `source-provenance` | Source provenance | `SourceDescriptor`, FileTypeBox facts, AVIF item/property identity, raw source relationships, and declared-versus-confirmed fields are not Pillow result fields | `covered` | Manifest evidence: `tests/feature_gate_tests.rs` | `excluded` |
| `structured-diagnostics` | Structured diagnostics | Rust diagnostic kind, offset, consumed extent, recovery status, and provenance are not Pillow's ordinary return shape | `covered` | Manifest evidence: `tests/feature_gate_tests.rs`, `scripts/verify_diagnostic_provenance.py` | `excluded` |
| `feature-target-capability` | Feature and target capability | Pillow does not model this crate's Cargo feature-disabled errors or native versus `wasm32-wasip1` capability table | `covered` | Manifest evidence: `tests/feature_gate_tests.rs`, `tests/capability_table.rs` | `excluded` |
| `cache-concurrency-api-lifecycle` | Cache/concurrency/API lifecycle | Pillow does not expose `EncodedImage` lazy-cache states, Rust clone sharing, bounded native concurrent verification, or this crate's frame/page lifecycle | `covered` | Manifest evidence: `tests/feature_gate_tests.rs` | `excluded` |
| `release-package-surface` | Release package surface | Pillow cannot inspect this crate's Cargo archive, included source/legal files, or deliberate exclusion of parity fixtures and repository-only integration targets | `covered` | Manifest evidence: `tests/fixtures/package_surface_manifest.json`, `scripts/verify_package_surface.py` | `excluded` |
| `allocation-stack-coverage-models` | Allocation/stack/coverage models | Pillow cannot witness Rust allocator checkpoints, stack measurements, or private defensive branches | `planned` | No category-specific evidence is claimed. Planned context: `scripts/benchmark_fixture_workloads.py`, `scripts/verify_coverage_origins.py`, `tests/fixtures/coverage_origin_manifest.json` | `excluded` |

These cases must stay out of `coverage_matrix.json` unless a row also has a
separate Pillow-observable assertion. A Rust-only test may still use a
Pillow-generated image as input; that makes the picture reproducible, but it
does not turn the Rust-only result into Pillow parity.
