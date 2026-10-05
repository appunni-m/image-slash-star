# Native AVIF repetition evidence

This bundle records 88 complete files observed twice with Pillow 12.2.0 and
libavif 1.4.1/dav1d 1.5.3. Eighty accepted files decode completely; eight
malformed files fail native parsing and Pillow opening. The 267 hashed
artifacts total 9,816,480 bytes, including all inputs, the compiled observer's
source, and 9,293,948 bytes of Pillow-decoded frame outputs. Mutation cases
share the original frame artifacts; each observation records complete per-frame
pixel hashes and durations.

The 63 unchanged fixture sources are declared in
`scripts/generate_avif_loop_refs.py`; `index.json` records each source digest.
The 4:2:2 source adds
horizontal chroma ownership while a small block moves between frames. The
large 4:2:0 source covers large inter blocks. The 4:2:0 4x4 source includes a
small change in a sequence with forced 4x4 inter partitions. The 17x17 4:2:0
source moves a patch near the bottom-right corner and exposes an inter-intra
context edge plus odd-width chroma upsampling at the final visible column. Both
Pillow frame hashes are retained in the native record. The lossy 4:2:0 source
contains a final B16x16 inter leaf with one TX16x16-to-TX8x8 split, and retains
exact Pillow RGB frames and a native infinite-loop observation.
The three-frame mixed-topology B16x16 companion adds sparse luma residuals to
the top-right 8x8 child. In the final inter frame, that TX8 child splits to
TX4 while the other three stay at TX8; all three RGB frames match Pillow and
the native observer confirms infinite repetition.
The monochrome 128x128 source selects wide switchable-transform inter blocks.
The two-frame 8x16 all-lossless I420 source moves a 4x4 patch between frames
and selects a rectangular B8x16 inter transform. Its two exact Pillow RGB
frames, 100 ms durations, and native infinite-loop observation are retained.
The two-frame quality-80 I444 source selects an unsplit 128x128 inter
transform in mode 2; its exact Pillow frame bytes and native loop observation
are retained.
The companion two-frame quality-80 I444 source has a flat gray key frame and
a row-alternating 32-pixel sinusoid. It splits each TX64 root in all four
128x128 mode-2 inter leaves to TX32, without splitting the TX32 children. Both
exact Pillow frames and its native infinite-loop observation are retained.
The three-frame 64x64 source selects a B64 inter root with a TX64 transform
split into TX32 children at both x offsets; all RGB frames match Pillow. The
two 512x128 sequences move the same 96x64 textured patch between x=192 and
x=32 in opposite orders. They select projected temporal motion samples beyond
opposite sides of the x window, and both retain exact Pillow frame hashes,
durations, and native loop observations.
The two-frame 32x32 lossy 4:2:2 checker source records the exact Pillow RGB
pixels and native infinite-loop observation for its translated-patch sequence.
The full-range 10-bit I420 source starts with a flat key frame and changes to
an 8x8 tiled inter frame. Its inter picture uses 128x56 coded dimensions,
160x56 upscaled dimensions, and SGR restoration on Y, U, and V. Both exact
Pillow frame hashes, 33 ms durations, and the native infinite-loop observation
are retained.
The companion full-range 8-bit I420 source follows the same flat-key and
8x8-inter pattern. Its inter picture uses 128x56 coded dimensions, 160x56
upscaled dimensions, and SGR restoration on Y, U, and V, retaining the exact
Pillow frames and native infinite-loop observation.
The 64x64 I444 film-grain reuse sequence has a key frame with explicit grain
parameters and an inter frame that reuses reference slot 0 with a new grain
seed. Both 100 ms Pillow RGB frames match the generated reference hashes, and
the native observer reports two decoded frames with infinite repetition.
The inactive-reference companion disables grain on the key frame while the
inter frame reuses that listed reference with a new seed. Both exact Pillow
frames have no grain, and native observation confirms infinite repetition.
The companion full-range 10-bit I420 source uses the same deterministic frames
with a quality-39, speed-5 encode. Its inter picture uses TX_MODE_SELECT,
128x56 coded dimensions, and 160x56 upscaled dimensions, with no active
restoration. The exact Pillow frame hashes, 33 ms durations, and native
infinite-loop observation are retained.
The 17x17 all-lossless I420 source selects an inter B32x32 leaf whose TX4 grid
ends at the padded MI boundary; its two exact Pillow frames and native loop
observation are retained.
The 60x64 all-lossless I420 source selects an inter B32x32 leaf at pixel
`(32,32)` with a 28x32 visible extent at the right edge. Its exact two Pillow
frames and native loop observation are retained.
The 28x64 full-range 10-bit I420 companion fixes encoder partitions to 32x32.
AV1 pads its partition tree to 32 luma columns, so the translated patch stays
inside the full-width left B32 leaf. The generator checks its key/inter
bitstream, repeatable AV1 sample bytes, and exact Pillow frame hashes; the
native observer confirms both frames and infinite repetition.
The 56x64 companion also fixes partitions to 32x32, and moves a textured patch
inside the right-edge B32 leaf with a 24x32 visible extent. This exercises the
high-depth clipped lossless grid through exact Pillow frame parity and the
native observer's infinite-repetition check.
The 184x64 all-lossless I420 source uses a textured frame and fixed B32
partitions to select the clipped-lossless transform plan at the right edge.
Its two exact Pillow frames and native loop observation are retained.
The 185x64 companion source clips the right-edge B32 block to 25 visible
columns, exercising the public decode fallback for a partial extent that is
not divisible by four. Its two exact Pillow frames and native loop observation
are retained.
The companion 60x64 source fixes 16x16 partitions and shifts deterministic
texture one column left. It selects an inter B16x16 leaf at pixel `(48,16)`
with a 12x16 visible extent at the right edge. Its exact two Pillow frames and
native loop observation are retained.
The two-frame lossy I444 source selects a mode-2 B16x32 deep transform split
and exercises the true arm of the small-chroma terminal predicate while
retaining exact Pillow pixels and native sequence timing.
The 20x20 all-lossless I444 case exercises the B8x8 inter-intra size-group-1
context at clipped edges. The three-frame lossy I444 case selects an Alt
single-reference candidate from the second lane of a compound spatial
neighbor, then combines OBMC with a mixed B16x32 transform-split topology.
Both cases retain exact Pillow pixels and native sequence timing.
The three-frame 64x64 lossy I420 case fixes partitions to B16x16 and selects a
smooth non-wedge inter-intra blend. Its pinned dav1d block trace records one
positive `t=1,m=0,w=0` blend; all decoded RGB frames match Pillow exactly.
The matching mode-1 and mode-2 sequences place horizontal and vertical gray
stripes inside one B16x16 patch, respectively. Their pinned dav1d traces each
record one positive non-wedge blend (`t=1,m=1,w=0` and `t=1,m=2,w=0`), and all
decoded RGB frames match Pillow exactly.
The mode-3 sequence uses a smooth 16x16 grayscale patch in the final frame.
Pinned dav1d syntax tracing observes non-wedge mode 3 on the B16x16 at
`(16,16)`; branch-instrumented public decoding observes the mode-3 `x.min(y)`
mask path for all Y/U/V mask samples. Its three exact RGB frames and durations
also match Pillow.
The four-frame lossy global-motion I444 case carries non-identity global
RotZoom references and four frame-3 B128 compound GlobalGlobal blocks; its
exact Pillow pixels, durations, and native sequence timing are retained.
The four-frame identity/RotZoom I444 case has five frame-3 compound spatial
neighbors where the identity lane has no affine vector and the RotZoom lane
does. It retains four exact Pillow frames, 100ms durations, and an independent
infinite-loop observation.
The quality-80 and all-lossless quality-100 256x128 I420 motion sources
carry two tile columns and one key frame followed by three inter frames.
Their companions retain the frame headers and tile bytes in ordered standalone
tile groups. All four native observations decode every exact Pillow frame and
report infinite repetition. The two complete track-header version-zero variants
of `animated.avif`, including its unknown-duration sentinel, retain all five
Pillow frames and native repetition zero (one total play). Their separately
collected observer build identities remain in `additional_collections`.
`animated_opidc_0x101.avif` is derived from `animated.avif`; it selects
operating-point IDC `0x101` and adds layer extensions for temporal/spatial
layer zero while retaining all five source frames. Its loop observation is
collected from the complete derived file.
Native repetition counts are recorded per input below. Pillow's loop field is
also recorded for every accepted case. These are separate facts: native
repetition 0 means one total play, -1 means infinite, and -2 from an absent
edit list maps to the public `Unspecified` state. Nonnegative native
repetitions map to total plays by adding one.

All edit-list mutations preserve complete file length and media payload bytes.
They exercise omitted edit lists, nonrepeating header-only lists, ignored version,
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

The active Pillow parity matrix compares complete decoded frames, timing,
repetition and first-image pixels. This native evidence bundle does not prove
complete Rust sequence reconstruction or resource accounting; those remain
bounded by the separate Rust parity and coverage reports.
