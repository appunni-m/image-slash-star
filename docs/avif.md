# AVIF support

Enable `avif` deliberately. It selects the in-tree safe Rust AVIF/AV1
implementation. There is no native codec fallback, FFI bridge, or runtime
download.

## Application scope

The maintained matrix contains 343 AVIF decode/inspect/verify rows: 340 active
and three explicitly planned. All 32 encoder rows are planned. These counts
describe selected files and operations, not all legal AVIF streams.

The implementation includes bounded still-image parsing, reconstruction,
container relationships, color metadata, and specific tested transforms.
High-bit-depth, HDR, and animation paths remain incomplete at the application
evidence boundary, even where source code admits selected states.

CICP metadata retention and bounded RGB conversion do not establish HDR transfer,
primaries conversion, or tone mapping. A parsed movie track does not establish
complete sequence decoding or timing/disposal behavior. Tile/grid witnesses
cover their recorded geometry, not every layout.

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
