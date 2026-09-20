# AVIF support

Enable `avif` deliberately. It selects the in-tree safe Rust AVIF/AV1
implementation. There is no native codec fallback, FFI bridge, or runtime
download.

## Application scope

The high-depth, HDR and animation additions below are on `main`, awaiting
the next release. The published 0.1.3 crate retains its earlier partial scope.

The maintained matrix contains 343 active AVIF decode/inspect/verify rows.
All 32 encoder rows remain planned. These counts describe selected files and
operations, not all legal AVIF streams.

Selected still images, grids, high-depth images and animations have exact
Pillow output checks. The tested animations include frame pixels, timing,
loop metadata, auxiliary alpha, hidden references and show-existing frames.
The HDR witness preserves encoded values in Pillow-compatible RGB output;
it does not add tone mapping or general color management.

Check your own files before adopting these paths. Untested syntax, geometry
and color declarations can still return an unsupported error.

## Unsupported behavior and errors

Unsupported syntax, transforms, auxiliary relationships, sequence states, and
encoding remain explicit failures or planned capabilities. Inspect the
[generated capability page](capabilities.md) and [open roadmap](roadmap-new.md)
for the exact tracked scope. Do not route these files through a native decoder
and label the result Rust parity.

## Reproduce evidence

The [fixture provenance guide](../tests/fixtures/input/images/avif/README.md)
records the actual assets and pinned oracle tooling. The reconstruction index
and sidecars are regenerated together from reviewed inputs and the pinned
dav1d/Pillow oracles. Keep them available to clean source checkouts.

Add a complete public input and exact oracle comparison for a new state.
A traced entropy sentence or bounded leaf proof alone cannot establish whole-
frame pixel parity. The final caller-visible frame must be compared before
promoting the corresponding capability.
