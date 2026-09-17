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
It does not label them as Pillow loop-key parity. The subsequent
[loop evidence bundle](../avif_loops/README.md) now supplies the independent
native origin to matrix generation and retains Pillow's missing key separately.

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
track declarations remain unsupported. The later frame-ID candidate described
below removes the remaining frame-ID presentation gate.

The later display-retention candidate keeps only the selected completion per
track, releasing superseded/lower-priority candidates at each completed-frame
commit. It preserves the existing temporal-unit/spatial/temporal/latest-tie
ordering. Reference-slot ownership is separate; hidden samples still require
their own displayed completion. Deferred `Weak` ownership tests are internal
resource-model assertions, not new native or Pillow observations. This bounds
the number of retained display candidates, not total working memory.

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

## Frame IDs and error-resilient presentation

The `error_resilient` bundle observes the unchanged complete
`animated_error_resilient.avif` with the same pinned native versions. Both
16×16 RGB8 displays are retained (1,536 bytes total), alongside 768 YUV bytes,
exact 100/1000-second timing, and native infinite repetition. Its 19-event
trace includes seven actual native reference reads: slots 0, 1, 7, 6, 7, 7, 0,
each with delta 1, current ID 4627 and reference ID 4626.

The trace exposed an error shared by Rust and the Python syntax inspector:
both previously read all indices before all deltas. Native dav1d reads each
index and delta together and rejects a mismatched reference ID immediately.
The corrected inspector is checked against all seven native read positions
and values, with the OBU-header versus payload position origin made explicit.
Its source hash is retained separately from native decoder provenance.

Two deterministic full-file mutations extend that evidence:

- `valid/frame_id_wraparound.avif` changes the consecutive IDs to 32767 and 0.
  Both unmodified and instrumented native decoders preserve every YUV byte;
  native traces observe all seven wrapped reference IDs as 32767. Repeated
  Pillow observations preserve both RGB frames, durations and metadata.
- `malformed/reference_delta_mismatch.avif` flips file bit 8447, changing the
  first reference delta from 1 to 2. Both native decoders emit only the unchanged
  first YUV frame and report a decoding error. Their CLI exit code is 0, so
  rejection is established by the diagnostic, missing second output and native
  mismatched-ID trace, not the exit code. Pillow preserves the first RGB frame
  and raises `RuntimeError` when loading frame 1. The corrected syntax inspector
  also rejects this complete file. Every observation repeats identically.

The Rust candidate parses each pair, checks modulo ID arithmetic, and removes
the frame-ID-only presentation gate. Current-ID continuity is checked before
reference deltas, retaining the existing repeated-current-ID diagnostic.
Show-existing IDs retain their separate equality check. Deferred regressions
cover full sequence pixels, timing, budgets, wraparound and the malformed file.
No Rust behavioral execution or managed coverage is claimed.

The broader frame-ID/reference state space remains unfinished. At this
candidate's revision, the short-signaling no-future-reference fallback differed
from pinned dav1d. The subsequent [short-reference candidate](../av1_short_references/README.md)
addresses that gap with separate native witnesses. Stale-reference policies
still require additional independent evidence. This fixture has explicit
indices; it does not prove those other cases.
Loop-reference reconciliation is recorded in the later loop bundle; resource
limitations above still apply.

Regenerate this bundle with the same command plus
`--fixture animated_error_resilient` and a fresh `--output` path.
