# Oracle, fixtures, tests, and coverage

Status: current contributor reference

Reviewed: 2026-07-31 on the working tree based on revision `c2f1773`

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
6. `tests/fixtures/outputs/` contains exact expected metadata, pixels, frames,
   entropy traces, and encoded files.
7. The Rust integration harnesses execute those contracts.

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
  and evidence origin;
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

GIF source rectangles are not mislabeled as rendered-pixel parity: their
source/presentation metadata is independently asserted, while exact raw source
sample bytes remain a documented gap. The schema does not yet compare
auxiliary retained metadata such as ICC, EXIF, XMP, text, or orientation.

## Current revision-bound evidence

For the current working tree based on revision `c2f1773`, the generated matrix
reports:

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
measured manifest values.

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
is generated by `scripts/generate_capability_tables.py` from a probe test
(`tests/capability_table.rs`) executed in every feature lane on the native
host and on `wasm32-wasip1` under Node's WASI preview1 runtime. The fixture
keys evidence by full target triple for the WASM lane and records the
generating native host triple; CI regenerates the tables in memory and fails
on any drift, so native and WASM capability tables cannot diverge from the
implementation. The same script is part of the feature-matrix command
registered with Coverage MCP as `feature-matrix-runtime-tables`.

The trailing-input manifest pins the per-format trailing policy: three payloads
appended to a valid asset of every format must produce identical still pixels,
identical sequence frames, and identical inspection results, with
`consumed_bytes` unchanged. JPEG, PNG, GIF, WebP, TIFF, and AVIF report the
container-defined extent; BMP and ICO report `None`. Pillow 12.2.0 accepts all
three payloads for every format (fixture evidence), while the consumed values
and policy labels are `defensive_model` evidence. The manifest is SHA-256
pinned and exercised only in each format's enabled feature lane.

The verification-strength contract is table-driven rather than manifest rows:
for every format, the enabled feature lane loads the smallest Pillow-verified
fixture and asserts `ImageFormat::verification_scope()` and
`EncodedImage::verification_scope()` agreement, weaker/equal
`verify_with_scope` success, stronger-request failure with the exact format
and a non-empty diagnostic, and the never-provided `FullPixels` boundary.

The operation-stage contract is a separate feature-gate test: one real
failure is driven through inspection, still decode, sequence decode, source
construction, verification, still encode, and sequence encode, and each error
must report the exact `ImageErrorStage` alongside its stable kind. Caller-built
validation and option-construction errors remain stage-free by design, so the
test also pins which public paths produce a stage.

The same test now asserts the parse-site byte offset and container-structure
identity on codec-dispatched failures: truncated PNG chunks across
inspect/still/sequence/source/verify, GIF image descriptors, JPEG markers,
TIFF IFDs, and truncated AVIF boxes all carry `identity` values with offsets,
while encode and option-construction errors stay offset-free. BMP, ICO, and
WebP decode internals are documented as intentionally detail-free.

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

The accepted Coverage MCP result for the same implementation state is:

| Metric | Covered | Total |
| --- | ---: | ---: |
| Lines | 42,159 | 42,159 |
| Branches | 6,018 | 6,018 |
| Functions | 2,320 | 2,320 |
| Regions | 67,022 | 67,022 |

The same managed run executed every active manifest case with zero failures or
skips.

Coverage MCP run: `4491d487-a7a9-4d3c-b2c6-acaa3926b683`

Snapshot: `b487b689-5e66-48bc-b871-ae3b47e5be5f`

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

Cross-compilation proves compilation, not semantic browser or WASM runtime
parity. The `wasm32-wasip1` lanes are real runtime evidence for feature-gate
and capability-table behavior; full semantic manifest execution in a WASM
runtime remains planned.

### Coverage

Repository agents must run coverage only through Coverage MCP and request line,
branch, function, and region metrics. CI independently runs `cargo llvm-cov`
and `scripts/verify_llvm_coverage.py`.

Public behavior should reach coverage through a complete manifest fixture.
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
files, source, and distribution-relevant scripts must still be present and
usable.

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
