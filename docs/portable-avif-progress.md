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
  legal audit pass. The source package contains 132 files, is 2.0 MiB
  unpacked, and is 415.0 KiB compressed; and
- Coverage MCP run `1d6d8a08-9718-4c41-80cf-a8948457ba98`, snapshot
  `50a0d22f-8b21-4167-8722-f6605e30201b`, passes all seven test binaries with
  36,064/36,064 lines, 5,324/5,324 branches, 1,820/1,820 functions, and
  59,641/59,641 regions.

## Slice 18 Exploration Plan: Square Multi-Leaf Partitioning

Status: accepted.

### Current mismatch

The portable decoder accepts one level-3 `PARTITION_NONE` 16x16 coded leaf and
the one-axis level-3 split into two 8x8 children. It does not reconstruct a
level-3 partition tree when both the horizontal and vertical split conditions
are true. The smallest missing square topology is therefore a 12x12 or 16x16
declared frame whose root selects `PARTITION_SPLIT` and whose four level-4
children select `PARTITION_NONE`.

A deterministic 22-color constant corpus at both dimensions selected
`PARTITION_NONE` in every case. Constant images cannot prove square recursion,
so enabling a square split from dimensions alone would be a fixture-selected
shortcut rather than an AV1 implementation.

### Reverse-mapping corpus

Before production changes, add a diagnostic pattern corpus generated from
already accepted source colors and Pillow's pinned AVIF encoder settings:

1. left/right halves to isolate vertical partition selection;
2. top/bottom halves to isolate horizontal partition selection;
3. four constant quadrants to target a four-child square split;
4. 8x8 and 4x4 checkerboards to expose deeper or asymmetric partition trees;
5. one changed quadrant and one changed sample to find the minimum boundary
   that changes the root partition.

Generate every input twice and reject nondeterministic bytes. Trace the full
partition tree, scalar entropy operations, prediction state, coefficient
state, reconstructed Y/U/V planes, and Pillow RGB bytes through scalar dav1d
commit `b546257f770768b2c88258c533da38b91a06f737`.

The pinned Pillow/libaom bridge accepts the development-only advanced options
`min-partition-size=8` and `max-partition-size=8` (hyphenated spellings; the
underscore spellings are rejected). Add one constant-color candidate with
both constraints. This is a reverse-mapping fixture-generation control only:
it may isolate a four-child traversal whose leaf syntax is already proved, but
it must not become a public encoder option or a dimension/case special branch
in the portable decoder.

Result: libaom accepts the two controls but its lossless still-image decision
remains one level-3 `PARTITION_NONE` leaf. At 12x12 and 16x16 the outputs are
byte-identical to the accepted constant-A fixtures
(`daccd674b98dc26baad851ef95d75e6099c0397db5d8e28fb7f5f1f6eef9ac6c`
and
`04de5e1b6e056c08fdb33131d04dec6708b1ee4912c515fda6d973a29e592381`).
The options bound permitted partitions; they do not force a split, so this
candidate is rejected.

The natural-pattern sweep selects `changed_bottom_right` as the smallest
four-child trace so far: 431 scalar entropy operations at 16x16. Its first
three children reuse the accepted constant-A reconstruction, but the changed
bottom-right child introduces multiple nonzero transform residuals. Before
approving that syntax, sweep deterministic replacement colors around A one
channel at a time and retain only candidates that select the same five-node
partition tree. Rank them by entropy-operation count, then rerun the smallest
closed candidate with the complete scalar trace.

The one-channel sweep found `(17,96,203)` at the first split boundary, reducing
the full trace to 272 operations. It is not a closed extension of the accepted
coefficient class: its bottom-right transforms decode high-token values 12, 4,
and 8, whereas the retained implementation proves only token 15 followed by
its Golomb extension. Reject operation count as the sole ranking metric.
Re-rank split candidates by exact syntax reuse and accept only a candidate
whose nonzero transforms stay in the already-proven token-15 DC path.

### Selected closed class

The deterministic syntax sweep report
`c694729770205db94ab8cdfcf0db5d57688a4571a240877e5121bb85fc7b6819`
selects three 16x16 bottom-right-quadrant cases:

| Replacement RGB | File SHA-256 | AV1 item SHA-256 | Y/U/V plane SHA-256 | Pillow RGB SHA-256 | Entropy operations |
| --- | --- | --- | --- | --- | ---: |
| `(17,64,203)` | `4a8703a56c56a2d6cbcdbec90e12d266fc28603db1f84e725f7f1a75f504fed7` | `2a970e96bba9c9e4890b80d3bc19f798d3282ac80a5f190528816c69711b3916` | `862462637f7b4afe86e2be91b212c34bd8db25383d8837333e035923e73e4fbe` / `941f432301170594563b3f1671f07f6c7e1b1053f8266080ea619795af0ec6ee` / `fa0bfcec1e33bfe2f27bff2dc47dd4a23aeb9b59ed45428ac2a1c65b2f7bfe09` | `d7efc58f710522b0c6e2609ab53339cf9aa4c3c419b4023593bffd94fcb883fe` | 363 |
| `(64,91,203)` | `fe7610630b212d87a5b9b9650fa156be9729e1bd49d8c01df5df416e5e524898` | `b6cc6afcd313c1117b1ca08fa827047b187387821f33bfe2bf9056c4afbad215` | `25660b9d240cebe64635b95718d50d50d80971172345d925cc052e038254675d` / `1027085f57119f35d66a03d1215f6d350d2bb591c9e38d7ba944106ed9ab8695` / `fab0aa558c5445a153519685611a63ef8bbf69a3607d7eebefeaf727504e0dad` | `6492bb904bafc0a5c8acedff1fd7cd70965e3be844e8fd19d0e04a6bd63e2017` | 369 |
| `(17,127,203)` | `4085fdb230e1bcc93a3a3be408d5fbbf0a5c740590df3983c07b191d3b59ba08` | `f974f3c11523d00b6064fab24b0de724c964948c5979d82ebb22e0914449c0df` | `00d80b1fdf83911abbe4f71cbe488c6499186f8269924a28831df0a4d4bf94d9` / `63c0ecc9d42fbd0e65dfa75c28ec022eaf922a88bfea6dfb3579dd5cb83b2e96` / `862462637f7b4afe86e2be91b212c34bd8db25383d8837333e035923e73e4fbe` | `d1ce3617b6228d74d2b208847c20486f1a6301cf8b0708242c0019894eeb055e` | 379 |

The complete trace reports were each generated twice and were byte-identical:
`70d1b6e2fa54b0191fe444f2297dfcbf5ee877f7745ebc294d0bbe1d7dbb1ce6`
for the first case and
`3e5b66e8c419d3ce81995edc6ac9a6dadcfbb8bfbe15a981b0ce44e653f94cf1`
for the two sign variants. All three have exactly the same partition topology:

1. level-3 root `PARTITION_SPLIT`;
2. level-4 top-left `PARTITION_NONE`;
3. level-4 top-right `PARTITION_NONE`;
4. level-4 bottom-left `PARTITION_NONE`;
5. level-4 bottom-right `PARTITION_NONE`.

The top-left leaf reuses the accepted vertical-predictor DC-only path. The
top-right and bottom-left leaves select DC prediction and skip all four
transforms on all planes. The bottom-right leaf selects DC prediction; each
plane has EOB sequence `0,0,0,-1`, every nonzero transform selects high token
15, and the fourth transform is skipped. The three colors cover positive and
negative DC residuals and both derived sign contexts.

The half-boundary 12x12 input is explicitly rejected from this slice. Its
six-pixel source boundary cuts through the coded 8x8 children, causing non-DC
EOB values and high-token values 9, 10, and 12 in the first three leaves. It is
not the same syntax class.

### Slice 18 implementation plan

Status: accepted.

1. Admit only a 16x16, eight-bit, full-range, lossless 4:4:4 root split with
   four level-4 `PARTITION_NONE` children. Keep 12x12 and every other square
   recursive topology unsupported.
2. Interleave all four child partition symbols with their leaf syntax while
   sharing the same child-partition CDF and block CDF state.
3. Retain the proved top-left `DcOrSkipped` policy and the all-skipped
   top-right/bottom-left policies. Add one bottom-right policy requiring
   token-15 DC-only coefficients for transforms zero through two and a skipped
   transform three on every plane.
4. Model the exact coefficient-skip contexts used by the trace: luma contexts
   1, 3, and 6 and chroma contexts 10, 11, and 12. Retain independent,
   adaptively updated DC-sign CDFs for contexts zero through two.
5. Retain the frame's `allow_screen_content_tools` and sequence
   `enable_filter_intra` flags in the block context. For each DC child, decode
   the 8x8 luma-palette, chroma-palette, and filter-intra decisions through
   their own shared CDFs when the corresponding tools are enabled, and require
   all three decisions to be false. Chroma mode is independently DC for the
   accepted top-left vertical leaf, so its chroma-palette decision is also
   consumed even though no luma-palette or filter-intra decision exists there.
6. Reconstruct the bottom-right leaf one 4x4 transform at a time. Compute each
   DC predictor from its real top and left four-sample edges, then apply the
   existing lossless inverse WHT. Do not expose this codec reconstruction as a
   reusable image-processing operation.
7. Compose the four coded 8x8 leaves into one 16x16 plane and require exact
   Y/U/V, Pillow RGB, entropy-operation, topology, and encoded-file hashes for
   all three fixtures.
8. Add the three inputs and exact Pillow references through the manifest
   generator, extend the independent dav1d reconstruction oracle, and keep
   structured unsupported behavior for every syntax outside the closed class.
9. Run rustfmt, strict native and WASM Clippy, strict rustdoc, legal/package
   verification, and Coverage MCP with exact line, branch, function, and
   region coverage.

### Selection and debugging rules

Prefer the smallest topology whose leaves use already proved prediction and
coefficient syntax. For each candidate:

1. compare the root and child partition sequence with dav1d;
2. compare adaptive state at each leaf boundary;
3. find the first prediction-mode or coefficient operation that differs from
   an accepted single- or two-leaf trace;
4. reverse-map a smaller input if the candidate introduces more than one new
   syntax class; and
5. document exact hashes and per-stage evidence before proposing production
   code.

The next implementation slice will be defined only after this sweep identifies
one closed class. Unsupported partitions, predictors, coefficients, filters,
and transforms must remain portable misses. Square composition remains private
codec reconstruction and must not expose a public image-processing operation.

### Slice 18 coverage-closure plan

The first exact-parity Coverage MCP run leaves six lines, one branch, and
twenty-three regions uncovered. Every gap is an unsupported-syntax exit around
the newly accepted four-leaf path, not an untested successful reconstruction:

| Gap | Reverse-mapped reason | Fixture-driven closure |
| --- | --- | --- |
| First level-4 child partition is not `PARTITION_NONE` | All three accepted square fixtures correctly select zero, so only the rejection side is missing. | Feed the accepted `partitioned_square_16x16_g64.avif` fixture through the existing deterministic AV1 sample mutation sweep. Mutate every byte in its coded span through every replacement byte and retain production validation as the observation point. |
| Syntax decode fails in the top-left, top-right, bottom-left, or bottom-right leaf | The manifest fixtures prove every success stage. Their coded span is the nearest valid prefix for reverse-mapping each staged failure. | Use the same square-fixture mutation sweep so corruptions cross each leaf boundary while preserving the real container, sequence, frame, and partition prefix whenever possible. |
| Bottom-right spatial context receives any predictor pair other than DC/DC | The closed class deliberately accepts only DC/DC, so no valid fixture should manufacture this private invalid state. | Extend the existing coverage-only private-branch hook with one direct non-DC neighbor-context probe. Keep the production predicate counted and the hook itself excluded. |

Acceptance remains exact 100% line, branch, function, and region coverage from
Coverage MCP. If the mutation sweep does not reach one of the staged exits,
inspect the first surviving scalar entropy prefix and reverse-map the smallest
deterministic mutation for that exact stage before changing production code.

The first mutation sweep closes every missing line and branch, but LLVM retains
seven uncovered expression regions. Region inspection shows that four are
redundant late checks after the square path has already established its closed
syntax invariants:

- top-right and bottom-left may currently decode a non-DC luma predictor only
  to be rejected when deriving the bottom-right context;
- the bottom-right leaf may currently decode a non-DC luma predictor only to be
  rejected during reconstruction; and
- the second of three identical child-partition callback propagations has its
  own source region even though the same callback failure is proved at the
  other child boundaries.

Refine the implementation before the next run:

1. distinguish the square side-leaf skipped policy from the generic two-leaf
   skipped policy and require DC prediction for both square side leaves;
2. require DC prediction as part of the boundary-DC syntax policy, making the
   later neighbor-context and reconstruction checks impossible by
   construction;
3. decode each following square child through one shared
   partition-advance-plus-syntax helper, so all three boundaries use one
   production failure propagation; and
4. retain and directly exercise the private 2x2 coefficient-grid guard. Remove
   the duplicate 16x16 guard inside the closure-generic four-leaf decoder:
   its sole production caller is already dominated by the exact 16x16
   topology gate in `validate_first_partition`, while keeping both creates an
   unreachable rejection in a distinct generic monomorph.

This does not broaden the accepted AV1 class. It moves rejection to the first
decoded syntax fact that violates the class and leaves the fixture-visible
success bytes unchanged.

### Accepted result

The first four-leaf square reconstruction slice is accepted:

- all three generated 16x16 fixtures match their documented file, AV1-item,
  five-node partition topology, scalar entropy-operation, reconstructed Y/U/V
  plane, and Pillow RGB hashes exactly;
- the independent reconstruction oracle expands from 86 to 89 positive cases.
  Its committed JSON has SHA-256
  `6dfc80be1a457e541fb0f6f902f53e5a9a849e9522ce1c7aa8bde772bbbc76de`,
  and every prior case remains byte-exact;
- production admits only the level-3 16x16 square split with four level-4
  `PARTITION_NONE` children. Child partition symbols and leaf syntax share
  adaptive state, square side leaves and the boundary leaf require DC
  prediction, and the boundary leaf accepts only the proved token-15 DC
  coefficient sequence;
- exact bottom/right edge prediction and lossless inverse-WHT reconstruction
  compose four private coded 8x8 leaves without exposing an image-processing
  API;
- the manifest now has 1,119 active rows: 841 decode and 278 encode. AVIF
  contributes 99 decode and 23 encode rows, with zero planned, skipped, or
  unwired rows;
- no dependency, unsafe Rust, public raster operation, target fork,
  fixture-selected production branch, or unlicensed source was added. The
  generated fixture provenance is recorded alongside the inputs, and the
  19-file third-party legal inventory passes;
- strict Clippy passes for no features, every individual codec feature,
  default features, and all features on both native and
  `wasm32-unknown-unknown`; formatting, strict rustdoc, whitespace, and the
  source-package verification also pass; and
- the source package contains 132 files, is 2.0 MiB unpacked, and is 427,076
  bytes (417.1 KiB) compressed. Coverage MCP run
  `0285d4e6-a959-47b8-af84-6e777e3b0ded`, snapshot
  `7f5dc69d-64d0-4055-b2a1-e347ab88d3f2`, passes all seven test binaries with
  36,366/36,366 lines, 5,334/5,334 branches, 1,834/1,834 functions, and
  60,026/60,026 regions.

## Slice 19 Exploration Plan: Direct High-Token DC Magnitudes

Status: accepted.

### Isolated mismatch

The rejected Slice 18 boundary candidate changes only the bottom-right
quadrant from `(17,91,203)` to `(17,96,203)`. It retains the accepted 16x16
five-node partition topology and reduces the scalar trace from 363 to 272
entropy operations, but its nonzero transforms select high-token values 12,
4, and 8. The current portable coefficient decoder accepts only high token 15
and then reads a Golomb extension.

Pinned scalar dav1d separates those cases: the high-token entropy symbol is the
coefficient magnitude when it is below 15, while only token 15 is replaced by
`read_golomb() + 15`. Admitting the candidate by sending every high token
through the existing Golomb path would consume the wrong bits and corrupt all
following adaptive state. This slice must first prove the direct-token syntax
and its exact coefficient, sign, dequantization, inverse-transform, and
reconstruction consequences.

### Reverse-mapping procedure

Before changing production code:

1. add a deterministic name filter to the existing partition-pattern
   diagnostic so one candidate can retain its complete trace without
   regenerating the unrelated corpus;
2. generate the accepted `(17,64,203)` square case and the candidate
   `(17,96,203)` case twice each with Pillow 12.2.0, libavif 1.4.1, libaom
   3.13.2, and scalar dav1d commit
   `b546257f770768b2c88258c533da38b91a06f737`;
3. reject either case if its two reports differ byte-for-byte, then record
   encoded-file, extracted-AV1-item, partition-topology, entropy-operation,
   reconstructed Y/U/V plane, and Pillow RGB hashes;
4. compare the two entropy streams event by event and identify the first
   divergence, including the high-token CDF context, decoded token, sign
   context, arithmetic-decoder state, coefficient value, and transform
   position;
5. map that event to dav1d's `recon_tmpl.c` high-token branch and independently
   verify that tokens below 15 consume no Golomb extension;
6. inspect every remaining nonzero transform in the candidate. Reject the
   candidate from this slice if it introduces a non-DC EOB, non-WHT transform,
   new predictor, new partition, palette/filter-intra decision, subsampling,
   alpha, or any coefficient placement beyond the already accepted DC-only
   class; and
7. reverse-map additional fixed replacement colors only when they are needed
   to cover a distinct direct high token, sign, or coefficient context. Do not
   add random fixtures or a fixture-selected production branch.

### Proposed implementation boundary

If the trace proves a closed class, refactor the private DC coefficient helper
to return the decoded magnitude:

- high token `1..=14` is the complete magnitude and consumes no Golomb bits;
- high token `15` retains the accepted `read_golomb() + 15` extension;
- token zero, non-DC EOB, nonzero AC coefficients, and every syntax class
  outside the documented boundary remain portable misses.

The caller must then apply the already decoded sign, lossless dequantization,
inverse WHT, and edge reconstruction exactly as before. This is codec-private
AV1 reconstruction, not a public image-processing operation. The change may
not add a dependency, unsafe Rust, target-specific behavior, public API, or
special case selected by fixture bytes, dimensions alone, or decoded RGB.

### Acceptance criteria

Production work is approved only after the exploration records exact
deterministic evidence and proves that the candidate is DC-only. Acceptance
then requires:

- manifest-driven exact Pillow parity for every new positive fixture and
  structured unsupported parity for the nearest excluded syntax;
- independent reconstruction-oracle parity for the encoded file, extracted
  AV1 item, partition tree, scalar entropy trace, Y/U/V planes, and Pillow RGB
  bytes;
- unchanged results for every retained fixture and oracle case;
- strict formatting, Clippy for no features, each codec feature, default
  features, and all features on native and `wasm32-unknown-unknown`, plus
  strict rustdoc, whitespace, legal, and source-package verification; and
- Coverage MCP proving exactly 100% line, branch, function, and region
  coverage.

### Exploration result and approved implementation

The two-case full trace was generated twice and is byte-identical. Both
reports have SHA-256
`a60624bab9fe1ace9c3d9be2869a7f17fd2ddb48bca15447b562d9210f0f1556`.
The oracle is Pillow 12.2.0, libavif 1.4.1, libaom 3.13.2, and scalar dav1d
`1.5.3-0-gb546257`. Both cases use quality 100, speed 8, one thread, lossless
4:4:4, disabled autotiling, and the same five-node square partition tree. Its
canonical topology hash is
`0cf74ff2639528faf5b68aa96a4731627a1dad0312eb14f4cf72e5f08fe3e990`.

| Case | File SHA-256 | AV1 item SHA-256 | Entropy trace SHA-256 | Operations |
| --- | --- | --- | --- | ---: |
| accepted `(17,64,203)` | `4a8703a56c56a2d6cbcdbec90e12d266fc28603db1f84e725f7f1a75f504fed7` | `2a970e96bba9c9e4890b80d3bc19f798d3282ac80a5f190528816c69711b3916` | `ba82b7751e44884bc48da5d7c1e3b2c031df26b001e8e887296702f00c5b0ccc` | 363 |
| candidate `(17,96,203)` | `1fcdc276a8521a7d248fa9382aca518c880921615a392d6116e3fff28320032d` | `12c92ef6e04a08cb044228c8cedd3e74767fedb7bb2afc31658b68ddd514f9ce` | `a697f0ba02f955967e04b1049860ac03fb34b22cac3d3b524bc8260ecca5cba2` | 272 |

The candidate's Y/U/V plane hashes are
`26a88b7b0ab184c7dc876773157965b847e1d4ebba7a145ac36e0e8ab1f08653`,
`2f395c7835ece618b848156a8e7ae2d7adfbd5be5a970684c831b26055ec8c50`,
and
`e3893487aef90949c3b0cfd18a259450e0d016c3dd954a846ce3c291fc26b9b6`.
Its Pillow RGB hash is
`87cf9f38f5bc4a0a75c3284ff3b5826e0c0734066e863bcf416f2296623b890f`.

The first syntax-value divergence is entropy operation 196, the fourth symbol
of the bottom-right luma transform's high-token cascade. The accepted stream
selects `3` and completes token 15; the candidate selects `0` and completes
token 12. The candidate immediately decodes the sign at operation 198 and
does not consume a Golomb bit. This matches dav1d
`src/msac.c:187-202` and `src/recon_tmpl.c:615-632`.

Every bottom-right candidate transform remains lossless WHT and DC-only:

- luma has EOB sequence `0,0,0,-1`, direct tokens `12,4,4`, and positive
  signs;
- U has EOB sequence `0,0,0,-1`, direct tokens `8,4,4`, and negative signs;
- V has EOB sequence `0,0,0,-1`, direct tokens `8,4,4`, and negative signs;
- the root and all four child partitions, predictors, palette decisions,
  filter-intra decisions, coefficient-skip contexts, transform positions, and
  reconstruction topology remain in the accepted Slice 18 class.

The production change is therefore limited to returning the decoded high
token from the private DC coefficient helper and reading the Golomb extension
only for token 15. The complete AV1-defined direct-token interval `3..=14` is
one syntax branch and must not be reduced to the observed values `4`, `8`, and
`12`. Add `(17,96,203)` as a generated manifest fixture and independent
reconstruction-oracle case, retain structured unsupported behavior for all
excluded syntax, and run every acceptance gate above before changing this
status to accepted.

### Accepted result

The direct high-token DC slice is accepted:

- the private coefficient decoder now preserves every AV1 high token below 15
  as the complete coefficient magnitude and reads the existing Golomb
  extension only for token 15; all EOB, AC-position, transform, predictor,
  partition, color-layout, and container restrictions remain unchanged;
- `partitioned_square_16x16_g96_direct_tokens.avif` matches its documented
  325-byte encoded file, 50-byte extracted AV1 item, five-node topology,
  272-operation scalar entropy trace, reconstructed Y/U/V planes, and Pillow
  RGB bytes exactly;
- the independent reconstruction oracle expands from 89 to 90 positive cases.
  Its committed JSON has SHA-256
  `fd1c864b74e44c52a63e4139f68a37f9b3977716b82030144537bf10144bd14e`,
  and every retained case remains byte-exact;
- the manifest now has 1,120 active rows: 842 decode and 278 encode. AVIF
  contributes 100 decode and 23 encode rows, with zero planned, skipped, or
  unwired rows;
- no dependency, unsafe Rust, public raster operation, target fork,
  fixture-selected production branch, or third-party source was added. The
  deterministic fixture generator and provenance document the original input,
  pinned Pillow stack, and exact oracle hashes;
- strict Clippy passes for no features, every individual codec feature,
  default features, and all features on both native and
  `wasm32-unknown-unknown`; formatting, strict rustdoc, whitespace, and the
  19-file third-party legal inventory pass;
- `cargo package --allow-dirty --locked` verifies the publishable crate. It
  contains 132 files, is 2.0 MiB unpacked, and is 427,253 bytes (417.2 KiB)
  compressed; and
- Coverage MCP run `12922eab-c261-4eed-9299-e13bee6f8af2`, snapshot
  `9c50d5bb-5fc8-4b40-b43e-44a587b86482`, passes all seven test binaries with
  36,367/36,367 lines, 5,336/5,336 branches, 1,834/1,834 functions, and
  60,023/60,023 regions.

## Slice 20 Exploration Plan: Edge-Clipped Square Visibility

Status: accepted.

### Candidate boundary

The current four-leaf path is restricted to a declared 16x16 frame even
though its private reconstruction already composes a coded 16x16 plane and
the common visibility helper can retain a smaller declared rectangle. A
declared 12x12 frame is the nearest square geometry, but the existing
half-quadrant pattern changes at source coordinate six. That boundary crosses
the coded 8x8 child edges and introduces non-DC residuals in the first three
leaves, so it is not evidence that geometry alone can reuse Slice 19.

The next diagnostic candidate keeps a 12x12 declared frame but changes the
source only where `x >= 8 && y >= 8`. This aligns the visible replacement with
the bottom-right coded child. It may prove a strictly smaller extension:
top-left remains a complete 8x8 leaf, top-right and bottom-left expose 4x8 and
8x4 rectangles, and bottom-right exposes 4x4, while the coded reconstruction
still uses four 8x8 children.

### Reverse-mapping procedure

Before production changes:

1. extend the existing diagnostic with an explicit bottom-right origin so a
   declared-frame midpoint and a coded-child boundary are different named
   inputs rather than implicit geometry behavior;
2. generate the fixed-origin 12x12 candidate twice with the pinned Pillow,
   libavif, libaom, and scalar dav1d stack and reject nondeterministic output;
3. compare it with the accepted 16x16 `(17,96,203)` case and the rejected
   12x12 midpoint control, recording exact file, AV1-item, topology,
   entropy-operation, Y/U/V, and Pillow RGB hashes;
4. identify whether edge child partitions are entropy-coded, inferred by the
   frame boundary, or use a different block size. Record the exact order in
   which each partition decision and leaf syntax is consumed;
5. inspect every predictor, palette/filter-intra decision, EOB, coefficient
   token, sign context, transform type, and reconstruction edge;
6. reject the candidate if it requires non-DC coefficients, a new predictor,
   a new transform, a deeper/asymmetric partition, subsampling, alpha,
   filtering, or any syntax beyond Slice 19 plus declared-edge clipping; and
7. only if the candidate is closed, compare its visible planes with the
   top-left 12x12 rectangle of its independently reconstructed coded planes.
   Visibility remains private AV1 output selection, not a public crop.

### Possible implementation boundary

If the trace proves exact syntax reuse, admit 12x12 only through its decoded
level-3 square split. Consume or infer each level-4 child partition exactly as
dav1d does at the right and bottom frame boundaries; do not reuse the 16x16
four-callback topology without trace evidence. Decode all four leaves through
the existing policies and shared adaptive state, compose the coded 16x16
planes, and retain the declared 12x12 top-left rectangle through the existing
visibility helper.

Dimensions alone must not select this path. A 12x12 `PARTITION_NONE` stream
continues through the existing single-leaf path, while any excluded partition
or leaf syntax remains a portable miss. No dependency, unsafe Rust, public
raster operation, target fork, or fixture-byte special case is permitted.

### Acceptance criteria

Acceptance requires a generated manifest fixture, complete pinned dav1d
reconstruction case, exact Pillow RGB parity, exact retained-case parity,
structured unsupported behavior for the midpoint control, strict native and
WASM feature matrices, rustdoc, formatting, whitespace, legal and package
verification, and Coverage MCP at exactly 100% line, branch, function, and
region coverage.

### Exploration result and approved implementation

Both complete fixed-origin reports are byte-identical with SHA-256
`2afde10ecb2d3002aeecbb97de3416c08adc8d2a5935685ea96a94a11cf5b294`.
Both midpoint-control reports are byte-identical with SHA-256
`f122162cb61172c51c3c9e294647f51ee7f194a77d7de8405d36f4cb8e55c17b`.
They use the same pinned oracle identities and deterministic encoder settings
as Slice 19.

The fixed-origin candidate is 325 bytes with file SHA-256
`b61f62f12306af9744ea06ac8c68bfd86f8b10f27caca820405b295756a3f194`.
Its 50-byte AV1 item has SHA-256
`e7265c75566e1a9f09e7059511bbee255443e32abbd719d5eed9f8cb75ba933d`.
The partition topology hash is
`0cf74ff2639528faf5b68aa96a4731627a1dad0312eb14f4cf72e5f08fe3e990`,
and the 272-operation entropy trace hash is
`a697f0ba02f955967e04b1049860ac03fb34b22cac3d3b524bc8260ecca5cba2`.
Both are exactly equal to the accepted 16x16
`partitioned_square_16x16_g96_direct_tokens` case, including every arithmetic
decoder state, CDF, symbol, partition range, predictor, coefficient, sign, and
skip decision. Dav1d emits all four level-4 `PARTITION_NONE` decisions in the
same order; no inferred edge-only topology is introduced.

The fixed candidate's visible Y/U/V hashes are
`ff66d42d0061e15fc4752e373cfe86a29080134b7bd74989dc998d493e89a6c2`,
`67135d8a5accb7bba94750eecb2f554a32d900f21558c3d281e97ea4dc660f4b`,
and
`647ed911037b576c2bbae2d62afcdef4f76709613e07554651b76864364e2617`.
Its Pillow RGB hash is
`8fd169458756409edfaf3380195c6ab881e3d7043d5c3b158a82feaaa82b993f`.
Every plane row and every Pillow RGB row is exactly the top-left 12x12
rectangle of the independently decoded 16x16 case.

The midpoint control remains excluded. It is 337 bytes with file SHA-256
`d10972f944777129121ef100ee66903959138ae946295bb5fe271cef8035b258`
and a 62-byte AV1 item with SHA-256
`2b5355aa7d702243dcf6e16933fe18241d94d0bddac3cd7f827c0b83c11cbd84`.
Its 349-operation trace has SHA-256
`a9363dc28dd49dff8441bdd7938b20a1567eb7f6092769322ddd62f3e4618a1c`.
It selects horizontal and vertical predictors in different leaves and AC
coefficient EOB values 1, 2, and 4, so it is not the same codec class.

Production is approved to:

1. add 12x12 to the square-recursive dimension gate while retaining the
   decoded root/child partition requirements;
2. reuse the exact four-child entropy and reconstruction path without a new
   edge-specific syntax branch;
3. pass each composed 16x16 coded plane through the existing declared
   visibility helper before returning it;
4. add the fixed-origin candidate as a positive manifest and reconstruction
   case; and
5. add the midpoint control as a Pillow-success manifest fixture that must
   remain a private portable reconstruction miss.

No other square dimension, source boundary, predictor, AC coefficient,
partition, or transform class is approved.

### Accepted result

The edge-clipped square visibility slice is accepted:

- the square-recursive dimension gate now admits decoded level-3 square splits
  at 12x12 and 16x16. Both geometries consume the same five partition nodes,
  shared adaptive state, four leaf policies, direct/token-15 coefficient
  rules, prediction, inverse transforms, and coded-plane composition;
- the four-leaf path now applies the existing declared-frame visibility helper
  to its composed 16x16 coded planes. The 16x16 outputs remain unchanged, and
  the new 12x12 output is the exact top-left rectangle proved independently by
  dav1d;
- `partitioned_square_12x12_g96_direct_tokens.avif` is an active positive
  manifest and reconstruction fixture. Its file, AV1 item, complete entropy
  trace, Y/U/V planes, and Pillow RGB bytes match the documented exploration
  hashes exactly;
- `partitioned_square_12x12_midpoint_g96_ac.avif` is an active Pillow/native
  success fixture and a required private portable miss. Its horizontal and
  vertical predictors plus AC EOB values 1, 2, and 4 are not admitted by this
  slice;
- the independent reconstruction oracle expands from 90 to 91 positive cases.
  Its committed JSON has SHA-256
  `630708138dabca467ef2c9e14a1e7bda74fc818dec2c2ddc27e3b986efe53e89`,
  and all 89 cases from the pushed baseline plus the Slice 19 case remain
  byte-exact;
- the manifest now has 1,122 active rows: 844 decode and 278 encode. AVIF
  contributes 102 decode and 23 encode rows, with zero planned, skipped, or
  unwired rows;
- no dependency, unsafe Rust, public raster operation, target fork,
  fixture-selected production path, or third-party source was added. Strict
  Clippy passes for no features, every individual codec feature, default
  features, and all features on native and `wasm32-unknown-unknown`;
  formatting, strict rustdoc, whitespace, and the 19-file legal inventory
  pass;
- `cargo package --allow-dirty --locked` verifies 132 publishable files,
  2.0 MiB unpacked and 427,265 bytes (417.3 KiB) compressed; and
- Coverage MCP run `09532aec-d649-4ba1-931b-2ea8e0d35c3b`, snapshot
  `de3abd79-822b-4f3c-9cc6-38dfbf037ec5`, passes all seven test binaries with
  36,372/36,372 lines, 5,336/5,336 branches, 1,835/1,835 functions, and
  60,032/60,032 regions.

## Slice 21 Exploration Plan: First Lossless AC Coefficient

Status: accepted.

### Why the midpoint control is not the implementation fixture

`partitioned_square_12x12_midpoint_g96_ac.avif` proves that the next missing
stage is real lossless coefficient decoding, but it is not one closed
extension. Its 349-operation trace changes the top-left leaf's last luma
transform to EOB 4, selects horizontal prediction in the top-right leaf,
selects vertical prediction in the bottom-left leaf, and adds chroma EOB
values 1, 2, and 4 across those leaves. Implementing that file directly would
mix coefficient scan placement, coefficient contexts, multiple EOB classes,
new square-side predictor policies, prediction edges, and reconstruction.

The smallest useful input must retain the accepted five-node square tree and
all four accepted leaf predictors while introducing one lossless AC
coefficient class in one bottom-right transform. A source change confined to
the visible part of the bottom-right coded child can isolate that behavior:
on the 12x12 frame, vary a replacement rectangle whose origin is within
`8..=11` on each axis instead of moving the boundary into the first three
children.

### Deterministic candidate sweep

Before production changes:

1. allow the partition-pattern diagnostic to accept repeated explicit
   bottom-right origins and generate the Cartesian product with repeated fixed
   replacement colors;
2. sweep origins `(8..=11, 8..=11)` using the already pinned source color
   `(17,91,203)` and deterministic nearby replacement colors. Encode every
   candidate twice and reject nondeterministic bytes;
3. retain only candidates with the exact Slice 20 five-node topology and
   accepted predictor sequence: vertical top-left, then DC top-right,
   bottom-left, and bottom-right;
4. rank retained candidates lexicographically by:
   - number of leaves containing nonzero AC coefficients;
   - number of planes containing nonzero AC coefficients;
   - number of transforms containing nonzero AC coefficients;
   - maximum EOB;
   - number of nonzero AC positions;
   - scalar entropy-operation count;
5. generate the smallest candidate twice with the complete scalar dav1d trace
   and record file, AV1 item, topology, entropy, Y/U/V, and Pillow RGB hashes;
6. map every coefficient operation to dav1d `decode_coefs` in
   `src/recon_tmpl.c`: EOB-bin/EOB-extra decoding, scan order, low/base/high
   token contexts, signs, coefficient-context propagation, lossless
   dequantization, and WHT input placement;
7. trace the exact inverse-WHT input and output for the first affected 4x4
   transform. The Rust implementation must match the independently
   reconstructed coefficient vector before pixel reconstruction is attempted;
   and
8. reject the candidate if it adds a predictor, transform type, partition,
   palette/filter-intra decision, subsampling, alpha, loop filter, or any
   coefficient class not explicitly represented by the trace.

Random pixels, arbitrary public raster operations, fixture-byte dispatch, and
dimension-only dispatch remain prohibited.

### Implementation boundary if the sweep is closed

Add one private lossless 4x4 coefficient-vector decoder rather than weakening
the DC-only helper. It may consume only the exact proved EOB/scan/context
class, but every arithmetic step and context update within that class must be
the general AV1 rule. Feed the decoded 16-entry coefficient vector through an
independently verified full inverse WHT, then through the existing
edge-derived predictor and coded-plane composition.

The existing DC-only path must remain byte- and operation-exact. Unsupported
EOBs, coefficient positions, contexts, predictors, and transforms must stop
the portable attempt at their first unproved syntax value.

### Completed reverse mapping

The initial deterministic sweep covered 112 candidates: seven nearby colors
at all sixteen origins in the bottom-right coded child. Repeating the complete
sweep produced byte-identical reports with SHA-256
`08621f05dd6780ea4236eaacd87d5ab71a74ee24f7884786afe84b65c7508ac6`.
Only eight candidates retained the required five-node square tree. The best of
those still changed all three planes, so a second fixed-origin color-direction
sweep isolated luma from chroma.

The fifteen-case color-direction reports are byte-identical with SHA-256
`d81d56896dc63b261e0b3cf9a4c2dee99b7227a040cf998c1d31723b08d5250e`.
The selected input changes source RGB `(17,91,203)` to `(22,96,208)` beginning
at `(10,8)` in a 12x12 image. It retains:

- the level-3 `PARTITION_SPLIT` followed by four level-4
  `PARTITION_NONE` children;
- the predictor sequence vertical, DC, DC, DC;
- the accepted palette and filter-intra decisions;
- DC-only or skipped syntax in the first three leaves;
- skipped U and V transforms in the bottom-right leaf; and
- exactly two luma transforms with EOB 1 in the bottom-right leaf.

The complete trace was generated twice and is byte-identical with SHA-256
`aaf515808f92b9bf9d2733c637024ab2aa638702b57c6e83ec447effbb301ccb`.
The candidate is 320 bytes with file SHA-256
`db9102a9b302387df2214814ac2cd02c8414beaf4751f3f374370237a210e9bc`.
Its 45-byte AV1 item has SHA-256
`04faf25091666ab62fe193c680174fb63a31e47adc06140aca17dfd995213201`
and consumes 247 scalar entropy operations. Pinned dav1d produces Y/U/V
SHA-256 values
`b644fd44e9e27da42e1f52a6287f9b2e42d13891b7853ce3adcaf87c1da37ace`,
`97981aad65721ea7dfb43cfd031404db089113459940f68f0a9109f1cc8d73d2`,
and
`ea6aeb0009d508f51b98a099abb01981af1694d0060b9e7821bebc23d8d91cf9`.
Pillow's RGB bytes have SHA-256
`d8ddfb34c1d4da25851a33b0515d025bd092a6bfd942eeda21683b9e564d6691`.

### Closed coefficient class

Dav1d 1.5.3 `decode_coefs` in `src/recon_tmpl.c:321-728` proves the
following exact path for both affected transforms:

1. `eob_bin_16[0][0]` decodes symbol 1. For a 4x4 two-dimensional WHT,
   this is EOB 1 and requires no EOB high bit or extra bits.
2. `eob_base_tok[0][0][1]` decodes symbol 2, producing base token 3.
   `br_tok[0][0][7]` then decodes direct high token 10.
3. `scan_4x4[1]` from `src/scan.c:35-40` maps the one AC coefficient to
   raster index 4.
4. `base_tok[0][0][0]` decodes the DC token. The preceding AC magnitude ten
   derives high-token context five, so `br_tok[0][0][5]` supplies the DC
   direct high token. Dav1d's diagnostic label reports context zero here, but
   the indexed source and scalar state transition prove context five.
5. The DC sign is adaptive; the AC sign is equiprobable. The first and third
   transform DC values are positive and their AC values are negative.
6. Q-index-zero eight-bit dequantization multiplies both direct token
   magnitudes by four. No quantization matrix, Golomb residual, saturation, or
   coefficient-context scan loop is entered.

The additional default descending CDFs, converted exactly from dav1d
`src/cdf.c:839-924,1316-1339`, are:

- EOB base context 1: `[3168, 1322, 0]`;
- base-token context 0: `[28734, 23838, 20041, 0]`; and
- high-token context 5: `[28965, 25451, 22222, 0]`; and
- high-token context 7: `[18376, 12817, 10012, 0]`.

The trace proves their adaptive updates. For example, the first EOB-base
decode changes `[3168,1322,0]` to `[5018,3287,1]`, and the first context-7
high-token decode changes `[18376,12817,10012,0]` through symbols `3,3,1`
to `[18825,13440,10723,1]` while returning token 10. The second EOB 1
transform reuses the updated tables, so shared CDF state is part of the
required implementation.

The four bottom-right luma transform vectors, in row-major transform order,
are:

1. `[40,0,0,0,-40,0,0,0,0,0,0,0,0,0,0,0]`;
2. `[32,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]`;
3. `[24,0,0,0,-40,0,0,0,0,0,0,0,0,0,0,0]`; and
4. all zero because the transform is skipped.

Dav1d's lossless inverse transform in `src/itx_tmpl.c:184-207` first reads
the coefficient matrix by columns and arithmetic-shifts every input by two.
For the first vector, the first one-dimensional pass receives
`[10,-10,0,0]` and produces `[0,0,10,10]`. The column pass therefore
produces residual rows `[0,0,5,5]`. Adding the DC predictor 81 gives
`[81,81,86,86]` on every row, exactly matching dav1d's reconstruction dump.
The third vector starts with `[6,-10,0,0]`, produces residual rows
`[-1,-1,4,4]`, and reaches the same reconstructed rows from predictor 82.
This proves that the Rust transform must accept the complete 16-entry vector;
passing only a scalar DC value cannot represent this class.

### Focused rejection control

A same-color sweep over all sixteen bottom-right origins was also repeated
byte-for-byte. Both reports have SHA-256
`3b59bb8379555274680c4f55e1ce0e796163957e7b414802cd934bc43a0b3a50`.
The nearby `(8,10)` origin retains the same tree, predictors, luma-only change,
and skipped chroma, but its two affected transforms have EOB 2. Its file
SHA-256 is
`89842483e159b7d9d98f58282679f9b6d09f4e164576270e9546b13df176c986`
and its AV1-item SHA-256 is
`b82a922f6ed1d120cea78da616206109707d3d4b9022d943a4d28b55093bb57e`.
This is the required private portable miss: EOB-bin symbol two identifies the
unsupported EOB-2 class, so the portable path rejects before consuming any
high bit, extra bits, or unproved coefficient scan loop.

### Acceptance criteria

Acceptance requires:

- a deterministic Pillow-success manifest fixture and a complete independent
  dav1d reconstruction case with exact stage hashes;
- at least one nearby valid control that remains a private portable miss at
  the first excluded coefficient class;
- exact parity for decoded coefficient vectors, inverse-WHT residuals, Y/U/V
  planes, and Pillow RGB bytes, with every retained fixture unchanged;
- no dependency, unsafe Rust, public image-processing API, target fork, or
  fixture-selected production branch;
- strict native and WASM feature matrices, rustdoc, formatting, whitespace,
  legal, and package verification; and
- Coverage MCP at exactly 100% line, branch, function, and region coverage.

### Acceptance result

The implementation keeps complete 16-entry coefficient vectors per 4x4
transform, decodes the exact luma EOB-1/direct-token-10 class, selects the DC
high-token CDF from the preceding AC magnitude, and runs the full lossless
inverse WHT. The nearby EOB-2 fixture remains a private portable miss at its
first unsupported EOB-bin symbol. A fixture-driven byte mutation sweep over
the accepted EOB-1 item covers alternate rejected DC base symbols without
injecting decoder state or adding fixture-selected production behavior.

The manifest now has 1,124 active rows: 846 decode and 278 encode. AVIF has
104 active decode rows and 23 active encode rows. The independent
reconstruction oracle expands from 91 to 92 positive cases; its JSON SHA-256
is
`406229a4d724629e2813f0f7bfe85a8bd9dd6a3c75c93090575fc1d101bf057c`.

Coverage MCP run `dfc373c8-6c1d-4aad-a616-f0176881dc7b`, snapshot
`050874ed-680d-43e5-a482-166925d773e0`, passes all seven managed test
counters with 36,448/36,448 lines, 5,342/5,342 branches, 1,840/1,840
functions, and 60,136/60,136 regions. Strict native and
`wasm32-unknown-unknown` all-feature and no-default-feature Clippy, strict
rustdoc, formatting, whitespace, the 19-file third-party legal audit, and
offline source-package verification also pass.

## Slice 22 Exploration Plan: Lossless EOB 2

Status: accepted.

### Current boundary

`partitioned_square_12x12_luma_eob2_control.avif` is the smallest retained
portable miss after Slice 21. It changes the same source RGB `(17,91,203)` to
`(22,96,208)`, beginning at `(8,10)` instead of `(10,8)`. The fixed-origin
sweep proves that it retains:

- the same level-3 split and four level-4 leaves;
- the same vertical, DC, DC, DC luma predictors;
- the same DC-only or skipped first three leaves;
- skipped U and V transforms in the bottom-right leaf; and
- two bottom-right luma transforms with EOB 2, followed by one DC-only and
  one skipped transform.

The 320-byte file SHA-256 is
`89842483e159b7d9d98f58282679f9b6d09f4e164576270e9546b13df176c986`.
Its 45-byte AV1 item SHA-256 is
`b82a922f6ed1d120cea78da616206109707d3d4b9022d943a4d28b55093bb57e`,
and the pinned scalar trace consumes 253 entropy operations. Pinned dav1d
produces Y/U/V SHA-256 values
`437a03ca722eed08fa3bd8154288bd3f03f3c437049effd2dfe6d99cf93d62d3`,
`97981aad65721ea7dfb43cfd031404db089113459940f68f0a9109f1cc8d73d2`,
and
`ea6aeb0009d508f51b98a099abb01981af1694d0060b9e7821bebc23d8d91cf9`.
Pillow's RGB SHA-256 is
`13878ffdf1168508a15759ff58c897370e8428fe522422d52149126a9cc42ef4`.

The existing scalar trace identifies the first new syntax exactly:

1. `eob_bin_16[0][0]` decodes symbol 2;
2. `eob_hi_bit[0][0][0]` decodes zero, producing EOB 2;
3. reverse scan position two uses `scan_4x4[2] == 1`;
4. `base_tok[0][0][1]` decodes symbol 3 and
   `br_tok[0][0][7]` produces direct token 10;
5. reverse scan position one uses `scan_4x4[1] == 4` and its base token is
   zero, so only raster coefficient one is nonzero; and
6. DC remains a direct high token with context five derived from AC magnitude
   ten.

The complete instrumented trace below closes this mapping.

### Completed reference mapping

Two fresh runs of the pinned Pillow 12.2.0/libavif 1.4.1/libaom 3.13.2
encoder reproduced the committed 320-byte fixture exactly. Two complete
instrumented scalar dav1d 1.5.3 reports are also byte-identical, with report
SHA-256
`8f295b8caf08e7c9d285b66e8a2984fc59c6cfadb9dfe56f25a47ff53090055c`.
Each report retains the partition tree, all 253 entropy operations and
adaptive CDF states, coefficient vectors, reconstructed plane rows, and
Pillow rows.

The first affected transform maps directly to dav1d
`src/recon_tmpl.c:407-564`:

- EOB-bin symbol two selects EOB-bin index zero. The luma
  `eob_hi_bit[0][0][0]` CDF begins at `[15807,0]`; decoding zero updates it
  to `[14820,1]`. The second affected transform decodes zero again and
  updates it to `[13894,2]`.
- `eob = ((0 | 2) << 0) | 0` is exactly two and consumes no equiprobable
  EOB-extra bits.
- `scan_4x4[2] == 1` selects raster coefficient one. EOB-base context one
  returns symbol two, and high-token context seven returns direct token ten.
- The reverse loop then visits `scan_4x4[1] == 4`. Base-token context one
  returns zero, updating its CDF from `[14686,3027,891,0]` to
  `[14228,2933,864,1]`, then to `[13784,2842,837,2]` after the second
  affected transform. No coefficient is stored at raster position four.
- DC base-token context zero returns symbol three. The AC neighborhood
  magnitude ten selects DC high-token context five, which returns direct
  tokens ten and six. DC signs are positive and the equiprobable raster-one
  AC signs are negative.
- Q-index-zero luma dequantization multiplies every direct magnitude by four.
  The first three coefficient vectors are therefore
  `[40,-40,0,0,0,0,0,0,0,0,0,0,0,0,0,0]`,
  `[24,-40,0,0,0,0,0,0,0,0,0,0,0,0,0,0]`, and
  `[32,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]`; the fourth transform is skipped.

An independent application of dav1d `src/itx_1d.c:1066-1080` and
`src/itx_tmpl.c:184-207` gives residual rows `[0,0,0,0]`,
`[0,0,0,0]`, `[5,5,5,5]`, `[5,5,5,5]` for the first vector and
`[-1,-1,-1,-1]`, `[-1,-1,-1,-1]`, `[4,4,4,4]`,
`[4,4,4,4]` for the second. Their DC predictors are 81 and 82, so both
reconstruct to rows 81, 81, 86, 86. The third transform's predictor is 84
and its constant residual is two; the skipped fourth transform propagates
predictor 86. The complete coded bottom-right luma leaf therefore contains
two rows of 81 followed by six rows of 86, exactly matching the
instrumented C trace. Clipping that coded leaf to the declared 12x12 frame
produces the two changed visible rows and the pinned Y hash above.

The `(10,10)` rejection control begins its bottom-right leaf with EOB-bin
symbol three, EOB-high index one, and final EOB four. It therefore remains
outside the closed EOB-two implementation before any EOB-four coefficient
token is consumed.

### Required reverse mapping before implementation

Completed before changing production code:

1. regenerate the target twice with the pinned Pillow 12.2.0/libavif
   1.4.1/libaom 3.13.2 encoder and require byte-identical AVIF bytes;
2. generate the complete pinned dav1d 1.5.3 scalar trace twice and require
   byte-identical partition, entropy, coefficient, reconstructed-plane, and
   Pillow reports;
3. map EOB-bin symbol two and the EOB high bit to dav1d
   `src/recon_tmpl.c:321-442`, including the exact adaptive CDF and update;
4. map both reverse scan positions, their low/high token contexts, signs, DC
   context, and Q-index-zero dequantization through
   `src/recon_tmpl.c:443-718` and `src/scan.c:35-40`;
5. independently calculate all four bottom-right luma coefficient vectors,
   inverse-WHT residuals, predictors, and reconstructed rows, then compare
   them with the instrumented C output; and
6. stop if the trace introduces any unlisted EOB extra bit, coefficient
   context, token class, predictor, chroma residual, transform type, or
   reconstruction rule.

### Focused next rejection control

The same deterministic sweep identifies origin `(10,10)` as the next closed
control. It retains the five-node tree, predictor sequence, and skipped
chroma, but its bottom-right luma transforms have EOB values 4, 2, 1, and
skipped. Its 323-byte file SHA-256 is
`307512d55df127d8546273a57dedd182fbdb5282aa830191f7fe201b8eff419f`;
the 48-byte AV1 item SHA-256 is
`5a69a48c7b9f35f00c3131041eb2ce2e2bf724761051a22be18e2250be58acf1`.
Pinned dav1d reports 264 entropy operations and Y/U/V SHA-256 values
`99aec1ce0f240e9ef0096a4e792de99cced21a592c3506f44ed6d314b552e22d`,
`97981aad65721ea7dfb43cfd031404db089113459940f68f0a9109f1cc8d73d2`,
and
`ea6aeb0009d508f51b98a099abb01981af1694d0060b9e7821bebc23d8d91cf9`.
Pillow's RGB SHA-256 is
`299dc7d8cf7b620bb3cc3a56ab17da5414d8377e0b79196fce64cae0e05ca7f3`.
It must remain a private portable miss at its first EOB-4 syntax value.

### Implementation boundary and acceptance

If the complete trace closes the class, extend the private luma coefficient
decoder with the general AV1 EOB-2 rule. Do not dispatch by fixture, file
hash, dimensions, or transform ordinal. The decoder may admit only the proved
EOB-2 scan/token class, but the EOB high-bit CDF update, scan placement,
coefficient-vector construction, sign handling, dequantization, and inverse
WHT must follow the general reference rule within that class.

Acceptance requires:

- the existing EOB-2 manifest fixture becoming an independent reconstruction
  positive with exact entropy, Y/U/V, and Pillow RGB parity;
- a new deterministic EOB-4 manifest success fixture that remains a private
  portable miss at its first unsupported syntax value;
- every prior reconstruction case and manifest row remaining byte-exact;
- no dependency, unsafe Rust, public image-processing API, target fork, or
  fixture-selected production path;
- strict native and WASM feature matrices, rustdoc, formatting, whitespace,
  legal, and package verification; and
- Coverage MCP at exactly 100% line, branch, function, and region coverage.

### Acceptance result

The private coefficient decoder now implements the general proved EOB-2
syntax class. It decodes and adaptively updates luma EOB-high context zero,
places the final direct token at `scan_4x4[2] == 1`, consumes the zero token
at `scan_4x4[1] == 4`, derives the DC high-token context from the AC
magnitude, preserves the reference sign order, and reconstructs the complete
coefficient vector through the existing inverse WHT. No fixture name, file
hash, dimension, transform ordinal, target-specific branch, dependency,
unsafe Rust, or public image-processing API selects this path.

The former EOB-2 control is now the ninety-third independent reconstruction
positive. Its production result matches all 253 pinned scalar entropy
operations, exact adaptive CDF states, Y/U/V rows and hashes, and Pillow RGB
bytes. The new 323-byte
`partitioned_square_12x12_luma_eob4_control.avif` is an active manifest
success with exact Pillow RGB SHA-256
`299dc7d8cf7b620bb3cc3a56ab17da5414d8377e0b79196fce64cae0e05ca7f3`;
the private portable classifier rejects it at EOB-bin symbol three before
consuming any EOB-4 coefficient token.

The manifest contains 1,125 active rows: 847 decode and 278 encode. AVIF has
105 active decode rows and 23 active encode rows, with no planned, skipped,
or unwired rows. The deterministic 93-case reconstruction oracle has SHA-256
`f0b3c1e49f8f254112806bf24fa49438e39268d13b78373044c9d7445536275c`,
and every retained case remains byte-exact.

Strict Clippy passes for no features, every individual codec feature,
defaults, and all features on native and `wasm32-unknown-unknown`. Strict
rustdoc, formatting, whitespace, the 19-file third-party legal audit, and
offline source-package verification also pass. The source package contains
132 files, is 2.0 MiB unpacked, and is 428,368 bytes compressed.

Coverage MCP run `a4220bfc-8686-460c-9341-863a7f53bcce`, snapshot
`5494e33b-7726-4b41-b515-6a8ececc985f`, passes all seven test binaries with
36,477/36,477 lines, 5,346/5,346 branches, 1,843/1,843 functions, and
60,180/60,180 regions.

## Slice 23 Exploration Plan: Lossless EOB 4

Status: accepted.

### Current boundary and reproducibility

`partitioned_square_12x12_luma_eob4_control.avif` is the smallest retained
portable miss after Slice 22. It changes source RGB `(17,91,203)` to
`(22,96,208)` beginning at `(10,10)`. It retains the same five-node square
partition tree, vertical/DC/DC/DC luma predictor sequence, DC-only or skipped
first three leaves, and skipped U and V transforms in the bottom-right leaf.
Only the first bottom-right luma transform is new: its four transforms have
EOB values 4, 2, 1, and skipped.

Two fresh runs through the pinned Pillow 12.2.0, libavif 1.4.1, libaom
3.13.2, and scalar instrumented dav1d 1.5.3 stack reproduced both requested
cases exactly. The complete reports are byte-identical with SHA-256
`d55eda1170c890ef84a4c58d1b6e44354f665e95777e3aab27a422c3ece1831d`.
Each report retains the encoded file and AV1 item hashes, five partition
nodes, every adaptive entropy operation and CDF state, coefficient vectors,
reconstructed Y/U/V rows, and Pillow RGB rows.

The EOB-4 target remains the committed 323-byte file with SHA-256
`307512d55df127d8546273a57dedd182fbdb5282aa830191f7fe201b8eff419f`.
Its 48-byte AV1 color item has SHA-256
`5a69a48c7b9f35f00c3131041eb2ce2e2bf724761051a22be18e2250be58acf1`,
and the complete scalar decode consumes 264 entropy operations. Pinned dav1d
produces Y/U/V SHA-256 values
`99aec1ce0f240e9ef0096a4e792de99cced21a592c3506f44ed6d314b552e22d`,
`97981aad65721ea7dfb43cfd031404db089113459940f68f0a9109f1cc8d73d2`,
and
`ea6aeb0009d508f51b98a099abb01981af1694d0060b9e7821bebc23d8d91cf9`.
Pillow's RGB SHA-256 is
`299dc7d8cf7b620bb3cc3a56ab17da5414d8377e0b79196fce64cae0e05ca7f3`.

### First-divergence entropy mapping

The first unsupported value is dav1d `src/recon_tmpl.c:403-546` EOB-bin
symbol three for a 4x4 two-dimensional WHT. The complete trace maps the
syntax as follows:

1. `eob_bin_16[0][0]` returns symbol three. EOB-high context one begins at
   `[15545,0]`, the reverse representation of dav1d
   `src/cdf.c:787-797` `CDF1(17223)`. It decodes zero and updates to
   `[14574,1]`.
2. The one equiprobable EOB-extra bit is zero, so
   `((0 | 2) << 1) | 0` is exactly four. The next-control EOB-six path
   instead decodes the same high bit as one and must remain rejected before
   coefficient syntax.
3. `scan_4x4[4] == 5` selects the final coefficient. EOB-base context two
   begins at `[1924,890,0]`, from dav1d `src/cdf.c:839-847`
   `CDF2(30844,31878)`, returns base symbol two, and updates to
   `[3851,2882,1]`. High-token context seven returns direct token five and
   updates from `[18376,12817,10012,0]` to `[18825,13440,9700,1]`.
4. The reverse loop visits `scan_4x4[3] == 2`. Base-token context six begins
   at `[12101,2222,839,0]`, returns zero, and updates to
   `[11723,2153,813,1]`.
5. The loop then visits `scan_4x4[2] == 1` and `scan_4x4[1] == 4`.
   Both use base-token context three, which begins at
   `[23322,11650,5763,0]`, returns symbol three twice, and updates through
   `[23617,12309,6606,1]` to `[23902,12948,7423,2]`. Their neighborhood
   magnitudes select high-token context ten. It begins at
   `[23592,17128,12509,0]`, returns direct token five twice, and updates
   through `[23878,17616,12119,1]` to `[24155,18089,11741,2]`.
6. DC base-token context zero returns symbol three. The three neighboring
   level bytes are direct-token values `197`; their low six bits sum to
   fifteen, so dav1d `src/recon_tmpl.c:535-545` selects high-token context
   six. It returns direct token five and updates from
   `[31072,29451,27897,0]` to `[31125,29554,27026,1]`.
7. DC sign is read first and is positive. The coefficient link chain is
   raster 4 to raster 1 to raster 5, so the three equiprobable signs are read
   in exactly that order: negative, negative, positive.

No token is fifteen, so this target consumes no Golomb residual extension.
The final raster coefficient vector is
`[20,-20,0,0,-20,20,0,0,0,0,0,0,0,0,0,0]` after Q-index-zero luma
dequantization by four. This is the first point at which Slice 22 cannot
continue: it has neither EOB-high context one nor the EOB-base, base-token,
and high-token contexts needed to construct that vector.

### Independent inverse-transform and reconstruction check

Applying dav1d `src/itx_1d.c:1066-1080` and
`src/itx_tmpl.c:184-203` independently to the new vector gives residual
rows:

```text
0 0 0 0
0 0 0 0
0 0 5 5
0 0 5 5
```

The first transform's DC predictor is 81, so its reconstructed rows are
81, 81, `[81,81,86,86]`, and `[81,81,86,86]`, exactly matching the
instrumented C trace. The next transform is the already proved EOB-2 vector
`[24,-40,0,0,0,0,0,0,0,0,0,0,0,0,0,0]` with predictor 82; it
reconstructs rows 81, 81, 86, and 86. The third is the already proved EOB-1
vector `[24,0,0,0,-40,0,0,0,0,0,0,0,0,0,0,0]` with predictor 82;
every row reconstructs to `[81,81,86,86]`. The fourth transform is skipped
and propagates predictor 86.

The complete coded bottom-right 8x8 luma leaf therefore has two rows of 81;
its remaining six rows begin with two samples of 81 and end with six samples
of 86. Clipping that coded leaf to the declared 12x12 frame changes only the
last two pixels of visible rows ten and eleven from 81 to 86, matching every
pinned dav1d row and the target Y hash.

### Focused next rejection control

The repeated fixed-color origin sweep identifies `(9,8)` as the closest
same-topology control after EOB 4. It preserves all five partition nodes,
the vertical/DC/DC/DC luma modes, and skipped bottom-right chroma, but its
bottom-right luma transforms have EOB values 6, 0, 6, and skipped. Its first
new syntax shares EOB-bin symbol three and EOB-high context one with the
target, then decodes the high bit as one. The one EOB-extra bit is zero,
producing EOB six. Slice 23 must reject at that high bit before consuming its
different scan, token, Golomb, or coefficient syntax.

The control is a deterministic 322-byte AVIF with SHA-256
`90583dd6d88fce42d0cfdb8f9e7217d02d5a711f873955a04526dd99f5886efa`.
Its 47-byte AV1 item has SHA-256
`af985c2ea7d4dd688a143ca8597d117beebd6a8fa8aee3196afea781383312f2`,
and pinned dav1d consumes 273 entropy operations. Its Y/U/V SHA-256 values
are
`cd323900161eb78d0081093fb1aee1c05d7d5fb419d8c9c133b508fce5c402c3`,
`97981aad65721ea7dfb43cfd031404db089113459940f68f0a9109f1cc8d73d2`,
and
`ea6aeb0009d508f51b98a099abb01981af1694d0060b9e7821bebc23d8d91cf9`.
Pillow's RGB SHA-256 is
`84c006c2c0f8e322453101374baeb3c0f1e30653b7960fb1068cfc8f33c96e68`.

### Implementation boundary

Extend only the private lossless 4x4 two-dimensional luma coefficient
decoder:

1. add the exact q-context-zero EOB-high context one, EOB-base context two,
   base-token contexts three and six, and high-token context ten CDFs;
2. split the existing single-AC helper so DC low/high token and sign parsing
   can receive the reference-derived DC high-token context without changing
   EOB-1 or EOB-2 entropy order;
3. decode EOB four through the reference scan order, coefficient link order,
   direct-token extension rule, signs, and dequantization; and
4. leave EOB-high one, every unproved low/high token or context, token
   fifteen, other plane, transform type, or transform size rejected.

The production path must be selected only by parsed AV1 syntax. It must not
inspect a fixture name, file hash, dimensions, transform ordinal, encoded
byte offset, target, or expected output. Do not broaden this slice to the
EOB-6 control or any untraced coefficient class.

### Acceptance criteria

- The existing EOB-4 manifest fixture becomes an independent reconstruction
  positive matching all 264 entropy operations, every adaptive CDF state,
  exact Y/U/V bytes and hashes, and exact Pillow RGB bytes.
- A regenerated `(9,8)` EOB-6 manifest success fixture remains a private
  portable miss at EOB-high one before coefficient syntax.
- The EOB-6 bytes are regenerated by the existing deterministic fixture
  script and retain the pinned file, AV1 item, plane, and Pillow hashes.
- Every previous reconstruction case and all manifest rows remain
  byte-exact, with no planned, skipped, or unwired row.
- There is no new dependency, unsafe Rust, public image-processing API,
  target fork, native-only behavior, or fixture-selected production path.
- Strict Clippy passes for no features, each codec feature, defaults, and all
  features on native and `wasm32-unknown-unknown`; strict rustdoc, rustfmt,
  whitespace, third-party legal audit, and offline source-package checks pass.
- Coverage MCP is the only test runner and reports exactly 100% line, branch,
  function, and region coverage after all implementation and documentation
  changes are complete.

### Acceptance result

The private coefficient decoder now implements the proved lossless EOB-4
syntax class. Its q-context-zero CDF storage is indexed by the same EOB-high,
EOB-base, base-token, and high-token contexts used by dav1d. The decoder
accepts the zero EOB-high bit and zero extra bit, follows scan positions
5, 2, 1, and 4, derives DC high-token context six from the three neighboring
direct magnitudes, reads signs through the 4-to-1-to-5 coefficient link chain,
and reconstructs the exact coefficient vector through the existing inverse
WHT. EOB-high one remains rejected before coefficient syntax. No fixture
name, hash, dimension, transform ordinal, byte offset, target, dependency,
unsafe Rust, or public image-processing API selects this path.

The former EOB-4 control is now the ninety-fourth independent reconstruction
positive. The production trace matches all 264 pinned scalar entropy
operations and adaptive CDF states, exact Y/U/V rows and hashes, and exact
Pillow RGB bytes. The deterministic reconstruction oracle has SHA-256
`c676c92062ba74c34881fba07350ae92ca709dc99b17777f97c246d78d87d763`.

The new 322-byte
`partitioned_square_12x12_luma_eob6_control.avif` is an active manifest
success with file SHA-256
`90583dd6d88fce42d0cfdb8f9e7217d02d5a711f873955a04526dd99f5886efa`
and exact Pillow RGB SHA-256
`84c006c2c0f8e322453101374baeb3c0f1e30653b7960fb1068cfc8f33c96e68`.
The private portable classifier rejects it at the EOB-high-one value before
consuming its EOB-extra bit or coefficient syntax.

The manifest contains 1,126 active rows: 848 decode and 278 encode. AVIF has
106 active decode rows and 23 active encode rows, with no planned, skipped,
or unwired rows. Every retained manifest and reconstruction case remains
byte-exact.

Coverage MCP run `fdaa7e18-8e8c-4769-8ca8-d3c1f3581099`, snapshot
`d9e81050-c253-4438-8e3c-2c1ccfe1d283`, passes all seven test binaries with
36,530/36,530 lines, 5,350/5,350 branches, 1,845/1,845 functions, and
60,300/60,300 regions.

Strict Clippy passes for no features, every individual codec feature,
defaults, and all features on native and `wasm32-unknown-unknown`. Strict
rustdoc, formatting, whitespace, and the 19-file third-party legal inventory
also pass. Offline `cargo package` verifies 132 publishable files, 2.0 MiB
unpacked and 428,962 bytes compressed.

## Slice 24 Exploration Plan: Lossless EOB 6

Status: accepted.

### Current boundary and reproducibility

`partitioned_square_12x12_luma_eob6_control.avif` is the smallest retained
portable miss after Slice 23. It changes source RGB `(17,91,203)` to
`(22,96,208)` beginning at `(9,8)`. It retains the same five-node square
partition tree, vertical/DC/DC/DC luma predictor sequence, DC-only or skipped
first three leaves, and skipped U and V transforms in the bottom-right leaf.
Its four bottom-right luma transforms have EOB values 6, 0, 6, and skipped.

Two fresh runs through the pinned Pillow 12.2.0, libavif 1.4.1, libaom
3.13.2, and scalar instrumented dav1d 1.5.3 stack produced byte-identical
complete reports with SHA-256
`4fe2b8457ae3ab73572d0a0cdee7b458791d29f078a900b1c035475f57496122`.
Each report retains the encoded file and AV1 item hashes, all five partition
nodes, every adaptive entropy operation and CDF state, coefficient vectors,
reconstructed Y/U/V rows, and Pillow RGB rows.

The EOB-6 target is the committed 322-byte file with SHA-256
`90583dd6d88fce42d0cfdb8f9e7217d02d5a711f873955a04526dd99f5886efa`.
Its 47-byte AV1 color item has SHA-256
`af985c2ea7d4dd688a143ca8597d117beebd6a8fa8aee3196afea781383312f2`,
and the complete scalar decode consumes 273 entropy operations. The five
partition ranges remain 34880, 40768, 50626, 52336, and 54330. Pinned dav1d
produces Y/U/V SHA-256 values
`cd323900161eb78d0081093fb1aee1c05d7d5fb419d8c9c133b508fce5c402c3`,
`97981aad65721ea7dfb43cfd031404db089113459940f68f0a9109f1cc8d73d2`,
and
`ea6aeb0009d508f51b98a099abb01981af1694d0060b9e7821bebc23d8d91cf9`.
Pillow's RGB SHA-256 is
`84c006c2c0f8e322453101374baeb3c0f1e30653b7960fb1068cfc8f33c96e68`.

### First-divergence entropy mapping

The first unsupported value remains dav1d `src/recon_tmpl.c:403-546`
EOB-bin symbol three for a 4x4 two-dimensional WHT. Unlike the proved EOB-4
path, EOB-high context one returns one:

1. `eob_bin_16[0][0]` returns symbol three. EOB-high context one begins at
   `[15545,0]`, the reverse representation of dav1d
   `src/cdf.c:787-797` `CDF1(17223)`. It decodes one and updates to
   `[16621,1]`.
2. The one equiprobable EOB-extra bit is zero, so
   `((1 | 2) << 1) | 0` is exactly six. This is the first point where the
   Slice 23 classifier stops.
3. `scan_4x4[6] == 12` selects the final coefficient. EOB-base context three
   begins at `[7842,3820,0]`, from dav1d `src/cdf.c:839-847`
   `CDF2(24926,28948)`, returns base symbol two, and updates to
   `[9399,5629,1]`. Its two-dimensional distance selects high-token context
   fourteen, which begins at `[20632,14719,11342,0]`, returns direct token
   five, and updates to `[21011,15283,10988,1]`.
4. The reverse loop first visits `scan_4x4[5] == 8`. Base-token context eight
   begins at `[24617,14011,7990,0]`, from dav1d
   `src/cdf.c:881-890` `CDF3(8151,18757,24778)`, returns symbol three, and
   updates to `[24871,14597,8764,1]`. Its neighborhood selects high-token
   context seventeen, which begins at `[24396,18324,13921,0]`, returns
   direct token five, and updates to `[24657,18775,13486,1]`.
5. The loop then visits `scan_4x4[4] == 5` and `scan_4x4[3] == 2`.
   Both use base-token context six and return zero. It next visits
   `scan_4x4[2] == 1`, where base-token context one also returns zero.
6. `scan_4x4[1] == 4` uses base-token context four. It returns symbol three,
   and its neighborhood selects the already proved high-token context ten,
   which returns direct token five.
7. DC base-token context zero returns symbol three. The three neighboring
   level bytes select DC high-token context three. On the first EOB-6
   transform, four adaptive high-token symbols produce token fifteen. DC
   sign is positive, then dav1d's residual rule consumes a Golomb value:
   its first equiprobable bit is one, so the extension length and value are
   zero and the final DC token remains fifteen.
8. The nonzero link chain reads the three AC signs in raster order 4, 8,
   and 12. All three are negative. The final raster coefficient vector is
   `[60,0,0,0,-20,0,0,0,-20,0,0,0,-20,0,0,0]` after Q-index-zero
   luma dequantization by four.

The third transform of the same leaf repeats the EOB-6 EOB, scan, base-token,
and AC sign syntax after all adaptive CDF updates from the first two
transforms. Its DC high-token context three returns direct token seven, so
it has no Golomb extension. Its final coefficient vector is
`[28,0,0,0,-20,0,0,0,-20,0,0,0,-20,0,0,0]`. This second occurrence
proves that the production implementation must use the adaptive context
tables rather than hard-coded post-update states or transform ordinals.

### Independent inverse-transform and reconstruction check

Applying dav1d `src/itx_1d.c:1066-1080` and
`src/itx_tmpl.c:184-203` independently to the first EOB-6 vector gives the
same residual row four times:

```text
-3 2 2 2
-3 2 2 2
-3 2 2 2
-3 2 2 2
```

The first transform's DC predictor is 84, so each reconstructed row is
`[81,86,86,86]`. The second transform is the existing DC-only vector
`[32,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]` with predictor 83 and
reconstructs entirely to 86. The third EOB-6 vector produces residual rows
`[-2,3,3,3]`; with predictor 83, it also reconstructs each row to
`[81,86,86,86]`. The fourth transform is skipped and propagates predictor
86.

The complete bottom-right 8x8 luma leaf therefore has every row equal to
`[81,86,86,86,86,86,86,86]`. Clipping it to the declared 12x12 frame
changes the visible bottom four rows from x zero through eight to 81 and
from x nine through eleven to 86, matching every pinned dav1d row and the
target Y hash.

### Focused next rejection control

The repeated fixed-color origin sweep identifies `(8,9)` as the closest
same-topology control after EOB 6. It preserves all five partition nodes,
the vertical/DC/DC/DC luma modes, and skipped bottom-right chroma, while its
bottom-right luma transforms have EOB values 9, 9, 0, and skipped.

The control is a deterministic 323-byte AVIF with SHA-256
`d57ddb0c7dbcdfc63aa77f3bdbd64a793246451528dfec553c0bf800f8137d4b`.
Its 48-byte AV1 item has SHA-256
`8bf74644553aa669766cedd7d155f6738f8eecab1c977a12c27c6e2f54d3c770`,
and pinned dav1d consumes 289 entropy operations. Its Y/U/V SHA-256 values
are
`d320724da5e2f9665b24dc94e697d0291098acc5f1613bd8e0f0575e9bc6499c`,
`97981aad65721ea7dfb43cfd031404db089113459940f68f0a9109f1cc8d73d2`,
and
`ea6aeb0009d508f51b98a099abb01981af1694d0060b9e7821bebc23d8d91cf9`.
Pillow's RGB SHA-256 is
`7b69d30ebe2894d11aa6d4f7c3385c8675a4cf8daf702d5b6cd709a6001ce506`.

Its first new syntax is EOB-bin symbol four. EOB-high context two begins at
`[25147,0]`, the reverse representation of dav1d
`src/cdf.c:787-797` `CDF1(7621)`, and returns zero. Two equiprobable extra
bits then return zero and one, producing EOB nine. Slice 24 must reject
symbol four before consuming its EOB-high, EOB-extra, or coefficient syntax.
That boundary keeps every EOB-9 scan, token, context, sign, and transform
outside this slice.

### Implementation boundary

Extend only the private lossless 4x4 two-dimensional luma coefficient
decoder:

1. retain EOB-bin symbol three as a shared EOB-4/EOB-6 class, branching on
   EOB-high context one and requiring the proved zero extra bit;
2. add the exact q-context-zero EOB-base context three, base-token contexts
   seven and eight, and high-token contexts eleven through seventeen so the
   context-indexed tables remain aligned with dav1d;
3. decode EOB six through scan positions 12, 8, 5, 2, 1, and 4, the proved
   base/high contexts, coefficient link order, DC-context derivation,
   token-fifteen Golomb extension, signs, and dequantization; and
4. leave EOB-bin symbol four, a one extra bit, every unproved token or
   context, other plane, transform type, or transform size rejected before
   its downstream syntax.

The production path must be selected only by parsed AV1 syntax. It must not
inspect a fixture name, file hash, dimensions, transform ordinal, encoded
byte offset, target, or expected output. Do not broaden this slice to the
EOB-9 control or any untraced coefficient class.

### Acceptance criteria

- The existing EOB-6 manifest fixture becomes the ninety-fifth independent
  reconstruction positive, matching all 273 entropy operations, every
  adaptive CDF state, exact Y/U/V bytes and hashes, and exact Pillow RGB
  bytes.
- A regenerated `(8,9)` EOB-9 manifest success fixture remains a private
  portable miss at EOB-bin symbol four before EOB-high or coefficient syntax.
- The EOB-9 bytes are regenerated by the existing deterministic fixture
  script and retain the pinned file, AV1 item, plane, and Pillow hashes.
- Both EOB-4 and EOB-6 branches, the rejected one extra bit, and the EOB-9
  boundary are exercised through manifest-derived reverse-mapped inputs.
- Every previous reconstruction case and all manifest rows remain
  byte-exact, with no planned, skipped, or unwired row.
- There is no new dependency, unsafe Rust, public image-processing API,
  target fork, native-only behavior, or fixture-selected production path.
- Strict Clippy passes for no features, each codec feature, defaults, and all
  features on native and `wasm32-unknown-unknown`; strict rustdoc, rustfmt,
  whitespace, third-party legal audit, and offline source-package checks pass.
- Coverage MCP is the only test runner and reports exactly 100% line, branch,
  function, and region coverage after all implementation and documentation
  changes are complete.

### Acceptance result

The private coefficient decoder now implements the proved lossless EOB-6
syntax class. EOB-bin symbol three shares the exact EOB-high context-one
update for both EOB four and EOB six, while the equiprobable extra bit remains
zero. The EOB-6 path follows scan positions 12, 8, 5, 2, 1, and 4, uses the
reference-derived EOB-base, base-token, and high-token contexts, derives DC
high-token context three from the AC neighborhood, and preserves the raster
4-to-8-to-12 sign chain. The first transform exercises token fifteen with a
zero Golomb extension; the second exercises direct token seven after the
adaptive CDF updates. No fixture name, hash, dimension, transform ordinal,
byte offset, target, dependency, unsafe Rust, or public image-processing API
selects this path.

The former EOB-6 control is now the ninety-fifth independent reconstruction
positive. The production trace matches all 273 pinned scalar entropy
operations and adaptive CDF states, exact Y/U/V rows and hashes, and exact
Pillow RGB bytes. The deterministic reconstruction oracle has SHA-256
`799c877ecf878ac72562a394ef0d2898cdbc948ae77675e918d87dbf2c3b7a90`.

The new 323-byte
`partitioned_square_12x12_luma_eob9_control.avif` is an active manifest
success with file SHA-256
`d57ddb0c7dbcdfc63aa77f3bdbd64a793246451528dfec553c0bf800f8137d4b`
and exact Pillow RGB SHA-256
`7b69d30ebe2894d11aa6d4f7c3385c8675a4cf8daf702d5b6cd709a6001ce506`.
The private portable classifier rejects it at EOB-bin symbol four before
consuming its EOB-high, EOB-extra, or coefficient syntax.

The manifest contains 1,127 active rows: 849 decode and 278 encode. AVIF has
107 active decode rows and 23 active encode rows, with no planned, skipped,
or unwired rows. Every retained manifest and reconstruction case remains
byte-exact.

Coverage MCP run `1e7f8026-cd28-4252-8889-9acbc04e4cc8`, snapshot
`11e51243-c1b7-4764-a654-90a2b732d012`, passes all seven test binaries with
36,587/36,587 lines, 5,352/5,352 branches, 1,847/1,847 functions, and
60,419/60,419 regions.

Strict Clippy passes for no features, every individual codec feature,
defaults, and all features on native and `wasm32-unknown-unknown`. Strict
rustdoc, formatting, whitespace, and the 19-file third-party legal inventory
also pass. Offline `cargo package` verifies 132 publishable files, 2.0 MiB
unpacked and 429,436 bytes compressed.

## Slice 25 Exploration Plan: Lossless EOB 9

Status: accepted.

### Current boundary and reproducibility

`partitioned_square_12x12_luma_eob9_control.avif` is the smallest retained
portable miss after Slice 24. It changes source RGB `(17,91,203)` to
`(22,96,208)` beginning at `(8,9)`. It retains the same five-node square
partition tree, vertical/DC/DC/DC luma predictor sequence, DC-only or skipped
first three leaves, and skipped U and V transforms in the bottom-right leaf.
Its four bottom-right luma transforms have EOB values 9, 9, 0, and skipped.

A deterministic 36-origin sweep over the same 12x12 source family first
classified every nearby partition topology and coefficient sequence. Two
subsequent full runs through the pinned Pillow 12.2.0, libavif 1.4.1, libaom
3.13.2, and scalar instrumented dav1d 1.5.3 stack reproduced the selected
EOB-9 target and EOB-10 control exactly. The complete reports are
byte-identical with SHA-256
`cc6c85749e2b6753605b7f6c056d2a56f3a96ab3cbd3e963e2428088ab7c3728`.
Each report retains the encoded file and AV1 item hashes, all five partition
nodes, every adaptive entropy operation and CDF state, coefficient vectors,
reconstructed Y/U/V rows, and Pillow RGB rows.

The EOB-9 target is the committed 323-byte file with SHA-256
`d57ddb0c7dbcdfc63aa77f3bdbd64a793246451528dfec553c0bf800f8137d4b`.
Its 48-byte AV1 color item has SHA-256
`8bf74644553aa669766cedd7d155f6738f8eecab1c977a12c27c6e2f54d3c770`,
and the complete scalar decode consumes 289 entropy operations. The five
partition ranges remain 34880, 40768, 50626, 52336, and 54330. Pinned dav1d
produces Y/U/V SHA-256 values
`d320724da5e2f9665b24dc94e697d0291098acc5f1613bd8e0f0575e9bc6499c`,
`97981aad65721ea7dfb43cfd031404db089113459940f68f0a9109f1cc8d73d2`,
and
`ea6aeb0009d508f51b98a099abb01981af1694d0060b9e7821bebc23d8d91cf9`.
Pillow's RGB SHA-256 is
`7b69d30ebe2894d11aa6d4f7c3385c8675a4cf8daf702d5b6cd709a6001ce506`.

### First-divergence entropy mapping

The first unsupported value is dav1d `src/recon_tmpl.c:403-546` EOB-bin
symbol four for a 4x4 two-dimensional WHT:

1. `eob_bin_16[0][0]` returns symbol four. EOB-high context two begins at
   `[25147,0]`, the reverse representation of dav1d
   `src/cdf.c:787-797` `CDF1(7621)`. It decodes zero and updates to
   `[23576,1]`.
2. The two equiprobable EOB-extra bits are zero and one, so
   `((0 | 2) << 2) | 1` is exactly nine.
3. `scan_4x4[9] == 3` selects the final coefficient. EOB-base context three
   begins at `[7842,3820,0]`, returns base symbol two, and updates to
   `[9399,5629,1]`. High-token context fourteen returns token six through
   adaptive symbols three and zero, updating from
   `[20632,14719,11342,0]` through `[21011,15283,12011,1]` to
   `[20355,14806,11636,2]`.
4. The reverse loop visits `scan_4x4[8] == 6`,
   `scan_4x4[7] == 9`, `scan_4x4[6] == 12`,
   `scan_4x4[5] == 8`, and `scan_4x4[4] == 5`. All five use
   base-token context six and return zero, updating its CDF from
   `[12101,2222,839,0]` to `[10327,1897,718,5]`.
5. `scan_4x4[3] == 2` uses base-token context eight and returns symbol three.
   High-token context seventeen returns token six through adaptive symbols
   three and zero, ending at `[23887,18189,14056,2]`.
6. `scan_4x4[2] == 1` uses base-token context four and returns symbol three.
   High-token context ten returns token six through adaptive symbols three
   and zero, ending at `[23132,17066,12732,2]`.
7. `scan_4x4[1] == 4` uses base-token context one and returns zero. The one
   neighboring direct magnitude selects DC high-token context three.
8. DC base-token context zero returns symbol three. Four adaptive high-token
   symbols return token fifteen. DC sign is positive, then Golomb bits zero,
   one, and zero encode extension one, producing final DC token sixteen.
9. The nonzero link chain reads AC signs in raster order 1, 2, and 3. All
   three are negative. The first transform's final raster coefficient vector
   is `[64,-24,-24,-24,0,0,0,0,0,0,0,0,0,0,0,0]` after
   Q-index-zero luma dequantization by four.

The second transform repeats EOB nine and the same AC scan/token/sign class
after all adaptive CDF updates from the first transform. Its DC high-token
context three returns direct token eight, so it has no Golomb extension. Its
final coefficient vector is
`[32,-24,-24,-24,0,0,0,0,0,0,0,0,0,0,0,0]`. This second
occurrence again proves that production must use context-indexed adaptive
tables rather than fixture-specific post states or transform ordinals.

### Independent inverse-transform and reconstruction check

Applying dav1d `src/itx_1d.c:1066-1080` and
`src/itx_tmpl.c:184-203` independently to the first EOB-9 vector gives:

```text
0 0 0 0
5 5 5 5
5 5 5 5
5 5 5 5
```

The first transform's DC predictor is 81, so its first reconstructed row is
81 and its remaining three rows are 86. The second EOB-9 vector produces
residual rows of negative two followed by three rows of positive three; with
predictor 83, it reconstructs to the same 81/86 row pattern. The third
transform is the existing DC-only vector
`[32,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]` with predictor 84 and
reconstructs entirely to 86. The fourth transform is skipped and propagates
predictor 86.

The complete bottom-right 8x8 luma leaf therefore has one row of 81 followed
by seven rows of 86. Clipping it to the declared 12x12 frame changes only
visible coordinates from `(8,9)` through the lower-right corner, matching
every pinned dav1d row and the target Y hash.

### Focused next rejection control

The 36-origin sweep identifies `(10,9)` as the closest same-topology control
with the next EOB value. It preserves all five partition nodes, the
vertical/DC/DC/DC luma modes, and skipped bottom-right chroma, while its
bottom-right luma transforms have EOB values 10, 9, 1, and skipped.

The control is a deterministic 326-byte AVIF with SHA-256
`2cb6cfd94fb6cfaf62375d0c7c9dd51b9193d4b3740b31a62750b14ddc39e072`.
Its 51-byte AV1 item has SHA-256
`a223fe1af18bb44cdc2161e2f590f6fec45721f10af076827d7f92ffdb89e361`,
and pinned dav1d consumes 301 entropy operations. Its Y/U/V SHA-256 values
are
`9703bae4c02ef874cce9dfe174437a520f97f291d37017c88c94f3a70791719d`,
`97981aad65721ea7dfb43cfd031404db089113459940f68f0a9109f1cc8d73d2`,
and
`ea6aeb0009d508f51b98a099abb01981af1694d0060b9e7821bebc23d8d91cf9`.
Pillow's RGB SHA-256 is
`edb3552022d80b01938371e9e0d78ea4544d2b1bab41cfe67253a89458774264`.

Its first new syntax shares EOB-bin symbol four and the zero EOB-high context
two value with the target. The first equiprobable EOB-extra bit is one,
whereas EOB nine requires zero; the second bit is zero, producing EOB ten.
Slice 25 must reject at that first extra bit before consuming the second bit
or any EOB-10 coefficient syntax. This is a narrower boundary than the
nearby EOB-12 and EOB-15 candidates.

### Implementation boundary

Extend only the private lossless 4x4 two-dimensional luma coefficient
decoder:

1. add exact q-context-zero EOB-high context two and admit EOB-bin symbol four
   only when high is zero and the two extra bits are zero then one;
2. decode EOB nine through scan positions 3, 6, 9, 12, 8, 5, 2, 1, and 4,
   the proved base/high contexts, coefficient link order, DC-context
   derivation, token-fifteen Golomb extension, signs, and dequantization;
3. preserve both independently traced adaptive occurrences and the existing
   EOB-1, EOB-2, EOB-4, and EOB-6 entropy order; and
4. leave EOB ten, every other symbol-four high/extra combination, every
   unproved token or context, other plane, transform type, or transform size
   rejected before its downstream syntax.

The production path must be selected only by parsed AV1 syntax. It must not
inspect a fixture name, file hash, dimensions, transform ordinal, encoded
byte offset, target, or expected output. Do not broaden this slice to the
EOB-10 control or any untraced coefficient class.

### Acceptance criteria

- The existing EOB-9 manifest fixture becomes the ninety-sixth independent
  reconstruction positive, matching all 289 entropy operations, every
  adaptive CDF state, exact Y/U/V bytes and hashes, and exact Pillow RGB
  bytes.
- A regenerated `(10,9)` EOB-10 manifest success fixture remains a private
  portable miss at the first EOB-extra bit before its second bit or
  coefficient syntax.
- The EOB-10 bytes are regenerated by the existing deterministic fixture
  script and retain the pinned file, AV1 item, plane, and Pillow hashes.
- EOB nine's accepted high/extra values, the rejected first EOB-10 extra bit,
  all proved coefficient branches, and the symbol-four rejection boundary
  are exercised through manifest-derived reverse-mapped inputs.
- Every previous reconstruction case and all manifest rows remain
  byte-exact, with no planned, skipped, or unwired row.
- There is no new dependency, unsafe Rust, public image-processing API,
  target fork, native-only behavior, or fixture-selected production path.
- Strict Clippy passes for no features, each codec feature, defaults, and all
  features on native and `wasm32-unknown-unknown`; strict rustdoc, rustfmt,
  whitespace, third-party legal audit, and offline source-package checks pass.
- Coverage MCP is the only test runner and reports exactly 100% line, branch,
  function, and region coverage after all implementation and documentation
  changes are complete.

### Acceptance result

The private coefficient decoder now implements the proved lossless EOB-9
syntax class. EOB-bin symbol four consumes the exact EOB-high context-two
update and accepts only extra bits zero then one. The coefficient path follows
scan positions 3, 6, 9, 12, 8, 5, 2, 1, and 4, uses the
reference-derived base/high contexts, derives DC high-token context three,
and preserves the raster 1-to-2-to-3 sign chain. The first transform exercises
token fifteen with Golomb extension one; the second exercises direct token
eight after the adaptive CDF updates. No fixture name, hash, dimension,
transform ordinal, byte offset, target, dependency, unsafe Rust, or public
image-processing API selects this path.

The former EOB-9 control is now the ninety-sixth independent reconstruction
positive. The production trace matches all 289 pinned scalar entropy
operations and adaptive CDF states, exact Y/U/V rows and hashes, and exact
Pillow RGB bytes. The deterministic reconstruction oracle has SHA-256
`569c0e194cbcc4a6ff1da22a178cb69a44d1c15fd6fd3601c3e4ebe1de783cc4`.

The new 326-byte
`partitioned_square_12x12_luma_eob10_control.avif` is an active manifest
success with file SHA-256
`2cb6cfd94fb6cfaf62375d0c7c9dd51b9193d4b3740b31a62750b14ddc39e072`
and exact Pillow RGB SHA-256
`edb3552022d80b01938371e9e0d78ea4544d2b1bab41cfe67253a89458774264`.
The private portable classifier rejects it at the first EOB-extra bit before
consuming the second bit or any EOB-10 coefficient syntax.

The manifest contains 1,128 active rows: 850 decode and 278 encode. AVIF has
108 active decode rows and 23 active encode rows, with no planned, skipped,
or unwired rows. Every retained manifest and reconstruction case remains
byte-exact.

Coverage MCP run `27b8b2ab-cb47-4d64-8061-4c3405ee46e6`, snapshot
`f3955aab-ae57-416c-a5ad-0a779f3ba3be`, passes all seven test binaries with
36,626/36,626 lines, 5,356/5,356 branches, 1,848/1,848 functions, and
60,536/60,536 regions.

Strict Clippy passes for no features, every individual codec feature,
defaults, and all features on native and `wasm32-unknown-unknown`. Strict
rustdoc, formatting, whitespace, and the 19-file third-party legal inventory
also pass. Offline `cargo package` verifies 132 publishable files, 2.0 MiB
unpacked and 429,765 bytes compressed.

## Slice 26 Exploration Plan: Lossless EOB 10

Status: accepted.

### Current boundary and reproducibility

`partitioned_square_12x12_luma_eob10_control.avif` is the smallest retained
portable miss after Slice 25. It changes source RGB `(17,91,203)` to
`(22,96,208)` beginning at `(10,9)`. It retains the same five-node square
partition tree, vertical/DC/DC/DC luma predictor sequence, DC-only or skipped
first three leaves, and skipped U and V transforms in the bottom-right leaf.
Its four bottom-right luma transforms have EOB values 10, 9, 1, and skipped.

A fresh deterministic 36-origin sweep over the same 12x12 source family
selected `(10,9)` as the EOB-10 target and `(9,10)` as the nearest EOB-12
control with identical partition topology and predictor sequence. Two
subsequent full runs through the pinned Pillow 12.2.0, libavif 1.4.1, libaom
3.13.2, and scalar instrumented dav1d 1.5.3 stack produced byte-identical
complete reports with SHA-256
`b2a185b04f7f4892aff2e65f5138f1ee78ec96f7f6b568229fb5a93f318db64c`.
Each report retains the encoded file and AV1 item hashes, all five partition
nodes, every adaptive entropy operation and CDF state, coefficient vectors,
reconstructed Y/U/V rows, and Pillow RGB rows.

The EOB-10 target is the committed 326-byte file with SHA-256
`2cb6cfd94fb6cfaf62375d0c7c9dd51b9193d4b3740b31a62750b14ddc39e072`.
Its 51-byte AV1 color item has SHA-256
`a223fe1af18bb44cdc2161e2f590f6fec45721f10af076827d7f92ffdb89e361`,
and the complete scalar decode consumes 301 entropy operations. The five
partition ranges remain 34880, 40768, 50626, 52336, and 54330. Pinned dav1d
produces Y/U/V SHA-256 values
`9703bae4c02ef874cce9dfe174437a520f97f291d37017c88c94f3a70791719d`,
`97981aad65721ea7dfb43cfd031404db089113459940f68f0a9109f1cc8d73d2`,
and
`ea6aeb0009d508f51b98a099abb01981af1694d0060b9e7821bebc23d8d91cf9`.
Pillow's RGB SHA-256 is
`edb3552022d80b01938371e9e0d78ea4544d2b1bab41cfe67253a89458774264`.

### First-divergence entropy mapping

The first unsupported value is the first EOB-extra bit after dav1d
`src/recon_tmpl.c:403-546` EOB-bin symbol four:

1. `eob_bin_16[0][0]` returns symbol four. EOB-high context two begins at
   `[25147,0]`, the reverse representation of dav1d
   `src/cdf.c:787-797` `CDF1(7621)`. It decodes zero and updates to
   `[23576,1]`.
2. The two equiprobable EOB-extra bits are one and zero, so
   `((0 | 2) << 2) | 2` is exactly ten.
3. `scan_4x4[10] == 7` selects the final coefficient. EOB-base context three
   returns base symbol two. High-token context fourteen returns direct token
   three.
4. The reverse loop visits `scan_4x4[9] == 3` and
   `scan_4x4[8] == 6`. Both use base-token context eight and high-token
   context sixteen, returning direct token three after their respective
   adaptive updates.
5. `scan_4x4[7] == 9`, `scan_4x4[6] == 12`, and
   `scan_4x4[5] == 8` use base-token context six and return zero.
6. `scan_4x4[4] == 5` introduces base-token context nine. Its initial CDF is
   `[27513,19929,14136,0]`, the reverse representation of dav1d
   `src/cdf.c:881-905` `CDF3(5255,12839,18632)`. It returns symbol three and
   updates to `[27677,20330,14718,1]`; high-token context nine then returns
   direct token three.
7. `scan_4x4[3] == 2` introduces base-token context ten. Its initial CDF is
   `[29948,25562,21607,0]`, the reverse representation of
   `CDF3(2820,7206,11161)`. It returns symbol three and updates to
   `[30036,25787,21955,1]`. Its neighborhood introduces high-token context
   nineteen, whose initial CDF is `[27431,22870,19008,0]`, the reverse
   representation of dav1d `src/cdf.c:1316-1340`
   `CDF3(5337,9898,13760)`. It returns direct token three and updates to
   `[26574,22156,18414,1]`.
8. `scan_4x4[2] == 1` uses base-token context five and high-token context
   twelve, returning direct token three.
9. `scan_4x4[1] == 4` uses base-token context four. Its reused high-token
   context nine first returns adaptive symbol three and then symbol two,
   producing direct token eight after the earlier raster-five update.
10. The neighboring levels select DC high-token context six. DC base-token
    context zero returns symbol three and DC high-token context six returns
    direct token eight. The DC sign is positive; no Golomb extension is read
    for any direct token.
11. The nonzero link chain reads AC signs in raster order 4, 1, 2, 5, 6, 3,
    and 7. Their signs are negative, negative, negative, positive, positive,
    negative, and positive. The final raster coefficient vector is
    `[32,-12,-12,-12,-32,12,12,12,0,0,0,0,0,0,0,0]` after
    Q-index-zero luma dequantization by four.

The second transform is the independently proved EOB-9 class with coefficient
vector `[32,-24,-24,-24,0,0,0,0,0,0,0,0,0,0,0,0]`. The third
is the independently proved EOB-1 class with coefficient vector
`[24,0,0,0,-40,0,0,0,0,0,0,0,0,0,0,0]`. This sequence proves
that EOB-10 must leave every shared adaptive table in exactly the state
expected by the existing EOB-9 and EOB-1 decoders.

### Independent inverse-transform and reconstruction check

Applying dav1d `src/itx_1d.c:1066-1080` and
`src/itx_tmpl.c:184-203` independently to the EOB-10 vector gives:

```text
0 0 0 0
0 0 5 5
0 0 5 5
0 0 5 5
```

The first transform's DC predictor is 81, so it reconstructs to one row of
`[81,81,81,81]` followed by three rows of `[81,81,86,86]`. The
second EOB-9 transform has predictor 83 and reconstructs to one row of 81
followed by three rows of 86. The third EOB-1 transform has predictor 82 and
reconstructs every row to `[81,81,86,86]`. The fourth transform is skipped
and propagates predictor 86.

The complete bottom-right 8x8 luma leaf therefore has a first row entirely
equal to 81 and every later row equal to
`[81,81,86,86,86,86,86,86]`. Clipping it to the declared 12x12 frame
changes visible coordinates from `(10,9)` through the lower-right corner,
matching every pinned dav1d row and the target Y hash.

### Focused next rejection control

The same 36-origin sweep identifies `(9,10)` as the closest same-topology
control whose first transform uses the next unsupported EOB-high value. Its
bottom-right luma transforms have EOB values 12, 2, 6, and skipped.

The control is a deterministic 326-byte AVIF with SHA-256
`52293589be5deed92756c5e11447b571686381ec3c873dbb9e5a221b91eb820c`.
Its 51-byte AV1 item has SHA-256
`a62aca255bdc1ebdb8b2aca233bf82fc975c3f846bc9244df943911d8ba18e14`,
and pinned dav1d consumes 293 entropy operations. Its Y/U/V SHA-256 values
are
`6a0bfe91c043e35738f5d6ac96e0c23ff865f37c29978c03caf937c39e78998c`,
`97981aad65721ea7dfb43cfd031404db089113459940f68f0a9109f1cc8d73d2`,
and
`ea6aeb0009d508f51b98a099abb01981af1694d0060b9e7821bebc23d8d91cf9`.
Pillow's RGB SHA-256 is
`a98fa8dc8ff3ed903815016c02089c888bee48bfb8774903c8bf70d57aed2735`.

Its first new syntax shares EOB-bin symbol four but EOB-high context two
returns one, updating from `[25147,0]` to `[25623,1]`. Its two extra bits
are zero and zero, producing EOB twelve. Slice 26 must reject at the high bit
before consuming either extra bit or any EOB-12 coefficient syntax.

### Implementation boundary

Extend only the private lossless 4x4 two-dimensional luma coefficient
decoder:

1. retain EOB-bin symbol four and EOB-high context two, accept only high zero
   with extra bits one then zero for EOB ten, and reject high one before
   either extra bit;
2. decode EOB ten through scan positions 7, 3, 6, 9, 12, 8, 5, 2, 1, and
   4 using the exact traced base/high contexts, direct tokens, coefficient
   link order, DC-context derivation, signs, and dequantization;
3. add q-context-zero base-token contexts nine and ten and aligned high-token
   contexts eighteen and nineteen, while exercising only the traced contexts
   nine, ten, and nineteen;
4. preserve the following EOB-9 and EOB-1 adaptive entropy states exactly;
   and
5. leave EOB twelve, every other symbol-four high/extra combination, every
   unproved token or context, other plane, transform type, or transform size
   rejected before its downstream syntax.

The production path must be selected only by parsed AV1 syntax. It must not
inspect a fixture name, file hash, dimensions, transform ordinal, encoded
byte offset, target, or expected output. Do not broaden this slice to the
EOB-12 control or any untraced coefficient class.

### Acceptance criteria

- The existing EOB-10 manifest fixture becomes the ninety-seventh independent
  reconstruction positive, matching all 301 entropy operations, every
  adaptive CDF state, exact Y/U/V bytes and hashes, and exact Pillow RGB
  bytes.
- A regenerated `(9,10)` EOB-12 manifest success fixture remains a private
  portable miss at EOB-high context two before either extra bit or any
  coefficient syntax.
- The EOB-12 bytes are regenerated by the existing deterministic fixture
  script and retain the pinned file, AV1 item, plane, and Pillow hashes.
- EOB ten's accepted high/extra values, the rejected EOB-12 high value, every
  new base/high context, direct-token branch, sign, and downstream adaptive
  EOB-9/EOB-1 occurrence are exercised through manifest-derived
  reverse-mapped inputs.
- Every previous reconstruction case and all manifest rows remain
  byte-exact, with no planned, skipped, or unwired row.
- There is no new dependency, unsafe Rust, public image-processing API,
  target fork, native-only behavior, or fixture-selected production path.
- Strict Clippy passes for no features, each codec feature, defaults, and all
  features on native and `wasm32-unknown-unknown`; strict rustdoc, rustfmt,
  whitespace, third-party legal audit, and offline source-package checks pass.
- Coverage MCP is the only test runner and reports exactly 100% line, branch,
  function, and region coverage after all implementation and documentation
  changes are complete.

### Acceptance result

The private coefficient decoder now implements the proved lossless EOB-10
syntax class. EOB-bin symbol four shares EOB-high context two with EOB nine
and accepts extra bits one then zero for EOB ten. The coefficient path follows
scan positions 7, 3, 6, 9, 12, 8, 5, 2, 1, and 4, uses the exact traced
base/high contexts, derives DC high-token context six, and preserves the
raster 4-to-1-to-2-to-5-to-6-to-3-to-7 sign chain. Every magnitude is a
direct token, including the reused high-context-nine token eight; no Golomb
extension is consumed. No fixture name, hash, dimension, transform ordinal,
byte offset, target, dependency, unsafe Rust, or public image-processing API
selects this path.

The former EOB-10 control is now the ninety-seventh independent reconstruction
positive. The production trace matches all 301 pinned scalar entropy
operations and adaptive CDF states, exact Y/U/V rows and hashes, and exact
Pillow RGB bytes. The deterministic reconstruction oracle has SHA-256
`d90c4ceb873c94cdc43caaa6e9acfa1abae6a8cf6a29ee91e264e1c42fed987d`.

The new 326-byte
`partitioned_square_12x12_luma_eob12_control.avif` is an active manifest
success with file SHA-256
`52293589be5deed92756c5e11447b571686381ec3c873dbb9e5a221b91eb820c`
and exact Pillow RGB SHA-256
`a98fa8dc8ff3ed903815016c02089c888bee48bfb8774903c8bf70d57aed2735`.
The private portable classifier rejects it when EOB-high context two returns
one, before consuming either EOB-extra bit or any EOB-12 coefficient syntax.

The manifest contains 1,129 active rows: 851 decode and 278 encode. AVIF has
109 active decode rows and 23 active encode rows, with no planned, skipped,
or unwired rows. Every retained manifest and reconstruction case remains
byte-exact.

Coverage MCP run `548d675d-a850-4324-a957-8358b1372935`, snapshot
`bfe299d5-86fd-4c69-8e47-05e52d89a694`, passes all seven test binaries with
36,700/36,700 lines, 5,358/5,358 branches, 1,850/1,850 functions, and
60,734/60,734 regions.

Strict Clippy passes for no features, every individual codec feature,
defaults, and all features on native and `wasm32-unknown-unknown`. Strict
rustdoc, formatting, whitespace, and the 19-file third-party legal inventory
also pass. Offline `cargo package` verifies 132 publishable files, 2.0 MiB
unpacked and 430,191 bytes compressed, with crate SHA-256
`5c0e496f30bc894986afc653b77821316ac2b44a0bde3250836644bc060c75c8`.

## Slice 27 Exploration Plan: Lossless EOB 12

Status: accepted.

### Current boundary and reproducibility

`partitioned_square_12x12_luma_eob12_control.avif` is the smallest retained
portable miss after Slice 26. It changes source RGB `(17,91,203)` to
`(22,96,208)` beginning at `(9,10)`. It retains the same five-node square
partition tree, vertical/DC/DC/DC luma predictor sequence, DC-only or skipped
first three leaves, and skipped U and V transforms in the bottom-right leaf.
Its four bottom-right luma transforms have EOB values 12, 2, 6, and skipped.

The existing deterministic 36-origin sweep selected `(9,10)` as the EOB-12
target and `(9,9)` as the nearest same-topology EOB-15 control. Two fresh full
runs through the pinned Pillow 12.2.0, libavif 1.4.1, libaom 3.13.2, and
scalar instrumented dav1d 1.5.3 stack produced byte-identical complete reports
with SHA-256
`e09f1bd24d96e6d7548d52d80e9930b5634ba2d68c17c41428a466ac5a9b8b4b`.
Each report retains the encoded file and AV1 item hashes, all five partition
nodes, every adaptive entropy operation and CDF state, coefficient vectors,
reconstructed Y/U/V rows, and Pillow RGB rows.

The EOB-12 target is the committed 326-byte file with SHA-256
`52293589be5deed92756c5e11447b571686381ec3c873dbb9e5a221b91eb820c`.
Its 51-byte AV1 color item has SHA-256
`a62aca255bdc1ebdb8b2aca233bf82fc975c3f846bc9244df943911d8ba18e14`,
and the complete scalar decode consumes 293 entropy operations. The five
partition ranges remain 34880, 40768, 50626, 52336, and 54330. Pinned dav1d
produces Y/U/V SHA-256 values
`6a0bfe91c043e35738f5d6ac96e0c23ff865f37c29978c03caf937c39e78998c`,
`97981aad65721ea7dfb43cfd031404db089113459940f68f0a9109f1cc8d73d2`,
and
`ea6aeb0009d508f51b98a099abb01981af1694d0060b9e7821bebc23d8d91cf9`.
Pillow's RGB SHA-256 is
`a98fa8dc8ff3ed903815016c02089c888bee48bfb8774903c8bf70d57aed2735`.

### First-divergence entropy mapping

The first unsupported value is EOB-high context two returning one in dav1d
`src/recon_tmpl.c:403-546`:

1. `eob_bin_16[0][0]` returns symbol four. EOB-high context two begins at
   `[25147,0]`, the reverse representation of dav1d
   `src/cdf.c:787-797` `CDF1(7621)`. It decodes one and updates to
   `[25623,1]`.
2. The two equiprobable EOB-extra bits are zero and zero, so
   `((1 | 2) << 2) | 0` is exactly twelve.
3. `scan_4x4[12] == 13` selects the final coefficient. EOB-base context three
   returns symbol one, producing direct base token two without a high-token
   read.
4. The reverse loop visits `scan_4x4[11] == 10` and
   `scan_4x4[10] == 7`. Both introduce base-token context twenty-one. Its
   initial CDF is `[17032,5215,2164,0]`, the reverse representation of dav1d
   `src/cdf.c:881-905` `CDF3(15736,27553,30604)`. Both symbols are zero,
   updating it through `[16500,5053,2097,1]` to
   `[15985,4896,2032,2]`.
5. `scan_4x4[9] == 3` and `scan_4x4[8] == 6` use base-token context six
   and return zero.
6. `scan_4x4[7] == 9` uses base-token context seven and returns symbol two,
   producing direct base token two. `scan_4x4[6] == 12` reuses the adaptively
   updated context seven, returns symbol three, and selects high-token context
   fifteen, which returns direct token three.
7. `scan_4x4[5] == 8` uses base-token context ten and introduces high-token
   context eighteen. Its initial CDF is `[26245,20989,16768,0]`, the reverse
   representation of dav1d `src/cdf.c:1316-1340`
   `CDF3(6523,11779,16000)`. It returns direct token three and updates to
   `[25425,20334,16244,1]`.
8. `scan_4x4[4] == 5` uses base-token context eight and returns symbol two,
   producing direct base token two. `scan_4x4[3] == 2` uses base-token
   context six and returns zero.
9. `scan_4x4[2] == 1` uses base-token context three. High-token context eight
   returns token seven through adaptive symbols three and one.
10. `scan_4x4[1] == 4` uses base-token context five. High-token context
    eleven returns direct token three.
11. The neighboring levels select DC high-token context six. DC base-token
    context zero returns symbol three and DC high-token context six returns
    direct token eight. The DC sign is positive; no Golomb extension is read.
12. The nonzero link chain reads AC signs in raster order 4, 1, 5, 8, 12, 9,
    and 13. Their signs are negative, negative, positive, negative, negative,
    positive, and positive. The final raster coefficient vector is
    `[32,-28,0,0,-12,8,0,0,-12,8,0,0,-12,8,0,0]` after
    Q-index-zero luma dequantization by four.

The second transform is the independently proved EOB-2 class with coefficient
vector `[24,-40,0,0,0,0,0,0,0,0,0,0,0,0,0,0]`. The third is the
independently proved EOB-6 class with coefficient vector
`[28,0,0,0,-20,0,0,0,-20,0,0,0,-20,0,0,0]`. This sequence proves
that EOB-12 must leave every shared adaptive table in exactly the state
expected by the existing EOB-2 and EOB-6 decoders.

### Independent inverse-transform and reconstruction check

Applying dav1d `src/itx_1d.c:1066-1080` and
`src/itx_tmpl.c:184-203` independently to the EOB-12 vector gives:

```text
0 0 0 0
0 0 0 0
0 5 5 5
0 5 5 5
```

The first transform's DC predictor is 81, so it reconstructs to two rows of
81 followed by two rows of `[81,86,86,86]`. The second EOB-2 transform has
predictor 82 and reconstructs to two rows of 81 followed by two rows of 86.
The third EOB-6 transform has predictor 83 and reconstructs every row to
`[81,86,86,86]`. The fourth transform is skipped and propagates predictor
86.

The complete bottom-right 8x8 luma leaf therefore has its first two rows
entirely equal to 81 and every later row equal to
`[81,86,86,86,86,86,86,86]`. Clipping it to the declared 12x12 frame
changes visible coordinates from `(9,10)` through the lower-right corner,
matching every pinned dav1d row and the target Y hash.

### Focused next rejection control

The same 36-origin sweep identifies `(9,9)` as the closest same-topology
control whose first transform uses the next EOB value. Its bottom-right luma
transforms have EOB values 15, 9, 6, and skipped.

The control is a deterministic 329-byte AVIF with SHA-256
`3265cf40613523eab69cba5ae73af453f781a29ab3b36f13c21b6720a4d42d7a`.
Its 54-byte AV1 item has SHA-256
`51e80d7551ce4d8882e851ae7c9f454a1579152fe87e0c7f089e7b795eed6442`,
and pinned dav1d consumes 325 entropy operations. Its Y/U/V SHA-256 values
are
`3537a0e7b58ee08b95158e8246e3943d29f9072f41ae0ffb2e9bc5f3417463a3`,
`97981aad65721ea7dfb43cfd031404db089113459940f68f0a9109f1cc8d73d2`,
and
`ea6aeb0009d508f51b98a099abb01981af1694d0060b9e7821bebc23d8d91cf9`.
Pillow's RGB SHA-256 is
`2d41c17b74e78417fd7ab3fdb5da3225f52c4035e39133275ee01496cc21a77a`.

Its first new syntax shares EOB-bin symbol four and EOB-high context two
returning one. Its first equiprobable EOB-extra bit is one, whereas EOB twelve
requires zero; the second bit is also one, producing EOB fifteen. Slice 27
must reject at the first extra bit before consuming the second bit or any
EOB-15 coefficient syntax.

### Implementation boundary

Extend only the private lossless 4x4 two-dimensional luma coefficient
decoder:

1. retain EOB-bin symbol four and EOB-high context two, accept high one only
   when both extra bits are zero for EOB twelve, and reject the EOB-15 first
   extra bit before reading its second bit;
2. decode EOB twelve through scan positions 13, 10, 7, 3, 6, 9, 12, 8, 5,
   2, 1, and 4 using the exact traced base/high contexts, direct base and high
   tokens, coefficient link order, DC-context derivation, signs, and
   dequantization;
3. add aligned q-context-zero base-token contexts eleven through twenty-one,
   while exercising only the newly traced context twenty-one;
4. preserve the following EOB-2 and EOB-6 adaptive entropy states exactly;
   and
5. leave EOB fifteen, every other symbol-four high/extra combination, every
   unproved token or context, other plane, transform type, or transform size
   rejected before its downstream syntax.

The production path must be selected only by parsed AV1 syntax. It must not
inspect a fixture name, file hash, dimensions, transform ordinal, encoded
byte offset, target, or expected output. Do not broaden this slice to the
EOB-15 control or any untraced coefficient class.

### Acceptance criteria

- The existing EOB-12 manifest fixture becomes the ninety-eighth independent
  reconstruction positive, matching all 293 entropy operations, every
  adaptive CDF state, exact Y/U/V bytes and hashes, and exact Pillow RGB
  bytes.
- A regenerated `(9,9)` EOB-15 manifest success fixture remains a private
  portable miss at the first EOB-extra bit before its second bit or
  coefficient syntax.
- The EOB-15 bytes are regenerated by the existing deterministic fixture
  script and retain the pinned file, AV1 item, plane, and Pillow hashes.
- EOB twelve's accepted high/extra values, the rejected EOB-15 first extra
  bit, context twenty-one, every direct-token branch and sign, and downstream
  adaptive EOB-2/EOB-6 occurrence are exercised through manifest-derived
  reverse-mapped inputs.
- Every previous reconstruction case and all manifest rows remain
  byte-exact, with no planned, skipped, or unwired row.
- There is no new dependency, unsafe Rust, public image-processing API,
  target fork, native-only behavior, or fixture-selected production path.
- Strict Clippy passes for no features, each codec feature, defaults, and all
  features on native and `wasm32-unknown-unknown`; strict rustdoc, rustfmt,
  whitespace, third-party legal audit, and offline source-package checks pass.
- Coverage MCP is the only test runner and reports exactly 100% line, branch,
  function, and region coverage after all implementation and documentation
  changes are complete.

### Acceptance result

The private coefficient decoder now implements the proved lossless EOB-12
syntax class. EOB-bin symbol four shares EOB-high context two with EOB nine
and ten, accepts extra bits zero then zero for EOB twelve, and rejects the
EOB-15 control at its first extra bit of one. The coefficient path follows
scan positions 13, 10, 7, 3, 6, 9, 12, 8, 5, 2, 1, and 4, including the
newly proved base-token context twenty-one and high-token context eighteen.
Its exact dequantized coefficients are
`[32,-28,0,0,-12,8,0,0,-12,8,0,0,-12,8,0,0]`.

The committed EOB-12 fixture is now the ninety-eighth independent positive
in the reconstruction oracle. All 293 entropy operations, adaptive CDF
states, coefficient vectors, reconstructed Y/U/V bytes, and Pillow RGB bytes
match exactly. The regenerated reconstruction oracle has SHA-256
`a66198fb36acde59ab02912e79ce735b32280a3546a1b2bc696da9c908c5d235`.
The deterministic EOB-15 control remains a private portable miss at the first
extra bit, before its second extra bit or any coefficient syntax is consumed.

The manifest contains 1,130 active rows: 852 decode and 278 encode. AVIF has
110 active decode rows and 23 active encode rows, with no planned, skipped,
or unwired rows. Every retained manifest and reconstruction case remains
byte-exact.

Coverage MCP run `9bcd31a0-4546-45e0-b03d-29c69d52c667`, snapshot
`7d72d435-cc3e-470b-a93a-eb5ef6108fb6`, passes all seven test binaries with
36,775/36,775 lines, 5,360/5,360 branches, 1,851/1,851 functions, and
60,930/60,930 regions.

Strict Clippy passes for no features, every individual codec feature,
defaults, and all features on native and `wasm32-unknown-unknown`. Strict
rustdoc, formatting, whitespace, and the 19-file third-party legal inventory
also pass. Offline `cargo package` verifies 132 publishable files, 2.0 MiB
unpacked and 430,571 bytes compressed, with crate SHA-256
`620a6d5a784857d9e624522b85e752b79ab581a135e0fbbac8d3be5eeb93c295`.

## Slice 28 Exploration Plan: Lossless EOB 15

Status: accepted.

### Current boundary and reproducibility

`partitioned_square_12x12_luma_eob15_control.avif` is the smallest retained
portable miss after Slice 27. It changes source RGB `(17,91,203)` to
`(22,96,208)` beginning at `(9,9)`. It retains the same five-node square
partition tree, vertical/DC/DC/DC luma predictor sequence, DC-only or skipped
first three leaves, and skipped U and V transforms in the bottom-right leaf.
Its four bottom-right luma transforms have EOB values 15, 9, 6, and skipped.

Two complete runs through the pinned Pillow 12.2.0, libavif 1.4.1, libaom
3.13.2, and scalar instrumented dav1d 1.5.3 stack produced byte-identical
reports with SHA-256
`e09f1bd24d96e6d7548d52d80e9930b5634ba2d68c17c41428a466ac5a9b8b4b`.
Each report contains both the accepted EOB-12 predecessor and the EOB-15
target, retaining encoded files and AV1 items, all five partition nodes, every
adaptive entropy operation and CDF state, coefficient vectors, reconstructed
Y/U/V rows, and Pillow RGB rows.

The EOB-15 target is the committed 329-byte file with SHA-256
`3265cf40613523eab69cba5ae73af453f781a29ab3b36f13c21b6720a4d42d7a`.
Its 54-byte AV1 color item has SHA-256
`51e80d7551ce4d8882e851ae7c9f454a1579152fe87e0c7f089e7b795eed6442`,
and the complete scalar decode consumes 325 entropy operations. The five
partition ranges are 34880, 40768, 50626, 52336, and 54330. Pinned dav1d
produces Y/U/V SHA-256 values
`3537a0e7b58ee08b95158e8246e3943d29f9072f41ae0ffb2e9bc5f3417463a3`,
`97981aad65721ea7dfb43cfd031404db089113459940f68f0a9109f1cc8d73d2`,
and
`ea6aeb0009d508f51b98a099abb01981af1694d0060b9e7821bebc23d8d91cf9`.
Pillow's RGB SHA-256 is
`2d41c17b74e78417fd7ab3fdb5da3225f52c4035e39133275ee01496cc21a77a`.

### First-divergence entropy mapping

The first unsupported value is the first equiprobable EOB-extra bit in dav1d
`src/recon_tmpl.c:403-546`:

1. `eob_bin_16[0][0]` returns symbol four. EOB-high context two returns one
   and updates from `[25147,0]` to `[25623,1]`, exactly as in EOB twelve.
2. EOB fifteen's two equiprobable extra bits are one and one. Slice 27 rejects
   at the first one before consuming the second bit; accepting both gives
   `((1 | 2) << 2) | 3 == 15`.
3. `scan_4x4[15] == 15` selects the final coefficient. EOB-base context three
   returns symbol zero, producing direct base token one.
4. The reverse loop visits raster positions 11, 14, 13, 10, and 7. Positions
   11, 14, 13, and 7 use base-token context twenty-two; position 10 uses
   context twenty-three. All five return symbol one and direct token one.
   Context twenty-two starts at `[21558,8974,3981,0]`, the reverse
   representation of dav1d `src/cdf.c:906`
   `CDF3(11210,23794,28787)`. Context twenty-three starts at
   `[26821,18894,13067,0]`, from `src/cdf.c:907`
   `CDF3(5947,13874,19701)`.
5. Raster position 3 uses base-token context seven, returns symbol three, and
   high-token context fifteen returns direct token four. Raster positions 6
   and 9 use base-token context eight and return direct token one.
6. Raster position 12 reuses context seven and high-token context fifteen,
   producing token four. Raster position 8 uses base-token context nine and
   high-token context seventeen, also producing token four.
7. Raster position 5 uses base-token context nine and returns direct token
   one. Raster position 2 reuses context nine and high-token context
   seventeen, producing token four.
8. Raster positions 1 and 4 use base-token context five and high-token context
   ten, each producing token four.
9. The neighboring levels select DC high-token context five. The stored
   neighboring levels sum to 457; masking to six magnitude bits gives nine,
   and `(9 + 1) >> 1` selects context five. DC base-token
   context zero returns symbol three; high-token context five returns token
   twelve through adaptive symbols three, three, three, and zero. The DC sign
   is positive and no Golomb extension is read.
10. The nonzero link chain reads signs in raster order 4, 1, 2, 5, 8, 12, 9,
    6, 3, 7, 10, 13, 14, 11, and 15. The negative positions are 4, 1, 2, 8,
    12, and 3. The final Q-index-zero dequantized coefficient vector is
    `[48,-16,-16,-16,-16,4,4,4,-16,4,4,4,-16,4,4,4]`.

Every value above is selected by AV1 syntax and shared adaptive state. No
fixture identifier, byte offset, file hash, dimension shortcut, transform
ordinal, target, or expected output participates in the mapping.

### Independent inverse-transform and reconstruction check

Applying dav1d `src/itx_1d.c:1066-1080` and
`src/itx_tmpl.c:184-203` independently to the EOB-15 vector gives:

```text
0 0 0 0
0 5 5 5
0 5 5 5
0 5 5 5
```

The transform's DC predictor is 81, so it reconstructs one row of 81 followed
by three rows of `[81,86,86,86]`. This changes the first visible sample at
global coordinate `(9,9)`, matching every pinned dav1d row and the target Y
hash. The following independently proved EOB-9 and EOB-6 transforms must
consume the exact adaptively updated coefficient tables and retain their
existing vectors, reconstructions, and final plane hashes.

### Focused next rejection control

After EOB fifteen, every EOB value available to a 4x4 two-dimensional
transform has at least one closed luma path. The existing
`partitioned_square_12x12_midpoint_g96_ac.avif` is therefore retained as the
next broader control rather than inventing another EOB value. Its two
independent complete reports are byte-identical with SHA-256
`f122162cb61172c51c3c9e294647f51ee7f194a77d7de8405d36f4cb8e55c17b`.

The control is a 337-byte file with SHA-256
`d10972f944777129121ef100ee66903959138ae946295bb5fe271cef8035b258`.
Its 62-byte AV1 item has SHA-256
`2b5355aa7d702243dcf6e16933fe18241d94d0bddac3cd7f827c0b83c11cbd84`,
and pinned dav1d consumes 349 entropy operations. Its Y/U/V SHA-256 values
are
`f7772f81549eec68ab54ae7799bd8090c9898058f6f3873b127c07fd25e8fb3c`,
`2b90f39a18f397971a7da8a663662751ab60d84196371c2813f1e8b009116fb3`,
and
`d6d3b6aee2d52121d6a0204d1f66171b23c04ef562a2bc4815138ad43986bc8c`.
Pillow's RGB SHA-256 is
`1d316f3236ecba0ebb2e4483622a7dbaa736686fc6ce609a44c3e7c7380a0ff4`.

Its first reference coefficient class outside the current top-left policy
occurs in the fourth luma transform. That transform uses EOB four, reaches
EOB-base context-two symbol two, and then returns direct token three from
high-token context seven instead of the closed token-five EOB-4 class. The
existing top-left `DcOrSkipped` policy does not admit a nonzero fourth
transform, so this slice must continue to reject before admitting that
coefficient body. It must not consume the remaining coefficients, broaden
horizontal or vertical predictor policy, or admit the control's later chroma
EOB-1, EOB-2, and EOB-4 classes. A subsequent slice must trace the exact
Rust/dav1d first divergence through the top-left transform-grid policy before
choosing its implementation boundary.

### Implementation boundary

Extend only the private lossless 4x4 two-dimensional luma coefficient decoder:

1. retain EOB-bin symbol four and EOB-high context two, accept high one with
   extra bits one then one for EOB fifteen, and keep the existing EOB nine,
   ten, and twelve combinations byte-exact;
2. decode EOB fifteen through scan positions 15, 11, 14, 13, 10, 7, 3, 6, 9,
   12, 8, 5, 2, 1, and 4 using the exact traced base/high contexts, tokens,
   coefficient link order, DC-context derivation, signs, and dequantization;
3. add only q-context-zero base-token contexts twenty-two and twenty-three;
4. preserve the following EOB-9 and EOB-6 adaptive entropy states exactly;
   and
5. leave every unproved token/context combination, chroma AC coefficient,
   other transform type or size, and broader predictor sequence rejected
   before its downstream syntax.

The production path must remain target-independent, dependency-free, safe
Rust selected only by parsed AV1 syntax. Do not broaden this slice to the
midpoint-g96 control or any untraced coefficient class.

### Acceptance criteria

- The existing EOB-15 manifest fixture becomes the ninety-ninth independent
  reconstruction positive, matching all 325 entropy operations, every
  adaptive CDF state, exact coefficient vectors, Y/U/V bytes and hashes, and
  exact Pillow RGB bytes.
- The midpoint-g96 manifest success fixture remains a private portable miss
  before its unadmitted fourth-transform coefficient body.
- Contexts twenty-two and twenty-three, every EOB-15 direct-token branch,
  DC-token branch and sign, plus downstream adaptive EOB-9/EOB-6 occurrences,
  are exercised through manifest-derived reverse-mapped inputs.
- Every previous reconstruction case and all 1,130 active manifest rows remain
  byte-exact, with no planned, skipped, or unwired row.
- There is no new dependency, unsafe Rust, public image-processing API,
  target fork, native-only behavior, or fixture-selected production path.
- Strict Clippy passes for no features, each codec feature, defaults, and all
  features on native and `wasm32-unknown-unknown`; strict rustdoc, rustfmt,
  whitespace, third-party legal audit, and offline source-package checks pass.
- Coverage MCP is the only test runner and reports exactly 100% line, branch,
  function, and region coverage after all implementation and documentation
  changes are complete.

### Acceptance result

The private coefficient decoder now implements the proved lossless EOB-15
syntax class. EOB-bin symbol four shares EOB-high context two with EOB nine,
ten, and twelve, and accepts extra bits one then one for EOB fifteen. The
coefficient path visits all fifteen AC positions, introduces only base-token
contexts twenty-two and twenty-three, and produces the exact dequantized
vector
`[48,-16,-16,-16,-16,4,4,4,-16,4,4,4,-16,4,4,4]`.

Runtime first-divergence tracing caught and corrected the DC-context
calculation before acceptance. The three neighboring stored levels sum to
457; masking to six magnitude bits gives nine, so dav1d selects high-token
context five and token twelve. Context six instead decoded token fifteen and
caused the portable path to reject. With context five restored, all 325
entropy operations, adaptive CDF states, coefficient vectors, reconstructed
Y/U/V bytes, and Pillow RGB bytes match exactly.

The committed EOB-15 fixture is now the ninety-ninth independent positive in
the reconstruction oracle. The regenerated oracle has SHA-256
`35c1dec1e800f7350e19a1b6bd8d144cb2188b9d80c22e5554f32fa1facf1619`.
The midpoint-g96 fixture remains a private portable miss before its
unadmitted top-left fourth-transform coefficient body.

The manifest remains at 1,130 active rows: 852 decode and 278 encode. AVIF
has 110 active decode rows and 23 active encode rows, with no planned,
skipped, or unwired rows. Every retained manifest and reconstruction case
remains byte-exact.

Coverage MCP run `f4f90661-c2da-4861-9273-2359f494ea79`, snapshot
`d47edc7e-2e1b-4903-afba-efb9b91ec1f9`, passes all seven test binaries with
36,851/36,851 lines, 5,362/5,362 branches, 1,852/1,852 functions, and
61,145/61,145 regions.

Strict Clippy passes for no features, every individual codec feature,
defaults, and all features on native and `wasm32-unknown-unknown`. Strict
rustdoc, formatting, whitespace, and the 19-file third-party legal inventory
also pass. Offline `cargo package` verifies 132 publishable files, 2.0 MiB
unpacked and 431,000 bytes compressed, with crate SHA-256
`2e8adf26832d966867385b49b13c2c4399dc3f89b0557f63eee6402013e4d076`.

## Slice 29 Exploration Plan: Contextual Top-Left Transform Grid

Status: accepted.

### Exact Rust first divergence

The first unsupported syntax is now proved at the transform-grid boundary,
not inferred from the final pixels or from dav1d alone. A temporary
coverage-only trace was exercised through the existing manifest-backed
`partitioned_square_12x12_midpoint_g96_ac.avif` error case by Coverage MCP
run `c1165619-249d-4c4e-b4f4-8fe246bb5a0a`. That deliberately failing
diagnostic run showed the following Rust decisions for the top-left leaf's
luma 2x2 transform grid:

```text
transform 0: base skip CDF,     skipped=false
transform 1: trailing skip CDF, skipped=true
transform 2: trailing skip CDF, skipped=true
transform 3: base skip CDF,     skipped=false
```

Pinned dav1d makes the same four decisions. Rust then returns `None`
immediately after transform three's `skipped=false` because
`decode_dc_coefficients` requires every transform after the first nonzero
transform to be skipped. It does not consume transform three's EOB bin or
coefficient body. The temporary trace and deliberately inverted assertion
were removed after the run, leaving the committed error test unchanged.

This rules out the range decoder, adaptive skip tables, EOB dispatcher,
inverse transform, predictor, and color conversion as the first cause. The
first divergence is the private top-left coefficient-grid policy.

### Reverse-mapped isolator and neighboring control

The retained 36-origin `(22,96,208)` sweep identified origin `(6,6)` as the
smallest closed isolator. Two new complete runs through pinned Pillow 12.2.0,
libavif 1.4.1, libaom 3.13.2, and scalar instrumented dav1d 1.5.3 generated
byte-identical two-case reports with SHA-256
`e4ca622f4f85b6e4d53473769c1c02cdd28654e1ccaeb9fff5f22b4f0b8755d0`.
The reports retain the encoded AVIF and extracted AV1 hashes, every partition
and arithmetic operation, coefficient vectors, reconstructed Y/U/V rows, and
Pillow RGB rows for origins `(6,6)` and `(7,6)`.

The `(6,6)` isolator is a deterministic 317-byte AVIF with SHA-256
`fbc5e3cec5da21a1c1095ecf82525dac5d6ae60ff4a71b101502392de754cc45`.
Its 42-byte AV1 color item has SHA-256
`b6ba409e1eb6068ae86da05e613231145bd53082f0ec2db8ccba39465df0415a`,
and scalar dav1d consumes 229 entropy operations. The five partition ranges
are 34880, 40768, 47278, 60530, and 38697. Its Y/U/V SHA-256 values are
`43577826d2dd195a7cc19e7824af21d6a9aa7c45068ea83b7c23a142435912fa`,
`97981aad65721ea7dfb43cfd031404db089113459940f68f0a9109f1cc8d73d2`,
and
`ea6aeb0009d508f51b98a099abb01981af1694d0060b9e7821bebc23d8d91cf9`.
Pillow's RGB SHA-256 is
`fcfe3605207a28cd1596ae0cb2b9b4ad1b8b356f7457cd2e60276b8d6530a691`.

This fixture changes only luma after YUV conversion: its U and V plane hashes
are identical to all preceding `(22,96,208)` luma controls. The top-left
leaf's luma transforms are DC-only, skipped, skipped, and EOB four. Its U and
V transforms are DC-only followed by three skips. Every transform in the
other three leaves is skipped. The predictor sequence is
vertical/horizontal/vertical/DC.

That predictor sequence is not incidental. Once the fourth top-left luma
transform reconstructs nonzero residuals, the top-left leaf has nonuniform
right and bottom edge vectors. The skipped top-right horizontal leaf must
repeat the complete right edge by row, and the skipped bottom-left vertical
leaf must repeat the complete bottom edge by column. Treating either edge as
one scalar would decode all arithmetic syntax correctly but reconstruct the
wrong Y plane. The contextual coefficient grid and directional propagation
therefore form one closed reconstruction boundary for this encoder-produced
input.

An additional deterministic 32-color equal-RGB-delta sweep at origin `(6,6)`
confirmed that the behavior cannot be split with the current pinned encoder.
The report has SHA-256
`9f688390b8db6851455739dbc8d785c099aaef9a3706c71d6c5895471ad6fba9`.
The two smallest deltas remain unsplit. All 30 partitioned cases keep the
same top-left EOB-4 grid: 21 use predictor sequence
vertical/horizontal/vertical/DC, while the nine largest deltas replace the
top-right predictor with unproved mode 12. No partitioned case uses DC in
both following edge leaves. This sweep is diagnostic only and is not added
to the repository.

The directional entropy syntax and zero-angle decisions were already proved
for two-leaf horizontal and vertical slices. Slice 29 reuses those exact CDF
paths in a four-leaf tree and adds the required full-edge reconstruction:
the top-right luma rows repeat the top-left right edge, the bottom-left luma
columns repeat the top-left bottom edge, and one-sided DC chroma uses the
rounded average of the available edge. No new directional mode or angle is
admitted.

A temporary stage trace then located the first remaining production
divergence after directional modes were admitted. Coverage MCP run
`1d982d5a-f198-49a1-9922-1ea67e0096cb` completed the top-left and top-right
syntax but rejected during the bottom-left syntax. The earlier arithmetic
divergence occurs inside the top-right coefficient grid: the fixed skipped
decoder selects the base luma skip table for all four transforms, while
dav1d carries the top-left transform residual contexts across the leaf
boundary.

For the top-right leaf, the external left contexts are the top-left right
edge, transform indices one and three. Index one is skipped and stores
`0x40`; index three is the nonzero EOB-4 transform. The first transform of
the second row therefore has one nonzero left neighbor and must select luma
skip context three, not base context one. The same relationship is rotated
for the bottom-left leaf: its external above contexts are top-left indices
two and three, so the second transform of the first row selects context
three. Every other skipped luma transform in those leaves selects context
one, and all chroma transforms select context ten. Choosing the wrong table
can still return a skipped symbol, but it mutates the wrong adaptive CDF and
causes the later bottom-left rejection. The temporary trace is removed once
this boundary state is proved by the manifest fixture.

After carrying those residual edges, Coverage MCP diagnostic run
`616fad03-aaeb-4401-9878-f7a87299d6c4` completed both following skipped
leaves. It recorded exactly two one-neighbor luma decisions:
top-right transform two with external left residual context 148, and
bottom-left transform one with external above residual context 148. All
other luma and chroma skips used their base tables. The next rejection occurs
before bottom-right syntax completes.

The cause is the bottom-right keyframe luma-mode context. This is the first
admitted leaf with two coded predictor neighbors. Its above neighbor is the
horizontal top-right leaf, whose AV1 intra-mode context is two; its left
neighbor is the vertical bottom-left leaf, whose context is one. Dav1d
therefore selects `kfym[2][1]`, while the previous Rust path selects the
origin-boundary table `kfym[0][0]`. Pinned dav1d 1.5.3
`src/cdf.c:639-708` defines cumulative values

```text
9687, 13470, 18506, 19230, 19604, 20147,
20695, 22062, 23219, 27743, 29211, 30907
```

which map to the decoder's inverse CDF representation

```text
23081, 19298, 14262, 13538, 13164, 12621,
12073, 10706, 9549, 5025, 3557, 1861, 0
```

Only DC symbol zero is admitted under this new two-neighbor context. All
other bottom-right modes remain rejected before their angle, chroma, or
coefficient syntax.

Origin `(7,6)` is the adjacent negative control. It is a deterministic
318-byte AVIF with SHA-256
`b8b703ee9e1f2d8200fea338ee85f7ada1b905539bb163712209f60d83af0713`;
its 43-byte AV1 item has SHA-256
`4ce82eb946854cffea4231555baba29283a63a9853362c15924da06fddb5d80d`,
and dav1d consumes 242 entropy operations. Its Y/U/V hashes are
`666b8f17ef98a5dc4101858348ccc8025d706f95009f8e0231f41d6b7943ce63`,
`97981aad65721ea7dfb43cfd031404db089113459940f68f0a9109f1cc8d73d2`,
and
`ea6aeb0009d508f51b98a099abb01981af1694d0060b9e7821bebc23d8d91cf9`;
Pillow's RGB hash is
`16195f9646d15f2857da1864cbffdd3f12a965bbd287ca888b7dde113c2d7ec7`.
It preserves the same topology, predictors, first three luma transforms,
chroma transforms, and later skipped leaves, but its fourth luma transform
uses a different EOB-12 coefficient body.

### Context and coefficient mapping

For q-index-zero 4x4 transforms, dav1d stores each transform's residual
context as the coefficient-token sum capped to 63 in bits zero through five,
plus the DC sign class in bits six and seven. A skipped transform stores
`0x40`. For luma, `dav1d_skip_ctx[min(above & 63,4)][min(left & 63,4)]`
selects the next skip CDF. The closed high-magnitude classes therefore use:

```text
above zero,    left zero:    skip context 1
one nonzero neighbor:        skip context 3
two nonzero neighbors:       skip context 6
```

The equivalent 4:4:4 chroma contexts for this 2x2 grid are 10, 11, and 12.
These are exactly the existing `coefficient_skip`,
`trailing_coefficient_skip`, and `double_neighbor_coefficient_skip` tables.
The first negative DC coefficient in the isolator stores capped magnitude 63.
It therefore selects context three for transforms one and two. Both are
skipped and store `0x40`, so transform three correctly returns to context
one. Its two skipped neighbors also select DC-sign context zero.

The isolator's fourth luma transform then follows the already-proved Slice 23
EOB-4 path byte for byte:

1. EOB-bin symbol three, EOB-high context one value zero, and extra bit zero
   produce EOB four.
2. EOB-base context two returns symbol two; high-token context seven returns
   token five at raster position five.
3. Raster position two uses base context six and returns zero. Raster
   positions one and four use base context three, then high-token context ten,
   and each return token five.
4. DC base context zero returns symbol three and DC high-token context six
   returns token five. DC is positive; raster positions four and one are
   negative, and raster position five is positive.
5. Q-index-zero dequantization produces
   `[32,-32,0,0,-32,32,0,0,0,0,0,0,0,0,0,0]`.

The inverse WHT adds zero to the first two rows and five to the last two
samples of the last two rows. With predictor 81, the reconstructed transform
is:

```text
81 81 81 81
81 81 81 81
81 81 86 86
81 81 86 86
```

The adjacent `(7,6)` control reaches the same fourth-transform
`skipped=false` decision and the same EOB-position base symbol one as Slice
27. Its first different value is later: coefficient nine returns symbol three
from base context seven, while the admitted Slice 27 EOB-12 class requires
symbol two. The original midpoint-g96 control reaches EOB four but returns
token three from high-token context seven; the admitted Slice 23 class
requires token five and must continue to reject there. These two independent
controls separate the grid-policy change from accidental coefficient
broadening.

### Implementation boundary

1. Replace the top-left decoder's fixed “first nonzero, every later transform
   skipped” assumption with a private contextual 2x2 grid walk that derives
   the next skip and DC-sign contexts from decoded residual contexts exactly
   as dav1d does.
2. Select only the proved q-context-zero skip tables for luma contexts one
   and three and 4:4:4 chroma contexts ten, eleven, and twelve. Luma context
   six remains outside this slice and rejects before reading its skip bit, as
   do all other unproved luma contexts.
3. Preserve the existing `DcOrSkipped` policy byte for byte. Add a distinct
   four-leaf top-left policy whose first transform remains DC-only, whose
   later luma transforms may enter an already-proved AC body, and whose
   chroma transforms remain DC-only or skipped.
4. In the two following skipped square leaves, retain only the already-proved
   zero-angle horizontal mode to the right and zero-angle vertical mode
   below. Reconstruct their luma from the complete neighboring right or
   bottom edge instead of collapsing that edge to one sample. Reconstruct
   one-sided DC planes from the rounded eight-sample edge average.
5. Carry each top-left transform's residual magnitude and DC-sign class into
   the adjacent leaf. Derive top-right left contexts from transform indices
   one and three and bottom-left above contexts from indices two and three.
   Select only the already-proved base and one-neighbor skip CDFs; do not
   hard-code transform ordinals or fixture identity in entropy decoding.
6. Derive the bottom-right luma-mode context from both parsed neighbors.
   Preserve origin context `[0][0]` for DC/DC neighbors; admit only the
   proved `[2][1]` table for horizontal-above/vertical-left neighbors. Keep
   the leaf DC-only and preserve its existing two-edge reconstruction.
7. Derive residual magnitude and sign state from parsed coefficients. Do not
   select behavior by fixture name, hash, byte offset, image dimensions,
   encoded color, expected output, or target architecture.
8. Admit only the `(6,6)` EOB-4 fixture. Keep the `(7,6)` EOB-12 body and
   midpoint-g96 EOB-4 token-three body as manifest-backed private portable
   errors at their first unproved coefficient value.

The production path remains safe Rust, dependency-free, target-independent,
and codec-private. It does not add image processing or weaken any public
structured-error contract.

### Acceptance criteria

- Commit both reverse-mapped fixtures through `generate_test_assets.py`:
  `(6,6)` as the one-hundredth reconstruction positive and `(7,6)` as the
  adjacent private portable error control.
- Regenerate the Pillow/dav1d reconstruction oracle and match all 229
  isolator entropy operations, every adaptive CDF state, coefficient vector,
  exact Y/U/V row and hash, and exact Pillow RGB row and hash.
- Prove the `(7,6)` control rejects at EOB-base context-three symbol two and
  midpoint-g96 rejects at high-token context-seven token three; neither may
  consume downstream unproved coefficient or chroma syntax.
- Exercise luma skip contexts one and three, chroma contexts ten and eleven,
  residual-context derivation, the later nonzero branch, horizontal
  right-edge repetition, vertical bottom-edge repetition, one-sided DC
  averaging, and every new structured error branch through manifest-derived
  fixtures. Luma context six remains rejected; chroma context twelve retains
  its existing manifest coverage.
- Preserve every preceding reconstruction result and every active manifest
  input/output/error row exactly, with no planned, skipped, or unwired row.
- Add no dependency, unsafe Rust, target fork, native-only behavior, public
  image-processing API, or fixture-selected production path.
- Use Coverage MCP as the only test runner and restore exactly 100% line,
  branch, function, and region coverage after all implementation and
  documentation changes.
- Pass strict Clippy for no features, every individual codec feature,
  defaults, and all features on native and `wasm32-unknown-unknown`, plus
  strict rustdoc, rustfmt, whitespace, third-party legal, deterministic
  fixture generation, and offline source-package gates.

### Acceptance result

The contextual top-left grid and its closed four-leaf reconstruction boundary
are accepted. The production decoder now derives each 2x2 transform's
residual magnitude and sign state, carries the right and bottom residual
edges into adjacent leaves, selects the two one-neighbor luma skip
occurrences exactly, and reconstructs horizontal and vertical leaves from
their complete pixel edges. The bottom-right DC leaf selects the proved
`kfym[2][1]` context from its horizontal-above and vertical-left neighbors.
No behavior is selected from a fixture name, byte offset, hash, dimensions,
target, or expected output.

The `(6,6)` EOB-4 fixture is the one-hundredth independent reconstruction
positive. It matches all 229 pinned dav1d entropy operations, adaptive CDF
states, coefficient vectors, exact Y/U/V bytes, and exact Pillow RGB bytes.
The regenerated reconstruction oracle has SHA-256
`e7579f9edd61d2e6f9f7da7d12dbce0b452a1ed87cc5084d690a0eaa79c61da7`.
The adjacent `(7,6)` EOB-12 fixture remains a structured private portable
error, as does the midpoint-g96 token-three control.

The manifest has 1,132 active rows: 854 decode and 278 encode. AVIF has 112
active decode rows and 23 active encode rows, with no planned, skipped, or
unwired row. The generated coverage matrix has SHA-256
`c30092c2b3520329442a51f8912a06d2bd09469c4d22b6fe39f2325555f44fe0`.

Final Coverage MCP run `241bed7c-f922-47b6-af7e-ab79c22b81fc`, snapshot
`ce3d7541-b8fb-4a4a-b2b9-e44da8546ffb`, passes all seven test binaries with
37,062/37,062 lines, 5,376/5,376 branches, 1,868/1,868 functions, and
61,420/61,420 regions.

Strict Clippy passes for no features, every isolated codec feature, defaults,
and all features on native and `wasm32-unknown-unknown`. Strict native and
WASM rustdoc, rustfmt, whitespace, and the 19-file third-party legal audit
also pass. Offline source-package verification contains 132 files, is 2.0
MiB unpacked and 432,958 bytes compressed, with crate SHA-256
`70f5f24324da6a90650cc85defc90cc5adaa7bbeddbfaa198be0c68d9644dfba`.
The slice adds no dependency, unsafe Rust, target fork, public image
processing, or native-only default behavior.

## Slice 30 Exploration Plan: Alternate Lossless EOB 12 Body

Status: planned.

### Exact Rust first divergence

The adjacent `(7,6)` fixture is already manifest-backed and shares Slice 29's
accepted partition tree, predictor sequence, contextual top-left grid,
cross-leaf coefficient state, directional edge reconstruction, and
bottom-right mode context. A temporary coverage-only trace and deliberately
failing assertion exercised that exact fixture through Coverage MCP run
`59670fa6-a41a-4d04-b1d1-dc1454b14dce`.

Rust and dav1d both decode EOB twelve, then consume EOB-base context three
symbol one, two zero values from base context twenty-one, and two zero values
from base context six. The first divergence is the next arithmetic symbol:

```text
raster coefficient 9
base context 7
Slice 27 accepted value: 2
this fixture's value:    3
```

Rust returns `None` immediately after value three. It does not consume the
coefficient-nine high token or any later coefficient, DC, sign, predictor, or
reconstruction syntax. The temporary diagnostics and inverted assertion were
removed after the run.

### Reproducible oracle evidence

The two complete scalar reports
`/private/tmp/image-star-slice29-grid-a.json` and
`/private/tmp/image-star-slice29-grid-b.json` were generated independently
through pinned Pillow 12.2.0, libavif 1.4.1, libaom 3.13.2, and instrumented
dav1d 1.5.3. They are byte-identical with SHA-256
`e4ca622f4f85b6e4d53473769c1c02cdd28654e1ccaeb9fff5f22b4f0b8755d0`.

The target is a deterministic 318-byte AVIF with SHA-256
`b8b703ee9e1f2d8200fea338ee85f7ada1b905539bb163712209f60d83af0713`.
Its 43-byte AV1 color item has SHA-256
`4ce82eb946854cffea4231555baba29283a63a9853362c15924da06fddb5d80d`,
and scalar dav1d consumes 242 entropy operations. The five partition ranges
are 34880, 40768, 53060, 33964, and 42488.

Its Y/U/V SHA-256 values are
`666b8f17ef98a5dc4101858348ccc8025d706f95009f8e0231f41d6b7943ce63`,
`97981aad65721ea7dfb43cfd031404db089113459940f68f0a9109f1cc8d73d2`,
and
`ea6aeb0009d508f51b98a099abb01981af1694d0060b9e7821bebc23d8d91cf9`.
Pillow's RGB SHA-256 is
`16195f9646d15f2857da1864cbffdd3f12a965bbd287ca888b7dde113c2d7ec7`.
The accepted Slice 29 `(6,6)` EOB-4 fixture is the adjacent smaller control.

### Coefficient and reconstruction mapping

After the shared prefix, dav1d decodes the alternate EOB-12 body as follows:

1. Raster nine returns base-context-seven symbol three and high-context
   fifteen token three.
2. Raster twelve returns base-context-seven symbol three and the adaptively
   updated high-context-fifteen token three.
3. Raster eight returns direct token two from base context ten; raster five
   returns direct token two from base context nine.
4. Raster two returns zero from base context six. Raster one returns direct
   token two from base context four.
5. Raster four returns base-context-five symbol three and high-context-eleven
   token three.
6. DC returns base-context-zero symbol three. The neighboring stored levels
   for raster one, four, and five are 130, 195, and 130; their sum 455 masks
   to seven, so `(7 + 1) >> 1` selects high-token context four and token three.
7. The DC sign is positive. The nonzero link chain reads raster signs four,
   one, five, eight, twelve, nine, and thirteen; four, one, twelve, and nine
   are negative.

Q-index-zero dequantization produces

```text
[12,-8,0,0,-12,8,0,0,8,-12,0,0,-12,8,0,0]
```

Applying dav1d's 4x4 lossless inverse WHT independently gives residual rows:

```text
0 0 0 0
0 0 0 0
0 0 0 5
0 0 0 5
```

With predictor 81, the last sample of the last two transform rows becomes 86.
The already-accepted horizontal and vertical full-edge propagation then
repeats that value through the visible lower-right region, matching the
recorded Y rows and Pillow RGB bytes.

### Implementation boundary

1. Preserve the shared EOB-12 prefix and Slice 27 coefficient body byte for
   byte.
2. Dispatch only base-context-seven value three to a private alternate body.
   Decode the exact base/high contexts, direct tokens, DC context four, and
   sign chain above.
3. Derive every token, sign, adaptive update, coefficient, and residual
   context from parsed AV1 syntax. Do not select the path from fixture name,
   byte offset, dimensions, file hash, color, target, or expected output.
4. Reuse Slice 29's contextual grid, edge propagation, following-leaf skip
   state, and two-neighbor DC mode context without broadening them.
5. Keep every other EOB-12 value/context combination and the midpoint-g96
   EOB-4 token-three body rejected before its downstream unproved syntax.

The implementation remains private safe Rust, dependency-free,
target-independent, and codec-only.

### Acceptance criteria

- Promote the existing `(7,6)` manifest fixture to the one-hundred-first
  reconstruction positive and regenerate the pinned Pillow/dav1d oracle.
- Match all 242 entropy operations, adaptive CDF states, the exact coefficient
  vector, Y/U/V rows and hashes, and Pillow RGB rows and hash.
- Keep midpoint-g96 and every other retained private portable miss rejected at
  its first unproved syntax value.
- Preserve all 1,132 active manifest rows with no planned, skipped, or unwired
  row and preserve every preceding reconstruction result byte for byte.
- Add no dependency, unsafe Rust, target fork, native-only behavior, public
  image-processing API, or fixture-selected production branch.
- Use Coverage MCP as the only test runner and restore exactly 100% line,
  branch, function, and region coverage.
- Pass strict Clippy for no features, every codec feature, defaults, and all
  features on native and `wasm32-unknown-unknown`, plus strict rustdoc,
  rustfmt, whitespace, legal inventory, deterministic oracle generation, and
  offline source-package verification.
