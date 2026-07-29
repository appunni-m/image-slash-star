# Portable AVIF Implementation Progress

Date: 2026-07-29

Status: portable container inspection, borrowed sample extraction, AV1
sequence/frame parsing, tile boundaries, and scalar entropy decoding accepted;
portable closed-class 4x4, 4x8, 8x4, 8x8, 12x12, and 16x16 pixel decoding
plus 12x16 and 16x12 visibility accepted; full portable decode/encode remains
in progress.

## Scope And Non-Negotiable Constraints

The final `avif` feature must decode, encode, and inspect AVIF on
`wasm32-unknown-unknown` without a native library or a new crate dependency.
`bytemuck` remains the only approved Rust library dependency.

This work is codec implementation, not image processing. ISO-BMFF parsing,
AV1 entropy decoding, prediction, inverse transforms, in-loop filtering,
sample reconstruction, codec-mandated YUV/RGB conversion, alpha combination,
and deterministic AV1 encoding are in scope. Resizing, cropping, arbitrary
color adjustment, compositing unrelated images, and other reusable raster
operations are prohibited.

The pinned native stack remains an intermediate oracle only. It must disappear
from the published build/runtime contract before AVIF is declared portable.

## Authoritative References

The implementation is checked against:

- libavif `v1.4.1`, exact commit
  `6543b22b5bc706c53f038a16fe515f921556d9b3`;
- dav1d `1.5.3`, exact commit
  `b546257f770768b2c88258c533da38b91a06f737`, for decoded AV1 behavior;
- libaom `3.13.2`, exact commit
  `ad44980d7f3c7a2605c25d51ea96946949000841`, for deterministic encoder
  behavior;
- Pillow `12.2.0` for the public success/error, decoded-mode, pixel, sequence,
  metadata, and encoded-byte oracle pinned by `pillow-oracle.lock.yaml`.

The first container slice follows these libavif reference locations:

- `src/stream.c:248-305`: ISO-BMFF box header, large-size, size-zero, and
  truncation rules;
- `src/read.c:2471-2480`: `ispe`;
- `src/read.c:2482-2489`: `auxC`;
- `src/read.c:2648-2693`: `av1C`;
- `src/read.c:2913-3145`: `ipco` property parsing and `ipma` association
  semantics;
- `src/read.c:3148-3170`: `pitm`;
- `src/read.c:3190-3243`: `iprp`;
- `src/read.c:3246-3328`: `infe` and `iinf`;
- `src/read.c:3333-3415`: `iref`;
- `src/read.c:3515-3595`: track dimensions and media timescale;
- `src/read.c:3655-3680`: `stsz`;
- `src/read.c:3765-3858`: track sample-table traversal;
- `src/read.c:3936-4050`: `trak` and `moov`;
- `src/read.c:4775-5031`: `ftyp` compatibility and required `meta`/`moov`
  semantics.

Reference source is downloaded only into `/private/tmp/libavif-1.4.1`,
`/private/tmp/dav1d-1.5.3`, and `/private/tmp/aom-3.13.2`; it is not copied
wholesale or added to the repository.

## Committed Fixture Map

The committed fixtures establish the following first-slice requirements:

| Fixture | Brand | Primary geometry | Encoded depth | Alpha evidence | Presentation frames |
| --- | --- | ---: | ---: | --- | ---: |
| `baseline.avif` | `avif` | 128x128 `ispe` | 8-bit `pixi` | none | 1 |
| `alpha.avif` | `avif` | 64x64 `ispe` | 8-bit `pixi` | item 2 has alpha `auxC` and `auxl` targets primary item 1 | 1 |
| `grid.avif` | `avif` | 80x80 primary grid `ispe` | 8-bit `pixi` | alpha auxiliary tile items target the primary grid's `dimg` children | 1 |
| `hdr.avif` | `avif` | 200x200 `ispe` | 10-bit `pixi` | HDR gain-map `auxC`, not alpha | 1 |
| `animated.avif` | `avis` | 150x150 `ispe` | 8-bit `pixi` | none | pict-track `stsz` count 5 |
| `10bit.avif` | `avis` | 64x64 `ispe` | 12-bit `pixi` | alpha item plus auxiliary track | pict-track `stsz` count 5 |
| `unsupported_major_brand.avif` | truncated `heic` `ftyp` | invalid | invalid | invalid | rejected |

The filename `10bit.avif` is historical; its committed bytes declare 12-bit
color and alpha planes. Inspection must report the encoded declaration, not
infer depth from the filename or the final Pillow 8-bit output buffer.

## Slice 1: Portable ISO-BMFF Inspection

### Parser model

Add a private, feature-scoped AVIF container module consumed immediately by
`avif::inspect`. It must:

1. parse the first `ftyp` before variable-length data;
2. support 32-bit sizes and `largesize`;
3. accept size zero only for a final top-level box;
4. reject headers smaller than their encoded header or larger than their
   enclosing slice;
5. bound box, property, item, reference, association, and track counts;
6. parse `meta` as a full box;
7. retain `pitm`, `infe`, `ipco`, `ipma`, and `iref` relationships;
8. select the primary item's `ispe`, `pixi`, and `av1C` properties;
9. distinguish the standard alpha auxiliary URN from gain maps and other
   auxiliary data;
10. follow primary `dimg` references when detecting grid alpha;
11. parse `moov`/`trak`/`mdia`/`hdlr`/`minf`/`stbl`/`stsz`;
12. take frame count from the main `pict` or `vide` track, never sum color and
    auxiliary tracks; and
13. require `meta` when `avif` is declared and `moov` when `avis` is declared.

No independent bit reader is added in this slice. The four-byte `av1C` header
can be decoded directly, and the future AV1 bit reader will be introduced only
when the sequence-header parser consumes it.

### Output contract

Portable inspection returns:

- `ImageFormat::Avif`;
- the primary display width and height;
- `ImageMode::Rgb8` or `ImageMode::Rgba8`, matching Pillow's materialized
  output mode;
- the encoded primary-plane depth from `pixi`, falling back to `av1C`;
- no palette;
- exact `frame_count`; and
- `is_animated == frame_count > 1`.

The same parser runs on native and WASM. Native inspection must no longer ask
libavif for these fields, preventing target-specific semantic drift.

### Acceptance criteria

Before the slice is complete:

1. all committed AVIF success/error rows pass through direct and auto-detected
   inspection;
2. native and WASM compile the same inspection code;
3. malformed box-size, full-box, association, reference, property, and track
   inputs are exercised through manifest fixtures when Pillow can express the
   public outcome, otherwise through the private coverage hook;
4. strict all-feature Clippy passes;
5. the all-feature `wasm32-unknown-unknown` check passes; and
6. Coverage MCP reports 100% line, branch, function, and region coverage.

### Accepted result

Slice 1 was accepted on 2026-07-29:

- all 1,027 active manifest cases pass, including every AVIF success and error
  row;
- native and `wasm32-unknown-unknown` builds use the same repository-owned,
  bounded inspection parser;
- direct and auto-detected AVIF inspection retain exact format, dimensions,
  materialized mode, encoded bit depth, alpha status, and frame count;
- malformed public outcomes remain fixture-driven, while otherwise
  unreachable private truncation and budget states are exercised by the
  coverage-only parser hook;
- strict all-feature Clippy and the all-feature WASM check pass; and
- final Coverage MCP snapshot `81048b6b-0f65-45d1-9680-ca941b2eb916` reports
  29,710/29,710 lines, 4,066/4,066 branches, 1,533/1,533 functions, and
  50,175/50,175 regions.

This does not declare AVIF fully portable. On WASM, detection and inspection
now work; pixel decode, sequence decode, still encode, and sequence encode
remain unavailable until their later portable slices replace the native
bridge.

## Slice 2: Borrowed AV1 Sample Extraction

Status: accepted on 2026-07-29.

### Boundary

This slice extends only the private AVIF container parser. It identifies the
encoded AV1 payloads that a later AV1 decoder will consume. It does not parse
AV1 syntax, reconstruct samples, allocate a pixel canvas, convert color, or
expose a new public operation.

The canonical input remains the caller's `&[u8]`. Extracted payloads borrow
that input and retain ranges into it:

```text
AvifPayload<'input>
  ├─ still color plane → one item or ordered grid-cell items
  ├─ still alpha plane → absent, one item, or ordered grid-cell items
  └─ sequence tracks
       ├─ color samples → one borrowed span per presentation sample
       └─ alpha samples → optional matching borrowed spans
```

An item may contain multiple extents. Those extents remain an ordered list of
borrowed byte spans; the parser must not concatenate encoded data. A future
segmented AV1 bit reader will consume the spans directly. Track samples are
also represented as borrowed spans. Offsets are stored as checked
`Range<usize>` values, not raw pointers.

The data model must retain only codec inputs:

- item or track identity;
- color versus codec-declared alpha role;
- ordered encoded byte spans;
- `av1C` configuration bytes;
- sync-sample status;
- duration in source timescale units; and
- the nonzero media timescale.

No type from this slice is public outside the AVIF codec.

### Reference mapping

The implementation must follow libavif 1.4.1 commit
`6543b22b5bc706c53f038a16fe515f921556d9b3`:

- `src/read.c:1979-2103` — `iloc` versions, field widths, construction
  methods, item IDs, base offsets, and extents;
- `src/read.c:1416-1527` — `idat` versus file-backed extent resolution and
  extent bounds;
- `src/read.c:354-366` — `stts` sample-duration lookup;
- `src/read.c:520-607` — expansion of chunks into ordered encoded samples;
- `src/read.c:3566-3595` — `mdhd` timescale;
- `src/read.c:3597-3620` — `stco` and `co64`;
- `src/read.c:3622-3653` — `stsc`;
- `src/read.c:3655-3675` — `stsz`;
- `src/read.c:3677-3694` — `stss`;
- `src/read.c:3696-3712` — `stts`; and
- `src/read.c:3765-3805` — sample-table child dispatch.

The repository diagnostic `scripts/inspect_avif_bitstreams.py` must derive the
committed fixture's item extents, track samples, durations, and SHA-256 values
without importing Pillow or the Rust implementation. Its output is the
independent boundary oracle for this slice.

### Required parsing

The item path must:

1. retain the absolute payload range of a unique `idat`;
2. parse `iloc` versions 0, 1, and 2;
3. accept only 0-, 4-, or 8-byte offset, length, base-offset, and index
   fields, matching libavif;
4. accept construction method 0 (file) and 1 (`idat`) and reject method 2;
5. reject zero item IDs, duplicate locations, nonzero reserved fields,
   unsupported data references, offset overflow, length overflow, and
   out-of-input extents;
6. preserve all extents in declared order without copying their bytes;
7. resolve the primary `av01` item or every `dimg` child of a primary `grid`
   item in reference order; and
8. resolve alpha through the existing `auxC` plus `auxl` relationship,
   including per-cell alpha for a color grid.

The track path must:

1. parse a unique `mdhd`, `stco` or `co64`, `stsc`, `stsz`, optional `stss`,
   and optional `stts`;
2. require the first `stsc.first_chunk` to be 1 and every later value to be
   strictly increasing;
3. expand every chunk using the most recent `stsc` entry;
4. reject zero-sample chunks, missing sizes, sample-count mismatches, zero
   sample sizes, arithmetic overflow, and spans beyond the input;
5. treat frame zero as sync when `stss` is absent, matching libavif;
6. map each presentation sample to its `stts` duration, using libavif's
   final-entry fallback and default duration of one when `stts` is absent;
7. select the first `pict` or `vide` track as color and a matching `auxv`
   `auxl` track as alpha; and
8. require matching color/alpha sample counts before a later decoder can pair
   the planes.

All box, item, extent, chunk, mapping, sample, timing, and reference records
are charged to the existing bounded parser budget before allocation.

### Fixture attack

The diagnostic must first record the current committed inputs:

| Fixture class | Required extraction evidence |
| --- | --- |
| `baseline.avif`, `alpha.avif`, `hdr.avif` | primary item extent; alpha item extent when present |
| `grid.avif` | ordered color grid cells and their corresponding alpha cells |
| `animated.avif`, `10bit.avif` | color and alpha track sample ranges, sync flags, durations, and timescale |
| `unsupported_major_brand.avif` | rejection before any payload is exposed |

### Fixture boundary findings

Running `python3 scripts/inspect_avif_bitstreams.py` against the committed
fixtures produces the following independent byte-boundary oracle.

Item-backed payloads:

| Fixture | Role/item | File span | SHA-256 |
| --- | --- | --- | --- |
| `baseline.avif` | color 1 | 282+2795 | `be0bf650b8612533577e4b47a60989bb3f092cfb256b81f3bd84e3b5ce8ea199` |
| `alpha.avif` | color 1 | 727+5714 | `1cf5dcf897202917ef22dafa24474b08c16a99586633249d93489a1b26c5dca9` |
| `alpha.avif` | alpha 2 | 457+270 | `9fe5f18ee67731045491acb65eaa4ed04f10189fa1ef36979709c3aa78aa0fee` |
| `hdr.avif` | color 1 | 687+5378 | `aeb47204621cee679da8eafa42e3face431c0f3d55b99f5a4542189aa6559a1c` |
| `grid.avif` | color cell 2 | 1467+781 | `f3ba37cb451cf5cdab39cb3560a0e6e2ac8461cd387f30fb529e25e9347d7756` |
| `grid.avif` | color cell 3 | 2248+125 | `1b7cae0bdad03ae46f0732b4cb6d89473cae3f8bbf25cae14f8cc1352723defd` |
| `grid.avif` | alpha cell 5 | 635+589 | `fed30bb522f5b0eeb94fe82690ba95e2bc0147c4543feeae0ec1877fe1c8d9df` |
| `grid.avif` | alpha cell 6 | 1224+243 | `11c9174261542871287d9282d964d81af5c3413cf2ac8955f9bc406a820a6632` |
| `animated.avif` | color 1 | 1023+39 | `8ec6d1190463e5d3defa50a8f270a2daa51be85e636ee1e482be91be2aedebeb` |
| `10bit.avif` | color 1 | 2022+39 | `ff737aa321c2a7e5b458ca833be7cceffb19e06baafbc42cf76d649ec879611c` |
| `10bit.avif` | alpha 2 | 1852+29 | `3864ca8289696be458cc3d807cf68f036eea460f55310ed65f6ed062ee180f1f` |

All committed item extents use construction method 0 and contain one extent.
The parser still must support ordered multi-extent items and construction
method 1 because both are valid libavif inputs and are required before the
future segmented AV1 reader can be correct.

Item-level AV1 configuration boundaries are:

| Fixture | Role | `av1C` span | Bytes |
| --- | --- | --- | --- |
| `baseline.avif` | color | 228+4 | `81000c00` |
| `alpha.avif` | color | 294+4 | `81200000` |
| `alpha.avif` | alpha | 359+4 | `81001c00` |
| `grid.avif` | color cells | 483+4 | `81200000` |
| `grid.avif` | alpha cells | 565+4 | `81001c00` |
| `hdr.avif` | color | 436+4 | `81204000` |
| `animated.avif` | color | 240+4 | `81000c00` |
| `10bit.avif` | color | 302+4 | `81406800` |
| `10bit.avif` | alpha | 347+4 | `81407c00` |

`animated.avif` color track 1 has timescale 30 and `av1C` bytes `81000c00`:

| Sample | File span | Sync | Delta | SHA-256 |
| ---: | --- | --- | ---: | --- |
| 0 | 1023+39 | yes | 1 | `8ec6d1190463e5d3defa50a8f270a2daa51be85e636ee1e482be91be2aedebeb` |
| 1 | 1062+113 | no | 1 | `a4a77230e45708a7cbe33be90201dd715803a6014c54e7c7df81621bacff24ad` |
| 2 | 1175+5 | no | 1 | `e9e33cf3c379510fa68d4b4fc7d0bfb5677c717177cf5831033332b14cc15bdb` |
| 3 | 1180+30 | no | 1 | `5990be45371440951b1a8f7e8b50f65c48f447a7eabf5aec5b2f21e228a0adb7` |
| 4 | 1210+25 | no | 1 | `2f9b713fe83554cbc9701e85e93b6453d2a7e43edc5c19ea3fea2e457c926aa6` |

`10bit.avif` color track 1 has timescale 1 and `av1C` bytes `81406800`:

| Sample | File span | Sync | Delta | SHA-256 |
| ---: | --- | --- | ---: | --- |
| 0 | 2022+39 | yes | 1 | `ff737aa321c2a7e5b458ca833be7cceffb19e06baafbc42cf76d649ec879611c` |
| 1 | 2061+36 | no | 1 | `9e2b5e2e757650a2ec2be0e59d0d69b86eb3b90864455ebfe3e1eba878026312` |
| 2 | 2097+38 | yes | 1 | `e794df340052ea3cc216a4fe4570124709ee51cf69ab7861056ed4053e8b385c` |
| 3 | 2135+103 | yes | 1 | `ec667405796e815656e28f44c6ffe547530945f65ccc062eb2ae3b5a5df6f391` |
| 4 | 2238+29 | no | 1 | `fa2e49efd8ed07385a4e5cda0e334a799e3fee83151410167138a066ac0d03b1` |

Its alpha track 2 targets track 1, has the same timescale, and has `av1C`
bytes `81407c00`:

| Sample | File span | Sync | Delta | SHA-256 |
| ---: | --- | --- | ---: | --- |
| 0 | 1852+29 | yes | 1 | `3864ca8289696be458cc3d807cf68f036eea460f55310ed65f6ed062ee180f1f` |
| 1 | 1881+20 | no | 1 | `147ead8dd7ba93146d730d077b288c81a01fbbaa02caed1af747867d87aad997` |
| 2 | 1901+26 | yes | 1 | `1a1ef2a1721db3891cea023de75d0baf8bb9ce55e2723bf0c05c5740f0e8724b` |
| 3 | 1927+44 | yes | 1 | `f887b3e4e3459a8a71691dba043758d5e0c0d951617ec92d44753576052b5274` |
| 4 | 1971+51 | yes | 1 | `32c510176873723bbe0920369f0890d147904079990d24b9fa3a754441ec20e4` |

`unsupported_major_brand.avif` is structurally truncated: its declared first
`ftyp` box exceeds the committed input. It is rejected before an item or track
payload can be exposed.

Public malformed behavior must be represented by committed manifest fixtures
once the portable decoder consumes this extractor. Coverage-only probes are
permitted only for private field-width, overflow, budget, or synthetic
multi-extent states that Pillow cannot expose independently.

### Acceptance criteria

Before this slice is accepted:

1. the diagnostic output for every committed AVIF fixture is recorded in this
   document and independently reproducible;
2. the Rust extractor returns the exact same ordered spans, SHA-256-observable
   bytes, sync flags, durations, and timescale;
3. no encoded payload bytes are copied or merged;
4. malformed arithmetic and table relationships fail before returning a
   partial payload;
5. existing native Pillow pixel/byte parity remains unchanged;
6. native, no-default-feature `avif`, and all-feature
   `wasm32-unknown-unknown` checks pass;
7. strict all-target, all-feature Clippy passes; and
8. Coverage MCP reports 100% line, branch, function, and region coverage.

### Accepted result

Slice 2 was accepted on 2026-07-29:

- the private extractor follows the pinned libavif item-location and
  sample-table behavior and retains only borrowed byte spans, configuration
  spans, sync state, durations, and timescale;
- committed baseline, alpha, grid, HDR, animated, and high-bit-depth fixtures
  match the independent diagnostic's exact ordered item and track boundaries;
- `iloc` versions 0, 1, and 2, file and `idat` construction, 0/4/8-byte field
  widths, `stco`/`co64`, `stsc`, `stsz`, `stss`, `stts`, duplicate boxes,
  arithmetic overflow, truncation, parser budgets, and color/alpha pairing are
  covered without copying encoded AV1 payload bytes;
- all 1,027 active manifest cases pass without changing native Pillow
  pixel/byte parity;
- strict all-target, all-feature Clippy, the AVIF-only WASM check, and the
  all-feature `wasm32-unknown-unknown` check pass; and
- Coverage MCP snapshot `9a7c38c6-b02d-4cd2-b7fb-f5b3c206efde` reports
  32,440/32,440 lines, 4,474/4,474 branches, 1,631/1,631 functions, and
  53,803/53,803 regions.

This slice introduces no public API and performs no image processing. It only
identifies validated encoded AV1 inputs for the next private decoder slice.

## Slice 3: Segmented OBU And Sequence-Header Parsing

Status: planned and reference-reviewed; implementation must follow this
contract.

### Boundary

This slice parses AV1 framing and sequence-level syntax only. It does not parse
a frame header, entropy-decode tiles, reconstruct planes, perform inverse
transforms, apply filters, convert YUV to RGB, combine alpha, or allocate a
pixel canvas.

The parser remains private under `src/codecs/avif/av1/`. It consumes the
borrowed sample spans from Slice 2 directly. Multi-extent items must be read by
a segmented cursor; concatenating or copying encoded payloads is prohibited.
No OBU or sequence-header type becomes part of the crate's public API.

The slice adds:

```text
borrowed sample spans
  └─ SegmentedBitReader
       └─ bounded OBU iterator
            ├─ header/type/extension/size
            └─ sequence_header_obu → private Av1SequenceHeader
```

`SegmentedBitReader` must preserve the logical byte offset across extent
boundaries, support 1–32-bit reads, byte alignment, bounded unsigned LEB128,
and exact end-of-payload detection. It must never read beyond a declared
sample or OBU payload.

### Reference mapping

The normative reference is the AV1 Bitstream & Decoding Process Specification:

- section 5.3.1 — general OBU syntax and size fields;
- section 5.3.2 — OBU header syntax;
- section 5.3.3 — unsigned LEB128;
- section 5.3.4 — trailing bits;
- section 5.5 — sequence-header syntax; and
- section 6.4 — sequence-header semantics and color configuration.

The implementation must cross-check the exact pinned decoder sources:

- dav1d 1.5.3 `src/getbits.c:36-136` — bit reads, refill, ULEB128, and
  UVLC behavior;
- dav1d 1.5.3 `src/obu.c:48-299` — trailing bits and complete sequence-header
  parsing;
- dav1d 1.5.3 `src/obu.c:302-339` — sequence-header discovery inside an OBU
  stream;
- dav1d 1.5.3 `src/obu.c:1169-1209` — OBU header, extension, size, and
  operating-point filtering;
- libaom 3.13.2 `av1/decoder/obu.c:104-275` — independent sequence-header
  validation; and
- libaom 3.13.2 `av1/decoder/decodeframe.c:4216-4298` — dimensions and
  sequence-level coding tools.

Every ported Rust function must carry a concise `✅ VERIFIED` source mapping.
Differences from either reference require a written reason in this document
before implementation.

### Data retained

The private sequence model must retain every field needed by later frame
decoding:

- profile, still-picture, and reduced-header flags;
- timing and decoder-model parameters;
- bounded operating points and selected operating-point IDC;
- maximum frame dimensions and frame-ID widths;
- superblock size and sequence-level tool flags;
- order-hint and screen-content/integer-motion policy;
- bit depth and monochrome state;
- color primaries, transfer characteristics, matrix coefficients, range,
  chroma subsampling, and chroma sample position;
- separate-UV-delta-Q and film-grain flags; and
- the exact sequence-header payload range for repeat-consistency checks.

The model must distinguish encoded bit depth from Pillow's eventual
materialized 8-bit output. It must not infer any field from a filename,
container mode, or native decoder result.

### Required validation

The OBU layer must:

1. reject a nonzero forbidden or reserved bit;
2. accept only defined OBU types needed by the AV1 stream and skip unknown
   reserved types by their declared size;
3. validate extension reserved bits and retain temporal/spatial IDs;
4. require a size field for the low-overhead stream used by AVIF;
5. reject truncated, overflowing, or more-than-eight-byte ULEB128 sizes while
   accepting the non-minimal encodings accepted by the pinned decoders;
6. bound the number of OBUs per sample;
7. reject payload sizes beyond the segmented sample;
8. require a sequence header before the first frame-bearing OBU; and
9. require repeated sequence headers to be bit-identical except for the
   operating-parameter exception allowed by the specification.

The sequence-header layer must implement all syntax branches, not only those
present in current fixtures. It must reject invalid profile/reduced-header
combinations, invalid operating points, zero timing values under strict
semantics, invalid frame-ID widths, unsupported color-layout combinations,
invalid identity-matrix subsampling, truncation at every field, and malformed
trailing bits.

### Independent fixture attack

Add `scripts/inspect_av1_obus.py`, using only the Python standard library. It
must consume the byte ranges emitted by `inspect_avif_bitstreams.py` and report
for every item and track sample:

- ordered OBU type, extension IDs, header length, payload range, and SHA-256;
- sequence-header payload range and exact bytes;
- every retained sequence field listed above; and
- whether the sample contains a frame header, frame OBU, tile group, metadata,
  padding, or an unknown/reserved OBU.

The script must not import Pillow, libavif, dav1d, libaom, or Rust code. Its
committed findings become the reverse-mapping oracle for the Rust parser.

### Fixture boundary findings

Running `python3 scripts/inspect_av1_obus.py` over the committed fixtures
finds that every current OBU uses a size field, no OBU uses an extension, and
all temporal/spatial IDs are zero. The parser must still implement extensions
because they are valid AV1 syntax.

The unique committed sequence headers are:

| Fixture/role | Payload bytes | Profile | Depth | Mono | Maximum size | Subsampling | CP/TC/MC | Range |
| --- | --- | ---: | ---: | --- | --- | --- | --- | ---: |
| `baseline` color | `1819bfff6880868342` | 0 | 8 | no | 128x128 | 4:2:0 | 1/13/6 | full |
| `alpha` color | `38157ffda404341a40` | 1 | 8 | no | 64x64 | 4:4:4 | 1/13/6 | full |
| `alpha` alpha | `18157ffda540` | 0 | 8 | yes | 64x64 | mono | 2/2/2 | full |
| `grid` color cells | `381967fec2021a0d20` | 1 | 8 | no | 80x64 | 4:4:4 | 1/13/6 | full |
| `grid` alpha cells | `181967fec2a0` | 0 | 8 | yes | 80x64 | mono | 2/2/2 | full |
| `hdr` color | `381df1f1d8c2440264` | 1 | 10 | no | 200x200 | 4:4:4 | 9/16/9 | full |
| `animated` color | `00000003bcaca9b5f22021a0d080` | 0 | 8 | no | 150x150 | 4:2:0 | 1/13/6 | full |
| `10bit` color | `40000002afffbfff3c44` | 2 | 12 | no | 64x64 | 4:2:2 | 2/2/2 | limited |
| `10bit` alpha | `40000002afffbfff3ea0` | 2 | 12 | yes | 64x64 | mono | 2/2/2 | full |

As with the container inspection result, `10bit.avif` is a historical
filename; its AV1 sequence header independently confirms 12-bit color and
alpha.

OBU order for every committed sample is:

| Fixture/sample group | Ordered OBUs |
| --- | --- |
| Every still item, every grid cell, `animated` item/track sample 0, and `10bit` color/alpha item/track sample 0 | temporal delimiter, sequence header, frame |
| `animated` track sample 1 | temporal delimiter, frame, frame, frame |
| `animated` track sample 2 | temporal delimiter, frame header |
| `animated` track samples 3–4 | temporal delimiter, frame |
| `10bit` color track samples 1 and 4 | temporal delimiter, frame |
| `10bit` color track samples 2–3 | temporal delimiter, sequence header, frame |
| `10bit` alpha track sample 1 | temporal delimiter, frame |
| `10bit` alpha track samples 2–4 | temporal delimiter, sequence header, frame |

The unsupported-brand fixture fails at its declared `ftyp` size before any
OBU can be exposed. The diagnostic JSON also retains exact OBU header and
payload spans plus payload SHA-256 for reverse mapping during implementation.

### Acceptance criteria

Before this slice is accepted:

1. the diagnostic's OBU and sequence-header map for every committed color and
   alpha sample is recorded below this plan;
2. Rust returns the exact same framing and sequence fields without copying
   sample bytes;
3. `av1C`, sequence-header, item, track, and container declarations are
   cross-validated where the specifications require consistency;
4. public malformed behavior is fixture-driven; private truncation,
   segmented-boundary, bit-reader, and budget states use the coverage-only
   hook;
5. all existing 1,027 manifest rows retain exact success/error, pixel, and
   encoded-byte parity;
6. strict all-target, all-feature Clippy passes;
7. AVIF-only and all-feature `wasm32-unknown-unknown` checks pass; and
8. Coverage MCP reports 100% line, branch, function, and region coverage.

### Slice 3 coverage sweep 2 plan

Coverage MCP snapshot `e215fe63-176a-43ab-907b-e6ac1edd6ad0` passes all
1,027 manifest rows. The new parser is at 100% function coverage, but the
repository totals are 33,089/33,105 lines, 4,647/4,662 branches, and
54,991/55,063 regions. Every deficit is confined to the new private AV1
parser.

The reverse-mapped attack is:

| Gap | Reason | Fix or input |
| --- | --- | --- |
| `bit_reader.rs:44` | `byte()` first rejects positions beyond the logical length, so falling through the valid-span loop is mathematically unreachable. | Remove the duplicate precheck and let the loop's terminal `None` handle out-of-range positions. This preserves behavior and makes the single bounds path testable. |
| `sequence.rs:87,96` | Current headers have no decoder-model parameters, so normalization sees only `None`. | Clone a parsed header, attach decoder parameters to both operands, vary only the three values that AV1 section 7.5 permits to change, and prove `consistent_with()` normalizes them. |
| `sequence.rs:110-111` | Configuration probes use in-bounds spans only. | Call `matches_config()` with a span outside its backing input. |
| `sequence.rs:132-134` | Existing `av1C` probes fail at profile/level/depth before reaching monochrome and subsampling comparisons. | Starting from the known baseline `av1C`, independently toggle monochrome, subsampling-X, and subsampling-Y. |
| `sequence.rs:176-177` | Corpus mutation did not produce syntactically aligned zero timing numerators or denominators. | Build two prefix bitstreams: zero `num_units_in_tick` with nonzero `time_scale`, then the reverse. |
| `sequence.rs:193-194` | Corpus mutation did not retain valid timing syntax while setting the decoder tick to zero. | Build an aligned timing prefix with decoder-model information present and a zero `num_units_in_decoding_tick`. |
| `sequence.rs:350` | The identity-matrix rule needs all four legal profile/depth decision shapes, not random byte mutations. | Generate reduced-header sequences for profile 0, profile 1, profile 2 at 12-bit, and profile 2 below 12-bit with CP/TC/MC `1/13/0`. |
| `sequence.rs:364` | The wildcard match arm is unreachable because profile values above 2 are rejected at the parser entrance. | Express the already-proven `profile == 2` remainder as the final `else`, removing the dead wildcard arm without weakening validation. |
| `sequence.rs:369` | Short-circuit instrumentation asks for `subsampling_x == false && subsampling_y == true`, a state the preceding AV1 layout rules cannot produce. | Evaluate the two booleans with non-short-circuit OR. The validity predicate is unchanged and the impossible evaluation edge disappears. |
| `av1/mod.rs:106` | Every accepted sequence header is already checked against the immutable sample `av1C`; checking the same pair again after the loop creates an unreachable mismatch branch. | Retain the required sequence-presence and frame-bearing checks, and remove the duplicate configuration comparison. |
| `av1/mod.rs:122,127` | Public extracted AVIFs always contain still and/or sequence payloads because extraction validates that invariant first. | Exercise the private validation layer directly with neither payload, proving its neutral behavior independently of extraction. |
| remaining region-only records | LLVM's normalized line projection cannot expose every expression region. | Apply the inputs above, rerun Coverage MCP, then inspect raw region coordinates only for any residual records while keeping Coverage MCP as the sole test runner and acceptance source. |

No case adds a public processing API or manufactured encoded-image fixture.
The generated syntax is confined to the coverage-only private parser hook.

Sweep 2 produced snapshot `b9688b11-36d4-41c2-a154-26be5c85cc41`:
33,226/33,226 lines, 4,668/4,668 branches, 1,659/1,659 functions, and
55,202/55,263 regions, with all 1,027 rows passing. Raw LLVM coordinates
confirm that all 61 residual regions are failure exits attached to `?`
operators; Coverage MCP's normalized line view therefore correctly reports no
line or branch gaps.

Before sweep 3:

- remove checked arithmetic where preceding bounds or fixed AV1 field widths
  prove failure impossible (`5-bit + 1`, `4-bit + 1`, bounded span offsets,
  single-bit shifts, and loop counters capped at 4,096);
- keep genuinely fallible arithmetic, but drive it through a reachable helper
  or a deliberately constructed private state;
- split sequence parsing into byte-bounded construction plus a reader-based
  core, and give only the coverage hook a bit-bounded reader constructor;
- truncate every known valid sequence at every bit boundary so every genuinely
  reachable read failure is exercised, rather than pretending an AVIF OBU can
  end at a fractional byte; and
- construct invalid still/alpha/sequence routing values only inside the
  coverage hook to cover propagation at the AV1 validation layer. Public
  malformed-container behavior remains manifest-fixture driven.

Sweep 3 produced snapshot `37bf1d4e-584f-4fc5-809f-aff76fc05a43`:
33,375/33,375 lines, 4,670/4,670 branches, 1,665/1,665 functions, and
55,364/55,377 regions. The remaining 13 raw regions are:

- 11 reads on syntax paths absent from the committed corpus: equal-picture
  interval, decoder-model continuation, high-level tier, per-operating-point
  decoder parameters, frame IDs, explicit screen-content policy, and explicit
  integer-motion policy;
- the byte-bounded `parse()` constructor's invalid-range propagation; and
- `inspect()` propagating a failure from AV1 validation after container/sample
  extraction has already succeeded.

Sweep 4 will generate one complete private non-reduced sequence enabling all
11 syntax paths, truncate it at every bit, add an equal-interval UVLC-overflow
prefix, and call `parse()` with invalid byte bounds. The final public
`inspect()` failure must be covered by a committed malformed AVIF manifest
fixture, not by another private bypass.

The derived reserved-profile fixture exposes an important Pillow contract:
Pillow 12.2.0 opens and verifies the container metadata successfully, while
frame decoding fails with `Decoding of color planes failed`. Therefore AV1
bitstream validation belongs on `decode` and `decode_sequence`, not
metadata-only `inspect`. `EncodedImage::new` and `verify` must retain the
successful Pillow metadata/verification behavior; later materialization must
return the structured AVIF decode error. The production AV1 parser will be
moved to that decode boundary before the final sweep.

Sweep 5 also proved that the timing overflow branch was impossible:
`uvlc()` rejects 32 leading zeroes, so its maximum value is
`u32::MAX - 1`; adding the syntax-defined one cannot overflow. The parser now
uses the bounded saturating addition directly. Common validated extraction is
shared by metadata inspection and decode, while the AV1 syntax check remains
decode-only.

### Slice 3 accepted result

Coverage MCP snapshot `47354cb4-2acd-4dc8-8bdd-cce53e8227bf` passes all
1,028 active manifest rows with no skips and reports:

- 33,464/33,464 lines;
- 4,670/4,670 branches;
- 1,668/1,668 functions; and
- 55,547/55,547 regions.

Strict all-target/all-feature Clippy passes. Both
`--no-default-features --features avif` and `--all-features` pass for
`wasm32-unknown-unknown`. The production decode boundary now performs
zero-copy AVIF sample extraction plus complete OBU and sequence-header
validation before entering the pinned native pixel oracle. The new
`invalid_sequence_profile.avif` row records exact Pillow open/verify/decode
behavior and proves structured still/sequence decode rejection.

## Later Portable AVIF Slices

After sequence-header parsing is accepted:

1. add the complete frame-header parser and reference-frame state;
2. port scalar entropy decoding and symbol tables;
3. port intra/inter prediction, inverse transforms, loop restoration, and
   frame reconstruction;
4. combine codec-declared alpha and convert reconstructed samples to the exact
   Pillow-visible RGB/RGBA bytes;
5. replace native still decode;
6. replace native sequence decode and timing;
7. port deterministic libaom-equivalent encoder primitives;
8. replace still and sequence encode;
9. preserve metadata and transforms; and
10. remove the C bridge, native link search, and fixed external stack from the
    shipped feature.

Each slice must keep the relevant native reference as an oracle, add
manifest-driven fixtures, pass WASM, and restore full Coverage MCP metrics
before the next slice begins.

## Slice 4 Plan: Complete Frame Headers and Reference State

### Scope boundary

This slice parses encoded AV1 syntax and advances decoder reference state. It
does not reconstruct, transform, resample, combine, or expose pixels and adds no
public API. The result remains private to `codecs::avif::av1`.

The following are in scope because later entropy and reconstruction stages
cannot interpret an AV1 tile without them:

- the complete `uncompressed_header()` syntax, including frame size, tiling,
  quantization, segmentation, loop-filter declarations, CDEF declarations,
  restoration declarations, transform/reference/skip modes, global motion, and
  film-grain declarations;
- the exact bit position at which tile data begins in an `OBU_FRAME`;
- `OBU_FRAME_HEADER`, `OBU_REDUNDANT_FRAME_HEADER`, `OBU_TILE_GROUP`, and
  `OBU_FRAME` ordering;
- eight reference slots and their syntax-visible inherited state; and
- show-existing-frame and refresh-frame transitions.

Those declarations are codec control data. Applying prediction, inverse
transforms, in-loop filters, film grain, alpha composition, or color conversion
to samples remains a later private decode stage. General-purpose image
processing remains prohibited throughout the repository.

### Fixed reference map

The implementation is mapped to these exact pinned sources:

- dav1d 1.5.3 commit
  `b546257f770768b2c88258c533da38b91a06f737`:
  - `src/obu.c:341-395`, `read_frame_size()`;
  - `src/obu.c:398-402`, `tile_log2()`;
  - `src/obu.c:409-1151`, `parse_frame_hdr()`, the complete frame header;
  - `src/obu.c:1154-1167`, `parse_tile_hdr()`;
  - `src/obu.c:1211-1323`, frame/header/tile-group OBU state transitions; and
  - `src/obu.c:1528-1685`, show-existing completion and reference refresh.
- libaom 3.13.2 commit
  `ad44980d7f3c7a2605c25d51ea96946949000841`:
  - `av1/decoder/decodeframe.c:139-155`, transform and reference mode;
  - `av1/decoder/decodeframe.c:1431-1508`, segmentation inheritance;
  - `av1/decoder/decodeframe.c:1757-1821`, CDEF and quantization;
  - `av1/decoder/decodeframe.c:1867-2084`, interpolation, frame size,
    render size, and super-resolution;
  - `av1/decoder/decodeframe.c:2086-2199`, uniform and non-uniform tiling;
  - `av1/decoder/decodeframe.c:3907-4085`, film-grain syntax;
  - `av1/decoder/decodeframe.c:4300-4416`, global-motion syntax;
  - `av1/decoder/decodeframe.c:4424-4480`, frame-ID/reference refresh and
    show-existing reset; and
  - `av1/decoder/decodeframe.c:4486-5145`,
    `read_uncompressed_header()`.

The Rust port will prefer the AV1 specification's normative field order, use
dav1d as the primary implementation map, and use libaom to detect
implementation-specific assumptions. Every ported Rust parser/derivation
function must carry a concise `✅ VERIFIED` mapping to one of these ranges.

### State contract

`SequenceHeader` will expose only `pub(super)` access needed by the private
frame parser. No sequence or frame syntax type becomes part of the crate's
public API.

The frame parser must retain all syntax that affects a later tile decode:

- frame type, show/showable/existing flags, error resilience, CDF policy,
  screen-content/integer-motion policy, frame ID, order hint, primary
  reference, and refresh mask;
- coded/upscaled/render dimensions, super-resolution denominator, reference
  indices, interpolation and motion-mode policy;
- tile rows/columns, their superblock boundaries, context-update tile, tile-size
  byte width, and the exact first tile-data bit;
- quantizer deltas and matrices;
- segmentation features and inherited segmentation state;
- delta-Q/delta-loop-filter declarations and derived per-segment lossless state;
- loop-filter levels, sharpness, and inherited mode/reference deltas;
- CDEF strengths, restoration types/unit sizes, transform mode, reference mode,
  skip-mode references, warped-motion permission, reduced transform set;
- all seven global-motion models; and
- complete film-grain parameters or the validated reference from which they
  are inherited.

Reference state is exactly eight optional slots. A retained slot must include
the frame header state inherited by later headers: frame ID, order hint,
dimensions/render dimensions, showable status, segmentation data,
loop-filter deltas, global motion, and film grain. A refresh is applied only
after a complete frame has the required tile groups. `show_existing_frame`
selects an already valid slot and does not parse tile syntax.

### OBU state machine

- `OBU_FRAME_HEADER` parses and stores one pending header.
- `OBU_REDUNDANT_FRAME_HEADER` is legal only with a pending header and must
  reproduce the same syntax-visible header.
- `OBU_TILE_GROUP` is legal only with a pending non-show-existing header. Tile
  ranges must be ordered, bounded by the declared tile count, and collectively
  complete before the frame refresh is committed.
- `OBU_FRAME` contains a frame header followed immediately by tile-group
  syntax in the same payload. The frame parser must return its bit position;
  it must not treat the remaining tile bytes as header trailing bits.
- A temporal delimiter ends any incomplete temporal-unit state and must not
  manufacture a completed frame.
- A new non-redundant frame header before completion of the previous frame is
  malformed.
- `show_existing_frame` is forbidden in `OBU_FRAME`, matching dav1d
  `src/obu.c:1311-1315`.

The current committed corpus already proves all four important layouts:
standalone `OBU_FRAME`, multiple `OBU_FRAME`s in one sample, a standalone
`OBU_FRAME_HEADER` show-existing sample, and later samples that inherit the
earlier sequence/reference state.

### Independent reverse-mapping oracle

Before Rust implementation, extend `scripts/inspect_av1_obus.py` using only the
Python standard library. For every committed item and track sample it must emit:

- every retained frame-header field listed above;
- header start/end bit positions and the first tile-data bit for `OBU_FRAME`;
- tile-group start/end tile indices;
- reference slots before and after each completed frame; and
- explicit show-existing and refresh transitions.

The diagnostic will implement the specification directly and must not import
Pillow, libavif, dav1d, libaom, or Rust output. A development-only trace of the
pinned dav1d headers may be used to diagnose a disagreement, but the committed
Python report remains independent. Pillow remains the public success/error and
pixel oracle.

### Independent frame-header findings

The first complete diagnostic sweep parses every valid committed AVIF item and
track sample, reaches a byte-aligned first tile byte for every `OBU_FRAME`, and
leaves no incomplete frame. It reports 28 frame headers:

| Fixture | Header shapes |
| --- | --- |
| `baseline` | one reduced key frame, 67 header bits, 128x128, base Q 120 |
| `alpha` | lossless color/alpha reduced key frames, 18/16 header bits |
| `grid` | two 80x64 color keys at 50 bits and two lossless alpha keys at 17 bits |
| `hdr` | one 200x200 reduced key frame, 44 bits, base Q 8 |
| `10bit` | color/alpha item keys plus two five-frame tracks; key headers are 60-73 bits and inter headers are 109-131 bits |
| `animated` | initial key, three inter frames in one sample, one four-bit show-existing frame header, then two inter frames |

All committed frames declare one tile. The independent parser therefore finds
no tile-index bits and locates tile data by zero-padding each uncompressed
header to its next byte boundary.

The `animated` track proves delayed reference refresh and inheritance:

| Presentation step | Type/order hint | Refresh mask | Seven references | Primary reference |
| --- | --- | ---: | --- | ---: |
| initial | key / 0 | `0xff` | none | 7 |
| hidden 1 | inter / 4 | `0x02` | `0,0,0,0,0,0,0` | 7 |
| hidden 2 | inter / 2 | `0x04` | `0,0,0,0,0,0,1` | 7 |
| hidden 3 | inter / 1 | `0x08` | `0,0,0,0,2,0,1` | 7 |
| display | show existing slot 3 | none | inherited | none |
| later 1 | inter / 3 | `0x10` | `2,3,0,0,0,0,1` | 1 |
| later 2 | inter / 4 | `0x00` | `4,2,3,0,0,0,1` | 0 |

The `10bit` filename remains historically misleading at the frame layer too:
its sequence is 12-bit, and its color and alpha tracks independently retain
64x64 dimensions through key/inter refreshes. Repeated sequence headers reset
neither track because their syntax is consistent with the active sequence.

The two committed malformed inputs still fail at the intended earlier
boundaries: reserved sequence profile for
`invalid_sequence_profile.avif`, and invalid declared `ftyp` size for
`unsupported_major_brand.avif`.

### Error-fixture attack

Public failures must be derived from accepted AVIF files by one local encoded
syntax mutation at a time. Candidate manifest rows are:

- truncated frame header;
- invalid show-existing reference slot;
- illegal show-existing `OBU_FRAME`;
- invalid reference index or frame-ID delta;
- invalid context-update tile;
- incomplete or out-of-order tile groups;
- invalid segmentation feature value;
- non-increasing film-grain control points; and
- redundant frame header without a matching pending header.

Each candidate is kept only when the pinned Pillow oracle supplies a stable
exact open/verify/decode outcome. The mutation offset and changed bits must be
reported by the independent script. States that cannot be expressed through a
Pillow-observable encoded input, such as checked-arithmetic impossibilities or
private inheritance shapes, belong only in the `cfg(coverage)` hook.

### Acceptance criteria

Slice 4 is accepted only when:

1. the independent diagnostic reports complete frame headers and reference
   transitions for every committed AVIF item and track sample;
2. Rust accepts the same valid samples and rejects every retained malformed
   fixture before entering the native pixel decoder;
3. standalone frame-header, frame, redundant-header, and tile-group OBU
   ordering is fully validated;
4. all eight reference slots, show-existing behavior, and delayed refresh
   commits match the pinned decoders;
5. frame parsing is zero-copy, dependency-free, WASM-safe, private, and adds no
   image-processing API;
6. every public error is manifest/fixture based and compares Pillow's exact
   status/type/message contract;
7. strict all-target/all-feature Clippy and both AVIF-only and all-feature
   `wasm32-unknown-unknown` checks pass; and
8. Coverage MCP is the only test runner and reports 100% line, branch,
   function, and region coverage with all manifest rows active.

### Slice 4 coverage sweep 1

Coverage snapshot `1eb1849c-ace8-4e02-baf5-472539dfd2b7` proves that all
1,028 active manifest rows pass, but the new private frame parser initially
reaches only 1,011/1,231 lines, 308/440 branches, 51/58 functions, and
1,565/2,290 regions. The missing code is grouped by syntax responsibility
before any coverage-only input is added:

| Group | Missing behavior | Reverse-mapped input |
| --- | --- | --- |
| OBU/reference state | redundant headers, standalone tile groups, key show-existing reset, delayed partial-tile refresh, invalid tile order | Direct private state transitions with one- and multi-tile headers; encoded public failures remain manifest fixtures |
| Frame IDs and timing | first/wrapped/repeated/too-distant IDs, presentation delay, per-operating-point removal delay | Minimal `SequenceHeader`, `Timing`, and `OperatingPoint` values plus bit-exact payloads |
| Frame kind and references | intra-only/switch paths, short and explicit references, inherited frame size, bad reference frame ID | Direct frame-header helper calls with populated reference slots |
| Frame geometry and tiling | super-resolution, render size, uniform/non-uniform multi-tile layouts, invalid context-update tile | Bit-exact helper payloads at small and large synthetic dimensions |
| Coding metadata | separate UV quantizers, matrices, segmentation inheritance/features, delta Q/LF, loop-filter deltas, CDEF, restoration | Bit-exact metadata payloads with state inherited from a synthetic primary reference |
| Prediction metadata | skip-mode before/after selection, warped/high-precision motion, translation/rotzoom/affine global motion | Synthetic inter headers and reference order hints, with subexponential-coded parameters |
| Film grain | inherited grain, invalid reference, luma/chroma control points, coefficients, 4:2:0 plane consistency, bounds failures | Bit-exact film-grain payloads for monochrome, chroma-from-luma, and independent UV modes |
| Defensive arithmetic | invalid enum value, impossible shifts/indexes/overflow | Direct `cfg(coverage)` calls only; these states cannot be emitted by a conforming AV1 bitstream |

The coverage hook is compiled only under `cfg(coverage)`, stays inside the
private AV1 module, and may call private syntax helpers directly. It must not
expose decoded-frame manipulation or any image-processing API. Each helper
input is retained only when it reaches a documented syntax branch; blind
mutation remains a fallback for parser truncation, not the primary proof of
semantic paths.

After this sweep, Coverage MCP is rerun once. Its exact missing line, branch,
function, and region records become the input to the next sweep. Publicly
observable failures are added only through the Pillow-oracle manifest; private
defensive states remain coverage-only.

### Slice 4 coverage sweep 2

Coverage snapshot `a2693b92-e91c-4afe-b4dc-4c14f7bcb345` completes the frame
parser sweep with all 1,028 active manifest rows passing and exact aggregate
coverage:

| Metric | Covered / total |
| --- | ---: |
| Lines | 34,543 / 34,543 |
| Branches | 5,134 / 5,134 |
| Functions | 1,728 / 1,728 |
| Regions | 57,320 / 57,320 |

Reverse mapping exposed and fixed two parser-design errors instead of merely
manufacturing inputs for their branches:

- stored header bit positions are now payload-relative, so a redundant header
  can be compared with its original even when it appears at another segmented
  input offset; and
- current-frame-ID progression is validated when `FrameState` accepts a new
  non-redundant header, not while raw header syntax is parsed. This prevents a
  redundant copy from being progression-validated after its original header
  has already advanced the state.

The independent `scripts/inspect_av1_obus.py` diagnostic also reports the
reference-mode and skip-mode bit positions. The committed animated fixture
places the reference-mode flag at bits 107 or 108 of its inter-frame payloads;
targeted single-bit mutations at those reported positions reached the
select-reference and skip-reference syntax without relying on Rust parser
output. Subexponential decoding now uses bounded wider arithmetic, and
unreachable shift probes were removed where AV1 syntax widths already prove
the invariant.

All additional state probes remain private under `cfg(coverage)`. No decoded
pixel transform or public image-processing API was introduced.

## Current validation after codec-only encapsulation

The canonical public codec boundary, strict arithmetic cleanup, and private
AV1 parser remain exact after the processing layer and public implementation
modules were removed.

Coverage MCP run `1021abe5-d106-473a-bec5-5a2b560718f9`, snapshot
`d98b6bc6-6852-40c9-8800-b9937e711a91`, passes all five test suites and reports
34,683/34,683 lines, 5,136/5,136 branches, 1,735/1,735 functions, and
57,658/57,658 regions. All 1,028 manifest rows remain active.

This validation does not change the remaining AVIF release blocker: portable
container inspection and AV1 syntax parsing work on WASM, while portable AV1
pixel reconstruction and encoding are still incomplete.

## Slice 5 Plan: Tile Byte Boundaries And Scalar Entropy Core

Status: accepted on 2026-07-29.

### First-divergence correction

The Slice 4 frame parser proves tile-group syntax and ordering, but its original
acceptance boundary stopped immediately after `tg_start`/`tg_end`. That is one
stage too early for a real decoder. AV1 stores a little-endian
`tile_size_minus_1` before every tile except the final tile in a group. The
remaining bytes belong to the final tile. The original Rust parser neither
split those ranges nor rejected a size that crossed the OBU payload.

This is the first unimplemented value consumed by both pinned decoders:

- dav1d 1.5.3 `src/decode.c:3149-3181` reads `n_bytes`, adds one, bounds the
  declared tile, and passes the exact byte span to `setup_tile()`;
- dav1d 1.5.3 `src/decode.c:2425-2457` initializes the scalar MSAC decoder from
  that span;
- libaom 3.13.2 `av1/decoder/obu.c:295-372` validates the group header before
  tile decode; and
- libaom 3.13.2 `av1/decoder/decodeframe.c:3618-3663` resolves and consumes the
  individual tile buffers.

The correction must land before range decoding. Otherwise an entropy mismatch
could be caused by feeding the right arithmetic decoder the wrong bytes.

### Boundary and scope

This slice remains a private AV1 codec stage. It must:

1. retain the aligned first tile byte for each tile group;
2. read one-to-four little-endian size bytes exactly as declared by the frame
   header;
3. add the syntax-defined one without overflow;
4. reject a truncated size field or a tile extending beyond its OBU payload;
5. assign all remaining bytes to the final tile;
6. retain ordered zero-copy logical ranges across segmented AVIF item extents;
7. initialize the scalar multi-symbol arithmetic decoder from each exact tile
   range;
8. port equal-probability, fixed-probability, adaptive boolean, adaptive
   symbol, uniform, high-token, and subexponential primitives; and
9. retain CDF adaptation state only inside the AV1 decoder.

It does not expose pixels or a general transform. Prediction, coefficient
decoding, inverse transforms, filtering, and sample reconstruction remain later
private codec stages.

### Independent fixture attack

`scripts/inspect_av1_obus.py` is extended before the Rust boundary. For every
tile it reports:

- tile index;
- logical start and length inside the encoded AV1 sample;
- physical AVIF spans;
- exact SHA-256; and
- the encoded size-field width and value when the tile is not last in its
  group.

The existing corpus has one tile per frame, so it cannot prove size-field
handling. A deterministic Pillow 12.2.0/libaom fixture with two tile columns is
added. A second fixture changes only its first tile size so Pillow supplies the
public malformed decode oracle. Synthetic private probes remain limited to
segmented-boundary and arithmetic states that cannot be expressed as a stable
Pillow input.

The scalar entropy primitives use exact trace vectors generated from the pinned
dav1d 1.5.3 C implementation. Each vector records input bytes, initial CDF,
update policy, decoded values, final CDF, range, difference window, byte
position, and bit count. Rust must match every retained state value, not merely
the decoded symbols.

### Initial tile-boundary findings

The pinned oracle deterministically generated `multitile.avif` as 7,467 bytes.
Its color item is one 7,192-byte extent at file offset 275 and contains a
single `OBU_FRAME` with two tile columns:

| Tile | Size field | Logical tile span | Physical file span | SHA-256 |
| ---: | --- | --- | --- | --- |
| 0 | two bytes at logical 31, value 3,661 | 33+3,662 | 308+3,662 | `53b324f623abc8c616e3d2192f852ee693e2c4f9fabc5158b017b20b70b202d4` |
| 1 | implicit remainder | 3,695+3,497 | 3,970+3,497 | `f9c98cafd7a6db7fd777e9045bf01cfd82c235ded70a674640face1da088b2f6` |

The source file SHA-256 is
`28bd09d7f17a15fcf3457eb21d2bebc36054718b20338191793e2d5faa61f253`.
Pillow opens, verifies, and loads it as exact 256x128 RGB bytes.

`invalid_tile_size.avif` changes only the high byte of the first size field to
`0xff`. Its SHA-256 is
`82a07a29a8631d60a2d83bd9973afac0e494882580d3542bb953875817eb0f67`.
Pillow open and verify still succeed, while materialization fails with
`builtins.RuntimeError: Failed to decode frame 0: Decoding of color planes
failed`. The independent diagnostic rejects the same input specifically at
`AV1 tile size exceeds tile-group payload`.

The pinned dav1d trace generator now records 103 states in
`tests/fixtures/outputs/av1_entropy.json`:

| Primitive | Recorded states including initialization |
| --- | ---: |
| equal probability | 17 |
| fixed probability | 8 |
| adaptive boolean | 17 |
| adaptive four-symbol CDF | 17 |
| frozen four-symbol CDF | 9 |
| high coefficient token | 9 |
| uniform integer | 6 |
| subexponential integer | 5 |
| real 4:2:2 still and inter-frame partitions | 4 |
| real 4:2:2 restoration prefix and partition | 11 |

The file SHA-256 is
`e36cf4228d6e4bbfc821f68662c4a7c7782065707087e280dfffe5cedcedc310`.
Regenerating it twice from dav1d commit
`b546257f770768b2c88258c533da38b91a06f737` produces identical bytes. The
generator compiles the unmodified pinned `src/msac.c`; it refuses any other
commit and never calls Rust or Pillow.

### Tile-boundary prerequisite accepted

The production frame parser now splits and validates tile payloads before the
native pixel decoder. Reverse mapping caught one implementation error during
the first coverage run: a one-tile group has no encoded size field, so its
frame-header `tile_size_bytes` value is legitimately zero. Widths one through
four are required only when a non-final tile actually carries
`tile_size_minus_1`. The corrected rule retains all existing one-tile AVIFs,
accepts the new two-tile fixture, and rejects the one-byte malformed derivative
at the same materialization boundary as Pillow.

Coverage MCP run `cf14ddbd-671b-46f6-844e-850a06a863b8`, snapshot
`055e8e8b-16c9-451d-af49-ee7bb264cc86`, passes all five suites and all 1,030
active manifest rows with:

- 34,741/34,741 lines;
- 5,150/5,150 branches;
- 1,737/1,737 functions; and
- 57,737/57,737 regions.

Strict native all-target/all-feature Clippy and strict AVIF-only plus
all-feature `wasm32-unknown-unknown` Clippy also pass. This accepts the
tile-boundary prerequisite, not all of Slice 5: the scalar entropy port must
still consume the committed dav1d traces before the slice can be declared
complete.

### Scalar entropy implementation design

The scalar entropy port uses one representation on every Rust target:

```text
RangeDecoder
  ├─ SegmentedData reference
  ├─ logical byte position and tile end
  ├─ 64-bit difference window
  ├─ 16-bit-normalized range stored as u32
  ├─ signed refill count
  └─ CDF update policy
```

Using a fixed `u64` difference window is deliberate. dav1d defines its window
as `size_t`, so a direct Rust `usize` port would use different arithmetic on
native 64-bit builds and `wasm32`. The committed C oracle is the pinned 64-bit
scalar implementation. A fixed `u64` therefore preserves the oracle state and
gives native and WASM exactly the same decoder semantics.

The production path supplies each exact zero-copy tile range to
`RangeDecoder::new`. Initialization refills from `SegmentedData`, so a tile may
cross any number of AVIF item extents without concatenation. The decoder never
reads beyond the tile end; dav1d's end-of-buffer fill rule supplies terminal
one bits inside the arithmetic window.

The port maps these pinned functions without algebraic simplification:

| Rust operation | dav1d 1.5.3 reference |
| --- | --- |
| initialization and refill | `src/msac.c:41-58`, `204-219` |
| normalization | `src/msac.c:80-97` |
| equal-probability boolean | `src/msac.c:99-112` |
| fixed-probability boolean | `src/msac.c:117-128` |
| adaptive symbol | `src/msac.c:132-166` |
| adaptive boolean | `src/msac.c:168-185` |
| high coefficient token | `src/msac.c:187-201` |
| multi-bit and uniform integer | `src/msac.h:94-108` |
| subexponential integer | `src/msac.c:60-74` |

Every retained trace record compares the decoded value, logical byte position,
difference window, normalized range, refill count, and complete CDF. The Rust
test driver repeats the operation sequence independently of the C generator;
only the generated JSON supplies expected states.

This stage remains codec infrastructure. It has no public release API and
does not transform decoded pixels. Actual partition and coefficient syntax
will consume the decoder state in the following reconstruction slice.

### Restoration-prefix divergence and correction

The first production consumer initially decoded a partition symbol immediately
after MSAC initialization. That ordering is valid only when no restoration unit
starts at the first superblock. The `10bit.avif` color track's fourth sample is
the counterexample: its frame header declares SGRPROJ restoration on Y, U, and
V. dav1d `src/decode.c:2638-2712` calls `read_restoration_info()` for each
applicable unit before `decode_sb()`. Reading the untouched tile bytes with the
64x64 partition CDF therefore produced the forbidden 4:2:2 `PARTITION_V4`;
the frame header and tile boundary were both correct.

The correction must mirror dav1d `src/decode.c:2511-2578`:

1. initialize the three restoration CDFs and per-plane predictor state with
   dav1d defaults;
2. determine whether a restoration unit begins at the tile's first
   superblock;
3. decode NONE/WIENER/SGRPROJ selection from the frame restoration type;
4. decode the required Wiener coefficients or SGR parameter index and weights,
   updating the per-plane predictor only when syntax defines a new unit; and
5. decode the partition from the same advanced `RangeDecoder`.

The pinned C oracle records the real 12-bit 4:2:2 tile bytes, every restoration
operation, and the subsequent partition state. The public manifest continues
to prove all five Pillow-observable frames. This prefix is private AV1 codec
syntax; it does not expose restoration as an image-processing operation.

### Acceptance criteria

This slice is accepted only when:

1. the independent diagnostic and Rust parser return the same ordered tile
   ranges and hashes for every committed AVIF sample;
2. the two-tile success fixture retains exact Pillow pixels and the single-bit
   malformed derivative retains Pillow's exact success/error contract;
3. every scalar entropy trace matches pinned dav1d state after every symbol;
4. production parsing consumes the tile splitter and the next decoder stage
   consumes the entropy core—neither may exist as an independent unused helper;
5. no public image-processing API or new dependency is added;
6. native and `wasm32-unknown-unknown` strict Clippy gates pass for AVIF-only
   and all-feature builds; and
7. Coverage MCP remains the only test runner and reports 100% line, branch,
   function, and region coverage with every manifest row active.

### Slice 5 accepted result

The scalar entropy stage is accepted on 2026-07-29:

- the production tile path consumes exact split tile ranges and advances the
  scalar range decoder through restoration syntax before the first partition
  when the frame header requires it;
- all 103 retained Rust states match the pinned dav1d trace, including the
  real 4:2:2 still, inter-frame, and restoration-bearing samples;
- restoration declarations and decoded restoration-unit values use closed
  internal enums instead of ambiguous integers;
- the range-decoder primitives are total under their encoded-range
  invariants, while container, tile-range, frame-syntax, and unsupported
  reconstruction boundaries remain explicitly fallible;
- the former inherited-CDF helper was removed because constructing and
  discarding a fresh decoder did not validate inherited state. Inter tiles are
  left for the retained-CDF reconstruction slice instead of claiming false
  coverage;
- strict native all-target/all-feature Clippy, strict AVIF-only WASM Clippy,
  and strict all-feature WASM Clippy pass;
- Coverage MCP run `79156bd5-ca06-4c25-aec6-3fac2b6e2dc2`, snapshot
  `60db3959-e08b-4dc4-800e-9793be0b44d9`, passes all six test binaries with
  35,247/35,247 lines, 5,262/5,262 branches, 1,766/1,766 functions, and
  58,528/58,528 regions.

This acceptance remains private codec infrastructure. No resize, crop,
rotation, reusable restoration/filter, general color-conversion, compositing,
or mutable raster API is exposed. Portable AV1 coefficient decoding,
prediction, inverse transforms, in-loop filtering, pixel reconstruction, and
encoding remain future codec-internal slices.

## Slice 6: First Complete Leaf Block And Reconstructed Plane

Status: accepted on 2026-07-29.

### Implemented boundary

Before this slice, the verified production path stopped after the first
partition decision. It now consumes the first complete closed
`PARTITION_NONE` leaf class from the same scalar range-decoder instance,
retains the result in `FrameState`, and carries it through `ValidatedAv1` for
the future portable color-conversion stage.

The reference order is pinned to dav1d 1.5.3 commit
`b546257f770768b2c88258c533da38b91a06f737`:

- `src/decode.c:683-2059` — complete `decode_b()` syntax and state updates;
- `src/decode.c:2117-2380` — recursive partition geometry and leaf dispatch;
- `src/recon_tmpl.c:720-942` — transform/coefficient dispatch;
- `src/recon_tmpl.c:1125-1561` — intra prediction and residual reconstruction;
- `src/itx_tmpl.c` and `src/ipred_tmpl.c` — scalar inverse transforms and
  predictors selected by the decoded block;
- `src/cdf.c:76-689` — default block-syntax CDFs; and
- `src/tables.c` plus `src/levels.h` — block dimensions, partition-to-block
  mapping, transform sizes, and closed syntax enums.

This is private codec work. The predictors and inverse transforms may only be
called by the AV1 decoder. They must not be re-exported, generalized into
raster operations, or shared as a public image-processing layer.

### Oracle fixture selection

A deterministic script must generate a small Pillow 12.2.0/libaom corpus from
constant RGB inputs at 1x1, 2x2, 4x4, 8x8, and 16x16. It records the exact
Pillow/libavif/dav1d/libaom versions and selects the smallest fixture whose
first frame:

1. has one tile and one independently decodable key frame;
2. reaches one complete leaf block without palette, intrabc, segmentation,
   super-resolution, film grain, or inter prediction;
3. contains at least one nonzero reconstructed sample, so a zero-filled
   implementation cannot pass;
4. has stable exact Pillow RGB bytes; and
5. retains a second, deliberately different constant input to prove that the
   implementation does not special-case one encoded payload.

The committed manifest owns public success/error and RGB output parity. A
separate pinned dav1d trace owns internal AV1 correctness.

### Pinned dav1d trace

`scripts/generate_av1_reconstruction_refs.py` builds a temporary,
instrumented copy of the exact dav1d commit. It must refuse a dirty or
version-mismatched source and must never compile repository Rust code into the
oracle. For each selected fixture it records, in decode order:

- logical tile and block coordinates;
- partition level, partition value, and selected block size;
- every adaptive/fixed/equal entropy operation with its semantic label,
  decoded value, complete CDF, byte position, difference window, range, and
  refill count;
- segment ID, skip mode, skip flag, luma/chroma prediction modes, angle
  deltas, transform size/type, end-of-block values, coefficient positions and
  signed levels;
- prediction samples before residual addition;
- inverse-transform residual samples; and
- reconstructed Y, U, and V plane samples with dimensions, stride-independent
  row bytes, and SHA-256.

The trace must be deterministic across two clean generations. The committed
JSON contains values and hashes, not compiler logs or temporary build paths.

### Rust production model

The AV1 module has private, codec-specific types:

```text
BlockSyntax
  ├─ geometry and partition
  ├─ segment/skip state
  ├─ prediction modes
  └─ transform/coefficient syntax

ReconstructedPlane
  ├─ coded width and height
  ├─ bit depth
  ├─ chroma subsampling role
  └─ u16 samples in row-major coded order
```

The production tile decoder—not a detached test helper—consumes the same
range-decoder instance already advanced through restoration and partition
syntax. The first accepted implementation supports only the closed syntax
class proven by the selected all-intra fixtures and rejects an
unsupported syntax value at the exact point it is encountered. It must never
skip unknown entropy syntax and continue from a fabricated decoder state.

Native decoding continues to use the pinned libavif stack as a fallback until
the portable path can return a complete image. WASM may return a portable
result only after every plane and the codec-mandated color conversion are
implemented for that input class. No fixture-name, byte-hash, or dimension
special case is permitted.

### Acceptance criteria

Slice 6 is accepted only when:

1. the fixture-selection script produces the same two nontrivial fixtures and
   Pillow RGB references twice;
2. the instrumented pinned dav1d build produces byte-identical trace JSON
   twice;
3. the Rust path matches every retained block syntax, CDF state, coefficient,
   prediction sample, residual, and reconstructed plane sample;
4. the production tile path consumes the block implementation and no
   independent unused helper remains;
5. existing AVIF validation and native Pillow parity do not regress;
6. no public image-processing API, target-specific semantic fork, unsafe Rust,
   or new crate dependency is added;
7. strict native, AVIF-only WASM, and all-feature WASM Clippy pass; and
8. Coverage MCP remains the only test runner and reports exact 100% line,
   branch, function, and region coverage with all manifest rows active.

### Accepted result

The slice was accepted on 2026-07-29:

- `scripts/generate_test_assets.py --format avif` deterministically produces
  two independent 4x4, quality-100, single-threaded, 4:4:4 AVIF fixtures from
  RGB inputs `(17, 91, 203)` and `(199, 37, 83)`;
- `scripts/generate_av1_reconstruction_refs.py` verifies dav1d commit
  `b546257f770768b2c88258c533da38b91a06f737`, instruments a temporary scalar
  build, runs it twice, and commits only deterministic JSON values and hashes;
- reverse mapping established that the coded leaf is 8x8 but clipped to the
  visible 4x4 frame. Each plane contains four 4x4 lossless transforms: one
  nonzero DC transform followed by three coded skips;
- fixture A reconstructs constant Y/U/V samples `0x51/0xc4/0x51` from DC
  coefficients `-736/1088/-752`; fixture B reconstructs
  `0x5b/0x7b/0xcd` from `-576/-80/1232`;
- the integration gate compares every one of the 105 and 95 scalar operations
  against dav1d, including the decoded value, active CDF, CDF update count,
  byte position, difference window, normalized range, and refill count;
- that exact comparison found and corrected a real state-model error: luma
  has its own coefficient-skip CDFs, while U and V share the mutated chroma
  coefficient-skip and trailing-skip CDFs;
- the same test compares all 48 reconstructed Y/U/V samples per fixture, while
  the manifest compares Pillow's exact RGB bytes. The two Pillow results are
  constant `(15, 91, 201)` and `(199, 37, 82)`;
- unsupported syntax remains a portable-path miss and does not narrow the
  existing native fallback. WASM still returns no decoded AVIF pixels until
  complete frame reconstruction and codec-mandated color conversion land;
- no public resize, crop, rotation, filtering, drawing, compositing,
  color-adjustment, generic transform, or mutable raster API was added. The
  prediction and inverse transform remain private AV1 decoder machinery;
- strict native all-target/all-feature Clippy, strict AVIF-only WASM Clippy,
  and strict all-feature WASM Clippy pass; and
- Coverage MCP run `3faaddef-251d-4ec6-8fd7-50e78d192cb6`, snapshot
  `39a02455-1368-450b-9c9c-70f52b446e42`, passes all seven test binaries with
  35,535/35,535 lines, 5,282/5,282 branches, 1,786/1,786 functions, and
  58,958/58,958 regions.

## Slice 7: First Portable Still Decode

Status: accepted.

### Closed implementation boundary

The two Slice 6 fixtures already reconstruct every visible sample of one
complete image. Slice 7 may turn that retained decoder state into a public
`DecodedImage` only for this exact syntax class:

- one primary still-image color item and no sequence track;
- one encoded color sample and no alpha plane;
- one independently decoded 4x4 key frame whose reconstructed leaf covers the
  full visible frame;
- 8-bit, non-monochrome, full-range YUV 4:4:4;
- color primaries 1, transfer characteristics 13, and BT.601 matrix
  coefficients 6; and
- no unsupported frame, block, filter, restoration, super-resolution, or film
  grain state hidden after the retained reconstruction.

Every condition is decided from parsed container and AV1 state. Fixture names,
file hashes, encoded-byte equality, dimensions alone, and native-target checks
must not select the portable path. Any input outside the closed class remains a
portable-path miss. Native builds retain the existing pinned libavif fallback;
WASM continues to return unsupported for inputs outside the closed class.

### Retained production state

`ValidatedAv1` will retain a private portable-still candidate containing:

- visible width and height from the completed AV1 frame;
- bit depth, monochrome flag, chroma subsampling, color range, primaries,
  transfer characteristics, and matrix coefficients from the accepted
  sequence header; and
- the three reconstructed stride-independent Y, U, and V planes.

The candidate is present only when the container has exactly the still-image
topology above and the AV1 parser has completed that same sample. A first leaf
observed inside a larger frame, animation, alpha item, multi-sample plane, or
unsupported color declaration must never be mistaken for a complete portable
image.

### Pinned conversion reference

The private AVIF output stage follows libavif 1.4.1 commit
`6543b22b5bc706c53f038a16fe515f921556d9b3` and the libyuv 1922
implementation used by Pillow's linked libavif:

- `src/codec_dav1d.c:185-216` maps dav1d layout, depth, range, CICP values, and
  decoded planes into the libavif image;
- `src/avif.c:696-713` defines Pillow's `avifRGBImageSetDefaults` state;
- `src/reformat_libyuv.c:760-840` selects libyuv's JPEG-range BT.601 matrix
  for full-range matrix-coefficients-6 input;
- `src/reformat_libyuv.c:932-1104` dispatches I444-to-RGB24 conversion;
- libyuv `source/row_common.cc:1371-1417` and `1534-1547` define the
  fixed-point BT.601 constants; and
- libyuv `source/row_common.cc:1645-1692` and `1852-1866` apply those integer
  constants, shift, saturate, and store RGB24.

The diagnostic oracle loads Pillow's `_avif` extension and records
`avifLibYUVVersion() == 1922`; regeneration fails if this version changes.
The pinned arithmetic uses `YG=16320`, `YB=32`, `UB=113`, `UG=22`, `VG=46`,
and `VR=90`. This is the target-independent scalar form of the same integer
conversion and therefore preserves Pillow's exact rounding on WASM.

The implementation remains inside the AVIF decoder. It is codec-mandated
materialization, not a reusable color-conversion, adjustment, filter, mutable
raster, or other image-processing API.

### Fixture and reverse-mapping evidence

The pinned AV1 diagnostic reports the same sequence declaration for both
fixtures: 4x4, 8-bit, full-range 4:4:4, primaries 1, transfer 13, and matrix 6.
Its decoded planes are:

| Fixture | Y | U | V | Pillow RGB |
| --- | ---: | ---: | ---: | ---: |
| `portable_lossless_a.avif` | 81 | 196 | 81 | `(15, 91, 201)` |
| `portable_lossless_b.avif` | 91 | 123 | 205 | `(199, 37, 82)` |

The existing Pillow manifest owns the exact 48 output bytes per fixture. The
pinned dav1d trace owns the exact 48 input Y/U/V samples. The Rust integration
gate will compare the retained production-path conversion directly with both
boundaries, so a decoder cannot pass by using a detached helper or by matching
only output dimensions and byte counts.

An initial implementation followed libavif's internal float fallback and
produced green `38` instead of Pillow's green `37` for the second fixture.
Reverse mapping proved that the retained Y/U/V planes were already exact and
that Pillow had selected libyuv's integer fast path. Reproducing that pinned
path fixes the first divergence at the codec output boundary rather than
altering AV1 reconstruction or special-casing the fixture.

### Acceptance criteria

Slice 7 is accepted only when:

1. the production AV1 result retains all frame and color fields used to decide
   the closed portable class;
2. the portable candidate is absent for animation, alpha, multi-sample,
   partial-frame, non-4:4:4, non-eight-bit, limited-range, or non-BT.601 input;
3. both committed fixtures decode through the portable production path to the
   exact Pillow RGB bytes on native builds, while all other native AVIF cases
   retain their previous fallback behavior;
4. the same portable implementation compiles under
   `wasm32-unknown-unknown` without target-specific arithmetic or semantic
   forks;
5. no public image-processing API, unsafe Rust, fixture special case, or new
   crate dependency is added;
6. strict native, AVIF-only WASM, and all-feature WASM Clippy gates pass; and
7. Coverage MCP remains the only test runner and reports exact 100% line,
   branch, function, and region coverage with every manifest row active.

### Accepted result

The slice was accepted on 2026-07-29:

- both closed-class fixtures retain exact AV1 frame/color metadata and
  reconstructed Y/U/V planes, then decode through the production portable
  path to Pillow's exact 48 RGB bytes;
- reverse mapping found that Pillow's libavif uses libyuv 1922's fixed-point
  I444-to-RGB24 path. Replacing the float fallback corrected fixture B from
  green `38` to Pillow's green `37`;
- the deterministic oracle records `avifLibYUVVersion() == 1922`, fails on a
  version change, and regenerated twice to the identical SHA-256
  `5117c0588cb6f4f69b2bde3473c2cb3f4e5955da0c9ecc580c0aa009460ca35e`;
- exact topology matching prevents alpha, animation, and multiple color
  samples from selecting the portable still result. Native fallback behavior
  remains unchanged outside the class, while the accepted class uses the same
  integer semantics on native and WASM;
- a zero-count coverage region exposed a redundant
  `state.sequence()?.clone()` after `state.finish()?`. `finish()` now returns
  the validated sequence directly, removing an impossible state instead of
  adding a synthetic test for dead code;
- no public resize, crop, rotation, filtering, drawing, compositing,
  color-adjustment, mutable raster, or reusable color-conversion API was
  added. AV1 reconstruction and YUV-to-RGB materialization remain private
  codec-required decode operations;
- strict native all-target/all-feature Clippy, strict AVIF-only WASM Clippy,
  and strict all-feature WASM Clippy pass; and
- Coverage MCP run `18f1e86e-02df-44c8-9710-118a88c3fb6b`, snapshot
  `e05ff54c-3a30-4da2-b799-ab69bcdd0547`, passes all seven test binaries with
  35,673/35,673 lines, 5,300/5,300 branches, 1,792/1,792 functions, and
  59,138/59,138 regions.

## Slice 8: Zero-Residual Lossless Transforms

Status: accepted on 2026-07-29.

### Closed implementation boundary

This slice extends the accepted 4x4, eight-bit, full-range, lossless 4:4:4
still-image class with the AV1 coefficient-skip path for a 4x4 transform whose
residual is identically zero. It does not add another partition, predictor,
transform size, subsampling mode, bit depth, alpha plane, frame topology,
filter, restoration mode, or color-conversion matrix.

The existing prediction boundary remains exact:

- partition `PARTITION_NONE`;
- block skip flag false;
- luma directional mode 1 with zero angle delta;
- chroma DC mode 0; and
- four lossless 4x4 transforms per coded plane, clipped to the visible 4x4
  frame.

For each plane, the first visible transform may now be either the already
accepted nonzero DC-only transform or an AV1 coefficient-skip transform that
reconstructs the predictor unchanged. The remaining three coded transforms
must still be consumed as zero transforms with the exact shared or
plane-specific CDF state established by dav1d. Unsupported entropy syntax
must still stop the portable attempt at its first divergence.

This is private AV1 decoder machinery. It must not create or expose a public
transform, predictor, residual, color conversion, or other image-processing
API.

### Reverse-mapped fixture selection

`scripts/explore_avif_constant_corpus.py` encodes a deterministic constant
4x4 RGB corpus with Pillow 12.2.0, libavif 1.4.1, libaom 3.13.2, quality 100,
speed 8, one thread, 4:4:4 subsampling, and disabled autotiling. It extracts
the AV1 color item without the Rust implementation and decodes it through the
pinned scalar dav1d 1.5.3 commit.

The sweep establishes two minimal fixtures:

| RGB source | File SHA-256 | AV1 item SHA-256 | dav1d Y/U/V | First-transform EOB Y/U/V | Pillow RGB SHA-256 |
| --- | --- | --- | --- | --- | --- |
| `(32,32,32)` | `f57c5df28dc28add5b9913c9d3cc0c0aae2e69e0087e7a8614674c8658987875` | `2c5985522a7fa47e7965b590c46e5d181213c12ae037741b3249587c2c3fc3ba` | `32/128/128` | `0/-1/-1` | `b4a53f2b248b5701814756a08eb3435e49117eda791610ff85dd22e8a6a86df3` |
| `(127,127,127)` | `40de5aecb3fb4c8b6ad9e242beda63204aae00f6be34694c14554aab91c330f8` | `d7b22ddc5148c3b7d3615f340f9c4b05e354bf76859fc5036b6484e900b835d5` | `127/128/128` | `-1/-1/-1` | `a1fa26e9a041c510e9f8412accef2e5e0cda5eddd97fa6db80b30400b7964d42` |

The first fixture proves mixed nonzero-luma and zero-chroma reconstruction.
The second proves the same zero path independently for luma and both chroma
planes. Both retain luma mode 1 and chroma mode 0. Gray 128 selects luma mode
0, while gray 129 and larger values select luma mode 2; those inputs are
deliberately excluded so this slice does not conceal a predictor expansion.

### Oracle and implementation method

Before production code changes, the two selected AVIFs are generated twice
and committed as manifest inputs. The pinned reconstruction generator is then
extended to record their complete scalar entropy operation streams,
plane-specific CDF mutations, transform EOB states, reconstructed Y/U/V
planes, and exact Pillow RGB output. The generator must run twice and produce
byte-identical JSON.

Implementation proceeds by reverse mapping the first differing operation from
the existing nonzero coefficient path. A zero visible transform must:

1. decode the correct coefficient-skip CDF for luma or shared chroma state;
2. return coefficient zero without consuming EOB, token, sign, or Golomb
   syntax that is absent from the stream;
3. consume the remaining coded zero transforms in their exact order and CDF
   contexts;
4. reconstruct the existing predictor unchanged; and
5. retain the complete-image and CICP checks already required by Slice 7.

No fixture hash, filename, RGB source value, or native-target condition may
select the path.

### Acceptance criteria

Slice 8 is accepted only when:

1. both selected fixtures regenerate deterministically with the exact hashes
   above and are active Pillow-oracle manifest rows;
2. the Rust production decoder matches every retained dav1d entropy
   operation, CDF state, EOB state, and reconstructed plane sample;
3. native and WASM use the same zero-residual implementation, and both
   fixtures materialize to Pillow's exact 48 RGB bytes through the portable
   path;
4. gray 128 and 129 remain portable misses until their predictor modes are
   implemented rather than being decoded with a fabricated mode;
5. no public image-processing API, unsafe Rust, special case, or new
   dependency is added;
6. strict native all-target/all-feature, AVIF-only WASM, and all-feature WASM
   Clippy gates pass; and
7. Coverage MCP remains the only test runner and returns exact 100% line,
   branch, function, and region coverage with every manifest row active.

### Accepted result

The zero-residual slice is accepted:

- the deterministic corpus sweep isolated coefficient-skip syntax without
  mixing in a new partition, predictor, transform size, color matrix,
  subsampling mode, alpha plane, or frame topology;
- `portable_lossless_gray_32.avif` proves mixed nonzero-luma and zero-chroma
  residuals, while `portable_lossless_gray_127.avif` proves zero residuals in
  all three planes;
- the production decoder consumes four coefficient-skip decisions from the
  exact luma or shared-chroma CDF when a plane's coded 8x8 lossless leaf
  contains four zero 4x4 transforms. It consumes no absent EOB, token, sign,
  or Golomb syntax and reconstructs the predictor unchanged;
- all 57 scalar operations for gray 32 and all 31 operations for gray 127
  match pinned dav1d state exactly, including every mutated CDF, byte
  position, difference window, range, refill count, EOB result, and all 48
  reconstructed Y/U/V samples;
- the reconstruction oracle was generated twice and retained the identical
  SHA-256
  `4ebda96be0cc487030ad8ef12c0eb1ee8f56ce6a8f735680e10c5aecee97faf7`;
- `portable_probe_gray_128.avif` and `portable_probe_gray_129.avif` are active
  fixture-based boundary cases. Native fallback retains their exact Pillow
  pixels, while the private portable classifier rejects their unimplemented
  luma modes 0 and 2;
- the manifest now has 1,036 active rows, zero planned decode rows, and zero
  unwired encode rows;
- no public or reusable image-processing operation, unsafe Rust, target fork,
  special case, or dependency was added;
- strict native all-target/all-feature Clippy, strict AVIF-only WASM Clippy,
  and strict all-feature WASM Clippy pass; and
- final Coverage MCP run `18a5e14e-8bf7-4d77-933a-63e94b30684a`, snapshot
  `f85b4ae3-8edc-4c41-9566-42d1637fb5e2`, passes all seven test binaries with
  35,679/35,679 lines, 5,302/5,302 branches, 1,792/1,792 functions, and
  59,153/59,153 regions.

## Slice 9: DC And Horizontal Luma Prediction

Status: accepted on 2026-07-29.

### Closed implementation boundary

This slice retains the complete Slice 8 topology and extends only the luma
intra-prediction choice:

- luma mode 0, AV1 DC prediction, for
  `portable_probe_gray_128.avif`; and
- luma mode 2, AV1 horizontal prediction with its encoded zero angle delta,
  for `portable_probe_gray_129.avif`.

Chroma remains mode 0. Every visible transform in both fixtures is a
coefficient-skip transform, so Slice 8 already owns coefficient parsing and
the new evidence isolates prediction and its conditional syntax. No new
partition, transform, nonzero AC coefficient, dimension, subsampling, bit
depth, alpha plane, color declaration, filter, or frame topology enters this
slice.

The predictor implementations remain private to AV1 reconstruction. They must
not be exported, generalized into pixel-buffer operations, or reused as a
public image-processing layer.

### Reverse-mapped evidence and method

The accepted constant-corpus diagnostic reports:

| Fixture | RGB source | File SHA-256 | AV1 item SHA-256 | dav1d Y/U/V | Luma mode | Pillow RGB SHA-256 |
| --- | --- | --- | --- | --- | ---: | --- |
| `portable_probe_gray_128.avif` | `(128,128,128)` | `26713256cc2769ab320d6017dca8b0f822dbdb03e66351e8ebc37bda64e440dc` | `b04631940f83ad563489ca1b9b0104fd217f0e405206509ad00322229854b182` | `128/128/128` | 0 | `2ac4dd6f486e2f061ebe8ce8b651dbdf25d71b88184d0bf308608cdcaae05309` |
| `portable_probe_gray_129.avif` | `(129,129,129)` | `649c4e452a350a51a9230070d768e432ff03bf7b3fb5700a50653f5f2887fc7a` | `1f00d88dbc7361f361b54d6a2d22247ec88370ca4925fce79ecc6d914f9f093b` | `129/128/128` | 2 | `b34e1e1e7cd63c9fb7069154ccd855d827a3dd3eca076232b4217745a2b6db57` |

Before production changes, the pinned dav1d generator must add both fixtures
to the complete reconstruction trace and regenerate it twice. The trace
determines whether angle syntax is present for each mode, the exact
first-block edge initialization, and the resulting 4x4 predictor samples.
The Rust implementation then follows the first differing scalar operation and
sample rather than inferring behavior from the final RGB value.

### Acceptance criteria

Slice 9 is accepted only when:

1. both existing boundary fixtures become complete pinned-dav1d reconstruction
   cases without changing their Pillow manifest references;
2. every retained scalar operation, conditional angle symbol, predictor
   sample, zero-residual sample, Y/U/V plane, and RGB byte matches its oracle;
3. unsupported luma modes remain portable misses at their first syntax value;
4. native and WASM compile and use the same private predictor code;
5. no public image-processing API, unsafe Rust, special case, target fork, or
   dependency is added;
6. strict native and both WASM Clippy gates pass; and
7. Coverage MCP remains the only test runner and reports exact 100% line,
   branch, function, and region coverage with all manifest rows active.

### Accepted result

The DC and horizontal luma-prediction slice is accepted:

- the reconstruction oracle now contains six deterministic fixtures and was
  generated twice with identical SHA-256
  `634d9e51ff3fbfe7ae3267f0392e28c70f630c9f5d376d4af08003c0be545a97`;
- DC mode consumes its luma-specific chroma-mode CDF and then the conditional
  8x8 `use_filter_intra` flag. The accepted fixture proves the flag is false
  and reconstructs all luma samples from dav1d's first-block DC value 128;
- horizontal mode consumes its own zero-angle CDF and chroma-mode CDF, then
  reconstructs all luma samples from dav1d's initialized left-edge value 129;
- vertical mode retains its prior zero-angle CDF, chroma-mode CDF, and
  initialized top-edge value 127;
- mode 0 matches all 32 scalar operations and mode 2 matches all 31, including
  complete CDF updates, arithmetic state, conditional syntax, three Y/U/V
  planes, and Pillow's exact 48 RGB bytes;
- all three supported predictor variants are selected only from decoded AV1
  syntax. No fixture name, hash, RGB value, or target condition participates;
- no public or reusable image-processing operation, unsafe Rust, dependency,
  or native/WASM semantic fork was added;
- strict native all-target/all-feature Clippy, strict AVIF-only WASM Clippy,
  and strict all-feature WASM Clippy pass; and
- Coverage MCP run `3b73dcf1-3ed7-4e8f-8a35-68734961c14e`, snapshot
  `3d22a4ac-5861-4a1b-8d59-805a73b2ad9c`, passes all seven test binaries with
  35,719/35,719 lines, 5,306/5,306 branches, 1,794/1,794 functions, and
  59,187/59,187 regions.

## Slice 10 Plan: 8x8 Lossless Leaf Geometry

Status: reverse-mapped and ready for pinned fixture generation.

### Closed implementation boundary

This slice changes only the visible geometry of the already accepted
lossless leaf:

- the image is 8x8 instead of 4x4;
- the frame still contains one level-4 `PARTITION_NONE` 8x8 leaf;
- each 8x8 plane still contains four 4x4 WHT transforms in coded order, but
  all four transforms now intersect the visible image instead of only the
  first;
- the first transform follows the already accepted zero-or-DC-only syntax,
  and the remaining three transforms are coefficient skips;
- luma remains one of the already accepted DC, vertical, or horizontal
  predictors, with chroma mode 0; and
- the bit depth, lossless quantization, 4:4:4 sampling, full-range BT.601
  color declaration, no-alpha still topology, and single-tile container
  contract remain unchanged.

The implementation must retain and reconstruct the four already coded
transform results per plane. It must not add a generic resize, tiling, image
processing, arbitrary block traversal, AC-coefficient, subsampling, bit-depth,
alpha, animation, or encoder capability.

### Corpus evidence and selected fixtures

The diagnostic command

```text
.oracle-venv/bin/python scripts/explore_avif_constant_corpus.py \
  --dav1d /private/tmp/image-star-dav1d-trace-build/tools/dav1d \
  --output <report> --size 8x8
```

was run twice over 16 grayscale levels and six saturated primaries. Both
reports were byte-identical with SHA-256
`8bec76a08ecf2188d91480d6ec5526da34cfa5273a1b6db538dfc13c91a4bcda`.
A second 27-case RGB cube over levels 17, 91, and 203 was also run twice and
was byte-identical with SHA-256
`b9408a3f6ee6ebe48e9a7e61549311453bfbf55a139bc969301472d1fe2b992f`.

All 49 cases retain first-block level 4, partition 0, chroma mode 0, WHT
transform type 16, constant reconstructed planes, and only the predictor and
zero-or-DC residual syntax already accepted in Slices 7 through 9. Four
fixtures isolate geometry while pinning all three accepted luma predictors:

| Planned fixture | RGB source | File SHA-256 | AV1 item SHA-256 | dav1d Y/U/V | First-transform EOB Y/U/V | Pillow RGB SHA-256 |
| --- | --- | --- | --- | --- | --- | --- |
| `portable_lossless_8x8_a.avif` | `(17,91,203)` | `b7f758da88a2a9835bcda1a709b1de1ce47e232113d7d67f027ec430bb089714` | `33e38958e3a5751452e07432ba1848ab0804e2cc95dd5ac82cc24929f9b36f28` | `81/196/81` | `0/0/0` | `1f403e7f414473b888fcba438d60d269e54fc1d04c802dd32f96fa657932b2ac` |
| `portable_lossless_8x8_gray_127.avif` | `(127,127,127)` | `1b6a333257e226a63b6b33b34da54917a02518cf91864618222f60c160a883b7` | `24febd98d55022cc5251683097f52d54de431bcec0241b5a53eaa3fa755388c6` | `127/128/128` | `-1/-1/-1` | `c24e73f000a4255a612416ecc4df81c9313e4c099877384712e4d8530dd7acbd` |
| `portable_probe_8x8_gray_128.avif` | `(128,128,128)` | `6a4f26af5a873630c21a85fa1b7a1337026991acac0a305ecd6dd059a84fba63` | `23c20673ecd99f469fd55a553b70f70645635aef8bbdcd9dd09de60e5db08096` | `128/128/128` | `-1/-1/-1` | `fa7b78cc215df21d7ce54d8c3c6637c326dab95c10fbc12263101365973f4268` |
| `portable_probe_8x8_gray_129.avif` | `(129,129,129)` | `43ab3fc61b01b3b173323da584abe8a07c06eb4fcf4254cf8e04f3333f654237` | `1de8b894b33da9bfd62bd64ee894a3106a0b33b94932a0e6faa2c2fcd9405931` | `129/128/128` | `-1/-1/-1` | `fca06fef259b9ebb452449c7feda724ccec06a4a76b2b4fb1e6420a0beac435e` |

The first case proves nonzero DC reconstruction in all three planes without
RGB clipping. The second proves the zero-residual path in all twelve visible
transforms. The last two keep every transform zero while proving the already
accepted DC and horizontal modes under the new geometry.

### Oracle and implementation method

Before Rust decoder changes:

1. add the four inputs to `generate_test_assets.py` and regenerate each twice;
2. add all four as active Pillow-oracle manifest rows with exact 8x8 RGB bytes;
3. extend `generate_av1_reconstruction_refs.py` to accept the pinned per-case
   dimensions and record all twelve transform decisions, full scalar entropy
   state, three 8x8 planes, and Pillow's 192 RGB bytes; and
4. regenerate the reconstruction reference twice and require byte identity.

Production changes then follow the first differing retained transform or
sample:

1. preserve four DC results per plane instead of discarding the three
   trailing coefficient skips;
2. reconstruct the first 4x4 WHT result, then reproduce dav1d's first-frame
   intra-prediction edge propagation through the top-right, bottom-left, and
   bottom-right zero-residual transforms;
3. make the private reconstructed-plane storage match the validated 4x4 or
   8x8 dimensions without exporting a raster operation;
4. require exactly 4x4 or 8x8 in the portable classifier and verify all three
   plane lengths before color conversion; and
5. leave every other size as a portable miss so native fallback behavior is
   unchanged.

No fixture filename, hash, RGB value, target architecture, or final decoded
pixel may select the decoder path.

### Acceptance criteria

Slice 10 is accepted only when:

1. all four fixtures regenerate deterministically with the exact hashes above and
   are active Pillow-oracle manifest rows;
2. every dav1d scalar entropy operation, updated CDF, transform skip/EOB
   state, reconstructed 8x8 Y/U/V sample, and Pillow RGB byte has an exact
   Rust match;
3. all six pre-existing 4x4 fixtures remain byte-exact and retain identical
   entropy references;
4. 4x4 and 8x8 native decoding use the same portable implementation, and
   AVIF-only WASM accepts both dimensions without a native codec;
5. unsupported dimensions and syntax remain portable misses rather than
   fabricated decodes;
6. no public image-processing API, unsafe Rust, native/WASM semantic fork,
   special case, or dependency is added;
7. strict native all-target/all-feature, AVIF-only WASM, and all-feature WASM
   Clippy gates pass; and
8. Coverage MCP remains the only test runner and returns exact 100% line,
   branch, function, and region coverage with every manifest row active.

### Accepted result

The 8x8 lossless-leaf geometry slice is accepted:

- four deterministic 8x8 fixtures cover nonzero DC reconstruction, all-zero
  reconstruction, and DC, vertical, and horizontal luma prediction. Their
  committed file, AV1-item, Y/U/V, and Pillow RGB hashes match the corpus table
  above;
- expanded dav1d instrumentation records all four luma and all eight chroma
  transform decisions rather than only the top-left transform. The complete
  ten-fixture oracle was generated twice with identical SHA-256
  `3a9b81dae6ee8db0698b8d564b265d2bc2f779c6ba310d36ebff7e06cc1f6261`;
- reverse mapping proved that the 8x8 entropy-operation arrays are identical
  to the corresponding 4x4 syntax. The geometry difference occurs during
  reconstruction: after the first DC-only 4x4 result, dav1d's first-frame
  intra edges propagate that reconstructed value through the three skipped
  transforms;
- the production decoder now retains all four coefficient decisions, stores
  visible planes at their validated size, reproduces that codec-mandated edge
  propagation, rejects unimplemented super-resolution, checks all three plane
  lengths, and converts exactly 64 Y/U/V samples to Pillow's 192 RGB bytes;
- all six pre-existing 4x4 entropy-operation arrays are byte-identical to
  their prior oracle, and their pixels remain unchanged;
- the parity manifest now has 1,040 active rows, zero planned decode rows, and
  zero unwired encode rows;
- no public image-processing operation, unsafe Rust, dependency, target fork,
  or fixture-selected production path was added;
- strict native all-target/all-feature Clippy, strict AVIF-only WASM Clippy,
  strict all-feature WASM Clippy, third-party-license verification, formatting,
  and whitespace checks pass; and
- final Coverage MCP run `cb2b4e96-08f1-492d-bc5e-e253023e0bcd`, snapshot
  `f6b0429d-6fd4-4047-adc1-d95efd62350d`, passes all seven test binaries with
  35,761/35,761 lines, 5,314/5,314 branches, 1,796/1,796 functions, and
  59,289/59,289 regions.

## Slice 11 Plan: 4x8 And 8x4 Visible Rectangles

Status: reverse-mapped and ready for pinned fixture generation.

### Closed implementation boundary

This slice retains the complete Slice 10 coded topology:

- one level-4 `PARTITION_NONE` lossless leaf;
- one padded 8x8 coded block containing four row-major 4x4 WHT transforms per
  plane;
- zero-or-DC-only coefficient syntax with the remaining transforms skipped;
- the accepted DC, vertical, and horizontal luma predictors and chroma mode
  0; and
- 8-bit full-range BT.601 4:4:4, one tile, no alpha, and no super-resolution.

Only the visible dimensions change to 4x8 or 8x4. The decoder must reconstruct
the same codec-mandated padded 8x8 block and retain its declared top-left
visible samples. That visibility rule stays private to AV1 reconstruction and
must not become a public crop, view, or image-processing API.

The 12x12 and 16x16 sweeps select a level-3 leaf and are explicitly excluded:
they require a separate partition/block-context slice even though their
constant-source residual syntax looks similar.

### Corpus evidence and selected fixtures

The default 22-case corpus was generated twice for each orientation. The 4x8
reports were byte-identical with SHA-256
`038eff9abb9c3ebd6019d3e72176bfb95a90fe2bd908dc612562a2fab1afeb7d`;
the 8x4 reports were byte-identical with SHA-256
`3c6d0d7efc580652ae9a8bf45d298f6a14f8ed9c82d81b3001c2eec43f10aa56`.
The 27-case RGB cube over levels 17, 91, and 203 was also generated twice per
orientation. Its 4x8 report SHA-256 is
`611454bbcdcb635444bfffda161cc8658bf934c29aa456c23ceee87d2ad23d1f`;
its 8x4 report SHA-256 is
`c6f4fcbe879d9fa566cb46c2344d21b509933d4a8325dbad70a284a499605f1b`.

All 98 cases retain level 4, partition 0, chroma mode 0, WHT transform type 16,
constant reconstructed planes, and only syntax already accepted through Slice
10. Eight fixtures pin nonzero and zero residuals plus every accepted
predictor in both orientations:

| Planned fixture | RGB | File SHA-256 | AV1 item SHA-256 | Y/U/V | Pillow RGB SHA-256 |
| --- | --- | --- | --- | --- | --- |
| `portable_lossless_4x8_a.avif` | `(17,91,203)` | `cb0b3da2e8a31551ba34dc01d126a69aa9db266f5403a035291df0dac6c6e618` | `5059fcb71dc50b437030e05d6c1a30111d696bab4950fcb6dc2c8f2dc55be940` | `81/196/81` | `116d1d3509d9d2a7558a2fad832f923fc1193f04b8e0e57946f49e57fa045475` |
| `portable_lossless_4x8_gray_127.avif` | `(127,127,127)` | `ce82141d38ba572468c4ca15dec6e34f21a95597e58c37ffc48eefd941f9b24d` | `ef7a964ff88a8d5623fa0fc350644f426beeb2a94d193fd05ada0d817edc2089` | `127/128/128` | `faa8c27b41b2603cd12911cd93ee3953ff1f98c9fba83fdeef738cc8406c4b3f` |
| `portable_probe_4x8_gray_128.avif` | `(128,128,128)` | `4f4617e81863740af3f2342f115263e403318540e6766832956fd465a95bc832` | `ea7e7dbc9f11eb38f705ffa4d8dfb94216e9690a2990a18408085b1c3c64104b` | `128/128/128` | `1b34669db94decae583e183ee2ffeb07cf504b9f52fae0056c5cf343325157e4` |
| `portable_probe_4x8_gray_129.avif` | `(129,129,129)` | `0445afe75c7fdf364f93977071cca852eac150689c6cee27caa3756d77349736` | `ddb750963f906ae7097942398b4982ddf6e04e22dac28dfbfa30d2fb31241f87` | `129/128/128` | `780832a7ab39814257a857d37a67ab541a1152afbcf6a1883a16ad32c264ff4e` |
| `portable_lossless_8x4_a.avif` | `(17,91,203)` | `1a63667e7dc169346398a887b7e058e35908001823c5e2f88e591b292f9b15e8` | `2a149cac3088504a7cd56cf917df75537e11962e3052c28946deed9b0bf11fa0` | `81/196/81` | `116d1d3509d9d2a7558a2fad832f923fc1193f04b8e0e57946f49e57fa045475` |
| `portable_lossless_8x4_gray_127.avif` | `(127,127,127)` | `5bab699fbac28f885286c88d9dfc89c7211851fa508466ad1983400281c3bffe` | `0f4442ad9f05b20a52016e9a8587056a6f13659da1f54f1c58645011dcf2d4c9` | `127/128/128` | `faa8c27b41b2603cd12911cd93ee3953ff1f98c9fba83fdeef738cc8406c4b3f` |
| `portable_probe_8x4_gray_128.avif` | `(128,128,128)` | `fbe50a5bede60325dc8d41f980167c44085d6d4a4bcfbd7c8773eb36693aaef8` | `5cfdc4ac2d8f707c82ae240896416456e507eeca2b1d5dc6fbd8805cd011861a` | `128/128/128` | `1b34669db94decae583e183ee2ffeb07cf504b9f52fae0056c5cf343325157e4` |
| `portable_probe_8x4_gray_129.avif` | `(129,129,129)` | `1a42be7e964815aea58848948cf8b79272f48161449319f57d2ad49b928a68e9` | `d736970fa74d2db24285bbd091666e208f501ea14a21810f605142fb85a2e399` | `129/128/128` | `780832a7ab39814257a857d37a67ab541a1152afbcf6a1883a16ad32c264ff4e` |

### Oracle and implementation method

Before Rust changes, all eight fixtures must be generated twice, activated in
the Pillow manifest, and added to the complete pinned-dav1d reconstruction
oracle. The oracle must prove:

1. all four coded transform decisions per plane and every scalar entropy/CDF
   state;
2. the full padded-block prediction and reconstruction log;
3. exact visible 4x8 or 8x4 Y/U/V rows; and
4. Pillow's exact 96 RGB bytes.

Production then changes only the private visibility step:

1. reconstruct all four transforms into one fixed 8x8 coded-plane buffer;
2. copy the first `width` samples from each of the first `height` coded rows
   into the retained plane;
3. admit exactly the four dimension pairs 4x4, 4x8, 8x4, and 8x8 while
   continuing to reject super-resolution; and
4. derive RGB capacity and plane-length validation from the accepted visible
   dimensions.

No fixture identity, decoded color, architecture, arbitrary rectangle, or
public processing helper may select or implement the path.

### Acceptance criteria

Slice 11 is accepted only when:

1. all eight files, AV1 items, Y/U/V planes, and Pillow outputs match the
   hashes above after deterministic regeneration;
2. their full entropy operations and reconstruction logs match pinned dav1d;
3. the ten Slice 10 fixtures remain byte-exact;
4. unsupported dimensions, level-3 leaves, super-resolution, and other syntax
   remain portable misses;
5. native and WASM use the same production reconstruction code;
6. no public image-processing API, unsafe Rust, dependency, special case, or
   target fork is added;
7. strict native and both WASM Clippy gates pass; and
8. Coverage MCP remains the only test runner and returns exact 100% line,
   branch, function, and region coverage with every manifest row active.

### Accepted result

The visible-rectangle slice is accepted:

- all eight fixtures regenerate with the exact file and AV1-item hashes above,
  and their active Pillow rows retain exact 96-byte RGB references;
- all eighteen reconstruction cases were generated twice with byte-identical
  oracle SHA-256
  `d7d74cc649483db52b4405cd7a4776eb9eb775922e6b30aea4fd943459000f8b`;
- every rectangle's scalar entropy stream and CDF state matches its
  corresponding accepted 8x8 case. The nonzero fixtures each match the same
  105-operation stream, proving that visibility does not alter coded syntax;
- the production decoder reconstructs one private padded 8x8 plane, retains
  only its AV1-declared 4x4, 4x8, 8x4, or 8x8 top-left visible rectangle, and
  validates the resulting 16-, 32-, or 64-sample plane before conversion;
- 12x12 and 16x16 remain portable misses at their level-3 partition boundary;
- the manifest now has 1,048 active rows, zero planned decode rows, and zero
  unwired encode rows;
- no public crop/view/processing API, unsafe Rust, dependency, target fork, or
  fixture-selected production path was added;
- strict native all-target/all-feature Clippy, strict AVIF-only WASM Clippy,
  strict all-feature WASM Clippy, formatting, whitespace, and third-party
  license verification pass; and
- Coverage MCP run `97792401-eb39-4a3f-b6fe-60209f223c60`, snapshot
  `aba25439-30e7-4fa6-9257-9176442385d1`, passes all seven test binaries with
  35,774/35,774 lines, 5,310/5,310 branches, 1,796/1,796 functions, and
  59,292/59,292 regions.

## Slice 12 Plan: Level-3 Padded 16x16 Leaf

Status: accepted.

### Closed implementation boundary

This slice adds exactly two declared square geometries, 12x12 and 16x16.
Both select one level-3 `PARTITION_NONE` lossless leaf and reconstruct a
private padded 16x16 coded plane. Each plane contains sixteen row-major 4x4
WHT transforms. A 12x12 frame retains the declared top-left 12x12 rectangle;
a 16x16 frame retains the complete coded plane.

Every other accepted constraint remains unchanged:

- 8-bit, all-lossless, full-range BT.601, 4:4:4, three color planes;
- one tile, one still color item, no alpha, and no super-resolution;
- DC, vertical, or horizontal luma prediction and chroma mode 0;
- a zero or DC-only first transform with every remaining transform skipped;
  and
- the same target-independent Rust path for native and WASM.

The visibility copy is AV1 decoder-private codec machinery. This slice does
not add a crop, resize, transform, color-conversion, raster, or other public
image-processing API.

### Deterministic corpus and reverse-mapping evidence

The existing diagnostic tool was extended with repeatable `--color` probes,
`--full-trace`, and an exact-source `--dav1d-source` build path. Its default
schema and previously pinned report hashes remain unchanged. For each
dimension, the following command was run twice:

```text
.oracle-venv/bin/python scripts/explore_avif_constant_corpus.py \
  --dav1d-source /private/tmp/dav1d-1.5.3 \
  --meson /private/tmp/image-star-dav1d-build-tools/bin/meson \
  --ninja /private/tmp/image-star-dav1d-build-tools/bin/ninja \
  --python-path /private/tmp/image-star-dav1d-build-tools \
  --size <12x12-or-16x16> \
  --color 17,91,203 --color 127,127,127 \
  --color 128,128,128 --color 129,129,129 \
  --full-trace --output <report>
```

Both pairs were byte-identical. The 12x12 full-trace report SHA-256 is
`cb25558d576281e1929ea889ed8d7827e23714b19a880100d757bf550ff74318`;
the 16x16 report SHA-256 is
`f761180b29567ba71dc3cf4a159e2a1fa58a9821dae40bcbdcdcf4ac0824c50b`.
The executable was built from dav1d commit
`b546257f770768b2c88258c533da38b91a06f737` with assembly disabled.
Pillow 12.2.0, libavif 1.4.1, libaom 3.13.2, and dav1d 1.5.3 remain the
pinned oracle stack.

The traces prove:

1. 12x12 and 16x16 select level 3, partition context 0, symbol 0;
2. their tile entropy operations are identical for the same RGB source;
3. each plane visits sixteen 4x4 WHT transforms;
4. an all-zero plane decodes sixteen coefficient-skip decisions through its
   base luma or chroma CDF;
5. after a nonzero top-left DC, the right transform at index 1 and lower
   transform at index 4 use the residual-neighbor skip CDF; the other
   thirteen skipped transforms use the base CDF; and
6. the shared chroma CDF mutations continue from U into V exactly as in
   dav1d.

For the nonzero probe, each dimension has 177 traced scalar operations.
For gray 127 and gray 129 it has 103; gray 128 has 104 because DC prediction
also consumes the filter-intra decision. The decoded Y/U/V planes are
constant for every selected case.

### Selected fixtures

| Planned fixture | RGB | File SHA-256 | AV1 item SHA-256 | Y/U/V | Pillow RGB SHA-256 |
| --- | --- | --- | --- | --- | --- |
| `portable_lossless_12x12_a.avif` | `(17,91,203)` | `daccd674b98dc26baad851ef95d75e6099c0397db5d8e28fb7f5f1f6eef9ac6c` | `0d4e6a42be0adf2360b5dee20928f1f44b512d7fadaf0f820729670daa52c7ef` | `81/196/81` | `cbc97cf0c2652e60e6e36611be9869444f603abf5f48b292a03d340f501320f8` |
| `portable_lossless_12x12_gray_127.avif` | `(127,127,127)` | `6120db571a64651ce6a3579863b808a9f68d60474235fb80d32c53448990ab0b` | `ffc8e97ed9f99e3c2a7c94a74a224f72d95591430a012c3e5d1ec3a0236d319e` | `127/128/128` | `cb4987527501d0915664b8e624e5f51ebbf5f48b52917058615c1f3b96764076` |
| `portable_probe_12x12_gray_128.avif` | `(128,128,128)` | `fd155de27375f8569d97e0e0934d3f287b7ad41a18933dd98b4c9c95647e483a` | `2691e30d6e3179c72efc4db827053f03f20a766820339861001d7846162eb535` | `128/128/128` | `cc0fcf371bdd305ff6099895e60aac93968bf0358724de1678979a37a9bd7a17` |
| `portable_probe_12x12_gray_129.avif` | `(129,129,129)` | `1e9c6b6a39e1fe7b73686fe6d1ac75770abaef9e28b173ec938a1937de02bb78` | `4fcea30677b820d749692f46446d6fed1e9028a28d96e47209944ced5f05da04` | `129/128/128` | `143efd9552ea35a74333bbfc58d10ae5a0eccfe76d2283c05b2b4a9391c346cd` |
| `portable_lossless_16x16_a.avif` | `(17,91,203)` | `04de5e1b6e056c08fdb33131d04dec6708b1ee4912c515fda6d973a29e592381` | `55131042f7c551290904960bbc78265627c0a553c5220b3ab1e3766d3b39053b` | `81/196/81` | `8bdcc97ae19b09ec3d6b76a7d59f13d4aa3dd7a06d21db706f2a1d15caaa0431` |
| `portable_lossless_16x16_gray_127.avif` | `(127,127,127)` | `db4305845c2773ff873835b6d401f77635e6f5aafab08204486caf602945fd49` | `f146132038c22ee2c8993f4a19e58bd750c37a68733c8ef939ba4bf51a724a20` | `127/128/128` | `cbab715ff6cfaa81c9b09e014dc1406ceff24034caa265de65f9f948c5434807` |
| `portable_probe_16x16_gray_128.avif` | `(128,128,128)` | `eca269fc4be5813d9865a0b8c0db9fd652c58474cc583a208e15bcbaab0bd7cf` | `6f31c2fa86dae6298140fff86cfa662ab3dd779f71bb5ca12a9f7d30d9a15578` | `128/128/128` | `7f3e5e4e65eca4390e9242558012bc9bdad133d7ac9f6aed53fa156a2288f73b` |
| `portable_probe_16x16_gray_129.avif` | `(129,129,129)` | `543643938a746a7e68daf23c118fab0673dcbc36de80eddf8d1603940814015c` | `a7ead325c0e01e753faa8038c921b2c875acbde2e11cbf220e3311b33b6a2b6d` | `129/128/128` | `15dc2c3b0ea25a84b4994b9a73dbcf65eef174bad152c689cc1945843b543657` |

### Oracle and implementation method

Before Rust changes:

1. generate all eight files twice and require the hashes above;
2. add all eight as active Pillow-oracle manifest rows with exact RGB bytes;
3. expand reconstruction logging from two to four transform coordinates in
   each direction and add the eight fixtures to the pinned dav1d oracle;
4. regenerate that oracle twice and require byte identity; and
5. make the Rust coverage bridge compare every entropy operation, CDF state,
   visible Y/U/V row, and Pillow RGB byte.

Production then follows the reverse map:

1. parameterize private coefficient decoding by a validated 2x2 or 4x4
   transform grid;
2. select the neighbor-context skip CDF only at transform indices 1 and
   `grid_width`, after a nonzero first DC;
3. reconstruct into a fixed 8x8 or 16x16 private coded-plane buffer and
   retain the declared visible rectangle;
4. admit level 3 only for 12x12 and 16x16 while preserving the existing level
   4 dimension set; and
5. keep unsupported levels, dimensions, partitions, residuals, or metadata as
   portable misses so native fallback remains unchanged.

No filename, fixture hash, decoded color, target architecture, or final pixel
may select the production path.

### Acceptance criteria

Slice 12 is accepted only when:

1. all eight fixture, AV1-item, decoded-plane, and Pillow hashes match the
   table after deterministic regeneration;
2. all twenty-six reconstruction cases match the complete pinned dav1d
   entropy and reconstruction oracle;
3. all eighteen prior cases remain byte-exact;
4. native and WASM use the same private reconstruction implementation;
5. unsupported syntax remains a portable miss;
6. no public image-processing API, unsafe Rust, dependency, target fork, or
   fixture-selected special case is added;
7. strict native all-target/all-feature, AVIF-only WASM, and all-feature WASM
   Clippy gates, formatting, whitespace, and third-party-license verification
   pass; and
8. Coverage MCP remains the only test runner and returns exact 100% line,
   branch, function, and region coverage with every manifest row active.

### Accepted result

The level-3 padded-leaf slice is accepted:

- all eight fixtures regenerate twice with the exact file hashes in the table,
  and their AV1-item, decoded-plane, and Pillow RGB hashes match the pinned
  references;
- the twenty-six-case reconstruction oracle was generated twice with
  byte-identical SHA-256
  `67ba56b192eeafcab498fe9559272835f13e399bc1b24a2ba1716fdfaa4aeb21`;
- Rust matches every pinned scalar entropy operation and CDF mutation,
  including sixteen base-context skips for an all-zero plane and the exact
  right/lower residual-neighbor contexts after a nonzero top-left DC;
- reverse debugging isolated the only initial mismatch to dav1d's
  block-size-indexed filter-intra CDF: `BS_8x8` uses inverse threshold 24,902
  while `BS_16x16` uses 20,360. The implementation now selects that threshold
  from the validated coded-grid width;
- the level/dimension classifier is the sole owner of the 2x2-versus-4x4
  transform-grid invariant. The reconstruction helper receives the proven
  width instead of retaining an unreachable duplicate geometry branch;
- the decoder reconstructs private padded 8x8 or 16x16 coded planes and
  retains only the AV1-declared visible rectangle. No crop, resize, raster
  transform, or other public image-processing API was added;
- the parity manifest now has 1,056 active rows, zero planned decode rows, and
  zero unwired encode rows;
- strict native all-target/all-feature, AVIF-only WASM, and all-feature WASM
  Clippy gates, formatting, whitespace, the 19-file third-party legal audit,
  and the source-package legal-file inventory pass; and
- Coverage MCP run `59e48553-3e4d-4ecd-ba80-3b115d0ffbaf`, snapshot
  `a7b769df-b233-478d-9a52-1ad1d3be2b00`, passes all seven test binaries with
  35,811/35,811 lines, 5,316/5,316 branches, 1,797/1,797 functions, and
  59,334/59,334 regions.

## Slice 13 Plan: Level-3 12x16 And 16x12 Visibility

Status: accepted.

### Dimension sweep and closed boundary

Every unimplemented 4-pixel-aligned rectangle within a 16x16 coded area was
encoded twice for RGB `(17,91,203)`, `(127,127,127)`, `(128,128,128)`, and
`(129,129,129)`. All ten report pairs are byte-identical:

| Declared size | Report SHA-256 | First partition symbols in probe order | Classification |
| --- | --- | --- | --- |
| 4x12 | `7070471198c03bc0ca1d9cbd9c4db568374cdb07caeca73f21700ccf2e454aff` | `3,2,2,2` | content-dependent partition tree |
| 4x16 | `b2e4ee147fa54b9e49b902d8ec0756a30ac8adb317fadf88a4fc1e80d57e5150` | `3,2,2,2` | content-dependent partition tree |
| 8x12 | `28d43fdd3b60db4f1702fa3309790bf03fc4259b0972346c5011bd6df6a7a073` | `3,2,2,2` | content-dependent partition tree |
| 8x16 | `0d5f9f124901d350e5a27f2d5cf25c5c6d7fb4bfaab64b1dcfb1f39e77ffca8a` | `3,2,2,2` | content-dependent partition tree |
| 12x4 | `a09ee9adbdb7ca2aaa048e2ba7af0f6b46069597afbfc15d4f83489960933ba0` | `3,1,1,1` | content-dependent partition tree |
| 12x8 | `0e9a056309098437cfd3c50f6fa3ee20a2a6e1b89ba2a5998c8d46321b398401` | `3,1,1,1` | content-dependent partition tree |
| 12x16 | `05f13f7ef92fbb3e2698aa07a7911aff46a6f67f282eb85f8ac004f9a8a6be03` | `0,0,0,0` | Slice 12 coded class |
| 16x4 | `9eeb73ce057759ae35278d03bd44e3b32b7a4fa31d7d27c429f885a6934ff220` | `3,1,1,1` | content-dependent partition tree |
| 16x8 | `72a233252c95ffc7566cebdc6b3c27617b177dbc5df717e4d9606c8c5b6521a1` | `3,1,1,1` | content-dependent partition tree |
| 16x12 | `74614f657061366c5f51d9a7e38781f1cd9912931ab12727b0dced32dd03753e` | `0,0,0,0` | Slice 12 coded class |

Only 12x16 and 16x12 are admitted. They retain one level-3
`PARTITION_NONE` leaf, sixteen row-major 4x4 WHT transforms per plane, the
same zero-or-DC residual class, and the same predictor and color constraints
as Slice 12. The decoder reconstructs the complete private 16x16 coded plane
and retains the declared top-left 12x16 or 16x12 rectangle.

The eight other dimensions are explicitly excluded. Their first partition is
not merely geometry-dependent: the selected nonzero probe uses symbol 3 while
zero-residual probes use horizontal symbol 1 or vertical symbol 2. Supporting
them requires recursive partition traversal and multiple leaf reconstruction,
not a visibility-list extension.

### Full-trace equivalence

The 12x16 and 16x12 selected corpora were also generated twice through a
fresh scalar build of exact dav1d commit
`b546257f770768b2c88258c533da38b91a06f737`. Their full reports are
byte-identical:

- 12x16:
  `5ea46356126115a9435f2f96279fe580a8ac13e1db3d2ce3beb9a999542f0bed`;
- 16x12:
  `26b1fcce23a0a4bcb89a224fc8fb3a1ca86a6d054fcce09f6e0db7c1fe33f318`.

For each RGB probe, the 12x16, 16x12, 12x12, and 16x16 scalar entropy arrays
are exactly equal. The operation counts remain 177 for the nonzero probe, 103
for gray 127 and 129, and 104 for gray 128.

### Positive fixtures

| Planned fixture | RGB | File SHA-256 | AV1 item SHA-256 | Y/U/V | Pillow RGB SHA-256 |
| --- | --- | --- | --- | --- | --- |
| `portable_lossless_12x16_a.avif` | `(17,91,203)` | `67e0005a989d761d36df0ddb12e53f1535a6a04e3606b97dfd33829949bc30ca` | `e33c3f05e89fed6e06022b40ef11aa56c464a0b76abf29d7e925f2e4ad43e48d` | `81/196/81` | `f6b42085d682a064da2a9956545f33ae7595b288f7589e8e498c62e6bc26e874` |
| `portable_lossless_12x16_gray_127.avif` | `(127,127,127)` | `4597c81363e602580e94625c7a01b029201a533489f404465698a194b9531e31` | `05edd6234c9a496585fc05da30e7450a75292526139e5d8ccb63ddb8922d5941` | `127/128/128` | `1b9924ee11c55d5fd4d944003b8b272c1f4ce12ea8e800c33563bed483fa406d` |
| `portable_probe_12x16_gray_128.avif` | `(128,128,128)` | `3406413b66b7f10d4d760a26b62f5f74c9f13d7a245557b8ba79fd340614b7a4` | `59a05f2491399a7170c1a69831032da9058d86edfe414ed5d42c7381ccda93b1` | `128/128/128` | `af1857bf5516aa3e2e39b6842559746fa7b45daa8dc4cc6675ad86e0cfe425b9` |
| `portable_probe_12x16_gray_129.avif` | `(129,129,129)` | `eac42af171713fab4c6535de7f3c1874b5c9f14669bcb36b8c5df53b927bfa9f` | `4f967b272ca544e7d3687bba48811cc5c92430b9d4fdf50d13a25b4c25d87213` | `129/128/128` | `5269c00892aff8abcc6a4da60b82b890936aef6b1aa24c6b713c5a80a831c0b9` |
| `portable_lossless_16x12_a.avif` | `(17,91,203)` | `423d91243ff4e7d42bb9b77cf255dab869e31e3e12a306f139c2c73a3df9a807` | `1feae6ab8e5804235c118103899e8da35ccf74b17fd457e9e8a4362dd7da95c0` | `81/196/81` | `f6b42085d682a064da2a9956545f33ae7595b288f7589e8e498c62e6bc26e874` |
| `portable_lossless_16x12_gray_127.avif` | `(127,127,127)` | `5df228fa447ee7f788c5bf486520f1d51ecaff4db147dbc86c7d9a804b40b60a` | `3711798e23d739b0d8319e7bfabec4d54703ef43e999aa3412adfe356fa328c2` | `127/128/128` | `1b9924ee11c55d5fd4d944003b8b272c1f4ce12ea8e800c33563bed483fa406d` |
| `portable_probe_16x12_gray_128.avif` | `(128,128,128)` | `f5873bf081f14f72dad784bf4c2835d42ee4a68482da4f964a0d4ea7902d8a12` | `16f4417f74f6a3ce1501b05ae26b27eabbc6a7ecd5f74e6201ab134c0289a25f` | `128/128/128` | `af1857bf5516aa3e2e39b6842559746fa7b45daa8dc4cc6675ad86e0cfe425b9` |
| `portable_probe_16x12_gray_129.avif` | `(129,129,129)` | `ca46b221a159dc66999b184e1655f696ad4ef05512d200591f9769ef0851424b` | `53c7b20d41f5a54617691af3f5cd5109d9d740f6bf790f2eb7412d10895daa9a` | `129/128/128` | `5269c00892aff8abcc6a4da60b82b890936aef6b1aa24c6b713c5a80a831c0b9` |

### Partition-boundary fixtures

Four valid native-oracle fixtures pin the excluded recursive classes:

| Planned fixture | RGB | Partition | File SHA-256 | Pillow RGB SHA-256 |
| --- | --- | ---: | --- | --- |
| `partitioned_4x12_a.avif` | `(17,91,203)` | 3 | `0aa4e381b6412dd1ffa92a51e5bd4519ce038261485f59d1decd7ef5777690f8` | `09fddd84398ad9a9d3ce8b981fea278a82e6b1fa62483fa0ef3c45cd484ae29e` |
| `partitioned_4x12_gray_127.avif` | `(127,127,127)` | 2 | `a64e56b075bd724e0eeb25f29913010ff9fa3bd89fbfc3ba27eaa121fac0746f` | `35fc07c937c1c3d13641f32cdc94ce1315ec420dd26e12b81a4651cfc1786ee3` |
| `partitioned_12x4_a.avif` | `(17,91,203)` | 3 | `631fe6bf1c2f72cc60acdbb5e682d8195c512f67a5c8f38bd6ae10fdb7a8c59d` | `09fddd84398ad9a9d3ce8b981fea278a82e6b1fa62483fa0ef3c45cd484ae29e` |
| `partitioned_12x4_gray_127.avif` | `(127,127,127)` | 1 | `8050ba2808d418cb39c1462a71d061e261582b2f4dffc255a5cfc65cf26e1fd2` | `35fc07c937c1c3d13641f32cdc94ce1315ec420dd26e12b81a4651cfc1786ee3` |

They remain successful Pillow/native decode rows but must return no private
portable first-leaf reconstruction. This makes rejection evidence
fixture-based instead of relying on a synthetic byte buffer.

### Implementation and acceptance

Before production changes:

1. generate all twelve fixtures twice and require the hashes above;
2. activate all twelve in the Pillow manifest;
3. add only the eight `PARTITION_NONE` fixtures to the complete dav1d
   reconstruction oracle;
4. require the four boundary fixtures to miss the private portable
   reconstruction path; and
5. regenerate all Pillow and dav1d references twice.

Production changes are limited to admitting level-3 dimensions 12x16 and
16x12 and validating their 192-sample planes. Existing grid-width,
coefficient-context, transform, prediction, and color code must remain shared.

Slice 13 is accepted only when:

1. all twelve fixture and Pillow hashes match, and the eight positive AV1
   items and planes match pinned dav1d;
2. all thirty-four reconstruction-oracle cases match every scalar entropy
   operation, CDF state, visible plane sample, and RGB byte;
3. the four partition-boundary fixtures remain private portable misses;
4. all twenty-six prior reconstruction cases remain byte-exact;
5. native and WASM use the same code and unsupported partitions retain native
   fallback;
6. no public image-processing API, unsafe Rust, dependency, target fork, or
   fixture-selected production branch is added;
7. strict native and WASM Clippy, formatting, whitespace, third-party legal,
   and package gates pass; and
8. Coverage MCP remains the only test runner and reports exact 100% line,
   branch, function, and region coverage with every manifest row active.

### Accepted result

The remaining level-3 `PARTITION_NONE` visibility slice is accepted:

- all twelve fixtures regenerate twice with the documented hashes;
- the eight positive cases expand the deterministic dav1d reconstruction
  oracle to thirty-four cases with SHA-256
  `41bd357e5c84e778d125ebc69f89fee5fc0f9d350187b3199e6f6e70d012fc98`;
- for each positive source, the 12x16, 16x12, 12x12, and 16x16 entropy
  operation arrays are exactly equal, including every CDF mutation;
- production admits only the two new level/dimension tuples and validates
  their exact 192-sample planes. Grid-width, coefficient, prediction,
  transform, visibility, and color implementations remain shared;
- `partitioned_4x12_a`, `partitioned_4x12_gray_127`,
  `partitioned_12x4_a`, and `partitioned_12x4_gray_127` remain successful
  Pillow/native manifest rows while the private portable reconstruction
  returns no leaf for their partition symbols 3, 2, 3, and 1 respectively;
- the manifest now has 1,068 active rows, zero planned decode rows, and zero
  unwired encode rows;
- no public image-processing API, unsafe Rust, dependency, target fork, or
  fixture-selected production path was added;
- strict native all-target/all-feature, AVIF-only WASM, and all-feature WASM
  Clippy gates pass; and
- Coverage MCP run `75a8b682-7302-45d2-95f4-896c078bd4be`, snapshot
  `b41fb5e9-51a9-4881-b257-ff528f59df09`, passes all seven test binaries with
  35,812/35,812 lines, 5,316/5,316 branches, 1,797/1,797 functions, and
  59,335/59,335 regions.

## Slice 14 Plan: One-Axis Rectangular Leaves

Status: accepted.

### Closed syntax boundary

Slice 14 admits the single-leaf rectangular class selected by a level-3
one-axis partition:

- `PARTITION_H` (symbol 1) produces one visible `BS_16x8` leaf with a 4x2
  row-major grid of 4x4 WHT transforms per plane;
- `PARTITION_V` (symbol 2) produces one visible `BS_8x16` leaf with a 2x4
  row-major grid of 4x4 WHT transforms per plane;
- every transform remains either coefficient-skip or the already accepted
  DC-only WHT residual; and
- the retained image dimensions may clip that private coded leaf to 12 or 16
  samples on its long axis and 4 or 8 samples on its short axis.

This yields exactly eight admitted dimensions:

```text
PARTITION_H: 12x4, 12x8, 16x4, 16x8
PARTITION_V: 4x12, 8x12, 4x16, 8x16
```

The fixed one-axis partition decisions are the first tile operations after
range initialization. The pinned dav1d traces use inverse threshold 9,351 for
`PARTITION_H` and 11,693 for `PARTITION_V`. The leaf then uses the existing
skip, luma-mode, chroma-mode, coefficient, predictor, and color path. The only
new block-size-indexed filter-intra thresholds are 23,374 for `BS_16x8` and
20,217 for `BS_8x16`.

`PARTITION_SPLIT` (symbol 3) is explicitly excluded. Its selected boundary
fixtures recurse to two visible level-4 8x8 leaves. The second leaf consumes
CDF state mutated by the first leaf, so accepting it requires a later complete
recursive traversal and multi-leaf reconstruction slice. Slice 14 must retain
native fallback for that class.

All geometry handling is private codec reconstruction. It must not expose
crop, resize, view, transform, color-adjustment, mutable-raster, or any other
image-processing API.

### Deterministic full traces

Each report was produced twice through Pillow 12.2.0, libavif 1.4.1, libaom
3.13.2, and a scalar build of dav1d commit
`b546257f770768b2c88258c533da38b91a06f737`. Every report pair is
byte-identical.

The speed-8 zero-residual corpus contains RGB gray 127, 128, and 129 for every
admitted dimension:

| Size | Partition | Report SHA-256 |
| --- | ---: | --- |
| 12x4 | 1 | `36c831fdf6ab9327c0c31ec791c87f1b13530e3ae1b3d740c7a26044df5de3aa` |
| 12x8 | 1 | `989ef82cb9a83610f9e93935ec977efb5c0a7c23e02fe64dd0c6894a60f771be` |
| 16x4 | 1 | `c0abe983a4e74c52f9210a8c2e6121ee7095e65a47cf14343567822375100b43` |
| 16x8 | 1 | `4908d0818075f252e2b8243dd42e831851d264db9d2f22952e1bfefe1552305b` |
| 4x12 | 2 | `fc77b1cbc5352a5170300a35b77157f9852591e0a221bb1d4ddb984073ffb085` |
| 8x12 | 2 | `8fe9bbe425cf633d332c978b75a1f9947bd02f75d553b32ff1409fe4d0154769` |
| 4x16 | 2 | `945cc042b2a7f1ada2d4b40f639671bd66c029bd240f3d9bd0121bb4f3d7f06f` |
| 8x16 | 2 | `74761cb67a5d291489edc932a9487b4a7bb963ebac353bddc43119c82473be91` |

The speed-0 residual corpus uses the representative 12x4 and 4x12
orientations with RGB `(17,91,203)`, gray 32, gray 127, gray 128, and gray
129. Its byte-identical report SHA-256 values are:

- 12x4:
  `65abf9f9e74d4d85e53c7e6e55d267526db3ce8c5ce7f6cf04a94d5f3013a475`;
- 4x12:
  `26cabd061f33fc737efc9046eccad0b6642b2754ceb636dc20d91cbd8c393b14`.

Those traces prove the complete accepted coefficient/context behavior:

1. each plane visits exactly eight transforms in row-major order;
2. `(17,91,203)` has a DC-only first transform on Y, U, and V followed by
   seven coefficient skips per plane;
3. gray 32 has a DC-only first luma transform, all-zero chroma, and every
   remaining transform skipped;
4. gray 127, 128, and 129 have all transforms skipped;
5. after a nonzero first DC, transform index 1 and transform index
   `grid_width` use the residual-neighbor skip context and every other skipped
   transform uses the base context; and
6. U and V continue to share and mutate the same chroma CDF state.

The speed-0 color probe has 130 scalar entropy operations in both
orientations, gray 32 has 82, gray 127 has 55, gray 128 has 56, and gray 129
has 57. The speed-8 gray cases have 55, 56, and 55 operations respectively.
The gray-129 count difference between speed 0 and speed 8 is encoded CDF
history, not a new decoded syntax class.

### Selected zero-residual fixtures

The existing `partitioned_12x4_gray_127.avif` and
`partitioned_4x12_gray_127.avif` become positive portable fixtures. Their
names record the boundary exploration that introduced them; production still
selects them only from parsed syntax.

| Size / gray | File SHA-256 | AV1 item SHA-256 | Y/U/V | Pillow RGB SHA-256 |
| --- | --- | --- | --- | --- |
| 12x4 / 127 | `8050ba2808d418cb39c1462a71d061e261582b2f4dffc255a5cfc65cf26e1fd2` | `e03d7fa2d93ec2722995234eb0c61ff2dc6a61ac7ffcc66e4dabc45b1a01e0f8` | `127/128/128` | `35fc07c937c1c3d13641f32cdc94ce1315ec420dd26e12b81a4651cfc1786ee3` |
| 12x4 / 128 | `4c8481a5800bcd2a54314d6fef24544240f601e818396b420495d5d84d623101` | `d5160f2de94761bb2559762148566ffec580bd510197b6c0aaed8607291ae875` | `128/128/128` | `7053108d4e37b600ae17d35890c69102ee6484d79a3a5cd622afca6f5606c543` |
| 12x4 / 129 | `be3773c9d0cc723661c3275d786b4c3e5928bdc5db7cba32c1bf9c063f7885d6` | `ac95cb8d8042e15b1555b08d2326f918c8b143cea0277e0aa63295073e346aac` | `129/128/128` | `c60b05f1911c0ccc80c5af2cd922c7cf1836279d44a17682c918cdaa5c7747e6` |
| 12x8 / 127 | `bb70428bb33c88bb106d407da50bd9358b2ea2db05f00164a3300abe80be9873` | `bdf81aedd19ae24ec37cdc6ee0878b99c0ee6468131ec8ca67f87f086f783172` | `127/128/128` | `cf8691a9b8c6c8e329b94f40345d822ef7d4f6e8e5c2343d74b12aa16e84838a` |
| 12x8 / 128 | `a3949ecc8e13ffdea49ae02e58ef42876db36062b5ee194dac7ec6b0eaba737f` | `104c7d8cca781b62f1f0ca63b0d5ccce4ad48873bd6cdd0038b193379fa90c77` | `128/128/128` | `88f2f6050a4ef8c9fd8bd69d3e51689155f6aa570f0ac0da6d3c0ee794bf3867` |
| 12x8 / 129 | `e329d7d9e35a6674dc8badffd19e2ef6812af3914c9c6701569b16a94a6efba0` | `ee4b3a39c2d85c002556cb4195e554efba0ff437743137b8d2eb334b9de6d63f` | `129/128/128` | `fe124f63ee1300955e9b2ffbed15cf383e9f4ae7c5cf60a09b074e4b0d73947f` |
| 16x4 / 127 | `55c0aaab5f00ea3d2c39703d4914c7a5bc1a59e73232c9771be551396fef2ebf` | `8ea807ec023f4747271e8b6168e04b478919a6985a3fcd7305f1d2a79bd78320` | `127/128/128` | `c24e73f000a4255a612416ecc4df81c9313e4c099877384712e4d8530dd7acbd` |
| 16x4 / 128 | `6918691db9f48f4c94cf42b75c61897f17b504043ccab34e25698bb9d971993e` | `815da0f91daedb96b02371c98e8ec2df857f9111a114a2a6353a0906a717f2b8` | `128/128/128` | `fa7b78cc215df21d7ce54d8c3c6637c326dab95c10fbc12263101365973f4268` |
| 16x4 / 129 | `c7af47b14d2b54c9a4eae925f77723627f26f79c6e1fdb5012c9e1599ed91f5c` | `ccfdcbff75c4ff904f8398f0d4c45f346d687d41f1879c207d6a68d8d220b6ea` | `129/128/128` | `fca06fef259b9ebb452449c7feda724ccec06a4a76b2b4fb1e6420a0beac435e` |
| 16x8 / 127 | `91125e2b485b356da5284cb4112d57dd5509628137c203406d48c47153a63e13` | `a6aea925fc850a43e0e2cc9d8a35dfb45a3717353db5e2672bfac1898c6b7f4d` | `127/128/128` | `7e18f1b2ca4e075b955848b4deafd56e47eeda83cc15b3ecdeb71d7ff58a5f57` |
| 16x8 / 128 | `691c0a4b301177f935791b4b5cc404f749096536d38128c7ab983e31dae0f65b` | `72b7a78e7e3b86082a5302cba415c00aff6be3c1733bb8df41f9ae5f326b3ce0` | `128/128/128` | `f83545d43c6939ec393b6b8310959b6174fd764b08a12fc22d908408a7e6a43e` |
| 16x8 / 129 | `97594ae3755cb1579095ab520fc986fb94d8de1782e67cd73db99ef48ffae9d7` | `5a7c2b26086695c7dda0d5f819326f97022e94bcdcf8b8fa3949077e2c96f8b1` | `129/128/128` | `7d965db8cbcf57e71b10b16973c9c2439222485594191da31460986a000f497c` |
| 4x12 / 127 | `a64e56b075bd724e0eeb25f29913010ff9fa3bd89fbfc3ba27eaa121fac0746f` | `ab441f7465cd795e78eac506ca4a3e5714ccac2fa7bf4b30e3c1139a3b7b646e` | `127/128/128` | `35fc07c937c1c3d13641f32cdc94ce1315ec420dd26e12b81a4651cfc1786ee3` |
| 4x12 / 128 | `eb1fcc1d6176aae147183e8d292209c24431f6d45b2a63b5cb8710f9979ec79a` | `2d0fa64aabbcd1072aba226d87d96ce8e4a8474b2efa8ee36aab5ce18186cf87` | `128/128/128` | `7053108d4e37b600ae17d35890c69102ee6484d79a3a5cd622afca6f5606c543` |
| 4x12 / 129 | `6d3be68bfdc174208f172e20bd309b352d9789b704249c34aba11eb96e3f5c31` | `5fc04ff84cee35ce704e6577cba95e7445bbee590f708316039906d497653225` | `129/128/128` | `c60b05f1911c0ccc80c5af2cd922c7cf1836279d44a17682c918cdaa5c7747e6` |
| 8x12 / 127 | `eeebe03575aa80c8be38b65671619d1a81f1be96dfcde917c47a7d602fe5bf51` | `bfd0d783cf2a861e8061840e0393bebf73373aebc836e72eddf196a61ac19c83` | `127/128/128` | `cf8691a9b8c6c8e329b94f40345d822ef7d4f6e8e5c2343d74b12aa16e84838a` |
| 8x12 / 128 | `ff4d86a80c112eb02fd81848ee5107b82d55f95817511189a15558517a1a047e` | `b3b797ecedd0969b32391bee95192c5850ae56801edf8912337b74e1c406850a` | `128/128/128` | `88f2f6050a4ef8c9fd8bd69d3e51689155f6aa570f0ac0da6d3c0ee794bf3867` |
| 8x12 / 129 | `4e181377befea58e23cd989fdeffaeffd8d45ca08fe9ebb5d7cfa262eddf5678` | `2fc39ced64bc3177c18250b73dbd922abf3d4ca0b05d26fe9e5c78fd6f76c7dd` | `129/128/128` | `fe124f63ee1300955e9b2ffbed15cf383e9f4ae7c5cf60a09b074e4b0d73947f` |
| 4x16 / 127 | `dcb0bd66d21c10ebc035dd4d598c24b6d92bf7cd19fc2b779678478820acf616` | `141b1f0542baaada147398e0ca93d6fd235319567724bee0b1edf9403399ac49` | `127/128/128` | `c24e73f000a4255a612416ecc4df81c9313e4c099877384712e4d8530dd7acbd` |
| 4x16 / 128 | `dd06b06c8cc9fa8416ce8debc24f7374c4f682f9aaf11b57cf9ce70af3e5a03a` | `2c463533574b8c34c37069fdd45a001346e31a93a4bec42dcd1662b696caa8ab` | `128/128/128` | `fa7b78cc215df21d7ce54d8c3c6637c326dab95c10fbc12263101365973f4268` |
| 4x16 / 129 | `6b0e87f423614d87d5c30200327cb1960d9f34f11d499a6f61fe0dfd15bbe5df` | `c0a0fb1be9bb6a77d809a13d128eb6bc0e5f671c6e2c8164dba8230e36e25585` | `129/128/128` | `fca06fef259b9ebb452449c7feda724ccec06a4a76b2b4fb1e6420a0beac435e` |
| 8x16 / 127 | `5596cf6f0e74c0dee066142ce6c1906a044c307d0cacdbba676e47d22d9d4487` | `4253c286c5b067a7ac58c0a49d12a2a62248c25d9035ee5779447428be9d0b2c` | `127/128/128` | `7e18f1b2ca4e075b955848b4deafd56e47eeda83cc15b3ecdeb71d7ff58a5f57` |
| 8x16 / 128 | `c2606f2088bdffafe58340d972bca033935e418144eb6b697ba06e27e968ed1a` | `b061084b2d675d261539898c0867c3e4543d339e0297b173018939eeba7ce96c` | `128/128/128` | `f83545d43c6939ec393b6b8310959b6174fd764b08a12fc22d908408a7e6a43e` |
| 8x16 / 129 | `907501fb1471801f5ef669630fcc9ec55932ecdd7df350624dcfd52058e31d66` | `f8cdc806528cf805b20ac0a76031d0e457d70eba3671b4602111fbdde780cc24` | `129/128/128` | `7d965db8cbcf57e71b10b16973c9c2439222485594191da31460986a000f497c` |

### Selected residual fixtures

Only the two representative orientations need additional speed-0 fixtures;
the decoded syntax and context sequence are otherwise geometry-equivalent.

| Planned fixture | RGB | File SHA-256 | AV1 item SHA-256 | Y/U/V | Pillow RGB SHA-256 |
| --- | --- | --- | --- | --- | --- |
| `portable_rect_12x4_a_speed0.avif` | `(17,91,203)` | `c1d9d3dd4532845f4fe5f337afaa768fcc54d57e1a69448950cce21a00175a06` | `e8ed60c896250778cdab44b3c30791b2a7893e20c043d62046be16324e99ab84` | `81/196/81` | `09fddd84398ad9a9d3ce8b981fea278a82e6b1fa62483fa0ef3c45cd484ae29e` |
| `portable_rect_12x4_gray_32_speed0.avif` | `(32,32,32)` | `49f4f000e4670bbe3069871e5114e6f26531a7157d50a6c98907ee19f7bb4f88` | `28074f43d71f0622f52f2dd097c72328692f83aa256c9c754a271ad4c9d7aee2` | `32/128/128` | `31178565d9d883446d9e273ee881220f43cb4c5de74e237f590f845e25659f38` |
| `portable_rect_4x12_a_speed0.avif` | `(17,91,203)` | `89a2e559311b51e2e334f60a8026470991651af2466d051eff77d1b992708e0d` | `0dff39b2e23a975da71aef44883417c2845db183914235c974f3d313d65c2f11` | `81/196/81` | `09fddd84398ad9a9d3ce8b981fea278a82e6b1fa62483fa0ef3c45cd484ae29e` |
| `portable_rect_4x12_gray_32_speed0.avif` | `(32,32,32)` | `c19f188da8ac6c2cdd160090038471363c949798fddb9586fb11a680ef8fb66a` | `b05968570c6d24f6845bded6999dbf8be7e8f434f9f3ff1b34e023ee31bda18c` | `32/128/128` | `31178565d9d883446d9e273ee881220f43cb4c5de74e237f590f845e25659f38` |

The speed-8 `(17,91,203)` fixtures
`partitioned_12x4_a.avif` and `partitioned_4x12_a.avif` remain negative
portable boundary fixtures because they select `PARTITION_SPLIT`.

### Oracle, implementation, and acceptance

Before production changes:

1. generate all twenty-eight positive fixtures twice and require the hashes
   above;
2. keep both recursive-split fixtures active as successful Pillow/native rows
   and private portable misses;
3. expand the complete dav1d reconstruction oracle from 34 to 62 cases;
4. compare every scalar entropy operation, full updated CDF, transform EOB,
   visible reconstructed Y/U/V sample, and Pillow RGB byte; and
5. regenerate both Pillow and dav1d references twice and require byte identity.

Production then follows only the reverse-mapped syntax:

1. carry validated transform-grid width and height through private block
   syntax, coefficient decoding, and reconstruction;
2. use `width * height` transform count and `grid_width` for the lower-neighbor
   context;
3. reconstruct a private 16x8 or 8x16 coded plane and retain only the
   AV1-declared visible rectangle;
4. dispatch a rectangular leaf only after parsing level 3 partition 1 or 2
   for one of the eight admitted dimension tuples; and
5. return a portable miss for partition 3 or any unproved syntax so the
   existing native fallback remains unchanged.

Slice 14 is accepted only when:

1. all twenty-eight positive fixtures match their file, AV1-item, plane, mode,
   dimension, and Pillow pixel references exactly;
2. all sixty-two reconstruction cases match the pinned dav1d entropy and
   reconstruction oracle while all thirty-four prior cases remain byte-exact;
3. both recursive-split fixtures remain private portable misses;
4. native and WASM execute the same private rectangular reconstruction;
5. no filename, fixture hash, pixel color, target, or final output selects a
   production branch;
6. no public image-processing API, unsafe Rust, dependency, target fork, or
   unlicensed third-party source is added;
7. strict native all-target/all-feature, AVIF-only WASM, and all-feature WASM
   Clippy, formatting, whitespace, legal, and source-package gates pass; and
8. Coverage MCP remains the sole test runner and reports exact 100% line,
   branch, function, and region coverage with every manifest row active.

### Accepted result

The one-axis rectangular-leaf slice is accepted:

- all twenty-eight positive files regenerated twice with the exact documented
  hashes, and the two speed-8 recursive-split files remain native/Pillow
  successes plus private portable misses;
- the complete 62-case reconstruction oracle regenerated twice with
  byte-identical SHA-256
  `0b7731aa26f62b726828d075d5de4e3eae58111e8706cc15ba1b2fd4f3d7668c`;
- Rust matches every pinned scalar partition, skip, mode, filter-intra,
  coefficient, CDF mutation, reconstructed plane, and Pillow RGB value;
- private block syntax now carries a closed four-variant coded-grid type for
  8x8, 16x16, 16x8, and 8x16. This removed unreachable raw-dimension
  fallbacks instead of manufacturing coverage-only invalid states;
- coefficient context uses the exact 4x2 or 2x4 transform count and lower
  neighbor stride, while reconstruction retains only the declared 12/16 by
  4/8 visible rectangle;
- the manifest now has 1,094 active rows, 816 active decode rows, 278 active
  encode rows, zero planned rows, and zero unwired rows;
- no public raster operation, image-processing API, unsafe Rust, dependency,
  target fork, or fixture-selected production path was added;
- strict native all-target/all-feature, AVIF-only WASM, and all-feature WASM
  Clippy, formatting, whitespace, the 19-file third-party legal audit, and the
  source-package inventory pass; the package contains 132 files, is 2.0 MiB
  unpacked, and is 412.7 KiB compressed; and
- Coverage MCP run `11edc033-05a0-4403-b284-95093a876148`, snapshot
  `856fee90-cd8a-41aa-89a8-7498aa891126`, passes all seven test binaries with
  35,857/35,857 lines, 5,322/5,322 branches, 1,801/1,801 functions, and
  59,385/59,385 regions.

## Slice 15 Plan: Closed Two-Leaf Recursive Split

Status: accepted.

### Closed syntax boundary

Slice 15 admits the smallest complete recursive class selected by a level-3
`PARTITION_SPLIT` (symbol 3) at a one-axis frame boundary:

```text
12x4:  level 3 (0,0) PARTITION_SPLIT
       level 4 (0,0) PARTITION_NONE
       level 4 (2,0) PARTITION_NONE

4x12:  level 3 (0,0) PARTITION_SPLIT
       level 4 (0,0) PARTITION_NONE
       level 4 (0,2) PARTITION_NONE
```

Each child is one coded 8x8 leaf with a 2x2 row-major grid of 4x4 WHT
transforms per plane. The second leaf is accepted only when all four
transforms in every plane are coefficient-skip. The first leaf remains the
already accepted DC-only-or-skipped class. Production must parse and validate
that syntax; it must not select a path from a filename, fixture hash, source
color, decoded coefficient value, or final pixel.

For the second child, a horizontal split admits DC or horizontal luma
prediction from the reconstructed left edge; a vertical split admits DC or
vertical luma prediction from the reconstructed top edge. The untraced
cross-direction combinations remain portable misses instead of being
reconstructed from speculative synthetic boundary samples.

The two leaves share the same adaptive AV1 tile state. In particular:

- the root fixed split probabilities are 9,351 for the horizontal boundary
  and 11,693 for the vertical boundary;
- both level-4 `PARTITION_NONE` symbols mutate the same partition CDF, from
  `[13_210,7_032,2_302,1]` after the first child to
  `[12_798,6_813,2_231,2]` after the second;
- both block skips mutate the same CDF, with inverse thresholds 1,097 then
  1,029;
- luma mode uses the AV1 keyframe spatial context selected by the available
  neighboring mode, not one global origin CDF;
- luma angle, chroma mode, filter-intra, coefficient, EOB, sign, and high-token
  CDFs remain shared across both children; and
- coefficient skip context for the second child is derived from the first
  child's decoded nonzero state. Luma and chroma retain separate initial
  contexts while U and V share and mutate the chroma state.

The exact inverse keyframe luma-mode CDFs required by this closed class are:

| Second-leaf location / first mode | Inverse CDF before decoding |
| --- | --- |
| right / vertical | `[20752,14702,13252,12465,12049,11324,10880,9736,8334,4110,2596,1359,0]` |
| below / vertical | `[22745,13183,11920,11328,10936,10008,9679,8745,7387,3754,2286,1332,0]` |
| right / horizontal | `[22716,21997,10472,9980,9713,9529,8635,7148,6608,3432,2839,1201,0]` |
| below / horizontal | `[20155,19177,11385,10764,10456,10191,9367,7713,7039,3230,2463,691,0]` |

The origin keeps the existing
`[17180,15741,13430,12550,12086,11658,10943,9524,8579,4603,3675,2302,0]`
CDF. These values are reverse-mapped from dav1d 1.5.3 `src/cdf.c` keyframe
mode tables and `src/decode.c` spatial mode selection.

### Prediction and composition

The first leaf is reconstructed before the second leaf is decoded. The
supported syntax makes every reconstructed child plane constant:

- a nonzero first DC transform establishes the constant;
- an all-skip leaf preserves its predictor; and
- every remaining transform is skipped.

The second horizontal child therefore takes its actual left edge from the
first child; the second vertical child takes its actual top edge. DC with only
that one neighbor and the matching horizontal or vertical directional mode
all reproduce the same edge sample for the selected corpus. The implementation
must carry the reconstructed edge and prior luma mode explicitly; it must not
reuse the old synthetic 127/128/129 origin samples for the second child.

The two private coded leaves are composed into a 16x8 or 8x16 plane and then
clipped once to the AV1-declared 12x4 or 4x12 rectangle. This is
codec-mandated reconstruction and visibility handling only. It adds no public
crop, resize, view, transform, color conversion, mutable-raster, or other
image-processing API.

### Deterministic trace corpus

The complete scalar dav1d reports were generated twice for both orientations
with RGB `(17,91,203)`, `(199,37,83)`, gray 32, red, green, and blue. Each
report pair is byte-identical:

- 12x4 report SHA-256:
  `24875e3b64bb8b6ba845af65a2b97d4ad66c400fe0bf13af03b7e81d55e22e3e`;
- 4x12 report SHA-256:
  `9fd07e4ed9bcf8970bef95885ab2ad8505380c3094b0dada0fd4f82d1ec16738`.

All twelve cases select exactly the three-node partition tree above. A wider
22-color sweep confirms that this topology is stable for the selected
deterministic encoder settings. The six fixtures retained for the independent
oracle cover the distinct decoded behavior:

| Fixture | File SHA-256 | AV1 item SHA-256 | Y/U/V | Pillow RGB SHA-256 |
| --- | --- | --- | --- | --- |
| `partitioned_12x4_a.avif` | `631fe6bf1c2f72cc60acdbb5e682d8195c512f67a5c8f38bd6ae10fdb7a8c59d` | `f81aaaab100d0d15cd7c4c45a0b79e5d9747b0c87b5fb34f88344d6c7abceacc` | `81/196/81` | `09fddd84398ad9a9d3ce8b981fea278a82e6b1fa62483fa0ef3c45cd484ae29e` |
| `partitioned_12x4_gray_32.avif` | `b2d4d2b81939374c9e11cc4a57b3ec0cc9212e2e1eb8d199c6c411cf41d5360b` | `b6f70f40139b377b61e3321ed0767ae43e203a0e8cad70e56384f296816850b1` | `32/128/128` | `31178565d9d883446d9e273ee881220f43cb4c5de74e237f590f845e25659f38` |
| `partitioned_12x4_green.avif` | `f0e1f0effed11bfde68129920f87590ee7af0962aca855272cbf1a1c586b9f62` | `738effb513052d629e8f17d0157d7873fcd83726cd09567eead7fdb9719ecb27` | `149/43/21` | `7f5e545c140df34ec243d4449ab8c4c0e476f532d3f6472ce956e7060b271e1c` |
| `partitioned_4x12_a.avif` | `0aa4e381b6412dd1ffa92a51e5bd4519ce038261485f59d1decd7ef5777690f8` | `84adc1dc5aad836f6b86f6a35c468034ed0aa9089c14fe279464471dcfa5b7b2` | `81/196/81` | `09fddd84398ad9a9d3ce8b981fea278a82e6b1fa62483fa0ef3c45cd484ae29e` |
| `partitioned_4x12_gray_32.avif` | `0a20cbc9fca468a631f0ccee83080296e8ce51813eb97430fc686ccc6700d218` | `0ad76ea55c7970933f4daf58917913dbc4da61a171ed4996c808d257f1f3a898` | `32/128/128` | `31178565d9d883446d9e273ee881220f43cb4c5de74e237f590f845e25659f38` |
| `partitioned_4x12_green.avif` | `ad3e35211dccfcefeff87fccb96319573a32d92f2d42c7564c39fbc88a9aee9c` | `4f8679f8a2166812450f10685ca517adcf3eac69fcd67c3627da70ecd2dc4914` | `149/43/21` | `7f5e545c140df34ec243d4449ab8c4c0e476f532d3f6472ce956e7060b271e1c` |

The `(17,91,203)` cases exercise a first vertical predictor followed by
second-leaf DC and nonzero Y/U/V DC residuals. Gray 32 exercises a nonzero
luma DC residual with zero chroma. Green exercises a first horizontal
predictor, horizontal edge prediction in the 12x4 orientation, and DC from
the top edge in the 4x12 orientation. Their scalar entropy operation counts
are respectively 137, 89, and 136/137.

### Oracle, implementation, and acceptance

Before production changes:

1. add the four new gray-32 and green fixtures through the deterministic
   Pillow/libavif asset generator and regenerate them twice;
2. expand the independent dav1d reconstruction oracle from 62 to 68 cases,
   retaining the three-node partition topology and complete scalar entropy
   operation sequence for every recursive fixture;
3. add all six recursive cases to the manifest-driven exact decode contract;
4. keep unsupported AVIF syntax as explicit portable misses so native fallback
   remains unchanged; and
5. require the regenerated Pillow, manifest, and dav1d documents to be
   byte-identical across two runs.

Production then follows only the reverse-mapped state machine:

1. parse the fixed level-3 split and both adaptive level-4 `PARTITION_NONE`
   symbols with one shared partition CDF;
2. retain one private block-CDF state across both children and select the
   second luma-mode CDF from its decoded spatial neighbor;
3. decode the first child with the existing closed 8x8 syntax, then decode an
   all-coefficient-skip second child with neighbor-derived coefficient
   contexts;
4. reconstruct the second child from the actual first-child edge and compose
   the two coded planes before visibility clipping; and
5. return a portable miss for any different tree, predictor, angle, chroma
   mode, filter-intra selection, transform coefficient syntax, or context.

Slice 15 is accepted only when:

1. all six recursive fixtures match file, AV1-item, topology, scalar entropy,
   reconstructed Y/U/V, dimensions, Pillow RGB, and manifest output exactly;
2. all 68 independent reconstruction cases pass while all 62 prior cases
   remain byte-exact;
3. native and WASM execute the same private recursive reconstruction;
4. no filename, fixture hash, color, target, or final output selects a
   production branch;
5. no public image-processing API, unsafe Rust, dependency, target fork, or
   unlicensed third-party source is added;
6. strict native all-target/all-feature, AVIF-only WASM, and all-feature WASM
   Clippy, formatting, whitespace, legal, and source-package gates pass; and
7. Coverage MCP remains the sole test runner and reports exact 100% line,
   branch, function, and region coverage with every manifest row active.

### Accepted result

The closed two-leaf recursive split is accepted:

- the four new gray-32 and green fixtures regenerate deterministically with
  the exact documented hashes, while the two existing A fixtures now decode
  through the portable path;
- the independent 68-case reconstruction oracle regenerates twice with
  byte-identical SHA-256
  `e4124fdfe8031ecd29934c2d9990c3a3fa053993b3c34b7f14ebb54ed82a069f`;
- Rust matches every pinned three-node partition topology, scalar entropy
  operation, full adaptive CDF mutation, reconstructed Y/U/V sample, and
  Pillow RGB byte;
- the implementation retains one private block-CDF state across both children,
  selects the second luma CDF from the decoded spatial neighbor, requires an
  all-skip second leaf, and predicts from the actual reconstructed left or top
  edge;
- unproved cross-direction second predictors, different child partitions, and
  other syntax remain portable misses. A parsed `superres_enabled` flag is now
  rejected once in the common closed-reconstruction gate instead of repeating
  dead geometry checks;
- the manifest now has 1,098 active rows, 820 active decode rows, 278 active
  encode rows, zero planned rows, and zero unwired rows;
- no public raster operation, image-processing API, unsafe Rust, dependency,
  target fork, or fixture-selected production path was added;
- strict native all-target/all-feature, AVIF-only WASM, and all-feature WASM
  Clippy, formatting, whitespace, the 19-file third-party legal audit, and the
  source-package inventory pass. The package contains 132 files, is 2.0 MiB
  unpacked, and is 414.9 KiB compressed; and
- Coverage MCP run `6fa7eb57-1654-4ce9-a1f1-ec5dbfe0b0ee`, snapshot
  `1953f4c0-b1d7-443f-a7b8-47c17877f89e`, passes all seven test binaries with
  36,064/36,064 lines, 5,324/5,324 branches, 1,820/1,820 functions, and
  59,641/59,641 regions.

## Slice 16 Plan: Full-Width Two-Leaf Visibility

Status: accepted.

### Closed syntax boundary

Slice 16 extends the accepted two-leaf recursive class from partial 12x4 and
4x12 visibility to the complete coded 16x4 and 4x16 rectangles. It adds no AV1
syntax:

```text
16x4:  level 3 (0,0) PARTITION_SPLIT
       level 4 (0,0) PARTITION_NONE
       level 4 (2,0) PARTITION_NONE

4x16:  level 3 (0,0) PARTITION_SPLIT
       level 4 (0,0) PARTITION_NONE
       level 4 (0,2) PARTITION_NONE
```

For the same RGB source, every scalar entropy operation and partition header
is byte-for-byte equal to the corresponding 12x4 or 4x12 Slice 15 trace.
Transform grids, adaptive state, spatial luma contexts, coefficient syntax,
reconstructed edge prediction, and two-leaf composition are therefore shared.
Only the final private visibility step retains all 16 coded samples on the
long axis rather than the first 12.

This remains codec-mandated reconstruction and declared-frame visibility. It
does not add a public crop, resize, view, raster transform, mutable image, or
other image-processing API.

### Deterministic trace corpus

Both three-case reports were generated twice through Pillow 12.2.0, libavif
1.4.1, libaom 3.13.2, and scalar dav1d commit
`b546257f770768b2c88258c533da38b91a06f737`. Each pair is byte-identical:

- 16x4 report SHA-256:
  `26f2b98f911e843ce8360b362f98b2f197a6e442135eb5ed1ce8fa0d7b17b04c`;
- 4x16 report SHA-256:
  `64e27d87729a7c3a2c6ea46f7feebe6ccce7641d19d60c6a3351c613e656c834`.

| Planned fixture | File SHA-256 | AV1 item SHA-256 | Y/U/V | Pillow RGB SHA-256 |
| --- | --- | --- | --- | --- |
| `partitioned_16x4_a.avif` | `1d6bd1273fb2890b27dd1999e0766220059d99cd3973da859a0d13070b068dc4` | `5b1e325f3f3e21d4e7f370512da7b2ecaa047255d7a534d03ffc75f8f5659348` | `81/196/81` | `1f403e7f414473b888fcba438d60d269e54fc1d04c802dd32f96fa657932b2ac` |
| `partitioned_16x4_gray_32.avif` | `5603b8d4ff018f442ca8c2008a5f4d75b1877094f65adc121d8a634059ba359e` | `077fec8540ee2759021c4a0351786d42bbe2b455b855c630db734da75df731b4` | `32/128/128` | `1d3659ada1bf4b80ae974a7b544090591793cb954ac3f9ad13d3af3f09c21967` |
| `partitioned_16x4_green.avif` | `065ca6683c44348263bb922862a2dfd459650b2929464acc3e71a56b09cafa85` | `2abafb2adcd5a2c108b59376985118ef921ea4f1542a1660fa5cdd24a8d9f5ed` | `149/43/21` | `32e7c45e59200de4c1012eac0ef31f3fa35d02b40d563f4602644bca9266f7fc` |
| `partitioned_4x16_a.avif` | `d2c62b8197dd5080a461202ff18b4c218e47f436eef05cc9856e6e7c1fec0245` | `49a8a4878c7c1ff3dfb477d35d62fe6edc837e8cdd6ec3a72a1c64682b1628cd` | `81/196/81` | `1f403e7f414473b888fcba438d60d269e54fc1d04c802dd32f96fa657932b2ac` |
| `partitioned_4x16_gray_32.avif` | `a5e8c50931fdb5cf94f7d2d1a4edf114f23ea9fc83f782fb24a30f793035b7a0` | `e406b2d96188d6e1daab71bd6ccb7ae8ca4f39f858471e2b8afaab7482b97323` | `32/128/128` | `1d3659ada1bf4b80ae974a7b544090591793cb954ac3f9ad13d3af3f09c21967` |
| `partitioned_4x16_green.avif` | `0e6cbfb32ea49625e8e43b221cc57f47cacbd9594a79786185b78ea4336253ed` | `74401112c7ba1907b5d1a9e8f953d39b38f7f22ce1d53a9d36954fd891c04de5` | `149/43/21` | `32e7c45e59200de4c1012eac0ef31f3fa35d02b40d563f4602644bca9266f7fc` |

The A, gray-32, and green cases retain the Slice 15 branch coverage:
nonzero YUV DC, luma-only DC, first vertical/horizontal modes, second
DC/horizontal modes, shared CDF mutation, and actual left/top edge prediction.
Their operation counts remain 137, 89, and 136/137.

### Oracle, implementation, and acceptance

Before production changes:

1. add all six files through the deterministic Pillow/libavif asset generator
   and require the hashes above;
2. expand the independent reconstruction oracle from 68 to 74 cases and
   require its two-run output to be byte-identical;
3. add the six exact decode rows to the manifest; and
4. prove each new entropy operation array equals its Slice 15 orientation and
   source-color counterpart.

Production then adds only `(16,4)` and `(4,16)` to the closed recursive
dimension gate. The shared decoder already composes a private 16x8 or 8x16
coded plane and clips it to the declared width and height.

Slice 16 is accepted only when:

1. all six files match their file, AV1-item, topology, entropy, reconstructed
   plane, dimension, and Pillow RGB references exactly;
2. all 74 independent reconstruction cases pass while all 68 prior cases
   remain byte-exact;
3. native and WASM execute the same private reconstruction;
4. unsupported syntax remains a portable miss;
5. no public image-processing API, unsafe Rust, dependency, target fork,
   fixture-selected production branch, or unlicensed source is added;
6. strict native and WASM Clippy, formatting, whitespace, legal, and package
   gates pass; and
7. Coverage MCP remains the sole test runner and reports exact 100% line,
   branch, function, and region coverage with every manifest row active.

### Accepted result

The full-width two-leaf visibility extension is accepted:

- all six generated 16x4 and 4x16 fixtures match the documented file and AV1
  item hashes, and their partition headers and scalar entropy operation arrays
  are byte-for-byte equal to the corresponding 12x4 and 4x12 source-color
  cases;
- the independent 74-case reconstruction oracle has SHA-256
  `07113842d040cd7a934781b62230ac1cbaa96fd6649dcbb4ab26f1f03804667e`
  and Rust matches its reconstructed Y/U/V planes and Pillow RGB bytes;
- the production change is limited to admitting `(16,4)` and `(4,16)` through
  the already accepted private recursive dimension gate. Shared entropy state,
  second-leaf restrictions, reconstructed-edge prediction, composition, and
  declared-frame clipping are unchanged;
- the manifest now has 1,104 active rows, 826 active decode rows, 278 active
  encode rows, zero planned rows, and zero unwired rows;
- no public raster operation, image-processing API, unsafe Rust, dependency,
  target fork, or fixture-selected production path was added;
- strict native all-target/all-feature, AVIF-only WASM, and all-feature WASM
  Clippy, formatting, whitespace, the 19-file third-party legal audit, and the
  source-package inventory pass. The package contains 132 files, is 2.0 MiB
  unpacked, and is 415.0 KiB compressed; and
- Coverage MCP run `b6a1713b-b08b-47d7-b9d9-64e5ae8fa233`, snapshot
  `3c157d2b-a60c-4f70-92ba-89f371490239`, passes all seven test binaries with
  36,064/36,064 lines, 5,324/5,324 branches, 1,820/1,820 functions, and
  59,641/59,641 regions.

## Slice 17 Plan: Eight-Pixel Two-Leaf Visibility

Status: accepted.

### Closed syntax boundary

Slice 17 extends the accepted two-leaf recursive class to 12x8, 16x8, 8x12,
and 8x16 declared frames. The partition tree remains exactly the three-node
tree accepted in Slices 15 and 16:

```text
12x8 / 16x8: level 3 (0,0) PARTITION_SPLIT
              level 4 (0,0) PARTITION_NONE
              level 4 (2,0) PARTITION_NONE

8x12 / 8x16: level 3 (0,0) PARTITION_SPLIT
              level 4 (0,0) PARTITION_NONE
              level 4 (0,2) PARTITION_NONE
```

This geometry adds no AV1 syntax or reconstruction algorithm. For a fixed
source color, every partition header, scalar entropy operation, adaptive CDF
state, prediction mode, coefficient decision, transform, and reconstructed
edge is byte-for-byte equal to the accepted 16x4 or 4x16 trace. The existing
private composition already reconstructs a complete 16x8 or 8x16 coded plane;
this slice admits declared frames that retain all eight samples on the short
axis.

The existing partition-symbol-1 and partition-symbol-2 rectangular leaves at
these dimensions remain independently accepted. Production must continue to
select the rectangular or recursive path from decoded partition syntax, never
from dimensions, filenames, fixture hashes, colors, or target.

Declared-frame visibility is part of codec reconstruction. This slice does
not add a public crop, resize, view, raster transform, mutable image, or other
image-processing API.

### Deterministic reverse mapping

A fixed 22-color corpus was encoded through Pillow 12.2.0, libavif 1.4.1, and
libaom 3.13.2 at quality 100, speed 8, one thread, full-range YUV 4:4:4, and
disabled autotiling. At every planned dimension, colors 127, 128, and 129
select the already accepted single rectangular leaf while all other nineteen
colors select `PARTITION_SPLIT`. This proves the recursive fixtures exercise a
bitstream decision rather than a geometry shortcut.

The A `(17,91,203)`, gray-32, and green reports were generated twice with
scalar dav1d commit `b546257f770768b2c88258c533da38b91a06f737`. Each pair is
byte-identical:

- 12x8 report SHA-256:
  `8b626b783f4bcabadf583b5e5d7357f26ff82bc20921ae3b0105406370ff7ca4`;
- 16x8 report SHA-256:
  `a678bcb9ddc52c9f7ce57c8cc7274725717419642c89f2b948de947cc67d5be2`;
- 8x12 report SHA-256:
  `c35476b33981f9084903a5ea4b2d2f07ce325817e3e8425cfc66f7f391107b2b`;
- 8x16 report SHA-256:
  `703329562d2bbe0e0fe485d3cac86b4954b27dc2f451691dc2e66c619bbc6389`.

| Planned fixture | File SHA-256 | AV1 item SHA-256 | Y/U/V | Pillow RGB SHA-256 |
| --- | --- | --- | --- | --- |
| `partitioned_12x8_a.avif` | `3c84461626938d85b0b51798f18666e9476c881d5a0f1289b583cd6ed691a7bf` | `3ed812fc15bd94b35660835d7889e4fc6c5560a274487fc5bd328c5a3c06b3bb` | `81/196/81` | `47c4a5d65d8ac82aa68f04754b38e5bf00438aeb64b2e48c2bb54a9268e6e4e7` |
| `partitioned_12x8_gray_32.avif` | `1ec10cc4930d5934d77ffda0fceef577daae0c3e003f08ad315b8ada4a9d6f26` | `202156c28a5e06dc075cc51e8eb68c4b97795ca288b73169cf488faf119e29ba` | `32/128/128` | `a80ec409692fd6c32b82fa895a118a06751d63671cd6da6ed14ef5bb59f41541` |
| `partitioned_12x8_green.avif` | `a1ade0b1491bfbb45a220dabe3dd7199df0d7c43d0bac44b538fa88e78e74b3b` | `9d73cb15464e8599f0618ad727ae745b55dbc9f02a0bbe1f891f81bd6a4a8945` | `149/43/21` | `c1046797ae8db85c1b32d232085bdc2251d6e94567771f20ce9f86b6a2cc5cbc` |
| `partitioned_16x8_a.avif` | `ed6197a68ca5ce65a447217beddfd5c4ef89e3e370579727f75b364263c5cd56` | `3e1a27cd9c6f14a8f483c2ee9b7b113fd7cddafabb8c6287df467a3853ed7985` | `81/196/81` | `983aef668db1ea0d5801725fdf2b49d32232fc7f1d9ae578a03ffad6aebc4fc2` |
| `partitioned_16x8_gray_32.avif` | `b46eac4615030f89bf2e9027d3763e7059cf9c80788c4a3e0fd32cbdef85cb8a` | `68150fd365fd5707dd449c08e8e2dd68f066a4ee0fe88edde2fbfa7fe618a62c` | `32/128/128` | `f89d41f00d89e8b0bf8cb8cff89f9f23e9fa1e5113473dda8d16098575db7388` |
| `partitioned_16x8_green.avif` | `d39d76dad2e9a3c939db591ade9c6e655a06d8e6ada0ddc42c49a13089244840` | `427c568b515d25bcd8f07b899b8739f9976e39ac053dc72fbf0eedf11283d85e` | `149/43/21` | `ff87dfd10bc6c01f8e9dac23bb518192e6579a383b2ff1bbd8b8c80a58e677b4` |
| `partitioned_8x12_a.avif` | `498c54f6c31f6b2ff0620da37b1e3fdd56760c7951fd1281c7c2baf21ade0bb8` | `2c519a7573fae444512d2e9a8448de2d33e69f10f87c36be54b6c77ac1b15215` | `81/196/81` | `47c4a5d65d8ac82aa68f04754b38e5bf00438aeb64b2e48c2bb54a9268e6e4e7` |
| `partitioned_8x12_gray_32.avif` | `d368148298780e652a20fbbf3a77d671bc72cf67d38f88b91b4a20ff4bcbdc9a` | `f4d945a0eb2bd52cfed54f12d32698c33ba4d977453dcc40b26d5491ef010e25` | `32/128/128` | `a80ec409692fd6c32b82fa895a118a06751d63671cd6da6ed14ef5bb59f41541` |
| `partitioned_8x12_green.avif` | `b11a0b802bddcb8654978d53395afef62afce9adc571a94f87b820c4f128da7d` | `75e07aac46538f4e463d59d8bd1eabbd203aab718fe713d51bbc32f3d6438b5c` | `149/43/21` | `c1046797ae8db85c1b32d232085bdc2251d6e94567771f20ce9f86b6a2cc5cbc` |
| `partitioned_8x16_a.avif` | `003d1203ec343319da24875b1c66d3be4626804300d36cff089f1efeb9145dae` | `6613db79bafc8594deb9aca84181083b6e8f52d638b236c865086bf0a27091ba` | `81/196/81` | `983aef668db1ea0d5801725fdf2b49d32232fc7f1d9ae578a03ffad6aebc4fc2` |
| `partitioned_8x16_gray_32.avif` | `967f4e6c4d216cea64514c7de72ccb5b81b800ca62f928cb4e043fe06736982f` | `61f0e7648cb80ca5d4115f40b84ac7f3fbfe2851aaeb306c5de0bc6376a9460f` | `32/128/128` | `f89d41f00d89e8b0bf8cb8cff89f9f23e9fa1e5113473dda8d16098575db7388` |
| `partitioned_8x16_green.avif` | `fe5069db1c23f0335fcc9855de689932e3b1d195e16250f97ea6c07004394954` | `7eb236c81d5dd39fd08da6f0ee639f5eef596a79345c2a167f3e9fa41750c5e1` | `149/43/21` | `ff87dfd10bc6c01f8e9dac23bb518192e6579a383b2ff1bbd8b8c80a58e677b4` |

### Oracle, implementation, and acceptance

Before production changes:

1. add all twelve files through the deterministic Pillow/libavif asset
   generator and require the hashes above;
2. expand the independent reconstruction oracle from 74 to 86 cases and
   require its regenerated output to be byte-identical across two runs;
3. add twelve exact decode rows to the manifest; and
4. prove every new partition header and entropy operation array equals the
   accepted orientation and source-color counterpart.

Production then adds only `(12,8)`, `(16,8)`, `(8,12)`, and `(8,16)` to the
closed recursive dimension gate. The single-leaf and recursive paths remain
selected by the decoded partition bit.

Slice 17 is accepted only when:

1. all twelve files match their file, AV1-item, topology, entropy,
   reconstructed plane, dimension, and Pillow RGB references exactly;
2. all 86 independent reconstruction cases pass while all 74 prior cases
   remain byte-exact;
3. native and WASM execute the same private reconstruction;
4. rectangular leaves at the same dimensions remain exact and unsupported
   syntax remains a portable miss;
5. no public image-processing API, unsafe Rust, dependency, target fork,
   fixture-selected production branch, or unlicensed source is added;
6. strict native and WASM Clippy, formatting, rustdoc, whitespace, legal, and
   package gates pass; and
7. Coverage MCP remains the sole test runner and reports exact 100% line,
   branch, function, and region coverage with every manifest row active.

### Accepted result

The eight-pixel two-leaf visibility extension is accepted:

- all twelve generated fixtures match the documented file and AV1-item hashes;
- the independent 86-case reconstruction oracle regenerates twice with
  byte-identical SHA-256
  `fc8c9ff1a2877b6e6bd680e23525ebd23e698190a8c1db4355d9048aa239d32f`;
- every new partition header, scalar entropy operation, adaptive CDF state,
  reconstructed Y/U/V sample, and Pillow RGB byte matches its accepted
  orientation and source-color counterpart exactly;
- the existing partition-symbol-1 and partition-symbol-2 rectangular fixtures
  at the same dimensions remain exact, proving decoded partition syntax
  selects the path;
- production adds only `(12,8)`, `(16,8)`, `(8,12)`, and `(8,16)` to the
  closed recursive dimension gate. Shared entropy state, child restrictions,
  reconstructed-edge prediction, composition, and visibility remain
  unchanged;
- the manifest now has 1,116 active rows, 838 active decode rows, 278 active
  encode rows, zero planned rows, and zero unwired rows;
- no public raster operation, image-processing API, unsafe Rust, dependency,
  target fork, or fixture-selected production path was added;
- strict native all-target/all-feature, AVIF-only WASM, and all-feature WASM
  Clippy, strict rustdoc, formatting, whitespace, and the 19-file third-party
  legal audit pass; and
- Coverage MCP run `1d6d8a08-9718-4c41-80cf-a8948457ba98`, snapshot
  `50a0d22f-8b21-4167-8722-f6605e30201b`, passes all seven test binaries with
  36,064/36,064 lines, 5,324/5,324 branches, 1,820/1,820 functions, and
  59,641/59,641 regions.
