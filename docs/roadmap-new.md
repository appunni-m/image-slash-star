# Roadmap: the one source of truth

Status: canonical pending-work plan

Reviewed: 2026-08-10

- Baseline repository revision for this review: `f8e57d67a9d6fe6509e63a4e435eabf3806364b1`
- Last code-and-test revision: `371354b0a92d83f4384b7a9129ddc63bcbb326d3`
- Claim-ledger base revision: `487348d01389eb8d100b8a668c9921d97634c022`
- Project: `image-slash-star`, a Rust image-codec library with optional native
  AVIF support and a dependency-free WASM direction
- Detailed historical audit: [the old roadmap](roadmap.md)

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
| W1 | Pillow-visible packed JPEG `L1` input expands to luminance output | Held in review checkout; not integrated | Isolated JPEG parity passed 1/1 and local checks passed. Its manifest/projection changes need a fresh same-revision parity and Coverage MCP acceptance before their hashes can enter the claim ledger. Native AVIF remains the current all-feature blocker. |
| W2 | Cancellation during the final output-sink segment is observed before `flush` | Integrated as `28bd91d` | Existing fixture-backed feature-gate contract passes 45/45. This is Rust-only, so it has no Pillow row. A fresh accepted coverage snapshot is still required. |
| W3 | GIF dispatch rejects PNG options with a typed parameter error and zero writes | Integrated as `3c48f2d` | Existing `static.gif` fixture and feature-gate contract pass 45/45; origin verifier passes. A fresh accepted coverage snapshot is still required. |
| W4 | Native/WASI/unknown-target capability distinction | Held in review checkout; not integrated | Native and WASI feature-gate checks plus unknown-target compile checks passed. Capability-table changes need a fresh same-revision acceptance snapshot; native AVIF remains the blocker. |
| W5 | Machine-checked catalog of contracts Pillow cannot prove | Integrated as `f8a372a` | Nine categories are mapped: 8 covered by existing Rust-only evidence and 1 explicitly planned. The verifier, claim ledger, docs audit, and CI wiring pass. |

The held W1 and W4 commits remain recoverable in their isolated review
checkouts. They are intentionally not counted as accepted `main` behavior
until the evidence ledger and coverage snapshot can refer to the same code
revision.

## Contract catalog: behavior Pillow cannot prove

This is the separate Rust-only list. “Cannot prove” means Pillow may have a
similar idea internally, but it cannot return this crate's exact field, token,
target, sink, or typed result for comparison.

The bounded v1 map is machine-checked by
`python3 scripts/verify_unreachable_contracts.py` from
`tests/fixtures/unreachable_contract_manifest.json`. `covered` means that the
manifest names an existing fixture-backed integration contract; `planned`
means that no such contract is claimed yet. The map is an evidence index, not
a claim that every legal format state is implemented.

The verifier also parses this nine-row table. It requires each row's status and
Pillow-parity column to match the manifest, every covered row to name the exact
manifest evidence paths, and the planned allocation/stack/coverage row to say
that no category-specific evidence is claimed while naming its bounded context
paths. This is documentation-integrity evidence only: it does not promote the
planned category to covered and does not add any Rust-only result to Pillow
parity.

| Map ID | Rust-only contract | Why Pillow cannot prove it | v1 status | Separate evidence | Pillow parity |
| --- | --- | --- | --- | --- | --- |
| `decode-encode-policy-limits` | `DecodePolicy` and `EncodePolicy` limits | Pillow does not expose this crate's pre-detection, canvas, metadata, decoded-byte, encoded-output, or work-budget result with the same boundary/error fields | `covered` | Manifest evidence: `tests/decode_policy_tests.rs`, `tests/feature_gate_tests.rs` | `excluded` |
| `cancellation-work-budgets` | Cancellation and work budgets | Pillow has no caller-owned `CancellationToken`, checkpoint budget, or `EncodeWorkUnits` result | `covered` | Manifest evidence: `tests/feature_gate_tests.rs` | `excluded` |
| `output-sink-delivery` | `OutputSink` delivery | Pillow does not accept this crate's dependency-free sink, expose delivered prefixes, flush failures, or rollback semantics | `covered` | Manifest evidence: `tests/feature_gate_tests.rs` | `excluded` |
| `caller-owned-destination-buffers` | Caller-owned destination buffers | Pillow does not expose `decode_into` capacity, short-destination rejection, or no-partial-write guarantees | `covered` | Manifest evidence: `tests/feature_gate_tests.rs` | `excluded` |
| `source-provenance` | Source provenance | `SourceDescriptor`, FileTypeBox facts, AVIF item/property identity, raw source relationships, and declared-versus-confirmed fields are not Pillow result fields | `covered` | Manifest evidence: `tests/feature_gate_tests.rs` | `excluded` |
| `structured-diagnostics` | Structured diagnostics | Rust diagnostic kind, offset, consumed extent, recovery status, and provenance are not Pillow's ordinary return shape | `covered` | Manifest evidence: `tests/feature_gate_tests.rs`, `scripts/verify_diagnostic_provenance.py` | `excluded` |
| `feature-target-capability` | Feature and target capability | Pillow does not model this crate's Cargo feature-disabled errors or native versus `wasm32-wasip1` capability table | `covered` | Manifest evidence: `tests/feature_gate_tests.rs`, `tests/capability_table.rs` | `excluded` |
| `cache-concurrency-api-lifecycle` | Cache/concurrency/API lifecycle | Pillow does not expose `EncodedImage` lazy-cache states, Rust clone sharing, or this crate's frame/page lifecycle | `covered` | Manifest evidence: `tests/feature_gate_tests.rs` | `excluded` |
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
| Active fixture rows | 1,417/1,417 wired | 1,024 decode/inspect/verify rows plus 393 encode rows exist; none is planned or unwired. |
| Managed Pillow checks | 1,445/1,445 passed | The current Pillow-observable parity surface passed at the last managed code revision. |
| Immediate correction queue | 0 | No newly confirmed defect is waiting ahead of capability work. |
| Baseline implementation state | clean at `f8e57d6` | The implementation state being measured was already pushed to `origin/main` before this roadmap rewrite. |

The separate feature-gated contract has 45 non-Pillow assertions per native
and `wasm32-wasip1` lane. The current managed run is 44/45 because the native
AVIF alpha decoder still returns status 5. That failure is real test evidence,
not a reason to relabel the source-descriptor work as Pillow parity.

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

The accepted Coverage MCP snapshot is old relative to the current revision:

| Metric | Covered | Total | Covered % | Gap | Gap % |
| --- | ---: | ---: | ---: | ---: | ---: |
| Lines | 55,926 | 56,803 | 98.456% | 877 | 1.544% |
| Branches | 8,011 | 8,228 | 97.363% | 217 | 2.637% |
| Functions | 3,122 | 3,218 | 97.017% | 96 | 2.983% |
| Regions | 85,972 | 87,930 | 97.773% | 1,958 | 2.227% |

That snapshot is `44cec31e-7345-4673-a9a4-e9f8fa21cc08`, measured at
`1d1b36100925f830408f5d41f0026e71fd220d6e`. It is a baseline, not a current
100% claim. The next valid coverage result must be generated at the current
code revision and ingested by Coverage MCP; a stale or skipped artifact means
“not measured,” never “unchanged.”

The latest current-revision runtime matrix is Coverage MCP run
`9484b9ce-5aa2-4185-a114-a0c20762bec5` at `f05d2771b1799b842c9579ad73f78bb415708a33`:
44/45 assertions passed. The only failure is
`source_alpha_matches_the_container_contract` in native AVIF, where the
decoder returns status 5; the WASI WebP and WASI AVIF lanes passed. This
command does not emit a coverage artifact, so the aggregate coverage numbers
above remain the accepted baseline rather than fresh coverage for `main`.

The largest measured weak areas are:

| File | Lines | Branches | Functions | Regions | First question |
| --- | ---: | ---: | ---: | ---: | --- |
| `src/codecs/webp/encode/mod.rs` | 649/775 | 88/102 | 49/62 | 1,052/1,327 | Which real WebP encode modes or work boundaries are missing? |
| `src/codecs/avif/encode.rs` | 587/640 | 46/54 | 38/48 | 825/897 | Which native-only AVIF options or errors need a supported fixture? |
| `src/codecs/gif/encode.rs` | 2,704/2,907 | 418/462 | 159/189 | 4,343/4,670 | Which source-mode and quantizer boundaries are still unproved? |
| `src/codecs/mod.rs` | 1,064/1,118 | 108/120 | 92/96 | 1,328/1,377 | Which dispatch/capability/error combinations lack a lane? |
| `src/codecs/tiff/encode.rs` | 844/886 | 131/134 | 38/50 | 1,504/1,569 | Which page/strip/output paths are not exercised? |
| `src/types/mod.rs` | 1,596/1,644 | 123/134 | 160/176 | 1,590/1,639 | Which public-state constructors/accessors lack real use? |
| `src/lib.rs` | 765/793 | 94/98 | 76/78 | 1,149/1,223 | Which root API error and dispatch paths need a fixture? |

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

### RN-001 — Coverage baseline and honest accounting — NEXT

**Why:** We need to know which flashlight beams are missing before choosing
new tests. Otherwise we may add tests that do not reach the code we think they
reach.

**Work:** Refresh the all-feature native Coverage MCP snapshot at the current
revision; classify every uncovered line/branch/function/region; close real
behavior with fixtures; remove dead paths; register only justified private
models; keep the origin inventory and claim ledger green.

**Source IDs:** `QA-003`, `QA-010`, `QA-020`, `QA-030`, `DOC-005`.

**Done when:** a fresh snapshot reports 100% lines, branches, functions, and
regions, with no skipped artifact and with Pillow, Rust-only, and private-model
origins still distinct.

### RN-002 — Remaining WebP interior boundary — NEXT after RN-001

**Why:** WebP has the largest measured weak implementation area. The next
boundary must represent a real encoder/decoder operation or resource limit,
not a coverage-only branch.

**Work:** Choose one remaining WebP inner bitstream, result-trace, transform,
or transient-work boundary after an ownership/lifetime audit. Preserve the
fast no-token path. Use existing Pillow rows for unchanged byte/error behavior
and the existing feature-gated contract for caller-budget behavior.

**Source IDs:** `API-023`, `API-036`, `QA-026`, `WEP-003`, `WEP-004`,
`WEP-005`, `WEP-007`, `WEP-008`, `WEP-009`, `WEP-010`, `WEP-011`, `WEP-012`,
`WEP-013`, `WEP-014`, `WEP-015`, `WEP-016`, `WEP-017`, `WEP-018`, `WEP-019`,
`WEP-020`, `WEP-021`, `WEP-022`.

**Done when:** a committed fixture reaches the selected boundary, the
feature-gated contract proves the Rust-only result if needed, parity remains
unchanged, and the relevant uncovered coverage disappears for the right
origin.

### RN-003 — Resource limits, interruption, and output recovery — LATER

**Why:** A picture library must not unexpectedly spend all of a caller's
memory or time, and a failed output write must not pretend that a complete
file was delivered.

**Work:** Finish transient allocation/peak accounting, deeper interruption and
progress semantics, remaining work-budget checkpoints, short-write behavior,
rollback, cleanup, and error precedence. Keep recoverable-OOM promises out of
scope unless the public contract can actually support them.

**Source IDs:** `API-014`, `API-017`, `API-018`, `API-023`, `API-030`,
`API-036`, `API-041`, `API-043`, `API-044`, `API-045`, `API-046`, `QA-016`,
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
moved into the appropriate contract document. The list contains **270 active
finding rows**.

| Area | Count | Open IDs |
| --- | ---: | --- |
| Common API | 26 | `API-008`, `API-014`, `API-017`, `API-018`, `API-019`, `API-020`, `API-023`, `API-026`, `API-027`, `API-030`, `API-033`, `API-034`, `API-036`, `API-038`, `API-041`, `API-043`, `API-044`, `API-045`, `API-046`, `API-047`, `API-048`, `API-050`, `API-051`, `API-052`, `API-053`, `API-054` |
| JPEG | 20 | `JPG-001`, `JPG-002`, `JPG-003`, `JPG-004`, `JPG-006`–`JPG-021` |
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

These 270 rows are not 270 equal-sized coding tasks. A row may be a small
documentation or policy decision, a new fixture, a codec algorithm, a WASM
runtime experiment, or a release gate. The reliable “how much is left” numbers
today are the exact row count, the four coverage gaps above, and the explicit
dependency order; an hour estimate would be invented until the next slice is
chosen and measured.

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
