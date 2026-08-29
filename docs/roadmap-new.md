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

Reviewed: 2026-08-29

<!-- current-claim-ledger:begin -->
Current claim-ledger baseline (not current `HEAD`):
- Measured revision: `93ec80ec99c42671dce6cf70694bce27ad8a2ef4`.
- Coverage MCP run: `ec4c4bbd-dbda-4e49-8109-d7da07722dc0`; snapshot: `7665cda3-f4a7-4568-b871-a9d34afaa92c`.
- Coverage: 100,389/110,015 lines (91.2503%), 12,861/14,246 branches (90.2780%), 5,125/5,794 functions (88.4536%), and 150,221/166,375 regions (90.2906%).
- Manifest SHA-256: `0068a99aab9c70d3fa3863f9cb9d1ece83edf71d9d61ab3623c493e312e77698`; generated matrix SHA-256: `84ff26313b9ef8e445936869560c816d7617d6834636190511f5aa2f47e1c1e6`.
<!-- current-claim-ledger:end -->

- Current claim-ledger implementation anchor: `93ec80ec99c42671dce6cf70694bce27ad8a2ef4`
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
not close the 250-item active-finding roadmap below: format capability, metadata,
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

- AVIF decode/inspect/verify: 315 rows total, 312 active, 3 explicit planned
  gaps.
- AVIF encode: 32 rows total, all 32 explicit planned gaps; no encoder is
  wired yet.
- Whole matrix: 1,539 rows total, 1139 active decode rows, 365 active encode
  rows, 3 planned decode rows, and 32 planned encode rows.
- New bounded AVIF witness: `coverage_h16x4_tx4x4_split_01.avif` is a 16x16,
  8-bit 4:2:0 `PARTITION_H4` stream whose following `Horizontal16x4` leaf
  selects transform depth two, yielding four TX4x4 luma children and
  exercising matrix-8 luma dequantization. Safe Rust matches the pinned
  768-byte Pillow RGB8 reference (fixture SHA-256
  `cbd0c319225f4542fdc99bc9b414252e259f0416a6b43878ca82880d7b98ced2`; RGB
  SHA-256 `cafbd0adea2de9433149b1e444a7bf8d67f2a09578c815dd69c8f098fd58c626`).
  This is bounded depth-two evidence only: depth one with TX8x4 children,
  other H16x4 states, and `AVF-STILL-001` remain partial.
- The new bounded AVIF witness is `coverage_h16x4_tx8x4_split_01.avif`: a
  deterministic 16x16 8-bit 4:2:0 `PARTITION_H4` stream whose following
  `Horizontal16x4` leaf selects transform depth one, yielding two TX8x4 luma
  children with DCT-DCT transforms and EOB values 5 and 2. The qcat-three
  stream exercises the verified luma matrix-6 and R8x4 coefficient sentence;
  safe Rust matches the independent 768-byte Pillow RGB8 reference (fixture
  SHA-256 `546b40c28569c5d751fd4ba435f73e9af91da1f2e773dce54e94d9b3fb27873d`;
  RGB SHA-256
  `ef2809ae0834bdb5f3aaf71eeddbad0f5589ed0391fb4d010ac879ff5622bb54`).
  The matrix row keeps header-only container verification and exact public RGB
  parity; an input-only pinned dav1d probe confirms the two-child topology.
  One-dimensional V_DCT/H_DCT, other H16x4 states, and `AVF-STILL-001` remain
  partial.
- The newest bounded AVIF witness is `coverage_h16x4_h_dct_cfl_01.avif`: a
  deterministic 16x16 8-bit 4:2:0 origin `PARTITION_H4` stream with four
  ordered, unsplit `Horizontal16x4` luma leaves. Every luma leaf selects H_DCT
  (CDF symbol 3, dav1d `txtp=11`, EOB 63); leaves 1 and 3 additionally select
  UV mode 13/CFL with nonzero TX6 U/V payloads. Safe Rust matches the pinned
  3,780-operation scalar dav1d trace, exact partition and Y/U/V planes, and
  Pillow RGB8 output. Fixture SHA-256 is
  `c7597bf32c95f175e814bc2962e295ee7369b396892cd649d1f623d8a86f881c`;
  encoded-item SHA-256 is
  `800f1eeb5dd7a6e11a4550a2a027835fb471654c7728a60937bc48d5994db703`;
  Pillow RGB SHA-256 is
  `8b61bc973b7dadbed03b497390a1cef5640cce91d9d09196cd9bf212bebc267e`.
  This promotes only the exact observed H_DCT/CFL class; broader H_DCT,
  V_DCT, other predictor/partition/transform states, and `AVF-STILL-001`
  remain partial.
- The newest bounded following-leaf proof is
  `coverage_h16x4_following_h_dct_01.avif`: a deterministic 32x16 8-bit
  4:2:0 `PARTITION_SPLIT` frame with two level-3 `PARTITION_H4` children and
  eight ordered unsplit `Horizontal16x4` luma leaves. The top right-hand leaf
  has no chroma by 4:2:0 geometry, consumes the preceding left group's
  four-sample right edge, and selects H_DCT (CDF symbol 3, dav1d `txtp=11`,
  EOB 63); the lower right leaves select UV mode 13/CFL with nonzero TX6 U/V
  payloads. Safe Rust now admits the missing horizontally following
  monochrome H16x4 path and matches the pinned 7,485-operation scalar dav1d
  trace, exact partition and Y/U/V planes, and the independent 1,536-byte
  Pillow RGB8 reference. The input-only campaign evaluated 100 candidates
  across 10 families, qualified 65, and promoted
  `h16x4-following-f07-n02` (seed 2,164,602) without invoking repository Rust;
  its report is
  `tests/fixtures/outputs/av1_search/coverage_horizontal16x4_following_campaign_01.json`
  (SHA-256
  `33bcdaaca0c559de0efa1d4c01372db3d25212bf63809fd9aff2bf278e67d97a`).
  Fixture, encoded-item, and Pillow RGB SHA-256 values are
  `623d8ac1eb5ecfc846c6d16c503d131109134b7ca6ad248b98155995da27af5f`,
  `06810448987533c5e4d14a00629c3f609738a5a758edbdebae0c13bbcac0c1d1`, and
  `85977d9e8beab45b30906bbe60c7918b332d5e6fa4c4719177e203f92ce82356`.
  This closes only the bounded following right-hand H16x4 H_DCT/CFL
  edge-handoff class; V_DCT, broader H16x4 states, other dimensions and
  optional tools, sequences, encoding, and `AVF-STILL-001` remain partial.
- The newest bounded upper-context witness is
  `coverage_r16x8_neighbor_01.avif`: a valid 32x16 8-bit 4:2:0 reduced-still
  frame whose two-block partition places a 16x8 neighbor below the upper
  context. The safe-Rust fix is generic: direct `Square16` luma skip decoding
  selects contextual transform-context-two rows for qcat 0/2 and the scalar
  sentence for qcat 1/3; qcat 2 context zero shares the equivalent scalar
  adaptive state, while qcat 0 retains its independently evidenced row. The
  qcat-0 16x16 EOB-high rows are stored in dav1d source order while retaining
  `eob_symbol - 2` indexing. The corrected trace matches all 416 pinned dav1d
  entropy operations, partition records, Y/U/V planes, and the independent Pillow
  RGB8 hash. Fixture SHA-256 is
  `12e2ed4b6327eacb73015c074fcef1b5ba3c3c141ff3de7cc195263c8d9a7b70`; the
  encoded-item SHA-256 is
  `82523f7f2d713f0ebb5bf42d2c6ebcd01406dee89742afbf25e3ade4dd2c640c`; and
  the Pillow RGB SHA-256 is
  `1d491d7f9084f851562b16b5f6027cfccd0077bd028dc9b914f5e86b4d890808`.
  The permanent reconstruction suite passes 245/245 cases, all 312 active AVIF
  rows pass, and the new row is active in the generated matrix. The durable
  implementation commit is `212d273bb757c214ee9079e845cad2e6e033523b`; its
  managed incremental Coverage MCP run is
  `540ac99d-c866-4103-aab8-5ad2990b8ede` (42,582 ms), with ingested snapshot
  `4018c9f3-8a2d-4158-8ed4-f136c003b8db` against explicit baseline
  `7665cda3-f4a7-4568-b871-a9d34afaa92c`. The exact selected run passed 1/1,
  filtered 43, and reports additive-union deltas of +8 lines, +10 branches,
  +0 functions, and +979 regions; denominator changes are +4,493/+566/+67/
  +12,861. Its selected snapshot diff reports 660 newly covered line
  identities, 75,292 baseline observations not observed in this selected
  subset, and zero tool-reported regressions. Test attribution was unavailable,
  merge exactness was false with conservative fallback, and the ordinary
  snapshot metadata retains commit `3272b3ef49a87c2947c08b46596b442195c6a8db`
  as a provenance caveat. This is supported aggregate, bounded selected-subset
  evidence—not a complete four-metric release measurement or a global
  regression claim. Broader neighbor geometries, alternate contexts, optional
  tools, sequences, encoding, and `AVF-STILL-001` remain partial.
- The managed incremental Coverage MCP run for this new reconstruction
  selector is `fc9d6269-8fd4-4a6b-9497-14cc6bd28ea3`, with ingested snapshot
  `809c0839-05a7-4cb5-beae-4c059d9405b7` against explicit baseline
  `7665cda3-f4a7-4568-b871-a9d34afaa92c`. It passed in 34,607 ms at exact
  implementation commit `a67a00d043b799d94ed0798745b69acb934bc343`; the
  selected test passed 1/1 with 43 filtered out. Its additive baseline-union
  review reports +8 lines, +10 branches, +0 functions, and +938 regions;
  denominator changes are +4,249/+446/+49/+12,526, and the additive-union
  projection reports 2,022 newly covered line identities. The selected
  snapshot diff reports 661 newly covered line identities, 75,660 baseline
  observations were not observed, and zero regressions were reported. Merge
  exactness is false and named-test attribution is unavailable, so this is
  bounded selected-subset evidence rather than a complete four-metric release
  measurement.
- The bounded H16x4 coefficient-state follow-up is explicit negative evidence,
  not a production admission. `scripts/explore_avif_horizontal16x4_eob.py`
  evaluated 100 deterministic candidates across 10 families in the proven
  32x16 8-bit 4:2:0 following-H16x4 H_DCT topology, using Pillow 12.2.0,
  libavif 1.4.1, libaom 3.13.2, and scalar dav1d 1.5.3 at commit
  `b546257f770768b2c88258c533da38b91a06f737`; repository Rust was not invoked.
  Sixteen candidates retained the complete topology and 13 candidates emitted
  a novel EOB-bin/base signature, but no candidate satisfied both predicates;
  the 16 topology-qualified candidates reused the two active signatures. The
  durable report is
  `tests/fixtures/outputs/av1_search/coverage_horizontal16x4_eob_campaign_01.json`
  (SHA-256
  `bf8c89e10086caeb5983e2ccfb313793c0f42246aa7db4734efd40e3b7b9f79c`).
  This bounds only the declared corpus and does not prove any EOB sentence
  unreachable; no fixture, manifest, decoder admission, or denominator change
  follows.
- The new bounded AVIF witness is
  `coverage_v4x16_predictor_adst_adst_01.avif`: a deterministic 16x16 8-bit
  4:2:0 `PARTITION_V4` stream with four `Vertical4x16` luma leaves. Its
  input-only five-campaign search evaluated 100 candidates per campaign and
  promoted `v4x16-f07-n07` (seed 107) after one candidate qualified. The
  third leaf selects AV1 `DCT_ADST` (CDF symbol 6 / dav1d `txtp=2`), while the
  other leaves select DC, Paeth/`ADST_ADST`, and Horizontal/`DCT_DCT`.
  Pinned dav1d reports 263 entropy operations; safe Rust now maps AV1’s
  vertical-then-horizontal names to the width-pass/height-pass rectangular
  kernels and matches the exact reconstructed Y/U/V planes and 768-byte
  Pillow RGB8 output. Fixture SHA-256 is
  `e5c6fe86bdc3a1339836521421aba220e4fe703b1379483d7b69972af974920b`; RGB
  SHA-256 is
  `66d1531446de70283fcb048f1f82f7c0a5e454eaf8e2bee70afa8efecf683994`.
  This closes only the bounded predictor-enabled Vertical4x16 transform-
  dispatch class; other rectangular states and `AVF-STILL-001` remain partial.
- The completed high-depth slice is
  `high_bitdepth_still_10bit_444_lossless.avif`: a single-frame 10-bit
  profile-1, full-range BT.601/SDR, all-lossless 4:4:4 image. Checked u16
  reconstruction matches the independent dav1d 1.5.3 Y/U/V oracle and the
  Pillow 12.2.0 RGB8 reference exactly. This closes one bounded 10-bit class;
  the separate 12-bit still class below is now active, while HDR, high-depth
  alpha/sequence, other subsampling, restoration, and encoding remain planned.
- The new bounded 12-bit slice is
  `high_bitdepth_still_12bit_444_lossless.avif`: a single-frame AV1 profile-2,
  full-range BT.601/SDR, all-lossless 4:4:4 image. Independent AV1C/ISO-BMFF
  inspection records 12-bit samples, still-picture mode, one tile, and base
  qindex 0; safe Rust matches the exact 768-byte Pillow 12.2.0 RGB8 reference.
  The fixture SHA-256 is
  `8645ee1ecc437868c5842248444ea6c8400d983a03bcfe75710bb0a424915abd`, and the
  RGB SHA-256 is
  `9bb7dfcac6b47a80ec62d5d1732dc2d5954390e55c8462b312f5eb2ccb332661`. This
  closes only this single-frame 12-bit 4:4:4 class; the animated
  `high_bitdepth` row, other subsampling, restoration, alpha/sequence, HDR,
  and encoding remain open.
- The new bounded 4:2:2 witness is
  `coverage_422_square16_vertical_halves_01.avif`: a deterministic 16x16
  8-bit origin `Square16` stream with Y/UV palettes, skipped residuals, one
  TX16x16 luma transform, and TX8x16 U/V transforms over half-width,
  full-height chroma. Safe Rust matches the pinned dav1d 1.5.3 partition,
  461-operation entropy trace, exact 16x16 Y and 8x16 U/V planes, and Pillow
  RGB8 output. Fixture, encoded-item, and Pillow RGB SHA-256 values are
  `6c524c2b189f47893ace4e93ea0bdb1123cfbd555c5102991679e0e9d9854a49`,
  `c4aa6f97fa02301e61829ab68dc85808f8d16a640daa32e5aecdec90bb0a63c9`, and
  `bf1af25691e0092747fa281f45b6023dfeab8d34946e10e20f4500674e7931d7`.
  This closes only the simple `Square16` palette class; other 4:2:2
  geometries, residual-bearing palette blocks, optional tools/filters,
  broader AV1/AVIF support, and `AVF-STILL-001` remain partial.

- The newest bounded AVIF witness is
  `coverage_h16x4_filter_intra_tx8x4_split_01.avif`: a deterministic 16x16
  8-bit 4:2:0 `PARTITION_H4` stream whose following `Horizontal16x4` leaf
  selects filter-intra mode 4 (trace value `13/4`) and transform depth one.
  It reconstructs two terminal TX8x4 DCT-DCT luma children with EOB values 0
  and 2. Safe Rust predicts each child separately, publishes the first
  child's reconstructed right edge before predicting the second, and matches
  the pinned 110-operation dav1d trace, exact Y/U/V planes, and independent
  768-byte Pillow RGB8 output. Fixture, encoded-item, and Pillow RGB SHA-256
  values are
  `e2660fd8efe6609ec42182bf7edfab466b0dee57c12d5beebcea8aad03ff67c0`,
  `fb39cd215b49aac96047e79924569f9db3a2d4f20b781776d157a180601a817f`,
  and `bdc12de89d8516533e6678fe9f3eb3639b45dff2f7e91402003a3a7ff4d2bdc3`.
  This closes only the bounded split filter-intra TX8x4 class; other
  filter-intra modes, transforms, topologies, and `AVF-STILL-001` remain
  partial.

- The common-valid candidate from the H16x8 campaign is now promoted as
  `coverage_h16x8_origin_dct_dct_01.avif`: a 16x8 8-bit 4:2:0 origin
  `Horizontal16x8` leaf with one unsplit TX16x8 DCT-DCT luma payload (transform
  CDF symbol 1, EOB 127) and skipped TX8x4 DCT-DCT U/V payloads. Its pinned
  dav1d trace has 154 entropy operations and one exact partition record
  (`poc=0,y=0,x=0,level=3,context=0,partition=1,range=46840`). Safe Rust
  matches the exact Y/U/V planes and 384-byte Pillow RGB8 output. The
  production boundary adds the qcat-three CDF sentences and matrix-7 R16x8
  dequantization, then admits only this origin/no-filter/DC/DCT-DCT class.
  Fixture SHA-256 is
  `abcc5033662bf727681061787deddc63e3bf8e9c16af6dd700848339c09ca9f2`, encoded
  item SHA-256 is
  `bebf964204dc4e5be3a103fc621cfb19ea35447ce131e0d675c567a6df492cca`, and
  Pillow RGB SHA-256 is
  `2252e089ce514157ab53e4e99f73bab1840ae1b78dab2dab4d52cf78c372f0ab`. The
  campaign's identity/V_DCT target remains 0/100; this promotion closes only
  the common DCT-DCT witness, not identity/V_DCT, following H16x8, split/depth
  variants, alternate transforms, nonzero chroma, or `AVF-STILL-001`.

  The bounded origin R16x8 H_DCT follow-up search is retained as explicit
  negative evidence, not a completion claim. The deterministic input-only
  campaign ran 100 candidates across 10 families, required the exact H_DCT
  transform CDF symbol 3/dav1d `txtp=11`, one 128-value luma dequantized dump
  with direct nonzero AC, and two skipped TX8x4 DC chroma payloads; it
  qualified 0 candidates and promoted none. The report is
  `tests/fixtures/outputs/av1_search/coverage_horizontal16x8_h_dct_campaign_01.json`
  (SHA-256
  `184dfc73926ad1e320b0fd356f3a4782b71875a952d38e3c52a1acd0f2aa1940`).
  It used Pillow 12.2.0/libavif 1.4.1/libaom 3.13.2 and pinned dav1d
  `b546257f770768b2c88258c533da38b91a06f737`, without invoking repository
  Rust. No production, matrix, fixture, or Coverage MCP change follows: the
  nearby following-H16x8 DCT-DCT class is already active, and no genuinely
  unpromoted positive class was found. This result bounds only this search
  corpus and predicate; it does not prove H_DCT unreachable.

  Managed Coverage MCP recorded the exact selected-fixture incremental run
  `a399cf98-f51b-48ac-a248-6c267fe56d8b` against explicit baseline snapshot
  `7665cda3-f4a7-4568-b871-a9d34afaa92c`. It passed in 78,479 ms at exact
  implementation commit `7f7e76be3b94128f65472f17631d098aa19be7ea` and
  ingested snapshot `469dc5dd-36f1-47a2-b3fd-ce2a995336af`. The standalone
  additive baseline-union review reports +663 covered lines, +17 branches,
  +1 function, and +5,289 regions, with denominator changes of
  +3,639/+364/+39/+10,218 and zero reported regressions. The merge is
  conservative rather than exact; 7,400 selected-projection line identities
  were newly covered, 39,411 baseline observations were not observed, and
  named-test attribution is unavailable. The compact snapshot projection
  retains metadata commit `3272b3ef49a87c2947c08b46596b442195c6a8db`; the
  durable run record is authoritative for implementation provenance. This is
  bounded selected-subset evidence, not a complete four-metric release
  measurement or a global regression claim.

  Managed Coverage MCP then ran the exact reconstruction selector against
  baseline snapshot `7665cda3-f4a7-4568-b871-a9d34afaa92c`: run
  `44360fd2-4d9c-4ce0-845b-deef0d7c0ef1` passed in 31,912 ms and ingested
  snapshot `5b0a5d63-dfe0-447a-9e3e-ffe7a97a08cb` for implementation commit
  `72759602317c50016a6bf38fc80ee06bb1de9afe`. The standalone incremental
  review reports additive deltas of +8 covered lines, +7 branches, +0
  functions, and +281 regions, with denominator changes of +3,211, +126,
  +26, and +9,543 and zero reported regressions. The merge is conservative
  rather than exact, attribution is unavailable, and the selected projection
  reports 1,474 newly covered line identities; this is bounded subset
  evidence, not a complete release measurement.

  The matching selected matrix-row run `a740718c-1912-4280-8cff-4969d1acf19e`
  passed in 30,162 ms and ingested snapshot
  `ff859be1-e7ab-4f82-96c9-84044d1f24cc` at the same implementation commit.
  Its additive review reports +8 covered lines, +8 branches, +0 functions,
  and +281 regions with the same denominator changes and zero reported
  regressions. Both compact snapshots retain metadata commit
  `3272b3ef49a87c2947c08b46596b442195c6a8db`; the exact run records retain
  the implementation commit above.

  Managed Coverage MCP then ran the exact selected-row incremental command
  against baseline snapshot `7665cda3-f4a7-4568-b871-a9d34afaa92c`: run
  `270c0935-f337-42d1-9249-2b6e8b37624b` passed in 65,949 ms and ingested
  snapshot `58bf18f6-f5a1-46c2-a5c3-ac64c5ec7439` at commit
  `1cc231d0c0a28ad2eb7a0caf2a32a643b357d6d4`. The bounded incremental review
  reports additive deltas of +302 covered lines, +0 branches, +0 functions,
  and +0 regions, with denominator changes of +2,823, +72, +10, and +582.
  The merge is conservative rather than exact, named-test attribution is
  unavailable, and this selected-subset run is not the complete four-metric
  release measurement.
- Earlier bounded AVIF witness: `coverage_r32x8_h4_ripple_01.avif` is a 32x32 8-bit
  4:2:0 `PARTITION_H4` frame with three 32x8 luma leaves, 16x4 subsampled
  chroma leaves, an exact 1,522-operation dav1d trace, exact Y/U/V planes,
  and exact Pillow RGB output. It exercises geometry-specific matrix-9
  dequantization and one-sided DC prediction; it does not close AVF-STILL-001.
- The corrected bounded AVIF witness is
  `coverage_r32x8_filter_intra_cdf9_false_01.avif`, a 32x32 8-bit 4:2:0
  `PARTITION_H4` frame with four terminal Horizontal32x8 luma leaves, four
  CDF-index-9 false filter-intra sentinels, and four 16x4 DCT-DCT chroma
  leaves. The input-only campaign searched 100 deterministic candidates and
  qualified 1 (F10/N05, seed 7095); its pinned scalar dav1d trace has 2,015
  entropy operations, and safe Rust matches exact Y/U/V and Pillow RGB bytes.
  This proves parser coverage for the false CDF decision, not filter-intra
  reconstruction or general AV1 completion.
- The newest bounded proof is `coverage_h64x16_horizontal_ramp_01.avif`, a
  64x64 8-bit 4:2:0 `PARTITION_H4` frame whose first 64x16 block exercises
  four 32x16 luma children, chroma-only palette reconstruction, the 4:1
  R64x16 DCT-DCT transform, and frame-edge context publication. Its pinned
  dav1d trace has 641 entropy operations, and safe Rust matches the exact
  Y/U/V planes and Pillow RGB output. It closes only this H64x16
  transform/topology/palette class; AVF-STILL-001 remains partial.
- The current bounded full-resolution proof is `coverage_i444_rect_01.avif`, a
  16x16 8-bit lossy 4:4:4 frame with a split root and four 8x8 terminals.
  Its pinned dav1d trace has 499 entropy operations; safe Rust matches exact
  16x16 Y/U/V planes and Pillow RGB8 bytes, including full-resolution chroma
  residuals, matrix-10 U/V AC deltas, and delta-Q. This closes only this
  I444 topology/predictor/residual class; AVF-STILL-001 remains partial.
- The second bounded full-resolution proof is `coverage_i444_rect_02.avif`, a
  16x16 8-bit lossy 4:4:4 horizontal-gradient frame with the same split-root/
  four-leaf geometry but a distinct 553-operation dav1d trace. Safe Rust matches
  its exact Y/U/V planes and Pillow RGB8 bytes; it adds distinct residual/EOB
  states and a filter-intra leaf. This remains bounded evidence, not general
  I444 or AV1 completion. The previous-slice managed snapshot below records
  that promotion; its zero metric delta is retained as historical context.
- The newest bounded following-leaf proof is
  `coverage_h16x8_following_dct_dct_01.avif`: a deterministic 32x8 8-bit
  4:2:0 frame whose root is `PARTITION_SPLIT` with two level-3 top-row
  `PARTITION_HORZ` Horizontal16x8 leaves. The right leaf begins at pixel
  x=16, consumes the completed left luma edge, uses an unsplit TX16x8
  DCT_DCT luma payload with transform CDF symbol 1 and EOB 99, and uses
  skipped TX8x4 DCT_DCT U/V payloads with EOB -1. The pinned dav1d 1.5.3
  trace has 248 entropy operations and exact partition ranges
  `38416/54924/43772`; safe Rust matches the exact Y/U/V planes and 768-byte
  Pillow RGB8 output. The input-only campaign evaluated 100 candidates across
  10 families, qualified 38, and promoted `h16x8-following-f01-n00` (seed
  328000) without invoking repository Rust. Fixture, encoded-item, Pillow
  RGB, Y/U/V, and trace SHA-256 values are recorded in `roadmap.json`. This
  closes only the bounded following-Horizontal16x8 DC/DCT-DCT/unsplit/
  skipped-chroma class; broader following geometry, alternate transforms,
  optional tools, sequences, encoding, and AVF-STILL-001 remain open.
  Managed Coverage MCP run `56fb2027-255c-4e34-a8d0-b67e88f7db04` passed in
  81,649 ms at exact implementation commit `349f5d436b79a26086a28b6110aeb67eb3c374d6`
  and ingested snapshot `12bfe199-0967-4e75-8246-c1fbd4724b5a` against
  explicit baseline `7665cda3-f4a7-4568-b871-a9d34afaa92c`. Its standalone
  incremental review reports +662/+17/+1/+5,289 covered line/branch/function/
  region deltas, denominator changes +3,639/+364/+39/+10,218, 7,393 selected
  line identities newly covered, 39,434 baseline observations not observed,
  and zero regressions. This is conservative selected-subset evidence with
  unavailable named-test attribution, not the complete four-metric release
  measurement.
- The newest bounded origin proof is `coverage_r32x32_filter_intra_probe_01.avif`,
  a 32x32 8-bit 4:2:0 horizontal split with an origin Horizontal32x16
  filter-intra leaf followed by another Horizontal32x16 leaf. Its origin uses
  y-mode 13/filter mode 2 with top=127, left=129, and top-left=128, has a
  pinned 1,168-operation dav1d trace, and matches exact Y/U/V planes and
  Pillow RGB output. It closes only this origin filter-intra class.
- The latest bounded following-leaf proof is
  `coverage_r32x32_filter_intra_mode3_01.avif`, a 32x32 8-bit 4:2:0
  horizontal split with filter-intra mode 3 on both Horizontal32x16 leaves.
  Its pinned dav1d trace has 4,359 entropy operations; safe Rust reconstructs
  the following leaf from its prepared spatial edges and matches exact Y/U/V
  planes and Pillow RGB8 bytes. It closes only this following-leaf mode-3
  class; broader filter-intra modes and AVF-STILL-001 remain partial.
- The newest bounded I444 proof is
  `coverage_i444_v16x32_following_filter_intra_mode3_01.avif`, a 32x32 8-bit
  lossy 4:4:4 frame with two side-by-side Vertical16x32 terminals. The right-
  hand following leaf uses filter-intra mode 3, two TX16x16 luma children, and
  one RTX16x32 transform for each full-resolution chroma plane. Its pinned
  dav1d trace has 7,446 entropy operations; safe Rust matches exact Y/U/V
  planes and Pillow RGB8 bytes. This closes only this bounded I444 following-
  leaf class; broader rectangular transforms and AVF-STILL-001 remain partial.
- The newest bounded origin split proof is
  `coverage_r32x16_filter_intra_tx8x8_01.avif`, a 32x16 8-bit 4:2:0 origin
  Horizontal32x16 split leaf with filter-intra disabled and four TX8x8 luma
  children. Its trace records the `[0/0]` disabled sentinel, not a filter-intra
  mode selection.
  Its pinned dav1d trace has 2,328 entropy operations; safe Rust matches exact
  Y/U/V planes and Pillow RGB8 bytes. This closes only the origin
  TX8x8 split-transform class; following-leaf split routing, filter-intra modes
  and edges, and AVF-STILL-001 remain partial.
- The previous bounded following-leaf split proof is
  `coverage_r32x32_following_filter_intra_split_mode0_01.avif`, a 32x32
  8-bit 4:2:0 horizontal split whose following Horizontal32x16 leaf selects
  filter-intra mode 0 and a TX16x16 luma split. Its pinned dav1d trace has
  2,204 entropy operations; safe Rust matches exact Y/U/V planes and Pillow
  RGB8 bytes. It closes only this following-leaf mode-0/TX16x16 class; broader
  filter-intra modes, transform depths, and AVF-STILL-001 remain partial.
- The newest bounded following-leaf split proof is
  `coverage_r16x32_following_filter_intra_split_mode3_01.avif`, a 32x32 8-bit
  4:2:0 vertical split with a right-hand following `Vertical16x32` leaf using
  filter-intra mode 3, two TX16x16 luma children, and one R8x16 U/V transform
  each. Its pinned dav1d trace has 4,444 entropy operations; safe Rust matches
  the exact partition record, every entropy operation, reconstructed Y/U/V
  planes, and Pillow RGB8 bytes. The production path uses the prepared left
  edge and dav1d's left-only DC availability rule. This closes only this
  right-hand 4:2:0 mode-3/R8x16 split class; broader filter-intra modes,
  topologies, transforms, and AVF-STILL-001 remain partial.
- The newest bounded right-hand mode-0 proof is
  `coverage_r16x32_following_filter_intra_split_mode0_01.avif`, a deterministic
  32x32 8-bit 4:2:0 vertical split whose right-hand `Vertical16x32` leaf uses
  `FILTER_PRED[13/0]`, two TX16x16 luma children, and R8x16 U/V transforms.
  Its pinned dav1d trace has partition range `35904` and 3,064 entropy
  operations; safe Rust matches every entropy operation, reconstructed Y/U/V
  plane, and Pillow RGB8 byte. The slice also fixes the reachable TX8x8
  left-only-DC availability bug exposed by this witness. It closes only this
  right-hand mode-0/R8x16 split class; broader filter-intra modes, topologies,
  transforms, and AVF-STILL-001 remain partial.
- The newest bounded origin proof is
  `coverage_square16_filter_intra_mode0_01.avif`, a 16x16 8-bit 4:2:0
  origin `Square16` leaf using `FILTER_PRED[13/0]`, one unsplit TX16x16 luma
  transform, and TX8x8 U/V transforms. Its exact pinned dav1d trace has
  partition range `62320` and 1,116 entropy operations; the safe-Rust path
  matches every entropy record, reconstructed Y/U/V plane, and Pillow RGB8
  byte. The fixture, encoded-item, Pillow RGB, and Y/U/V plane SHA-256 values
  are `2fb3de2676b560d379d05782b3e57c7af028b2fdac0350364389b3f9ceb77bcc`,
  `2afe883ff75f1b7ce779969b5ac7397ade8f690a11f75edeb3c534579fe9888c`,
  `4090aed7681e287536328b3ec8ee9235c8e32979b8a249824d258fd57145b008`,
  `49d3cfe8c3a0c5db878cb8f17f9de079f273b944f7490b48ec9045bc7d7fc0ee`,
  `3a432bf4885785a8e1b7b27ef823de21bce1fb7f696eee015ebe74dd15036128`, and
  `1b3eaf525d2f5a085858c437fd0ebc98d8ab3593df7b696900eb258df064bb75`.
  This closes only the origin Square16/mode-0/unsplit-TX16x16/TX8x8-chroma
  class; filter-intra modes, topologies, transforms, and AVF-STILL-001 remain
  partial.
- The latest bounded origin proof is
  `coverage_vertical8x16_filter_intra_mode0_01.avif`, an 8x16 8-bit 4:2:0
  origin Vertical8x16 leaf using `FILTER_PRED[13/0]`, one unsplit TX8x16 luma
  transform, and TX4x8 U/V transforms. Its exact pinned dav1d trace has
  partition range `42232` and 584 entropy operations; safe Rust matches every
  entropy record, reconstructed Y/U/V plane, and Pillow RGB8 byte. The fixture,
  encoded-item, Pillow RGB, and Y/U/V plane SHA-256 values are
  `da511e016e1e8720cb21af34b4cf41001a97af0f0380576dc47355dcd630f39a`,
  `e86cc0fdfc27ec55e542a581bb22b4c619f5dfac793593ec7b276a13df6d8224`,
  `82b2100ac5f6f02e88ea931a90b2abab261b7486209ee4f63c538464c52b5c30`,
  `b2785ade1a3c4756d80bf67138b50d410eb2863ff39410e94b8cfd44467baba6`,
  `fe140aecdaf68c2a55f594a0a1eb6f9404e9e70f452aaee2d73fe7b98af6014a`, and
  `a085afa18ac9de9d6f9c09b3fa6050395bbf1cc71d4444f3cfae4354057469e8`.
  It closes only this origin Vertical8x16/mode-0/unsplit-TX8x16/TX4x8-chroma
  class; broader filter-intra modes, transforms, and AVF-STILL-001 remain
  partial.
- The newest bounded following-leaf proof is
  `coverage_square8_chroma_diagonal113_01.avif`, a 16x8 8-bit 4:2:0
  horizontal split whose right-hand `Square8` leaf selects chroma
  `Diagonal113`, ADST-DCT U/V transforms, and a split TX4x4 luma grid. Its
  pinned dav1d trace has partition ranges `37392`, `43662`, and `63946`, plus
  304 entropy operations. Safe Rust now prepares origin angular luma edges,
  honors the AV1 IDTX matrix rule, and supplies the following leaf's
  left-only chroma edge; it matches the exact partition, entropy, Y/U/V, and
  Pillow RGB evidence. The fixture, encoded-item, and Pillow RGB SHA-256
  values are
  `c014c0d3a2108ab2e97b3dd7575985dec029390b049d08335faa8b3d2aad31f7`,
  `6940c3d9ff199ebb028dda748b79fb56c649c4438f0bc4166163a498eabf5c8c`, and
  `05f6f725de2e882646a7bf059b444ffc26e2a7b048ad09f573890222bd029462`.
  This closes only the right-hand Square8/chroma-Diagonal113/ADST-DCT class;
  broader AV1 partition states, chroma modes, transforms, and AVF-STILL-001
  remain partial.
- The newest bounded origin proof is
  `coverage_vertical8x16_chroma_vertical_01.avif`, a 16x16 8-bit 4:2:0
  vertical split whose origin `Vertical8x16` leaf selects chroma `Vertical`
  (mode 1), while the following `Vertical8x16` leaf selects `Diagonal113`
  (mode 5). Both leaves use ADST-DCT TX4x8 U/V transforms and non-palette
  luma. Its pinned origin partition record is
  `poc=0,y=0,x=0,level=3,context=0,partition=2,range=57408`, with 539
  entropy operations. Safe Rust matches the exact partition, entropy trace,
  reconstructed Y/U/V planes, and Pillow RGB bytes. Fixture, encoded-item,
  Pillow RGB, and Y/U/V plane SHA-256 values are
  `2e397a17d61aad197148e86f64f2d93b6afa1c3ac3f7acb9a72370d43b3da108`,
  `dd2247182b63cced68e717c22e7d293dd6e1db542d932be6fb112851fd9bba63`,
  `56c7822ea3a4ea606bd563b91d17a96a25fb54afa85aea7ce57d3b75f60fa794`,
  `2b259272c564c93445d7bda8e21d3ea47a40d5d84788223551ddc9e2e2ac4155`,
  `7838e36888788274aba05902e63c486a08a94c07ac076bdd499d3cd9f53bb6a7`, and
  `058b8c5017e816cc8b12604008589121c78ac7ff9068f5d55c48d711eebdbaf8`.
  The bounded search evaluated 100 deterministic candidates across 10 input
  families and found one qualified witness; its exact report is
  `tests/fixtures/outputs/av1_search/coverage_vertical8x16_chroma_vertical_campaign_01.json`
  (SHA-256
  `132b265231fba4adf836ae3901fbf967d1cccac08185be8a60a3bf1a22ab53a8`).
  This closes only the origin Vertical8x16/chroma-Vertical plus following
  Vertical8x16/chroma-Diagonal113 class; broader AV1 partitions, chroma modes,
  sample depths, and AVF-STILL-001 remain partial.
- The newest bounded Paeth proof is the three-fixture
  `coverage_vertical8x16_chroma_paeth_01.avif` through `_03.avif` set. Each is
  a deterministic 16x16 8-bit 4:2:0 vertical split with origin UV mode 0
  (DC), following UV mode 12 (Paeth), one TX4x8 U/V block per leaf, and
  following ADST-ADST chroma. The 100-candidate input-only campaign qualified
  exactly 3 non-palette cases and recorded every rejection in
  `tests/fixtures/outputs/av1_search/coverage_vertical8x16_chroma_paeth_campaign_01.json`
  (SHA-256
  `4b85de67e8e8b25e01959ac9eb40a50906fe2e930d60a2a333f9272153d8539d`).
  Their pinned traces contain 276, 426, and 323 entropy operations; safe Rust
  matches exact partition, entropy, reconstructed Y/U/V, and Pillow RGB
  evidence. The production fix reconstructs the split origin luma with its
  prepared missing-edge DC values and uses one-sided DC for the internal child
  when no external left edge exists. The fixture, encoded-item, and RGB
  SHA-256 triples are respectively
  `880fa280f92839b65e46a15f81a72fcf8ff5ffb7bd16820d42b303fe1ea1a587`,
  `0fa07a83890b29f0a1f7d7ff239d50a2252d8d19c9e0c2e04f57979273f381fa`,
  `0a05b452b8f1d623db4a663260696241fb183938c8718f7bc4eb1bc5d019914b`;
  `5c4ce0eb3a7679b32619ca39277433ca7d85b8dfea04f6ab08946bd61c519297`,
  `bdf087a8d7a443029b663341abf2dd2cd1876be81ec063a6ec25b83d81c91e92`,
  `9edeaf44a0e8ef22777109c1228a491ea1d879d9bb75051d2c5200675e20c9ca`;
  and
  `13fd1d5aff12ff7157f6cb114653c5fedb4085f247af008c9ae8557e7f0f088c`,
  `f2f671620dab4ce9ba09c54218d0317dcbd7617e0239a01974c7c718101d1a0f`,
  `bdb2eefd28dbe8a00d21d18a45cfed874e635ea82fa138dcef67247bc84400fb`.
  This closes only the following Vertical8x16/chroma-Paeth/ADST-ADST class;
  broader AV1 partitions, predictors, transforms, and AVF-STILL-001 remain
  partial.
- The newest bounded full-sampling proof is the three-fixture
  `coverage_i444_square16_cfl_01.avif` through `_03.avif` set. Each is a
  deterministic 16x16 8-bit lossy 4:4:4 origin `Square16` leaf with luma DC,
  coded CFL UV mode 13, nonzero U/V alpha, one unsplit TX16x16 luma block, and
  two nonempty DCT-DCT TX16x16 chroma blocks. The 100-candidate, 10-family
  input-only campaign qualified 11 candidates and promoted exactly 3; its
  durable report is
  `tests/fixtures/outputs/av1_search/coverage_square16_cfl_campaign_01.json`
  (SHA-256
  `6aa456f3ee396f12526e67b9b3c9f7a1eea513cd4025341a48463213362f31a1`).
  Their pinned traces contain 419, 229, and 388 entropy operations, and safe
  Rust matches every entropy record, reconstructed Y/U/V plane, and Pillow
  RGB8 result. The root partition range is 62320 for all three. The fixture,
  encoded-item, and Pillow RGB SHA-256 triples are respectively
  `7b6d33f6ca51ca5ce5f69fcd4e1960d1d1b20d52aa4d0b954f555d6e8d47dc6d`,
  `16826aa3cc1b551ab2490ec931aabfdd2f8ac69812989167d124de8cc413c718`,
  `937289169b35c042aa7000bcac5896cc781979f96867c872176a19cd08763d20`;
  `496d2b4edf3ed6f4d9882087b047ac5d5e3e979f1762486762c352ef4d3da8e8`,
  `def47bf3486eda5705dbefa68b39b6e5dd97e30f9b1397be61b1201ca0774a82`,
  `c5672465e10df70e92f05c07e8ad290410ff778f748c70abd564c59766ec5b44`;
  and
  `475f6ce83fd295a52e59d97cb9504cf6309371b9fe74cd3306d64964875b3663`,
  `ba9e9fb7e3ec60e991e9f4fb9db5a5482afb01985f79999b30b3d1d8853b4a70`,
  `3b0bdcbaa2f2b1495939a79b77c4ec273ecc5cb9cc5770ca2fe6947b86763128`.
  The production path is pure safe Rust: it adds the verified qcat-two
  16x16 chroma EOB CDF, matrix-2 U/V dequantization table, and full-resolution
  CFL reconstruction. This closes only the origin Square16/I444/CFL/DCT class;
  broader AV1 partitions, transforms, chroma modes, sample depths, sequences,
  encoding, and AVF-STILL-001 remain partial.
- Two disjoint, input-only Diagonal67 campaigns are now recorded as explicit
  no-hit evidence. `scripts/explore_avif_chroma_diagonal67.py` evaluated 100
  deterministic candidates and found zero coded UV mode-8 cases; its report is
  `tests/fixtures/outputs/av1_search/coverage_square8_chroma_diagonal67_campaign_01.json`
  (SHA-256
  `1a54c0803c30443bd7ca2fd24a70be2e146c1235b878c19bd3ef0c5b8f66a977`). The
  separately reviewed chroma-biased campaign
  `scripts/explore_avif_chroma_diagonal67_biased.py` also evaluated 100 and
  found zero; its report is
  `tests/fixtures/outputs/av1_search/coverage_square8_chroma_diagonal67_campaign_02.json`
  (SHA-256
  `fb14d289c4e0f4d200c673687228a33f1f682eb235605005dc8f632f1dab4af7`). Both
  kept the 16×8 4:2:0 geometry, pinned Pillow/libavif/libaom/dav1d versions,
  unchanged strict mode/transform/AC predicates, and recorded every rejection.
  They do not prove the adjacent horizontal Diagonal67 class unreachable; no
  speculative edit was made for that class, which remains planned.
- A separate input-only campaign found a reachable vertical Diagonal67 class:
  `scripts/explore_avif_chroma_diagonal67_vertical.py` evaluated 100 candidates
  across 10 families, qualified 59, and promoted `D67V-F01-N00` (seed 12000).
  The fixture is `coverage_square8_chroma_diagonal67_vertical_01.avif`, an
  8×16 4:2:0 clipped vertical split with a following bottom `Square8` leaf,
  coded UV mode 8/angle symbol 3 (resolved angle 180), ADST-DCT TX4×4 U/V,
  split TX4×4 luma, and non-empty residuals. Its pinned dav1d trace has 118
  entropy operations and partition ranges `46608/54426/48340`; safe Rust
  matches exact partition, entropy, Y/U/V planes, and Pillow RGB bytes. The
  fixture, encoded-item, Pillow RGB, Y/U/V, and trace SHA-256 values are
  `7251e37d120b6cd170d0f2de705b2e56cccda3dfbd3ea4384369132bd0ea0f3f`,
  `90e351ddef37743cbde928804b30965b9c3099fb32c71f989d5dfdb971146ec8`,
  `2c5534101754f03cecccf894872055062fba481fd0886fb68eb853a55b2cf2ae`,
  `8686c05fa88a9d164fc4be87227e3f09189aa1c7ca4016f2e9a8a9cb5ed7b6dc`,
  `1c2902255e36f70fb4d51da60bbd81470d8851d678afb49324bc581ec5d65df0`,
  `5183ad5bc6643e34ce0794e3a19c3c11e28cf7a6faa3799e0719fe22eb2f795e`, and
  `1b5c7e5dda87d07e4b4cdd37fe99ce3b33d56368a05026deb74c124e308589ba`.
  The production correction uses the explicit vertical top-right extension
  availability and the shared eight-sample Z1 edge/filter predicate. This
  closes only this following-vertical Square8/chroma-Diagonal67 class; broader
  Diagonal67, AV1, and AVF-STILL-001 remain partial.
- The next reachable luma Diagonal67 class is
  `coverage_square8_luma_diagonal67_vertical_01.avif`: an 8×16 visible 8-bit
  4:2:0 frame with a clipped 16×16 split root, two vertically stacked Square8
  leaves, and a bottom luma mode-8/angle-symbol-3 (resolved 67°) unsplit
  TX8×8 DCT-DCT block with EOB 2, a decoded dequantized AC coefficient
  `dq[1] = -77`, and skipped TX4×4 U/V. Its input-only
  100-candidate/10-family campaign qualified 8 and promoted `D67V-F06-N01`
  (seed 12051), with `repository_rust_invoked=false`; the durable report is
  `tests/fixtures/outputs/av1_search/coverage_vertical_square8_luma_diagonal67_campaign_01.json`
  (SHA-256
  `03f31a3a96d208daa431d5759441f61cc4fd876cc59d0018b1c68277238ec613`).
  The pinned trace has 67 entropy operations and partition ranges
  `46608/54426/37798`; safe Rust matches exact partition, trace, Y/U/V planes,
  and Pillow RGB bytes. The production correction is gated by semantic
  no-left/top-only availability, zero-delta resolved 67°, unsplit TX8×8
  DCT-DCT, and the actual eight-sample top edge; transform-split and other
  Diagonal67 contexts remain on their prior path. The full active AVIF matrix
  passes 304/304 and the independent reconstruction selector passes 1/1.
  This closes only this bounded luma class; broader angle deltas, split/depth/
  transform variants, other availability contexts, chroma Diagonal67, and
  AVF-STILL-001 remain partial.
  The managed incremental Coverage MCP run
  `f5d16417-a1d5-4947-8c38-0631cf01388b` passed in 75,327 ms at exact
  implementation commit `c69c882cfe45d6a1e534e70ebc5786d687908c15` and
  ingested snapshot `e5252c8c-b8b0-4584-82a8-891dddef1dca` against explicit
  baseline `7665cda3-f4a7-4568-b871-a9d34afaa92c`. The standalone incremental
  review is supported selected-subset evidence: +658/+17/+1/+5,289 covered
  line/branch/function/region identities, denominator changes
  +3,664/+388/+41/+10,273, 7,381 newly covered line identities, 39,379
  unobserved baseline observations, and zero regressions. The merge is
  conservative and named-test attribution is unavailable; this is not a
  complete four-metric release measurement.
  The follow-up symbol-2/64° campaign is retained as bounded no-hit evidence
  in `tests/fixtures/outputs/av1_search/coverage_vertical_square8_luma_diagonal67_angle_symbol2_campaign_01.json`
  (SHA-256
  `607c7fcd591b7298b9eeafe28cd2724468d2fc627dab55fa471748eb295e242f`):
  100 candidates across 10 families emitted 37 symbol-2 cases and 26 unsplit
  TX8×8 cases, but zero had a nonzero decoded AC coefficient. It does not
  prove the nonzero-delta class unreachable and causes no decoder admission.
- The split-transform fallback is now proven by
  `coverage_square8_luma_diagonal67_vertical_split_tx4x4_01.avif`: an 8×16
  visible 8-bit 4:2:0 frame with the same clipped 16×16 vertical split and
  following Square8 geometry. Its bottom leaf uses luma mode 8/angle symbol 3
  (resolved 67°, zero delta), four split TX4×4 DCT-DCT luma payloads with EOB
  values `15/-1/-1/-1`, and skipped U/V. The input-only campaign evaluated
  100 candidates across 10 families, qualified 40, and promoted
  `D67V-F02-N00` (seed 12010) without invoking repository Rust. Its pinned
  dav1d trace has 92 entropy operations and partition ranges
  `46608/54426/35039`; the nonzero luma residual is decoded dequantized AC
  scan index 15 = 160. Fixture, encoded-item, Pillow RGB, Y/U/V, decoded-YUV,
  and trace SHA-256 values are
  `97d00ee7b26556ea9c1e68e11c435727c5224373074cc183ff9c4a7c688809ee`,
  `fff30cbc7e67c22a881670f397405f1a3849d0522b7130be48a556c1032eaaaf`,
  `eb2bebe4dbb452c932c1334ec8420fd5b3ca8589641254938dc52d7d41365a2a`,
  `a9d2d3e0149e5a24f358400a9e3f8d2dbe04087da393675dbe61928179ab11d2`,
  `bd75a82b9957d6d043076dea52262635042693f1fe23bcadadaecc908e1e5cc6`,
  `bd75a82b9957d6d043076dea52262635042693f1fe23bcadadaecc908e1e5cc6`,
  `70e5d44d47f62641b00dff5f10523af69a7b66704d67a0aedcc521f8e3c6fb0f`, and
  `8584b9626db9c39301c610e704682190990ee7aa508e2993429c0f8c26298a15`.
  Safe Rust now passes the actual split helper's intra-edge-filter state and
  disables filtered upsampling only for the proven top-only/no-left/no-
  extension, zero-delta/67°/DCT-DCT semantic class; other split and
  Diagonal67 contexts retain their prior path. The selected reconstruction
  proof passes 1/1 alongside the earlier unsplit witness; the full active AVIF
  matrix passes 305/305 with 5 explicit planned skips. This closes only this
  split-TX4×4 class; broader split/depth/transform, availability, sequence,
  encoding, and AVF-STILL-001 work remains partial.
  The managed incremental Coverage MCP run
  `870a1027-6885-4154-a8d1-698420520772` passed in 76,798 ms at exact
  implementation commit
  `175772d882cdb4f2bd1c8e7228d577d0f49bb7d3` and ingested snapshot
  `33c346b6-73cf-4217-955b-363f706fb57e` against explicit baseline
  `7665cda3-f4a7-4568-b871-a9d34afaa92c`. Its standalone incremental review
  is supported selected-subset evidence: +670/+17/+1/+5,289 covered
  line/branch/function/region identities, denominator changes
  +3,639/+388/+41/+10,275, 7,393 newly covered line identities, 39,306
  unobserved baseline observations, and zero regressions. Merge is conservative
  and named-test attribution is unavailable; the snapshot metadata retains
  commit `3272b3ef49a87c2947c08b46596b442195c6a8db` as a provenance caveat.
- The latest bounded angle-delta proof is
  `coverage_square8_luma_diagonal67_vertical_split_tx4x4_angle70_01.avif`: an
  8×16 visible 8-bit 4:2:0 frame with the same clipped 16×16 vertical split
  and following Square8 geometry. The bottom leaf uses luma mode 8 Diagonal67
  at angle symbol 4 (signed delta 1, resolved 70°), four split TX4×4 DCT-DCT
  payloads with EOB values `4/-1/-1/-1`, one decoded dequantized AC at scan
  index 5 = 84, and skipped U/V. The input-only split campaign evaluated 100
  candidates across 10 families, qualified 5, and promoted `D67V-F05-N01`
  (seed 12041) without invoking repository Rust. Its pinned dav1d trace has
  78 entropy operations and partition ranges `46608/54426/34793`; fixture,
  encoded-item, Pillow RGB, decoded Y/U/V/YUV, and trace hashes are recorded
  in `roadmap.json`. The separate unsplit symbol-4 campaign qualified zero
  cases, so it remains no-hit evidence and does not admit unsplit angle 70.
  Safe Rust matches the exact raw-edge predictor, partition, trace, planes,
  and Pillow RGB output after an exact semantic gate for top-only/no-left/
  no-extension, split-TX4×4, DCT-DCT, resolved-70 behavior; the prior 67°
  gate remains separate. The selected reconstruction proof passes 1/1 and
  the full coverage-configured matrix passes 44/44, including 306/306 active
  AVIF rows with 5 explicit planned skips. Managed incremental Coverage MCP
  run `9aec028b-b175-410b-bef9-3ad0ca87c070` passed in 88,715 ms at exact
  implementation commit `f027d3366db0ed4b1fa085011561a733916acedc` and
  ingested snapshot `c82ba70d-f026-40c2-aaed-052c7ebb140c` against explicit
  baseline `7665cda3-f4a7-4568-b871-a9d34afaa92c`. Standalone incremental
  review is supported selected-subset evidence: +665/+17/+1/+5,289 covered
  line/branch/function/region identities, denominator changes
  +3,676/+414/+41/+10,301, 7,388 newly covered line identities, 39,326
  unobserved baseline observations, and zero regressions. The merge is
  conservative and named-test attribution is unavailable; this is not a
  complete four-metric release measurement. This closes only the bounded
  split angle-70 class; broader angles, availability, transforms, AV1 still
  states, AVIF encoding, and AVF-STILL-001 remain partial.
- The newest bounded following-leaf proof is
  `coverage_vertical8x16_chroma_horizontal_01.avif`, a deterministic 16x16
  8-bit 4:2:0 vertical split with origin UV mode 0 (DC) and following UV mode
  2 (Horizontal). Both leaves use one R4x8 U/V pair; the origin is DCT-DCT,
  the following leaf is DCT-ADST, both have non-empty AC, and the following
  UV angle symbol is recorded as `3` (delta `0`, absolute angle `180`). The
  100-case/10-family input-only campaign qualified exactly two cases in this
  same zero-delta zone and promoted one. Its pinned trace has 149 entropy
  operations and root range `57408`; safe Rust matches exact partition,
  entropy, Y/U/V planes, and Pillow RGB bytes. The durable report is
  `tests/fixtures/outputs/av1_search/coverage_vertical8x16_chroma_horizontal_campaign_01.json`
  (SHA-256
  `3556712a1a4f2a9a79fb48072dd1108582e4220f0aafe935faab2849d287463a`).
  This closes only the exact-horizontal 4:2:0 class; nonzero-angle
  Horizontal zones and broader AVF-STILL-001 remain open.
- The newest bounded luma proof is
  `coverage_square8_luma_diagonal_down_right_01.avif`, a deterministic 16x8
  8-bit 4:2:0 split-root frame with two Square8 leaves. The right leaf selects
  luma mode 4 (`DiagonalDownRight`) with angle symbol `3` (delta `0`, absolute
  `135` degrees); both leaves use four TX4x4 luma payloads, both chroma leaves
  use DCT-DCT TX4x4 U/V, and the right luma/U/V residuals are non-empty. Its
  pinned trace has 259 entropy operations and partition ranges
  `37392/43662/34793`; safe Rust matches exact partition, entropy, Y/U/V, and
  Pillow RGB evidence. The input-only 100-case campaign qualified exactly
  3/100 candidates and promoted `LDR-F01-N03`; this closes only this bounded
  mode-4/135-degree Square8 class.
- The newest bounded luma-predictor proof is the three-fixture
  `coverage_square8_luma_smooth_01.avif`,
  `coverage_square8_luma_smooth_vertical_01.avif`, and
  `coverage_square8_luma_smooth_horizontal_01.avif` set. Each is a deterministic
  16x8 8-bit 4:2:0 horizontal split with two visible Square8 leaves, DC/skipped
  chroma, and a varied reconstructed left edge. The right leaf covers luma
  Smooth mode 9 with an unsplit TX8x8 (159 entropy operations), SmoothVertical
  mode 10 with an unsplit TX8x8 (188 operations), and SmoothHorizontal mode 11
  with four raster TX4x4 transforms (205 operations). The 100-case/10-family
  input-only campaign qualified 73 candidates and promoted exactly one witness
  per mode: `LS-F01-N00`, `LS-F02-N02`, and `LS-F07-N00`. Safe Rust now matches
  the exact pinned partition, entropy, Y/U/V, and Pillow RGB evidence; the
  SmoothHorizontal fix uses the per-child no-top edge prepared by the AV1
  reference. This closes only these bounded mode-9/10/11 Square8 classes.
- The newest bounded following-leaf luma proof is
  `coverage_square8_luma_diagonal45_01.avif`, a deterministic 16x8 8-bit
  4:2:0 horizontal split with two Square8 leaves. The right leaf selects luma
  Diagonal45 mode 3 with angle symbol `3` (delta `0`, absolute `45` degrees),
  DC/skipped chroma, and an unsplit TX8x8 luma transform; the preceding
  top-right edge is `[120; 8]`. Its pinned trace has 47 entropy operations and
  partition ranges `37392/43662/62592`; safe Rust matches exact partition,
  entropy, Y/U/V, and Pillow RGB evidence. The 100-case/10-family input-only
  campaign qualified 60 candidates, and its durable report has SHA-256
  `2ce8523741d3e8a3eaf723bd85db584803445afde8967db668473a4d89b84aad` with
  deterministic double-encode/double-trace/double-YUV/double-RGB checks. The
  production fix uses the preceding luma top-right edge for this proven
  unsplit Diagonal45 path; Diagonal67 remains separate planned work. This
  closes only this following-leaf Diagonal45 class, not general AV1/AVIF.
- The newest bounded chroma-angle proof is
  `coverage_square8_chroma_diagonal45_angle51_01.avif`, a deterministic 16x8
  8-bit 4:2:0 horizontal split with two visible Square8 leaves. The right
  leaf selects nominal chroma Diagonal45 (coded UV mode 3), angle symbol `5`,
  delta `+2`, and resolved angle `51` degrees; both chroma leaves use TX4x4
  DCT-DCT with non-empty residuals, while all luma leaves use DC and the
  right leaf has top unavailable/left available context. The 100-case,
  10-family input-only campaign qualified 5 candidates; all five were this
  symbol-5/+2/51-degree class and none was the separate symbol-3/delta-0/
  45-degree class. Its durable report is
  `tests/fixtures/outputs/av1_search/coverage_square8_chroma_diagonal45_angle51_campaign_01.json`
  with SHA-256
  `e8599c33aff2b5abc6baff55dc4cf571c1841d7fe683413b5c99e12b4f158e65`.
  The promoted witness has 119 entropy operations and partition ranges
  `37392/43662/34871`; safe Rust matches the exact partition, entropy trace,
  Y/U/V planes, and Pillow RGB bytes. This closes only the reachable
  right-hand Square8 chroma Diagonal45 angle-51 class and provides bounded
  evidence for the nominal `ChromaPredictor::Diagonal45` arm; it does not
  close general angular chroma, AV1, or AVF-STILL-001 support.
- The newest bounded chroma proof is
  `coverage_square16_chroma_smooth_horizontal_01.avif`, a deterministic
  32x16 8-bit 4:2:0 clipped root split with origin/following Square16 leaves
  at x=0/x=4. The following leaf selects chroma SmoothHorizontal mode 11
  with DCT-ADST TX8x8 U/V transforms and non-empty AC; both luma leaves use
  unsplit TX16x16 DCT_DCT transforms. Its pinned dav1d trace has 414 entropy
  operations and partition ranges `38416/36560/62182`; safe Rust matches the
  exact partition, entropy, reconstructed Y/U/V planes, and Pillow RGB bytes.
  The 100-case/10-family input-only campaign qualified 9 candidates and
  promoted `SF16-F06-N01`, with deterministic double-encode/double-trace/
  double-YUV/double-RGB checks. No production decoder edit was required for
  this already-correct safe-Rust path; this closes only the bounded following-
  Square16/chroma-SmoothHorizontal/DCT-ADST class.
- The newest bounded chroma proof is
  `coverage_square16_chroma_smooth_vertical_01.avif`, a deterministic 32x16
  8-bit 4:2:0 clipped root split with origin/following Square16 leaves at
  x=0/x=4. The following leaf selects chroma SmoothVertical mode 10 with
  ADST-DCT TX8x8 U/V transforms and non-empty AC; both luma leaves use unsplit
  TX16x16 DCT_DCT transforms. Its pinned dav1d trace has 239 entropy operations
  and partition ranges `38416/36560/44978`; safe Rust matches exact partition,
  entropy, reconstructed Y/U/V planes, and Pillow RGB bytes. The 100-case/
  10-family input-only campaign qualified 9 candidates and promoted
  `SV16-F06-N03` (seed 9053), with deterministic double-encode/double-item/
  double-trace/double-YUV/double-RGB checks and no repository Rust invocation.
  This closes only the bounded following-Square16/chroma-SmoothVertical/
  ADST-DCT class; broader AV1 and AVF-STILL-001 remain partial.
- The newest bounded chroma proof is
  `coverage_square16_chroma_smooth_01.avif`, a deterministic 32x16 8-bit
  4:2:0 clipped root split with origin/following Square16 leaves at x=0/x=4.
  The following leaf selects chroma Smooth mode 9 with ADST-ADST TX8x8 U/V
  transforms and non-empty AC; both luma leaves use unsplit TX16x16 DCT_DCT
  transforms. Its pinned dav1d trace has 359 entropy operations and partition
  ranges `38416/36560/62182`; safe Rust matches the exact partition, entropy,
  reconstructed Y/U/V planes, and Pillow RGB bytes. The 100-case/10-family
  input-only campaign qualified 10 candidates and promoted `SS16-F06-N01`
  (seed 10051), with deterministic double-encode/double-item/double-trace/
  double-YUV/double-RGB checks and no repository Rust invocation. This closes
  only the bounded following-Square16/chroma-Smooth/ADST-ADST class; broader
  AV1 and AVF-STILL-001 remain partial.
- Current local Rust contracts: 44/44 matrix tests and 66/66 feature-gate
  tests pass with all features enabled; the dedicated animation-loop contract
  suite is 4/4.
- The current loop-contract slice closes `API-050` and `GIF-013`. Public
  `AnimationLoop` distinguishes omitted metadata, finite total plays, infinite
  repetition, and source-only unknown semantics. GIF, APNG, and WebP decoders
  normalize their format-specific fields explicitly; GIF and WebP encoders
  check representable target fields, while TIFF and unknown loop semantics are
  rejected instead of being silently discarded.
- The incremental matrix-row selector is implemented with the reserved
  `--skip` transport prefix `__image_slash_star_matrix_row_selector__=` and
  exact qualified keys `decode:<format>:<row-id>` or
  `encode:<format>:<row-id>`. With no selector, the full matrix behavior is
  preserved; selected runs execute only selected active rows, report selected
  planned rows as `planned-not-executed`, and never count planned rows as
  pass/fail. Malformed, empty, unknown, duplicate, and planned-only
  selections fail, and `test_coverage_matrix` is the central selected-run
  dispatcher. Selected coverage is subset evidence, not a release or
  full-coverage gate.
- Selected-subset evidence only: managed Coverage MCP run
  `3a0d2ca9-0ff7-4d35-b712-1aa44a6f5403` selected two rows, passed in 52.8s, and
  ingested snapshot `82c70c05-eea0-4431-b8c6-80b052916283` against baseline
  `cbd4fcaa-0640-4dda-be05-3e368b690955`. It recorded zero aggregate gain;
  because this was a selected subset, it makes no aggregate no-regression
  claim and is not a full-coverage or release-gate run.
- The AV1 reconstruction test now has a separate exact fixture selector for
  fast incremental campaigns: repeat `--skip` with the reserved prefix
  `__image_slash_star_av1_fixture_selector__=` and a bare, case-sensitive
  `.avif` basename from `av1_reconstruction.json`. No selector still runs all
  245 reconstruction cases; a selected run reads and executes only the
  requested active fixtures, reports the exact set, and rejects malformed,
  empty, duplicate, unknown, planned, path, glob, ordinary-skip-mixed, and
  matrix-selector-mixed arguments. This is test-system filtering only; it does
  not widen AV1 support or close `AVF-STILL-001`.
- The managed selected-fixture run
  `76845131-98f3-4295-841f-61173b796041` passed in 30,712 ms at exact
  implementation commit `6de450703ab6ffef68b9bfa405800cfd90e76ccb` and
  ingested snapshot `f4d5dbce-b9cb-43ca-92e7-3e1e6f11c15e` against baseline
  `7665cda3-f4a7-4568-b871-a9d34afaa92c`. It selected exactly
  `coverage_square16_chroma_smooth_vertical_01.avif` and
  `coverage_r16x32_following_filter_intra_split_mode0_01.avif`; the additive
  review recorded zero aggregate covered-metric gain, no regressions, and
  denominator changes of +2,920 lines, +76 branches, +17 functions, and
  +8,962 regions. The selected-snapshot projection reports 1,252 newly
  covered line identities, but selected-subset evidence is not a complete
  release measurement.
- The newest managed selected-fixture run
  `44360fd2-4d9c-4ce0-845b-deef0d7c0ef1` passed in 31,912 ms at exact
  implementation commit `72759602317c50016a6bf38fc80ee06bb1de9afe` and
  ingested snapshot `5b0a5d63-dfe0-447a-9e3e-ffe7a97a08cb` against baseline
  `7665cda3-f4a7-4568-b871-a9d34afaa92c`. It selected exactly
  `coverage_h16x4_filter_intra_tx8x4_split_01.avif`; the log reports 1 passed,
  0 failed, and 43 filtered out. The additive baseline-union review reports
  +8 covered lines, +7 branches, +0 functions, +281 regions, and no reported
  regressions; the selected projection reports 1,474 newly covered line
  identities. The snapshot metadata still names
  `3272b3ef49a87c2947c08b46596b442195c6a8db`, so the durable run commit is the
  authoritative implementation provenance. This remains bounded selected-
  subset evidence, not a complete four-metric release measurement.
  The matching matrix-row run `a740718c-1912-4280-8cff-4969d1acf19e`
  passed in 30,162 ms and ingested snapshot
  `ff859be1-e7ab-4f82-96c9-84044d1f24cc`; its additive review reports +8
  covered lines, +8 branches, +0 functions, +281 regions, and no reported
  regressions. Its exact run record is also bound to implementation commit
  `72759602317c50016a6bf38fc80ee06bb1de9afe`.
- The bounded rectangular proof pairs
  `coverage_h16x4_filter_intra_cdf14_false_01.avif` and
  `coverage_v4x16_filter_intra_cdf19_false_01.avif`: deterministic 16x16
  8-bit 4:2:0 quality-12/speed-0 frames with root `PARTITION_H4`/
  `PARTITION_V4`, four DC `Horizontal16x4`/`Vertical4x16` luma leaves, false
  filter-intra decisions from CDF rows 14/19, and rectangular DCT-DCT chroma
  transforms. Each pinned dav1d trace has 162 entropy operations; safe Rust
  matches exact partition, entropy, reconstructed Y/U/V, and Pillow RGB
  bytes. The H following leaf also exercises two-entry UV palette prediction,
  while the V following leaf reaches the transposed rectangular path. The
  input-only campaign explored 100 candidates across 10 families and
  qualified 5 candidates per orientation without invoking repository Rust.
  This closes only that bounded class; AVF-STILL-001 remains partial.
- The newest bounded proof is
  `coverage_square32_origin_tx16x16_split_01.avif`: a deterministic 32x32
  8-bit 4:2:0 quality-76/speed-0 origin `Square32` leaf with four TX16x16
  luma DCT children, DC prediction, non-empty luma residuals, and TX16x16 DCT
  chroma. Its pinned dav1d trace has 4,436 entropy operations; safe Rust
  matches exact partition, entropy, reconstructed Y/U/V, and Pillow RGB
  bytes. The input-only campaign explored 100 candidates across 10 families,
  qualified 5, promoted `S32-F01-N01` (seed 32001), and invoked no repository
  Rust. Fixture, encoded-item, and Pillow RGB SHA-256 values are
  `f4bf64e6de7a7265a1c5564324c812103135c043a05b7119ef4c97bf9892c987`,
  `e97269cfe2a869fa66c947e6165c712a313aaa301d621575fe646591e58023dd`, and
  `6f55403182b74ed6bb0f581ebb3e53b6857d0a1934c0650923feac0a0e52b88b`.
  This closes only the origin Square32 split luma/residual class; broader
  AV1, AVF-STILL-001, and AVIF encoding remain open.
- The newest bounded H_DCT/CFL incremental run
  `90d68af5-f06f-4392-b5e6-b2fda4a74c1c` passed in 38,688 ms at exact
  implementation commit `f05cdf26436e80f751b6f98646151db7f226cdc9` and
  ingested snapshot `9467e718-f36b-4910-bace-d04d89ebc5c8` against explicit
  baseline `7665cda3-f4a7-4568-b871-a9d34afaa92c`. Its exact reconstruction
  test passed 1/1 with 43 filtered-out tests. The additive baseline-union
  review reports +8 covered lines, +10 branches, +0 functions, and +938
  regions, with denominator changes of +4,121/+440/+46/+12,367; the selected
  projection reports 1,867 newly covered line identities, 75,809 baseline
  observations were not observed, and zero regressions were reported. Merge
  exactness is false and named-test attribution is unavailable; the snapshot
  projection retains metadata commit
  `3272b3ef49a87c2947c08b46596b442195c6a8db` while the durable run record is
  exact for `f05cdf26`. This is bounded selected-subset evidence, not a
  complete four-metric release measurement.
- The newest bounded full-resolution proof is
  `coverage_i444_full_chroma_top_left_paeth_01.avif`: a deterministic 32x32
  8-bit 4:4:4 frame whose target fourth top-left `Square8` leaf at pixel
  `(8,8)` has distinct top, left, and upper-left neighbors and coded chroma
  Paeth mode 12. The input-only campaign evaluated 100 candidates across 10
  families, qualified 1, and promoted `i444-tl-f01-n05` (seed 7005) without
  invoking repository Rust. Its pinned dav1d trace has 4,680 entropy
  operations; safe Rust matches exact partition, entropy, reconstructed Y/U/V
  planes, and Pillow RGB bytes. The campaign report is
  `tests/fixtures/outputs/av1_search/coverage_i444_full_chroma_top_left_paeth_campaign_01.json`
  (SHA-256
  `def1e9fd8f9246fdfdb43f6daf3d3be0232008f78ced1202079a83ab904d2e04`).
  Fixture, encoded-item, and Pillow RGB SHA-256 values are
  `695fd9288686eec0cfa8abb174eead2d745ac3155f755222e05cefd694695dd6`,
  `b7f0ad6aa050384a5f7bff33719026ec498e3b5440c3d6dff30a17fff24504f`, and
  `41fed0113dd24525e6c094748beb78a75b94f2825bacdf7dc5d009375f32dd89`.
  The production fix adds the verified UV matrix-2 tables, derives the
  full-resolution chroma DC-sign context, assembles exact horizontal edges,
  and applies one-sided AV1 DC availability to split luma/chroma. This closes
  only the bounded top-left-sensitive I444 chroma-Paeth/Square8 class; broader
  AV1 partitions, chroma modes, sample depths, sequences, encoding, and the
  four-metric 100% coverage gate remain open.
- The latest bounded proof is
  `coverage_square64_origin_tx32x32_split_01.avif`: a deterministic 64x64
  8-bit 4:2:0 quality-76/speed-0 origin `Square64` leaf with four TX32x32
  luma DCT children, DC prediction, non-empty luma residuals, and TX32x32 DCT
  chroma. Its pinned dav1d trace has 17,158 entropy operations; safe Rust
  matches exact partition, entropy, reconstructed Y/U/V, and Pillow RGB
  bytes. The input-only campaign explored 100 candidates across 10 families,
  qualified 60, promoted `S64-F01-N00` (seed 64000), and invoked no repository
  Rust. Fixture, encoded-item, Pillow RGB, Y/U/V, and trace SHA-256 values are
  recorded in `roadmap.json`; this closes only the origin Square64 split
  luma/residual class while broader AV1, AVF-STILL-001, and AVIF encoding
  remain open.
- The current evidence-only mode-2 proof is
  `coverage_vertical8x16_filter_intra_mode2_01.avif`: an 8x16 8-bit 4:2:0
  origin `Vertical8x16` leaf selecting `FILTER_PRED[13/2]`, one unsplit
  TX8x16 luma transform, and one TX4x8 U/V pair. The strict input-only search
  evaluated 100 deterministic candidates and qualified exactly 6; the
  selected seed is 107. Its pinned dav1d trace has 578 entropy operations,
  partition range 42232, and a complete 192-byte YUV output. Safe Rust matches
  the exact entropy records, partition, Y/U/V planes, and Pillow RGB8 bytes.
  The generic mode-2 production path was already implemented, so this closes
  only the origin Vertical8x16/mode-2 evidence class; broader mode-4 classes
  and broader AV1 still remain partial.

- The preceding bounded origin proof is
  `coverage_vertical8x16_filter_intra_mode3_01.avif`: an 8x16 8-bit 4:2:0
  origin `Vertical8x16` leaf selecting `FILTER_PRED[13/3]`, one unsplit
  TX8x16 luma transform with EOB 78, and TX4x8 U/V transforms with EOB 6.
  The input-only campaign evaluated 100 candidates across 10 families and
  qualified 4; it promoted `f10_mosaic_04` (seed 1005), whose non-empty luma
  and chroma residuals prevent a neutral-prediction false positive. Its pinned
  dav1d trace has 187 entropy operations and exact partition, transform-type,
  Y/U/V, and Pillow RGB8 evidence. The fixture, encoded-item, and Pillow RGB
  SHA-256 values are
  `091bac9643129816c6a0a1dddc94cba4965c1849acc2fc46175ce1a117ba0c17`,
  `cd7e03206d361c1f66428b6da304af0e1f4e56120c6bac0313bf5de44de28e61`, and
  `a900cd81f92250ea4b1057109066cb0d0ebbbcdb4d8568e4675e2816ff549777`.
  The generic safe-Rust mode-3 dispatcher was already implemented, so this
  slice adds exact evidence without claiming a production decoder edit; mode
  4, other contexts/topologies, and broader AV1 still remain partial.

- The preceding bounded origin implementation proof is
  `coverage_vertical8x16_filter_intra_mode4_tx4x4_grid_01.avif`: an 8x16,
  8-bit 4:2:0 `Vertical8x16` origin leaf selecting `FILTER_PRED[13/4]`,
  transform-luma mode 0, and a depth-two 2-column × 4-row grid of eight
  row-major TX4x4 luma DCT-DCT children with EOB values
  `2/2/14/15/14/4/2/2`. The two TX4x8 chroma payloads have EOB 0, proving
  their geometry and zero residuals only. An input-only 100-candidate,
  10-family campaign qualified exactly one deterministic double-encoded
  candidate (`f03_color_ramp_08`, seed 309) and never invoked repository Rust.
  Its pinned 361-operation dav1d trace, exact partition, reconstructed Y/U/V
  planes, and independent Pillow RGB8 bytes match the safe-Rust production
  path. Here transform-luma mode 0 names the residual transform/CDF context;
  it does not mean ordinary DC prediction was used—the predictor is
  `FILTER_PRED` mode 4. This closes only the proven origin mode-4 grid class;
  other modes,
  following-leaf contexts, transform grids, subsampling/depth/HDR/sequence
  cases, encoding, and the four-metric 100% coverage gate remain open.

  Managed Coverage MCP ran this exact fixture selector against complete
  baseline snapshot `7665cda3-f4a7-4568-b871-a9d34afaa92c`: run
  `6b78c91c-d417-4ad0-af44-96d3623e3f64` passed in 32,449 ms at exact
  implementation commit `520e38284a263a4e07b392bf02f4b6322e6a3e31` and
  ingested snapshot `75d6b159-3727-4d5b-b7f4-a4252124a171`. It selected
  exactly `coverage_vertical8x16_filter_intra_mode4_tx4x4_grid_01.avif` and
  passed 1/1 cases. The additive baseline-union review reports +8 covered
  lines, +7 branches, +0 functions, and +281 regions; denominator changes are
  +3,459 lines, +162 branches, +35 functions, and +9,984 regions. The
  selected projection reports 385 newly covered line identities, with zero
  reported regressions limited to this selected subset. The compact snapshot
  projection retains metadata commit `3272b3ef49a87c2947c08b46596b442195c6a8db`;
  the exact run record is authoritative for implementation provenance. This
  is bounded incremental evidence, not a complete release measurement.

  The latest bounded following-leaf proof is
  `coverage_vertical8x16_following_filter_intra_mode2_01.avif`: a 16x32,
  8-bit 4:2:0 split-root frame with four coded-order 8x16 `Vertical8x16`
  leaves. The lower-left following leaf selects `FILTER_PRED[13/2]` with a
  `TX8x16` luma transform; the lower-right selects `FILTER_PRED[13/0]` and
  the pinned `TX[1]` two-payload luma state. The upper-right `0/0` trace is
  ordinary DC, not FILTER_PRED. All four U/V payloads use TX4x8 DCT-DCT with
  EOB 0. Safe Rust matches the exact three partition records, all 1,023
  entropy operations, reconstructed Y/U/V planes, and the independent
  1,536-byte Pillow RGB8 reference. The fixture SHA-256 is
  `252f1ef0ac2b5af88a90d8f6c6952186ea968db0350af5e7f5c19a1465581ec2`, the
  encoded-item SHA-256 is
  `49620e57e1d5749c7e6ee2c76d8cf14e29709922db4c249a3da50cc8b2940bfb`, and
  the RGB SHA-256 is
  `403dfa0053c7a79267a72b0c4b8aad0462efb45e9baac12dd488468b3d3d924b`.
  The reconstructed plane SHA-256 values are Y
  `369245ee4125261a45aab212133114ed8e44b7d9707011d23f5e1e65c5a5e854`, U
  `d8eeb09b5d4d74c33d5db255677a84ef0dff4e019d8c256805097a7e91dd20cb`, and V
  `9fe1f341a82af9fbd72d403bfd860c658b0ee7ea3caa7be0d48ea4cc5567e59e`.
  The maintained deterministic generator uses seed 1406 and the input-only
  oracle never invokes repository Rust; no separate campaign identifier is
  claimed for this direct topology witness. This closes only the proven
  following mode-2/mode-0 class; other following modes/geometries and
  `AVF-STILL-001` remain partial.

  Managed Coverage MCP ran the exact fixture selector against complete
  baseline snapshot `7665cda3-f4a7-4568-b871-a9d34afaa92c`: run
  `00dc6cfa-f1a3-45fa-8038-bdc494a4db4b` passed in 42,067 ms at exact
  implementation commit `9061135585461cb9309f2760e525a230cec68d22` and
  ingested snapshot `fd53e0b9-0465-49cc-b44f-a2233eea41f1`. The log reports
  1 passed, 0 failed, and 43 filtered out. The additive baseline-union review
  reports +8 covered lines, +7 branches, +0 functions, and +281 regions;
  denominator changes are +3,553 lines, +254 branches, +38 functions, and
  +10,093 regions. The selected projection reports 562 newly covered line
  identities, 75,363 baseline hits were not observed, and zero regressions
  were reported. The merge is conservative rather than exact and named-test
  attribution is unavailable; the compact snapshot retains metadata commit
  `3272b3ef49a87c2947c08b46596b442195c6a8db` while the durable run record is
  authoritative for implementation provenance. This is bounded selected-
  subset evidence, not a complete four-metric release measurement or global
  regression claim.

  Managed Coverage MCP ran the exact fixture selector against baseline
  snapshot `7665cda3-f4a7-4568-b871-a9d34afaa92c`: run
  `096b83e7-ccab-4a2b-93b4-a39d051817cf` passed in 36,099 ms and ingested
  snapshot `f1f52e16-9275-43c0-9e28-7e38bea081e8` at implementation commit
  `430c5beb39757ce570c2f07ea5fb2e044a580205`. The additive baseline-union
  review reports +8 covered lines, +7 branches, +0 functions, and +281
  regions, with denominator changes of +3,211, +126, +26, and +9,543 and zero
  reported regressions. The selected projection is limited because unselected
  baseline hits are not observed; this is bounded subset evidence, not a
  complete release measurement.

An earlier managed Coverage MCP run was
`a90eb75b-d62d-4c80-a75f-a753990fdea6`, bound to implementation commit
`f92b3d6896e7e03a46396d53ad44dba96866de0e`, passed in 149,159 ms, and ingested
snapshot `bd67106e-6a28-41c2-9947-5c278e929f83`. It measures 98,968/109,130
lines (90.6882%), 12,568/13,930 branches (90.2225%), 5,045/5,744 functions
(87.8308%), and 148,099/165,103 regions (89.7010%). The explicit
`coverage_review` comparison against baseline snapshot
`6fa9ab92-2f3e-4551-b107-6710dda14e3d` reports direct covered/total deltas of
+5/+0 lines, +0/+0 branches, +1/+0 functions, and +5/+0 regions. Newly covered
source lines are `src/codecs/avif/av1/block.rs:27618-27621`; named test
attribution is unavailable, so the aggregate measurement remains supported.
The four-metric 100% gate and the broader AVIF roadmap remain open.

The prior managed Coverage MCP run was `9212d568-8e60-4701-9cfe-089f74cf481b`,
bound to implementation commit `a8af95eb129934ee0d3becfc91b0b7a98f2ea316`,
passed in 148,795 ms, and ingested snapshot
`64548143-b655-4ae7-87d3-4578e9ee4de4`. Its complete all-feature LLVM result was
98,968/109,130 lines (90.6882%), 12,568/13,930 branches (90.2225%),
5,045/5,744 functions (87.8308%), and 148,099/165,103 regions (89.7010%).

An earlier managed Coverage MCP baseline is
`8740ab80-5d32-4bcf-8026-e5df72346f0e`, bound to implementation commit
`d85561351dd1e779aff5eedbf5e562eebb7201e9`, passed in 151,052 ms, and ingested
snapshot `02ea7870-faee-4687-94e4-4af30f443dbb`. Its result is retained as the
explicit baseline for the current slice.

The immediately previous current-tree managed Coverage MCP run was
`1220905e-e31a-4c4f-876d-bc47f26bbbc1`, bound to implementation checkout
`1bfe20f465435fd474def91770a06a1289d71544`; it passed in 146,803 ms and
ingested snapshot `b5a94b2e-fdf6-4838-beb0-f970a616b5ad`. Its result is retained
as historical context for the current measurement.

The previously latest managed Coverage MCP run is
`1c2bb88e-0523-4641-ad69-5482124055eb`. Its run record is bound to exact
implementation commit `24a80016f7e89566494e9fea22f7fb999720e383`; it passed in
162,453 ms and ingested snapshot `c78cac84-591f-4091-a0eb-8ffb44f8321c`.
Its complete all-feature LLVM snapshot records 100,057/109,749 lines (91.1689%),
12,809/14,188 branches (90.2805%), 5,109/5,777 functions (88.4369%), and
149,698/165,957 regions (90.2029%), leaving 9,692 lines, 1,379 branches, 668
functions, and 16,259 regions uncovered. The run record is exact for the
implementation commit, while the compact snapshot projection retains commit
metadata `3272b3ef49a87c2947c08b46596b442195c6a8db`; this provenance caveat is
explicit. Coverage review against prior complete snapshot
`283cc397-39ec-4dc5-98ec-21cc4720cdf8` records -21 covered lines with +101 total
lines, +20 covered branches with +36 total branches, -3 covered functions with
+7 total functions, and -60 covered regions with +125 total regions; this is an
instrumentation/denominator comparison, not a claim of a release-gate
improvement. Named test attribution is unavailable. This exact run does not
close AVF-STILL-001, AVIF encoding, or the four-metric 100% gate.

The previous managed Coverage MCP run was
`3b4748a2-f5bf-43d2-9f56-1067f4210257`, whose run record is bound to exact
implementation checkout `de664cc6dc8d12f6f7f5fe3b73c01faef3709d63`. It passed
in 145,657 ms and ingested snapshot
`1ac75559-0323-47ab-81bd-d9c6dac620fb`. Its current all-feature LLVM snapshot
records 99,150/109,322 lines (90.6954%), 12,606/13,988 branches (90.1201%),
5,053/5,751 functions (87.8630%), and 148,354/165,369 regions (89.7109%). The
compact snapshot projection retains commit metadata
`3272b3ef49a87c2947c08b46596b442195c6a8db`; this provenance caveat is retained
rather than silently relabeled. The LLVM JSON report normalizes segment ranges
to segment-start lines while preserving aggregate region coverage. No compatible
snapshot comparison is claimed for this measurement. The paired rectangular
H16x4/V4x16 AVIF witnesses are exact through safe Rust with independent Y/U/V
and Pillow RGB evidence, but the four-metric 100% gate remains open.

The strict all-target/all-feature Clippy gate passes on the installed nightly
rustc 1.99.0 / Clippy 0.1.99 toolchain with `cargo clippy --workspace --all-targets
--all-features --locked -- -D warnings`; no wrapper change or lint
suppression was used.

The previous-slice managed Coverage MCP run `a90eb75b-d62d-4c80-a75f-a753990fdea6` completed the
all-feature workload against exact implementation commit
`f92b3d6896e7e03a46396d53ad44dba96866de0e` in 149,159 ms and passed. Its LLVM
artifact was ingested as snapshot `bd67106e-6a28-41c2-9947-5c278e929f83`. It
measures 98,968/109,130 lines (90.6882%), 12,568/13,930 branches (90.2225%),
5,045/5,744 functions (87.8308%), and 148,099/165,103 regions (89.7010%). The
run and stored snapshot metadata are exact for `f92b3d68`. Direct aggregate
comparison with prior snapshot `6fa9ab92-2f3e-4551-b107-6710dda14e3d` reports
+5/+0/+1/+5 covered metric deltas and +0/+0/+0/+0 denominator deltas. Named
test attribution is unavailable, so no named-test attribution is claimed. The
following-Horizontal AVIF witness is exact through safe Rust; the AVIF planned
gaps, transient allocation work, and four-metric 100% release gate remain open.
The largest misses remain in the intentionally incomplete AV1 block/entropy
surface.

The current run supersedes that previous-slice record: run
`9212d568-8e60-4701-9cfe-089f74cf481b` passed at exact commit `a8af95eb`,
ingested snapshot `64548143-b655-4ae7-87d3-4578e9ee4de4` in 148,795 ms, and
reported the same four aggregate metric totals. Its explicit incremental
review against snapshot `bd67106e-6a28-41c2-9947-5c278e929f83` found zero
newly covered identities and zero regressions.

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
task is `supported` and records the aggregate improvement. Current totals are
98,331/108,795 lines (90.3819%), 12,462/13,824 branches (90.1476%), 5,009/5,728 functions (87.4476%), and 147,310/164,674 regions (89.4555%).
This slice adds real codec paths and increases the denominator, so the 100%
gate remains open rather than being relabeled as complete.

An earlier revision-bound hash tuple was refreshed at base revision
`2a141b6fe640af41549b71421fbe4b8f2b134e4f`; its managed coverage run was
bound to implementation commit `3fc0b58cbdb50acc8f4ee0d2a340207f47e79d21`.
That historical snapshot metadata reported project commit `cb82fc38`; the
mismatch remains retained as a provenance caveat.
`python3 scripts/verify_claim_ledger.py` checks the manifest, generated matrix,
coverage-origin inventory, roadmap, and all auxiliary fixture hashes against
the committed tree. This ledger refresh records current source/evidence
integrity; it does not close the separate 100% LLVM coverage gate or relabel
the historical Pillow parity run.

## Current gate status

The current committed tree passes formatting, locked all-feature check, strict
Clippy, the complete all-feature test suite, strict rustdoc, coverage-origin,
diagnostic-provenance, unreachable-contract, package-surface, license, roadmap,
claim-ledger, and diff checks. The latest complete managed Coverage MCP run is
`ec4c4bbd-dbda-4e49-8109-d7da07722dc0`; it passed in 149,142 ms at implementation
commit `93ec80ec99c42671dce6cf70694bce27ad8a2ef4` and ingested snapshot
`7665cda3-f4a7-4568-b871-a9d34afaa92c`. The snapshot metadata retains commit
`3272b3ef49a87c2947c08b46596b442195c6a8db`, which is recorded as a provenance
caveat. It reports 100,389/110,015 lines (91.2503%), 12,861/14,246 branches
(90.2780%), 5,125/5,794 functions (88.4536%), and 150,221/166,375 regions
(90.2906%); 9,626 lines, 1,385 branches, 669 functions, and 16,154 regions
remain below the 100% release target. Compared with the prior complete snapshot
`c694d0a5-4b6e-490c-b7b7-df010e668fb8`, the covered deltas are +73 lines, +26
branches, +1 function, and +97 regions; denominator deltas are +6, +26, +0,
and +12 respectively.
The current H16x4 H_DCT/CFL implementation commit `f05cdf26436e80f751b6f98646151db7f226cdc9`
also passes the 33-lane native/WASM feature matrix, the clean package-consumer
check, and the 44 coverage-configured matrix tests; its bounded incremental
Coverage MCP evidence is recorded above.
The bounded incremental run `95dc20e0-33b6-499a-9567-2d54f37c73ae` ingested
snapshot `de02b397-48fa-44ed-bdaf-df4487b096bf` against baseline
`c694d0a5-4b6e-490c-b7b7-df010e668fb8`, selected exactly
`decode:jpeg:huffman_default` and `decode:jpeg:huffman_optimized`, and passed
both active rows. Its incremental review reports +66 newly covered lines,
+1 function, +125 regions, and zero regressions; attribution is unavailable
and the selected scope is not a full-release measurement. The source review
shows the compile-time AC pair-table builder has no uncovered source regions
in the complete refresh; the extra `cfg(coverage)` probe is classified as the
existing `defensive_model` origin and leaves the non-coverage const tables
unchanged.
The new bounded AVIF incremental run
`6ff67b6a-f8e8-4b32-97cb-48e4ce3bd8ae` passed in 68,405 ms at exact
implementation commit `9f333de8096faaa4e0f9d8bb2eddde41dbbc2727` and ingested
snapshot `b8793e3c-91bf-4bd6-a927-fe0091ccfadb` against explicit baseline
`7665cda3-f4a7-4568-b871-a9d34afaa92c`. It selected only
`decode:avif:coverage_h16x4_tx4x4_split_01`; the additive baseline-union review
adds 192 covered line identities, no newly covered branch/function/region
identities, and no regressions. Snapshot metadata retains commit
`3272b3ef49a87c2947c08b46596b442195c6a8db`; test attribution is unavailable,
and this selected-subset result is bounded evidence rather than a replacement
for the complete release measurement.
The newest bounded AVIF incremental run
`ec9eeafa-f383-40f0-a5fb-d938e19de70f` passed in 65,013 ms at exact
implementation commit `bcb6fa5f205d987434f579c71853a3f3252e0c77` and ingested
snapshot `0e194d9f-dfa5-4746-b1af-9b6316281d48` against explicit baseline
`7665cda3-f4a7-4568-b871-a9d34afaa92c`. It selected only
`decode:avif:coverage_h16x4_tx8x4_split_01`; the additive baseline-union review
adds 686 covered line identities, no newly covered branch/function/region
identities, and no regressions. The aggregate denominator grew by 2,823 lines,
72 branches, 10 functions, and 582 regions because this slice adds production
code. Snapshot metadata retains commit
`3272b3ef49a87c2947c08b46596b442195c6a8db`; test attribution is unavailable,
and this selected-subset result is bounded evidence rather than a replacement
for the complete release measurement.
The clean-revision R4x16 reconstruction run
`2794788f-4fe8-4212-8256-edd7f57e0b37` passed in 40,221 ms at exact
implementation commit `8e695c7f3c437f91597e3acfb0959de3eeff5a8c`. Automatic
ingestion marked its generated report stale after the test passed, so the exact
report was explicitly imported as snapshot
`c1df95e5-7a3f-4db6-bac9-b438856faa5b` and reviewed against baseline
`7665cda3-f4a7-4568-b871-a9d34afaa92c`. The additive baseline-union review
reports +1,638 covered lines, +0 branches, +0 functions, and +0 regions, with
denominator changes of +2,815, +72, +10, and +582. Merge exactness is false,
test attribution is unavailable, and unobserved baseline hits are not
regressions; this selected-subset result is not a full-release measurement.
The final clean-revision cfg-coverage hook run
`7cdea475-3fdc-4194-8bd9-7509249580ff` passed in 107,163 ms at exact
implementation commit `9831b09d65dd02754785d6e3de4d02bf5e43559f` and ingested
snapshot `1007a9cf-9c6e-4f5f-ae72-b1c99f3c6cbd` against explicit baseline
`7665cda3-f4a7-4568-b871-a9d34afaa92c`. It selected only
`test_internal_coverage_hooks`; the hook’s intentional defensive panics were
caught as expected. The additive baseline-union review reports +174 covered
lines, +0 branches, +0 functions, and +0 regions, with merge exactness false
and no global regression claim. This is bounded cfg-coverage evidence, not a
replacement for the complete four-metric release measurement.
The committed loop-contract run `576b9aa4-c43f-45a9-b5aa-525dbfa4968e` passed
in 23,078 ms at exact implementation commit
`c7704f81fa83dd6d272a028ef0800b7ab5999f34` and ingested snapshot
`f70c91be-9755-44e5-b6c3-564a5cdbfa8f` against explicit baseline
`7665cda3-f4a7-4568-b871-a9d34afaa92c`. Its additive baseline-union review
reports 223 newly covered line identities, +1 covered function, +967 covered
regions, and zero regressions; merge exactness is false and test attribution
is unavailable. The selected subset is bounded evidence, not a replacement for
the complete four-metric release measurement.
The latest selected AVIF policy run `4aae5760-8537-4500-8afd-a92e1235cf5e`
passed in 31,512 ms at exact implementation commit
`fb5497e97daf596b42d36e94ae0cb3f9377417cb` and ingested snapshot
`1cb3b222-bbf5-4b8e-a0de-ebcf85eec477` against explicit baseline
`7665cda3-f4a7-4568-b871-a9d34afaa92c`. It selected exactly
`decode:avif:animated_error_resilient` and
`decode:avif:error_animated_repeated_frame_id`; the matrix reported 2/2
active rows passed, 0 failed, and 0 planned-not-executed. The standalone
incremental review reports additive baseline-union deltas of +8 lines, +10
branches, +0 functions, and +1,081 regions, with denominator changes of
+4,121/+440/+46/+12,367 and 1,315 selected-projection newly covered line
identities. It reports 74,613 unobserved baseline observations and zero
regressions; merge is conservative and named-test attribution is unavailable.
The ordinary snapshot projection retains metadata commit
`3272b3ef49a87c2947c08b46596b442195c6a8db`, so the durable run commit is the
authoritative implementation provenance. This is bounded selected-subset
evidence, not a complete four-metric release measurement.
The pure-Rust cutover also closes the obsolete native-path findings
`AVF-002`, `AVF-007`, `AVF-017`, `FTR-003`, `FTR-004`, `FTR-005`, `FTR-007`,
`FTR-008`, `FTR-019`, `FTR-030`, `FTR-031`, `FTR-033`, `QA-013`, and `QA-038`.
The current active inventory is therefore 248 findings. Resolved rows are
removed from the canonical JSON and this rendering; historical resolution
evidence remains in Git history and `docs/roadmap.md`.
The next implementation item selected by the JSON dependency order is
`AVF-STILL-001`: broaden the safe AV1 walker beyond the now-proven baseline,
accepted-brand variants, grid fixture, and two-column multitile fixture. The
work item remains partial until broader partition/block states, independent
evidence, and target checks exist.
The latest managed insight ranks `src/codecs/avif/av1/block.rs` first, with
8,903 uncovered executable lines, 1,067 uncovered branches, 450 uncovered
functions, and 2,173 uncovered regions; its first compact ranges are 350, 1,054,
and 1,348-1,352. This is the next investigation target, not evidence that
those states are reachable from the current Pillow corpus.

The 3 decode gaps and their pure-Rust dependencies are recorded exactly in
the ledger below. A planned row is a real input or operation that must become
supported by safe Rust, not permission to call libavif, dav1d, libaom, or a C
shim at runtime. Those libraries remain oracle/provenance material only.

The integration test
`test_avif_planned_gaps_are_explicit_safe_rust_contracts` also walks every
planned decode and encode fixture. It requires a concrete gap reason and no
pixel or encoded-output reference, and checks that the current public result
is the declared typed safe-Rust gap. That test is a guardrail for the roadmap;
it does not count a planned row as completed. The two adjacent EOB mutation
controls are active negative rows instead: the matrix verifies their exact
typed `Malformed` result, so they are not counted as open capability gaps.

The input-only EOB campaign
`tests/fixtures/outputs/av1_search/coverage_luma_eob_bin_campaign_01.json`
generated exactly 100 candidates in 10 deterministic families and qualified
one stable candidate, `luma-eob-bin-f01-n03`. It is promoted as
`portable_lossy_420_q99_luma_eob_bin2_eob3.avif`: a 4×4 8-bit 4:2:0 one-tile
origin TX8×8 DCT-DCT block with luma EOB-bin two, EOB three, direct EOB-base
zero, and a non-empty AC coefficient. Its pinned dav1d trace contains 27
entropy operations, and its encoded-item, Y, U/V, and Pillow RGB hashes are
recorded in `roadmap.json` and
`tests/fixtures/outputs/av1_reconstruction.json`. The existing generic safe
Rust coefficient path already handles this sentence, so the slice adds
independent structural and pixel evidence without claiming broader EOB
support.

A follow-up input-only campaign reused the same 100-candidate/10-family
contract for the legal 8×8 luma EOB-bin-one/EOB-one/direct-EOB-base-zero
sentence. It qualified 0/100 candidates. The pinned corpus produced EOB-bin
two for 2 cases, four for 4, five for 20, six for 39, and no EOB trace for 35
skipped cases. The complete report is
`tests/fixtures/outputs/av1_search/coverage_luma_eob_bin1_campaign_01.json`
(SHA-256
`2f2de5fc6d5a551e9d883a3f7c75a3ff4d26c167f2a399c3ca62398439c9bb08`). This is
bounded reachability evidence for the pinned encoder/settings/families only:
it is not proof that EOB-bin one is unreachable, so no fixture, production
admission, denominator, or 100% coverage claim changes. The campaign invoked
no repository Rust and keeps `AVF-ENTROPY-001` partial; the next search must
target another independently reachable legal coefficient sentence.

The managed Coverage MCP incremental run for this exact filtered fixture was
`a94d18a6-52c1-4729-9056-88ed1f84c3cd`, passed in 80,843 ms at implementation
commit `288223f66f31814c2bb30f2047284cd20550f4bc`, and ingested snapshot
`61afd9af-e181-494b-a55c-7fa3f0306495` against explicit baseline
`7665cda3-f4a7-4568-b871-a9d34afaa92c`. The managed command passed all 44
coverage-harness tests; the reconstruction dispatcher executed exactly one
requested fixture. Standalone
`coverage_review(task=incremental)` reports additive baseline-union deltas of
+661 lines, +17 branches, +1 function, and +5,289 regions; denominator changes
are +3,639/+364/+39/+10,218. Its selected projection reports 7,393 newly
covered line identities, 39,423 unobserved baseline observations, and zero
regressions. Named-test attribution is unavailable and the selected subset is
not a complete release measurement; the compact snapshot metadata retains
commit `3272b3ef49a87c2947c08b46596b442195c6a8db`, while the durable run is
bound to the implementation commit above.

Managed Coverage MCP then ran the exact central matrix selection for both
controls: run `bcd2e044-53c5-4c06-a9c6-d6ca8b02220d` passed in 31,206 ms at
implementation commit `feb851bdb1c882191c1467117b2acbb5b533ec3a` and ingested
snapshot `287b9937-982e-471d-92f0-4137927a0730` against baseline
`7665cda3-f4a7-4568-b871-a9d34afaa92c`. The log reports `selected=2 active=2`
and `2/2 active rows passed`. The standalone incremental review is measured
with additive baseline-union deltas of +1 covered branch and +0 covered lines,
functions, or regions; the selected projection reports 1,080 newly covered
line identities. The merge is conservative, 75,631 baseline observations were
not observed in the selected subset, named-test attribution is unavailable,
and the run makes no regression claim. Snapshot metadata retains commit
`3272b3ef49a87c2947c08b46596b442195c6a8db`; the durable run commit is the
authoritative implementation provenance. This is bounded coverage evidence,
not a replacement for the complete four-metric release measurement.

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
and two 16×8 chroma sentences. The new mode-3 witness also proves the
following Horizontal32x16 leaf reconstructed from prepared spatial edges,
including its 4,359-operation entropy trace and exact Y/U/V/RGB8 output. The
R16x64 witness also proves safe 16×16
matrix-10 luma dequantization and top-only DC prediction for all four vertical
children when no left neighbor exists; these witnesses do not promote their
broader families. Broader
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
| W3 | Coverage-origin inventory and justified defensive-path evidence | Evidence-only; no new product behavior | The origin verifier passes for 513 exact `cfg(coverage)` guards across 86 files, with no Pillow-parity origin assigned. The current managed snapshot `7665cda3-f4a7-4568-b871-a9d34afaa92c` is recorded from run `ec4c4bbd-dbda-4e49-8109-d7da07722dc0` at implementation commit `93ec80ec99c42671dce6cf70694bce27ad8a2ef4`; the selected incremental run `de02b397-48fa-44ed-bdaf-df4487b096bf` covered the runtime validation of the compile-time JPEG AC table builder through the two standard/custom Huffman rows. Snapshot metadata commit `3272b3ef49a87c2947c08b46596b442195c6a8db` is retained as a caveat. The four metrics remain below 100% and stay visible in the current coverage table. |
| W4 | AVIF `iloc` item-location/source-provenance contract and pure-Rust cutover | Integrated locally; capability gaps remain planned | Item extents and source locations are retained and asserted by the Rust-only feature contract. The runtime no longer depends on `libavif`/`dav1d`/`libaom`; 311 AVIF decode rows are active, 3 decode rows are explicit pure-Rust gaps, and all 32 encode rows remain planned. The 10-bit and 12-bit still witnesses, H16x4 TX4x4/TX8x4/H_DCT-CFL and split filter-intra TX8x4 witnesses, the origin Vertical8x16 mode-4 TX4x4-grid witness, the new V4x16 predictor/transform witness, the qcat-one square AV1/CDF path, and the bounded first-frame sequence policy cases are bounded production classes, not general AVIF completion. |
| W5 | Machine-checked unreachable-contract catalog and Cargo package surface | Integrated in the current tree | The ten-category catalog and exact package-path manifest both verify successfully; claim-ledger, diagnostic, license, and package-surface checks remain release evidence rather than Pillow parity. |

The five worker checkouts were disposable execution spaces. Their reviewed
slices are represented by reviewed commits on `main`; no worker pushed
directly. The accepted product-claim tuple remains revision-bound to the
historical Pillow parity record at `36b9396`; the current hash and
coverage-evidence refresh is bound to implementation anchor `93ec80ec` and does not silently rewrite
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
| Fixture rows | 1,538 total | 1,141 decode/inspect/verify rows plus 397 encode rows exist. Current status is 1,138 active decode rows, 365 active encode rows, 3 planned decode rows, and 32 planned encode rows; the planned rows are explicit rather than mislabeled malformed cases. |
| Managed Pillow checks | 1,449/1,449 passed | Managed parity run `84716077-aee7-4396-8328-e6735202b044` is bound to revision `36b9396`. |
| Immediate correction queue | 0 | No newly confirmed defect is waiting ahead of capability work. |
| Current native all-feature ordinary contracts | 44/44 matrix tests and 66/66 feature-gate tests passed | The current local tree is behaviorally green for these Rust integration contracts. |
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

The latest complete managed Coverage MCP snapshot above remains the accepted
release-measurement baseline; the current H16x8 commit has a separate
selected-subset incremental result recorded below. That bounded result is never
substituted for the complete denominator.

| Metric | Covered | Total | Covered % | Gap | Gap % |
| --- | ---: | ---: | ---: | ---: | ---: |
| Lines (managed Coverage MCP) | 100,389 | 110,015 | 91.2503% | 9,626 | 8.7497% |
| Branches (managed Coverage MCP) | 12,861 | 14,246 | 90.2780% | 1,385 | 9.7220% |
| Functions (managed Coverage MCP) | 5,125 | 5,794 | 88.4536% | 669 | 11.5464% |
| Regions (managed Coverage MCP) | 150,221 | 166,375 | 90.2906% | 16,154 | 9.7094% |

The newest H16x8 fixture is a real safe-Rust reconstruction class, not a
coverage-only test. Its input-only search reports and exact pinned dav1d,
Y/U/V, and Pillow RGB evidence are recorded in the AVIF section above, along
with the bounded selected-fixture Coverage MCP review.
The current managed LLVM JSON report carries the warning that segments are
normalized to segment-start lines; aggregate region coverage is preserved from
its report summary. RN-001 therefore remains open for the current source tree:
the release target is still 100% for all four measures. The explicit aggregate
comparison found covered deltas of +36 lines, +0 branches, +10 functions, and
+66 regions, with denominator deltas of +15 lines, +0 branches, +3 functions,
and +24 regions; named-test attribution is unavailable.
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
bound to code-bearing commit `de664cc6dc8d12f6f7f5fe3b73c01faef3709d63`; run
`3b4748a2-f5bf-43d2-9f56-1067f4210257` ingested snapshot
`1ac75559-0323-47ab-81bd-d9c6dac620fb` and its exact aggregate result is
recorded above. The corrected H32x8 false-filter sentinel slice adds one
safe-Rust witness and a 100-case input-only campaign; the following-leaf luma Diagonal45 slice adds one safe-Rust
witness, a 100-case input-only campaign, and the proven preceding-edge seed
fix; the luma Smooth/SmoothVertical/SmoothHorizontal slice adds three
safe-Rust witnesses and the per-child no-top SmoothHorizontal split edge fix;
the preceding H4 slice adds the safe-Rust
32x8 luma and 16x4 chroma matrix/dequantization implementation and proves the
pinned `coverage_r32x8_h4_ripple_01.avif` candidate byte-for-byte against the
pinned dav1d planes, 1,522-operation entropy trace, and Pillow RGB reference.
Its `PARTITION_H4` tree has three ordered 32x8 luma leaves and 16x4
subsampled chroma leaves; it also exercises one-sided DC prediction. It
changes source mapping and coverage denominators; it does not claim that the
aggregate 100% gate is done.
The current witness `coverage_r32x8_h4_ripple_01.avif` extends this bounded
class to three 32x8 luma leaves and 16x4 subsampled chroma leaves, with an exact
1,522-operation trace, exact reconstructed planes, exact Pillow RGB bytes,
and geometry-specific matrix-9 plus one-sided-DC evidence.
Real behavior uses Pillow-visible fixtures or Rust-only feature contracts,
private models remain origin-registered, and the claim ledger remains separate
from this cleanup checkpoint.
The current bounded chroma-angle slice adds the
`coverage_square8_chroma_diagonal45_angle51_01.avif` fixture and its 100-case
input-only campaign: 5 candidates qualified, all with coded UV mode 3, angle
symbol 5, delta +2, and resolved angle 51 degrees, while zero right-hand
symbol-3/delta-0/45-degree cases were observed. Its exact 119-operation
reconstruction covers `src/codecs/avif/av1/block.rs:345` and matches the
independent Y/U/V and Pillow RGB evidence without a production code edit.

**Source IDs:** `QA-003`, `QA-010`, `QA-020`, `QA-030`.

**Done:** not yet. The accepted current managed report keeps Pillow, Rust-only,
and private-model origins distinct, but it reports 90.6954% lines, 90.1201%
branches, 87.8630% functions, and 89.7109% regions. Close this item only when
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
at that historical revision, the complete inventory therefore remained 266 active finding rows; this slice
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
partial and, at that historical revision, the complete inventory remained 266 active finding rows.

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
at that historical revision, the inventory remained 266 active finding rows.

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
open. RN-003 remains partial and, at that historical revision, the complete inventory remained 266 active
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
at that historical revision, remained 266 active finding rows.

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
at that historical revision, remained 266 active finding rows.

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
`API-051`, `API-052`, `API-053`, `API-054`, `AVF-013`, `AVF-022`, `AVF-023`,
`AVF-024`, `AVF-025`, `AVF-034`, `GIF-009`, `GIF-019`,
`PNG-012`, `PNG-013`, `PNG-018`, `PNG-020`, `TIF-009`, `TIF-013`, `TIF-016`,
`TIF-025`, `TIF-026`, `TIF-030`, `WEP-003`, `WEP-010`, `WEP-019`, `WEP-020`.

**Done when:** a frame/page fixture proves the selected access path, the
one-shot result stays identical, incomplete input has a stable lifecycle, and
no second accidental cache or sequence model is introduced.

## AVIF planned-gap ledger (current tree)

These are the exact 3 decode rows currently marked `planned` in the generated
matrix. The child-friendly reason is simple: the safe-Rust decoder can read
some small AV1 building blocks, but it cannot yet read every kind of AV1
sentence that an AVIF file may contain. Each row below is a named lesson for
the decoder, not an excuse to route around Rust.

| Pure-Rust work category | Planned rows | Why the work is needed |
| --- | --- | --- |
| General still brand/container control | Closed: baseline and all three accepted-brand rows | The 128×128 baseline and each legal generic-HEIF major-brand ordering now decode through the same safe Rust AV1 path with exact independent 49,152-byte RGB references. |
| Partitioned-square public raster | Closed: all 16 partitioned-square rows | The safe decoder now materializes all twelve cropped 12×12 and four 16×16 4:4:4 square fixtures with exact pinned planes and entropy traces. This category is no longer a planned matrix gap; broader baseline/tile/sequence classes remain separate work. |
| Adjacent entropy and tile syntax | No planned row; `portable_lossy_420_q99_eob_bin_control` and `portable_lossy_420_q99_eob_base_control` are active malformed controls | The safe decoder proves legal luma EOB-bin-two, EOB-bin-five, and EOB-bin-six AC classes plus legal chroma EOB-base/high branches, with independent fixtures and exact traces. The two one-byte mutations are independently rejected at EOB-bin/EOB-base with no YUV output, and safe Rust reports the AV1 symbol-coder overread as typed `Malformed`; this is invalid-input evidence, not a claim that the mutations are legal syntax. Empty-tile malformed input and the adjacent lossy DC predictor are also active. |
| Sample depth and future alpha variants | `high_bitdepth` (the committed `with_alpha` row and bounded 10-bit/12-bit still slices are active) | A picture may use more than 8 bits or carry a second transparency picture. Pure safe Rust now decodes the committed 64×64 alpha pair and bounded 16×16 10-bit and 12-bit 4:4:4 stills to exact public bytes; animated/high-depth sequence materialization, other subsampling, and broader alpha relationships/depths remain explicit future work. |
| Color | `hdr` | HDR changes how numbers become colors. It needs explicit safe-Rust bounds, declared color conversion, and metadata rules. |
| Sequences and frame identity | `animated` | A movie is many pictures plus timing and frame IDs. The active `animated_error_resilient` row independently materializes its first sample but records a typed sequence-level `Unsupported` gap; the repeated-ID fixture rejects its later repeated current ID before publishing sequence state. Timing, references, and multi-frame presentation remain. |
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

There were 39 former-native AVIF rows in the cutover census. Two EOB mutation
controls are now active malformed-input rows with independent rejection and
typed safe-Rust error evidence. Exactly 35 former-native rows remain planned:
3 decode rows and 32 encode rows. The executable matrix test rejects any
remaining former-native row that becomes active without the corresponding
pure safe-Rust implementation and independent evidence.

The exact planned groups are:

- partitioned-square public raster and its admitted coefficient classes (closed: all 16 rows are active);
- adjacent AV1 EOB entropy syntax (the two mutation controls are active
  malformed rows; legal positive EOB work remains in `AVF-ENTROPY-001`);
- 10/12-bit reconstruction and broader auxiliary-alpha composition (1 decode row);
- HDR conversion (1 decode row);
- animation, timing, and error-resilient track presentation (2 rows); repeated-ID validation and independent first-frame materialization are closed;
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
| 10/12-bit samples and broader alpha variants | HDR workflows, transparent icons, UI assets, and compositing pipelines | Reconstruct higher-depth planes, convert them with checked arithmetic, and extend relationship-aware alpha pairing beyond the committed 64×64 unassociated 8-bit fixture | Native libraries already owned sample conversion and auxiliary-item composition; pure safe Rust now closes the committed `with_alpha` fixture plus one 10-bit and one 12-bit still class, while `high_bitdepth` animation and broader alpha relationships remain planned |
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
| `AVF-STILL-001` frame raster | broader partition/tile states | Walk the AV1 partition tree across every tile, retain syntax/CDF and above/left contexts, reconstruct bounded luma/chroma blocks, and compose the visible frame without native state. The 128×128 lossy 4:2:0 baseline, all three legal accepted-brand orderings, and the 256×128 two-column `multitile.avif` frame are now proven full-frame cases; the committed 64×64 lossless 4:4:4 primary in `alpha.avif` is also exact through the alpha row. | Partial implementation: the safe walker and production path now consume all sixteen partitioned 4:4:4 square fixtures—twelve cropped 12×12 cases and four 16×16 cases—in coded payload order, plus the committed two-column multitile frame, the promoted `coverage_r8x16_band_05.avif`/`_06.avif` 8×32 4:2:0 pair, the pinned `coverage_r32x16_origin_01.avif` 32×16 4:2:0 Horizontal32x16 origin leaf, the `coverage_r32x16_filter_intra_tx8x8_01.avif` 32×16 origin Horizontal32x16 split with filter-intra disabled and four TX8x8 luma children, the new `coverage_r32x32_following_filter_intra_split_mode0_01.avif` 32×32 horizontal split with a following mode-0 filter-intra leaf and a TX16x16 luma split, and the pinned `coverage_r16x64_grid_01.avif` 16×64 4:2:0 Vertical16x64 depth-two TX16x16 luma split. The R8x16, Horizontal32x16, TX8x8 split, following mode-0, and R16x64 fixtures share pinned dav1d topology, checked 4:2:0 plane dimensions, exact entropy traces, and exact public Pillow RGB references; the TX8x8 witness has a 2,328-operation trace and proves split residual placement through the generic safe path, while the following mode-0 witness has a 2,204-operation trace and proves prepared spatial-edge publication for a true following filter-intra leaf. R16x64 additionally verifies the 16×16 matrix-10 luma table and top-only DC prediction for every child without a left neighbor. The promoted `coverage_adst_public_04.avif` adds a 16×4 full-chroma bottom-crop case with two coded 8×8 leaves, an exact 407-operation trace, and exact public RGB/YUV references. The `coverage_i444_rect_01.avif` witness adds a 16×16 full-resolution 4:4:4 split-root/four-leaf case with a 499-operation trace, exact Y/U/V planes, full-resolution chroma residuals, matrix-10 U/V AC deltas, delta-Q, and exact public RGB bytes. The new `coverage_i444_rect_02.avif` witness holds the same topology but adds a 553-operation trace, distinct residual/EOB states, exact Y/U/V planes and RGB bytes, and a filter-intra leaf. The focused `baseline_six_terminal_then_stops_at_vertical_8x16_gap` contract remains a bounded syntax sub-gap. Broader partition/block state, all predictors/residual classes, every filter-intra mode and edge case, additional tile shapes, and independent full-frame proofs remain open. `FrameCanvas::place_cells` validates and atomically places complete reconstructed cells. The new `coverage_vertical8x16_filter_intra_mode0_01.avif` origin witness adds a checked Vertical8x16 filter-intra mode-0/unsplit-TX8x16/TX4x8-chroma class with exact 584-operation entropy, plane, and Pillow RGB evidence; broader classes remain open. |
| `AVF-ENTROPY-001` adjacent EOB syntax | active malformed controls plus future legal EOB-bin/base classes | Implement the remaining legal EOB-bin and EOB-base branches with their coefficient scans, tokens, signs, dequantization, and transform output; preserve typed `Malformed` for invalid arithmetic termination and typed `Unsupported` for valid syntax not yet proven. | Partial: safe Rust handles legal luma EOB-bin 0, 2, 3, 4, 5, and 6 classes, including the promoted 4×4 4:2:0 TX8×8 DCT-DCT witness `portable_lossy_420_q99_luma_eob_bin2_eob3.avif` (27 entropy operations, EOB-bin two, EOB three, direct EOB-base zero, exact Y/U/V and Pillow RGB hashes), legal chroma EOB-base/high branches including EOB-bin-four, and exact UV dequantization plus matrix-10 data. The 100-candidate/10-family campaign qualified exactly one candidate and did not invoke repository Rust. A follow-up 100-candidate/10-family bin-one campaign qualified 0/100 and is retained as bounded no-hit evidence only; it does not establish unreachability or change admission. The two one-byte controls are active negative evidence: pinned dav1d 1.5.3 rejects them at the recorded EOB stages with no YUV output, and safe Rust reports the symbol-coder overread as `Malformed`. They do not activate unproven positive EOB syntax. |
| `AVF-SAMPLE-001` sample depth | `high_bitdepth`; later `hdr` | Reconstruct 10/12-bit planes, apply checked sample-to-8-bit conversion at the public boundary, and test overflow, limits, and cancellation. | Partial: `av1/sample_depth.rs` validates 8/10/12-bit nominal ranges and performs explicit high-depth bit truncation. Safe Rust now proves exact public RGB8 parity for the bounded 16×16 10-bit profile-1 and 12-bit AV1 profile-2 full-range 4:4:4 all-lossless still classes. Entropy reconstruction, restoration, 4:2:2 decoding, animated/high-depth alpha and sequence materialization, HDR conversion, and broader 12-bit cases remain open. |
| `AVF-ALPHA-001` auxiliary composition | broader grid and alpha variants | Decode the primary and monochrome auxiliary AV1 items, validate matching dimensions/depth, distinguish unassociated from premultiplied alpha, and emit the correct RGBA result and source descriptor. | Implemented for the committed `alpha.avif` fixture: safe Rust reconstructs all 37 terminal leaves of the 64×64 monochrome auxiliary tile, derives neighbor state by geometry, pairs the primary and auxiliary planes, emits RGBA8 with source alpha `Auxiliary`, and matches the independent 16,384-byte reference exactly. General alpha dimensions, high bit depth, premultiplied relationships, and broader grid pairing remain planned under the named sample/composition work items. |
| `AVF-COLOR-001` declared color pipeline | `hdr` | Implement transfer, primaries, matrix, range, and sample-position conversion with bounded arithmetic and explicit source metadata. | Planned; current RGB conversion is the narrow 8-bit BT.601 full-range class. |
| `AVF-COMPOSE-001` grid canvas | broader grid counts, dimensions, and relationships | Decode each referenced color/alpha cell, validate cell geometry, place cells in a bounded canvas, and apply relationships without treating metadata inspection as pixel composition. | Implemented for the committed `grid.avif` fixture: safe Rust decodes both 80×64 color cells and their monochrome auxiliary alpha cells, validates complete 80×80 coverage, crops the second row to 80×16, and matches the exact 25,600-byte RGBA8 reference. Broader grid counts, dimensions, tile-boundary contexts, and relationships remain open. |
| `AVF-SEQUENCE-001` track presentation | `animated`; `animated_error_resilient` | Parse sample tables, retain frame state and references, enforce IDs/timing/limits, and present frames with default-image and disposal/blend rules. | Planned; both valid multi-frame cases still lack presentation. The repeated-ID error case has exact first-frame RGB8 evidence and stateful validation rejects its later repeated ID; the valid error-resilient case has exact first-frame RGB8 evidence but a typed sequence-level `Unsupported` result. |
| `AVF-TILE-001` tile raster | broader tile counts/shapes | Decode independently sized tile payloads into one frame canvas with tile-local bounds and shared state only where the AV1 syntax requires it. | Implemented for the committed 256×128 two-column `multitile.avif` fixture: safe Rust decodes and places both tile payloads exactly once, applies frame-global deblocking/CDEF, and matches the independent 98,304-byte RGB reference; the focused reconstruction proof also matches real dav1d all-filter YUV byte-for-byte. Broader tile counts, size combinations, boundary contexts, and full-frame references remain open. |
| `AVF-ENCODE-001` encoder | all 32 encode rows | Write the AVIF container and a safe Rust AV1 intra encoder, then round-trip emitted bytes through an independent decoder. | Planned; no native or pure-Rust encoder is currently wired. |

The previous `AVF-STILL-001` evidence addition was
`coverage_vertical8x16_filter_intra_mode0_01.avif`: an 8×16 8-bit 4:2:0
origin Vertical8x16 leaf with `FILTER_PRED[13/0]`, an unsplit TX8x16 luma
transform, and TX4x8 U/V transforms. Its pinned dav1d trace has partition
range `42232` and 584 entropy operations, and safe Rust matches exact
partition/entropy/Y/U/V/Pillow RGB evidence. The fixture, encoded-item, RGB,
and Y/U/V SHA-256 values are
`da511e016e1e8720cb21af34b4cf41001a97af0f0380576dc47355dcd630f39a`,
`e86cc0fdfc27ec55e542a581bb22b4c619f5dfac793593ec7b276a13df6d8224`,
`82b2100ac5f6f02e88ea931a90b2abab261b7486209ee4f63c538464c52b5c30`,
`b2785ade1a3c4756d80bf67138b50d410eb2863ff39410e94b8cfd44467baba6`,
`fe140aecdaf68c2a55f594a0a1eb6f9404e9e70f452aaee2d73fe7b98af6014a`, and
`a085afa18ac9de9d6f9c09b3fa6050395bbf1cc71d4444f3cfae4354057469e8`.
The safe-Rust fix uses dav1d's origin missing-edge values, adds the qcat2
TX4x8 chroma tables, corrects the 4:2:0 skip context and rectangular EOB
scratch index, and closes only this origin mode-0/Vertical8x16 class. The
earlier Square16, following-leaf, and TX8x8 split witnesses remain separate
bounded proofs; broader filter-intra modes and edges, partition/block states,
tile-local contexts, transform variants, and broader independent full-frame
proofs remain open.

The previous `AVF-STILL-001` witness was
`coverage_vertical8x16_filter_intra_mode1_01.avif`: an 8x16 8-bit 4:2:0
origin Vertical8x16 leaf with `FILTER_PRED[13/1]`, an unsplit TX8x16 luma
transform, and TX4x8 U/V transforms. Its pinned dav1d trace has partition
range `42232` and 559 entropy operations; safe Rust matches exact
partition/entropy/Y/U/V/Pillow RGB evidence. The fixture, encoded-item, RGB,
and Y/U/V SHA-256 values are
`7c04bf5be19e0e1acf757dbdda04b3fd48419a2df1dcf7a12871cdefbce99917`,
`2ce5e66bfed511611e28f06c13f3014e6863e026b9e22ea6fd2c2145e36adbde`,
`6051c012bac9735f10fb18bfe680fc9e3582ef6acfaa295a028f02ead7a642fe`,
`d5f1f32b7f3bc6d635a7a9bd89b9efa59670ffb723b6f5ff8f7d65a0eca940c9`,
`f3238ddee04bccf67e555675f978da2a2cd114f0eac6cf751f355763e84dde85`, and
`ce452bca9cac19f45e3e2257f2ae531197097512d1bd6f76cb914c7eb34f9615`.
The generic safe-Rust mode-1 prediction path was already covered, so this
slice adds exact parity evidence without claiming aggregate coverage gain;
it closes only this origin mode-1/Vertical8x16 class. Broader filter-intra
modes and edges, partition/block states, tile-local contexts, transform
variants, and broader independent full-frame proofs remain open.

The newest bounded luma-angle proof before the current rectangular slice is
`coverage_square8_luma_diagonal_down_right_01.avif`: a deterministic 16x8
8-bit 4:2:0 split-root frame whose right Square8 leaf selects luma mode 4
(`DiagonalDownRight`) with angle symbol `3` (delta `0`, absolute `135` degrees).
Both leaves use four TX4x4 luma payloads; both chroma leaves use DCT-DCT TX4x4
U/V, and the right luma/U/V residuals are non-empty. Its exact pinned trace
has 259 entropy operations and partition ranges `37392/43662/34793`; safe Rust
matches exact partition, entropy, Y/U/V, and Pillow RGB evidence. The selected
fixture/item/RGB SHA-256 values are
`fddb447f61b8aa89d5d2bc4dee0baf8dd2c3711ade6d4384edb052841cf4940f`,
`e78b3ce456d6a58f455cf8a3dd2bf800f78a273038374d5d1c25cc95f4126a48`, and
`44a7d5e7b2c778b65ee4dbd1379b87a2fc33cca36b2a180519d68cfc34eea01b`. The
100-case input-only campaign qualified 3/100 candidates and is recorded in
`tests/fixtures/outputs/av1_search/coverage_square8_luma_diagonal_down_right_campaign_01.json`
with SHA-256
`a77ccf867a514d80e8837486ba118b882ed5e6addc779f3a45c435b8a340854d`.
This closes only the bounded mode-4/135-degree Square8 class; it does not
close general AV1 or AVF-STILL-001.

The current latest `AVF-STILL-001` evidence addition is the paired
`coverage_h16x4_filter_intra_cdf14_false_01.avif` and
`coverage_v4x16_filter_intra_cdf19_false_01.avif` fixtures. The 16x16 4:2:0
`PARTITION_H4`/`PARTITION_V4` frames select four DC `Horizontal16x4`/
`Vertical4x16` luma leaves and false filter-intra decisions from CDF rows 14
and 19; both use rectangular DCT-DCT chroma transforms and 162-operation
pinned dav1d traces. Safe Rust matches exact partition, entropy, Y/U/V, and
Pillow RGB bytes. The H following leaf exercises two-entry UV palette
prediction, and the V following leaf reaches the transposed rectangular path.
The input-only campaign explored 100 candidates across 10 families and
qualified five per orientation with `repository_rust_invoked=false`. This is
bounded evidence only; broader rectangular predictors, filter-intra states,
and AVF-STILL-001 remain partial.

The newest rectangular-transform campaign tested three 100-candidate,
input-only workloads with the pinned Pillow 12.2.0/libavif 1.4.1/libaom
3.13.2 encoder and dav1d 1.5.3 decoder. The 16x8 origin Horizontal16x8
IDTX/V_DCT search qualified 0/100, and the 16x16 PARTITION_H4
Horizontal16x4 V_DCT/H_DCT search qualified 0/100. The predictor-enabled
16x16 PARTITION_H4 Horizontal16x4 search qualified 2/100 for ADST-DCT and
0/100 for ADST-ADST; it promoted `h16x4-f10-n00` for ADST-DCT. None of the
campaigns invoked repository Rust, and each report records deterministic
double-encode/item/trace predicates.

The promoted `coverage_h16x4_predictor_adst_dct_01.avif` fixture is a 16x16
8-bit 4:2:0 quality-26/speed-0 frame with four Horizontal16x4 luma leaves.
Its pinned root partition is `poc=0,y=0,x=0,level=3,context=0,partition=8,
range=43136`; the trace has 77 entropy operations and the encoded item is 35
bytes. The four luma leaves use modes 12, 9, 0, and 9; the first luma leaf
selects CDF transform symbol 5 / dav1d `txtp=1` (ADST-DCT), while the other
three use DCT-DCT. Both chroma residuals are skipped with EOB -1, so this
fixture does not prove the UV matrix-7 lookup. Exact evidence is fixture SHA
`f736f845547eab5d301e5e2b5ae9b3f306224dea14179090679736c5eaecc535`, encoded
item SHA `8a8a38d3ede09fabddb1775277c6337b5f0b0c487e78a4068b3d809f0adf172d`,
Y SHA `978f0ef5c9800af08f2a938392d863e20c158d41230c5c91db0ff74c2c3f7c1d`,
U/V SHA `1df1b7ce1fd8fcbe20cde61646875e54fe38d8945ea7911afd59e025cc520a68`,
Pillow RGB SHA
`84fdaf2915f3f338bb4620a89640a7a44b2eb13099b31f5ff1437e6a05f08167`, and
trace SHA `62ab3da7e02c479ce6d2beff1a6774865fd00ee5cc8c22c664e66d4b3a0047f0`.
The production fix seeds the H4 walker with the effective qindex for the
qcat-three frame CDF state, admits only this narrow no-filter frame-tools
class, and wires dav1d-verified luma matrix 6 for `RTX_16X4`. This closes only
the predictor-enabled H16x4 ADST-DCT class; ADST-ADST, IDTX, V_DCT, H_DCT,
broader rectangular states, and AVF-STILL-001 remain partial.

The independent follow-up witness
`coverage_h16x4_predictor_adst_dct_f02_n08.avif` is a 16x16 8-bit 4:2:0
`PARTITION_H4` frame with four unsplit `Horizontal16x4` luma leaves and modes
`[10, 1, 1, 3]`; the third leaf selects ADST-DCT and the other three select
DCT-DCT. Its second leaf's coded Vertical mode resolves to 81 degrees from
angle delta -3. The pure safe-Rust decoder applies the native, non-upsampled
Zone-1 edge policy and repeats the final available top sample. The pinned
origin trace records partition range `43136` and 251 entropy operations, and
the exact YUV/Pillow RGB references match. This closes only the observed
predictor/edge-policy and matrix-9 class; broader angles, predictors,
transforms, matrices, qcats, following-leaf states, and AVF-STILL-001 remain
partial.

The managed incremental Coverage MCP run for this exact follow-up selector
`29ab26e8-5069-4d43-bc19-7dadee91cb33` passed in 41,734 ms at exact
implementation commit `4377ca3e29db98fd1099d00cf8e673727ebe82de` and ingested
snapshot `5679a889-1feb-48e2-8253-4bf2c5ab020d` against explicit baseline
`7665cda3-f4a7-4568-b871-a9d34afaa92c`. The managed test log reports 1 passed,
0 failed, and 43 filtered out. Its additive baseline-union review reports +8
covered lines, +10 branches, +0 functions, and +937 regions; denominator
changes are +4,492/+562/+67/+12,846, with 1,966 selected-projection newly
covered line identities. The selected snapshot diff reports 633 newly covered
line identities, 75,764 unobserved baseline observations, and zero regressions.
Merge is conservative and named-test attribution is unavailable; the snapshot
metadata retains commit `3272b3ef49a87c2947c08b46596b442195c6a8db` as a
provenance caveat. This is bounded selected-subset evidence, not a complete
four-metric release measurement.

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

### RN-006 — Portable AVIF completion — IN PROGRESS

**Why:** The old implementation used a native AVIF bridge on some targets and
had a different effective contract on WASM. The bridge is now removed. The
final promise is one predictable, pure safe-Rust implementation on every
supported target, with every unsupported case named instead of hidden behind
a native fallback.

**Current exact state:** 314 AVIF decode/inspect/verify rows exist: 311 are
active and 3 are explicit planned gaps. All 32 AVIF encode rows are planned
because no pure-Rust encoder is wired. The exact decode gap ledger is below;
the generated source is `manifest.yaml`, and the generated counts are in
`tests/fixtures/coverage_matrix.json`.

The preceding bounded origin proof is
`coverage_vertical8x16_filter_intra_mode3_01.avif`: an 8x16 8-bit 4:2:0
`Vertical8x16` leaf with `FILTER_PRED[13/3]`, unsplit TX8x16 luma EOB 78, and
TX4x8 U/V EOB 6. A 100-candidate/10-family input-only campaign qualified 4
and promoted `f10_mosaic_04` (seed 1005), retaining non-empty luma and chroma
residuals. The pinned 187-operation dav1d trace, exact Y/U/V planes, and
Pillow RGB8 bytes match safe Rust. The exact fixture selector run
`096b83e7-ccab-4a2b-93b4-a39d051817cf` passed 1/1 in 36,099 ms at commit
`430c5beb39757ce570c2f07ea5fb2e044a580205`, ingesting snapshot
`f1f52e16-9275-43c0-9e28-7e38bea081e8`; its additive review is +8 lines,
+7 branches, +0 functions, and +281 regions with zero reported regressions.
This remains bounded subset evidence and does not close broader AV1 support.

The preceding bounded origin implementation proof is
`coverage_vertical8x16_filter_intra_mode4_tx4x4_grid_01.avif`: an 8x16,
8-bit 4:2:0 `Vertical8x16` origin leaf with `FILTER_PRED[13/4]`, transform-
luma mode 0, and a depth-two 2×4 grid of eight row-major TX4x4 luma DCT-DCT
children with EOB values `2/2/14/15/14/4/2/2`. Its two TX4x8 chroma payloads
have EOB 0, so they prove geometry and zero residuals only. The input-only
100-candidate/10-family campaign qualified exactly one deterministic
double-encoded candidate (`f03_color_ramp_08`, seed 309) without invoking
repository Rust. The pinned 361-operation dav1d trace, exact partition,
reconstructed Y/U/V planes, and independent Pillow RGB8 bytes match safe Rust;
here transform-luma mode 0 names the residual transform/CDF context, not
ordinary DC prediction—the predictor is `FILTER_PRED` mode 4. The production
path uses a dedicated checked eight-child grid and dav1d-compatible child-local
missing-edge propagation. This closes only the proven origin mode-4 grid class
and does not close general filter-intra, AV1,
`AVF-STILL-001`, or the four-metric 100% coverage gate.

The managed Coverage MCP result for this exact single-fixture selector is run
`6b78c91c-d417-4ad0-af44-96d3623e3f64`, snapshot
`75d6b159-3727-4d5b-b7f4-a4252124a171`, at implementation commit
`520e38284a263a4e07b392bf02f4b6322e6a3e31`, against baseline
`7665cda3-f4a7-4568-b871-a9d34afaa92c`. It passed 1/1 in 32,449 ms. Its
additive baseline-union deltas are +8 lines, +7 branches, +0 functions, and
+281 regions, with denominator changes of +3,459/+162/+35/+9,984 and zero
reported regressions limited to the selected subset; the selected projection
reports 385 newly covered line identities. This is bounded incremental
evidence, not the complete four-metric release measurement.

The latest bounded following-leaf proof is
`coverage_vertical8x16_following_filter_intra_mode2_01.avif`: a 16x32, 8-bit
4:2:0 split-root frame with four coded-order 8x16 `Vertical8x16` leaves. The
lower-left following leaf selects `FILTER_PRED[13/2]` with a TX8x16 luma
transform; the lower-right selects `FILTER_PRED[13/0]` and the pinned `TX[1]`
two-payload luma state. The upper-right `0/0` trace is ordinary DC, not
FILTER_PRED. All four U/V payloads use TX4x8 DCT-DCT with EOB 0. Safe Rust
matches the exact three partition records, all 1,023 entropy operations,
reconstructed Y/U/V planes, and the independent 1,536-byte Pillow RGB8
reference. The fixture SHA-256 is
`252f1ef0ac2b5af88a90d8f6c6952186ea968db0350af5e7f5c19a1465581ec2`, the
encoded-item SHA-256 is
`49620e57e1d5749c7e6ee2c76d8cf14e29709922db4c249a3da50cc8b2940bfb`, and the
RGB SHA-256 is
`403dfa0053c7a79267a72b0c4b8aad0462efb45e9baac12dd488468b3d3d924b`. The
reconstructed Y/U/V SHA-256 values are
`369245ee4125261a45aab212133114ed8e44b7d9707011d23f5e1e65c5a5e854`,
`d8eeb09b5d4d74c33d5db255677a84ef0dff4e019d8c256805097a7e91dd20cb`, and
`9fe1f341a82af9fbd72d403bfd860c658b0ee7ea3caa7be0d48ea4cc5567e59e`.
The maintained deterministic generator uses seed 1406 and the input-only
oracle never invokes repository Rust; no separate campaign identifier is
claimed for this direct topology witness. This closes only the proven
following mode-2/mode-0 class; other following modes/geometries and
`AVF-STILL-001` remain partial.

The managed Coverage MCP result for this exact single-fixture selector is run
`00dc6cfa-f1a3-45fa-8038-bdc494a4db4b`, snapshot
`fd53e0b9-0465-49cc-b44f-a2233eea41f1`, at exact implementation commit
`9061135585461cb9309f2760e525a230cec68d22`, against baseline
`7665cda3-f4a7-4568-b871-a9d34afaa92c`. It passed 1/1 in 42,067 ms with 43
filtered-out cases. The additive baseline-union deltas are +8 lines, +7
branches, +0 functions, and +281 regions, with denominator changes of
+3,553/+254/+38/+10,093; the selected projection reports 562 newly covered
line identities, 75,363 baseline hits were not observed, and zero regressions
were reported. The merge is conservative rather than exact, named-test
attribution is unavailable, and the compact snapshot retains metadata commit
`3272b3ef49a87c2947c08b46596b442195c6a8db`; the durable run record is
authoritative for implementation provenance. This is bounded selected-subset
evidence, not the complete four-metric release measurement or a global
regression claim.

The latest bounded rectangular proof is the paired
`coverage_h16x4_filter_intra_cdf14_false_01.avif` and
`coverage_v4x16_filter_intra_cdf19_false_01.avif` fixtures. They are
deterministic 16x16 8-bit 4:2:0 quality-12/speed-0 frames with root
`PARTITION_H4`/`PARTITION_V4`, four DC `Horizontal16x4`/`Vertical4x16` luma
leaves, false filter-intra decisions from CDF rows 14/19, rectangular DCT-DCT
chroma transforms, and 162-operation pinned dav1d traces. Safe Rust matches
exact partition, entropy, reconstructed Y/U/V, and Pillow RGB evidence. The H
following leaf exercises two-entry UV palette prediction, while the V following
leaf reaches the transposed rectangular path. The input-only search explored
100 candidates across 10 families and qualified 5 per orientation without
invoking repository Rust. This closes only the bounded rectangular
false-filter-CDF/palette class; broader rectangular predictors and AVF-STILL-001
remain open.

The newest bounded proof is
`coverage_square16_chroma_smooth_horizontal_01.avif`: a 32x16 8-bit 4:2:0
clipped root split with origin/following Square16 leaves at x=0/x=4. The
following leaf selects chroma SmoothHorizontal mode 11 with DCT-ADST TX8x8
U/V transforms and non-empty AC; both luma leaves use unsplit TX16x16
DCT_DCT transforms. Its pinned dav1d trace has 414 entropy operations and
partition ranges `38416/36560/62182`; safe Rust matches exact partition,
entropy, Y/U/V planes, and Pillow RGB bytes. The 100-case input-only campaign
qualified 9 candidates and promoted `SF16-F06-N01`. This closes only the
bounded following-Square16/chroma-SmoothHorizontal/DCT-ADST class.

The newest bounded chroma-angle proof is
`coverage_square8_chroma_diagonal45_angle51_01.avif`: a deterministic 16x8
8-bit 4:2:0 horizontal split with two visible Square8 leaves. The right leaf
uses coded UV mode 3 (nominal Diagonal45), angle symbol 5, delta +2, and
resolved angle 51 degrees; both U/V leaves use TX4x4 DCT-DCT with non-empty
residuals, and the right leaf has top unavailable/left available context. The
100-case input-only campaign qualified 5 candidates, all in this symbol-5/
+2/51-degree class, with zero symbol-3/delta-0/45-degree cases. Its pinned
dav1d trace has 119 entropy operations and partition ranges
`37392/43662/34871`; safe Rust matches exact partition, entropy, Y/U/V, and
Pillow RGB evidence. The fixture SHA-256 is
`49a5be35748530ce5747f0f73f24d2e1e84f94a443c72274e92cfc605351655e`, the
encoded-item SHA-256 is
`51c6a128e63997c28550a27cd4079efa31339e5d9e0324992ad93a4f4848f2d4`, and
the Pillow RGB SHA-256 is
`2b09c1b7c72c153a4ad6456a06bf63a6cd31b2b8952dcb8a78a714d0d6b0d08a`.
This provides bounded evidence for `ChromaPredictor::Diagonal45` without a
production decoder edit; it does not close general angular chroma or AV1.

The newest bounded proofs are `coverage_i444_rect_01.avif` and
`coverage_i444_rect_02.avif`: both are 16x16 8-bit lossy 4:4:4 frames with a
split root and four 8x8 terminals whose exact dav1d entropy traces,
reconstructed Y/U/V planes, and Pillow RGB output match the pinned references.
The second case has 553 operations, distinct residual/EOB states, and a
filter-intra leaf. Its fixture SHA-256 is
`fad07546f32d265ddcf03122c8b148705ebff833785b655a1b5e44bbd1d98897`; the RGB
reference SHA-256 is
`81b867c7a1081b13395b3a37a7dd79d41f43542f095f048ab71693fb471c8bbb`.

The newest bounded origin proof is `coverage_r32x32_filter_intra_probe_01.avif`:
it is a 32x32 8-bit 4:2:0 horizontal split with an origin Horizontal32x16
filter-intra leaf followed by another Horizontal32x16 leaf. The origin selects
y-mode 13/filter mode 2 and uses top=127, left=129, and top-left=128 before
the filter-intra taps. Safe Rust matches its pinned 1,168-operation dav1d
trace, exact Y/U/V planes, and Pillow RGB8 output; the fixture SHA-256 is
`2c4eb6014ec79e58d5fbc79b8e89024fbf624b918c4decee0cef790d98914c56` and the
RGB reference SHA-256 is
`979a9de4159e978b1fdbf2fb33f240da857c8a69107d635ca0a00550e459299b`.

The latest bounded following-leaf proof is
`coverage_r32x32_filter_intra_mode3_01.avif`: it is a 32x32 8-bit 4:2:0
horizontal split with filter-intra mode 3 on both Horizontal32x16 leaves.
Safe Rust reconstructs the following leaf from prepared spatial edges and
matches its pinned 4,359-operation dav1d trace, exact Y/U/V planes, and Pillow
RGB8 output. The fixture SHA-256 is
`79efb409fcfecd8cc3cd1fccb4ab22dd33190b6d91148524c03affaf9b809b29` and the
RGB reference SHA-256 is
`8593fcb0b09a3d12243a6600505f3c77262e8103d453604099a29c500c1f9495`.

The newest bounded I444 proof is
`coverage_i444_v16x32_following_filter_intra_mode3_01.avif`: it is a 32x32
8-bit lossy 4:4:4 frame with two side-by-side Vertical16x32 terminals. The
right-hand following leaf uses filter-intra mode 3, two TX16x16 luma children,
and one RTX16x32 transform for each full-resolution chroma plane. Safe Rust
matches its pinned 7,446-operation dav1d trace, exact Y/U/V planes, and Pillow
RGB8 output. The fixture SHA-256 is
`fd4465d0f0c47266f7999731081eb8f5dc1f0cb4ad74b33e38b6f013b940484e` and the
RGB reference SHA-256 is
`968e7f9616cf2236f5f94d18c48ef532319d3b338d5fab45d2dfef76a74eb2f4`.

The previous bounded following-leaf split proof is
`coverage_r32x32_following_filter_intra_split_mode0_01.avif`: it is a 32x32
8-bit 4:2:0 horizontal split whose following Horizontal32x16 leaf selects
filter-intra mode 0 and a TX16x16 luma split. Its pinned dav1d trace has
2,204 entropy operations; safe Rust matches exact Y/U/V planes and Pillow
RGB8 output. The fixture SHA-256 is
`925c90b4341178968e1ed74c2abef6148b826c77be869730b6ca9b6f0cf8f1db`, the
encoded-item SHA-256 is
`4950daa82b44d07c98f9bd5a48547f060c9e44b40ab625273955e2906d09f608`, and the
RGB reference SHA-256 is
`ea277bdded250f326c4dd7da3cd87e6ab514db4e14870857f5e79b5276a43e16`.

The newest bounded following-leaf split proof is
`coverage_r16x32_following_filter_intra_split_mode0_01.avif`: it is a 32x32
8-bit 4:2:0 vertical split whose right-hand Vertical16x32 leaf selects
filter-intra mode 0, two TX16x16 luma children, and one R8x16 U/V transform
each. Its pinned dav1d trace has partition range `35904` and 3,064 entropy
operations; safe Rust matches exact partition and entropy records,
reconstructed Y/U/V planes, and Pillow RGB8 output. The fixture SHA-256 is
`f903b64aa74c2d7d4132a43061af1e10ace4cbf1d9cc883043e223cc5de7ba54`, the
encoded-item SHA-256 is
`5e9186a2eb4e53d5dc0dcf3f42aa038f8c3ea83795400a37c0d833dd08f503bf`, and the
RGB reference SHA-256 is
`cac42b39973f40158ad8fec42946726538adddb9a0d113ed0a16b054a9189272`.
The witness also exposed and fixes the general TX8x8 split helper's incorrect
averaging of synthetic unavailable edges; ordinary DC now receives explicit
external top/left availability while internal child edges remain available.
The bounded search found no exact mode-4, mode-1, or mode-2 witness in its
100-candidate corpus; those are search non-hits, not claims of unreachability.

The newest bounded following-leaf split proof is
`coverage_r32x32_following_filter_intra_split_mode0_01.avif`: it is a 32x32
8-bit 4:2:0 horizontal split whose following Horizontal32x16 leaf selects
filter-intra mode 0 and a TX16x16 luma split. Its pinned dav1d trace has
2,204 entropy operations; safe Rust matches exact Y/U/V planes and Pillow
RGB8 output. The fixture SHA-256 is
`925c90b4341178968e1ed74c2abef6148b826c77be869730b6ca9b6f0cf8f1db`, the
encoded-item SHA-256 is
`4950daa82b44d07c98f9bd5a48547f060c9e44b40ab625273955e2906d09f608`, and the
RGB reference SHA-256 is
`ea277bdded250f326c4dd7da3cd87e6ab514db4e14870857f5e79b5276a43e16`.

The current bounded right-hand 4:2:0 proof is
`coverage_r16x32_following_filter_intra_split_mode3_01.avif`: it is a 32x32
8-bit vertical split whose right-hand `Vertical16x32` leaf selects filter-intra
mode 3, two TX16x16 luma children, and one R8x16 U/V transform each. Its
pinned dav1d trace has partition range `35904` and 4,444 entropy operations.
Safe Rust matches the exact partition record, all entropy operations,
reconstructed Y/U/V planes, and Pillow RGB8 bytes. The fixture SHA-256 is
`cd15edc5af5d16f553595f9a81a35b472e6e37b4c933a471d613b380037a76f4`, the
encoded-item SHA-256 is
`e25ae8c207af18502b43bebb7ec15f273d437e1673a24168c317040e95915408`, and
the RGB reference SHA-256 is
`d135a06efafa72998c7c55dfa25f7ec0603cf9fa2231fd874ea10074234ea186`.
The production branch passes the rectangular chroma DC-sign context used by
the entropy sentence and applies dav1d's left-only DC predictor when the top
edge is unavailable. A deterministic 100-candidate search across 10 input
families qualified two candidates; seed 211 is the promoted stable fixture and
the second qualified candidate remains non-promoted evidence. This closes
only this right-hand mode-3/R8x16 split class; broader filter-intra modes,
topologies, transform variants, and AVF-STILL-001 remain open.

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

**Source IDs:** 19 JPEG (`JPG-*`), 15 PNG (`PNG-*`), 16 GIF (`GIF-*`), 20 BMP
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
capability tables; maintain the claim ledger; maintain changelog and release
links; define governance and recovery before production reliance.

**Source IDs:** `DOC-003`, `DOC-007`, `DOC-008`.

**Done when:** every material claim has an independent source, a revision/date
scope, a validation command, and a visible proved/planned/unknown label.

## Complete open-task inventory

The following is the exact set of active roadmap IDs at this review. A task is
not complete until its ID is removed from this list and its current behavior is
moved into the appropriate contract document. The list contains **248 active
finding rows**. Resolved findings are pruned from this rendering and from
`roadmap.json`; their historical evidence remains in Git history and the
historical [roadmap](roadmap.md).

| Area | Count | Open IDs |
| --- | ---: | --- |
| Common API | 24 | `API-008`, `API-014`, `API-017`, `API-018`, `API-019`, `API-020`, `API-023`, `API-026`, `API-027`, `API-030`, `API-033`, `API-034`, `API-036`, `API-041`, `API-043`, `API-044`, `API-045`, `API-046`, `API-047`, `API-048`, `API-051`, `API-052`, `API-053`, `API-054` |
| JPEG | 19 | `JPG-002`, `JPG-003`, `JPG-004`, `JPG-006`–`JPG-021` |
| PNG | 15 | `PNG-003`, `PNG-004`, `PNG-005`, `PNG-006`, `PNG-008`, `PNG-010`–`PNG-013`, `PNG-015`–`PNG-020` |
| GIF | 16 | `GIF-002`, `GIF-005`–`GIF-007`, `GIF-009`–`GIF-012`, `GIF-014`–`GIF-021` |
| BMP | 20 | `BMP-001`–`BMP-020` |
| ICO/CUR | 20 | `ICO-001`, `ICO-002`, `ICO-004`–`ICO-021` |
| TIFF | 26 | `TIF-002`, `TIF-003`, `TIF-005`–`TIF-014`, `TIF-016`–`TIF-018`, `TIF-020`–`TIF-030` |
| WebP | 20 | `WEP-001`, `WEP-003`–`WEP-005`, `WEP-007`–`WEP-022` |
| AVIF | 30 | `AVF-001`, `AVF-003`–`AVF-006`, `AVF-008`–`AVF-009`, `AVF-011`–`AVF-016`, `AVF-018`–`AVF-020`, `AVF-022`–`AVF-035` |
| Features/package | 24 | `FTR-001`–`FTR-002`, `FTR-006`, `FTR-009`–`FTR-018`, `FTR-020`–`FTR-024`, `FTR-027`, `FTR-029`, `FTR-034`–`FTR-035`, `FTR-037`–`FTR-038` |
| Assurance | 31 | `QA-001`, `QA-002`, `QA-003`, `QA-005`, `QA-006`, `QA-008`, `QA-009`–`QA-012`, `QA-016`, `QA-019`–`QA-024`, `QA-026`–`QA-028`, `QA-030`, `QA-031`, `QA-033`–`QA-037`, `QA-039`–`QA-042` |
| Documentation | 3 | `DOC-003`, `DOC-007`, `DOC-008` |

The shorthand ranges above expand only to the IDs actually present in the
current audit. The historical roadmap is retained for provenance and original
finding context; this file is the canonical status inventory, dependency order,
and acceptance contract.

These 248 rows are not 248 equal-sized coding tasks. A row may be a small
documentation or policy decision, a new fixture, a codec algorithm, a WASM
runtime experiment, or a release gate. The reliable “how much is left” numbers
today are the exact 248 active finding rows, the current four-metric coverage
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
