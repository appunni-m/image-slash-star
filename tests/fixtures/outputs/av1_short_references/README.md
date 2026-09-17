# AV1 short-reference selection evidence

These six complete AVIF files are deterministic header mutations of the pinned
`animated_error_resilient.avif`. They retain both 16×16 frames and every tile
byte. The second frame uses short reference signaling. Its current order hint
and LAST/GOLDEN anchors select the cases below; all eight retained references
have native order hint 0.

| Case | Current hint | LAST / GOLDEN | Native distance | Native selected slots |
| --- | ---: | --- | ---: | --- |
| `past_distinct` | 1 | 0 / 6 | -1 | 0, 7, 5, 6, 4, 3, 2 |
| `equal_distinct` | 0 | 0 / 6 | 0 | 0, 0, 0, 6, 1, 2, 7 |
| `wrapped_future_distinct` | 127 | 0 / 6 | +1 | 0, 0, 0, 6, 1, 2, 7 |
| `past_duplicate` | 1 | 7 / 7 | -1 | 7, 6, 5, 7, 4, 3, 2 |
| `equal_duplicate` | 0 | 7 / 7 | 0 | 7, 0, 0, 7, 0, 1, 6 |
| `wrapped_future_duplicate` | 127 | 7 / 7 | +1 | 7, 0, 0, 7, 0, 1, 6 |

The observations come from pinned scalar dav1d 1.5.3 commit
`b546257f770768b2c88258c533da38b91a06f737`. Instrumentation reads the actual
retained headers and selected indices after native reference derivation.
Unmodified and instrumented decoders each repeat twice and agree on all 4,608
YUV bytes. Pillow 12.2.0 with libavif 1.4.1/dav1d 1.5.3 repeats all 9,216 RGB
bytes and native 100/1000-second durations. The complete source, native build
identities, patch, full-file hashes and 49 artifacts are recorded in
[index.json](index.json). All source and native execution remain development
oracles; no runtime native bridge is introduced.

The source shape is checked before mutation: 1,084 bytes, final `mdat`, two
samples of 41 and 46 bytes, a 291-bit second frame header and five tile bytes.
Replacing seven explicit index/delta pairs with two anchors and seven deltas
removes 15 header bits. Repacking alignment produces a 40-byte OBU payload and
a 44-byte second sample. The collector repairs the OBU, `stsz` and `mdat`
lengths together. The resulting 1,082-byte files preserve primary-item bytes,
chunk offsets and the independent first image. This is a pinned mutation
recipe, not a general AVIF writer.

The evidence fixes two implementation mistakes: Rust and the Python inspector
rejected short signaling without a future reference, and Python selected the
first slot on maximum-distance ties where native selects the last. Native
also allows identical anchors and repeated fallback to the earliest reference,
even when that slot is already selected. The corrected inspector matches all
six native selected arrays.

Deferred Rust regressions compare every public frame and exact timing. A
coverage-only entry point calls the same production selection helper and
compares the selected arrays directly. This is necessary because the retained
keyframe pixels are identical in every slot: pixel equality alone cannot prove
correct tie-breaking. Invalid scalar arguments to that entry point are
separate internal model assertions, not malformed-file Pillow claims. No
coverage exclusions were added. Rust behavior and managed coverage remain
unexecuted; no roadmap row or finding is promoted.

This bundle covers equal retained hints, no-future fallback, wrapped positive
distance, duplicate anchors and both tie directions. It does not establish all
mixed-distance arrangements, empty reference histories, every AV1 reconstruction
tool, complete resource accounting or public sequence parity beyond these
deferred cases.

Regenerate into a fresh ignored directory:

```sh
CC=/usr/bin/clang .oracle-venv/bin/python scripts/generate_av1_short_reference_refs.py \
  --dav1d-source /path/to/pinned/dav1d \
  --meson /path/to/meson --ninja /path/to/ninja \
  --verify-syntax --output target/oracle-staging/av1-short-references/fresh
```
