# AVIF support

Enable `avif` deliberately. It selects the in-tree safe Rust AVIF/AV1
implementation. There is no native codec fallback, FFI bridge, or runtime
download.

## Application scope

The high-depth, HDR and animation additions below are on `main`, awaiting
the next release. The published 0.1.3 crate retains its earlier partial scope.

The maintained matrix lists AVIF decoder and encoder cases; the generated
[capability page](capabilities.md) reports their active and planned counts.
These fixtures cover selected files and operations, not all legal AVIF streams.

Selected still images, grids, high-depth images and animations have exact
Pillow output checks. The tested animations include frame pixels, timing,
loop metadata, auxiliary alpha, hidden references and show-existing frames.
The HDR witness preserves encoded values in Pillow-compatible RGB output;
it does not add tone mapping or general color management.
The still matrix includes exact 8-bit all-lossless I444, I420
chroma-scaling-from-luma, and monochrome film-grain witnesses; other grain
declarations remain subject to the supported-subset checks.
It also includes 8-, 10-, and 12-bit limited-range monochrome RGB and RGBA
witnesses with exact Pillow byte parity, including full-range auxiliary alpha.

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
