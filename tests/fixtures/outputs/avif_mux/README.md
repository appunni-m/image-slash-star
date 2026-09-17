# Native still-container evidence

The index records 28 complete files generated twice by pinned Pillow 12.2.0,
libavif 1.4.1 and libaom 3.13.2. Both native observations agree for every case.
The 105 hashed artifacts contain untouched native encoded files, decoded
Pillow pixels and extracted muxer input payloads (314,731 bytes in total).

Sixteen cases are the currently planned successful still-encode matrix rows.
Twelve supplements cover all eight EXIF orientations on an RGBA source,
alpha with ICC/EXIF/XMP, empty metadata, reuse inside an XMP payload, and
reuse crossing the EXIF/XMP boundary. The collector explicitly verifies that
the latter two native files actually reuse the expected data spans.

The complete native container is the expected output. Rust receives only
dimensions, color declarations, transform values, optional metadata and
encoded color/alpha samples with their four-byte configurations. Expected
box serialization, property indices and item offsets are not writer inputs.
Every extracted payload records its source span and hash. The index retains
source image hashes, manifest specifications, native library and generator
identities, and the clean pinned libavif source tree and `write.c` hash.

```sh
.oracle-venv/bin/python scripts/generate_avif_mux_refs.py \
  --libavif-source /path/to/clean/pinned/libavif \
  --output target/oracle-staging/avif-mux/fresh
```

The private Rust writer implements standard still-image boxes, alpha and
metadata references, property sharing, orientation and native media sharing.
A virtual concatenated payload supports reuse across item boundaries without
copying all compressed samples into temporary storage. A counting pass checks
32-bit container bounds and the output policy before allocating the result.
Copies and substring comparisons poll cancellation. Prepared input allocation
and total codec working memory remain outside this output-size guarantee.

All native inputs in this bundle are 8-bit. The writer's 10/12-bit descriptor
handling is source-derived and still needs native encoder witnesses. AV1
compression, metadata preparation, sequence/grid/layered containers and full
public encoding remain unfinished. The prepared-sample contract requires the
compressor to establish dimensions/header agreement and decide whether opaque
alpha can be omitted. The muxer does not parse or repair compressed samples.

Deferred integration tests compare complete files and exact output limits.
Invalid descriptors, very large EXIF offsets, 32-bit size overflow and work
budgets are separate Rust model checks. The writer is compiled in ordinary
builds; a coverage-only input adapter exposes the same helper to the existing
matrix integration test target. Rust behavioral tests and managed coverage
have not run, and none of the 32 planned encoder rows is promoted.
