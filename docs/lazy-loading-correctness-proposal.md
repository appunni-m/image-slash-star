# Correct Lazy Loading Proposal

Status: implemented native correctness contract for the `image-slash-star`
integration in `pillow-rs`. Binding crates compile against the contract;
binding-specific runtime and WASM size work remain separate follow-on slices.

## Decision

`pillow-rs::Image` should use immutable encoded-source snapshots and shared,
once-initialized materialization caches. Opening an image may perform I/O and
header inspection, but must not decode pixels. The first operation that needs
pixels decodes or executes the image exactly once and makes that result
available to every subsequent read and every clone that still shares the same
image node.

This proposal favors deterministic behavior and state correctness over minimum
peak memory. Streaming and encoded-byte eviction may be added later only if
they preserve the invariants below.

## Problems Identified In The Previous Model

The reviewed implementation was lazy only when callers remembered to invoke
`load(&mut self)` explicitly:

- `materialize(&self)` returns an owned `DynamicImage`, so a lazy path or byte
  source is decoded again on every pixel-reading call.
- even a loaded image is cloned in full by `materialize()`.
- `open(path)` reads the complete file for inspection, discards those bytes,
  and reads the path again for verification or decoding.
- inspection and decoding can therefore observe different files if the path is
  replaced or modified after `open()`.
- pipeline execution is also repeated unless `load()` is called explicitly.
- the type does not distinguish immutable source metadata, derived output
  properties, and encoded-container provenance clearly enough.

Passing explicit-load tests is insufficient: ordinary pixel access must have
persistent lazy-load semantics without requiring callers to know an internal
optimization rule.

## Implementation Evidence

The accepted implementation follows this contract at both ownership layers:

- `image-slash-star::EncodedImage` owns an `Arc<[u8]>` snapshot, inspected
  `ImageInfo`, and a shared `OnceLock` that publishes either one decoded value
  or one stable structured error;
- `EncodedImage::verify` independently validates the same snapshot without
  populating the ordinary decode cache;
- clones and concurrent readers receive the same cached decoded allocation;
- `pillow-rs::Image::{Path,Bytes}` retain the canonical `EncodedImage` instead
  of reimplementing detection or rereading a path;
- lazy sources and immutable pipeline nodes share success-and-failure caches,
  while internal read paths borrow the cached `Arc<DynamicImage>`;
- `load()` reuses the cache and preserves source format, exact mode, metadata,
  palette indices, palette RGB, and palette alpha;
- mutations detach from shared materialized storage and the Pillow-oracle
  manifest proves clone isolation for direct and indexed/pipeline-produced
  images;
- reduced-feature manifests prove exact `FeatureDisabled` behavior at the
  backend and through downstream feature forwarding.

The backend acceptance run is Coverage MCP run
`dbd8a9cf-1e51-49aa-94b5-476235da7f3a`, immutable snapshot
`bc41e67e-4be2-4eac-9444-abe318a0a151`: 28,702/28,702 lines,
3,824/3,824 branches, 1,751/1,751 functions, and 45,458/45,458 regions.
The downstream integration is deliberately validated with its maintained
Makefile targets rather than Coverage MCP.

## Required Invariants

The implementation is acceptable only when all of these are enforced by code
and fixture-driven integration tests.

1. **Stable source bytes:** inspection, verification, and decoding for one
   opened image observe the same encoded bytes.
2. **No pixel decode during open:** `open` and `open_bytes` may read/copy encoded
   bytes and inspect headers, but may not decompress pixel payloads.
3. **At-most-once ordinary materialization:** excluding an explicit
   non-mutating `verify()`, successful decode or pipeline execution happens at
   most once for a shared image node.
4. **Persistent implicit loading:** the first pixel-dependent call populates
   the shared cache even when its public receiver is `&self`.
5. **Stable failures:** deterministic decode and pipeline failures are cached
   and returned consistently; a malformed immutable source is not retried on
   every access.
6. **Atomic publication:** no caller observes partially decoded pixels,
   partially applied operations, or incomplete palette state.
7. **Clone consistency:** cloning a lazy image shares its encoded source and
   cache. Decoding either clone makes the same immutable result available to
   the other.
8. **Copy-on-write mutation:** mutation detaches shared materialized storage
   before changing it. Mutating one image cannot change another image clone.
9. **Exact retained semantics:** source format, exact `ImageMode`, palette RGB,
   palette alpha, dimensions, and other retained metadata remain correct before
   and after materialization.
10. **Feature determinism:** disabled codecs return the same structured
    `FeatureDisabled` error whether failure occurs during inspection, explicit
    load, implicit load, or verification.
11. **No downstream codec decisions:** signature detection, header parsing,
    decoding, encoding, and canonical format parsing remain exclusively in
    `image-slash-star`.
12. **No hidden full-buffer copies on reads:** read-only access borrows or shares
    cached pixel storage. An owned copy is created only for an API that promises
    ownership or for copy-on-write mutation.

## Source Snapshot

Both path and in-memory inputs should become the same internal source type:

```rust,ignore
struct EncodedSource {
    bytes: Arc<[u8]>,
    info: ImageInfo,
    decoded: OnceLock<Result<Arc<MaterializedImage>, DecodeFailure>>,
}
```

`open(path)` should:

1. read the path once into `Arc<[u8]>`;
2. call `image_slash_star::inspect` on that exact slice;
3. validate any explicit format hint against the detected format;
4. retain the byte snapshot and `ImageInfo` in `EncodedSource`.

`open_bytes` performs the same steps without filesystem I/O. The two entry
points must converge after source acquisition; there must not be separate path
and byte decoding implementations.

Snapshotting is intentional. Holding only a `PathBuf` makes the image's identity
depend on later external filesystem state. Holding an open file descriptor is
also insufficient because in-place writes remain observable and platform file
replacement behavior differs. A byte snapshot gives one image object one
stable encoded identity.

## Materialized Storage

Materialized storage must retain semantics that `DynamicImage` alone cannot
express:

```rust,ignore
struct MaterializedImage {
    pixels: PixelStorage,
    mode: ImageMode,
    source_format: Option<ImageFormat>,
    source_info: Option<ImageInfo>,
}

enum PixelStorage {
    Direct(DynamicImage),
    Indexed {
        indices: GrayImage,
        palette: ImagePalette,
    },
}
```

Palette indices must never be represented as ordinary grayscale without the
`Indexed` discriminator. Packed mode `1`, CMYK, integer, and floating-point
modes must retain their exact `ImageMode` even when an operation buffer uses a
different physical layout.

The cached error should be a cloneable codec-domain failure or a dedicated
cloneable `DecodeFailure`. Do not cache `std::io::Error` or binding-specific
exceptions. Path I/O completes before `EncodedSource` exists; host-specific
error mapping remains at the public boundary.

## Image Node Model

The runtime state should describe data ownership rather than whether a caller
remembered to call `load()`:

```rust,ignore
enum ImageNode {
    Encoded(Arc<EncodedSource>),
    Materialized(Arc<MaterializedImage>),
    Pipeline(Arc<PipelineNode>),
}

struct PipelineNode {
    source: Image,
    operations: Arc<[PipelineOp]>,
    properties: DerivedProperties,
    output: OnceLock<Result<Arc<MaterializedImage>, PipelineFailure>>,
}
```

`PipelineNode::output` follows the same once-publication rule as source decode.
Appending an operation creates a new immutable node. It does not invalidate or
modify an existing pipeline cache.

Runtime typestate is preferable to a public generic typestate API here. Python
and JavaScript bindings need one stable `Image` type, and loading may be forced
through methods that take `&self`. The enum plus private typed payloads still
makes invalid combinations unrepresentable without exposing state parameters
to downstream users.

## Read And Mutation APIs

The internal primitive should return shared storage:

```rust,ignore
fn materialized(&self) -> Result<Arc<MaterializedImage>, PilError>;
```

Read-only methods such as `getpixel`, `getdata`, `tobytes`, histogram, save, and
rendering should borrow from the returned `Arc`. They must not call a helper
that clones the complete pixel buffer.

Pipeline executors that require owned output may allocate their result once and
publish it into the pipeline cache. Operations must not mutate cached source
pixels.

Mutating APIs should require `&mut self`, force materialization, then detach
shared storage with an explicit copy-on-write step. The mutation is committed
only after all validation succeeds. If it fails, the original image remains
unchanged.

The existing public `materialize() -> DynamicImage` can temporarily remain as a
compatibility method, but it must be documented as producing an owned copy and
must not be used by internal read paths. A later breaking release should prefer
a borrowed/shared view API.

## Observable Loading Semantics

`load(&mut self)` remains useful for Pillow compatibility, but it is no longer
the only route to persistent decoding. It should force `materialized()` and
replace the handle's node with `ImageNode::Materialized`, releasing the encoded
snapshot for that handle when no other clone needs it.

`is_materialized()` should report whether pixels for the current node are
already cached successfully, regardless of whether materialization was forced
by `load()`, `getpixel()`, `tobytes()`, saving, or pipeline execution.

`verify(&self)` is deliberately separate:

- it fully validates the immutable encoded snapshot;
- it does not change observable load state;
- it does not populate the ordinary materialization cache;
- it may perform work again when pixels are later requested.

This is the sole documented exception to the at-most-once work rule. Verification
is a validation operation, not a request for usable decoded pixels.

## Metadata And Provenance

Three concepts must not share one ambiguous `format` or `info` field:

- `source_format`: detected container format of the original encoded snapshot;
- `source_info`: immutable header facts about that original source;
- `properties`: mode, dimensions, palette state, and animation properties of
  the current image node after any operations.

An operation result may retain source provenance while having different current
dimensions or mode. It must not expose stale `source_info` as if those were
current output properties. Each `PipelineOp` should either derive its output
properties without pixels or mark a property unknown. Accessing an unknown
property may materialize the pipeline once.

The public API should name provenance explicitly. If `format_name()` continues
to return the original container after operations, its documentation must call
that source provenance. A future encoded output format is selected only by
`save`/`encode` and must not be inferred from provenance.

## Concurrency

Use `std::sync` primitives so `Image` remains safe to share across native
threads and compatible with parallel compute paths. `RefCell` is not acceptable.

Initialization must have these properties:

- one successful value or one stable failure is published;
- waiters receive the same complete result;
- a panic cannot expose partially initialized data;
- recursive materialization of the same node is rejected as an internal cycle,
  not allowed to deadlock;
- pipeline construction prevents direct or indirect source cycles.

If the project's MSRV does not provide a stable fallible `OnceLock`
initializer, initialize a `Result` value with `get_or_init`. Both success and
failure are valid cached values.

## Memory Policy

Correctness comes before eviction:

- lazy clones share one encoded snapshot;
- materialized clones share one immutable pixel allocation;
- a pipeline shares its source and owns only its operation list and cached
  output;
- explicit `load()` may detach the handle from encoded storage;
- mutation copies pixels only when storage is shared.

Do not introduce automatic cache eviction in the first implementation. Eviction
would make decode counts, latency, and failures dependent on memory pressure.
Any later bounded-cache design requires a separate observable contract and
stress tests.

## Error Contract

Fixture-backed failures must preserve structured categories:

- unknown signature;
- disabled codec feature;
- malformed header during inspection;
- malformed payload during verification or materialization;
- invalid dimensions;
- unsupported retained mode or operation;
- pipeline execution failure.

Opening failure leaves no image. Materialization or pipeline failure leaves the
existing image node unchanged and caches the deterministic failure on that node.
Copy-on-write mutation failure leaves both the current image and all clones
unchanged.

## Manifest-Driven Acceptance Tests

All encoded inputs, including failures, must come from fixture manifests. Add a
lazy-lifecycle manifest or extend the backend migration manifest with actions
and expected state transitions.

For every supported codec:

1. open a fixture and assert header metadata without materialization;
2. clone the lazy image;
3. force pixels through a read method without calling `load()`;
4. assert both handles observe cached materialization and exact Pillow bytes;
5. repeat pixel access and prove decode count remains one;
6. call explicit `load()` and prove mode, source format, metadata, palette, and
   pixels do not change;
7. execute a fixture-backed pipeline twice and prove execution count remains
   one;
8. mutate one clone and prove the other retains exact original pixels;
9. for path fixtures, replace the path after `open()` and prove later inspect,
   verify, and decode still use the original snapshot;
10. exercise concurrent reads from cloned handles and prove one complete result
    or one identical structured failure is published.

Decode and execution counts may use narrowly scoped `cfg(test)` instrumentation,
but the image bytes and expected results must remain manifest fixtures. Do not
replace parity assertions with counters or synthetic byte arrays.

Failure rows must prove that repeated and concurrent access returns the same
structured error and never retries a deterministic malformed source.

## Implementation Slices

Each slice must format, pass its manifest rows, and preserve full line, branch,
function, and region coverage before the next slice begins.

### Slice 1: Stable Encoded Source

- add `EncodedSource` with `Arc<[u8]>`, `ImageInfo`, and detected format;
- make path and byte opening converge on it;
- eliminate path rereads from decode and verify;
- add path-replacement and clone-sharing fixture tests.

### Slice 2: Shared Decode Cache

- add cached `Result<Arc<MaterializedImage>, DecodeFailure>`;
- preserve exact mode, palette, alpha, and source metadata;
- route explicit and implicit loading through one initializer;
- add success, failure, repeated-read, and concurrent-read fixture tests.

### Slice 3: Borrowed Read Paths

- add the internal shared/borrowed materialization primitive;
- migrate all read-only callers away from owned `DynamicImage` clones;
- retain the owned compatibility method only at the public boundary;
- prove repeated reads allocate no full pixel copies with targeted
  instrumentation or allocation-aware benchmarks.

### Slice 4: Cached Pipelines

- make pipeline nodes immutable and once-initialized;
- derive current properties per operation;
- reject cycles;
- prove exact Pillow output and at-most-once execution for every accepted
  palette-safe operation before widening coverage.

### Slice 5: Copy-On-Write Mutation

- migrate mutating APIs to detach shared pixels explicitly;
- guarantee failure atomicity;
- prove clone isolation for direct, indexed, and pipeline-produced images.

### Slice 6: Binding Semantics

- make Python and JavaScript pixel reads use the persistent cache;
- retain explicit `load()` as a force-and-surface-state operation;
- verify host exception mapping is identical for explicit and implicit load;
- measure native and WASM memory after semantic parity is complete.

## Rejected Designs

- **Require callers to call `load()`:** violates persistent implicit loading and
  makes correctness depend on caller discipline.
- **Keep only `PathBuf`:** permits inspection/decode races and repeated I/O.
- **Return owned images internally:** hides full-buffer copies and defeats cache
  sharing.
- **Use `RefCell`:** loses thread-safe sharing and conflicts with parallel
  backends.
- **Cache only successful decodes:** retries deterministic malformed input and
  permits inconsistent failure timing.
- **Put detection or decoding into `pillow-rs`:** recreates the backend split
  already removed by the migration.
- **Add eviction immediately:** introduces nondeterministic re-decoding before
  the core state contract is proven.

## Completion Gate

The lazy-loading migration is complete only when:

- all twelve invariants are represented by code and manifest-driven tests;
- all eight codecs pass the lifecycle matrix;
- fixture-backed structured failures are stable under repetition and
  concurrency;
- path replacement cannot change an already opened image;
- implicit and explicit loading produce identical retained state;
- palette operations still match exact Pillow pixels and encoded bytes;
- native migration and feature-forwarding tests pass, and binding crates
  compile against the changed API;
- Coverage MCP reports 100% lines, branches, functions, and regions for the
  `image-slash-star` scope; `pillow-rs` uses its maintained manual targets per
  the agreed repository scope;
- the tracking document records the exact coverage artifact and commands.
