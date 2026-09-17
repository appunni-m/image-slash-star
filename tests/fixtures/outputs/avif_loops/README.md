# Native AVIF repetition evidence

This bundle records 28 complete files observed twice with Pillow 12.2.0 and
libavif 1.4.1/dav1d 1.5.3. Twenty accepted files retain every source pixel;
eight malformed files fail native parsing and Pillow opening. The 41 hashed
artifacts include all inputs, the compiled observer's source and 420,956 bytes
of original RGB/RGBA frames. Mutation cases share the original frame artifacts;
each observation records complete per-frame pixel hashes and durations.

The three unchanged sources are `animated.avif`, `animated_error_resilient.avif`
and `10bit.avif`. Their native repetition counts are respectively 0, -1 and -1.
Pillow omits `info["loop"]` for all accepted cases. These are separate facts:
native repetition 0 means one total play, -1 means infinite, and -2 from an
absent edit list maps to the public `Unspecified` state. Nonnegative native
repetitions map to total plays by adding one.

All mutations preserve complete file length and media payload bytes. They
exercise omitted edit lists, nonrepeating header-only lists, ignored version,
flags and media fields, both segment-width versions, exact/rounded/partial
repeats, the largest finite native count, its first overflowing value,
indefinite and very large track durations, and disagreement between alpha and
color repetition. The color track supplies the loop count. Invalid metadata
in either track still rejects the container.

The largest finite observation is 2,147,483,647 repetitions, meaning
2,147,483,648 total plays. Larger repetition counts normalize to infinite.
Malformed cases cover zero track/segment durations, invalid entry count or
version, missing fields, and a zero-duration alpha edit. A shortened edit box
uses a trailing `free` sibling to preserve the valid enclosing box extents.

The observer compiles against the clean pinned libavif header at commit
`6543b22b5bc706c53f038a16fe515f921556d9b3`, then reads the actual decoder fields
and decodes all frames through the pinned Pillow wheel's native library. It
does not reproduce the repetition algorithm. The index retains source, binary,
compiler, artifact and input identities. The retained libavif license is
checked before collection. Native observer code is development-only.

```sh
.oracle-venv/bin/python scripts/generate_avif_loop_refs.py \
  --libavif-source /path/to/clean/pinned/libavif \
  --output target/oracle-staging/avif-loops/fresh
```

`scripts/avif_loop_evidence.py` resolves a full input hash to this native record
for matrix generation. Missing evidence is a generation error. Matrix evidence
binds the index hash, case, actual input hash, signed native count, normalized
public value and independent origin. The original Pillow loop observation is
retained. No matrix row is promoted by changing that provenance.

Deferred Rust regressions compare complete frames, timing, repetition and
first-image pixels, and require malformed container errors from inspection,
first-image decode and sequence decode. Rust behavioral execution and managed
coverage remain deferred; this bundle does not prove complete Rust sequence
reconstruction or resource accounting.
