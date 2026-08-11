# Roadmap: the one source of truth

Status: canonical pending-work plan; current v1 evidence is recorded below

Reviewed: 2026-08-11

- Measured source/evidence revision: `36b939696415a962285d37f9120ff389aebf0205`
- Claim-ledger base revision: `36b939696415a962285d37f9120ff389aebf0205`
- Managed Pillow parity run: `84716077-aee7-4396-8328-e6735202b044`
  (1,449/1,449 passed at this revision)
- Managed Coverage MCP run: `54ce9d6c-3c1f-43e5-9120-c79984bc9166`
- Ingested Coverage MCP snapshot: `05b6674e-e7d9-43f4-b62b-a63a2ca45cf6`
- Project: `image-slash-star`, a Rust image-codec library with optional native
  AVIF support and a dependency-free WASM direction
- Detailed historical audit: [the old roadmap](roadmap.md)

### Latest API-038 implementation candidate

The caller-controlled decode-format restriction slice is implemented on `main`
at `e82034db8a8a95dc0c660e70bc4571d820ff3b69`:

- A caller can install an optional `DecodeFormatSet` on `DecodeLimits` or
  `DecodePolicy`. Absent means unrestricted for compatibility; an explicit
  empty set rejects every detected format. Signature detection remains
  independent, and explicit-format dispatch still validates the input before
  the policy is applied.
- Policy-aware inspection, explicit-format, complete-slice, prefix, token,
  and owned/borrowed source paths reject an excluded format with typed
  `ImageError::Unsupported { reason: PolicyDenied }` and the correct public
  operation stage. The owned lazy cache cannot bypass a later allow-list.
- The Rust-only `decode_allowed_formats_are_a_rust_policy_contract` contract
  covers all eight current format bits, allowed and rejected PNG/JPEG paths,
  explicit hints, partial-input and token entry points, and source cache reuse.
  Pillow cannot observe this caller policy or typed result, so no parity row is
  added.
- Exact local LLVM evidence at this source revision is 100% for lines
  (65,100/65,100), branches (8,478/8,478), functions (3,323/3,323), and
  regions (97,222/97,222). Native `feature_gate_tests` passed 48/48, the
  focused `wasm32-wasip1` runtime contract passed 1/1, and the native parity
  matrix passed 28/28 locally.
- Managed Coverage MCP and managed parity could not be rerun after this source
  change: even `project_context` returned `Transport closed` on repeated
  attempts. The older managed parity and coverage identifiers above remain
  the only accepted revision-bound evidence; this candidate is therefore
  locally verified but **not accepted as the current managed proof** and the
  claim ledger remains unchanged.

This slice explains why the feature exists: a server or WASM page may accept
only PNG files, for example, and should reject a JPEG before decoding it. It is
not a missing Pillow codec; it is a missing caller safety rule around the
codec. The broader API-038 inventory row remains open until this candidate's
managed evidence is recovered and the adjacent detection-policy boundaries
are reviewed.

### Latest API-045 implementation candidate — owned verification-result cache

The next narrow runtime slice is implemented on `main` at
`3506a70dc8b1681e55f6dfa5fc96d021f22a6ea3`:

- **Caller problem:** A server, UI, or WASM page may inspect the same immutable
  upload more than once—for example, verify it, pass a clone to another
  component, and verify again before decoding. Re-running the same
  format-specific verification parse wastes work. Pillow cannot express this
  owned-source/clone lifecycle, so this is not a missing Pillow codec feature.
- **Implemented behavior:** `EncodedImage` now has a separate
  `OnceLock<ImageResult<()>>`. The first supported `verify()` stores either the
  successful result or the deterministic `ImageError`; later owned calls and
  clones reuse that result. `verify_with_scope` checks that the requested scope
  is provided before reuse, stronger unsupported requests are never hidden by
  a weaker cached result, and still/sequence decode caches are unchanged.
  `EncodedImageView` caches only its immutable verification result per view;
  its pixel decodes remain uncached because it borrows caller bytes and is
  meant for short-lived use. A cloned view starts a separate verification
  cache. A bounded native integration contract also calls the same owned source
  concurrently from eight clones for both a valid PNG and a deterministic
  bad-CRC verification failure; WASI stays sequential because its test runtime
  has no portable thread support.
- **What this does not do:** It does not retain a parsed header/index,
  decompressor, temporary workspace, allocator state, or borrowed-view result.
  It closes only repeated verification-result work; parsed codec-state reuse,
  eviction, allocation counts, and retained-cache-byte measurements remain
  open under API-045/RN-003.
- **Local evidence:** Native `feature_gate_tests` passed 49/49, the native
  parity matrix passed 28/28, the focused `wasm32-wasip1` runtime contract
  passed 1/1, and the complete WASI feature-test binary compiled. Exact local
  LLVM coverage is 65,117/65,117 lines, 8,478/8,478 branches, 3,326/3,326
  functions, and 97,236/97,236 regions. Formatting, locked all-feature check,
  strict Clippy, rustdoc warnings, and doctests also pass.
- **Current runtime observation:** At this clean revision, a warm repeat of the
  fixed schema-`@3` benchmark passed all 1,421 active Pillow-visible rows in
  `0.930113 s` wall time (`2.785177 s` user, `0.176559 s` system,
  254,164,992-byte peak RSS). The Rust-only feature-gate suite passed in
  `1.489344 s` wall time (`2.187826 s` user, `0.094414 s` system,
  165,642,240-byte peak RSS). A preceding cold run was materially slower from
  build/cache state, confirming that these are host/cache/toolchain
  observations, not proof of a universal speedup; the standard workload does
  not isolate repeated verification and allocation counts, retained cache
  bytes, and WASM runtime cost remain unmeasured.
- **Evidence boundary:** The managed Coverage MCP transport again closed at
  `project_context`, so no fresh managed snapshot or parity rerun can replace
  the accepted baseline tuple. The candidate is locally verified but is not
  accepted as the current managed proof, and the claim ledger remains
  unchanged. No parity row, fixture, diagnostic origin, or coverage-only test
  was added.

This slice explains why the feature exists in five-year-old terms: if the same
picture box is checked twice, the machine should remember the answer instead
of opening the box twice. We remember only “yes” or “no,” not the machine's
private tools, so the optimization is safe and small. Pillow does not need to
implement it because Pillow is the comparison oracle for picture results; this
is a Rust ownership/performance rule around that oracle's visible behavior.

### Current WEP-022 evidence slice — VP8L predictor-mode maps

This is an evidence-only slice of the open WebP structural-property task. A
fixture name such as `lossless_predictor_mode10` is only a recipe name; it does
not prove that libwebp actually selected predictor mode 10, or even that the
stream contains a predictor transform. Pillow returns the finished pixels but
does not expose that internal transform map. The independent VP8L inspector now
keeps predictor-transform green-channel values in a dedicated
`predictor_modes` field, so ordinary decoded green samples cannot be mistaken
for codec state.

- The inspector and map verifier change is pinned to
  `a556a7f88f77ddbbcc325ebe7495491ffc91bb10`; the map records inspector SHA-256
  `8fbe5bbbf50f80bc89fbaa9df6c51a25ba09b6c1c395d8e59404764a70a77acd`.
- The property map now promotes 20 exact structural witnesses: 19 named
  decode streams (`vertical`, `horizontal`, `bilinear`, `diag_reverse`,
  `diamond`, `mode0_hybrid`, `mode5`–`mode10`, `mode13`, `product`, `radial`,
  `random_walk`, `saw`, `stripes`, and `xor`) plus the indexed encode artifact.
  Together they cover every predictor-mode value observed in this fixture set,
  including mode 4 from the encode artifact.
- The same map now promotes every successful color-cache width observed in the
  37-row lossless corpus: 1, 3, 4, 5, 7, and 10 bits. The added witnesses use
  actual cache-lookup activity; widths 2, 6, 8, 9, and 11 are not present in
  this corpus and are not claimed.
- The map also promotes all 17 distinct transform sequences present in the
  successful corpus: no transform, all observed color-indexing table sizes,
  four predictor-plus-color size pairs, and three subtract-green combinations.
  The color-transform property separately covers every observed block size
  (2, 3, and 9 bits). This proves the combinations that exist in the fixtures,
  not every legal VP8L sequence.
- The meta-Huffman and entropy-image witnesses now also cover the observed
  3×3 entropy image produced with six prefix-width bits in `mode0_hybrid`; the
  other observed dimensions remain 2×1 and 24×24. Unobserved dimensions and
  grouping patterns remain open.
- Distance witnesses now include the clamp, coordinate, full-width, and
  direct-distance branches (`10→1`, `5→16`, `24→4`, `23→256`, and `152→32`).
  Other mappings and malformed distance streams remain open.
- `python3 scripts/verify_webp_vp8l_property_map.py` passes with 15 witnessed
  properties, 94 named witnesses, 77 distinct active WebP rows, 79 structural
  witnesses, 40 malformed-parser witnesses, and all 37 active lossless success
  rows parsed. The property map and claim ledger hashes are updated together.
- The deliberately excluded `mode0`, `quadrants`, and `sparse` rows contain no
  predictor transform in the inspected stream; `steps` uses color indexing.
  Those rows remain candidates, as do predictor combinations not present in
  the current fixtures. WEP-022 therefore remains open; this slice proves only
  the named predictor maps and does not claim complete VP8L transform coverage.
- No Rust product code or compiled coverage surface changed. The accepted
  managed evidence tuple remains the baseline above because Coverage MCP still
  closes during `project_context`; this evidence-only slice must not be used to
  claim a refreshed managed snapshot. The next WEP-022 slice must select one
  additional structural combination or remaining inner-bitstream boundary and
  rerun the normal acceptance checks when managed evidence is available.

### Current fixed test-runtime observation

The repository's schema-`@3` benchmark was run on clean revision
`9623193182997ccb59084f0d9c5e4e10865a9ee5` with its fixed four-test-thread
budget. The Pillow-visible matrix passed all 1,421 active rows in `0.990571 s`
wall time (`2.813331 s` user, `0.201317 s` system, 256,983,040-byte peak RSS),
and the Rust-only feature-gate suite passed in `1.509984 s` wall time
(`2.188653 s` user, `0.103410 s` system, 181,010,432-byte peak RSS). These are
single-host/cache/toolchain observations, not a universal performance claim or
proof that one revision is faster. Allocation counts, retained cache bytes,
caller-buffer reuse, peak stack/recursion, and WASM runtime cost remain
unmeasured; an optimization should be accepted only after a repeatable
same-host comparison identifies a real bottleneck.

For local experimentation, the matrix-only lane also passed repeatedly with
eight test threads in `0.64`–`1.04 s` wall time, while the feature-gate lane
was unchanged at about `1.48`–`1.50 s` with four or eight threads. The standard
benchmark remains fixed at four threads so cross-revision results stay
comparable; eight threads is an observed local fast profile, not a new release
performance guarantee.

### Earlier RN-003 implementation candidate

The next resource-control slice is implemented on `main` at
`79ce9a98b32a39bbfab30023f4becd47ec3561d4`:

- A caller can set an inclusive decode checkpoint budget with
  `DecodeLimits::with_max_work_units` or `DecodePolicy::with_max_work_units`.
  Still, sequence, token-aware, and lazy `EncodedImage` paths preserve the
  typed `DecodeWorkUnits` result and do not retain a rejected lazy decode.
- The managed parity run `71ae3c8c-d6bf-4e49-99cf-913210a9311d` passed at this
  exact revision. Its 28 matrix workers passed all 1,421 active fixture rows
  plus their 28 worker-level checks: 1,449/1,449.
- The exact local LLVM report at this revision is 100% for lines
  (65,024/65,024), branches (8,474/8,474), functions (3,311/3,311), and
  regions (97,115/97,115). Native `feature_gate_tests` passed 47/47, and the
  focused `wasm32-wasip1` runtime contract passed 1/1.
- The managed Coverage MCP run
  `afcbc587-ed6f-4865-83b8-540bc8eee16b` was submitted at this exact
  revision, but the Coverage MCP transport closed before it returned a
  durable terminal status or snapshot. It is therefore **not accepted as
  same-revision coverage evidence yet**; the claim ledger still points to the
  last accepted snapshot above.

This candidate is behaviorally complete but remains evidence-pending until a
fresh managed snapshot is durably ingested. Do not move the claim-ledger base
revision or call this slice mergeable before that happens.

The old roadmap contains the original investigation and long-form findings.
This file is the authority for what is still to do, why it matters, what must
prove it, and which task comes next. When this file and the old audit disagree,
this file wins for status and order; the old audit remains useful background.

## The five-year-old explanation

Imagine we are building a machine that opens and creates picture boxes.

- A **Pillow parity test** asks, “Did our machine show the same picture and
  report the same ordinary result as Pillow?”
- A **Rust feature-gate test** asks, “Did our machine obey a Rust-only rule,
  such as a memory limit, cancellation token, source label, or WASM rule?”
- **Coverage** is a flashlight. It tells us which parts of the machine we
  actually made run. It does not tell us that every kind of picture exists.
- A **fixture** is a saved picture or a small, carefully-built picture used to
  ask one repeatable question.

We cannot use Pillow parity to test a Rust-only feature when Pillow cannot see
that feature. For example, Pillow does not return this crate's
`SourceDescriptor`, caller work budget, cancellation state, output-sink prefix,
target capability, diagnostic offset, or rollback result. Adding a Pillow row
for one of those would compare a real value with a made-up value. That would
look like progress while proving nothing.

Therefore every new task must answer four simple questions before code is
written:

1. What problem does this solve for a caller?
2. Can Pillow observe the result?
3. Which saved fixture or feature-gated contract proves it?
4. Which exact code paths and coverage origin does that evidence exercise?

If Pillow can observe it, use the existing Pillow-oracle parity manifest. If it
cannot, use an existing or extended fixture-based Rust feature-gate contract.
Do not create a unit test merely to turn a red coverage number green. If a path
cannot be reached by a real public behavior or a justified private defensive
model, remove it, simplify it, or document why it is intentionally unreachable.

## Five rolling workstreams

The old ten-package order is now executed through five independent rolling
workstreams. Each workstream takes one small v1 slice, proves it, and only then
chooses its next slice. A workstream is not allowed to claim that its whole
category is finished because one slice passed.

| Workstream | What it owns | v1 starting slice | Primary proof |
| --- | --- | --- | --- |
| W1 — Pillow-visible compatibility | Ordinary decoded pixels, dimensions, modes, observable metadata, frames, errors, and encoded bytes | Select one real open JPEG/PNG/GIF/BMP/ICO/TIFF/WebP row with a Pillow witness; implement it end-to-end | Existing fixture manifest plus `coverage_matrix_tests` and managed Pillow parity |
| W2 — Rust-only caller controls | Limits, cancellation, work budgets, caller-owned sinks, destination buffers, rollback, and typed Rust errors | Select one missing inclusive work/sink/limit boundary from `API-023`/`API-036`/`QA-026` | Existing `feature_gate_tests` fixture contract in native and relevant WASM lanes; no Pillow row |
| W3 — Coverage and defensive paths | Uncovered real branches, functions, regions, dead code decisions, and justified private defensive models | Start with one measured gap in a weak file; reach it through public behavior or register an allowed non-Pillow origin | Fresh Coverage MCP snapshot plus origin verifier; never a coverage-only unit test |
| W4 — AVIF and portable targets | Portable AVIF capability, native/WASM differences, target restrictions, sequence/encode gaps, and independent output compatibility | Select one bounded portable AVIF target capability or source-property contract | Feature-gated target fixture, native/WASI execution, and independent decoder evidence when bytes are emitted |
| W5 — Assurance, packaging, and documentation | Regeneration, determinism, fuzz/mutation, API/package checks, release evidence, and this roadmap | Build the Pillow-unreachable contract catalog and attach each category to an existing separate Rust-only test lane | Reproducible command/artifact, docs audit, claim ledger, and lane-specific test result |

Rolling rules:

1. Every v1 slice names its caller problem, exact source IDs, files changed,
   evidence origin, and next dependency.
2. W1 uses Pillow only for fields Pillow can actually observe. W2–W4 use the
   existing feature-gated integration contract for Rust-only fields.
3. W3 may add a private coverage model only when a valid public input cannot
   reach the state and the origin is recorded as `defensive_model`,
   `independent_implementation`, or `specification_reference`.
4. No workstream adds a unit test whose only purpose is to improve coverage.
5. A slice is mergeable only when its behavioral test, native/WASM evidence,
   verifiers, and same-revision Coverage MCP result are recorded. A failed or
   stale coverage artifact remains “not measured.”
6. Worktrees are disposable execution spaces. Only reviewed commits are
   integrated into `main`; agents do not push directly.

## Current v1 execution status

All five v1 slices were executed in separate local clones from the same
roadmap revision. These rows describe what is on `main` now; they do not claim
that an entire workstream is finished because one slice passed.

| Workstream | v1 slice actually executed | Main status | Evidence and next dependency |
| --- | --- | --- | --- |
| W1 | Pillow-visible GIF `enc_bilevel`, JPEG `enc_cmyk`, and WebP `I;16` normalization fixture projections | Integrated in the current tree | `Encode.gif`, `Encode.jpeg`, and `Encode.webp` have real Pillow-visible rows and retained encoded/raw fixtures. Managed parity run `84716077-aee7-4396-8328-e6735202b044` passes 1,449/1,449 at the measured revision. |
| W2 | `OutputSink` checkpoint/rollback plus cancellation at the final sink segment; API-038 decode-format allow-list candidate | Integrated locally; managed evidence pending for the latest candidate | `OutputSink` has caller-visible checkpoint/rollback behavior and the current all-feature `feature_gate_tests` contract passes 49/49. API-038 is Rust-only and has no Pillow row; its local exact coverage is recorded above, while managed evidence remains unavailable. |
| W3 | Coverage-origin inventory and justified defensive-path evidence | Evidence-only; no new product behavior | The origin verifier passes for 486 exact `cfg(coverage)` guards across 81 files, with no Pillow-parity origin assigned. Managed snapshot `05b6674e-e7d9-43f4-b62b-a63a2ca45cf6` is exact for all four aggregate metrics; the next audit cycle still owns any newly introduced gaps. |
| W4 | AVIF `iloc` item-location/source-provenance contract | Integrated in the current tree | Item extents and source locations are retained and asserted by the Rust-only feature contract. Native AVIF still depends on the pinned `libavif`/`dav1d`/`libaom` path, and portable sequence/encode support remains a product task. |
| W5 | Machine-checked unreachable-contract catalog and Cargo package surface | Integrated in the current tree | The ten-category catalog and exact package-path manifest both verify successfully; claim-ledger, diagnostic, license, and package-surface checks remain release evidence rather than Pillow parity. |

The five worker checkouts were disposable execution spaces. Their reviewed
slices are represented by reviewed commits on `main`; no worker pushed
directly. The accepted evidence tuple remains revision-bound to
`36b939696415a962285d37f9120ff389aebf0205`; the RN-003 candidate above is
newer but awaits managed coverage ingestion.

## Contract catalog: behavior Pillow cannot prove

This is the separate Rust-only list. “Cannot prove” means Pillow may have a
similar idea internally, but it cannot return this crate's exact field, token,
target, sink, or typed result for comparison.

The bounded v1 map is machine-checked by
`python3 scripts/verify_unreachable_contracts.py` from
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

The diagnostic provenance audit also maintains this canonical page. It checks
61 diagnostic cases: 38 use committed bytes that also have a Pillow parity row,
and 23 cases construct runtime mutations. Those counts describe separate
Rust-only diagnostic evidence; the unchanged bytes prove only the shared outer
result, and the runtime mutations are not Pillow matrix rows.

## What is already done

These are separate kinds of “done”; they must not be added together as if they
were the same unit.

| Evidence | Current result | What it proves |
| --- | ---: | --- |
| Confirmed correction records | `COR-001`–`COR-072` closed | The original reproduced defects and over-broad claims were corrected. |
| Test-system correction records | `TST-001`–`TST-010` closed | The original test/coverage-system defects were corrected. |
| Active fixture rows | 1,421/1,421 wired | 1,024 decode/inspect/verify rows plus 397 encode rows exist; none is planned or unwired. The two newest rows are WebP lossy/lossless `I;16` source-normalization cases. |
| Managed Pillow checks | 1,449/1,449 passed | Managed parity run `84716077-aee7-4396-8328-e6735202b044` is bound to revision `36b9396`. |
| Immediate correction queue | 0 | No newly confirmed defect is waiting ahead of capability work. |
| Current native all-feature ordinary contracts | 28/28 matrix tests and 49/49 feature-gate tests passed | The current local tree is behaviorally green for these Rust integration contracts. |
| Baseline implementation state | reviewed revision `36b9396` | The exact managed coverage result is bound to this source/evidence revision. |

The current native all-feature feature-gated contract has 49 passing assertions.
Some broader historical native/WASI matrix records still contain the known
libavif/dav1d/libaom-dependent AVIF alpha status-5 failure; that is real target
evidence, not a reason to relabel source-provenance work as Pillow parity.

## What “100% coverage” means here

The release target is 100% aggregate native all-feature **line, branch,
function, and region** coverage for the compiled implementation, including:

- Pillow parity tests for Pillow-visible behavior;
- feature-gated Rust fixtures for Rust-only behavior; and
- explicitly registered private coverage models for states that a valid public
  image cannot select, such as a defensive optimizer state.

It does **not** mean that every legal file in every image specification is
implemented, that the code is secure, or that a million random images were
tested. Those are different promises and have their own tasks below.

The current managed Coverage MCP snapshot is exact for the measured native
all-feature build:

| Metric | Covered | Total | Covered % | Gap | Gap % |
| --- | ---: | ---: | ---: | ---: | ---: |
| Lines | 64,909 | 64,909 | 100% | 0 | 0% |
| Branches | 8,464 | 8,464 | 100% | 0 | 0% |
| Functions | 3,301 | 3,301 | 100% | 0 | 0% |
| Regions | 96,968 | 96,968 | 100% | 0 | 0% |

That snapshot is `05b6674e-e7d9-43f4-b62b-a63a2ca45cf6`, produced by managed
run `54ce9d6c-3c1f-43e5-9120-c79984bc9166` with 86/86 tests passing and one
required artifact ingested. The LLVM JSON report carries the warning that
segments are normalized to segment-start lines; aggregate region coverage is
preserved from the report summaries. This closes RN-001 for the measured tree;
it does not claim complete format support or close the product roadmap.

There are no aggregate line, branch, function, or region gaps in this measured
build. RN-002 selected and closed one real WebP behavior boundary; the next
work item is therefore RN-003, a product/evidence boundary rather than a
coverage-only repair. Any future source change must rerun all four coverage
measures.

Coverage work follows this order:

1. Read the uncovered code and decide whether it is a real public behavior,
   a defensive state, or dead/unsupported code.
2. For real public behavior, add or extend a saved fixture in the appropriate
   integration contract.
3. For Rust-only behavior, extend `feature_gate_tests` or another existing
   feature-gated integration contract; do not add a unit-test hook.
4. For genuinely unreachable private state, use a declared coverage origin
   (`defensive_model`, `independent_implementation`, or
   `specification_reference`) and update the origin verifier.
5. Re-run parity, feature lanes, and Coverage MCP at the same revision.
6. Do not call the task complete until all four coverage measures and the
   task's behavioral evidence pass.

## The exact dependency order

The following packages are the work queue. `NEXT` means work can begin after
the preceding package's evidence is accepted. `LATER` means its prerequisites
are not complete. `PARKED` means it is deliberately not current work.

### RN-001 — Coverage baseline and honest accounting — DONE for this measured tree

**Why:** We need to know which flashlight beams are missing before choosing
new tests. Otherwise we may add tests that do not reach the code we think they
reach.

**Work/result:** The all-feature native Coverage MCP snapshot was refreshed at
the current HEAD; the exact aggregate result is recorded above. Real behavior
uses Pillow-visible fixtures or Rust-only feature contracts, private models
remain origin-registered, and the claim ledger is bound to the same HEAD.

**Source IDs:** `QA-003`, `QA-010`, `QA-020`, `QA-030`, `DOC-005`.

**Done:** the fresh snapshot reports 100% lines, branches, functions, and
regions, with no skipped artifact and with Pillow, Rust-only, and private-model
origins still distinct. Reopen this item on the next source revision if any
metric falls below 100%.

### RN-002 — WebP 16-bit luminance normalization boundary — DONE (selected slice)

**Why:** Pillow accepts `I;16` grayscale images as WebP inputs. Before this
slice, the Rust encoder rejected that real caller input as an unsupported mode.
Users converting scientific, camera, or PNG 16-bit grayscale data therefore
could not keep the Pillow-visible WebP workflow in this crate.

**Work/result:** The encoder now accepts `ImageMode::L16`, reads the PNG
little-endian sample bytes, clamps each sample to Pillow's `0..=255` RGB
conversion behavior, and expands it to RGB. The no-token path remains a tight
loop; the caller-token path checkpoints every 1,024 pixels. The fixture
`tests/fixtures/input/images/png/l16_clamp.png` deliberately contains both
values at or below 255 and values above 255. The manifest and generated
lossy/lossless WebP references add `enc_lossy_l16` and `enc_lossless_l16`.

**Files/evidence:** `src/codecs/webp/encode/mod.rs`,
`scripts/generate_test_assets.py`, `manifest.yaml`, the generated matrix and
references, plus the WebP property-map pin. Managed parity is
`84716077-aee7-4396-8328-e6735202b044` (1,449/1,449). Managed Coverage MCP
snapshot `05b6674e-e7d9-43f4-b62b-a63a2ca45cf6` is 100% lines, branches,
functions, and regions. Local matrix, feature-gate, check, strict Clippy, and
WebP structural-map verification also pass.

**Scope:** This closes one selected `WEP-004` mode-normalization boundary and
the supporting `API-023`/`API-036`/`QA-026` evidence path. It does not close
the broader WebP mode family: integer/float/YCbCr normalization, remaining
WebP interior behavior, and resource-limit work stay in the open inventory.

**Done when:** satisfied by the committed fixture, the Pillow-visible lossy
and lossless rows, unchanged parity, Rust-only token-path coverage model, and
same-revision exact coverage result above.

### Completed W1 slice — JPG-001 CMYK JPEG encode — DONE (selected row)

**Caller problem:** A caller may decode a CMYK JPEG, adjust or inspect its
four-channel pixels, and save those pixels back as JPEG. Before this slice the
Rust encoder rejected `Cmyk8`, even though Pillow accepts the workflow and
emits an Adobe-marked four-component JPEG.

**Pillow answer:** Pillow can observe the encoded component layout, Adobe APP14
marker, decoded CMYK mode, dimensions, pixels, and exact retained reference
bytes. This is therefore a real parity row, not a Rust-only compatibility
claim.

**Implemented behavior:** The JPEG encoder accepts direct `Cmyk8` input,
emits inverted CMYK samples with Adobe transform `0`, writes the four `C/M/Y/K`
component identifiers, and rejects only the still-open progressive-CMYK option.
The committed `enc_cmyk` fixture asserts the Pillow-visible result and the
encoded-byte reference.

**Evidence:** Implementation and fixture projection landed in
`54097c906f6ba098e441ccd4c39cb33d5ed5a820`; managed Pillow parity
`84716077-aee7-4396-8328-e6735202b044` passed 1,449/1,449 at the accepted
revision, including `Encode.jpeg/enc_cmyk`. The current native matrix remains
28/28, and the exact local LLVM build remains 100% across all four aggregate
metrics.

**Scope:** This removes only `JPG-001` from the active inventory. It does not
close YCbCr/bilevel input as a family, progressive CMYK, JPEG source-color
provenance, uncommon JPEG classes, or JPEG metadata/options.

### RN-003 — Resource limits, interruption, and output recovery — IN PROGRESS

**Why:** A picture library must not unexpectedly spend all of a caller's
memory or time, and a failed output write must not pretend that a complete
file was delivered.

**Work:** Finish transient allocation/peak accounting, deeper interruption and
progress semantics, remaining work-budget checkpoints, short-write behavior,
rollback, cleanup, and error precedence. Keep recoverable-OOM promises out of
scope unless the public contract can actually support them.

#### Completed candidate slice — inclusive decode work budgets

**Caller problem:** A caller may know that decoding a very large picture is
expensive, but the old decode policy could only limit bytes, dimensions, or
frames. It had no deterministic “stop after this many decoder checkpoints”
rule. This matters for a UI, server, or WASM page that must stay responsive.

**Pillow answer:** Pillow cannot prove this result. Pillow does not expose
this crate's checkpoint counter, caller token, typed `DecodeWorkUnits` error,
or lazy-cache state. The proof is therefore Rust-only, using the existing
fixture-backed `feature_gate_tests` lane; no parity row is added.

**Implemented behavior:** `DecodeLimits` and `DecodePolicy` expose an
inclusive `max_work_units` bound. Zero rejects at the first real checkpoint;
the exact discovered boundary succeeds; one less rejects with the observed
count; still and sequence operations retain their operation identity; token
combination preserves cancellation precedence; and a rejected lazy decode
does not poison `EncodedImage`'s shared cache. The complete-slice sequence
fallback is also exercised with a JPEG still input.

**Source/evidence:** `src/cancel.rs`, `src/decode_policy.rs`,
`src/codecs/error.rs`, `src/codecs/mod.rs`, `src/lib.rs`, `src/source.rs`,
`src/types/error.rs`, and `tests/feature_gate_tests.rs`. The implementation
commit is `79ce9a98b32a39bbfab30023f4becd47ec3561d4`; the local native/WASI
and parity evidence is listed in the candidate block above.

**Remaining dependency:** The managed Coverage MCP run must be rerun or
recovered and its 100% snapshot durably ingested at this revision. Until then,
this slice is implemented and locally verified but not yet accepted as the
roadmap's revision-bound coverage proof.

#### Current candidate slice — API-038 allowed decode formats

**Caller problem:** An application that accepts untrusted bytes may be willing
to decode PNG but not JPEG, AVIF, or another container. The signature detector
could identify formats, and the explicit-format API could validate a hint, but
`DecodePolicy` had no way to express the caller's allow-list across complete,
partial, token-aware, and lazy-source flows.

**Pillow answer:** Pillow can decode the image, but it cannot observe this
crate's caller-selected policy, the rejected format set, or the typed
`PolicyDenied` result. This is therefore a Rust-only feature-gated contract;
it must not become a fabricated parity row.

**Implemented behavior:** `DecodeFormatSet` provides const constructors and
membership operations for all current formats. An absent set preserves the
unrestricted compatibility behavior; an explicit empty set rejects all
detected formats. Policy-aware inspection and decode paths enforce the set
after signature detection, explicit format validation remains separate, and
owned/borrowed source decode checks the retained format before cache reuse.

**Source/evidence:** `src/decode_policy.rs`, `src/lib.rs`, `src/source.rs`,
`src/types/error.rs`, and `tests/feature_gate_tests.rs`; implementation commit
`e82034db8a8a95dc0c660e70bc4571d820ff3b69`. The exact local LLVM report at
this revision is 65,100/65,100 lines, 8,478/8,478 branches, 3,323/3,323
functions, and 97,222/97,222 regions. Native feature tests pass 48/48, the
focused WASI contract passes 1/1, and the local parity matrix passes 28/28.
README, architecture, testing, AVIF portability notes, and rustdoc describe
the public policy and its target-independent relationship to AVIF capability.

**Remaining dependency:** Coverage MCP transport was closed for repeated
`project_context` requests, so no durable same-revision managed snapshot or
managed parity rerun is available. The existing accepted claim-ledger tuple
must remain unchanged; this candidate cannot be promoted until managed
evidence is recovered.

**Source IDs:** `API-014`, `API-017`, `API-018`, `API-023`, `API-030`,
`API-036`, `API-038`, `API-041`, `API-043`, `API-044`, `API-045`, `API-046`, `QA-016`,
`QA-020`, `QA-026`, `QA-030`, plus the remaining resource rows in the codec
groups below.

**Done when:** each resource or sink boundary has a typed result, an
inclusive boundary fixture, a no-partial-output assertion where applicable,
and a feature-gated origin when Pillow cannot observe it.

### RN-004 — Metadata and source facts — LATER

**Why:** A caller may need to know what the file said even when decoded pixels
look the same. Pillow's normal result does not expose all of those labels, so
the labels need separate source-provenance contracts.

**Work:** Complete the remaining AVIF item/property graph and color fields,
codec metadata models, source subtype preservation, and declared-versus-
confirmed facts without silently changing pixels.

**Source IDs:** `API-008`, `API-019`, `API-033`, `API-034`, `API-047`,
`API-048`, `AVF-004`, `AVF-009`, `AVF-011`, `AVF-012`, `AVF-016`, `AVF-026`,
`AVF-028`, `AVF-029`, `AVF-031`, `AVF-035`, `JPG-003`, `JPG-012`, `JPG-015`,
`PNG-003`, `PNG-008`, `PNG-010`, `PNG-015`, `PNG-016`, `PNG-017`, `PNG-020`,
`GIF-007`, `GIF-010`, `GIF-014`, `GIF-015`, `GIF-016`, `GIF-017`, `GIF-020`,
`BMP-002`, `BMP-003`, `BMP-008`, `BMP-010`, `BMP-015`, `BMP-016`, `BMP-018`,
`BMP-020`, `TIF-007`, `TIF-012`, `TIF-020`, `TIF-022`, `TIF-028`, `TIF-029`,
`WEP-001`, `WEP-005`, `WEP-011`, `WEP-012`, `WEP-015`, `WEP-017`, `WEP-018`,
`WEP-020`.

**Done when:** the source state is retained exactly where promised, a
feature-gated fixture proves each non-Pillow field, and the documentation says
clearly what is provenance versus pixel processing.

### RN-005 — Frames, pages, strips, tiles, and partial input — LATER

**Why:** Opening one frame should not require pretending that the whole movie
or every TIFF page is one still picture. A caller also needs to know whether a
short input is incomplete or truly malformed.

**Work:** Extend source-bound frame/page/strip/tile access, random access,
iteration, partial-input lifecycle, progress, and cache behavior while keeping
the eager convenience APIs compatible.

**Source IDs:** `API-018`, `API-027`, `API-043`, `API-044`, `API-045`,
`API-050`, `API-051`, `API-052`, `API-053`, `API-054`, `AVF-013`, `AVF-022`,
`AVF-023`, `AVF-024`, `AVF-025`, `AVF-034`, `GIF-009`, `GIF-013`, `GIF-019`,
`PNG-012`, `PNG-013`, `PNG-018`, `PNG-020`, `TIF-009`, `TIF-013`, `TIF-016`,
`TIF-025`, `TIF-026`, `TIF-030`, `WEP-003`, `WEP-010`, `WEP-019`, `WEP-020`.

**Done when:** a frame/page fixture proves the selected access path, the
one-shot result stays identical, incomplete input has a stable lifecycle, and
no second accidental cache or sequence model is introduced.

### RN-006 — Portable AVIF completion — LATER

**Why:** Native AVIF currently does more than WASM. The final promise is that a
published codec should work predictably on every supported target without
silently depending on a native library.

**Work:** Close the portable still subset's remaining target boundary; add
sequence and encode support; expand bit depth, monochrome, planar YUV, alpha,
tracks, timing, progressive/layered content, gain maps, auxiliary selection,
grid composition, item relationships, strictness, limits, random access, AV1
syntax, and independent decoder/browser compatibility as each is justified.
Generate capability decisions from FileTypeBox declarations, item codec
declarations, and target restrictions. Do not treat the completed FileTypeBox
declaration-retention slice as complete decoder capability.

**Source IDs:** all 33 AVIF rows: `AVF-001`–`AVF-009`, `AVF-011`–`AVF-020`,
and `AVF-022`–`AVF-035`.

**Done when:** native and WASM behavior has an explicit capability table,
every claimed operation has fixture evidence on its target, portable encoded
bytes have an independent compatibility check, and all AVIF coverage is fresh.

### RN-007 — Remaining codec capabilities — LATER

**Why:** The current parity suite proves the pinned Pillow contract, but it
does not claim every legal JPEG, PNG, GIF, BMP, ICO, TIFF, or WebP feature.

**Work:** Close only the codec rows that have a real caller need and a clear
oracle/specification. Add one reverse-mappable fixture class at a time; keep
unsupported legal classes explicitly unsupported rather than accidentally
malformed.

**Source IDs:** 20 JPEG (`JPG-*`), 15 PNG (`PNG-*`), 17 GIF (`GIF-*`), 20 BMP
(`BMP-*`), 20 ICO (`ICO-*`), 26 TIFF (`TIF-*`), and 20 WebP (`WEP-*`) rows,
listed in the complete inventory below.

**Done when:** the selected source/mode/option/error class has a committed
fixture, exact Pillow comparison when observable, independent specification
evidence when not, and a documented target/capability result.

### RN-008 — WASM, packaging, and platform support — LATER

**Why:** Compiling a Rust library for WASM is not the same as proving that a
browser or JavaScript user can use it.

**Work:** Run the semantic matrix in a real WASM runtime; define JS bindings,
artifact roots, memory/copy behavior, workers, package contents, target-OS
support, native-oracle provenance, linkage, feature versioning, and
reproducible release artifacts.

**Source IDs:** all 33 feature rows: `FTR-001`–`FTR-024`, `FTR-027`,
`FTR-029`–`FTR-031`, `FTR-033`–`FTR-035`, `FTR-037`, `FTR-038`; plus
`QA-001`, `QA-002`, `QA-019`, `QA-022`, `QA-023`, `QA-038`.

**Done when:** a clean consumer can install the claimed artifact, run the
semantic tests in the claimed runtime, and reproduce the documented size and
capability results.

### RN-009 — Assurance and release evidence — LATER

**Why:** Passing the normal examples is not the same as checking panic freedom,
fuzz resilience, deterministic output, generator provenance, or API stability.

**Work:** Add the compact no-panic matrix, fuzz/mutation/differential lanes,
determinism policy, generator regeneration gate, debug-vs-optimized comparison,
concurrency checks, metamorphic container variants, error-message policy,
public API diff, and release package verification.

**Source IDs:** `QA-005`, `QA-006`, `QA-008`, `QA-009`, `QA-011`, `QA-012`,
`QA-013`, `QA-021`, `QA-024`, `QA-027`, `QA-028`, `QA-031`, `QA-033`,
`QA-034`, `QA-035`, `QA-036`, `QA-037`, `QA-039`, `QA-040`, `QA-041`,
`QA-042`.

**Done when:** each assurance claim has a reproducible command, a bounded
artifact/result, a clear evidence origin, and no claim is promoted from
“tested once” to “complete everywhere.”

### RN-010 — Documentation and project lifecycle — LATER

**Why:** Users need to know how to install, upgrade, contribute, report a
security problem, and understand which promises are real.

**Work:** Keep this file synchronized with code and tests; generate accurate
capability tables; add a clean-consumer package smoke test; maintain changelog
and release links; define governance and recovery before production reliance.

**Source IDs:** `DOC-002`–`DOC-008`.

**Done when:** every material claim has an independent source, a revision/date
scope, a validation command, and a visible proved/planned/unknown label.

## Complete open-task inventory

The following is the exact set of active roadmap IDs at this review. A task is
not complete until its ID is removed from this list and its current behavior is
moved into the appropriate contract document. The list contains **269 active
finding rows**.

| Area | Count | Open IDs |
| --- | ---: | --- |
| Common API | 26 | `API-008`, `API-014`, `API-017`, `API-018`, `API-019`, `API-020`, `API-023`, `API-026`, `API-027`, `API-030`, `API-033`, `API-034`, `API-036`, `API-038`, `API-041`, `API-043`, `API-044`, `API-045`, `API-046`, `API-047`, `API-048`, `API-050`, `API-051`, `API-052`, `API-053`, `API-054` |
| JPEG | 19 | `JPG-002`, `JPG-003`, `JPG-004`, `JPG-006`–`JPG-021` |
| PNG | 15 | `PNG-003`, `PNG-004`, `PNG-005`, `PNG-006`, `PNG-008`, `PNG-010`–`PNG-013`, `PNG-015`–`PNG-020` |
| GIF | 17 | `GIF-002`, `GIF-005`–`GIF-007`, `GIF-009`–`GIF-021` |
| BMP | 20 | `BMP-001`–`BMP-020` |
| ICO/CUR | 20 | `ICO-001`, `ICO-002`, `ICO-004`–`ICO-021` |
| TIFF | 26 | `TIF-002`, `TIF-003`, `TIF-005`–`TIF-014`, `TIF-016`–`TIF-018`, `TIF-020`–`TIF-030` |
| WebP | 20 | `WEP-001`, `WEP-003`–`WEP-005`, `WEP-007`–`WEP-022` |
| AVIF | 33 | `AVF-001`–`AVF-009`, `AVF-011`–`AVF-020`, `AVF-022`–`AVF-035` |
| Features/package | 33 | `FTR-001`–`FTR-024`, `FTR-027`, `FTR-029`–`FTR-031`, `FTR-033`–`FTR-035`, `FTR-037`, `FTR-038` |
| Assurance | 33 | `QA-001`, `QA-002`, `QA-003`, `QA-005`, `QA-006`, `QA-008`, `QA-009`–`QA-013`, `QA-016`, `QA-019`–`QA-024`, `QA-026`–`QA-028`, `QA-030`, `QA-031`, `QA-033`–`QA-042` |
| Documentation | 7 | `DOC-002`–`DOC-008` |

The shorthand ranges above expand only to the IDs actually present in the
current audit. The old roadmap remains the detailed description of each
finding while this table is the canonical status inventory.

These 269 rows are not 269 equal-sized coding tasks. A row may be a small
documentation or policy decision, a new fixture, a codec algorithm, a WASM
runtime experiment, or a release gate. The reliable “how much is left” numbers
today are the exact 269 active finding rows, zero aggregate coverage gaps, and
the explicit dependency order; an hour estimate would be invented until the
next slice is chosen and measured.

## Parked, not pending

`FMT-000`–`FMT-013` are 14 possible future format candidates. They are not
current tasks. Do not start one until portable AVIF, runtime WASM behavior,
resource limits, and the current format contracts are accepted.

The project also deliberately does not plan resizing, cropping, drawing,
filters, general compositing, runtime Python/Pillow, plugin-loaded codecs,
filesystem sandbox policy, or replacing dependency-free codecs with native
libraries.

## Rules for every task

Every task must finish with all of these:

- a reason a caller needs it;
- a dependency check against the package order above;
- a committed fixture or a clearly documented private model;
- Pillow parity only when Pillow can see the result;
- an existing feature-gated integration contract for Rust-only behavior;
- no new unit test whose only purpose is coverage;
- exact error/byte/source-state assertions at the relevant boundary;
- native and relevant WASM runtime evidence;
- strict formatting, Clippy, rustdoc, and repository verifiers;
- a fresh Coverage MCP result at the same revision; and
- updated README, architecture, testing, AVIF, and this roadmap when the
  public contract changes.

Coverage is never “fixed” by hiding code from the compiler, suppressing a
warning, inventing a Pillow field, or adding a synthetic test that users cannot
cause. The flashlight must shine on real machine behavior.

## Required acceptance commands

The exact registered commands and their approval remain in Coverage MCP. The
local checks that accompany each accepted slice are:

```text
cargo fmt --all
cargo check --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --all-features --locked --test coverage_matrix_tests -- --nocapture
RUSTFLAGS="--cfg coverage" cargo +nightly check --all-features --locked
RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps --locked
cargo test --doc --all-features --locked
python3 scripts/verify_claim_ledger.py
python3 scripts/verify_coverage_origins.py
python3 scripts/verify_diagnostic_provenance.py
python3 scripts/verify_unreachable_contracts.py
python3 scripts/verify_package_surface.py
python3 scripts/verify_third_party_licenses.py
git diff --check
```

The managed Pillow command and the managed nightly coverage command are
different commands with different evidence origins. A passing parity command
does not close a Rust-only task; a passing Rust-only contract does not add a
Pillow parity row.

## What changes the numbers

- A new real feature adds a fixture row only if Pillow can observe it.
- A Rust-only feature adds or extends a feature-gated integration contract.
- A private defensive state adds an explicitly inventoried coverage model only
  when no public input can reach it.
- A completed task is removed from the open-ID table and recorded in the
  owning current-contract document.
- A docs-only commit updates revision labels but does not pretend to refresh
  coverage.

This is the whole rule in one sentence: **build only a feature that solves a
real caller problem, then prove it with the test that can actually see it.**
