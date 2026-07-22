# AVIF implementation and repository review plan

Status: implemented and parity-verified. This document is the decision record
for the AVIF work and repository-wide review requested after commit `c16709f`.

## Observable target

The oracle remains the pinned macOS arm64 Pillow wheel recorded in
`pillow-oracle.lock.yaml`:

| Layer | Fixed implementation | Observable role |
| --- | --- | --- |
| Pillow plugin | Pillow 12.2.0, tag `3c41c09` | Public defaults, mode conversion, frame timing, metadata behavior |
| Container/conversion | libavif 1.4.1, tag `6543b22` | AVIF parsing/writing and RGB/YUV conversion |
| Decoder | dav1d 1.5.3 | AV1 pixels selected by Pillow's `auto` decoder |
| Encoder | libaom 3.13.2 | AV1 bytes selected by Pillow's `auto` encoder |

The wheel reports
`dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2`. Its
`libavif.16.4.1` shared library contains the codec implementations, so matching
only the BMFF container or a different AV1 crate is not an exact parity claim.

Primary references:

- Pillow 12.2.0 `src/PIL/AvifImagePlugin.py` and `src/_avif.c`.
- libavif 1.4.1 `include/avif/avif.h`, release tag, and test data.
- Pillow's wheel build flags in `.github/workflows/wheels-dependencies.sh`:
  libavif 1.4.1, local libaom and dav1d, local libyuv and libsharpyuv,
  `AVIF_CODEC_AOM_DECODE=OFF`, and `CONFIG_AV1_HIGHBITDEPTH=0`.

## Architecture decision

AVIF remains an opt-in Cargo feature. The default JPEG/PNG/GIF/BMP/TIFF/WebP/
ICO build remains Rust-only and keeps `bytemuck` as its sole third-party
runtime dependency.

The native AVIF feature will use a small, repository-owned C ABI bridge and the
exact libavif ABI. The bridge is compiled by `build.rs` without a Cargo build
dependency and is the only code allowed to inspect libavif structs. Rust owns
all input and output buffers; the bridge never transfers allocator ownership
between C and Rust. Unsafe Rust is confined to one documented FFI module.

Library resolution order:

1. an explicit `PILLOW_RS_AVIF_LIB_DIR`;
2. an exact libavif installation exposed by `pkg-config`;
3. the pinned `.oracle-venv` wheel library in a development checkout.

Every AVIF operation checks `avifVersion() == 1.4.1`. Decode additionally
requires dav1d 1.5.3; encode additionally requires libaom 3.13.2. A mismatch is
an unavailable codec, not permission to emit non-oracle output.

`wasm32` keeps compiling with `--all-features`, but AVIF returns unsupported
because the selected implementation is a native C library. All other format
features retain their existing WASM behavior.

## Pillow call mapping

Decode follows Pillow's `_avif.c` order:

1. create a decoder and set Pillow's thread count;
2. clear `AVIF_STRICT_CLAP_VALID` and `AVIF_STRICT_PIXI_REQUIRED`;
3. select dav1d and set in-memory IO;
4. parse, then decode the requested frame;
5. use `avifRGBImageSetDefaults`, force depth 8, and select RGB or RGBA from
   `alphaPresent`;
6. call `avifImageYUVToRGB` and return exact row-major bytes;
7. preserve every frame's libavif timing in milliseconds for sequence decode.

Encode follows the same field assignments as Pillow 12.2.0:

- quality 75, speed 6, full range, YUV 4:2:0, automatic tiling;
- max threads equal to Pillow's CPU-count default, capped at 64 for libaom;
- BT.709 primaries, sRGB transfer, and BT.601 matrix without ICC metadata;
- 8-bit depth and RGB/RGBA input;
- timescale 1000 and `AVIF_ADD_IMAGE_FLAG_SINGLE` for a still image;
- libaom selected explicitly, which is equivalent to `auto` in the pinned
  wheel but removes codec-order ambiguity.

Manifest parameters will cover Pillow's AVIF save arguments rather than pass
unknown keys through silently: `quality`, `subsampling`, `speed`,
`max_threads`, `range`, `tile_rows`, `tile_cols`, `autotiling`, and
`alpha_premultiplied`. Metadata and advanced codec options are added only with
explicit byte/string mappings.

## Fixture and parity matrix

Fixtures are fixed files, never randomized at test time. The initial decode
sweep uses small upstream files that exercise independent libavif paths:

| Manifest case | Fixture source | Reason |
| --- | --- | --- |
| baseline | Pillow `hopper.avif` | Pillow's ordinary 8-bit still AVIF |
| high bit depth | libavif `colors-animated-12bpc-keyframes-0-2-3.avif` | high-bit-depth AV1 decode |
| alpha | Pillow `transparency.avif` | separate alpha item and RGBA output |
| HDR | libavif `colors_hdr_rec2020.avif` | PQ/Rec.2020 conversion path |
| grid | libavif `sofa_grid1x5_420.avif` | derived grid item assembly |
| animation | libavif `colors-animated-8bpc.avif` | five frames and timing |

The libavif fixtures above are explicitly documented upstream as covered by
libavif's BSD-2-Clause license. Pillow fixtures remain under Pillow's retained
MIT-CMU text. Exact source tags and SHA-256 values are recorded alongside the
committed fixtures before activation.

Each active decode row must match Pillow's mode, dimensions, frame count,
frame duration, and every pixel byte. Each active encode row must match the
entire Pillow-produced AVIF file byte-for-byte and then match Pillow's decoded
roundtrip pixels. Planned/skipped AVIF rows are not accepted as completion.

## Coverage closure

Coverage is measured only through Coverage MCP with LLVM line, function,
branch, and region data. The baseline at `c16709f` is 100% for all four
metrics. After AVIF lands:

1. run the approved all-features line/branch command;
2. query AVIF files first, then the repository-wide missing-region and
   missing-branch views;
3. reverse-map each reachable gap to a fixed manifest fixture or a narrow
   private-state coverage probe when no file can express an allocator/FFI
   failure;
4. remove proven dead branches instead of manufacturing impossible inputs;
5. repeat until lines, functions, branches, and regions all return to 100%.

### First AVIF coverage sweep

Coverage MCP run `3fcf59b3-df08-4b55-8e47-f07b6ef4e1b8` proved all five
manifest test groups pass, including exact AVIF decode pixels and complete
encoded-file bytes. Its snapshot `ed2a5594-3e5d-41fb-befb-03c81143fc09`
measured 27,340/27,392 lines, 1,661/1,666 functions, 3,515/3,544 branches,
and 43,146/43,294 regions. All pre-existing codec files stayed at 100%; the
new gaps are isolated to the following reverse map:

| File/path | Why it is missing | Closure input or code decision |
| --- | --- | --- |
| `avif/encode.rs`: palette conversion | No active AVIF encode row starts from `P` mode. | Add fixed opaque and transparent indexed-image rows and generate complete Pillow byte references. |
| `avif/encode.rs`: sequence validation | Still encode reaches the common encoder but cannot express frame offsets, mismatched duration counts, or different dimensions. | Exercise the public sequence offset rejection with a fixed sequence; probe the private slice-length boundary that the public validated type makes unrepresentable. |
| `avif/encode.rs`: option parsers | The active matrix covers valid numeric/options but not every boolean spelling, hexadecimal class, odd length, malformed value, and codec/advanced rejection independently. | Add manifest-visible invalid metadata/options where Pillow exposes the behavior; use a narrow parser probe for equivalent spellings that would only duplicate encoded files. |
| `avif/encode.rs`: native add/finish errors | libavif can report allocation/codec failures after creation, but a deterministic file cannot force them. | Factor result validation into pure boundary helpers and probe every status/pointer/size state; retain real successful native encode fixtures. |
| `avif/decode.rs`: presentation checks | PTS regression and non-RGB channel checks duplicate guarantees already established by the pinned C bridge. A zero timescale is rejected during bridge creation. | Remove redundant behavioral rejection; represent channel/alpha and nonzero timescale as validated native information. Probe real banker-rounding inputs directly. |
| `avif/native.rs`: post-success invariants | Width, height, frame count, channel count, timescale, and output pointer checks defend the FFI boundary but cannot be produced by a conforming libavif success result. | Centralize conversion in pure validation helpers and probe malformed bridge outputs. Keep the unsafe boundary fully checked. |
| `avif/native.rs`: frame range | The sequence decoder only requests parsed frames. | Probe the wrapper with `frame_index == frame_count`; retain real multi-frame decode fixtures. |
| `lib.rs`: unknown `ftyp` brand | Every committed AVIF fixture uses an accepted major brand. | Add a fixed unknown-major-brand assertion to the existing format metadata sweep. |

The rule for this sweep is unchanged: publicly expressible image behavior gets
a committed fixture and exact oracle evidence. Only unrepresentable FFI,
allocator, and already-validated private states use `cfg(coverage)` probes.

### Second AVIF coverage sweep

After adding palette, metadata, orientation, tiling, monochrome, advanced-aom,
and five-frame animation references, Coverage MCP run
`74ee7f0e-b5d2-4c74-a8e1-6508664bcdd0` passed all five test groups. Snapshot
`db1d3fed-76e6-49ba-ad88-a135ca9b4505` reports 27,644/27,644 lines,
1,684/1,684 functions, and 3,526/3,526 branches. It reports 43,591/43,623
regions; all 32 residual regions are line-covered, branch-covered
subexpressions in three AVIF files:

| File | Missing regions | Reverse-mapped sweep |
| --- | ---: | --- |
| `avif/encode.rs` | 22 | Exercise top-only and height-only mismatch evaluation, opaque/short-alpha/invalid palette boundaries, and valid-key/invalid-value advanced options. Replace nested iterator/`Option` parser combinators with explicit loops so every retained region corresponds to an observable decision. Remove mathematically impossible checked subtraction and CMYK downcast paths. |
| `avif/native.rs` | 4 | Validate and retain decoded pixel length at decoder creation instead of rechecking it per frame. Factor thread-count fallback into a pure helper and probe both availability states. |
| `avif/decode.rs` | 6 | Remove the redundant final sequence validation, use the already bounded frame count directly, and isolate duration-to-frame conversion so overflow is exercised at the same boundary that consumes it. Add a fixed parse-success/load-failure AVIF if the native decode error return remains a residual. |

The animation investigation also found libavif 1.4.1 writes the current wall
clock into the version-1 `mvhd`, `tkhd`, and `mdhd` creation/modification
fields. A durable raw Pillow reference therefore cannot match a later run.
The animation manifest pins `sequence_time=1`; the bridge sets libavif's public
`creationTime` and `modificationTime` fields, while the oracle generator
canonicalizes exactly those six fields in Pillow's output. Default calls keep
Pillow/libavif's current-time behavior. All remaining container and AV1 bytes
are compared without normalization.

## Repository-wide review findings to resolve

| Area | Finding | Required resolution |
| --- | --- | --- |
| Claims | `Cargo.toml`, `build.rs`, `src/lib.rs`, and README say no native libraries even when `avif` is enabled. | State precisely that defaults are Rust-only and optional AVIF uses the pinned native stack. |
| AVIF detection | Detection accepts only major brand `avif`; Pillow also accepts `avis`, `mif1`, and `msf1`. | Match Pillow's accepted major brands while still requiring a complete `ftyp` prefix. |
| Lint gate | `cargo clippy ... -D warnings` exposes thousands of configured arithmetic/cast warnings plus four immediately actionable style lints. | Keep the documented repository lint command green; fix new AVIF warnings and separately track legacy warning debt rather than hiding new failures. |
| Unsafe boundary | The main crate currently denies unsafe code globally. | Keep unsafe limited to the FFI module, add safety invariants to every block, and leave all other modules under the deny policy. |
| CI | `--all-features` currently assumes AVIF needs no system setup. | Install or expose the exact native AVIF stack before native all-feature jobs; retain a WASM all-feature build with AVIF unsupported. |
| Supply chain | BSD-2-Clause is not yet in the package expression or cargo-deny allowlist. | Retain libavif's license, add the notice, and allow BSD-2-Clause. |
| API | `encode_sequence_format` only delegates animations to GIF. | Delegate AVIF sequence encode/decode when the feature is enabled. |
| Oracle tooling | The generator has no AVIF save mapping and writes sequence references only for WebP. | Add explicit AVIF mappings and frame references; keep exact evidence validation mandatory. |

Completion means the AVIF rows are active, the exact native versions are
enforced, formatting/lint/package checks pass, Coverage MCP reports 100% for
all metrics, and the resulting commit is pushed to `main`.

## Final coverage result

Coverage MCP run `a42ade5a-900c-47c8-a903-2c0186ad0726`, snapshot
`a0ee23ab-5441-46f2-9401-1f163212b44f`, passed all five manifest test groups
with the AVIF feature enabled. The snapshot reports 27,778/27,778 lines,
1,685/1,685 functions, 3,538/3,538 branches, and 43,782/43,782 regions.

The final reverse map showed the last uncovered compiler region was not a new
algorithm branch: the low-level packed-bilevel failure was exercised directly,
but its `prepare_pixels` caller had not propagated that failure. Routing the
same fixed synthetic boundary through the production caller closed the region
without weakening validation or adding dead code.
