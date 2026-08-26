# AVIF: the Rust-only plan

Status: safe Rust runtime, bounded still-decoder subset, explicit planned gaps

Reviewed: 2026-08-26

Current claim-ledger refresh base revision: `af6f0d0c297328131ce7d7ba12c14c65fa58efe1`.
The historical Pillow parity baseline remains bound to
`36b939696415a962285d37f9120ff389aebf0205`; the current managed LLVM evidence
is recorded in `roadmap-new.md`, and its 100% release gate remains open.

AVIF is a picture box with three jobs inside it:

1. The **box** (ISO-BMFF) says what the file contains and where its pieces are.
2. The **AV1 picture** is the compressed image data.
3. Optional **metadata and extra pictures** describe color, alpha, grids, or
   animation.

The runtime in this repository is now Rust-only. It does not compile C, link
libavif/dav1d/libaom, call `pkg-config`, run a build script, or cross an FFI
boundary. `unsafe_code = "deny"` remains a workspace rule.

## The five-year-old explanation

The old implementation hired a C helper to open the hard part of the picture
box. That was useful for getting broad Pillow-like behavior quickly, but it
made a native build depend on a compiler and three external codec libraries.
It also required an unsafe Rust-to-C doorway. A browser, embedded device, or
minimal Rust build should not need that doorway just to read an image.

Pillow does not require this crate to use C. Pillow is our observable behavior
oracle: it tells us what pixels, sizes, modes, errors, and encoded bytes to
match when those things are visible to Pillow. Its own AVIF build happens to
use libavif, dav1d, and libaom; those identities remain fixture and provenance
information only. They are not runtime dependencies here.

Pure Rust is harder because AV1 is a large algorithm, and encoding needs many
choices about prediction, transforms, quantization, entropy coding, tiles,
color, and timing. The bridge was built early because it covered more cases
before the Rust algorithm existed. The correct long-term answer is to build
those parts in small, testable safe-Rust stages—not to keep the bridge hidden.

## Current public contract

| Operation | All supported targets with `avif` enabled |
| --- | --- |
| Detection | Safe Rust signature detection |
| Inspection | Safe Rust bounded container inspection and source facts |
| Still decode | Safe Rust, manifest-bounded AV1 subset |
| Still decode outside that subset | Typed `Unsupported`, never a partial image or native fallback |
| Sequence decode | A supported still may be exposed as one frame; multi-frame tracks are an explicit planned gap |
| Still encode | Typed `Unsupported`: pure-Rust encoder not implemented yet |
| Sequence encode | Typed `Unsupported`: pure-Rust sequence encoder not implemented yet |

Every valid AVIF operation that is not implemented by the current safe-Rust
backend carries `UnsupportedReason::NotImplemented`. Malformed or forbidden
container input keeps its own `Malformed`/plain `Unsupported` classification;
the runtime never uses a native decoder to turn a planned gap into a success.

The capability table intentionally reports still decode as restricted and
still/sequence encode as not implemented. Native, `wasm32-unknown-unknown`,
and `wasm32-wasip1` do not get different AVIF implementations.

The checked-in matrix currently contains 278 AVIF decode rows and 32 encode
rows:

- 271 decode rows are active: portable still reconstruction and structural
  error contracts.
- 7 decode rows are planned pure-Rust gaps: two rejected EOB controls,
  high-bit-depth reconstruction, HDR color handling, and three sequence cases.
- 0 encode rows are active; all 32 are explicit planned gaps.

The newest bounded full-resolution witnesses are
`coverage_i444_rect_01.avif` and `coverage_i444_rect_02.avif`: both are 16x16
8-bit lossy 4:4:4 frames with a split root and four 8x8 leaves. Safe Rust
matches their exact dav1d 1.5.3 entropy traces, Y/U/V planes, and Pillow RGB8
bytes. The second trace has 553 operations, distinct residual/EOB states, and
a filter-intra leaf; its fixture SHA-256 is
`fad07546f32d265ddcf03122c8b148705ebff833785b655a1b5e44bbd1d98897` and its
Pillow RGB SHA-256 is
`81b867c7a1081b13395b3a37a7dd79d41f43542f095f048ab71693fb471c8bbb`. These are
bounded I444 topology/residual witnesses, not general 4:4:4 or AV1 support.

The newest full-resolution CFL witnesses are
`coverage_i444_square16_cfl_01.avif` through `_03.avif`: deterministic 16x16
8-bit 4:4:4 origin `Square16` leaves with luma DC, coded CFL mode 13, nonzero
U/V alpha, unsplit TX16x16 luma, and DCT-DCT TX16x16 chroma residuals. Their
pinned dav1d traces contain 419, 229, and 388 entropy operations; safe Rust
matches the complete entropy traces, Y/U/V planes, and Pillow RGB8 output.
This is a bounded CFL/DCT class, not general full-resolution AV1 support.

The preceding origin witness is `coverage_square16_filter_intra_mode0_01.avif`,
a 16x16 8-bit 4:2:0 `Square16` leaf with `FILTER_PRED[13/0]`, an unsplit
TX16x16 luma transform, and TX8x8 U/V transforms. Its pinned trace has
partition range `62320` and 1,116 entropy operations; safe Rust matches the
exact entropy records, Y/U/V planes, and Pillow RGB8 bytes. Its fixture,
encoded-item, and Pillow RGB SHA-256 values are
`2fb3de2676b560d379d05782b3e57c7af028b2fdac0350364389b3f9ceb77bcc`,
`2afe883ff75f1b7ce779969b5ac7397ade8f690a11f75edeb3c534579fe9888c`, and
`4090aed7681e287536328b3ec8ee9235c8e32979b8a249824d258fd57145b008`.
This is one bounded origin mode/transform class, not general filter-intra or
AV1 completion.

The newest origin witness is `coverage_vertical8x16_filter_intra_mode0_01.avif`,
an 8x16 8-bit 4:2:0 `Vertical8x16` leaf with `FILTER_PRED[13/0]`, an unsplit
TX8x16 luma transform, and TX4x8 U/V transforms. Its pinned trace has
partition range `42232` and 584 entropy operations; safe Rust matches the
exact entropy records, Y/U/V planes, and Pillow RGB8 bytes. Its fixture,
encoded-item, and Pillow RGB SHA-256 values are
`da511e016e1e8720cb21af34b4cf41001a97af0f0380576dc47355dcd630f39a`,
`e86cc0fdfc27ec55e542a581bb22b4c619f5dfac793593ec7b276a13df6d8224`, and
`82b2100ac5f6f02e88ea931a90b2abab261b7486209ee4f63c538464c52b5c30`.
Its reconstructed luma/chroma plane SHA-256 values are
`b2785ade1a3c4756d80bf67138b50d410eb2863ff39410e94b8cfd44467baba6`,
`fe140aecdaf68c2a55f594a0a1eb6f9404e9e70f452aaee2d73fe7b98af6014a`, and
`a085afa18ac9de9d6f9c09b3fa6050395bbf1cc71d4444f3cfae4354057469e8`.
The safe-Rust fix uses the origin missing-edge values top=127, left=129,
top-left=128, adds the TX4x8 chroma CDF tables, corrects the rectangular
4:2:0 skip context and EOB scratch index, and closes only this bounded origin
class; broader filter-intra modes, transforms, and AVF-STILL-001 remain open.

The latest origin witness is `coverage_vertical8x16_filter_intra_mode1_01.avif`,
an 8x16 8-bit 4:2:0 `Vertical8x16` leaf with `FILTER_PRED[13/1]`, an unsplit
TX8x16 luma transform, and TX4x8 U/V transforms. Its pinned trace has
partition range `42232` and 559 entropy operations; safe Rust matches the
exact entropy records, Y/U/V planes, and Pillow RGB8 bytes. Its fixture,
encoded-item, and Pillow RGB SHA-256 values are
`7c04bf5be19e0e1acf757dbdda04b3fd48419a2df1dcf7a12871cdefbce99917`,
`2ce5e66bfed511611e28f06c13f3014e6863e026b9e22ea6fd2c2145e36adbde`, and
`6051c012bac9735f10fb18bfe680fc9e3582ef6acfaa295a028f02ead7a642fe`.
Its reconstructed luma/chroma plane SHA-256 values are
`d5f1f32b7f3bc6d635a7a9bd89b9efa59670ffb723b6f5ff8f7d65a0eca940c9`,
`f3238ddee04bccf67e555675f978da2a2cd114f0eac6cf751f355763e84dde85`, and
`ce452bca9cac19f45e3e2257f2ae531197097512d1bd6f76cb914c7eb34f9615`.
The existing generic safe-Rust prediction path already covered this mode, so
no production decoder change was required; this closes only the origin
Vertical8x16/mode-1/unsplit-TX8x16/TX4x8-chroma class.

The newest following-leaf witness is
`coverage_square8_chroma_diagonal113_01.avif`, a 16x8 8-bit 4:2:0 horizontal
split whose right-hand `Square8` leaf selects chroma `Diagonal113`, ADST-DCT
U/V transforms, and a split TX4x4 luma grid. Its pinned dav1d partition ranges
are `37392`, `43662`, and `63946`, and its entropy trace has 304 operations.
Safe Rust prepares the origin angular luma edges, disables the quantization
matrix for IDTX as AV1 requires, and supplies the following leaf's left-only
chroma edge; it matches the exact partition, entropy, Y/U/V, and Pillow RGB
evidence. Its fixture, encoded-item, Pillow RGB, and Y/U/V plane SHA-256
values are
`c014c0d3a2108ab2e97b3dd7575985dec029390b049d08335faa8b3d2aad31f7`,
`6940c3d9ff199ebb028dda748b79fb56c649c4438f0bc4166163a498eabf5c8c`,
`05f6f725de2e882646a7bf059b444ffc26e2a7b048ad09f573890222bd029462`,
`8cd00fa1153aeaf0204349c8989237f0ee89ab01e7edcd6b093b3a8851f96380`,
`0fe0d5856d2af6835aae223b63f94c911c7fb438b6c540f4009429a93097a31c`, and
`4a49e2754a4657f7f7f7a60da66d756e707c1a30f9c8e938ca82502c54e85ea0`.
This closes only the right-hand Square8/chroma-Diagonal113/ADST-DCT class;
broader AV1 partition states, chroma modes, transforms, and AVF-STILL-001
remain open.

The newest following-leaf witness is
`coverage_vertical8x16_chroma_horizontal_01.avif`, a deterministic 16x16
8-bit 4:2:0 vertical split with origin UV mode 0 (DC) and following UV mode 2
(Horizontal). Both leaves use one R4x8 U/V pair; the origin is DCT-DCT, the
following leaf is DCT-ADST, both have non-empty AC, and the following UV angle
symbol is recorded as `3` (delta `0`, absolute angle `180`). The bounded
input-only campaign evaluated 100 candidates across 10 families, qualified
exactly two cases in this same zero-delta zone, and promoted one. Its pinned
trace has 149 entropy operations and root range `57408`; safe Rust matches the
exact partition, entropy, Y/U/V planes, and Pillow RGB bytes. The fixture,
encoded-item, Pillow RGB, and Y/U/V SHA-256 values are
`a4f4638ba60bc5ac4a5e15e161135a7cc51d521801dccbe83a1cdfbfb3cec00b`,
`71cba042f6b3d85ff48ace58a02c692c58d01f0ddd248f5175ee2218ae928d6f`,
`fe06a9e4a35a7a479f62725e4c0716a0f5133849e8d1e351c866506fdbae680f`,
`5a5f307aa9ce504d9235634f15cf382e8914c49fbd8dd4d4c47136c917886f7b`,
`a827fede50b0209c63cb5f591b182580a3e0d3cc153c685d5e8317261b768b48`, and
`fcd95e665768fca57727608db3f4d9d7fbafb50dfce932322f30cf68e1021336`.
The durable campaign report is
`tests/fixtures/outputs/av1_search/coverage_vertical8x16_chroma_horizontal_campaign_01.json`
(SHA-256
`3556712a1a4f2a9a79fb48072dd1108582e4220f0aafe935faab2849d287463a`). This
closes only the exact-horizontal 4:2:0 class; nonzero-angle Horizontal zones
and broader AVF-STILL-001 remain open.

Two separate input-only searches for the adjacent right-hand Square8 chroma
Diagonal67 class were bounded at 100 candidates each and found no coded UV
mode-8 sentence. The first report is
`tests/fixtures/outputs/av1_search/coverage_square8_chroma_diagonal67_campaign_01.json`
with SHA-256
`1a54c0803c30443bd7ca2fd24a70be2e146c1235b878c19bd3ef0c5b8f66a977`; the
chroma-biased second report is
`tests/fixtures/outputs/av1_search/coverage_square8_chroma_diagonal67_campaign_02.json`
with SHA-256
`fb14d289c4e0f4d200c673687228a33f1f682eb235605005dc8f632f1dab4af7`.
Both retain the pinned oracle versions and strict predicates. This is no-hit
evidence, not proof of unreachability; no speculative production change is
authorized and Diagonal67 remains a planned gap.

One narrow internal regression contract now consumes six terminal blocks of
the 128×128 lossy baseline in safe Rust: the first exact 16×16 coded square is
decoded in AV1 payload order, then two following top-row 8×8 blocks consume
shared adaptive CDF state, mode-zero filter-intra prediction, safe
luma/chroma prediction including Smooth chroma, scalar DCT-DCT, H-DCT, ADST-DCT,
DCT-ADST, and ADST-ADST transforms, the general 4×4 subsampled chroma
coefficient sentence, and the legal EOB-bin-three/four branch. The production
path exercises the exact closed square plus this bounded top-row continuation,
while the baseline row remains planned. This is still bounded leaf evidence,
not a full-frame claim:
the complete baseline needs all partition/block state, every filter-intra
mode and edge case, loop filtering, raster assembly, color conversion, and
independent full-frame pixel evidence.

The permanent entropy contract also walks to the next partition terminal and
records its exact geometry: an 8×16 coded block at block coordinates `(6, 0)`.
A focused safe-Rust contract now consumes that terminal's exact rectangular
coefficient/CDF sentence, reconstructs its Z2/angle-154 DCT-ADST 8×16 luma
plane and skipped 4:2:0 R4×8 chroma planes, and checks the following terminal
boundary `(0, 4, 4, 4)`. This is bounded syntax/leaf evidence, not a full
baseline claim: the production walker still stops before this terminal, and
full-frame above/left state, loop filtering, raster assembly, and independent
raw-frame pixel evidence remain explicit pure-Rust gaps. It is not a native
fallback and must not be “fixed” by decoding two 8×8 blocks.

The transform groundwork for that deliverable is now in
`src/codecs/avif/av1/transform.rs`: a safe scalar 16-point inverse DCT pass and
the AV1-scaled 8×16 DCT-ADST wrapper, with zero, bounded-input, and
non-repeated-8×8 regression tests. The focused contract uses these helpers and
proves exact rectangular skip/transform/EOB/coefficient CDF consumption and
checked plane dimensions. They are deliberately not wired into the full-frame
production walker yet; persistent frame state, complete 4:2:0 chroma geometry,
neighbor windows, and independent pixel evidence remain planned.

The sample-depth boundary is also isolated in
`src/codecs/avif/av1/sample_depth.rs`. Its checked `SampleDepth` type validates
nominal 8-, 10-, and 12-bit ranges and uses bit truncation—not rounded
normalization—for the current 8-bit transfer boundary. The current materializer
exercises it for its 8-bit color and auxiliary-alpha samples; this is a
reusable conversion prerequisite, not support for the 12-bit animated
fixture. Full high-bit-depth AV1 reconstruction, HDR range/transfer handling,
and sequence presentation remain planned.

The alpha prerequisite now has an explicit safe-Rust frame boundary: the AV1
block parser distinguishes monochrome lossless syntax from 4:4:4 color syntax,
walks all 37 terminal leaves in the committed `alpha.avif` auxiliary item, and
places them into a checked 64×64 one-plane canvas. The production sample
validator retains that complete auxiliary plane using the logical tile span
and parsed frame context; it does not use a fixture byte offset or a native
decoder. Tests cover the full leaf order, geometry-derived above/left neighbor
state, exact plane extent, and production retention. The paired `with_alpha`
fixture is now active: the 64×64 primary color frame and auxiliary plane
compose to exact RGBA8 bytes with the recorded independent reference. Broader
dimensions, high depth, and premultiplied relationships remain planned.

Regenerate or inspect the authoritative status with:

```bash
jq '.formats.avif, .summary' tests/fixtures/coverage_matrix.json
```

The active rows do not mean “all AVIF works.” They mean only that each listed
case has a checked-in contract that the current safe Rust implementation can
prove.

The planned rows are executable contracts too: the matrix test checks every
decode and encode row for a concrete gap reason, a named pure-Rust work item,
`former_native_only: true` provenance for every row previously covered by the
removed bridge, no claimed pixel or encoded-output reference, and a typed
safe-Rust `Unsupported` result (with the named repeated-frame-ID sequence case
intentionally returning `Malformed`). The generated matrix drops that marker
as soon as a row becomes active, so it cannot silently survive a real closure.

The current matrix contains 39 former-native AVIF rows: 7 decode gaps and 32
encode gaps. Every remaining row is explicitly planned until pure safe Rust and
independent compatibility evidence exist.

## Exact planned gaps

These are the 7 decode rows that must become real safe-Rust behavior before
they can move from `planned` to `active` in `manifest.yaml` and
`coverage_matrix.json`:

| Category | Planned rows | Why it is missing |
| --- | --- | --- |
| Adjacent entropy syntax | `portable_lossy_420_q99_eob_bin_control`; `portable_lossy_420_q99_eob_base_control` | Safe Rust now proves legal EOB-bin-five and EOB-bin-six 8×8 AC classes, including moving coefficient-context lookup, matrix-10 dequantization, and independent pixel fixtures. These two byte mutations are rejected by the independent Pillow oracle, so they remain explicit negative planned controls rather than being widened into successful decoding. |
| Sample depth | `high_bitdepth` | `high_bitdepth` needs 12-bit reconstruction and conversion. The 64×64 `with_alpha` primary/auxiliary pair is active with exact RGBA8 evidence; broader alpha dimensions, depths, and relationships remain future work. |
| Color pipeline | `hdr` | HDR transfer/primaries/matrix application is not implemented; the current public color path is the narrow checked 8-bit BT.601 full-range class. |
| Animation and tracks | `animated`; `animated_error_resilient`; `error_animated_repeated_frame_id` | Track references, timing, and sequence presentation are not implemented; the safe validator now rejects the repeated current frame ID in the named error fixture. |

The remaining active AVIF error rows prove that the safe parser rejects
malformed or forbidden structure with a stable typed result. They are not
claims that the corresponding valid feature is supported.

All 32 encode rows are planned, including RGB/RGBA and luminance conversion,
quality and subsampling, alpha, monochrome, tiles, metadata, orientation,
advanced options, animation, and option-error behavior. Option parsing remains
a public API contract; producing an AVIF file remains a planned codec task.

## What has already been built in safe Rust

- AVIF brand detection and bounded ISO-BMFF inspection.
- FileTypeBox, item locations, property associations, color declarations,
  codec declarations, alpha relationships, and raw metadata retention as
  source facts.
- AV1 sequence/frame-header validation, tile-boundary checks, scalar entropy
  decoding, and a bounded safe-Rust partition walker with adaptive CDF and
  above/left context state that reaches checked terminal block footprints in
  AV1 payload order for every legal partition shape. Because block syntax is
  interleaved between sibling partition symbols, the production walker stops
  at the first unsupported terminal block; it does not claim a full baseline
  tree or image decode. The documented portable partition/reconstruction
  classes also include a
  scalar safe-Rust 8×8 DCT-DCT inverse transform, the checked eight-bit luma
  dequantization table and legal 8×8 EOB-bin-five/six AC reconstruction, a
  safe rectangular R8×16 coefficient sentence, DCT-ADST reconstruction, and
  skipped 4:2:0 R4×8 chroma reconstruction in the focused baseline terminal
  contract,
  checked scalar CDEF block kernel, and RGB conversion
  for the active subset. The
  transform and table are reusable prerequisites; they do not by themselves
  widen the accepted EOB syntax or make the 128×128 baseline a full-frame
  decode, and the CDEF hook remains disabled until per-block selection syntax
  is retained by the frame walker.
- A checked safe-Rust frame canvas now places reconstructed luma/chroma planes
  with overflow, subsampling-alignment, bounds, overlap, and completeness
  checks, converts the entropy walker's four-by-four-unit leaf coordinates to
  pixel origins, can crop a coded cell to a checked top-left visible rectangle,
  and validates a complete grid/tile cell batch before copying any sample. This is
  the reusable atomic assembly primitive for baseline stills, tiles, grids,
  and auxiliary images; it does not by itself claim that those AV1 syntax
  classes are decoded.
- A checked pure-Rust auxiliary-alpha composition boundary for the portable
  class: alpha must be monochrome, match color dimensions and bit depth, and
  have exactly one decoded sample per color pixel. Unsupported alpha never
  silently degrades to RGB; the `with_alpha` fixture now proves the primary
  color payload, auxiliary plane, RGBA pairing, and raw RGBA parity.
- Cancellation, limits, destination-buffer, source-lifecycle, and typed-error
  boundaries shared with the other codecs.
- A capability table that is the same architecture on native and WASM.

Source facts are not pixel processing. For example, retaining a `grid` box
proves that the parser saw a grid; it does not mean that the Rust decoder has
composed the grid into one image. The feature-gated source-contract tests keep
that distinction explicit.

## Pure-Rust implementation order

1. **Finish still AV1 syntax.** Connect the new partition tree to block
   prediction and residual decoding, then add the missing transform,
   loop-filter/restoration, multi-tile, 4:2:2/4:0:0, and 10/12-bit classes.
   Each class gets an independent fixture and a bounded work/cancellation
   contract.
2. **Finish still-image composition.** Decode and combine alpha items and
   grids; implement the declared color pipeline for HDR and other supported
   sample layouts; preserve metadata without accidentally changing pixels.
3. **Implement sequences.** Add track/sample-table parsing, frame IDs,
   references, timing, default-image rules, disposal/blend semantics, random
   access, and limits. The validator already rejects the repeated-ID error
   case safely; first-frame materialization and full presentation remain
   planned. A single-frame fallback is not sequence completion.
4. **Implement encoding.** Write the AVIF container in Rust; convert RGB/RGBA
   safely to the supported YUV layouts; add an intra AV1 encoder, transforms,
   quantization, entropy/CDF coding, tiles, alpha, metadata, and sequences.
   Determinism and Pillow-visible output are release gates, not assumptions.
5. **Verify and release.** Activate one manifest category at a time only after
   native and WASM semantic tests pass, active rows match Pillow where visible,
   independently decode emitted bytes, strict checks pass, and line/branch/
   function/region coverage is 100% for the measured release surface.

The work is intentionally ordered this way: a container writer without a
correct AV1 payload is not an encoder, and a sequence API without timing and
frame-reference semantics is not an animation decoder.

## Oracle and third-party policy

Pinned Pillow/libavif/dav1d/libaom outputs and the independent AV1 traces in
`tests/fixtures/` are reproducible reference material. The copies under
`third_party/` supply license/provenance information and source-derived
algorithm references where documented. They are not compiled, linked, or
loaded by the crate.

When a future Rust encoder emits bytes, validation must include an independent
AVIF decoder and the pinned Pillow observable result. Byte identity is claimed
only when the algorithm and parameters genuinely match; a valid but different
AVIF file is not falsely reported as Pillow byte parity.

## Acceptance rule for every gap

A planned row becomes active only when all of these are true:

- the implementation is pure safe Rust;
- no native build/link/environment path is needed;
- the public result is correct for the declared case;
- a fixture or Rust-only contract exercises the real behavior;
- Pillow parity is used only for fields Pillow can observe;
- the same behavior is tested on the claimed native/WASM targets; and
- strict formatting, Clippy, rustdoc, tests, and managed coverage are fresh at
  the same source revision.

The roadmap and manifest are the source of truth. A native oracle result,
fixture presence, or successful container inspection never silently closes a
planned decoder or encoder gap.
