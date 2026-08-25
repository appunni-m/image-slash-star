# Roadmap: human rendering of the one source of truth

Status: `roadmap.json` is the canonical pending-work plan; this file is its
human-readable rendering. Update the JSON first, then synchronize this view.
Current v1 evidence is recorded below.

The machine-readable roadmap is at [`../roadmap.json`](../roadmap.json). It
contains the complete objective, constraints, dependency order, current
counts, every open ID with its finding and next action, AVIF planned-gap
record, acceptance commands, and status-transition rules. `scripts/verify_roadmap.py` reads that JSON first and
rejects drift between it, this rendering, and the generated fixture matrix.

The repository-root `roadmap.json` is the source of truth for what still needs
to be done. The manifest and generated matrix provide machine-backed fixture
status; CI runs `scripts/verify_roadmap.py` to reject drift between those
records, the JSON roadmap, and this human rendering.

Reviewed: 2026-08-25

- Current claim-ledger refresh base revision: `4c8f3257b68812d1d6c8e583dc4cde42a50dcd81`
- Managed Pillow parity run: `84716077-aee7-4396-8328-e6735202b044`
  (1,449/1,449 passed at its recorded revision `36b9396`; the current
  fixture/hash refresh does not silently relabel that historical parity run)
- Project: `image-slash-star`, a Rust image-codec library whose runtime is
  intended to be pure safe Rust on every supported target
- Historical finding evidence only: [the old roadmap](roadmap.md). It does not
  control current status, dependencies, or execution order.

## Historical pre-cutover evidence

The following source-quality checkpoint predates the pure-Rust AVIF cutover. It
is retained as historical evidence, not as a claim about the current working
tree. The checkpoint is commit
`2d3e7ecb32b5413b9683061805ff6fc8909ed82e`. It is separate from the older
claim-ledger base above: the cleanup changes do not silently promote old
feature claims or invent new Pillow rows.

- Managed Coverage MCP run `8d3e09cb-638c-434a-b7cc-a74ea576e667` passed
  108/108 tests and ingested snapshot
  `af56a0c3-5bca-4b7a-8e15-29ac36516edc` at this exact source revision.
- Aggregate native all-feature coverage is 73,473/73,539 lines (99.9103%),
  9,434/9,444 branches (99.8941%), 3,629/3,679 functions (98.6409%), and
  109,876/110,011 regions (99.8773%). The remaining source-level gaps are
  two JPEG encoder branch outcomes and the progressive JPEG decoder's
  defensive short-circuit branches. The function deficit is LLVM's
  compiler-generated SIMD/direct-path specializations: source file coverage
  has no uncovered function-start lines, and `src/codecs/jpeg/kernels.rs` has
  100% branch coverage. This is not claimed as 100% until the metric itself is
  closed or the instrumentation limitation is separately accepted.
- `cargo fmt --all -- --check`, locked all-feature check, strict workspace
  Clippy (`-D warnings`), rustdoc (`-Dwarnings`), the full locked test suite,
  doctests, the documentation audit, claim/provenance/unreachable/package/
  license verifiers, and `cargo deny check` all pass. The only intentional
  Rust `unsafe` boundary was the documented AVIF C bridge; the JPEG
  vectorization path remained safe Rust.
- Final managed Pillow parity run `3a8573dc-0e29-4ecb-8c2a-4ce1ab389a90`
  passed 1,449/1,449 with no skips at the docs-clean commit
  `33f8f85dd7860f95a6bd2b4beafcd2e010e0f0e9` (the source is unchanged from
  the strict-audit checkpoint). The registered feature-matrix wrapper
  `680a7e74-61af-4315-aee7-8a5fa09d0820` failed before test execution because
its immutable command invokes the configured Cargo `sccache`; the same script
  completed all 33 matrix lanes across
  native, `wasm32-unknown-unknown`, and `wasm32-wasip1`, and reported matching
  capability tables.
- The fixed public production comparison was run with five alternating rounds
  across 20 encode and 20 decode cases on the same macOS Arm host, using
  ordinary Cargo release Rust and TurboJPEG's release SIMD. All RGB/gray
  results matched exactly. Arithmetic mean `image-slash-star / TurboJPEG`
  was 1.150x for encode and 0.969x for decode; geometric mean was 1.096x and
  0.938x. CMYK output hashes are intentionally not compared because the
  public Rust/Pillow convention and direct TurboJPEG decode expose opposite
  Adobe sample conventions. Receipt metadata and raw rounds were written by
  `benchmarks/jpeg-production/run_matrix.py`; these are host-specific
  observations, not universal performance claims.

This checkpoint improves engineering hygiene and measurement honesty. It does
not close the 266-item active-finding roadmap below: format capability, metadata,
partial-input, target, assurance, and lifecycle items remain pending until their
own caller need and evidence are complete.

## Current pure-Rust AVIF cutover

The current working tree has removed the AVIF C bridge, `build.rs` AVIF build
path, native link variables, copied public C header, and old unsafe exception.
The `avif` Cargo feature is still opt-in, but it now means the in-tree safe-
Rust parser and bounded still-decoder only. There is no native runtime
fallback, and the same dispatch path is used on native and WASM targets.

The generated matrix is the executable numerical projection of this cutover;
the corresponding status is recorded in `roadmap.json`:

- AVIF decode/inspect/verify: 251 rows total, 244 active, 7 explicit planned
  gaps.
- AVIF encode: 32 rows total, all 32 explicit planned gaps; no encoder is
  wired yet.
- Whole matrix: 1,475 rows total, 1071 active decode rows, 365 active encode
  rows, 7 planned decode rows, and 32 planned encode rows.
- Current local Rust contracts: 34/34 matrix tests and 66/66 feature-gate
  tests pass with all features enabled.

The strict all-target/all-feature Clippy gate passes on the installed nightly
rustc 1.99.0 / Clippy 0.1.99 toolchain with `cargo clippy --workspace --all-targets
--all-features --locked -- -D warnings`; no wrapper change or lint
suppression was used.

Managed Coverage MCP run `ca0f3bf8-b30e-4408-a62a-7d0f0225a0ef` passed the
complete all-feature workload plus doctest at measured code-bearing commit
`4c8f3257` in 156,934 ms. Automatic ingestion retained stale registered-command
lineage; the exact LLVM artifact was explicitly imported with the run's commit
provenance as accepted snapshot
`990062a0-f78c-4d14-a0cb-62ef0b8f3f0f`. It measures 96,498/107,759 lines
(89.5498%), 12,298/13,688 branches (89.8451%), 4,909/5,668 functions
(86.6090%), and 145,086/163,503 regions (88.7360%). The new slice activates
one additional pure-safe-Rust 32x32 8-bit 4:2:0 entropy-mosaic regression
witness beside the existing reconstruction class; Coverage MCP reports no
newly covered lines, branches, or functions and one additional covered region.
It does not close the AVIF planned gaps, transient allocation work, or the
four-metric 100% release gate. The remaining misses are visible in the managed report,
with the largest concentration in the intentionally incomplete AV1
block/entropy surface.

Previous AVIF H4 parity checkpoint: commit `49c8f78ff5ddb3089b91e685245bd0ab3d6332bf`
adds the safe-Rust R16x4 luma and 8x4 chroma paths, including the rectangular
matrix-10 tables, chroma residual-context publication, rectangular DC-sign
contexts, and exact H4 edge ownership. The pinned 16x16 4:2:0 candidate
`/tmp/image-star-r16x4-search-q99-nofilter/1600-2.avif` now decodes to the
exact Pillow RGB bytes: both candidate and reference hash to
`f3fb754117962b22ac3705b4f18996f1cf6deb1a8728106dfabe65296581dda8`.
The strict format, all-feature test, and stable-toolchain Clippy gates pass.

Coverage MCP executed the registered all-feature LLVM command successfully at
this commit in runs `1fc0503b-97f0-4638-a133-ddc387005370` (164,768 ms) and
`c3817fee-921c-4b86-9cc0-e7a206874413` (203,756 ms). Automatic ingestion hit a
30-second DuckDB timeout, so the exact generated LLVM JSON was explicitly
imported with commit provenance as snapshot
`7e6d8a9f-be30-4aea-be84-22ef114ac517`; this was the accepted H4
four-metric Coverage MCP measurement at that revision. The current managed
measurement is recorded above. The managed `coverage_review` change
task is `supported` and requests regression inspection. Current totals are
96,290/107,614 lines (89.4772%), 12,283/13,684 branches (89.7618%),
4,907/5,661 functions (86.6808%), and 144,931/163,425 regions (88.6835%).
This increases the denominator because the slice adds real codec paths, so the
100% gate remains open rather than being relabeled as complete.

The revision-bound hash tuple is refreshed at base revision
`4c8f3257b68812d1d6c8e583dc4cde42a50dcd81`;
`python3 scripts/verify_claim_ledger.py` checks the manifest, generated matrix,
coverage-origin inventory, roadmap, and all auxiliary fixture hashes against
the committed tree. This ledger refresh records current source/evidence
integrity; it does not close the separate 100% LLVM coverage gate or relabel
the historical Pillow parity run.

## Current gate status

The current committed tree passes formatting, locked all-feature check, strict
Clippy, the complete all-feature test suite, strict rustdoc, coverage-origin,
diagnostic-provenance, unreachable-contract, package-surface, license, roadmap,
claim-ledger, and diff checks. The one remaining measured release gate is:

- LLVM coverage: 11,261 lines, 1,390 branches, 759 functions, and 18,417 regions
  remain below the 100% release target.
The next implementation item selected by the JSON dependency order is
`AVF-STILL-001`: broaden the safe AV1 walker beyond the now-proven baseline,
accepted-brand variants, grid fixture, and two-column multitile fixture. The
work item remains partial until broader partition/block states, independent
evidence, and target checks exist.

The 7 decode gaps and their pure-Rust dependencies are recorded exactly in
the ledger below. A planned row is a real input or operation that must become
supported by safe Rust, not permission to call libavif, dav1d, libaom, or a C
shim at runtime. Those libraries remain oracle/provenance material only.

The integration test
`test_avif_planned_gaps_are_explicit_safe_rust_contracts` also walks every
planned decode and encode fixture. It requires a concrete gap reason and no
pixel or encoded-output reference, and checks that the current public result
is the declared typed safe-Rust gap. That test is a guardrail for the roadmap;
it does not count a planned row as completed.

The current implementation slice is a checked `FrameCanvas` in
`src/codecs/avif/av1/raster.rs`. The current still path uses it to place all
three reconstructed planes, and its eleven Rust tests prove subsampling
alignment, checked extents, overlap rejection, no partial mutation on rejected
placement, complete-canvas enforcement, and top-left cropping for coded grid
cells whose visible rectangle is smaller than the coded extent. The new
`place_cells` operation validates a whole grid/tile batch—including overlap
between two new cells—before copying any sample. `place_partition_leaf` also
converts the entropy walker's checked four-by-four-unit coordinates to pixel
origins and crops only the visible frame edge. This closes a reusable, atomic
assembly prerequisite for the baseline and future tile/grid paths. The
bounded prerequisite now also proves that `FrameCanvas::new(5, 3, true,
false)` retains three chroma rows and the odd-width chroma extent, while
`av1/sample_depth.rs` rejects out-of-range 8/10/12-bit samples and performs
explicit high-depth bit truncation. These checks are assembly and conversion
contracts only: the production decoder still rejects the target 12-bit 4:2:2
class until reconstruction, restoration, reference state, and exact color
conversion are implemented. The
promoted `coverage_adst_public_02.avif` is an 8x4 lossy 4:4:4 frame whose coded
8x8 leaf is cropped at the bottom edge; it now has a pinned dav1d 218-operation
trace, exact reconstructed YUV planes, and exact public RGB parity. This is
fixture evidence for the existing padded-leaf path, not a claim for a separate
8x4 transform shape. The promoted `coverage_adst_public_04.avif` is a 16x4
lossy 4:4:4 frame whose two coded 8x8 leaves are both cropped at the bottom
edge; it has a pinned three-block dav1d topology, a 407-operation trace, exact
16x4 Y/U/V planes, and exact public RGB parity. The
seven additional repository-generated asymmetric-gradient witnesses
`coverage_adst_public_03.avif` (4x16, 334 operations),
`coverage_adst_public_05.avif` (8x16, 327),
`coverage_adst_public_06.avif` (16x8, 317),
`coverage_adst_public_07.avif` (8x32, 618),
`coverage_adst_public_08.avif` (32x8, 597),
`coverage_adst_public_09.avif` (16x16, 605), and
`coverage_adst_public_10.avif` (32x16, 1,109) now also have exact Pillow
RGB parity, pinned reconstructed planes, entropy operations, and recorded
bounded partition records. Their descriptions intentionally avoid claiming
unobserved predictor or transform semantics. The
committed grid and two-column multitile fixtures now also have exact public
pixel evidence; broader tile/grid shapes and auxiliary relationships remain
open.

The current lossy leaf also has scalar safe-Rust 8×8 and 4×4 inverse
transforms, exact eight-bit luma and U/V dequantization tables, and the general
4×4 chroma EOB/base/high-token/sign sentence. The checked-in
`baseline_six_terminal_then_stops_at_vertical_8x16_gap` contract proves that the first
four 8×8 terminal payloads in the 128×128 baseline can be consumed in coded
order and materialized as bounded leaf planes, including D135 and Smooth
neighbor contexts, and that the following two top-row 8×8 payloads consume
their mode-zero filter-intra modes, ADST-DCT/DCT-ADST residuals, Smooth
chroma prediction, and chroma sentences, then identifies the next
terminal as an 8×16 coded block at block coordinates `(6, 0)`. The focused
safe-Rust contract now consumes that terminal's rectangular coefficient
sentence, reconstructs its 8×16 luma and skipped 4:2:0 chroma planes, and
checks that the following terminal begins at `(0, 4)` with `(width, height) =
(4, 4)`. This remains bounded syntax/leaf evidence and a regression contract;
the production baseline now carries the complete frame walk through loop
filtering, raster, color conversion, and independent full-frame comparison.
Safe matrix-10 U/V dequantization remains covered for the closed leaf; other
matrix IDs remain an explicit unsupported boundary.

The former baseline sub-gap remains intentionally concrete. `PartitionNode`
geometry uses 4×4 block units, so the terminal at `(6, 0)` with
`(width, height) = (2, 4)` is one 8×16 coded block. It must not be “fixed” by
decoding two 8×8 blocks: AV1 gives the terminal its own rectangular
transform/coefficient sentence, transform-size contexts, 4:2:0 chroma
transform geometry, and above/left edge windows. Safe Rust now proves those
pieces both in the focused contract and in the production baseline walk,
including the exact next-node boundary. The contract is retained as a narrow
regression witness; broader multi-tile shapes remain open `AVF-STILL-001`
work.

The arithmetic prerequisite is now isolated in
`src/codecs/avif/av1/transform.rs`: a safe scalar AV1 16-point inverse DCT pass
and the rectangular 8×16 DCT-ADST wrapper are covered by zero, bounded-input,
vertical-basis, and non-repeated-8×8 tests. This is implementation progress,
not a status promotion. The helpers are now used by the focused R8×16
terminal contract. They are not yet connected to the full-frame walker because
production frame state, coefficient storage across terminals, 4:2:0 chroma
transforms, above/left windows, and independent raw-frame evidence remain
incomplete.

The current prerequisite slice also includes a scalar safe-Rust CDEF block
kernel in `src/codecs/avif/av1/cdef.rs`. It accepts only checked plane/block
geometry, treats out-of-frame neighbors as unavailable padding, and runs in the
baseline path without raw pointers. Its direction-cost ordering, deblocking
level deltas, and edge padding are checked against the independent scalar
decoder; the 128×128 baseline now matches the independent 49,152-byte RGB
reference exactly. The first
`AVF-STILL-001` implementation slice now has a safe-Rust partition walker that
reaches terminal footprints in coded payload order: it decodes the legal
partition symbol domain, selects the four above/left CDF contexts, updates
adaptive CDFs, expands terminal H/V, three-way, four-way, and implicit 4×4
footprints with checked edge clipping, and stops at a hard node bound. Because
AV1 interleaves each terminal block's prediction/residual syntax
between sibling partition symbols, the production path now consumes and proves
the first four baseline terminals in one exact closed 16×16 square: origin,
horizontal sibling, D135 vertical child, and the lower-right Smooth child with
its left neighbor. It then consumes the following two top-row 8×8 blocks with
mode-zero filter-intra prediction, the AV1 ADST-DCT and DCT-ADST transforms,
and Smooth chroma prediction. It then records the next terminal's 8×16
geometry as the next pure-Rust gap. The safe
scalar path carries the needed H-DCT, ADST-DCT, DCT-ADST, and ADST-ADST
transforms, directional/Smooth/filter-intra predictors, and legal chroma EOB
branches. This narrow runtime contract does not activate the multi-tile or
other former-native rows. The current R32x16 origin witness proves the
isolated Horizontal32x16 4:2:0 origin leaf, including its four luma transforms
and two 16×8 chroma sentences. The R16x64 witness also proves safe 16×16
matrix-10 luma dequantization and top-only DC prediction for all four vertical
children when no left neighbor exists; neither witness promotes its broader
family or following-neighbor cases. Broader
partition/block state, every filter-intra mode, and independent evidence for
those rows are still required before they can become active.

Revision rule: the detailed candidate records below preserve the history of
individual slices and their original evidence. A number in one of those
records is not current merely because the record is in this file. For the
current state, use this checkpoint, the current coverage table, and the open-ID
inventory; an older “100%” result is valid only for the exact revision named
beside it.

## Latest API-038 implementation candidate

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

### Latest API-045 implementation candidate — verification cache and retained policy preflight

The next narrow runtime slice is implemented on `main` at
`e26fb5ff534df1981365f1d5d7845a1af5356c4c`:

- **Caller problem:** A server, UI, or WASM page may inspect the same immutable
  upload more than once—for example, verify it, pass an owned or borrowed view
  clone to another component, and verify again before decoding. Re-running the
  same format-specific verification parse wastes work. Pillow cannot express
  this Rust source-lifecycle contract, so this is not a missing Pillow codec
  feature.
- **Implemented behavior:** `EncodedImage` now has a separate
  `OnceLock<ImageResult<()>>`. The first supported `verify()` stores either the
  successful result or the deterministic `ImageError`; later owned calls and
  clones reuse that result. `verify_with_scope` checks that the requested scope
  is provided before reuse, stronger unsupported requests are never hidden by
  a weaker cached result, and still/sequence decode caches are unchanged.
  `EncodedImageView` caches only its immutable verification result per view;
  its pixel decodes remain uncached because it borrows caller bytes and is
  meant for short-lived use. Cloned views share that verification result, so a
  caller can fan out a borrowed view without repeating the same verification
  parse. Bounded native integration contracts call both the owned source and
  borrowed-view clones concurrently from eight workers for a valid PNG and a
  deterministic bad-CRC verification failure; WASI stays sequential because
  its test runtime has no portable thread support.
- **Policy-preflight behavior:** Source-bound still and sequence operations now
  perform the encoded-input, allowed-format, and metadata checks for that
  operation, then reuse the retained `ImageInfo` for dimension, decoded-byte,
  frame-count, and primary sequence-byte checks. They dispatch through helpers
  that do not repeat generic header inspection. Metadata scanning remains
  per-operation, and a policy is still temporary rather than attached to the
  source forever.
- **What this does not do:** It does not retain a parsed header/index,
  decompressor, temporary workspace, or allocator state. It retains the
  borrowed-view verification result but no borrowed decoded pixels.
  It closes repeated verification-result work and repeated generic header
  inspection on source-bound policy paths; parsed codec-state reuse, eviction,
  allocation counts, and retained-cache-byte measurements remain open under
  API-045/RN-003.
- **Local evidence:** Native `feature_gate_tests` passed 50/50, the native
  parity matrix passed 28/28, two focused `wasm32-wasip1` source contracts
  passed 2/2, and the complete WASI feature-test binary compiled. The full
  local LLVM report passes exactly: 65,157/65,157 lines, 8,482/8,482
  branches, 3,328/3,328 functions, and 97,327/97,327 regions. Formatting,
  locked all-feature check, strict Clippy, rustdoc warnings, doctests, and the
  repository claim/provenance/package/license/WebP verifiers also pass.
- **Current runtime observation:** The fixed benchmark protocol on clean
  revision `b10129b919ac4c6bf8f242acacc26ecb8c947e42` passed all 1,421 active
  Pillow-visible rows in `0.975437 s` wall time (`2.783871 s` user,
  `0.189477 s` system, 258,293,760-byte peak RSS). The Rust-only feature-gate
  suite passed in `1.497900 s` wall time (`2.196903 s` user, `0.090615 s`
  system, 166,625,280-byte peak RSS). The prior warm reference at clean
  revision `a93234891f39a26d7a01336b8ceeba46d71fa15a` measured `0.974287 s`
  for parity and `1.519496 s` for the feature-gate suite; the small changes
  are within host/cache/toolchain variation, not proof of a universal
  speedup. The standard workload does not isolate repeated verification,
  allocation counts, retained cache bytes, or WASM runtime cost.
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
  `aeab9d6c3b18c6159a16da9d50c0fc900469ed4a`; the map records inspector SHA-256
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
- A new predictor-absence property now proves four named candidate streams
  independently contain no predictor transform: `mode0`, `quadrants`, and
  `sparse` have no transforms, while `steps` uses color indexing only. This is
  negative structural evidence, not a claim that every legal stream omits or
  uses a predictor.
- The meta-Huffman and entropy-image witnesses now also cover the observed
  3×3 entropy image produced with six prefix-width bits in `mode0_hybrid`; the
  other observed dimensions remain 2×1 and 24×24. Unobserved dimensions and
  grouping patterns remain open.
- Distance witnesses now include the clamp, coordinate, full-width, direct-
  distance, and repeated-back-reference branches (`10→1`, `5→16`, `24→4`,
  `23→256`, `152→32`, and `2→64` in the `lossless_noise` stream). Other
  mappings and malformed distance streams remain open.
- `python3 scripts/verify_webp_vp8l_property_map.py` passes with 16 witnessed
  properties, 98 named witnesses, 79 distinct active WebP rows, 84 structural
  witnesses, 40 malformed-parser witnesses, and all 37 active lossless success
  rows parsed. The property map and claim ledger hashes are updated together.
- The four named no-predictor rows are now independently witnessed. Other
  no-predictor streams, predictor combinations not present in the current
  fixtures, and complete VP8L transform coverage remain open. WEP-022
  therefore remains open; this slice adds only the named negative evidence and
  does not claim complete VP8L transform coverage.
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
| W4 — AVIF and portable targets | Portable AVIF capability, explicit pure-Rust planned gaps, target restrictions, sequence/encode work, and independent output compatibility | Select one bounded portable AVIF target capability or source-property contract | Feature-gated target fixture, native/WASI execution, and independent decoder evidence when bytes are emitted |
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
| W2 | `OutputSink` checkpoint/rollback plus cancellation at the final sink segment; the API-038 decode-format allow-list; PNG zlib-inflation/scanline, GIF LZW code/expansion, JPEG baseline/progressive-MCU, BMP raw payload/scanline, ICO embedded 24/32-bit BMP rows, and TIFF Deflate/PackBits/LZW/predictor/sample-conversion/raw-payload/raw-tile checkpoints; TIFF raw-strip/raw-tile allocation reuse; synchronous progress callbacks | Integrated locally; managed product-parity evidence remains revision-bound | `OutputSink` has caller-visible checkpoint/rollback behavior; the current all-feature `feature_gate_tests` contract passes 66/66, including progress callbacks and the listed codec work-budget boundaries. The allow-list and decoder checkpoint/allocation slices are Rust-only and have no Pillow rows. The 2d3e source-quality snapshot is historical; current local quality evidence is recorded in the current-tree sections above, while product-claim acceptance remains bound to the claim ledger until its parity evidence is refreshed. |
| W3 | Coverage-origin inventory and justified defensive-path evidence | Evidence-only; no new product behavior | The origin verifier passes for 502 exact `cfg(coverage)` guards across 85 files, with no Pillow-parity origin assigned. The current managed snapshot `990062a0-f78c-4d14-a0cb-62ef0b8f3f0f` is bound to measured commit `4c8f3257`; remaining gaps stay visible in the current coverage table. |
| W4 | AVIF `iloc` item-location/source-provenance contract and pure-Rust cutover | Integrated locally; capability gaps remain planned | Item extents and source locations are retained and asserted by the Rust-only feature contract. The runtime no longer depends on `libavif`/`dav1d`/`libaom`; 244 AVIF decode rows are active, 7 decode rows are explicit pure-Rust gaps, and all 32 encode rows remain planned. |
| W5 | Machine-checked unreachable-contract catalog and Cargo package surface | Integrated in the current tree | The ten-category catalog and exact package-path manifest both verify successfully; claim-ledger, diagnostic, license, and package-surface checks remain release evidence rather than Pillow parity. |

The five worker checkouts were disposable execution spaces. Their reviewed
slices are represented by reviewed commits on `main`; no worker pushed
directly. The accepted product-claim tuple remains revision-bound to the
historical Pillow parity record at `36b9396`; the current hash and
coverage-evidence refresh is bound to `4c8f3257` and does not silently rewrite
that parity result.

## Contract catalog: behavior Pillow cannot prove

This is the separate Rust-only list. “Cannot prove” means Pillow may have a
similar idea internally, but it cannot return this crate's exact field, token,
target, sink, or typed result for comparison.

The bounded v1 map is machine-checked by
`python3 scripts/verify_unreachable_contracts.py` against
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
| Fixture rows | 1,475 total | 1,078 decode/inspect/verify rows plus 397 encode rows exist. Current status is 1,071 active decode rows, 365 active encode rows, 7 planned decode rows, and 32 planned encode rows; the planned rows are explicit rather than mislabeled malformed cases. |
| Managed Pillow checks | 1,449/1,449 passed | Managed parity run `84716077-aee7-4396-8328-e6735202b044` is bound to revision `36b9396`. |
| Immediate correction queue | 0 | No newly confirmed defect is waiting ahead of capability work. |
| Current native all-feature ordinary contracts | 34/34 matrix tests and 66/66 feature-gate tests passed | The current local tree is behaviorally green for these Rust integration contracts. |
| Historical source-quality checkpoint | reviewed revision `2d3e7ec` | Strict fmt, locked check, Clippy, rustdoc, full tests, verifiers, managed coverage, and the production JPEG comparison are recorded in the historical checkpoint above. |
| Accepted product-claim baseline | revision `36b9396` | The managed Pillow claim ledger remains bound to this source/evidence revision. |

The current native all-feature feature-gated contract is green, including the
PNG zlib-inflation/scanline and TIFF Deflate/PackBits/LZW/predictor/sample-
conversion/raw-payload/raw-tile boundary tests described
in the RN-003 candidates below.
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

The managed Coverage MCP snapshot below is exact for the measured code-bearing
commit `4c8f3257`, with accepted imported snapshot
`990062a0-f78c-4d14-a0cb-62ef0b8f3f0f`. The registered command execution and
the explicit import provenance are recorded above; the accepted claim ledger
now records the same revision-bound tuple.

| Metric | Covered | Total | Covered % | Gap | Gap % |
| --- | ---: | ---: | ---: | ---: | ---: |
| Lines (managed Coverage MCP) | 96,498 | 107,759 | 89.5498% | 11,261 | 10.4502% |
| Branches (managed Coverage MCP) | 12,298 | 13,688 | 89.8451% | 1,390 | 10.1549% |
| Functions (managed Coverage MCP) | 4,909 | 5,668 | 86.6090% | 759 | 13.3910% |
| Regions (managed Coverage MCP) | 145,086 | 163,503 | 88.7360% | 18,417 | 11.2640% |

The compatible comparison snapshot is
`772591b5-d792-452a-9e35-1d3b7d9a8dd5` at code-bearing commit `ca444cd6`.
Coverage MCP reports deltas of +0/+0 lines, +0/+0 branches, +0/+0 functions,
and +1/+0 regions (covered/total). It reports no newly covered lines,
branches, or functions, 6,313 hit-count-only changes, and no regressions; the
additional region comes from the activated regression witness. The current
managed LLVM JSON report carries the warning that segments are normalized to
segment-start lines; aggregate region coverage is preserved from its report
summary. RN-001 therefore remains open for the current source tree: the
release target is still 100% for all four measures.
The snapshot does not claim complete format support or close the product
roadmap.

The remaining aggregate gaps are deliberately visible. Do not close them by
excluding files, suppressing compiler instrumentation, or adding tests that
users cannot cause. Any future source change must rerun all four coverage
measures at the same revision.

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

### RN-001 — Coverage baseline and honest accounting — OPEN for the current tree

**Why:** We need to know which flashlight beams are missing before choosing
new tests. Otherwise we may add tests that do not reach the code we think they
reach.

**Work/result:** The latest all-feature native Coverage MCP measurement is
bound to code-bearing commit `49c8f78f`; its exact aggregate result and
explicit-import provenance are recorded above. This slice adds the safe-Rust
R16x4/H4 luma and 8x4 chroma implementation and proves one pinned candidate
byte-for-byte against the Pillow reference. It changes source mapping and
coverage denominators; it does not claim that the aggregate 100% gate is done.
Real behavior uses Pillow-visible fixtures or Rust-only feature contracts,
private models remain origin-registered, and the claim ledger remains separate
from this cleanup checkpoint.

**Source IDs:** `QA-003`, `QA-010`, `QA-020`, `QA-030`, `DOC-005`.

**Done:** not yet. The accepted current managed report keeps Pillow, Rust-only,
and private-model origins distinct, but it reports 89.4772% lines, 89.7618%
branches, 86.6808% functions, and 88.6835% regions. Close this item only when
all four current metrics reach 100% or an explicit, reviewed instrumentation
decision changes the release target.

### RN-002 — WebP 16-bit luminance normalization boundary — DONE (selected slice; historical evidence)

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
`84716077-aee7-4396-8328-e6735202b044` (1,449/1,449). The historical
Coverage MCP snapshot `05b6674e-e7d9-43f4-b62b-a63a2ca45cf6` was exact for
that revision's measured lines, branches, functions, and regions. The current
source-quality snapshot and its remaining gaps are recorded at the top of
this file; local matrix, feature-gate, check, strict Clippy, and WebP
structural-map verification also pass.

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
28/28. Current aggregate coverage is the source-quality result in the table
above, not the historical selected-slice result.

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

#### RN-003 status at the current roadmap review

This table is the short, current view of RN-003. The candidate records below
retain the detailed history and exact boundaries; this table prevents a passed
coverage run from being mistaken for completion of every resource-control
category.

| Category | Status now | Evidence already in the tree | Exact remaining work |
| --- | --- | --- | --- |
| Cooperative work checkpoints | Partial / active | The current `feature_gate_tests` contract passes 66/66, including PNG, GIF LZW, JPEG baseline/progressive-MCU, BMP raw payload/scanline, ICO embedded 24/32-bit BMP rows, and selected TIFF Deflate, PackBits, LZW, predictor, sample-conversion, raw-payload, and raw-tile boundaries. The current aggregate LLVM result is the local result above; the 2d3e checkpoint and older candidate-specific exact totals are historical. | Add only independently enforceable long-running codec units; preserve the documented polling cadence and typed inclusive errors. |
| Transient allocation and peak behavior | Partial / unmeasured | TIFF raw strips reuse the final raster allocation at `122aae0`, and raw tiled layouts place visible rows directly into that raster at `96f5e50`; prior WebP allocation-reuse slices are recorded above and in `docs/testing.md` | Measure allocator counts/retained capacity/peak RSS with a repeatable protocol, then optimize one proven bottleneck at a time. No recoverable-OOM promise is allowed yet. |
| Progress callbacks | Implemented / locally verified | `CancellationToken::with_progress` emits monotonic `ProgressEvent` values at accepted cooperative checkpoints; `ProgressDecision::Cancel` maps to the existing typed cancellation result. The contract is synchronous, native/WASM identical, and callback panics are intentionally not caught. | Refresh managed target evidence and keep the callback contract covered as new codec checkpoints are added. |
| Short-write semantics | Current structural contract / partial | `OutputSink::write_all` requires complete acceptance or an error; partial structural writes are tested across available still and sequence writers, and the current checkpointed witness restores an accepted partial segment | Decide whether a future streaming writer needs a byte-counting write API; do not call current structural delivery universal streaming. |
| Rollback | Current checkpointed contract / partial | `OutputSink::checkpoint`/`rollback` restore opted-in sinks on write, cancellation, flush, and genuine multi-frame sequence failure; rollback failure is typed `OutputWrite` and feature-tested at commit `260c8646` | Extend rollback only where a caller can provide a real reversible sink position; append-only sinks intentionally retain their documented prefix. |
| Cleanup and error precedence | Current failure normalization / partially audited | `finish_sink` and `rollback_sink_on_error` now track actual write/flush contact, preserve untouched preflight and pre-cancel errors, suppress flush after failed delivery, and retain rollback-failure precedence | Audit every new progress, allocation, and future short-write path for deterministic cleanup and error precedence; add a regression only when a real branch is found. |

The AVIF sequence path now participates in the shared decoded-byte budget even
while presentation is still planned. `animated.avif` proves that per-frame and
cumulative limits reject before AV1 sequence validation returns its explicit
safe-Rust presentation gap. This is resource-safety evidence, not a claim that
the sequence decoder is complete.

#### Completed RN-003 slice — sink delivery-attempt recovery

**Caller problem:** A caller may give an encoder a reversible destination and
expect a failed write not to leave a partial image. But an error can also happen
before the sink is touched, and replacing that original validation or
cancellation error with a rollback error would make diagnosis worse.

**Pillow answer:** Pillow has no caller-owned `OutputSink`, checkpoint, flush
hook, or rollback result. This is a Rust-only resource contract and adds no
parity row.

**Implemented behavior:** Commit `260c8646ea89eb164bc5116f0d40eb704910dc21`
adds a safe tracking wrapper around all four public still/sequence sink roots.
It marks before every `write_all` and `flush`, so partial writes and flush
failures can restore a real checkpoint. Preflight and pre-cancel failures do
not invoke rollback. Successful rollback preserves the original typed error and
the exact pre-call prefix; failed rollback returns `OutputWrite` with both
failure causes and the requested still/sequence stage. Append-only sinks that
return `None` retain their documented prefix behavior.

**Evidence:** The feature-gated contract covers partial structural writes,
preflight, pre-cancel, final-segment cancellation, flush failure, still and
genuine multi-frame GIF sequence recovery, exact prefix restoration, and
rollback-failure precedence. Full locked tests, formatting, and strict Clippy
pass. Managed run `fd89bf16-bd58-4c30-afd5-c2dfb58acda9` passed in 163,099 ms;
the lineage-valid explicit LLVM snapshot is
`b6b8e5f8-30be-410b-a5f5-a549c101a8e2` at the same commit. Current aggregate
coverage is 96,330/107,660 lines, 12,285/13,686 branches, 4,915/5,671
functions, and 144,977/163,481 regions. The 100% release gate and RN-003
remain open.

**Remaining boundary:** This closes the recovery semantics for the current
structural sink layer only. It does not provide universal streaming,
transient-allocation accounting, remote transactional I/O, or recoverable-OOM
behavior. Future sink APIs and codec interiors still require their own
requirement packet, tests, and same-revision coverage.

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

#### Resolved slice — API-038 allowed decode formats

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

This row is resolved from the repository contract. The global managed coverage
and claim-ledger gates remain separate release gates; they do not change the
allow-list behavior or its Rust-only evidence.

**Source IDs:** `API-014`, `API-017`, `API-018`, `API-023`, `API-030`,
`API-036`, `API-041`, `API-043`, `API-044`, `API-045`, `API-046`, `QA-016`,
`QA-020`, `QA-026`, `QA-030`, plus the remaining resource rows in the codec
groups below.

**Done when:** each resource or sink boundary has a typed result, an
inclusive boundary fixture, a no-partial-output assertion where applicable,
and a feature-gated origin when Pillow cannot observe it.

#### Current candidate slice — API-036 PNG zlib inflation and scanline checkpoints

**Caller problem:** A large PNG can spend meaningful time reconstructing
filtered rows, unpacking samples, or expanding zlib output after the container
has been recognized. A caller-controlled token or work budget that polls only
at chunk boundaries cannot stop that interior work promptly enough for a
server, UI, or WASM page.

**Pillow answer:** Pillow can decode the same pixels, but it does not expose a
caller token, a checkpoint counter, or a typed `DecodeWorkUnits` result. This
is a Rust-only resource-control contract and must not become a fabricated
parity row.

**Implemented behavior:** Commit
`4320977aa62768009df74d2f3a0e3b4f4a218bdb` adds a token-aware PNG zlib-prefix
decoder; arithmetic fix `e97b514` keeps its checkpoint counters within the
repository's checked-arithmetic policy; and coverage-origin follow-up
`a7207ed` models the defensive error edges. The token-aware path polls before
each Deflate block, after dynamic
table construction, while stored blocks copy output in 1,024-byte intervals,
while fixed/dynamic blocks emit literals and back-references in 1,024-byte
intervals, and during Adler-32 in 5,552-byte chunks. It still stops at the
declared raster prefix, so the existing oversized-scanline diagnostic and
trailing-data policy remain unchanged. A missing token stays on the direct
no-token decode loops. The earlier commit
`7d9e256df33296be832869fb41670a8d1e07fbb6` threads the same token through PNG
still and APNG frame scanline reconstruction: it charges before every
filtered-row reconstruction and every sample-unpack row, including each Adam7
pass. A fired token or exhausted finite work budget returns before the next
row can publish into the decoded result; ordinary bytes and pixels remain
unchanged.

**Source/evidence:** The Rust-only
`png_decode_work_budget_covers_inflation_and_scanline_rows` contract uses the
committed 128×128 `no_interlace.png` normal-compressed witness and finds its
inclusive boundary at 327; the exact boundary succeeds and 326 rejects with
the typed work-budget result. The committed 128×128 `compress_none.png`
stored-block witness finds a second boundary at 322, with exact success and
321 rejection, proving that polling only after row reconstruction is
insufficient. The same contract keeps the decoded pixels byte-identical at
both exact boundaries. The coverage-only origin uses the small committed
`2x3.png` and `adam7_2x3.png` witnesses to reach both scanline layouts, plus
the existing Deflate private-branch model for malformed streams and
cancellation edges. Native `feature_gate_tests` passes 51/51, the native
matrix passes 28/28, and the exact local LLVM report passes 65,342/65,342
lines, 8,514/8,514 branches, 3,335/3,335 functions, and 97,627/97,627
regions.

**Remaining dependency:** The managed Coverage MCP transport still closes at
`project_context`, so no same-revision managed snapshot is available. The
accepted claim-ledger tuple remains unchanged until managed evidence is
recovered. The production cadence still permits up to one 1,024-byte output
interval between Deflate polls, and does not claim interruption inside every
bit read or Huffman-table operation. Transient allocation/peak accounting,
progress callbacks, short-write semantics, rollback, and cleanup remain open
RN-003 work.

#### Current candidate slice — API-036 TIFF Deflate inflation checkpoints

**Caller problem:** TIFF already passes a cancellation token around strips and
tiles, but a Deflate-compressed strip or tile can still spend most of its time
inside zlib inflation. Polling only before and after the strip means a server,
UI, or WASM page cannot stop that interior work promptly.

**Pillow answer:** Pillow can prove that the TIFF pixels are correct, but it
does not expose this crate's caller token, checkpoint counter, or typed
`DecodeWorkUnits` result. This is a Rust-only resource-control contract and
must not become a fabricated parity row.

**Implemented behavior:** Commit
`a8e4d3b61b6f8a79a1bad8196fd9dec7486b1af8` passes the existing token from
TIFF strip/tile dispatch into `decode_block`. Both TIFF Deflate compression
tags use the token-aware zlib
prefix inflater when a token is present; the missing-token path keeps the
direct no-token inflater. The shared inflater polls at zlib block and
dynamic-table boundaries, 1,024-byte stored/fixed/dynamic output intervals,
and 5,552-byte Adler-32 chunks. TIFF's ordinary decoded bytes and pixels are
unchanged.

**Source/evidence:** The Rust-only
`tiff_decode_work_budget_covers_deflate_inflation` contract uses the committed
128×128 `tests/fixtures/input/images/tiff/deflate.tiff` witness. Its exact
inclusive boundary is 71: 71 succeeds with byte-identical pixels and 70
rejects with the typed `DecodeWorkUnits` result (`observed = 71`). Native
`feature_gate_tests` passes 52/52, the native matrix passes 28/28, and the
exact local LLVM report passes 65,351/65,351 lines, 8,514/8,514 branches,
3,335/3,335 functions, and 97,638/97,638 regions.

**Remaining dependency:** The managed Coverage MCP transport still closes at
`project_context`, so no same-revision managed snapshot is available and the
accepted claim-ledger tuple remains unchanged. This slice covers TIFF Deflate
inflation only; transient allocation/peak accounting, progress callbacks,
short-write semantics, rollback, and cleanup remain open RN-003 work. The
production cadence still permits up to one 1,024-byte output interval
between Deflate polls and does not claim interruption inside every bit read or
Huffman-table operation.

#### Current candidate slice — API-036 TIFF PackBits packet checkpoints

**Caller problem:** TIFF PackBits can expand a long strip or tile through many
literal and repeat packets after the outer TIFF loop has already started. A
caller token that is checked only between strips or tiles cannot interrupt that
packet stream promptly.

**Pillow answer:** Pillow can prove the finished TIFF pixels, but it cannot
return this crate's caller-token state, packet checkpoint count, or typed
`DecodeWorkUnits` result. This is a Rust-only resource-control contract and
must not become a parity row.

**Implemented behavior:** Commit
`83e924ca9bed6ceb27024a50014dcc7904836812` adds a token-aware PackBits
decoder selected only when TIFF receives a caller token. It polls before every
literal, repeat, or no-op packet; the existing no-token decoder remains the
direct fast path. A packet can still expand up to the PackBits format's
bounded 128-byte packet size between polls, which is an explicit cadence
boundary rather than a claim of interruption inside a packet.

**Source/evidence:** The Rust-only
`tiff_decode_work_budget_covers_packbits_packets` contract uses the committed
128×128 `tests/fixtures/input/images/tiff/packbits.tiff` witness. Its exact
inclusive boundary is 895: 895 succeeds with byte-identical pixels and 894
rejects with the typed `DecodeWorkUnits` result (`observed = 895`). The
coverage-origin follow-up `c124d0ac89533030cbd9ceb17d7f974c9de392b1` models
the token-aware no-op, truncated, and short-stream defensive edges. Native
`feature_gate_tests` passes 53/53, the native matrix passes 28/28, and the
exact local LLVM report passes 65,402/65,402 lines, 8,520/8,520 branches,
3,336/3,336 functions, and 97,740/97,740 regions.

**Remaining dependency:** The managed Coverage MCP transport still closes at
`project_context`, so no same-revision managed snapshot is available and the
accepted claim-ledger tuple remains unchanged. This slice covers PackBits
packet expansion only; transient allocation/peak accounting, progress callbacks,
short-write semantics, rollback, and cleanup remain open RN-003 work.

#### Current candidate slice — API-036 TIFF LZW code/expansion checkpoints

**Caller problem:** TIFF already checks a caller token around strips and tiles,
but LZW-compressed payloads can spend most of their time reading compressed
codes and expanding dictionary phrases after the outer TIFF loop has started.
Without interior checkpoints, a server, UI, or WASM page cannot stop that work
promptly.

**Pillow answer:** Pillow can prove that the finished TIFF pixels are correct,
but it does not expose this crate's caller token, checkpoint counter, or typed
`DecodeWorkUnits` result. This is a Rust-only resource-control contract and
must not become a fabricated parity row.

**Implemented behavior:** Commit
`845ef5f4af68afa6d76caec799a238b98b36cc7f` selects a token-aware TIFF LZW
decoder only when a caller token is present; the no-token decoder remains the
direct path. The token-aware path polls before every LZW code read and every
1,024 bytes emitted while expanding a dictionary phrase. Clear/end handling,
dictionary growth, code-width changes, and dictionary saturation retain the
existing decoder behavior. The cadence is explicit: a long phrase can emit
up to 1,024 bytes between expansion polls, and polling is not claimed inside a
single bit read or dictionary-link traversal.

**Source/evidence:** The Rust-only
`tiff_decode_work_budget_covers_lzw_codes_and_expansion` contract uses the
committed 128×128 `tests/fixtures/input/images/tiff/lzw.tiff` witness. Its
exact inclusive boundary is 44,166: 44,166 succeeds with byte-identical
pixels and 44,165 rejects with the typed `DecodeWorkUnits` result (`observed =
44,166`). Native `feature_gate_tests` passes 54/54, the native matrix passes
28/28, and the exact local LLVM report passes 65,597/65,597 lines,
8,558/8,558 branches, 3,339/3,339 functions, and 98,027/98,027 regions.

**Remaining dependency:** The managed Coverage MCP transport still closes at
`project_context`, so no same-revision managed snapshot is available and the
accepted claim-ledger tuple remains unchanged. This slice covers TIFF LZW
code/expansion work only; transient allocation/peak accounting, progress
callbacks, short-write semantics, rollback, and cleanup remain open RN-003
work.

#### Current candidate slice — API-036 TIFF horizontal-predictor checkpoints

**Caller problem:** TIFF horizontal prediction reconstructs each sample from
the sample to its left after decompression. A large strip or tile can therefore
spend substantial time in predictor arithmetic after the compression
checkpoint has already returned.

**Pillow answer:** Pillow can prove the final TIFF pixels, but it does not
expose this crate's caller token, checkpoint counter, or typed
`DecodeWorkUnits` result. This is a Rust-only resource-control contract and
must not become a fabricated parity row.

**Implemented behavior:** Commit
`6682a00649b064bb8b328c721498844f9bc2785f` selects a token-aware predictor
path only when a caller token is present; the no-token predictor remains
direct. TIFF polls before each predictor row and after each 1,024 reconstructed
bytes for 8-, 16-, and 32-bit samples, covering both strip and tile dispatch.
The cadence does not claim interruption inside an individual sample
arithmetic operation.

**Source/evidence:** The Rust-only
`tiff_decode_work_budget_covers_predictor_rows` contract uses the committed
128×128 `tests/fixtures/input/images/tiff/rgb_deflate_predictor.tiff`
witness. Its exact inclusive boundary is 194: 194 succeeds with
byte-identical pixels and 193 rejects with the typed `DecodeWorkUnits` result
(`observed = 194`). Native `feature_gate_tests` passes 55/55, the native
matrix passes 28/28, and the exact local LLVM report passes 65,757/65,757
lines, 8,568/8,568 branches, 3,342/3,342 functions, and 98,264/98,264
regions.

**Remaining dependency:** The managed Coverage MCP transport still closes at
`project_context`, so no same-revision managed snapshot is available and the
accepted claim-ledger tuple remains unchanged. This slice covers horizontal
predictor reconstruction only; transient allocation/peak accounting, progress
callbacks, short-write semantics, rollback, and cleanup remain open RN-003
work.

#### Current candidate slice — API-036 TIFF sample-conversion checkpoints

**Caller problem:** TIFF may decode the compressed bytes successfully and then
still spend substantial time unpacking bit-depth samples, converting endian
order, inverting grayscale, scaling packed grayscale values, or compacting
YCbCr samples. A caller token checked only around the strip cannot stop this
second half of raster work.

**Pillow answer:** Pillow can prove the finished pixels, but it does not expose
this crate's caller token, checkpoint counter, or typed `DecodeWorkUnits`
result. This is a Rust-only resource-control contract and must not become a
fabricated parity row.

**Implemented behavior:** Commit
`80d5871dd982eeab7b205b138917f44724c7c490` adds a token-aware sample-conversion
path while keeping the no-token conversion path direct. It checkpoints
inversion, packed-index unpacking and grayscale scaling, 16-bit endian
conversion, palette index unpacking, and YCbCr compaction at row boundaries
and 1,024-byte or sample intervals; direct RGB/RGBA/CMYK and already-native
sample layouts retain their existing fast path.

**Source/evidence:** The Rust-only
`tiff_decode_work_budget_covers_sample_conversion` contract uses the committed
`tests/fixtures/input/images/tiff/16bit.tiff` witness. At this
sample-conversion revision its exact inclusive boundary is 164: 164 succeeds
with byte-identical pixels and 163 rejects with the typed `DecodeWorkUnits`
result (`observed = 164`). The revision's `feature_gate_tests` passed 56/56,
the native matrix passed 28/28, and the exact local LLVM report passed
66,280/66,280 lines, 8,598/8,598 branches, 3,345/3,345 functions, and
98,992/98,992 regions.

**Remaining dependency:** The managed Coverage MCP transport still closes at
`project_context`, so no same-revision managed snapshot is available and the
accepted claim-ledger tuple remains unchanged. The later raw-payload slice
below adds 32 copy checkpoints to this witness in the current tree, so the
same sample-conversion test now has a combined boundary of 196. Transient
allocation/peak accounting, progress callbacks, short-write semantics,
rollback, and cleanup remain open RN-003 work.

#### Current candidate slice — API-036 TIFF raw-payload copy checkpoints

**Caller problem:** An uncompressed TIFF strip or tile currently becomes the
decoded payload through one `encoded.to_vec()` copy. For a large raw payload,
a caller token checked only around the strip or tile cannot stop that copy
promptly.

**Pillow answer:** Pillow can prove that the finished TIFF pixels are correct,
but it does not expose this crate's caller token, checkpoint counter, or typed
`DecodeWorkUnits` result. This is a Rust-only resource-control contract and
must not become a fabricated parity row.

**Implemented behavior at that revision:** Commit
`775c09a1201223b53f49c3d2176b17ce775f4f83` selected a token-aware
raw-payload copy for TIFF `COMPRESSION_NONE` when a caller token was present.
It polled before each 1,024-byte copy chunk; the missing-token path kept the
direct `to_vec()` fast path. Later allocation-reuse candidates below replace
that temporary copy for raw strips and tiles while retaining the same
checkpoint contract. Raw decoded bytes and pixels remain unchanged. This
slice covers payload traversal only; allocation size and peak accounting are
separate work.

**Source/evidence:** The Rust-only
`tiff_decode_work_budget_covers_raw_payload_copy` contract uses the committed
128×128 RGB `tests/fixtures/input/images/tiff/uncompressed.tiff` witness. Its
exact inclusive boundary is 52: 52 succeeds with byte-identical pixels and
51 rejects with the typed `DecodeWorkUnits` result (`observed = 52`). Native
`feature_gate_tests` passes 57/57, the native matrix passes 28/28, and the
exact local LLVM report passes 66,300/66,300 lines, 8,598/8,598 branches,
3,346/3,346 functions, and 99,035/99,035 regions.

**Remaining dependency:** The managed Coverage MCP transport still closes at
`project_context`, so no same-revision managed snapshot is available and the
accepted claim-ledger tuple remains unchanged. This slice covers raw-payload
traversal only; the separate allocation-reuse slice below removes one
transient copy from the common strip path. Broader allocation/peak accounting,
progress callbacks, short-write semantics, rollback, and cleanup remain open
RN-003 work. The copy path polls before each 1,024-byte chunk and does not
claim interruption inside a single slice copy or allocator operation.

#### Current candidate slice — API-045/RN-003 TIFF raw-strip allocation reuse

**Caller problem:** The token-aware raw-payload slice still decoded each
uncompressed strip into a temporary `Vec`, then copied that strip into the
already pre-sized final raster. A large strip therefore held two copies of the
same raw pixels and did twice the memory traffic needed for the ordinary
no-predictor path.

**Pillow answer:** Pillow can prove the final TIFF pixels and errors, but it
does not expose allocator ownership, transient capacity, or copy counts. This
is an internal Rust runtime contract with Pillow-visible bytes as its
regression guard; it must not become a fabricated parity row.

**Implemented behavior at that strip-only revision:** Commit
`122aae07cbe81ba74aa40343261e461012ca1195` appended uncompressed TIFF strip
bytes directly into the pre-sized final raster. The token-aware path reused
the same 1,024-byte copy checkpoints, while compressed strips, predictor
reconstruction, and tiled layouts retained their scratch buffers because they
still needed decompression, transformation, or tile placement. No-token raw
strip decoding kept a bulk append fast path. The current raw-tile candidate
below removes the analogous scratch copy for uncompressed tiles.

**Source/evidence:** The existing Rust-only
`tiff_decode_work_budget_covers_raw_payload_copy` contract retains exact
pixel identity and the inclusive boundary of 52 (51 rejects with
`observed = 52`), and the all-feature TIFF matrix continues to cover ordinary
raw strips. Native `feature_gate_tests` passes 57/57, the native matrix passes
28/28, and the exact local LLVM report passes 66,318/66,318 lines,
8,602/8,602 branches, 3,348/3,348 functions, and 99,059/99,059 regions.

**Remaining dependency:** This slice removes one transient raw-strip copy; it
does not measure allocator counts or peak RSS and does not optimize every
codec path. Broader allocation/peak accounting, progress callbacks,
short-write semantics, rollback, and cleanup remain open RN-003 work. Managed
Coverage MCP still closes at `project_context`, so the accepted claim-ledger
tuple remains unchanged.

#### Current candidate slice — API-045/RN-003 TIFF raw-tile allocation reuse

**Caller problem:** The raw-payload checkpoint slice still decoded an
uncompressed tile into a temporary `Vec`, then copied each visible tile row
into the final image raster. Tiled TIFFs therefore paid for a scratch raster
even when no decompression or predictor transform was needed.

**Pillow answer:** Pillow can prove the finished tiled pixels and errors, but
it does not expose allocator ownership, transient capacity, copy counts, or
the caller's work-budget result. This is a Rust runtime contract with
Pillow-visible pixels as its regression guard; it must not become a fabricated
parity row.

**Implemented behavior:** Commit
`96f5e50e7cdce797e490a9b53d67e92aa05c7dc9` places uncompressed TIFF tile
rows directly into the pre-sized final raster, including edge tiles whose
visible width or height is smaller than the stored tile geometry. The
token-aware path polls before each 1,024-byte chunk across the complete raw
tile payload, including padding that is not copied to the visible raster. The
no-token path keeps a direct row copy. Compressed tiles still retain scratch
buffers because decompression, predictor reconstruction, or tile placement
requires a separate transformed payload.

**Source/evidence:** The Rust-only
`tiff_decode_work_budget_covers_raw_tile_copy` contract uses the committed
128×128 RGB `tests/fixtures/input/images/tiff/tiled.tiff` witness. Its exact
inclusive boundary is 67: 67 succeeds with byte-identical pixels and 66
rejects with the typed `DecodeWorkUnits` result (`observed = 67`). Native
`feature_gate_tests` passes 58/58, the native matrix passes 28/28, and the
exact local LLVM report passes 66,361/66,361 lines, 8,608/8,608 branches,
3,349/3,349 functions, and 99,138/99,138 regions. Locked all-feature check,
strict Clippy, rustdoc warnings, and the `wasm32-wasip1` TIFF feature-test
binary compile also pass.

**Remaining dependency:** This removes the raw-tile scratch copy but does not
measure allocator counts, retained capacity, or peak RSS and does not promise
recoverable OOM behavior. Progress callbacks, deeper codec work checkpoints,
short-write semantics, rollback, cleanup/error precedence, and managed
same-revision evidence remain open RN-003 work. The local proof is current;
the managed Coverage MCP transport still closes at `project_context`, so the
accepted claim-ledger tuple remains unchanged.

#### Current candidate slice — API-036 GIF LZW code/expansion checkpoints

**Caller problem:** Once a GIF image descriptor starts, the decoder can spend
most of its time reading compressed LZW codes, following a long dictionary
phrase, and copying that phrase into the output. A caller-controlled budget
that only polls at the outer GIF block boundary cannot stop a large or
adversarial image promptly enough for a responsive UI, server, or WASM page.

**Pillow answer:** Pillow can prove the final GIF pixels and ordinary errors,
but it does not expose this crate's `CancellationToken`, checkpoint counter,
inclusive `DecodeWorkUnits` error, or no-partial-state guarantee. This is a
Rust-only caller-control slice; it adds no Pillow parity row.

**Implemented behavior:** Commit
`1dac4bb4fc614d9fbba9e6f92e47f038e8a1fa90` passes the caller token through the
real GIF still/sequence decode path. With a token, GIF LZW now polls before
each compressed-code read, while traversing every 1,024 dictionary links, and
while materializing every 1,024 phrase bytes. The ordinary no-token decoder
keeps its direct path, so legacy byte production does not pay for an optional
token check. Malformed code, truncated-bitstream, and clipped KwKwK paths
retain their existing typed classification.

**Source/evidence:** The Rust-only
`gif_decode_work_budget_covers_lzw_codes_and_expansion` contract uses the
committed `tests/fixtures/input/images/gif/lzw_dictionary_saturation.gif`
witness. Its inclusive work boundary is 4,105: exact success preserves every
pixel and maximum 4,104 rejects with `observed = 4,105`. The same contract
checks the committed malformed GIF fixtures
`min_code_one.gif`, `lzw_end_only.gif`, `lzw_invalid_first.gif`,
`lzw_invalid_future.gif`, and `lzw_kwkwk_clipped.gif`. A deterministic public
API round trip of a 4,096×256 all-zero image supplies a real long dictionary
phrase and finds the expansion boundary at 2,298; this is a runtime-generated
model for the expansion stress, not a claim of a new fixture row. Native
`feature_gate_tests` passes 59/59, the native matrix passes 28/28, the GIF
`wasm32-wasip1` feature-test binary compiles, and exact local LLVM evidence is
66,502/66,502 lines, 8,644/8,644 branches, 3,351/3,351 functions, and
99,335/99,335 regions. Locked all-feature check, strict Clippy, rustdoc
warnings, doctests, and all repository claim/provenance/package/license/WebP
verifiers pass.

**Evidence boundary:** Managed Coverage MCP still closes at
`project_context` (`Transport closed`), so the accepted claim-ledger tuple
and managed snapshot remain unchanged. The local proof is current but is not
managed acceptance. The existing `cfg(coverage)` defensive-model hook covers
the private zero-length, truncated-bitstream, and cancellation branches; the
coverage-origin inventory already classifies `src/codecs/gif/decode.rs` as
`defensive_model`. No Pillow parity row or diagnostic origin is claimed for
this Rust-only behavior.

**Remaining dependency:** This is one independently enforceable GIF decoder
checkpoint slice, not completion of RN-003. Other codec interiors, progress
callbacks, allocator/peak measurement, short-write semantics, rollback,
cleanup/error precedence, and managed same-revision evidence remain open. The
complete inventory therefore remains 266 active finding rows; this slice
narrows the API-036/RN-003 work without closing the whole category.

#### Current candidate slice — API-036 JPEG baseline-MCU decode checkpoint

**Caller problem:** A no-restart baseline JPEG can place thousands of MCUs in
one entropy segment. A token that polls only at the segment boundary cannot
interrupt the decoder during that long entropy/IDCT loop, so a large upload can
still monopolize a UI, server, or WASM page between public checkpoints.

**Pillow answer:** Pillow can prove the final JPEG pixels and ordinary errors,
but it does not expose this crate's caller token, checkpoint counter, inclusive
`DecodeWorkUnits` result, or no-partial-state behavior. This is Rust-only
caller-control evidence and adds no Pillow parity row.

**Implemented behavior:** Commit
`0c41c4744790848f169b5e10eadce51ecf1349f` adds a token-aware checkpoint after
each completed 1,024-MCU batch in baseline JPEG reconstruction. The ordinary
no-token path keeps its existing per-segment behavior and does not enter the
checkpoint branch. Incomplete entropy data still follows the existing JPEG
classification before the batch checkpoint is charged.

**Source/evidence:** The Rust-only
`jpeg_decode_work_budget_covers_baseline_mcu_checkpoint` contract constructs
two deterministic images through the public API, encodes them with the normal
JPEG options, and decodes them through both the direct and policy-aware paths.
The 64×64 control image admits at work boundary 5; the 512×512 image has
exactly 1,024 default 4:2:0 MCUs and admits at boundary 6. That one-unit
difference is the interior checkpoint witness: the larger decode is
byte-identical at its exact boundary and maximum 5 returns the typed
`DecodeWorkUnits` error with `observed = 6`. Native `feature_gate_tests`
passes 60/60, the native parity matrix passes 28/28, the JPEG
`wasm32-wasip1` feature-test binary compiles, and exact local LLVM evidence is
66,506/66,506 lines, 8,648/8,648 branches, 3,351/3,351 functions, and
99,344/99,344 regions. Locked all-feature check, strict Clippy, rustdoc
warnings, doctests, and the repository claim/provenance/package/license/WebP
verifiers pass.

**Evidence boundary:** Managed Coverage MCP remains unavailable at
`project_context` (`Transport closed`), so the accepted claim-ledger tuple and
managed snapshot remain unchanged. This local candidate is not managed
acceptance and claims no Pillow row or diagnostic origin.

**Remaining dependency:** This slice covers baseline JPEG entropy work only;
other codec interiors, progress callbacks,
allocator/peak measurement, short-write semantics, rollback, cleanup/error
precedence, and managed same-revision evidence remain open. RN-003 remains
partial and the complete inventory remains 266 active finding rows.

#### Current candidate slice — API-036 JPEG progressive-MCU decode checkpoint

**Caller problem:** A progressive JPEG is made of several entropy scans. A
token that polls only when a scan starts or ends cannot interrupt a large scan
while it is decoding its inner MCU loop, so a large upload can still monopolize
a UI, server, or WASM page between public checkpoints.

**Pillow answer:** Pillow can prove the final JPEG pixels and ordinary errors,
but it does not expose this crate's caller token, checkpoint counter, inclusive
`DecodeWorkUnits` result, or no-partial-state behavior. This is Rust-only
caller-control evidence and adds no Pillow parity row.

**Implemented behavior:** Commit
`a47515f011dc269a0ffc5e537a1a7651afbc0493` adds a token-aware checkpoint after
each completed 1,024-MCU batch within each progressive entropy scan. The
ordinary no-token path keeps its existing scan-level behavior and does not
enter the checkpoint branch. Incomplete entropy data still follows the
existing progressive JPEG classification before the batch checkpoint is
charged.

**Source/evidence:** The Rust-only
`jpeg_decode_work_budget_covers_progressive_mcu_checkpoint` contract constructs
64×64 and 512×512 deterministic progressive JPEGs through the public API,
decodes them through both the direct and policy-aware paths, and discovers the
inclusive boundaries. The 64×64 control admits at work boundary 14; the
512×512 image admits at boundary 36, exactly 22 additional 1,024-MCU scan
checkpoints. The larger image is byte-identical at its exact boundary and
maximum 35 returns the typed `DecodeWorkUnits` error with `observed = 36`.
Native `feature_gate_tests` passes 61/61, the native parity matrix passes
28/28, the JPEG `wasm32-wasip1` feature-test binary compiles, and exact local
LLVM evidence is 66,510/66,510 lines, 8,652/8,652 branches, 3,351/3,351
functions, and 99,353/99,353 regions. Locked all-feature check, strict
Clippy, rustdoc warnings, doctests, and the repository claim/provenance/
package/license/WebP verifiers pass.

**Evidence boundary:** Managed Coverage MCP remains unavailable at
`project_context` (`Transport closed`), so the accepted claim-ledger tuple and
managed snapshot remain unchanged. This local candidate is not managed
acceptance and claims no Pillow row or diagnostic origin.

**Remaining dependency:** This slice covers progressive JPEG entropy work;
other codec interiors, progress callbacks, allocator/peak measurement,
short-write semantics, rollback, cleanup/error precedence, and managed
same-revision evidence remain open. RN-003 remains partial and the complete
inventory remains 266 active finding rows.

#### Current candidate slice — API-036 BMP raw payload and scanline checkpoints

**Caller problem:** An uncompressed BMP can put a large padded pixel payload
behind one bulk read and then spend another long interval converting packed or
packed-palette rows. A server, UI, or WASM page that supplies a cancellation
token or a decode work budget needs checkpoints inside both intervals; polling
only at codec entry cannot stop that work promptly.

**Pillow answer:** Pillow can prove the final BMP pixels and ordinary errors,
but it does not expose this crate's caller token, checkpoint counter, inclusive
`DecodeWorkUnits` result, or no-partial-state behavior. This is Rust-only
caller-control evidence and adds no Pillow parity row.

**Implemented behavior:** Commit `9c8faa7` keeps the ordinary no-token BMP
path on its direct bulk raw-payload read and row conversion. When a caller
token is present, uncompressed raw pixels are copied in 1,024-byte chunks and
each conversion row is checked before it begins. The row checkpoints cover
all six raw depths (1, 4, 8, 16, 24, and 32 bits), including canonical and
non-canonical 1-bit palettes. Truncated token-aware payloads retain the
complete-slice `Malformed` classification with `bmp_pixels` context rather
than turning a terminal decode into an implicit retry.

**Source/evidence:** The Rust-only
`bmp_decode_work_budget_covers_raw_scanline_checkpoints` contract generates
equal-height 64×64 and 128×64 RGB BMP controls through the public API. Their
inclusive work boundaries are 78 and 90 respectively; the twelve-unit delta
is exactly the twelve additional 1,024-byte raw-payload chunks, while row
checkpoint cadence stays constant. The same contract proves exact-boundary
pixel identity, one-unit-below typed rejection, truncated-payload error
context, and row-boundary rejection on committed 1-bit, 4-bit, 8-bit, 16-bit,
24-bit, 32-bit, and non-canonical 1-bit fixtures. Native `feature_gate_tests`
passes 62/62, the native parity matrix passes 28/28, and exact local LLVM
passes 66,531/66,531 lines, 8,654/8,654 branches, 3,354/3,354 functions, and
99,398/99,398 regions.

**Evidence boundary:** Managed Coverage MCP remains unavailable at
`project_context` (`Transport closed`), so the accepted claim-ledger tuple
and managed snapshot remain unchanged. This local candidate is not managed
acceptance and claims no Pillow row or diagnostic origin.

**Remaining dependency:** This slice covers uncompressed BMP payload-copy and
row-conversion work only; compressed/RLE BMP interiors, other codec interiors,
progress callbacks, allocator/peak measurement, short-write semantics,
rollback, cleanup/error precedence, and managed same-revision evidence remain
open. RN-003 remains partial and the complete inventory remains 266 active
finding rows.

#### Current candidate slice — API-036 ICO embedded BMP row checkpoints

**Caller problem:** ICO can carry an uncompressed BMP/DIB inside its directory
entry. The container and entry-selection code already polled a cancellation
token, but the embedded 24-bit BGR-to-RGBA conversion then walked every row
without another checkpoint. A large icon could therefore ignore a caller's
token or work budget during the most expensive part of the decode.

**Pillow answer:** Pillow can prove the final ICO pixels and ordinary errors,
but it does not expose this crate's caller token, checkpoint counter, inclusive
`DecodeWorkUnits` result, or no-partial-state behavior. This is Rust-only
caller-control evidence and adds no Pillow parity row.

**Implemented behavior:** Commit `edf6148` threads the token through the
embedded BMP dispatch and polls before every output row in the 24-bit
BGR-plus-alpha-mask conversion path. At that checkpoint, PNG-backed entries
and the other embedded BMP depth implementations retained their existing
behavior; the ordinary no-token 24-bit conversion still produced the same
pixels and bytes.

**Source/evidence:** The Rust-only
`ico_decode_work_budget_covers_embedded_bmp_rows` contract generates 64×64 and
64×128 RGB images through the public ICO BMP encoder, decodes them through the
public policy API, and proves inclusive boundaries of 68 and 132. The 64-unit
delta is exactly one additional checkpoint for each of the 64 added embedded
BMP rows. It also proves exact-boundary pixel identity and one-unit-below
typed `DecodeWorkUnits` rejection. Native `feature_gate_tests` passes 63/63,
the native parity matrix passes 28/28, and exact local LLVM passes
66,541/66,541 lines, 8,654/8,654 branches, 3,354/3,354 functions, and
99,405/99,405 regions.

**Evidence boundary:** Managed Coverage MCP remains unavailable at
`project_context` (`Transport closed`), so the accepted claim-ledger tuple
and managed snapshot remain unchanged. This local candidate is not managed
acceptance and claims no Pillow row or diagnostic origin.

**Remaining dependency:** This slice covers only 24-bit BMP-backed ICO row
conversion; ICO indexed conversion, CUR DIB conversion, other codec
interiors, progress callbacks, allocator/peak measurement, short-write
semantics, rollback, cleanup/error precedence, and managed same-revision
evidence remain open. RN-003 remains partial and the complete inventory
remains 266 active finding rows.

#### Current candidate slice — API-036 ICO embedded 32-bit BMP row checkpoints

**Caller problem:** ICO can also carry a 32-bit BGRA BMP/DIB inside its
directory entry. The 24-bit path now checks the caller's token before every
output row, but the adjacent 32-bit BGRA-to-RGBA conversion still walked all
rows without an interior checkpoint. A large icon could therefore spend its
most expensive conversion interval after the last container-level check.

**Pillow answer:** Pillow can prove the final ICO pixels and ordinary errors,
but it does not expose this crate's caller token, checkpoint counter, inclusive
`DecodeWorkUnits` result, or no-partial-state behavior. This is Rust-only
caller-control evidence and adds no Pillow parity row.

**Implemented behavior:** Commit `306d530` threads the token into the
embedded 32-bit BMP dispatch and polls before every output row while copying
BGRA samples into the public RGBA result. The ordinary no-token conversion
keeps its existing pixels and output behavior.

**Source/evidence:** The Rust-only
`ico_decode_work_budget_covers_embedded_32bit_bmp_rows` contract generates
64×64 and 64×128 RGBA images through the public ICO BMP encoder, decodes them
through the public policy API, and proves inclusive boundaries of 68 and 132.
The 64-unit delta is exactly one additional checkpoint for each of the 64
added embedded BMP rows. It also proves exact-boundary pixel identity and
one-unit-below typed `DecodeWorkUnits` rejection. Native
`feature_gate_tests` passes 64/64, the native parity matrix passes 28/28, and
exact local LLVM passes 66,547/66,547 lines, 8,654/8,654 branches,
3,354/3,354 functions, and 99,409/99,409 regions.

**Evidence boundary:** Managed Coverage MCP remains unavailable at
`project_context` (`Transport closed`), so the accepted claim-ledger tuple
and managed snapshot remain unchanged. This local candidate is not managed
acceptance and claims no Pillow row or diagnostic origin.

**Remaining dependency:** This slice covers only 32-bit BMP-backed ICO row
conversion; ICO indexed conversion, CUR DIB conversion, other codec
interiors, progress callbacks, allocator/peak measurement, short-write
semantics, rollback, cleanup/error precedence, and managed same-revision
evidence remain open. RN-003 remains partial and the complete inventory
remains 266 active finding rows.

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

## AVIF planned-gap ledger (current tree)

These are the exact 7 decode rows currently marked `planned` in the generated
matrix. The child-friendly reason is simple: the safe-Rust decoder can read
some small AV1 building blocks, but it cannot yet read every kind of AV1
sentence that an AVIF file may contain. Each row below is a named lesson for
the decoder, not an excuse to route around Rust.

| Pure-Rust work category | Planned rows | Why the work is needed |
| --- | --- | --- |
| General still brand/container control | Closed: baseline and all three accepted-brand rows | The 128×128 baseline and each legal generic-HEIF major-brand ordering now decode through the same safe Rust AV1 path with exact independent 49,152-byte RGB references. |
| Partitioned-square public raster | Closed: all 16 partitioned-square rows | The safe decoder now materializes all twelve cropped 12×12 and four 16×16 4:4:4 square fixtures with exact pinned planes and entropy traces. This category is no longer a planned matrix gap; broader baseline/tile/sequence classes remain separate work. |
| Adjacent entropy and tile syntax | `portable_lossy_420_q99_eob_bin_control`; `portable_lossy_420_q99_eob_base_control` | These are nearby AV1 bitstream sentences. The safe decoder now proves legal EOB-bin-five and EOB-bin-six 8×8 AC classes with independent ramp/diagonal fixtures; these two byte mutations are rejected by the independent decoder and remain explicit negative planned controls. Empty-tile malformed input and the adjacent lossy DC predictor are active. |
| Sample depth and future alpha variants | `high_bitdepth` (the committed `with_alpha` row is active) | A picture may use more than 8 bits or carry a second transparency picture. Pure safe Rust now decodes the committed 64×64 alpha pair to exact RGBA8 pixels; 10/12-bit reconstruction and broader alpha relationships/depths remain explicit future work. |
| Color | `hdr` | HDR changes how numbers become colors. It needs explicit safe-Rust bounds, declared color conversion, and metadata rules. |
| Sequences and frame identity | `animated`; `animated_error_resilient`; `error_animated_repeated_frame_id` | A movie is many pictures plus timing and frame IDs. Safe Rust now rejects the repeated current-ID error case and keeps a primary item independently eligible from a later movie track; first-frame materialization, track presentation, and partial-state rules remain. |
| Multi-tile frame payloads | Closed: `multitile` | Large AV1 frames can split work into independently sized tiles. The committed 256×128 two-column fixture now proves checked tile-size parsing, tile-local reconstruction, one-time canvas placement, frame-global deblocking/CDEF, and exact independent pixels; broader tile shapes remain in the implementation work item. |

All 32 AVIF encode rows are also planned: still conversion/modes, quality and
subsampling, alpha, tiles, metadata, orientation, advanced options, animation,
and invalid-option contracts. The encoder phase starts only after the still
decoder has reusable safe-Rust AV1 primitives; it must emit bytes accepted by
an independent decoder and must not resurrect the removed native bridge.

### Former native-only cases: explicit Rust work, never hidden fallback

The old native bridge used to make these valid inputs appear supported on
some host targets. That is no longer a capability. The current matrix makes
each case a real fixture with a named `gap`, no pixel/output reference, and a
runtime `UnsupportedReason::NotImplemented`. In five-year-old terms: the box
reader can say “I found a picture,” but the Rust picture-maker still needs to
learn that kind of picture before we may call it done.

There are exactly 39 former-native AVIF rows in the generated matrix: 7
decode rows and 32 encode rows. All 39 remaining rows are still `planned`; the executable
matrix test rejects any former-native row that becomes active without the
corresponding pure safe-Rust implementation and independent evidence.

The exact planned groups are:

- partitioned-square public raster and its admitted coefficient classes (closed: all 16 rows are active);
- adjacent AV1 EOB entropy syntax (2 rows);
- 10/12-bit reconstruction and broader auxiliary-alpha composition (1 decode row);
- HDR conversion (1 decode row);
- animation, timing, error-resilient tracks, and frame identity (3 rows);
- broader multi-tile reconstruction remains part of the still/tile work items (the committed `multitile` row is closed); and
- all still/sequence AVIF encoding (32 rows).

In five-year-old terms, these are not “extra Pillow features” that the caller
must configure. They are different kinds of picture sentences that a real
AVIF reader or writer must understand. Pillow is the observable picture
oracle; it does not require this crate to use Pillow's native libraries.

| Planned group | Who needs it | What the safe-Rust codec must learn | Why the old bridge looked finished first |
| --- | --- | --- | --- |
| General still baseline and brand variants | Any app opening an ordinary camera, browser, or export-tool AVIF | Walk every partition and block, reconstruct all luma/chroma samples, and produce one complete bounded frame | The C decoder already contained the large AV1 algorithm; the Rust path currently proves only a bounded closed 16×16 four-leaf contract |
| Partitioned-square public raster | Applications opening small lossless AVIFs with split 12×12/16×16 color blocks | Promote the already-tested partition/CDF walks into one complete public visible-frame raster, including edge crops and all admitted residual classes | The old native path rendered these rows even though the Rust structural probe stopped before public materialization; the gap is now visible instead of being mislabeled active |
| Adjacent EOB entropy syntax | Images whose residual coefficients use these ordinary AV1 coefficient sentences | Decode EOB bins/bases, coefficient signs and magnitudes, dequantization, scans, and transforms without treating a valid sentence as malformed | The native entropy decoder supported the whole syntax table; the first Rust slice intentionally stopped at a smaller proven subset |
| 10/12-bit samples and broader alpha variants | HDR workflows, transparent icons, UI assets, and compositing pipelines | Reconstruct higher-depth planes, convert them with checked arithmetic, and extend relationship-aware alpha pairing beyond the committed 64×64 unassociated 8-bit fixture | Native libraries already owned sample conversion and auxiliary-item composition; pure safe Rust now closes the committed `with_alpha` fixture, while `high_bitdepth` and broader alpha relationships remain planned |
| HDR color and grid composition | Display pipelines and large images assembled from several child pictures | Apply declared color rules and place each decoded child in a bounded cropped canvas | The bridge delegated color and grid assembly to mature native code; Rust now has the safe canvas prerequisite, not the child decoders |
| Animation and frame identity | Messaging, web, and media apps that display animated AVIF | Decode sample tables, references, timing, disposal/blend rules, and sequence limits while keeping state consistent | Native sequence state was already implemented; Rust currently validates structure and limits but does not present multiple frames |
| Multiple AV1 tiles | Large encodes and decoders that split a frame for parallel work | Decode each tile with the correct local entropy/context state and compose it exactly once | The native decoder handled tile scheduling and state; Rust currently proves tile-boundary validation and safe placement only |
| AVIF encoding | Applications exporting or generating AVIF files | Encode color, alpha, quantization, tiles, metadata, and sequences, then pass an independent decoder round trip | The old path called libaom/libavif; no pure-Rust encoder is wired, so every encode row remains honestly planned |

The machine-readable source is `roadmap.json`: every planned AVIF row names
one `pure_rust_work_item`, and the format-level planned encoder default maps
all 32 encode rows to `AVF-ENCODE-001`. `manifest.yaml` is the executable
fixture specification used to generate the matrix projection. The AVIF format
also marks planned rows with `former_native_only: true`; the generator copies
that provenance into `tests/fixtures/coverage_matrix.json` only while a row is
planned. The AVIF integration contract and the roadmap verifier reject a
planned row that has only a prose gap, no named safe-Rust deliverable, or no
explicit record that the removed bridge used to cover the case.

The implementation rule is strict: each group moves to `active` only after a
safe-Rust implementation, a fixture that proves the resulting pixels or
bytes, and an independent compatibility check. A native library, C build
step, FFI boundary, fixture special case, or widened acceptance boundary does
not close a gap.

The dependency map below is the implementation checklist for those groups. It
is deliberately more precise than “add AVIF support”: a future change must
name the syntax or composition boundary it teaches and the fixture that proves
it. The order is important because later work consumes earlier reconstructed
planes.

| Pure-Rust work item | Required before | Exact deliverable | Current state |
| --- | --- | --- | --- |
| `AVF-STILL-001` frame raster | broader partition/tile states | Walk the AV1 partition tree across every tile, retain syntax/CDF and above/left contexts, reconstruct bounded luma/chroma blocks, and compose the visible frame without native state. The 128×128 lossy 4:2:0 baseline, all three legal accepted-brand orderings, and the 256×128 two-column `multitile.avif` frame are now proven full-frame cases; the committed 64×64 lossless 4:4:4 primary in `alpha.avif` is also exact through the alpha row. | Partial implementation: the safe walker and production path now consume all sixteen partitioned 4:4:4 square fixtures—twelve cropped 12×12 cases and four 16×16 cases—in coded payload order, plus the committed two-column multitile frame, the promoted `coverage_r8x16_band_05.avif`/`_06.avif` 8×32 4:2:0 pair, the pinned `coverage_r32x16_origin_01.avif` 32×16 4:2:0 Horizontal32x16 origin leaf, and the pinned `coverage_r16x64_grid_01.avif` 16×64 4:2:0 Vertical16x64 depth-two TX16x16 luma split. The R8x16, Horizontal32x16, and R16x64 fixtures share pinned dav1d topology, checked 4:2:0 plane dimensions, exact entropy traces, and exact public Pillow RGB references; R16x64 additionally verifies the 16×16 matrix-10 luma table and top-only DC prediction for every child without a left neighbor. The promoted `coverage_adst_public_04.avif` adds a 16×4 full-chroma bottom-crop case with two coded 8×8 leaves, an exact 407-operation trace, and exact public RGB/YUV references. The focused `baseline_six_terminal_then_stops_at_vertical_8x16_gap` contract remains a bounded syntax sub-gap. Broader partition/block state, all predictors/residual classes, every filter-intra mode and edge case, additional tile shapes, and independent full-frame proofs remain open. `FrameCanvas::place_cells` validates and atomically places complete reconstructed cells. |
| `AVF-ENTROPY-001` adjacent EOB syntax | the two `portable_lossy_420_q99_eob_*` rows | Implement the EOB-bin and EOB-base branches with their coefficient scans, tokens, signs, dequantization, and transform output; preserve typed `Unsupported` for syntax not yet proven. | Partial: safe Rust handles legal luma EOB-bin 0, 3, 4, 5, and 6 classes, legal chroma EOB-base/high branches including EOB-bin-four, and exact UV dequantization plus matrix-10 data. The two mutation controls remain planned because their independent Pillow oracle rejects the mutated sentences; the six-terminal baseline contract is separate syntax evidence and does not activate those rows. |
| `AVF-SAMPLE-001` sample depth | `high_bitdepth`; later `hdr` | Reconstruct 10/12-bit planes, apply checked sample-to-8-bit conversion at the public boundary, and test overflow, limits, and cancellation. | Partial prerequisite: `av1/sample_depth.rs` now validates 8/10/12-bit nominal ranges and performs explicit high-depth bit truncation; the existing 8-bit materializer uses that checked boundary for color and alpha. Entropy reconstruction, restoration, 4:2:2 decoding, and sequence materialization remain 8-bit-only, so `high_bitdepth` stays planned. |
| `AVF-ALPHA-001` auxiliary composition | broader grid and alpha variants | Decode the primary and monochrome auxiliary AV1 items, validate matching dimensions/depth, distinguish unassociated from premultiplied alpha, and emit the correct RGBA result and source descriptor. | Implemented for the committed `alpha.avif` fixture: safe Rust reconstructs all 37 terminal leaves of the 64×64 monochrome auxiliary tile, derives neighbor state by geometry, pairs the primary and auxiliary planes, emits RGBA8 with source alpha `Auxiliary`, and matches the independent 16,384-byte reference exactly. General alpha dimensions, high bit depth, premultiplied relationships, and broader grid pairing remain planned under the named sample/composition work items. |
| `AVF-COLOR-001` declared color pipeline | `hdr` | Implement transfer, primaries, matrix, range, and sample-position conversion with bounded arithmetic and explicit source metadata. | Planned; current RGB conversion is the narrow 8-bit BT.601 full-range class. |
| `AVF-COMPOSE-001` grid canvas | broader grid counts, dimensions, and relationships | Decode each referenced color/alpha cell, validate cell geometry, place cells in a bounded canvas, and apply relationships without treating metadata inspection as pixel composition. | Implemented for the committed `grid.avif` fixture: safe Rust decodes both 80×64 color cells and their monochrome auxiliary alpha cells, validates complete 80×80 coverage, crops the second row to 80×16, and matches the exact 25,600-byte RGBA8 reference. Broader grid counts, dimensions, tile-boundary contexts, and relationships remain open. |
| `AVF-SEQUENCE-001` track presentation | `animated`; `animated_error_resilient`; repeated-ID case | Parse sample tables, retain frame state and references, enforce IDs/timing/limits, and present frames with default-image and disposal/blend rules. | Planned; stateful validation rejects the named repeated-ID error and primary-item validation is independent from a later movie track, but no multi-frame presentation exists. |
| `AVF-TILE-001` tile raster | broader tile counts/shapes | Decode independently sized tile payloads into one frame canvas with tile-local bounds and shared state only where the AV1 syntax requires it. | Implemented for the committed 256×128 two-column `multitile.avif` fixture: safe Rust decodes and places both tile payloads exactly once, applies frame-global deblocking/CDEF, and matches the independent 98,304-byte RGB reference; the focused reconstruction proof also matches real dav1d all-filter YUV byte-for-byte. Broader tile counts, size combinations, boundary contexts, and full-frame references remain open. |
| `AVF-ENCODE-001` encoder | all 32 encode rows | Write the AVIF container and a safe Rust AV1 intra encoder, then round-trip emitted bytes through an independent decoder. | Planned; no native or pure-Rust encoder is currently wired. |

Every row in this map is a pure safe-Rust task. A native oracle may explain a
bitstream or provide an independent pixel reference, but it cannot satisfy the
deliverable, change a `planned` row to `active`, or become a target-specific
fallback.

The current still-image implementation includes a safe monochrome branch:
the `alpha.avif` auxiliary item is now decoded through all 37 terminal leaves
of its 64×64 lossless tile. A geometry-driven production walker supplies
above, above-right, above-left, left, and left-below state; a checked
one-plane canvas proves that every one of the 4,096 alpha samples is written
exactly once; and the real sample span is obtained from the AVIF parser. The
primary 64×64 color frame is now reconstructed, paired, and checked against
the independent RGBA reference, so the committed `with_alpha` row is active.
This closes one concrete 8-bit unassociated-alpha case; it does not claim
general high-depth, premultiplied, or grid alpha support.

The current implementation slices in this group are present in the portable
decoder and raster module: safe Rust now distinguishes monochrome block syntax
and reconstructs the complete 37-leaf auxiliary-alpha tile, including checked
Paeth and Smooth prediction, generic coefficient sentences, diagonal neighbor
handling, and position-aligned edge CDF windows for unequal neighboring blocks. The
complete monochrome plane is paired with the reconstructed primary color plane
and emitted as a pixel-proven RGBA result for `alpha.avif`; broader alpha
relationships remain planned. The newer checked `FrameCanvas` is the
corresponding safe assembly prerequisite for color, alpha, grid, and tile
planes. It now also validates top-left cropping for coded grid cells whose
visible rectangle is smaller than their coded extent and validates a complete
cell batch before mutation. The committed `grid.avif` and `multitile.avif`
fixtures are active with exact
public references; broader AV1 classes still require safe-Rust implementation
before they can move to `active`.

### RN-006 — Portable AVIF completion — LATER

**Why:** The old implementation used a native AVIF bridge on some targets and
had a different effective contract on WASM. The bridge is now removed. The
final promise is one predictable, pure safe-Rust implementation on every
supported target, with every unsupported case named instead of hidden behind
a native fallback.

**Current exact state:** 251 AVIF decode/inspect/verify rows exist: 244 are
active and 7 are explicit planned gaps. All 32 AVIF encode rows are planned
because no pure-Rust encoder is wired. The exact decode gap ledger is below;
the generated source is `manifest.yaml`, and the generated counts are in
`tests/fixtures/coverage_matrix.json`.

**Work:** Close the planned still gaps in dependency order; add sequence
support and then encoding; expand bit depth, monochrome, planar YUV, alpha,
tracks, timing, progressive/layered content, gain maps, auxiliary selection,
grid composition, item relationships, strictness, limits, random access, AV1
syntax, and independent decoder/browser compatibility as each is justified.
Generate capability decisions from FileTypeBox declarations, item codec
declarations, and target-independent safe-Rust support. Do not treat the
completed FileTypeBox declaration-retention slice as complete decoder
capability.

**Source IDs:** all 33 AVIF rows: `AVF-001`–`AVF-009`, `AVF-011`–`AVF-020`,
and `AVF-022`–`AVF-035`.

**Done when:** native and WASM use the same safe-Rust implementation, every
claimed operation has fixture evidence on its target, portable encoded bytes
have an independent compatibility check, every former native-only case is
either implemented or explicitly justified as still planned, and all AVIF
coverage is fresh.

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

**Source IDs:** `DOC-003`, `DOC-005`–`DOC-008`.

**Done when:** every material claim has an independent source, a revision/date
scope, a validation command, and a visible proved/planned/unknown label.

Documentation audit closure for `DOC-002` (2026-08-13): the retained
`third_party/image-webp/README.md` command transcript now has an explicit
`text` fence, and the open-source documentation audit reports no unlabeled
code fences or other documentation findings. `DOC-002` is therefore removed
from the active inventory; it is not a codec capability claim.

Package-consumer closure for `DOC-004` (2026-08-13):
`examples/package_smoke.rs` is included in the release archive, and
`scripts/verify_package_consumer.py` creates the archive, extracts it into a
fresh temporary directory, installs it as a path dependency in a separate
consumer with default features disabled, and decodes a real PNG. The check
also creates the consumer lockfile and runs the package with `--locked`; it is
the release-package first-use proof, not a repository integration test.
`DOC-004` is therefore removed from the active inventory.

## Complete open-task inventory

The following is the exact set of active roadmap IDs at this review. A task is
not complete until its ID is removed from this list and its current behavior is
moved into the appropriate contract document. The list contains **266 active
finding rows**.

| Area | Count | Open IDs |
| --- | ---: | --- |
| Common API | 25 | `API-008`, `API-014`, `API-017`, `API-018`, `API-019`, `API-020`, `API-023`, `API-026`, `API-027`, `API-030`, `API-033`, `API-034`, `API-036`, `API-041`, `API-043`, `API-044`, `API-045`, `API-046`, `API-047`, `API-048`, `API-050`, `API-051`, `API-052`, `API-053`, `API-054` |
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
| Documentation | 5 | `DOC-003`, `DOC-005`–`DOC-008` |

The shorthand ranges above expand only to the IDs actually present in the
current audit. The historical roadmap is retained for provenance and original
finding context; this file is the canonical status inventory, dependency order,
and acceptance contract.

These 266 rows are not 266 equal-sized coding tasks. A row may be a small
documentation or policy decision, a new fixture, a codec algorithm, a WASM
runtime experiment, or a release gate. The reliable “how much is left” numbers
today are the exact 266 active finding rows, the current four-metric coverage
result recorded above, and the explicit dependency order; an hour estimate
would be invented until the
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
python3 scripts/verify_roadmap.py
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
