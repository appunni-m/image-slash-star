# AVIF sequence presentation evidence

The `animated` bundle contains independent observations of the unchanged
complete `animated.avif`. It does not establish that Rust reconstructs or
presents the sequence correctly. Rust behavioral tests and managed coverage
remain deferred; the public animated row and AVF-SEQUENCE-001 remain planned.

The pinned scalar dav1d trace observes six decoded frames, two hidden frames,
five displayed frames and one `show_existing` presentation. All displayed
canvases are 150×150. Unmodified and instrumented dav1d produce identical
168,750-byte YUV output. The complete instrumentation patch is retained in
`animated/provenance`, and `traces/color.jsonl` contains 136 one-line events.
The index separates independent BMFF/AV1 syntax from native decoder state.

Pillow 12.2.0 and its bundled libavif 1.4.1/dav1d 1.5.3 produce the retained
five RGB8 frames (337,500 bytes total). Each Pillow observation repeats twice.
An explicit native decoder agrees on every byte and exposes exact duration
1/30 second for each frame; Pillow separately reports rounded 33 ms durations.
Source, binary, compiler, artifact and input identities are retained in
[animated/index.json](animated/index.json).

Loop metadata has two distinct observations. Pillow omits its `loop` key.
A development-only observer compiled against the pinned libavif header reads
`avifDecoder.repetitionCount = 0`, meaning one total play. The existing
[high-depth native record](../avif_sequence_color/high_bitdepth/index.json)
reads -1, meaning infinite repetition. The public `AnimationLoop` model retains
container repetition; the deferred regression compares these native values.
It does not label them as Pillow loop-key parity. Before matrix promotion,
AVIF loop references must use this independent native origin where the
existing generator currently records Pillow's missing key.

The deferred regression also compares the unchanged AVIF movie-source
normalization in `scripts/generate_decode_refs.py`: full rendered canvases,
unspecified disposal/blend, no interlace and no additional default-only frame.
Ordinary first-image pixels are checked separately. Equal pixels alone are
not evidence for the default-image flag. The index's `rust_source_comparison`
is a source-only breadcrumb captured during oracle collection, not execution
evidence or a measurement of the eventual implementation commit.

The candidate implementation traverses color and alpha samples together with
persistent reference state, converts one complete display at a time, and keeps
RGB/RGBA frames private until the traversal succeeds. It requires completed
surface geometry, reserves later output transfer bytes before reconstruction,
and constrains actual coded/display geometry to the inspected canvas before
per-frame allocation. Inspection and track frame count/mode must agree.
This output-byte budget does not bound retained AV1 reference/scratch memory.
Variable-size hidden references and disagreements between primary-item and
track declarations remain unsupported. Frame-ID sequence presentation retains
its existing gap after full current-frame-ID validation.

Internal defensive regressions use these unchanged complete inputs to model
missing reconstruction surfaces, empty reference slots, absent display proof,
incorrect reservation geometry and an empty final tile range. They are Rust
model invariants, not new malformed-file Pillow observations.

Regenerate schema v2 into a fresh ignored directory with clean pinned sources:

```sh
CC=/usr/bin/clang .oracle-venv/bin/python scripts/generate_av1_sequence_refs.py \
  --dav1d-source /path/to/dav1d \
  --libavif-source /path/to/libavif \
  --meson /path/to/meson --ninja /path/to/ninja \
  --output target/oracle-staging/av1-sequence/fresh
```

The collector requires dav1d commit
`b546257f770768b2c88258c533da38b91a06f737` and libavif commit
`6543b22b5bc706c53f038a16fe515f921556d9b3`. It verifies retained licenses and
refuses changed inputs, versions or native observations. Native sources and
observers remain oracle-only and are excluded from the Rust package.
