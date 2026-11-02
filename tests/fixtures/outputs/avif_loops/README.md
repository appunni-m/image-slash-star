This bundle records 574 complete-file observations made with Pillow 12.2.0 and libavif 1.4.1/dav1d 1.5.3; 566 inputs decode completely and 8 malformed inputs fail both Pillow opening and native parsing. The 1983 hashed artifacts cover this README, 574 input files, the observer source, and exact Pillow-decoded frame outputs.


Fixture sources are declared in `scripts/generate_avif_loop_refs.py`;
`index.json` records each source digest.

The reduced-transform-set B16x64 I420 sequence retains two complete 64x64 RGB
frames of 100 ms and one total play. Its independent native collection records
successful parsing, two decoded frames, and repetition count zero.

`animated_sequence_header_cicp_mismatch_nclx` changes only the AV1 sequence
header's primaries and matrix to 9/13/0 while preserving valid item and track
`nclx` declarations at 1/13/6. Pillow and pinned libavif decode both exact
RGB frames and report one total play; the loop observation is collected as a
separate native build collection.

The high-frame-ID companion advances the contiguous IDs to 20,000 and 20,001,
above the 14-bit delta range. Pinned libavif and Pillow preserve the same two
exact frames and infinite-loop count as the low-ID source.

The wrapped-frame-ID companion advances the first frame to 32,760 and wraps
the next to 10 with all seven reference deltas set to 18. Pinned libavif and
Pillow preserve the same two exact frames and infinite-loop count.

The stale-reference companion uses frame IDs 0, 8,000, and 17,000. The middle
inter frame refreshes one reference slot, leaving the ID-0 references stale
until the final key frame invalidates them. Pinned libavif and Pillow preserve
all three exact frames and the infinite-loop count.

The future-reference companion uses frame IDs 30,000, 10,000, and 20,000. Its
wrapped inter frame refreshes one slot; the final key frame invalidates the
remaining references whose IDs are above its current ID. Pinned libavif and
Pillow preserve all three exact frames and the infinite-loop count.

The trailing top-level duplicate cases copy the original `meta` and `moov`
boxes after `mdat`. Pillow and the repeated native observer both report five
frames with identical pixels and repetition count zero (one play).
The duplicate-track-reference companion contains two `tref` boxes with ignored
`hint` references. Pillow preserves all five frames and their 33 ms timing; a
fresh pinned native observation reports five decoded frames and repetition
count zero (one play).
The zero-target auxiliary-reference companion contains a `tref/auxl` entry
with track ID zero. Pillow and pinned libavif ignore it and preserve all
five exact frames, their 33 ms timings, and the one-play count.
The alpha-timescale companion changes only the auxiliary track `mdhd`
timescale from one to two. Pillow retains all five RGBA frames and their
1,000 ms color-track durations; pinned libavif reports one play.
The duplicate-alpha companion adds a second `auxv` track with a distinct
track ID and the same color-track reference and samples. Pillow and pinned
libavif both preserve the five exact RGBA frames; the native observer reports
five decoded frames.
The two-chunk sample-table companion maps samples 1–2 and 3–5 through two
`stsc` rows and records both chunk offsets. Its five exact Pillow RGB frames,
33 ms timings, and native repetition observation match `animated.avif`.
The companion with one trailing byte in `stco` adjusts both chunk offsets and
the primary-item extent. Pillow and pinned libavif decode all five matching
frames with 33 ms durations and one play.
Its zero-description-index variant is also accepted by Pillow and pinned
libavif, which both decode the same five frames with 33 ms timing and one play.
The trailing-payload companion appends one ignored byte after both `stsc`
mapping rows and rebases the `iloc` and `stco` offsets by one. Pillow and pinned
libavif preserve the same five frames, 33 ms timing, and one play. The `stts`
trailing-payload companion adds four bounded bytes after the timing run and
adjusts enclosing sizes and media offsets; both oracles preserve the same five
frames, 33 ms timing, and one play.
The decoder-model companion repeats the AV1 sequence header in the second
temporal unit and changes only the allowed decoder-buffer delay from zero to
one. It adds a false buffer-removal flag to each coded frame header. Its five
exact Pillow RGB frames and 33 ms timings match `animated.avif`; pinned native
libavif reports repetition count zero (one total play).
The initial-display-delay companion enables the display model and uses the
maximum valid delay of 10. Pillow and pinned native libavif preserve all five
exact RGB frames at 33 ms and report one total play.
The high-tier operating-point companion changes only `seq_level_idx` to 8 and
inserts the required main-tier bit. Pillow and pinned native libavif preserve
the same five exact RGB frames at 33 ms and report one total play.
The duplicate-`stsc` companions append a second sample-to-chunk table to that
sequence. The identical table retains five Pillow frames; a later four-sample
mapping yields four. Both match the native repetition observations. The loop
evidence records the exact runtime and header identities because a clean
libavif source checkout was unavailable when these observations were made.
The 4:2:2 source adds
horizontal chroma ownership while a small block moves between frames. The
16x16 I422 CDEF source pins the active one-entry strength set on its inter
frame, with no loop filtering or restoration. The
large 4:2:0 source covers large inter blocks. The 64x64 all-lossless 4:2:0
source forces fixed 4x4 inter partitions around a moving luma patch. The 17x17 4:2:0
source moves a patch near the bottom-right corner and exposes an inter-intra
context edge plus odd-width chroma upsampling at the final visible column. Both
Pillow frame hashes are retained in the native record. The lossy 4:2:0 source
contains a final B16x16 inter leaf with one TX16x16-to-TX8x8 split, and retains
exact Pillow RGB frames and a native infinite-loop observation.
The eight-frame lossy 4:2:0 skip-mode source has four displayed inter frames
with skip mode enabled. Its native record retains all eight Pillow frame
hashes and confirms the sequence's infinite repetition count.
The three-frame mixed-topology B16x16 companion adds sparse luma residuals to
the top-right 8x8 child. In the final inter frame, that TX8 child splits to
TX4 while the other three stay at TX8; all three RGB frames match Pillow and
the native observer confirms infinite repetition.
The monochrome 128x128 source selects wide switchable-transform inter blocks.
The 124x128 all-lossless monochrome companion clips its 128x128 inter block by
four columns at the right frame edge; its exact Pillow RGB frames and native
infinite-repeat observation cover clipped wide-grid reconstruction.
The three-frame quality-80 I420 companion has two seeded full-range reference
frames and an averaged final frame with a residual patch in each 128x128
region. Its final frame advertises both references for compound selection;
native libavif decodes all three exact Pillow RGB frames and reports infinite
repetition.
The companion exact-average 4:2:0 sequence keeps the same two references and
uses their per-pixel average as its final frame. The pinned native observer
decodes all three Pillow-matching frames and reports infinite repetition.
The quality-99 64x64 I420 companion applies the AV1 soft B16x16 wedge mask to
its two seeded grayscale reference frames in every partition. All three exact
Pillow RGB frames and the native infinite-repetition observation are retained.
The selected Rust public parity case passes exact RGB comparison and records
inter-inter wedge blending plus 4:2:0 chroma-mask reduction.
The all-lossless 128x128 I420 companion uses the same seeded two-reference and
exact-average pattern with 128x128 superblocks. Its final inter block takes the
compound wide lossless-grid path in the Rust public decoder; Pillow and native
libavif retain all three exact RGB frames, each with a 100 ms duration.
The compound tall companion keeps the same three-frame exact-average
pattern and fixture-forces a vertical partition into two B64x128 leaves. It
reaches the height-only wide lossless-grid arm in the public Rust decoder; the
pinned native observer retains all three exact Pillow RGB frames and reports
infinite repetition.
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
The companion mixed-topology source also has a flat gray key frame and a
localized row-alternating 32-pixel sinusoid. Only the top-left 128x128 mode-2
inter leaf splits one TX64 root once to TX32; its other three roots and the
remaining 128x128 leaves remain unsplit. No TX32 child splits. Both exact
Pillow frames and its native infinite-loop observation are retained.
The three-frame 64x64 source selects a B64 inter root with a TX64 transform
split into TX32 children at both x offsets; all RGB frames match Pillow. The
two 512x128 sequences move the same 96x64 textured patch between x=192 and
x=32 in opposite orders. They select projected temporal motion samples beyond
opposite sides of the x window, and both retain exact Pillow frame hashes,
durations, and native loop observations.
The two-frame 32x32 lossy 4:2:2 checker source records the exact Pillow RGB
pixels and native infinite-loop observation for its translated-patch sequence.
The two-frame 64x32 lossy 4:2:2 checker companion exercises a subdivided inter
transform tree beside intra transform syntax; its complete Pillow RGB frames
and pinned libavif infinite-loop observation are retained. The three-frame
64x32 I422 compound-reference companion exposes both earlier references on
its final inter frame; all three exact Pillow RGB frames and the native
infinite-loop observation are retained. The three-frame 64x64 lossy I422
compound-reference companion uses largest-transform mode and a full-frame
B64x64 inter block; all exact Pillow RGB frames and the pinned native
infinite-loop observation are retained. A separate 64x64 I422 profile signals
TX_MODE_SELECT with a single reference and positive qindex; its exact Pillow
RGB frames and native loop observation are retained. The three-frame
128x64 I422 companion contains two 64x64 regions; its final TX_MODE_SELECT
frame repeats the right source region and blends the left, retaining exact
Pillow RGB frames and a separate native loop observation. The three-frame
128x64 I444 companion uses the same two 64x64 regions with a single-reference
TX_MODE_SELECT frame, exercising full-resolution direct chroma grids; its
exact Pillow RGB frames and independent native loop observation are retained.
The two-frame
32x64 lossy I422 vertical-split companion reaches the B32x64 TX4 chroma-grid
path; its complete Pillow RGB frames and pinned libavif infinite-loop
observation are retained. The two-frame 64x64 full-range lossy I420 companion
forces a vertical root split into B32x64 inter blocks and retains their deep
transform trees; its exact Pillow RGB frames and native infinite-loop
observation are retained. A second I420 companion preserves a mixed one-child
and deep transform topology across its B32x64 inter blocks; its exact Pillow
RGB frames and native infinite-loop observation are retained. The two-frame
32x64 lossy I422 small-partition companion uses 8–16-pixel inter partition
bounds and retains exact Pillow RGB frames plus the native one-play observation.
The two-frame 64x64 I422 qindex-zero companion keeps its key frame lossless
and makes its `TX_MODE_SELECT` inter frame non-lossless with a luma DC delta.
Its two exact Pillow RGB frames and pinned native one-play observation are
retained.
The two-frame 64x64 lossy I420 mixed-topology companion forces B8x16 inter
leaves with one TX8x8 child split into TX4x4 blocks and the other left unsplit.
It retains both exact Pillow RGB frames and the pinned native loop observation.
The two-frame
full-range 8-bit I420 companion forces `PARTITION_VERT_4`, producing four
B16x64 leaves with a low-frequency residual in the first leaf so the inter
frame retains TX_MODE_SELECT. Its exact Pillow frames and native loop
observation are recorded.
The qindex-zero companion retains that `PARTITION_VERT_4` layout and keeps its
key frame lossless. Its inter frame has base qindex zero with nonzero chroma
quantizer deltas; exact Pillow RGB frames and the native one-play sequence
observation are retained for this single-terminal `TX_MODE_SELECT` path.
The monochrome companion keeps the same partition and transform profile, with
a nonzero luma delta making its base-qindex-zero inter frame non-lossless. Its
exact Pillow RGB frames and native one-play sequence observation cover the
luma-only qindex-zero branch.
The full-range 10-bit I420 source starts with a flat key frame and changes to
an 8x8 tiled inter frame. Its inter picture uses 128x56 coded dimensions,
160x56 upscaled dimensions, and SGR restoration on Y, U, and V. Both exact
Pillow frame hashes, 33 ms durations, and the native infinite-loop observation
are retained.
The companion 32x32 full-range 10-bit I420 source fixes encoder partitions to
32x32 and translates a textured 31x31 patch. Its key/inter AV1 samples are
repeatable, both exact Pillow RGB frames match their pinned hashes, and the
native observer confirms two decoded frames with infinite repetition.
The companion 32x32 monochrome source uses the same pinned 10-bit all-lossless
encoder with a YUV400 plane layout. Both exact Pillow RGB frames and the native
two-frame infinite-loop observation are retained; the sequence covers the
high-depth inter path for monochrome input.
The companion full-range 8-bit I420 source follows the same flat-key and
8x8-inter pattern. Its inter picture uses 128x56 coded dimensions, 160x56
upscaled dimensions, and SGR restoration on Y, U, and V, retaining the exact
Pillow frames and native infinite-loop observation.
The full-range 8-bit I444 companion follows that flat-key and 8x8-inter
pattern, with 128x56 coded dimensions, 160x56 upscaled dimensions, and SGR
restoration on all three full-resolution planes. Its exact Pillow RGB frames
and native infinite-loop observation are retained.
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
The two-frame full-range 64x64 8-bit I420 mixed-segment sequence keeps segment 1
lossless at qindex zero while segments 0 and 2 remain lossy. Its second frame
is inter-coded and the pinned encoder trace confirms an unsplit, full-visible
B32x32 leaf at MI `(8,8)`. The matrix binds both exact Pillow RGB frames and
the native infinite-loop observation.
The 60x60 companion keeps the same mixed-lossless segment map and inter B32
leaf at MI `(8,8)`, but clips its visible extent to 28x28 at the bottom-right
canvas edge. It has exact Pillow RGB frame observations and a separate native
infinite-loop observation.
The residual 64x64 companion keeps the full-visible qindex-zero inter B32
leaf at MI `(8,8)` and changes all 1,024 pixels in its bottom-right ROI
between frames. The pinned encoder trace proves the leaf remains unsplit;
both exact Pillow RGB frames and a separate native infinite-loop observation
are retained.
The two-frame full-range 10-bit I420 mixed-segment sequence uses a qindex-zero
inter B32 ROI with one lossless residual at `(33,33)`. Its AV1 header selects
transform mode, both exact Pillow RGB frames are retained, and the native
observer confirms infinite repetition.
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
The two-frame 16x64 lossy I420 source uses a TX_MODE_LARGEST inter frame
with a positive qindex. Its exact Pillow RGB frames and native infinite-loop
observation are retained; its tall frame also exercises non-16x16 guard outcomes.
The quality-99 16x64 lossy I420 companion selects TX_MODE_SELECT with qindex 9.
Its exact Pillow RGB frames, 100 ms durations, and native infinite-loop observation
are retained.
The two-frame 64x16 lossy I420 companion transposes that frame geometry. Its
inter frame also signals TX_MODE_LARGEST with a positive qindex; exact Pillow
RGB frames and an independent native infinite-loop observation are retained.
The two-frame 64x64 lossy I420 pair forces an inter-frame PARTITION_HORZ_4 at the
origin, producing four coded B64x16 leaves with positive qindex. The largest-mode
input uses textured residuals. The TX_MODE_SELECT companion keeps its top B64x16
residual low-frequency while retaining texture in the other leaves, allowing the
frame to signal transform-size selection while the top leaf can retain its maximum
transform. Exact Pillow RGB frames and native infinite-loop observations are
retained; independent pinned dav1d block traces confirm each horizontal 1-to-4
partition.
The 10-bit and 12-bit TX_MODE_SELECT companions preserve the same mixed
transform pattern at high bit depth. The 12-bit input uses AV1 profile 2. Both
companions retain exact Pillow RGB frames and independent native loop
observations.
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
The companion wedge sequence selects inter-intra wedge index 2 in the
oblique117 direction with sign flipping disabled at B16x16 `(16,16)`. A public
Rust trace observes all 256 mask samples, and its three Pillow RGB frame hashes
are retained with the native infinite-repetition observation.
The four-frame lossy global-motion I444 case carries non-identity global
RotZoom references and four frame-3 B128 compound GlobalGlobal blocks; its
exact Pillow pixels, durations, and native sequence timing are retained.
The global-motion subexponential companion changes two same-length frame-2
parameter codes in that sequence. Pinned libavif and Pillow decode all four
frames and retain the independent infinite-repetition observation.
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
The five-frame 56x64 derived-skip sequence retains all Pillow frames and an
independent native repetition observation; its separately compiled observer
identity is recorded in `additional_collections`.
The two-frame 16x64 all-lossless I420 qmatrix sequence signals qindex zero and
matrix level 15 on both frames. Pillow and libavif decode both frames, report
100 ms timing, and report infinite repetition; its independently collected
native observer identity is recorded in `additional_collections`.
The `stts_zero_delta` mutation changes only the animated track's `stts`
`sample_delta` from one to zero without changing file length or media payload.
Pillow reports five zero-millisecond frames with unchanged pixels, and pinned
libavif decodes all five frames.
Five additional `stts` mutations retain the five frames and their pixels while
exercising libavif's timing fallbacks: an empty table uses one tick per frame,
a zero-count run uses its final delta, an under-filled run uses its final delta
for the uncovered frame, and an over-filled run supplies the covered frames.
These four mutations report five 33 ms Pillow durations. The empty table is
encoded as a size-16 `stts` box followed by an 8-byte `free` sibling, preserving
the complete file length and media payload. A two-run table with counts `[1,
u32::MAX]` and deltas `[1, 2]` wraps its cumulative `u32` count; it reports
durations 33 ms then four at 67 ms while preserving all five pixels. That
1,243-byte input adjusts enclosing box sizes and the `stco`/`iloc` offsets but
keeps the original media payload bytes.
The `sequence_alpha_duration_mismatch` input changes only the auxiliary alpha
track's five-sample delta from one to two. Pinned libavif and Pillow still decode
all five RGBA frames; Pillow reports the color track's 1,000 ms duration and
the exact frame pixels remain unchanged.
The `animated_sequence_only_no_meta` input has AVIS as both its major and sole
compatible brand, with no top-level still-image `meta` box. It retains the five
exact RGB frames and 33 ms timing values from `animated.avif`; Pillow and pinned
libavif both decode all frames, and libavif reports one total play.
`animated_sequence_only_trailing_meta_after_mdat` appends a handler-only
top-level `meta` box after the complete track media. Pillow and pinned libavif
ignore it and preserve the same five frames, 33 ms timing, and one total play.
`animated_sequence_only_interstitial_meta_before_moov` moves `moov` after
`mdat` and places the same handler-only `meta` box between them. Pillow and
pinned libavif ignore it and preserve all five frames, 33 ms timing, and one
total play.
`animated_stsd_trailing_payload` appends one zero byte after its one declared
`stsd` entry and rebases `stco` to the shifted media data. Pillow and pinned
libavif decode all five original RGB frames; libavif reports one total play.
`animated_unknown_sample_entry_ignored_hint_track` adds a `hint` track with one bounded sample and an unknown sample-entry type. Pillow and pinned libavif preserve all five RGB frames, and libavif reports one total play.
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
Three repeating edit-list cases set the FullBox repeating flag with track
and segment durations 5/5, 6/5, and 2³¹+1/1. They preserve all five exact
frames and 33 ms timings; pinned libavif reports 0, 1, and -1 repetitions,
respectively. A zero track duration with the repeating flag is rejected by
Pillow at image open.

Two sample-entry parity cases retain the first codec and auxiliary properties
when later duplicate `av1C` and `auxi` boxes conflict. Pillow preserves the
exact five frames; pinned libavif parses both inputs, decodes five frames, and
reports one total play.

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

The lossy 8-bit I422 B128x64 source selects a horizontal split of its 128x128 inter root into two 128x64 leaves. Its two exact Pillow RGB frames and an independent pinned libavif repetition observation are retained.

The two companion two-frame 16x16 10-bit I420 lossless sequences use distinct
visible inputs: one shifts a near-full-frame texture by one pixel, and the other
moves a 4x4 luma patch. Both retain exact Pillow RGB frames and independent
native infinite-loop observations. These inputs do not assert a particular
encoder leaf geometry.

The four three-frame temporal motion-range sequences use 16-pixel texture runs or bands, translated 600 pixels in each signed axis direction and back. Their exact Pillow frame hashes and independent native infinite-loop observations are retained.

The two-frame 128x128 10-bit I420 lossless sequence also has an independent
native infinite-loop observation. Its public decode parity case checks both
complete RGB frames against Pillow.
The new 128x128 8-bit I420 all-lossless sequence shifts a deterministic texture
one pixel horizontally between frames. Both exact Pillow RGB frames and the
independent native infinite-loop observation are retained.
The vertical-partition companion forces the 128x128 I420 inter root into two
B64x128 leaves. Its two all-lossless frames match exact Pillow RGB references,
and the independent native observer records infinite repetition.
The B8x8 companion uses a flat key frame and a textured lossy I420 inter
frame. A fixture-only libaom patch forces inter square partitions down to
8x8 while the frame retains TX_MODE_SELECT and a positive qindex. Its two
exact Pillow RGB frames and independent native one-play observation are
retained.

The lossy I444 B8x16 companion forces vertical 16x16 inter partitions, retains two exact Pillow RGB frames, and records an independent native libavif one-play observation.
The four inter-inter difference-weighted witnesses use seeded grayscale references
and final frames derived from AV1's mask `min(64, 38 + abs(ref0 - ref1) / 16)`.
They cover I420, I422, and I444 mask layouts plus the inverted I420 mask. The
public parity cases compare all RGB frames with Pillow; each complete input has
an independent pinned native repetition observation.

The two-frame 10-bit I422 super-resolution sequence codes its inter frame at
128x56, resizes to 160x56, and applies SGR restoration to all three planes.
Its `TX_MODE_LARGEST` syntax reaches the high-depth I422 restoration path; both
exact Pillow RGB frames and a fresh native infinite-loop observation are
retained for the public parity case.

The 8-bit I422 companion uses the same 128x56-to-160x56 super-resolution and
all-plane SGR profile with largest-transform mode. It retains both exact
Pillow RGB frames and an independent native infinite-loop observation for the
public parity case.

The 12-bit I422 companion uses a 128x56 coded inter frame that super-resolves to 160x56 with restoration disabled. Both exact Pillow RGB frames and the pinned native two-frame infinite-repetition observation are retained.

The `stss` companion declares zero sync-sample entries while retaining the original sample index as bounded trailing payload. Pillow 12.2.0 and the pinned libavif observer preserve the same five RGB frames, 33 ms durations, and one-play repetition count.

The clipped 60x64 10-bit I420 companion fixes B16x16 partitions and places a
textured inter residual in the right-edge leaf clipped to 12 visible columns.
Pillow retains both exact RGB frames; the pinned native observer decodes both
and reports infinite repetition.


The distance-compound companion contains three 64x64 8-bit 4:2:0 frames. It enables AV1 distance-weighted compound syntax and uses an 11:5 blend of its two reference frames for the final frame; all three Pillow pixel arrays and the native infinite-repeat observation are retained.
The super-resolution distance-compound companion codes both inter frames at
51x64 and upscales them to 64x64. Its final inter frame enables selectable
distance-weighted compound prediction; exact Pillow RGB pixels and an
independent native infinite-repeat observation are retained.
The six-frame altref companion enables AOM's hidden alternate-reference frames
and distance-weighted compound syntax. Its selected track prefix retains two
hidden references, six exact Pillow RGB frames, and the native infinite-repeat
observation.

The six-frame distance-ratio companion uses reference order hints 3 and 6 around
current hint 5. Its selected distance-compound path has absolute reference
distances 2 and 1; the native observer confirms six decoded frames and infinite
repetition.
The two-frame 128x128 lossy I420 companion changes one localized 16x16 patch between frames. Its public Rust parity case reaches a positive reference lane in single-reference extended motion-vector search; both exact Pillow RGB frames and the native infinite-repeat observation are retained.

The high-depth I444 companion uses a full-range 10-bit sequence whose second
frame forces a vertical B16x64 inter partition in mode 2. Both exact Pillow
RGB frames and a separate native one-play repetition observation are retained.


The `animated_monochrome_superres_sequence_17x16` case is a two-frame, full-range 8-bit monochrome sequence with single-unit SGR restoration. Both frames signal super-resolution denominator 9, reconstruct coded width 16 to visible width 17, and match exact Pillow RGB references at 33 ms per frame. The pinned native observer decodes both frames and reports infinite repetition; its observer build is recorded in `additional_collections`.

The `animated_timing_without_decoder_model` case adds valid timing information
to the five-frame AV1 sequence header while leaving
`decoder_model_info_present_flag` clear. Pillow and pinned libavif preserve all
five exact 150×150 RGB frames at 33 ms per frame, and libavif reports one play.
Its native observer build is recorded in `additional_collections`.

The `animated_decoder_model_without_operating_point_parameters` case enables
valid sequence-level decoder-model timing but leaves
`decoder_model_present_for_this_op` clear. Its frame headers include the
required false buffer-removal flags; Pillow and pinned libavif preserve the
same five exact 150×150 RGB frames at 33 ms per frame and report one play.
Its native observer build is recorded in `additional_collections`.

The `animated_order_hint_disabled` case contains five 64×64 RGB frames with
ordinary AV1 sequence headers and order hints disabled. Pillow and pinned
libavif preserve all five exact RGB frames at 33 ms per frame; the native
observer reports infinite repetition. Its observer build is recorded in
`additional_collections`.

The `animated_screen_content_tools_disabled` case fixes AV1 screen-content
tools off in the sequence header and omits the per-frame selector bits. The
generator preserves each tile payload; Pillow and pinned libavif decode all
five exact RGB frames at 33 ms per frame, and the native observer reports
infinite repetition. Its observer build is recorded in `additional_collections`.

The `animated_integer_mv_disabled` case keeps screen-content tools selectable
while selecting the explicit `force_integer_mv=0` sequence value. Pillow and
pinned libavif decode all five exact RGB frames at 33 ms per frame, and the
native observer reports infinite repetition. Its observer build is recorded
in `additional_collections`.

The `animated_transfer_characteristics_bt709` case signals BT.709 primaries
and transfer characteristics with the BT.601 matrix coefficients in a
five-frame 64×64 4:2:0 animation. Pillow and pinned libavif preserve all five
exact RGB frames at 33 ms per frame; the native observer reports infinite
repetition. Its observer build is recorded in `additional_collections`.

The `animated_identity_matrix_profile1_444` case signals CICP 1/13/0 in an
8-bit 4:4:4 AV1 profile-1 sequence. Pillow and pinned libavif preserve all five
exact RGB frames at 33 ms per frame; the native observer reports infinite
repetition. Its observer build is recorded in `additional_collections`.

The `animated_identity_matrix_profile2_12bit_444` case signals the same valid
CICP values in a 12-bit 4:4:4 AV1 profile-2 sequence. Pillow and pinned libavif
preserve all five exact RGB frames at 33 ms per frame; the native observer
reports infinite repetition. Its observer build is recorded in
`additional_collections`.

The `animated_equal_picture_interval_two_ticks` case encodes AV1 equal-picture timing with `num_ticks_per_picture_minus_1 = uvlc(1)` (`010`) while leaving the BMFF sample timing unchanged. Pillow and pinned libavif preserve all five exact RGB frames at 33 ms and report one total play; the separately collected observer identity is recorded in `additional_collections`.

The 24 `short_refs_q{100,99}_*` inputs retain two or three complete 64x64 I444
frames. Repeated native observations preserve their frame counts and infinite
loop count across aliased slots, tied and wrapped order hints, and references
to different retained images. Live Pillow supplies every exact RGB frame.

The 30 `animated_lossy_wide_*` inputs cover 8/10/12-bit 256x256 sequences
with gray, colored, and fractional planar samples. Native observations retain
frame and loop counts; live Pillow supplies exact RGB frames. Three 12-bit
checker sequences regress wide-color OBMC reconstruction.

The 21 `animated_lossy_wide_i400_*` inputs extend neutral single-tile
monochrome observations to 256-pixel frame axes at 8/10/12-bit depth. They
retain complete frame and loop evidence across multi-superblock and partial
right/bottom coded edges. Live Pillow supplies exact public RGB frames.

The fifteen `animated_lossy_inter_*_b16x4_q*` and `*_b4x16_q*` inputs
retain narrow inter leaves across qualities 45, 70 and 99 in 4:2:0, 4:2:2
and 4:4:4. Each input supplies two exact Pillow RGB frames and an independent
native observation of its loop count. Vertical 4:2:2 producer probes are
excluded because the pinned Pillow oracle rejects their second frame.

Ten quality-99 companions extend the narrow inter inputs to 10 and 12 bits.
Each supplies two exact Pillow RGB frames and independent native loop evidence.

Four fine-detail quality-99 inputs cover fully split vertical narrow transforms at 10 and 12 bits.
Each supplies two exact Pillow RGB frames and independent native loop evidence.

Twelve CDF-control sequences check frozen probability tables at 8/10/12 bits in
4:2:0, 4:2:2 and 4:4:4, including adaptive key frames followed by frozen inter frames.
Every sequence supplies two exact Pillow RGB frames and independent native loop evidence.

Forty-two narrow inter sequences check 8x32 and 32x8 transform trees at 8/10/12 bits
in monochrome, 4:2:0, horizontal 4:2:2 and 4:4:4, using fine quality-70/99 residuals.
Every sequence supplies two exact Pillow RGB frames and independent native loop evidence.

Forty-two thin-64 inter sequences check 16x64 and 64x16 transform trees at 8/10/12 bits
in monochrome, 4:2:0, horizontal 4:2:2 and 4:4:4, using fine quality-70/99 residuals.
Every sequence supplies two exact Pillow RGB frames and independent native loop evidence.

Eighteen five-frame square-64 inter sequences check complete chroma neighbor grids
in 4:2:2 and 4:4:4 at 8/10/12 bits and qualities 70/95/99. Exact Pillow RGB frames
and independent native loop observations cover adjacent blocks and transform trees.

Twenty mixed lossless/lossy super-resolution inter inputs cover monochrome and
4:2:0/4:2:2/4:4:4 at 8/10/12 bits, with luma SGR restoration at 8/10 bits.
All complete frames match pinned Pillow; normal native decode observations
are collected independently of the source-only input producer.

Three ordinary monochrome 8/10/12-bit super-resolution inter inputs cover
neutral filters with no segmentation or restoration. All complete frames
are collected independently with pinned Pillow and the native observer.

Two ordinary monochrome 8/10-bit super-resolution inter inputs cover active
luma SGR restoration without segmentation. All complete frames are collected
independently with pinned Pillow and the native observer.

Four ordinary 8-bit I422 neutral-filter super-resolution sequences cover small
frames, an odd coded width, and denominator-16 scaling beyond the restoration
height limit. Complete frames are observed independently with pinned Pillow
and the native sequence observer.

Five monochrome sequences carry mixed lossless/lossy key and inter frames,
including varied 8/10/12-bit keys. Complete frames are independently observed
with pinned Pillow and the native sequence observer.

Nine boundary sequences carry mixed lossless/lossy key and inter frames,
including 40x16, 60x20 and 128x128 varied 8/10/12-bit keys. Frames are observed
with pinned Pillow and the native sequence observer.

Twelve mixed-key monochrome sequences retain exact reconstructed coded-edge
samples at logical widths 71, 100 and 102, including 20-row bottom edges.
Pinned Pillow observes both frames; the native observer records frame syntax.

Fifty-four color mixed-key sequences cover I420/I422/I444 at 8/10/12 bits,
flat and varied keys, coded widths 71/100/102, and 20-row bottom edges.
Pinned Pillow observes both frames; the native observer records frame syntax.

Nine translated-source mixed-key sequences cover I420/I422/I444 at 8/10/12 bits
with a distinct second-frame source pattern and coded width 102.
Pinned Pillow observes both frames; the native observer records frame syntax.

Nine integer-motion mixed-key sequences cover I420/I422/I444 at 8/10/12 bits
with screen-content tuning, integer-only inter motion precision and coded width 102.
Pinned Pillow observes both frames; the native observer records frame syntax.
