# Versioned compatibility evidence

## WebP method-zero 32-color palette parity — 2026-10-27

Coverage MCP's filtered older full report listed missing branch outcomes in
`apply_palette`. Its source/build identity and test attribution were unknown,
so the report was used to choose a candidate, not to claim a current suite gap.
Added active parity row `encode:webp:enc_lossless_method_0_palette32` using
the existing 32-color WebP input. The method-zero path exercises the palette
size-over-17 encoding branch and its one-index-per-byte packing case.

The selected parity run passed 1/1: Pillow and Rust encoded the input to the
same 126 bytes, token and ordinary Rust encodes matched, and re-decoding
matched all 64x64 RGB pixels. Its selected LLVM report is
`.coverage-mcp/local-evidence/selected/webp-method0-palette32-20261027/coverage.json`
(SHA-256
`66f592f30727490741d38b0b5a543e214444bdbcb26aa4524581b0fdb3e6b84d`). The
Coverage MCP selected-report comparison against the existing 16-color case
returned `limited`, with six newly observed and six not-observed outcomes
across the token and ordinary `apply_palette` paths. The report records the
true outcome for `palette.len() > 17` and the false outcome for the transparent
tail check at `src/codecs/webp/native/encoder.rs:2315`. Because MCP source/build
identity and test attribution are unverified for these local reports, this is
selected-row coordinate evidence, not a verified full-suite coverage gain.
No production codec code or Rust unit tests were added, and no aggregate
coverage or performance claim is made.

## AVIF decoder-model parameter parity — 2026-10-27

Coverage MCP's imported full baseline identified the true arms in
`SequenceHeader::consistent_with` at `src/codecs/avif/av1/sequence.rs:87` and
`:96` as missing. Added active row
`decode:avif:animated_decoder_model_parameters_change` and a deterministic
mutation of the pinned animated AVIF that changes the decoder-buffer delay in
a repeated sequence header. Pillow 12.2.0 and Rust matched all five frames;
the selected parity run passed 1/1 with 67,500 RGB bytes matching.

The selected local LLVM report is
`.coverage-mcp/local-evidence/selected/avif-decoder-model-parameter-20261027/coverage.json`
(SHA-256
`1b4056a457fb9eec1c4901f00a84349ad0cf33a3d69c0681bc8863be4b9f8422`). It
records one hit on each true arm at lines 87 and 96. Coverage MCP cannot read
reports in this managed worktree, so the selected result has no MCP
source/build attribution or MCP incremental comparison. The full coverage
suite was not rerun. No production codec change or Rust unit test was added.

## WebP VP8 sharpness-4 parity — 2026-10-27

Added active row
`decode:webp:lossy_vp8_filter_variants_lossy_strong_sharp4`, generated with
pinned libwebp 1.6.0 strong filtering and sharpness 4. Pillow 12.2.0 expects a
128x128 RGB result; the selected parity run passed 1/1 with all 49,152 bytes
matching.

The selected LLVM report is
`.coverage-mcp/local-evidence/selected/20261027T035807.061420Z-4b00478634e347d2ac4abf91f352db84/coverage.json`
(SHA-256
`22d39a55d0400d2cd2e565187ab27aba13478723df12f79678a88a697cc46b7d`). It
records 192 visits to the nonzero sharpness path and 192 false outcomes for
`sharpness_level > 4` at `src/codecs/webp/native/vp8.rs:1926`. The previous
full report recorded no false outcomes there, and Coverage MCP's baseline gap
lookup identified that missing arm. Its incremental comparison was
incomparable because the baseline lacked complete exact branch detail; source
identity and test attribution remain unverified, so no aggregate coverage gain
is claimed. No production code or Rust unit tests were added.

## WebP hash-chain index guard contract — 2026-10-27

Added the target-only contract
`fault-contract:webp:webp_hash_chain_index_overflow_rejected` for the private
VP8L hash-chain index bound. Public WebP encoding caps each axis at 16,384
pixels, so a Pillow parity input cannot exceed the `i32` sample-index range.
The `cfg(coverage)` fault point drives this defensive guard through the public
encoder while leaving release builds without fault controls.

The case first decodes the active Pillow `lossless_vp8l` input, injects the
failure, and checks the structured WebP `Dimensions` error. An uninjected
retry must match the existing `enc_lossless` Pillow byte reference, produce
the same bytes as a fresh retry, and decode to the Pillow RGB reference. The
range check now runs once before the low-effort/high-effort const-generic split.
In the focused run, both outcomes of the `i32` conversion check were observed;
Coverage MCP reports no missing branch at that guard. The selected run passed
1/1. Its report is
`.coverage-mcp/local-evidence/selected/webp-hash-chain-index-overflow-20261027-attempt-3/coverage.json`
(SHA-256
`42919ee848aeab78d179ec885552704279c8a6eb3acde2855b779a84505165bb`).

The refreshed full `make coverage` run passed, including 57/57 coverage-matrix
tests, 7/7 decode-policy tests, 1/1 determinism test, and 69/69 feature-gate
tests. Its current report is
`.coverage-mcp/local-evidence/full-20261027-webp-alpha-raw-token/coverage.json`
(SHA-256
`379e6fe31021fce03cf965be6aa903938c93d70df50e226e549b80fc56c98ba3`). Totals
are 93,817/139,562 lines (67.2225%), 18,626/32,042 branches (58.1300%),
5,224/8,928 functions (58.5125%), and 143,267/216,612 regions (66.1399%). The
raw full-report branch counters now record one true and 14 false outcomes at
the token-aware raw-alpha choice (`encoder.rs:2959`); line, branch, and function
totals did not increase, while covered regions increased by one. Coverage MCP
cannot open this managed-worktree report, so no verified aggregate gain or
regression is claimed. This is target-only fault-contract evidence, not a
Pillow parity pass. The coverage-only fault hook is absent from release builds;
no performance speedup is claimed, and no Rust unit tests were added.

## WebP RIFF sink header fault contracts — 2026-10-27

Coverage MCP's imported full report marked the true arms of
`write_riff_to_sink`'s short-header, `RIFF` signature, and `WEBP` form-type
checks as missing. That baseline has no source/build receipt, so the gaps were
treated as candidates and confirmed against the current encoder before adding
tests.

Added target-only cases
`fault-contract:webp:webp_riff_header_too_short_rejected`,
`fault-contract:webp:webp_riff_signature_mismatch_rejected`, and
`fault-contract:webp:webp_form_type_mismatch_rejected`. Each references the
active `encode:webp:enc_lossless` parity row and checks the structured
`Malformed`/`StillEncode` result, preservation of caller sink contents, and a
clean retry against exact Pillow bytes and decoded RGB pixels. All three
selected cases passed. Their local LLVM report is
`.coverage-mcp/local-evidence/selected/webp-riff-sink-headers-20261027-attempt-4/coverage.json`
(SHA-256
`777f29b2f7b95157fbc83818328c001991e6842fb7426a6ff7659a0f349b390b`). It
records one true and respectively 8, 7, and 6 false-arm hits at
`src/codecs/webp/encode/mod.rs:189`, `:194`, and `:199`; every decision arm in
the new fault hook also has nonzero true and false counters. The
managed-worktree report is local-only; Coverage MCP cannot verify its
source/build identity or attribute its test run. No aggregate coverage
increase is claimed. Fault mutation is compiled only for coverage builds; its
three guards are recorded as `defensive_model` in the coverage-origin
inventory. No Rust unit tests were added.

## WebP token-aware raw-alpha parity — 2026-10-27

Added active parity row `encode:webp:enc_lossy_rgba_alpha_raw_token`, reusing
the seeded high-entropy alpha input from `enc_lossy_rgba_alpha_raw` and adding
the public cancellation-token encode path. The pinned Pillow 12.2.0 reference
is 4,210 encoded bytes (SHA-256
`c3fed186ee97cc2418fd1815880646118e73fdaa3fed896bf977118972b13e4d`); decoding
the result matched all 16,384 RGBA bytes. The selected parity run passed 1/1.

The selected LLVM report is
`.coverage-mcp/local-evidence/selected/webp-alpha-raw-token-20261027/coverage.json`
(SHA-256
`22bd017a9a5aad3a041ac86326483301ed97ec255b1bc94e2e3a70b319dac0a7`). Its
raw branch counters record the previously unobserved true arm of
`encode_alpha_stream` at `src/codecs/webp/native/encoder.rs:2959` once in the
`TokenBitWriterCheckpoint` specialization; the earlier full report recorded
zero true-arm hits at that coordinate. Coverage MCP could not open reports
from the managed worktree, so this remains selected LLVM evidence with no
verified aggregate gain. No production code changed, and no Rust unit tests
were added.

## AVIF qindex-zero non-lossless inter parity — 2026-10-27

Added active rows
`decode:avif:animated_lossy_inter_420_b16x64_qindex0_mode_select_64x64` and
`decode:avif:animated_lossy_inter_monochrome_b16x64_qindex0_mode_select_64x64`.
Both deterministic 64×64 sequences use a lossless key frame and a non-lossless
qindex-zero `TX_MODE_SELECT` inter frame. The I420 inter frame has chroma
quantizer deltas of 2; the monochrome inter frame has a luma DC delta of 2.
Their pinned generators verify repeatable timestamp-normalized AVIF output,
frame headers, and exact Pillow RGB frame hashes.

The decoder now passes the segment qindex to inter transform-type decoding.
Qindex zero selects the default DCT transform even when quantizer deltas make
the segment non-lossless. This avoids consuming a transform-type symbol that
these inputs do not signal. The focused public parity run passed 2/2, with
12,288 RGB bytes matching Pillow for each case. The full `make test` run also
passed, including the complete matrix suite and native, WASI, and WASM feature
lanes. The local LLVM report is
`.coverage-mcp/local-evidence/selected/avif-q0-final-coverage-20261027/coverage.json`
(SHA-256
`72010b8a5d9436661980a8d941c26b2b7f12382109386d40287c3c4236cfc4d0`). Its
selected counters record seven hits on the new qindex-zero DCT path, two hits
on the I420 qindex-zero single-terminal guard, and three on the monochrome
guard. Other generic instantiations remain unobserved in this selected run.

Coverage MCP's saved full report identified missing transform-type branch
observations, but has no source/build receipt and reports source identity as
unverified and test attribution as unknown. It cannot read the refreshed
report from this managed worktree, so the local counters are selected-scope
evidence only; no full-suite or marginal coverage gain is claimed. No Rust
unit tests were added.

## I444 compound split parity — 2026-10-26

Added active row `decode:avif:animated_lossy_inter_i444_compound_split_b16x32_mode2`.
The deterministic 1,457-byte fixture pins its normalized AVIF hash and all
three Pillow RGB frame hashes. The final frame header permits compound
selection and exposes both earlier frame references. The pinned native loop
observer reports three decoded frames and infinite repetition.

The selected parity/coverage run passed 1/1 with exact RGB output. Its LLVM
report is `.coverage-mcp/local-evidence/selected/i444-compound-probe-20261026/coverage.json`
(SHA-256 `07edc4ef031fafcc623a061136ca8023157be78ed6d489a391419f138a992b37`).
At `src/codecs/avif/av1/block.rs:48314`, the target
`decode_inter_compound_translation` specialization recorded two true-arm hits;
the saved full report recorded zero true-arm and 67 false-arm hits for that
specialization. This is branch-specific evidence, not an aggregate coverage
union. The saved full and selected reports lack Coverage MCP source/build
receipts; MCP therefore marks source identity unverified and test attribution
unknown. Its incremental comparison is incomparable because baseline branch
detail is incomplete. Other generic specializations at this location remain
uncovered. No production Rust code or unit tests were added.

## AV1 temporal motion-vector range parity — 2026-10-26

Added active rows `decode:avif:animated_motion_temporal_mv_range_2048x128`,
`decode:avif:animated_motion_temporal_mv_range_positive_x_2048x128`,
`decode:avif:animated_motion_temporal_mv_range_negative_y_128x2048`, and
`decode:avif:animated_motion_temporal_mv_range_positive_y_128x2048`. The four
A/B/A sequences use 16-pixel texture runs or bands and translate 600 pixels in
each signed axis direction. The deterministic generator pins complete AVIF
hashes and all exact Pillow RGB frame hashes; the independent native observer
records infinite repetition for each sequence.

The combined selected parity run passed 4/4, with every 786,432-byte RGB frame
matching its Pillow reference. Its local LLVM report is
`.coverage-mcp/local-evidence/selected/20261026T143900.923631Z-bec51189f9e34b9aa76a14cd518a81bb/coverage.json`
(SHA-256
`6dc06570ffd077bfd47027fe31154e937cd43a2d78071bdf462f085a78a5039b`). Raw
branch counters at `src/codecs/avif/av1/tile_state.rs:995` record all four
signed motion-vector boundary false arms: 2,304 hits for `y > -4096`, 2,083
for `y < 4096`, 2,560 for `x > -4096`, and 2,336 for `x < 4096`.

Coverage MCP's line-specific branch lookup returned no gap at line 995. The
report had to be copied temporarily from the managed worktree because the MCP
cannot read that worktree directly; it therefore marked source identity
unverified and test status unknown. The incremental comparison with the prior
full AVIF selection was incomparable because the provider lacked complete
branch detail, so no marginal coverage gain is claimed. In this selected run,
the `spatial_block` absence and ineligible-reference outcomes in
`temporal_entry_at` remain unobserved and are pending public-parity discovery.
No Rust source or unit tests were added.

## AVIF tall TX_MODE_LARGEST parity — 2026-10-26

Added active row `decode:avif:animated_lossy_inter_420_mode_largest_16x64`. Its
pinned generator verifies a two-frame 16x64 8-bit I420 sequence, an inter frame
signaling `TX_MODE_LARGEST` with positive base qindex, and exact Pillow RGB
frame hashes. Native libavif evidence records two decoded frames, infinite
repetition, and 100 ms frame durations.

The selected parity run passed 1/1 with 3,072-byte RGB output. Its report is
`.coverage-mcp/local-evidence/selected/20261026-avif-i420-mode-largest-16x64.json`.
Coverage MCP's baseline lacked the false arm of the 16x16 reference-height
guard at `src/codecs/avif/av1/entropy.rs:14344`; the selected report records
that arm for one generic instantiation. A coordinate-level comparison found
seven new false-arm observations across related 16x16 guards, while other
generic instantiations remain uncovered. Coverage MCP marks source identity
unverified and named-test attribution unknown, so this is selected evidence,
not a verified aggregate gain. This earlier case did not reach the candidate
`wide_lossy_single` path; the fixture claims the signaled transform mode, not a
particular transform size. No Rust code or unit tests were added.

## AVIF coded B64x16 inter parity — 2026-10-26

Added active rows
`decode:avif:animated_lossy_inter_420_b64x16_mode_largest_64x64`,
`decode:avif:animated_lossy_inter_420_b64x16_mode_select_64x64`,
`decode:avif:animated_lossy_inter_420_b64x16_10bit_mode_select_64x64`, and
`decode:avif:animated_lossy_inter_420_b64x16_12bit_mode_select_64x64`. The
fixture-only encoder forces `PARTITION_HORZ_4` at the 64x64 inter-frame root,
creating four coded B64x16 leaves with positive qindex. The 8-bit pair signals
`TX_MODE_LARGEST` and `TX_MODE_SELECT`; the 10-bit profile-0 and 12-bit
profile-2 companions preserve the select-mode pattern at high bit depths. The
generator requires deterministic timestamp-normalized encodes, exact Pillow
12.2.0 RGB frames, and the pinned AV1 frame-header profile. An independent
pinned dav1d trace confirms the inter-frame root partition for the 8-bit pair.

The combined selected public parity run passed 4/4 with exact RGB output. Its
local LLVM report is
`.coverage-mcp/local-evidence/selected/20261026-avif-i420-b64x16-depth-coverage-r2.json`
(SHA-256
`efc1a82884be25317b04465bf151728161006500a8d15a4db59ba120b62c5c63`). Across
the selected binary it observes 3,829 of 32,078 branch outcomes, 294 more than
the earlier 8-bit-pair-only selected report; this is selected-scope evidence,
not a full-suite union. At `src/codecs/avif/av1/block.rs:48406`, one of eight
generic branch records has four true-outcome hits and three false-outcome hits;
the other seven records have no hits. The mode-select cases reach the false
outcome at 8, 10, and 12 bits. The positive-qindex outcome at line 48407 is
observed three times; its qindex-zero alternative remains uncovered. Coverage
MCP cannot read this report from the managed worktree. Its saved full baseline
reports 13,447 of 32,078 branches missing, with source identity unverified and
test attribution unknown, so this selected run does not establish an aggregate
gain. No Rust source or unit tests were added.

## AVIF thin B16x64 lossy TX_MODE_SELECT parity — 2026-10-26

Added active row
`decode:avif:animated_lossy_inter_420_b16x64_mode_select_64x64`. Its
fixture-only pinned libaom patch forces `PARTITION_VERT_4` at the 64x64 inter
root, producing four B16x64 leaves. The first leaf has a low-frequency
residual; the other leaves retain texture so the frame signals
`TX_MODE_SELECT`. The deterministic generator verifies the pinned Pillow
12.2.0 RGB frames and AV1 headers. A temporary decoder trace confirmed the
first inter leaf is 16x64 at qindex 76 and selects the wide single transform;
both public Rust frame outputs match Pillow exactly. The pinned native loop
observer records both frames and infinite repetition.

The one-row selected parity run passed 1/1. Its LLVM report is
`.coverage-mcp/local-evidence/selected/20261026T124731.508781Z-b6bb2feb74784ffca8ae467eac027ce1/coverage.json`
(SHA-256
`dcd5fd136ea83db1da78563e4758f1098fdc9f31e8c689b2fbdafb0d57bb724f`). The
full active AVIF decode selection then passed 607/607; its retained report is
`.coverage-mcp/local-evidence/selected/20261026T125143.028760Z-27bd0323559441969424d44b79b1ec82/coverage.json`
(SHA-256
`0961e684dee04a06af1e1f2314866711d218571a45503933eeda12dc801d010f`). The
selected report records the thin-block and positive-qindex outcomes at
`src/codecs/avif/av1/block.rs:48418` and `:48420`. Coverage MCP reports source
identity unverified and test attribution unknown for the selected evidence;
the full report lookup timed out. No aggregate or marginal coverage gain is
claimed. This case remains a distinct B16x64 regression witness. No Rust unit
tests were added.

## JPEG trailing DHT segment-boundary parity — 2026-10-26

Added active decode rows
`decode:jpeg:baseline_444_single_scan_trailing_dht_short_length_with_tail_counts`
and
`decode:jpeg:baseline_444_single_scan_trailing_dht_short_length_with_tail_value`.
The first declares only one DHT count byte and places the remaining count bytes
outside the segment; the second declares one symbol and places its value byte
outside the segment. Both complete JPEG inputs pass Pillow structure
verification, while Pillow 12.2.0 rejects pixel decoding with
`OSError: broken data stream when reading image file`; the Rust public decoder
matches the malformed-input result.

The two selected parity cases passed 2/2. Their retained LLVM report is
`.coverage-mcp/local-evidence/selected/20261026T014941.848086Z-38eb7f1cd96f4921ae3377a017f4025c/coverage.json`
(SHA-256
`49be79c7021c13ad6d18c4d8667fb86355d166bf232164a37e7110a006eac206`). Raw
branch counters observed the DHT count-table and symbol-value boundary guards
in `src/codecs/jpeg/decode/decode.rs`. Coverage MCP marked source identity
unverified and test attribution unknown, so this is selected local evidence,
not a verified aggregate coverage gain. No Rust source changed and no unit
tests were added.

The current fixture matrix contains 2,639 total rows: 2,111 decode / inspect /
verify rows and 528 encode rows. Of those, 2,111 decode rows and 497 encode
rows are active; 31 planned encode rows remain.

## JPEG duplicate grayscale SOS selector parity — 2026-10-26

Added the active decode row
`decode:jpeg:baseline_grayscale_duplicate_sos_component`. Its deterministic
fixture duplicates the sole grayscale component record within one baseline SOS
and updates the segment length while preserving the entropy-coded scan. Pinned
Pillow accepts inspection and verification but rejects pixel decoding; the
Rust decoder matches its malformed-input outcome. This case covers the equal
component-index boundary in `validate_scan_component_order`, distinct from an
out-of-order selector.

The focused selected parity run passed 1/1. Its retained LLVM report is
`.coverage-mcp/local-evidence/selected/jpeg-duplicate-sos-component-20261026/coverage.json`
(SHA-256
`8508f00520443c50f370fe471049afbb5be832c4d91ffbcbe0bc1856db174f3b`), with an
unchanged source and fixture set during execution. It does not cover the
previously observed `scan.components.as_slice()` else arm at
`src/codecs/jpeg/decode/decode.rs:2669`: `decode` rejects duplicate scan
indexes in `validate_scan_component_order` before reaching the grayscale fast
path, and the parser rejects an empty SOS. No ordinary one-component JPEG scan
can reach that arm, so no parity input or unit test was added for the private
state. Coverage MCP was unhealthy during the gap query; its local receipt is
not a verified source/build receipt and records test attribution as unknown.

## JPEG duplicate grayscale scan parity — 2026-10-26

Added the active decode row
`decode:jpeg:baseline_grayscale_duplicate_component_scan`. Its deterministic
fixture duplicates the sole full baseline scan in the pinned grayscale source
and passes Pillow's container verification, while Pillow pixel loading rejects
the repeated scan. The JPEG grayscale direct-safe path now rejects a repeated
single-component scan as malformed; color multi-scan duplicate-component
behavior remains covered by its existing parity row.

The four-row selected parity batch passed 4/4, including the malformed input,
baseline grayscale, progressive grayscale, and color multi-scan regression
cases. Raw LLVM branch counters observed both outcomes at
`reconstruct_single_scan_grayscale_direct_safe`; Coverage MCP marked the report
unverified and did not attribute those outcomes to tests, so this is selected
local branch evidence only. The retained selected report SHA-256 is
`5845b9223258a5771661bcf65196ce88cf366203e2727d6e417145292098c4df`.

The subsequent full all-feature LLVM run passed the complete integration suite
and retained
`.coverage-mcp/local-evidence/full-20261026-gray-multiscan/coverage.json`
(SHA-256
`de5ee2500bf23f4d6002f6405607250cf225e2db66456bca01730cbcb5557917`). It
measured 93,857/139,522 lines (67.2704%), 18,638/32,078 branches (58.1021%),
5,225/8,928 functions (58.5237%), and 143,367/216,563 regions (66.2011%),
above the configured floors. Coverage MCP read an imported copy with source
unverified and test attribution unknown; its branch gaps are leads, not
verified test attribution or regression claims. The four-metric 100% goal
remains open.

The current fixture matrix contains 2,618 total rows: 2,091 decode / inspect /
verify rows and 527 encode rows. Of those, 2,091 decode rows and 496 encode
rows are active; 31 encode rows remain planned.

## JPEG baseline 4:4:4 unaligned-width parity — 2026-10-25

Added active encoder parity row
encode:jpeg:enc_sub_444_unaligned_large_width_fallback, reusing the 33×33
RGB JPEG source. Its area (1,089 pixels) passes the baseline row-streaming
threshold, while width 33 fails the 8-pixel alignment guard at
src/codecs/jpeg/encode/mod.rs:683 and selects the generic fallback. The
pinned Pillow reference is 1,470 encoded bytes with SHA-256
2e3477bd6c8e471005723e5a7c86a3aa2e6ba060a8fae116eae11a86eb4a71f8; the
decoded 3,267-byte RGB reference has SHA-256
1c215933edf3484516304582490a95e0d64d38ae506d992b5fe7ed6ac548cb63.

The exact selected parity run passed 1/1, matched all encoded bytes, and
re-decoded to 33×33 RGB pixel parity. Its report is
.coverage-mcp/local-evidence/selected/jpeg-444-wide-20261025/coverage.json
(SHA-256
e2eccb6e0f1143a1b542a936b09f094b3687576aa11798277d61d7846a5baa94).
Coverage MCP reports the true arm of the width guard as still missing in this
selection, consistent with this unaligned input covering the false edge. MCP
marks source/build identity unverified and test attribution unknown; the local
receipt records source revision f916e810ed7d435e0e1fc4493da3a881d9e2b469,
no source change during execution, and an unchanged fixture snapshot. The
report is selected-case evidence only, not a project-wide coverage claim. No
Rust source changed and no unit tests were added.

The current matrix contains 2,616 total rows: 2,089 decode / inspect / verify rows and 527 encode rows. Of those, 2,089 decode rows and 496 encode rows are active; 31 encode rows remain planned.

## AVIF B32 lossless coverage follow-up — 2026-10-25

Kept the 32x32 10-bit I420 all-lossless sequence with a translated 31x31
texture patch as a public parity case. The finalized five-row selection passed
5/5 exact Pillow pixel comparisons; its two-row control selection passed 2/2.
Coverage MCP's selected comparison observed 71 additional branch coordinates
across 32 function groups, with the same 32,056-branch denominator. Its result
is limited because source/build identity is unverified and test attribution is
unknown. The local receipts for both selections record the same source-manifest
SHA-256
`c06ebd6afe7cc9dcc5dda317bb59c2b606bba236d52b95646d1e0bf9f3399c5c`, no source
change during either run, and all selected parity rows passing.

The five-row report is
`.coverage-mcp/local-evidence/selected-from-worktree/av1-final-five/coverage.json`
(SHA-256
`df5052e93991f4dad8fb5be78211f4651fab05912fc9d01cb41f796ac5afd5eb`); the
controls are in
`.coverage-mcp/local-evidence/selected-from-worktree/av1-final-controls/coverage.json`.
The five-row LLVM data records both outcomes of the partial-grid geometry
check at `block.rs:47999` (one true, six false); controls recorded three false
outcomes. The combined selection does not isolate which clipped case produced
the true outcome, and selected evidence does not establish full-suite or
release coverage. No Rust unit tests were added; all selected cases use the
public Pillow parity matrix.

The current matrix contains 2,609 total rows: 2,083 decode / inspect /
verify rows and 526 encode rows. Of those, 2,083 decode rows and 495 encode
rows are active; 31 encode rows remain planned.

## AVIF 10-bit I420 lossless input variants — 2026-10-25

Retained two distinct two-frame 16x16 all-lossless AVIF parity inputs: a
near-full-frame texture shifted by one pixel and a moving 4x4 luma patch. Both
have exact Pillow RGB frame references and independent native infinite-loop
observations. The exact selected parity run passed 2/2 cases. Its LLVM report
is `.coverage-mcp/local-evidence/selected/20261025T155900.516310Z-a23aa3bbec5b4d718a3af53a981d7a71/coverage.json`
(SHA-256 `f9370f8dd76b8fc308283d017c27b985ef6e2518d1e969bb1ce5b72ce9782145`).

Coverage MCP's selected-test comparison with the earlier small-patch selection
observed 39 additional branch locations and no missing observations. Its
receipt leaves source/build identity unverified and test attribution unknown,
so this is selection-level evidence rather than a verified aggregate coverage
gain. At that source revision, the branch at
`src/codecs/avif/av1/block.rs:48152` remained uncovered; the fixtures did not
assert a particular encoder leaf geometry. No Rust unit tests were added. At
that checkpoint, the matrix contained 2,608 total rows: 2,082 decode / inspect
/ verify rows and 526 encode rows. Of those, 2,082 decode rows and 495 encode
rows were active; 31 encode rows remained planned.

## TIFF later-IFD BitsPerSample parity — 2026-10-25

Added public sequence-decode parity rows for a second TIFF directory with
empty or nonuniform `BitsPerSample`. Both fixtures derive from the pinned
`multipage.tiff` source; the generator checks source and output digests, first
page pixels, and Pillow 12.2.0's sequence error. TIFF inspection now marks a
sequence count incomplete when a later directory has empty or nonuniform
sample depths, while the normal decoder retains the matching malformed-data
error.

The seven-row selection covering these cases and existing multipage controls
passed 7/7. The LLVM report is
`.coverage-mcp/local-evidence/selected/20261025T132428.639043Z-6c94ed7ecadd415b95c0894d8a57d1f2/coverage.json`
(SHA-256 `aebd9cbeec4296e7e330293e9ec26400aea3a7c8597f39d483d523398e936457`).
Its branch counters show both outcomes for the two `BitsPerSample` checks at
`decode_ifd:201`. Coverage MCP still labels this selected report's source as
unverified and test attribution as unknown, and reports four missing branch
observations at that line; this is local selected-run evidence, not an
aggregate coverage or regression claim. No Rust unit tests were added.

## AVIF variable-luma loop-filter parity — 2026-10-25

Added the active `animated_lossy_i420_mode2_variable_tx_loopfilter_b128x128`
row, a deterministic two-frame 256x256 I420 AVIF whose inter frame exercises
variable luma transforms with nonzero loop-filter levels. The pinned Pillow
12.2.0 references match both decoded RGB frames, and the native libavif loop
evidence parses successfully.

The exact selected parity run passed 1/1 case. Its LLVM report is
`.coverage-mcp/local-evidence/selected-from-worktree/av1-variable-luma-loopfilter-final-2026-10-25/coverage.json`
(SHA-256 `26bac7919731c1c27d8a16e0f35de3a617af63de874fdb17557a4d27b47d8565`).
At `filter.rs:247`, the report has one `build_masks` instance with both branch
counters observed and a separately mangled zero-count copy. Coverage MCP lists
the true and false observations as gaps; it labels source identity unverified
and test attribution unknown. Direct LLVM records at the same source revision
show the active function's true counter moving from 0 in the full baseline to
1 in the selected run (with 7 false-edge hits); the zero-count copy remains.
Coverage MCP's incremental comparison is incomparable because the baseline's
branch detail is incomplete. This is a precise local edge observation, not an
MCP-verified aggregate coverage gain. No Rust unit tests were added.

The current matrix contains 2,606 total rows: 2,080 decode / inspect / verify rows and 526 encode rows. Of those, 2,080 decode rows and 495 encode rows are active; 31 encode rows remain planned.

## WebP color-transform size-field truncation parity — 2026-10-25

Added a malformed VP8L stream that completes a subtract-green transform and
leaves only two of the following transform's three size bits. Pinned Pillow
accepts it in `verify` and reports `OSError: failed to read next frame` during
decode. The independent VP8L parser classifies the failure as a truncated
transform header at bit offset 46. The selected public parity case passed 1/1.
The current matrix contains 2,605 total rows: 2,079 decode / inspect / verify rows and 526 encode rows. Of those, 2,079 decode rows and 495 encode rows are active; 31 encode rows remain planned.

## WebP lossy-alpha filter parity — 2026-10-25

Added seven active public encode rows for horizontal, vertical, and gradient
alpha prediction, nonzero palette gaps, palette wraparound, and the
token-aware horizontal-filter path. The alpha encoder now compares the
unfiltered and estimated-filter candidates and emits the exact pinned Pillow
WebP bytes. The selected alpha set passed 28/28 rows, including 124 public
encode calls with no panics. No Rust unit tests were added.

`make fmt`, strict `make lint`, `make lint-x86-simd`, `make verify
PYTHON=.oracle-venv/bin/python`, and `make test-feature-matrix` passed. The full
coverage matrix passed all 57 matrix tests, 69 feature-gate tests, and four
animation-loop checks. Configured coverage floors passed at 93,688/139,473
lines (67.172858%), 18,573/32,068 branches (57.917550%), 5,213/8,917
functions (58.461366%), and 143,021/216,455 regions (66.074242%). Strict
`make coverage-complete` still fails its 100% requirement. The retained local
report is
`.coverage-mcp/local-evidence/full-20261025-webp-alpha-filter-option-final/coverage.json`.
Coverage MCP reports no missing branches in the new filter estimator or
filter-plane helpers in that report. Its source identity is unverified and test
attribution is unknown; the largest remaining WebP groups include
`fill_hash_chain_impl`, `write_huffman_tree`, `write_token_stream`, and
`apply_palette`. Inspect their source paths before adding parity rows because
some gaps belong to mutually exclusive generic token/no-token paths.

A six-input A–B–B–A comparison against pinned Pillow confirmed exact encoded
bytes for every input. The Rust/Pillow speed ratio ranged from 0.35× to 0.81×;
this records an open encoder-performance gap and is not a speedup claim. The
x86 SSE2 and AVX2 builds pass strict cross-target lint, but runtime timing on
x86 hardware remains outstanding.

## Full local coverage snapshot — 2026-10-25

`make COVERAGE_REPORT='.coverage-mcp/local-evidence/full-20261025-jpeg-even-upsampling/coverage.json' coverage`
passed the full matrix and configured floors. The retained report is
`.coverage-mcp/local-evidence/full-20261025-jpeg-even-upsampling/coverage.json`:
93,362/139,140 lines (67.099324%), 18,508/32,020 branches (57.801374%),
5,201/8,903 functions (58.418511%), and 142,637/216,056 regions (66.018532%).
All 2,070 active decode rows, 486 active encode rows, 93 target-only fault
contracts, four animation-loop checks, and 69 feature-gate tests passed.
Coverage MCP reports no branch gap in the packed-tile row copier, either
bit-order implementation, the 2-by-2 converter, or the custom-coefficient 1-by-2
converter. It also reports no remaining branch gaps at the even-width YCCK
h2v1 and h2v2 final-sample checks in `src/codecs/jpeg/decode/upsample.rs:494`
and `:541`. It still reports the defensive zero-subsampling guard in the general
converter at `src/codecs/tiff/decode.rs:2434`; public TIFF validation rejects
unsupported pairs at `src/codecs/tiff/decode.rs:307` before this helper is called.
That private guard remains a documented coverage limitation, not a target-only
test target. Other TIFF gaps remain, including `decode_ifd`. The report source
identity is unverified and test attribution is unknown. The overall 100%
coverage objective remains open.
The report's fixture snapshot contained 2,587 total rows: 2,070 decode / inspect
/ verify rows and 517 encode rows. Of those, 2,070 decode rows and 486 encode
rows were active; 31 encode rows remained planned. Typed target-only fault
contracts are tracked separately from Pillow parity rows.

## AVIF mixed-lossless B32 residual parity — 2026-10-25

Added active parity row
`decode:avif:animated_mixed_lossless_inter_b32_i420_residual_64x64`. Its
deterministic 64×64 two-frame AVIF has a full-visible unsplit qindex-zero
inter B32 ROI whose 1,024 RGB pixels all change between frames. The fixture
SHA-256 is
`58552db6049a5220936acccbb483fd0366224744e7a2f81058a099b4016d5d36`; pinned
Pillow frame hashes are
`90ce523631e9fd385bcae414317ca29e655e9fa26a022e09d50795e71b32d634` and
`f99f8c1c96218f4dde446f18003ec53ae2c35e57a4f1e1b46ce406a2dbe6ef61`. The
native loop evidence confirms two frames and infinite repetition, normalized
to loop count zero. The exact selected parity row passed 1/1.

Coverage MCP still reports missing branch arms at
`src/codecs/avif/av1/block.rs:48152`. The selected report did not observe either
arm for this row, so it does not close that gap and no aggregate coverage gain
is claimed. MCP marks source identity unverified and test attribution unknown.
The current matrix contains 2,592 total rows: 2,074 decode / inspect / verify rows and 518 encode rows.
Of those, 2,074 decode rows and 487 encode rows are active; 31 encode rows
remain planned. The separate target-only fault lane contains 102 cases.

## TIFF compressed FillOrder 2 parity — 2026-10-25

Added three 17×13 grayscale TIFF parity rows covering Deflate, LZW, and
PackBits with `FillOrder=2`. Their complete fixture SHA-256 values are
`359ce018950a1959a1fc1f621e2313177817772bc42545346cd191b744d1e851`,
`688b5355bd642d870c6d8450fb943ad189d9dbd7b9df834127bdbbc3f060dae7`, and
`68b28cdf7af702becd02ccb328da7680593873563b51c14844c4305feef54924`.
All three decode to the same 221-byte grayscale payload with SHA-256
`b03d7423b9aaf79161be9a0f1a119cd3a814f4d81815f10b80aad8bdc2f5c4f4`; the
selected Pillow parity run passed 3/3.

The decoder now reverses bits in compressed FillOrder 2 block bytes before
decompression, using cancellation-aware chunks and fallible allocation. The
FillOrder 1 path continues to borrow the encoded input. The focused 23-row
FillOrder selection passed 23/23. Local LLVM recorded both outcomes of the
`compression == COMPRESSION_NONE` branch at
`src/codecs/tiff/decode.rs:254` (51 true and 12 false observations). Coverage
MCP returned no remaining missing location for that line in the selected
report at
`.coverage-mcp/local-evidence/selected-tiff-fillorder2-20261025/coverage.json`.
The complete TIFF decode selection passed 431/431 rows. Coverage MCP then
identified two remaining `decode_ifd` allocation-failure arms at lines 802 and
923; the corresponding existing target-only fault contracts passed 2/2, and
MCP returned no missing locations for those lines in
`.coverage-mcp/local-evidence/selected-tiff-fault-contracts-20261025/coverage.json`.
The 431-row report is
`.coverage-mcp/local-evidence/selected-tiff-parity-431-20261025/coverage.json`.
Comparison with the retained full report is incomparable because branch detail
is incomplete and the reported branch totals differ (32,020 versus 32,040).
The selected reports have unverified source identity and unknown test
attribution, so no aggregate coverage gain or full-suite closure is claimed.

The current matrix contains 2,592 total rows: 2,074 decode / inspect / verify
rows and 518 encode rows. TIFF has 431 decode rows. Of the total, 2,074 decode
rows and 487 encode rows are active; 31 encode rows remain planned.

## Focused AVIF monochrome and WebP token parity — 2026-10-25

After synchronizing the preserved AVIF work with merged main, the selected run
at
`.coverage-mcp/local-evidence/selected/20261024T225938.861678Z-7396731e19e64c819484d6e32561f3a7/coverage.json`
passed all 17 cases: four Pillow pixel-parity controls for portable AVIF
monochrome conversion, four WebP encode-parity rows using a fresh cancellation
token, and nine target-only AVIF fault contracts. The WebP rows produced exact
Pillow bytes; the new transparent-alpha case emitted 2,282 bytes and re-decoded
to the expected 128×128 RGBA pixels. The selected encoder made 20 public calls
with no panics. The nine fault rows returned the expected unsupported-subset
result and succeeded on an uninjected retry; they remain `not_applicable` to the
Pillow oracle and outside parity counts.

The current fixture matrix contains 2,588 total rows: 2,070 decode / inspect /
verify rows and 518 encode rows. Of those, 2,070 decode rows and 487 encode
rows are active; 31 encode rows remain planned. The separate target-only fault
lane contains 102 cases.

Coverage MCP reports missing branch coordinates in `decode_monochrome_portable`
and the test-only `inject_monochrome_portable_fault` guard. The local receipt
confirms 17 selected cases passed and that sources stayed unchanged during the
run, but MCP marks source identity unverified and test attribution unknown. The
coordinates are leads rather than source-verified closure; no aggregate
coverage gain is claimed. The broader 100% objective remains open.

## TIFF LZW cancellable decode parity — 2026-10-25

Enabled the existing cancellable decode API for the TIFF LZW parity fixtures.
This runs the same Pillow-backed inputs through both `decode` and
`decode_with_token`; it adds no Rust unit tests or duplicate fixtures. The
focused run passed 3/3 selected rows, and the latest retained full report passed
all 2,070 active decode rows, 486 active encode rows, 93 fault contracts, four
animation-loop checks, and 69 feature-gate tests. Aggregate coverage totals are
unchanged by the LZW token rows, so no gain is attributed to that API check.

Coverage MCP still reports the true arm of the `output.len() >= expected`
operand in `decode_lzw` and `decode_lzw_with_token` at
`src/codecs/tiff/decode.rs:3449` and `:3542`. A public TIFF cannot provide a
zero-row strip for this branch: image geometry is validated as positive, and
the decoder skips surplus strips with zero rows at `src/codecs/tiff/decode.rs:958`.
This remains a defensive private-path limitation rather than a parity case
target. Coverage MCP source identity remains unverified and test attribution
unknown.

## TIFF packed-tile bit-order dispatch — 2026-10-25

Coverage MCP previously reported opposite unobserved arms for `if LSB_FIRST`
inside the two boolean-const-generic packed-row specializations. A valid TIFF
input could select one specialization, but could never take the other
specialization's impossible arm. Replaced those conditionals with statically
dispatched `PackedTileBitOrder` implementations for most-significant-first and
least-significant-first samples. Their optimized per-sample methods are marked
`inline(always)` so orientation dispatch does not add calls to the inner loop.
The optimized AArch64 release assembly contains no emitted symbols for those
methods. This checks code generation for that target; it is not an end-to-end
benchmark or SSE2/AVX2 performance result.

The existing 14 Pillow parity rows for packed TIFF tiles passed, covering both
fill orders, aligned and unaligned copies, compressed tiles, and rejected
inputs. The selected report is
`.coverage-mcp/local-evidence/selected/20261024T194637.006124Z-b901994420c34603a3d1d2278d50e3fb/coverage.json`.
The full report above passes all 2,070 active decode rows, 486 active encode
rows, 93 fault contracts, four animation-loop checks, and 69 feature-gate
tests. Coverage MCP reports zero remaining branch groups for
`copy_packed_tile_row`. MCP source identity remains unverified and test
attribution unknown; no aggregate gain is attributed to these rows.

## TIFF custom-coefficient 2-by-2 odd-edge parity — 2026-10-25

Added active parity row
`decode:tiff:ycbcr_custom_coefficients_lzw_2x2_checkpoint_33x33_odd_edges`.
Its complete 2,453-byte input clips the final 2-by-2 YCbCr clump on both axes
and processes more than 1,024 pixels in the custom-coefficient converter.
Pinned Pillow 12.2.0 verifies the TIFF and returns 3,267 RGB bytes (SHA-256
`d6f82880d7c317dc0dab9a3a5c76fa388ac55fddafaabcf57e7a16419567902e`). The
input SHA-256 is
`9bf19211ac287e1d586cc0e2d037caff2dacfbfa6076bbe4e867bb64eda437f8`.
The deterministic generator validates those tags and pixels; the selected
public parity case passed 1/1.

The full report above adds no aggregate branch count, but Coverage MCP no longer
lists a missing branch group for the custom-coefficient 2-by-2 converter
specialization. Its full-report source identity remains unverified and test
attribution unknown, so this is recorded as a covered specialization rather
than an aggregate percentage gain. The report retains the full suite's
configured floors and includes the new row.

## TIFF custom-coefficient 1-by-2 checkpoint parity — 2026-10-25

Added active parity row
`decode:tiff:ycbcr_custom_coefficients_lzw_1x2_checkpoint_33x33_odd_edges`.
The 33×33 TIFF uses custom Rec. 709 coefficients, clips the odd final row, and
reaches the converter's 1,024-pixel cancellation checkpoint. Pinned Pillow
12.2.0 returns 3,267 RGB bytes (SHA-256
`4dbccd036ff2c6e28e6986c751ea075a6ba4d29345109390a131cbf325730100`). The
input SHA-256 is
`fa1f4ca256be97295b75647cd992f879778b41b30651203518ad5d093812a872`. The exact
selected parity row passed 1/1. Coverage MCP reports zero remaining branch
groups for `convert_ycbcr_subsampled_1x2_with` in the retained full report.
Aggregate coverage totals are unchanged; no percentage gain is claimed.

Coverage MCP still reports the general converter's zero-subsampling guard at
`src/codecs/tiff/decode.rs:2434` for the default and custom monomorphizations.
The public decoder rejects unsupported subsampling pairs before calling the
converter, and the existing zero-pair Pillow case verifies that public error.
Per the parity policy, no extra case was added solely to reach this private
defensive guard. The report's source identity is unverified and test
attribution is unknown.

## TIFF custom-table YCbCr fault contracts — 2026-10-25

Added four target-only fault rows for clump-length mismatch and RGB output
reservation failure in the custom general and custom 1-by-2 converter paths.
They reuse the active Pillow inputs
`decode:tiff:ycbcr_custom_coefficients_lzw_4x2` and
`decode:tiff:ycbcr_custom_coefficients_lzw_1x2_odd_height`. The selected run at
`.coverage-mcp/local-evidence/selected/20261024T201532.328967Z-cbfbf8bca61a4755bd61969194807191/coverage.json`
reported 4 selected, 4 executed, 4 passed, 0 failed, with
`oracle=not_applicable`. These contracts verify stable public errors and retry
success; they remain separate from Pillow parity counts. An incremental
comparison to the previous full report was incomparable because its branch
detail did not reconcile with its totals, so no aggregate coverage gain is
attributed to these fault rows.

## AVIF 12-bit I420 alpha sample-count fault contract — 2026-10-25

Added target-only row
`fault-contract:avif:avif_12bit_i420_alpha_sample_count_mismatch`, reusing the
active odd-edge 17×9 Pillow parity input. The coverage-only injection shortens
the reconstructed alpha slice before the specialized 12-bit I420 converter;
the public decoder returns the existing unsupported-subset error and a retry
without the fault succeeds with the Pillow reference. The selected run passed
1/1. Coverage MCP no longer lists the alpha-length mismatch arm at
`src/codecs/avif/decode.rs:950`; zero-width and remainder guards in that
condition remain uncovered. The injection is excluded from release builds,
so this coverage change does not add a production-path branch. The full report
above includes the new contract. MCP source identity remains unverified and
test attribution unknown.

## TIFF surplus-strip parity — 2026-10-25

Added active parity row
`decode:tiff:ycbcr_subsampled_lzw_2x2_unaligned_rows_per_strip_3_predictor_2_extra_strip`.
The complete TIFF declares three LZW strips for 4x4 image geometry that needs
two; the third entry points to a valid duplicate strip. Pillow 12.2.0 opens,
verifies, and decodes the image to the same 48-byte RGB output as the two-strip
control (`ee406bf055bf628b9b0a3b6d3ea29a5f32d97e963d3eb4964d5942c37f04881a`).
The exact selected parity row passed 1/1. Its selected LLVM report contains
positive true/false counters for `strip_rows == 0`; the full report has no
remaining Coverage MCP branch gap at that source line. The selected-only MCP
query still grouped an unexecuted decoder instantiation at the same location,
so the full report is the evidence that closes the grouped gap. The
incremental comparison against the prior report was incomparable because its
branch detail did not reconcile with the reported totals; no marginal gain is
claimed from that comparison.

The unaligned subsampled-strip assembler receives only compressed
`YcbcrSubsampled8` layouts; uncompressed YCbCr uses `Ycbcr8`. The redundant
`compression == COMPRESSION_NONE` decode arm was removed, with no change to the
compressed path. Runtime benchmark evidence was not collected, so no speedup is
claimed. Coverage MCP marks the full report's source identity unverified and
test attribution unknown.

## TIFF unaligned strip error parity — 2026-10-25

Added active parity row `decode:tiff:ycbcr_subsampled_lzw_2x2_unaligned_rows_per_strip_3_predictor_2_zero_first_byte_count` for a zero first `StripByteCounts` entry. Pillow opens and verifies the metadata, then reports `OSError("decoder error -2")` during pixel loading. The target previously decoded this file, so the unaligned compressed-strip path now rejects the explicitly zero count; the separate fallback for a missing count array is unchanged. The selected parity row passed 1/1.

Added active parity row `decode:tiff:ycbcr_subsampled_uncompressed_2x2_unaligned_rows_per_strip_3` for odd-height uncompressed 2-by-2 YCbCr strips with a three-row strip boundary. Pillow verifies metadata and rejects pixel loading with `OSError("image file is truncated (4 bytes not processed)")`; the target matches the error contract. Its selected parity row passed 1/1. The case adds a distinct malformed-input contract but did not change aggregate coverage counts.

Added active parity row `decode:tiff:ycbcr_subsampled_lzw_2x2_unaligned_rows_per_strip_3_predictor_2_oob_first_offset` with the first strip offset set beyond the complete input. Pillow opens and verifies the metadata, then reports `OSError("decoder error -2")` during pixel loading; the selected parity row passed 1/1. The updated full report no longer has a branch gap at `src/codecs/tiff/decode.rs:817`. Its aggregate increased by one branch, four lines, and two regions, although Coverage MCP cannot attribute those observations to a named test. The false arm at line 804 remains open.

The `compression != COMPRESSION_NONE` test in the unaligned YCbCr assembly loop was redundant: `inspect.rs` only supplies subsampling parameters when compression is enabled, and maps uncompressed YCbCr to `Ycbcr8`. Removing that impossible arm reduces two branch edges and one covered edge without changing the reachable TIFF contract; the zero-count rejection and offset-boundary parity cases still pass. Coverage MCP now reports no branch gaps at lines 804 or 817.

## TIFF predictor edge parity — 2026-10-24

Added active parity rows `decode:tiff:ycbcr_subsampled_uncompressed_predictor_3`
and `decode:tiff:ycbcr_subsampled_predictor_2_jpeg_compression_error`. The first
pins Pillow's 2x2 raw YCbCr interpretation for an uncompressed Predictor=3
directory (input SHA-256
`601a3c6a6ce74cbfdd92c442efc0901f8d77f6a13f2a926e3aa261a53621d6a6`; pixel
SHA-256 `b122cbf873524577a50522805777fb18d2fa2434b53a8bb6c0c0236ec118e80f`).
The second changes the compression tag of the LZW Predictor=2 control to TIFF
JPEG compression tag 7 (input SHA-256
`c692b5925e26200fe58d9679b6b176313560f602939c749d9ad10ec790c4499c`). Metadata
inspection and verification succeed, while decode rejects the unsupported
compression/predictor combination as Pillow does. The two-row selected parity
run passed 2/2; the full coverage run also passed the complete 2,578-row public
matrix and all configured floors. The TIFF predictor guard at line 337 has no
remaining missing branch observations in the refreshed full report.

## TIFF compressed tile byte-count omission — 2026-10-24

Added active parity row `decode:tiff:tiled_deflate_without_byte_counts` for a
complete Deflate-tiled RGB16 TIFF whose directory omits `TileByteCounts`. Pillow
opens and verifies the header, then returns `OSError("decoder error -2")` when
loading pixels. Input SHA-256:
`ed0b0e058f8fb8a7113c498f8a7a063d7f2ff28f9e45aa4bb33b63019cf86a7f`. TIFF
inspection now accepts this metadata shape; decode rejects the missing
compressed tile lengths. The selected parity row passed 1/1 at
`.coverage-mcp/local-evidence/selected/tiff-compressed-tile-missing-counts-20261024/coverage.json`.
The refreshed full report includes this case and passes the configured coverage
floors. Coverage MCP's full-report comparison identified one newly observed
branch at `src/codecs/tiff/decode.rs:398`; source/build identity is unverified,
so that coordinate change does not establish an aggregate gain or regression.

## JPEG even-width YCCK upsample parity — 2026-10-25

Added active Pillow parity rows
`decode:jpeg:ycck_h2v1_even_width_18x9` and
`decode:jpeg:ycck_h2v2_even_upsample_edges_18x10`. Each starts from a complete,
pinned 17×9 libjpeg-turbo YCCK input and changes only the SOF dimensions to
expose the padded final MCU samples at an even output edge. Pillow 12.2.0
verifies and decodes both inputs as CMYK. The 18×9 h2v1 input SHA-256 is
`3aa91b3831f8b82d66d978cc217ac4a00953c5b40e9c51d46eb93fdad9feb922`, with
pixel SHA-256
`cfd5480dc9878c522f1fad89048331ff773c45d871a893223d743d3b909c9f45`. The
18×10 h2v2 input SHA-256 is
`ad0e70ac2a81695d5a41379d8b9931d1333330514b0a1a930c0e835b9aa5a8c8`, with
pixel SHA-256
`e580c178a9dee525ef95849529c3f4ca0a7e1238cc77027d4133732fe78ea2a6`. The
focused parity run passed 2/2, and the refreshed full report passed all 2,070
active decode rows.

Coverage MCP's full-report comparison reports two newly observed branch
coordinates at the final-sample checks in
`src/codecs/jpeg/decode/upsample.rs:494` and `:541`; targeted gap queries now
return no missing observations at either line. The report totals increased by
two branches, two lines, and four regions with unchanged denominators. MCP
marks source identity unverified and test attribution unknown, so the
coordinate comparison is not a source-verified or named-test attribution.

## JPEG reserved post-scan marker parity — 2026-10-24

Added active parity row
`decode:jpeg:baseline_444_single_scan_reserved_jpg0` for an 810-byte complete
JPEG with a reserved JPG0 segment after a valid trailing DQT. The pinned input
SHA-256 is
`8cac78f1ec5ba03d629ac828e27d12eb579084476639c90155069457d2728c55`; its
standalone generator also checks the Pillow lifecycle. Pillow inspection and
structural verification succeed, while decode and sequence decode return the
`broken data stream when reading image file` error. The selected parity run
passed 1/1 at
`.coverage-mcp/local-evidence/selected/20261024T163312.730669Z-9747674b11f040a28c6ac17ecb02fbae/coverage.json`.
Its LLVM report records both outcomes at
`src/codecs/jpeg/decode/decode.rs:2890` (true: 3, false: 3). Coverage MCP's
selected-report gap lookup still lists both arms; source identity is unverified
and test attribution is unknown, so no aggregate or marginal coverage gain is
claimed. The full coverage snapshot above includes this row; named-test
attribution remains unavailable.

## AVIF clipped derived-skip compound parity — 2026-10-24

Added active parity row
`decode:avif:animated_lossless_inter_420_derived_skip_refs_clipped_b32x32_56x64`
for a deterministic five-frame, 56x64 lossless 8-bit 4:2:0 sequence. The
fixed-B32 sample has a clipped 24x32 right-edge grid; its pinned input SHA-256
is `b801ab1e84328dfe552543858fad84a59c7edeb7ac7497972c2499b0736d6eb5`.
Pillow 12.2.0 frame references and independent loop evidence from libavif
`6543b22b5bc706c53f038a16fe515f921556d9b3` were generated and validated.
The exact selected parity case passed 1/1, including all five frames' pixels,
at `.coverage-mcp/local-evidence/selected/20261024T160814.722483Z-fb991bd6146846538984b6647018796f/coverage.json`.

That raw LLVM report records three calls through
`decode_inter_compound_translation_lossless_grid`; the compound specialization
records both outcomes of the `lossless_partial_grid` predicate at
`src/codecs/avif/av1/block.rs:48160` (true: 1, false: 2). Coverage MCP still
reports uncovered generic instantiations at the same source coordinate. Its
incremental comparison with `target/release-evidence/coverage.json` was
incomparable because the baseline lacks compatible exact branch detail; its
source identity is unverified and test attribution is unknown. No aggregate
coverage gain is claimed from that comparison.

The public fixture matrix now contains 2,576 total rows: 2,059 decode / inspect /
verify rows and 517 encode rows. Of those, 2,059 decode rows and 486 encode
rows are active; 31 encode rows remain planned. The separate target-only fault
lane contains 88 cases.

## TIFF subsampled YCbCr Predictor=2 parity — 2026-10-24

Added twelve active parity rows for Predictor=2 with LZW, Deflate, and Adobe
Deflate compression over 2-by-1, 4-by-1, 4-by-2, and 4-by-4 YCbCr clumps.
Pillow 12.2.0 decodes each fixture to an exact 192-byte RGB reference. The rows
are
`decode:tiff:ycbcr_subsampled_lzw_2x1_predictor_2`,
`decode:tiff:ycbcr_subsampled_lzw_4x1_predictor_2`,
`decode:tiff:ycbcr_subsampled_lzw_4x2_predictor_2`, and
`decode:tiff:ycbcr_subsampled_lzw_4x4_predictor_2`,
`decode:tiff:ycbcr_subsampled_deflate_2x1_predictor_2`,
`decode:tiff:ycbcr_subsampled_deflate_4x1_predictor_2`,
`decode:tiff:ycbcr_subsampled_deflate_4x2_predictor_2`,
`decode:tiff:ycbcr_subsampled_deflate_4x4_predictor_2`,
`decode:tiff:ycbcr_subsampled_adobe_deflate_2x1_predictor_2`,
`decode:tiff:ycbcr_subsampled_adobe_deflate_4x1_predictor_2`,
`decode:tiff:ycbcr_subsampled_adobe_deflate_4x2_predictor_2`, and
`decode:tiff:ycbcr_subsampled_adobe_deflate_4x4_predictor_2`. Before the
decoder change, the four new LZW rows failed with
`unsupported TIFF subsampled YCbCr storage layout`. The decoder now accepts
Pillow-supported predictor/compression combinations and follows libtiff's
complete three-sample accumulator groups. The LZW selected run passed 4/4 at
`.coverage-mcp/local-evidence/selected/tiff-ycbcr-predictor2-final-20261024/coverage.json`;
the eight Deflate and Adobe Deflate rows passed 8/8 at
`.coverage-mcp/local-evidence/selected/tiff-ycbcr-deflate-predictor2-20261024/coverage.json`.
The cases cover scanline strides both divisible and not divisible by three.
The refreshed full report includes all twelve rows and passed the configured
coverage floors. Coverage MCP returned unverified source identity and unknown
test attribution for the selected reports, so no per-case aggregate gain is
claimed.

## TIFF RGB16 ExtraSamples rejection parity — 2026-10-24

Added active parity row
`decode:tiff:error_rgb16_extra_sample_without_fourth_channel` for a complete
188-byte big-endian RGB16 TIFF declaring one `ExtraSamples` value with only
three samples per pixel. Pillow 12.2.0 rejects it during opening. Fixture
SHA-256:
`086d099af708225e89e39ab59ab2e62576636da3e3083f3bb606bdffd4a2975a`. It
passed with the mixed and signed `SampleFormat` rejection cases and unsigned
RGB16 controls in a 7/7 selected parity run at
`.coverage-mcp/local-evidence/selected/tiff-rgb16-layout-edges-20261024/coverage.json`.

The public fixture matrix now contains 2,562 total rows: 2,045 decode / inspect /
verify rows and 517 encode rows. Of those, 2,045 decode rows and 486 encode
rows are active; 31 encode rows remain planned. The separate target-only fault
lane contains 88 cases.

## TIFF RGB16 contiguous tiled Deflate parity — 2026-10-24

Added active parity row
`decode:tiff:rgb16_contiguous_tiled_deflate_edges` for a 17x17 RGB16 TIFF in
four contiguous Deflate tiles, including clipped right and bottom edges.
Pillow 12.2.0 returns the exact 867 RGB bytes. The 2,015-byte fixture is pinned
at SHA-256
`3b29709ca7a616ada00ca7ca800378fed328f7353d6574ecb8c30181145d2031`. The
target now admits contiguous Deflate tiles through its existing tile decoder;
the selected case and planar-tile/strip controls passed 4/4 at
`.coverage-mcp/local-evidence/selected/tiff-rgb16-contiguous-tiled-deflate-20261024/coverage.json`.

The public fixture matrix now contains 2,561 total rows: 2,044 decode / inspect /
verify rows and 517 encode rows. Of those, 2,044 decode rows and 486 encode
rows are active; 31 encode rows remain planned. The separate target-only fault
lane contains 88 cases.

## TIFF RGB16 mixed SampleFormat rejection parity — 2026-10-24

Added active parity row `decode:tiff:error_rgb16_mixed_sample_format` for a
complete 188-byte big-endian RGB16 TIFF whose `SampleFormat` values are
`[1, 2, 1]`. Pillow 12.2.0 rejects this file during opening. Fixture SHA-256:
`e6c535b10127e845fa911f375f7e88bfefa4a39a0dd9f1bff6dcfb95b3649b55`. The new
row, the all-signed rejection case, and two unsigned RGB16 controls passed
4/4 at
`.coverage-mcp/local-evidence/selected/tiff-mixed-sampleformat-20261024/coverage.json`.
This exercises the mixed-channel rejection in `layout_and_palette`; selected
coverage does not establish aggregate gains.

The public fixture matrix now contains 2,560 total rows: 2,043 decode / inspect /
verify rows and 517 encode rows. Of those, 2,043 decode rows and 486 encode
rows are active; 31 encode rows remain planned. The separate target-only fault
lane contains 88 cases.

## TIFF RGB16 uncompressed Predictor 3 parity — 2026-10-24

Added active parity row `decode:tiff:rgb16_le_uncompressed_predictor3_strips`
for a deterministic 180-byte little-endian RGB16 TIFF. Pillow 12.2.0 ignores
Predictor 3 with uncompressed strips and returns the exact expected 12 RGB
bytes. Fixture SHA-256:
`1528a4d7a519f6a2f1b24c0f88cdee98b566d813533621cdf6971fd8f4aedec9`.
The selected run passed five TIFF parity rows and three separate JPEG
target-only fault contracts (8/8 total) at
`.coverage-mcp/local-evidence/selected/tiff-predictor3-jpeg-fault-20261024/coverage.json`.
The three fault contracts remain outside Pillow parity counts. Coverage MCP
reports only selected-run coordinates; its source identity is unverified and
test attribution is unknown, so no aggregate coverage gain is claimed.

Uncompressed TIFF decoding now ignores Predictor values as Pillow does;
compressed inputs retain the existing supported Predictor 1 and 2 checks.

The public fixture matrix now contains 2,559 total rows: 2,042 decode / inspect /
verify rows and 517 encode rows. Of those, 2,042 decode rows and 486 encode
rows are active; 31 encode rows remain planned. The separate target-only fault
lane contains 88 cases.

## TIFF RGB16 Deflate FillOrder 2 rejection parity — 2026-10-24

Added active parity row `decode:tiff:error_rgb16_deflate_fill_order_2` for a
complete 196-byte RGB16 Deflate TIFF with `FillOrder=2`. Pillow 12.2.0 rejects
it during opening. Fixture SHA-256:
`36a010a79dfeacec61408c2535b19eb7bdedbca460057604c407c2929d35dcbb`. The new
case and the FillOrder 1 RGB16 Deflate control passed together (2/2) at
`.coverage-mcp/local-evidence/selected/tiff-rgb16-fill-order2-20261024/coverage.json`.
Raw LLVM records both outcomes at `layout_and_palette` line 391. Coverage MCP's
selected-to-full comparison is incomparable because exact branch detail is
incomplete; its source identity is unverified and test attribution is unknown.
No aggregate coverage gain is claimed.

The public fixture matrix now contains 2,557 total rows: 2,040 decode / inspect /
verify rows and 517 encode rows. Of those, 2,040 decode rows and 486 encode
rows are active; 31 encode rows remain planned. The separate target-only fault
lane contains 85 cases.

## TIFF RGB16 Deflate horizontal-predictor parity — 2026-10-24

Added active parity row
`decode:tiff:rgb16_le_deflate_predictor2_contiguous_strips`. Its deterministic
196-byte TIFF uses two contiguous Deflate strips with 16-bit RGB horizontal
differencing (Predictor 2); Pillow 12.2.0 decodes the exact expected 12 RGB
bytes. Fixture SHA-256:
`37fc7bcedd0f550626fab36b6cfa772f925aa33e1b1bebb4c4f3e832fe4c978e`. The new
case and the Predictor 1 control passed together (2/2) at
`.coverage-mcp/local-evidence/selected/tiff-rgb16-deflate-predictor2-20261024/coverage.json`.
Coverage MCP still reports unobserved groups in `layout_and_palette`; source
identity is unverified and test attribution is unknown, so this selected run
does not establish an aggregate coverage gain.

The decoder now admits Predictor 2 for Deflate-compressed RGB16 strips. It uses
the existing horizontal-predictor reversal path; no per-sample conversion loop
was added.

The public fixture matrix now contains 2,556 total rows: 2,039 decode / inspect /
verify rows and 517 encode rows. Of those, 2,039 decode rows and 486 encode
rows are active; 31 encode rows remain planned. The separate target-only fault
lane contains 85 cases.

## TIFF signed RGB16 SampleFormat parity — 2026-10-24

Added active parity row `decode:tiff:error_rgb16_signed_sample_format` for a
complete 188-byte RGB16 TIFF whose `SampleFormat` tags declare signed samples.
The fixture is pinned at SHA-256
`acf7ac21e19739826a6f4783d36150ff139dd70a632b6223f55cee23f3fac142`; the
fixture generator verifies that Pillow 12.2.0 rejects it while opening. The
new row and the unsigned RGB16 control passed the selected parity run 2/2 at
`.coverage-mcp/local-evidence/selected/20261024T141450.184628Z-861a69d552174765889f4646202f9212/coverage.json`.
Coverage MCP returned no missing groups for the selected
`layout_and_palette` query. The report contains 173/32,010 observed branches;
source identity is unverified and test attribution is unknown, so it does not
support an aggregate coverage claim.

The TIFF YCbCr rational limit check now follows the parser's unsigned `u32`
denominator contract and widens values before comparing them. Five existing
YCbCr boundary and SampleFormat parity rows passed after this change.

The public fixture matrix now contains 2,555 total rows: 2,038 decode / inspect /
verify rows and 517 encode rows. Of those, 2,038 decode rows and 486 encode
rows are active; 31 encode rows remain planned. The separate target-only fault
lane contains 85 cases.

## AVIF nonzero auxC FullBox version parity — 2026-10-24

Added active parity row `decode:avif:error_auxc_nonzero_version`. The
deterministic fixture changes only the `meta/iprp/ipco/auxC` FullBox version
in `alpha.avif`, preserving its complete AV1 payload and 6,441-byte file size.
Pinned Pillow 12.2.0 rejects the complete input at open with
`PIL.UnidentifiedImageError: cannot identify image file <bytes>`. The fixture
SHA-256 is
`2357cdc2bc511390a97c6f84cdf1bbf03b0debe3b7bda4090b397d7bd20c3822`.

The focused public parity and coverage command passed 1/1 at
`.coverage-mcp/local-evidence/selected/avif-auxc-nonzero-version-20261024/coverage.json`.
Raw LLVM records six visits to the nonzero-version arm in
`src/codecs/avif/samples.rs::parse_property` at line 734. The full report before
this row recorded zero visits to that arm and 340 visits to the version-zero
arm; the refreshed full report records six and 340 visits, respectively.
Coverage MCP confirms no remaining gap at line 734. Its full comparison reports
one newly observed branch coordinate but remains limited because report
source/build identities and test attribution are unverified. No aggregate gain
is claimed from that comparison.

The refreshed full parity coverage is 93,218/139,046 lines (67.0411%),
18,488/32,028 branches (57.7245%), 5,189/8,891 functions (58.3624%), and
142,454/215,942 regions (65.9686%).

The public fixture matrix now contains 2,554 total rows: 2,037 decode / inspect /
verify rows and 517 encode rows. Of those, 2,037 decode rows and 486 encode
rows are active; 31 encode rows remain planned. The separate target-only fault
lane contains 85 cases.

## AVIF nonzero pixi FullBox version parity — 2026-10-24

Added active parity row `decode:avif:error_pixi_nonzero_version`. The
deterministic fixture changes only the baseline still's `pixi` FullBox version
from zero to one, preserving the complete AV1 payload and 3,077-byte file size.
Pinned Pillow 12.2.0 rejects the complete input at open with
`PIL.UnidentifiedImageError: cannot identify image file <bytes>`. The fixture
SHA-256 is
`a50bd4126788de5efdeefa05ec6583267311fe021af9168928850589d9fd5689`.

The focused public parity and coverage command passed 1/1 at
`.coverage-mcp/local-evidence/selected/avif-pixi-nonzero-version-20261024/coverage.json`.
Raw LLVM records six visits to the nonzero-version arm in
`src/codecs/avif/samples.rs::parse_property` at line 699. The full report before
this row recorded zero visits to that arm and 243,967 visits to the version-zero
arm; the refreshed full report records six and 243,967 visits, respectively.
Coverage MCP confirmed the arm is now covered in the refreshed report. Its
incremental comparison is `incomparable` because the baseline's exact branch
detail is incomplete and report source/test receipts are unverified. No
aggregate gain is claimed from that comparison.

The refreshed full parity coverage is 93,217/139,046 lines (67.0404%),
18,487/32,028 branches (57.7214%), 5,189/8,891 functions (58.3624%), and
142,453/215,942 regions (65.9682%).

The public fixture matrix now contains 2,553 total rows: 2,036 decode / inspect /
verify rows and 517 encode rows. Of those, 2,036 decode rows and 486 encode
rows are active; 31 encode rows remain planned. The separate target-only fault
lane contains 85 cases.

## AVIF zero-plane pixi parity — 2026-10-24

Added active parity row `decode:avif:error_pixi_zero_planes`. The deterministic
fixture changes only the baseline still's `pixi` plane count from three to zero,
preserving the complete AV1 payload and 3,077-byte file size. Pillow 12.2.0
reports `builtins.RuntimeError: Failed to decode image: Not implemented`; the
public parity runner confirms Rust and Pillow both reject the input as
malformed. The fixture SHA-256 is
`b29a15defd39037aebaa7dffdd878c828201d1d9adc852302e8c76b123036431`.

The focused parity and coverage run passed 1/1 at
`.coverage-mcp/local-evidence/selected/avif-pixi-zero-planes-20261024/coverage.json`.
Raw LLVM records six visits to the invalid plane-count outcome in
`src/codecs/avif/samples.rs::parse_property` at line 703; the saved full report
recorded zero visits to that outcome and 243,955 visits to the valid plane-count
outcome. Coverage MCP returned `incomparable` for the incremental comparison
because the baseline's exact branch detail is incomplete. No aggregate
coverage gain is claimed.

The public fixture matrix now contains 2,552 total rows: 2,035 decode / inspect /
verify rows and 517 encode rows. Of those, 2,035 decode rows and 486 encode
rows are active; 31 encode rows remain planned. The separate target-only fault
lane contains 85 cases.

## AVIF over-16 pixi depth parity — 2026-10-24

Added active parity row `decode:avif:error_pixi_depth_above_16`. The
deterministic fixture changes only the baseline still's first `pixi` channel
depth from eight to 17, preserving the complete AV1 payload and 3,077-byte file
size. Pillow 12.2.0 reports
`builtins.RuntimeError: Failed to decode image: Not implemented`; the public
parity runner confirms Rust and Pillow both reject the input as malformed. The
fixture SHA-256 is
`bdf5ecf32956f1c05ed93c9b3e5a9fb2381f5429c1996ccb6b5701201fdbd34d`.

The focused parity and coverage run passed 1/1 at
`.coverage-mcp/local-evidence/selected/avif-pixi-depth-above-16-20261024/coverage.json`.
Raw LLVM records six visits to the over-limit depth outcome in
`src/codecs/avif/samples.rs::parse_property` at line 707. The refreshed full
baseline recorded zero visits to that outcome and 243,949 visits to the in-range
depth outcome. Coverage MCP returned `incomparable` for the incremental
comparison because the baseline's exact branch detail is incomplete. No
aggregate coverage gain is claimed.

The public fixture matrix now contains 2,551 total rows: 2,034 decode / inspect /
verify rows and 517 encode rows. Of those, 2,034 decode rows and 486 encode
rows are active; 31 encode rows remain planned. The separate target-only fault
lane contains 85 cases.

## AVIF mismatched pixi channel-depth parity — 2026-10-24

Added active parity row `decode:avif:error_pixi_channel_depth_mismatch`. The
deterministic fixture changes only the second of three channel-depth entries
in the baseline still's `pixi` property, preserving the complete AV1 payload and
3,077-byte file size. Pillow 12.2.0 reports
`builtins.RuntimeError: Failed to decode image: Not implemented`; the public
parity runner confirms Rust and Pillow both reject the input as malformed. The
fixture SHA-256 is
`420568bd6387e4262a2b3076a466b0d39efe35f9c01c99a66364352389b92242`.

The focused parity and coverage run passed 1/1 at
`.coverage-mcp/local-evidence/selected/avif-pixi-channel-depth-mismatch-20261024/coverage.json`.
Raw LLVM records six visits to the mismatched-channel outcome in
`src/codecs/avif/samples.rs::parse_property` at line 711. The refreshed full
baseline recorded zero visits to that outcome and 487,078 visits to the matching
channel-depth outcome. Coverage MCP returned `incomparable` for the incremental
comparison because the baseline's exact branch detail is incomplete. No
aggregate coverage gain is claimed.

The public fixture matrix now contains 2,550 total rows: 2,033 decode / inspect /
verify rows and 517 encode rows. Of those, 2,033 decode rows and 486 encode
rows are active; 31 encode rows remain planned. The separate target-only fault
lane contains 85 cases.

## AVIF zero-depth pixi parity — 2026-10-24

Added active parity row `decode:avif:error_pixi_zero_depth`. The deterministic
fixture sets only the first channel depth in the baseline still's `pixi`
property to zero, preserving the complete AV1 payload and 3,077-byte file size.
Pillow 12.2.0 rejects the full input at open with
`PIL.UnidentifiedImageError: cannot identify image file <bytes>`. Its fixture
SHA-256 is
`7c815a023acc909df7a21d80d11aa1ca92aaae6ce0748bfc989288cae91d3c5f`.

The focused parity and coverage run passed 1/1 at
`.coverage-mcp/local-evidence/selected/avif-pixi-zero-depth-20261024/coverage.json`.
Raw LLVM records six visits to the invalid-depth outcome in
`src/codecs/avif/samples.rs::parse_property` at line 707. The saved full report
recorded zero visits to that outcome and 3,511 visits to the valid-depth
outcome; its source/build receipt is unverified. Coverage MCP therefore
returned `incomparable` for the incremental comparison because the baseline's
exact branch detail is incomplete. No aggregate coverage gain is claimed.

The public fixture matrix now contains 2,549 total rows: 2,032 decode / inspect /
verify rows and 517 encode rows. Of those, 2,032 decode rows and 486 encode
rows are active; 31 encode rows remain planned. The separate target-only fault
lane contains 85 cases.

## AVIF handler rejection parity — 2026-10-24

Added active parity rows `decode:avif:error_meta_hdlr_nonzero_version` and
`decode:avif:error_meta_hdlr_nonzero_predefined`. Each fixture mutates only one
field in the baseline `meta/hdlr` box and preserves the complete AV1 payload and
3,077-byte file size. Pillow 12.2.0 rejects both complete inputs at open with
`PIL.UnidentifiedImageError: cannot identify image file <bytes>`. Their fixture
SHA-256 values are `e5d7f45c33cc3aec2868d57bddaad27b62df23210326822382f81e2cb344dc62`
and `b9119779779df0addfdfcd92927ae06127109846796ca6806c1a0bc20ce7391d`.

The focused parity and coverage run passed 2/2 rows at
`.coverage-mcp/local-evidence/selected/avif-handler-errors-20261024/coverage.json`.
Its raw LLVM records six visits to each rejection outcome in
`src/codecs/avif/samples.rs::parse_handler` at line 542. The saved full report
`.coverage-mcp/local-evidence/parity-full-20261024-after-iloc-reserved/coverage.json`
records 4,051 visits to each corresponding normal outcome, but its source/build
receipt is unverified. Coverage MCP therefore returned `incomparable` for the
incremental comparison because the baseline's exact branch detail is
incomplete. No aggregate coverage gain is claimed.

The public fixture matrix now contains 2,548 total rows: 2,031 decode / inspect /
verify rows and 517 encode rows. Of those, 2,031 decode rows and 486 encode rows
are active; 31 encode rows remain planned. The separate target-only fault lane
contains 85 cases.

## AVIF duplicate idat parity — 2026-10-24

Added active parity row `decode:avif:error_meta_duplicate_idat`. The
deterministic generator appends an empty second `idat` box to the pinned
indexed-`idat` still, growing the complete file from 5,906 to 5,914 bytes while
preserving the original AV1 payload. The fixture SHA-256 is
`466e3ecd6e89e4cbd2dba8084642fc2ddbda83e196e005819a31aa54cc3c960b`. Pillow
12.2.0 rejects the complete input at open with
`PIL.UnidentifiedImageError: cannot identify image file <bytes>`.

The selected parity coverage command passed 1/1 at
`.coverage-mcp/local-evidence/selected/avif-duplicate-idat-20261024/coverage.json`.
Raw LLVM records four and two visits to the `idat.is_some()` true arm at
`src/codecs/avif/samples.rs:522` across `parse_meta::<false>` and
`parse_meta::<true>`. Coverage MCP's incremental comparison is incomparable
because the full baseline's exact branch detail is incomplete; no aggregate
coverage gain is claimed.

The public fixture matrix now contains 2,546 total rows: 2,029 decode / inspect /
verify rows and 517 encode rows. Of those, 2,029 decode rows and 486 encode
rows are active; 31 encode rows remain planned. The separate target-only fault
lane contains 85 cases.

## AVIF version-two infe zero item identifier parity — 2026-10-24

Added active parity row `decode:avif:error_infe_zero_item_id`. The deterministic
generator sets the baseline still's version-two `infe` item identifier to zero
inside `meta/iinf`, preserving the 3,077-byte file and complete AV1 media
payload. The fixture SHA-256 is
`9cd1623b8c3cf615b708968e90f6b9bb0fb081dbda2185348ebf087db82e7e7f`. Pinned
Pillow 12.2.0 rejects it at open with
`PIL.UnidentifiedImageError: cannot identify image file <bytes>`.

The selected parity coverage command passed 1/1 at
`.coverage-mcp/local-evidence/selected/avif-infe-zero-item-id-20261024/coverage.json`.
Coverage investigation isolated the zero-ID rejection in
`src/codecs/avif/samples.rs:617`; raw LLVM records six visits to the taken arm
for this selected case. The prior full-parity report has the same source hash
for `samples.rs` and recorded zero hits to that arm. Coverage MCP's incremental
comparison is incomparable because exact branch detail is incomplete, so no
aggregate coverage gain is claimed.

The public fixture matrix now contains 2,546 total rows: 2,029 decode / inspect /
verify rows and 517 encode rows. Of those, 2,029 decode rows and 486 encode
rows are active; 31 encode rows remain planned. The separate target-only fault
lane contains 85 cases.

## AVIF version-one pitm wide item identifier parity — 2026-10-24

Added active parity row `decode:avif:pitm_version_1_wide_item_id`. The
deterministic generator upgrades the pinned indexed-`idat` still's `pitm`
primary item field from version zero's 16-bit form to version one's 32-bit form,
retaining item ID 1. The file grows by two bytes while preserving the full
encoded media payload; its SHA-256 is
`985e27b0dbaa32a2835d940ac0729db4b43addf80d207f4598b11d518719874c`. Pinned
Pillow 12.2.0 output is unchanged at 49,152 bytes of RGB pixel parity.

The selected parity row passed 1/1 at
`.coverage-mcp/local-evidence/selected/avif-pitm-v1-wide-item-id-20261024/coverage.json`.
Raw LLVM at `src/codecs/avif/samples.rs:554` shows eight visits to the
version-one 32-bit branch in `parse_pitm`. Coverage MCP had identified that
arm as uncovered in the full parity report. Its local reports have unverified
source/build identity and unknown test attribution; incremental comparison is
incomparable because the full report's exact branch detail is incomplete. No
aggregate coverage gain is claimed.

The public fixture matrix now contains 2,544 total rows: 2,027 decode / inspect /
verify rows and 517 encode rows. Of those, 2,027 decode rows and 486 encode
rows are active; 31 encode rows remain planned. The separate target-only fault
lane contains 85 cases.

## AVIF reserved iloc construction-method bit parity — 2026-10-24

Added active parity row
`decode:avif:error_iloc_reserved_construction_method_bits`. The deterministic
generator sets one reserved bit in the `iloc` construction method of the pinned
indexed-`idat` still. The 5,906-byte file size and complete encoded media payload
are preserved; the fixture SHA-256 is
`14045959d327207b120b345879a1c83a6a0cb252526ce9c9a06101244cc60c2f`. Pinned
Pillow 12.2.0 rejects inspection, verification, and decode with
`UnidentifiedImageError("cannot identify image file <bytes>")`.

The selected parity row passed 1/1 at
`.coverage-mcp/local-evidence/selected/avif-iloc-reserved-construction-method-bits-20261024/coverage.json`.
Raw LLVM records at `src/codecs/avif/samples.rs:1124` show four and two visits
to the reserved-bit rejection arm across the two parser instantiations. The
full parity-only report at
`.coverage-mcp/local-evidence/parity-full-20261024/coverage.json` had this arm
uncovered before the case. Coverage MCP marks local report source/build
identity unverified and test attribution unknown; no aggregate coverage gain
is claimed.

The public fixture matrix now contains 2,543 total rows: 2,026 decode / inspect /
verify rows and 517 encode rows. Of those, 2,026 decode rows and 486 encode
rows are active; 31 encode rows remain planned. The separate target-only fault
lane contains 85 cases.

## AVIF duplicate item identifier parity — 2026-10-24

Added active parity row `decode:avif:error_iloc_duplicate_item_id`. The
deterministic generator duplicates the complete sole record in the pinned
file-backed `iloc` table, retaining the same item ID and AV1 media payload.
The fixture grows by 14 bytes to encode both records; its SHA-256 is
`37b723c1e183488bbf0ddf7c0cad7ae271f54dfe912b040c4721e0af7e1d48fd`. Pinned
Pillow 12.2.0 rejects inspection, verification, and decode with
`UnidentifiedImageError("cannot identify image file <bytes>")`.

The exact selected parity run passed 1/1 at
`.coverage-mcp/local-evidence/selected/avif-iloc-duplicate-item-id-20261024/coverage.json`.
The raw LLVM records at `src/codecs/avif/samples.rs:1115` show 4/4 and 2/2
outcomes across the two parser instantiations. Coverage MCP marks the selected
report's source/build identity unverified and test attribution unknown; its
incremental comparison with the saved full report is incomparable because the
baseline branch detail is incomplete. No aggregate coverage gain is claimed.

The public fixture matrix now contains 2,542 total rows: 2,025 decode / inspect /
verify rows and 517 encode rows. Of those, 2,025 decode rows and 486 encode
rows are active; 31 encode rows remain planned. The separate target-only fault
lane contains 85 cases.

## AVIF zero item identifier parity — 2026-10-24

Added active parity row `decode:avif:error_iloc_zero_item_id`. The deterministic
generator changes the sole item ID in the pinned file-backed `iloc` table from
one to zero while preserving the file size and AV1 media payload. The fixture
SHA-256 is
`6628085ef24f7bb3a62eb656950f609d585ebcb5e27b3fd1c2f9d04ad50c06fa`. Pinned
Pillow 12.2.0 rejects inspection, verification, and decode with
`UnidentifiedImageError("cannot identify image file <bytes>")`.

The exact selected parity run passed 1/1 at
`.coverage-mcp/local-evidence/selected/avif-iloc-zero-item-id-20261024/coverage.json`.
The raw LLVM branch records at `src/codecs/avif/samples.rs:1114` show the
zero-ID rejection arm with counters 4/0 and 2/0 across the two parser
instantiations. Coverage MCP marks the selected report's source/build identity
unverified and test attribution unknown; its incremental comparison with the
saved full report is incomparable because the baseline branch detail is
incomplete. No aggregate coverage gain is claimed.

The public fixture matrix now contains 2,541 total rows: 2,024 decode / inspect /
verify rows and 517 encode rows. Of those, 2,024 decode rows and 486 encode
rows are active; 31 encode rows remain planned. The separate target-only fault
lane contains 85 cases.

## AVIF overlong grid descriptor extent parity — 2026-10-24

Added active parity row `decode:avif:error_grid_descriptor_overlong_extent`.
The deterministic generator changes the primary grid item's declared extent
from eight bytes to nine while preserving the file size and encoded AV1 media
payload. The fixture SHA-256 is
`e0ef3f8597d7ae703cd5ccf7ee8eb24293a857ea4c1e86c8828a899508e54e4f`.
Pinned Pillow 12.2.0 rejects inspection, verification, and decode with
`UnidentifiedImageError("cannot identify image file <bytes>")`.

The exact selected parity run passed 1/1 at
`.coverage-mcp/local-evidence/selected/avif-grid-overlong-descriptor-20261024/coverage.json`.
The raw LLVM branch record for `src/codecs/avif/samples.rs:1746` has outcome
counters 6/0, exercising the exact-extent rejection. Coverage MCP marks the
selected report's source/build identity unverified and test attribution
unknown; its incremental comparison with the saved full report is
incomparable because the baseline branch detail is incomplete. No aggregate
coverage gain is claimed.

The public fixture matrix now contains 2,540 total rows: 2,023 decode / inspect /
verify rows and 517 encode rows. Of those, 2,023 decode rows and 486 encode
rows are active; 31 encode rows remain planned. The separate target-only fault
lane contains 85 cases.

## AVIF short grid descriptor extent parity — 2026-10-24

Added active parity row `decode:avif:error_grid_descriptor_short_extent`. Its
deterministic generator changes the primary grid item's declared descriptor
extent from eight bytes to three while preserving the file length and media
payload. The fixture SHA-256 is
`95670f2861407e944391adfdd75bbc97e6ebd010aedda7850dd65a7286cb49e5`.
Pinned Pillow 12.2.0 rejects inspection, verification, and decode with
`UnidentifiedImageError("cannot identify image file <bytes>")`.

The exact selected parity run passed 1/1 at
`.coverage-mcp/local-evidence/selected/avif-grid-short-descriptor-20261024/coverage.json`;
its local receipt records the selected, executed, and passed row IDs. The raw
LLVM branch record for `src/codecs/avif/samples.rs:1712` has outcome counters
6/0, exercising the short-extent error path. Coverage MCP returned measured
selected-run coordinates but marks source/build identity unverified and test
attribution unknown. Incremental comparison with the saved full report is
incomparable because that baseline's branch detail is incomplete; no aggregate
coverage gain is claimed.

The public fixture matrix now contains 2,539 total rows: 2,022 decode / inspect /
verify rows and 517 encode rows. Of those, 2,022 decode rows and 486 encode
rows are active; 31 encode rows remain planned. The separate target-only fault
lane contains 85 cases.

## AVIF grid without dimg references — 2026-10-24

Added active parity row `decode:avif:error_grid_without_dimg_references`. The
fixture renames the source grid's only `dimg` reference box to `zzzz` without
changing box sizes or encoded media payloads. Pillow 12.2.0 rejects it with
`RuntimeError("Failed to decode image: Invalid image grid")`. The fixture SHA-256
is `a283fb5590e31db1f491fe1e010a22fec3f55da99080dd4dcc61b95e89d6316a`.

The selected parity run passed 1/1 at
`.coverage-mcp/local-evidence/selected/avif-grid-no-dimg-current-20261024/coverage.json`.
The local LLVM report records the missing-child error arm in
`src/codecs/avif/samples.rs:1908`; Coverage MCP lists only the opposite arm as
unobserved in this selected run. It does not reach the similar container check
at `src/codecs/avif/container.rs:1590`, because sample extraction rejects the
input first. Coverage MCP marks source identity unverified and test attribution
unknown for the report, and its query against the saved full report timed out;
no full-suite coverage delta is claimed.

The public fixture matrix now contains 2,538 total rows: 2,021 decode / inspect /
verify rows and 517 encode rows. Of those, 2,021 decode rows and 486 encode
rows are active; 31 encode rows remain planned. The separate target-only fault
lane contains 85 cases.

## JPEG oversubscribed Huffman fallback parity — 2026-10-24

Added active row
`decode:jpeg:malformed_markers_dht_oversubscribed_retained_tables`. The
full-file fixture keeps the baseline AC and chroma Huffman tables while
replacing DC0 with three one-bit codes, an oversubscribed table. Its generator
is `scripts/generate_jpeg_oversubscribed_dht_fallback_fixture.py`; input
SHA-256 is
`d308eee3946fdfa39f65045ee77d49ba407868dbea4a8a421fde3179b45bc457`.
Pinned Pillow 12.2.0 verifies the JPEG and reports
`OSError("broken data stream when reading image file")` on decode. The exact
selected public parity row passed 1/1 with the same malformed result.

The three new JPEG parity rows passed together (3/3) in
`.coverage-mcp/local-evidence/selected/jpeg-oversubscribed-dht-parity-main-20261024/coverage.json`.
Coverage MCP's `decode_slow_fast` query observes the length-greater-than-16
path and the false arm of the invalid-table sentinel check at
`src/codecs/jpeg/decode/huffman.rs:389-390`; the opposite arms remain
unobserved. The report has no verified source/build identity or test
attribution, so these are coordinate observations, not a verified coverage
gain. No full coverage campaign was run.

The public fixture matrix now contains 2,537 total rows: 2,020 decode / inspect /
verify rows and 517 encode rows. Of those, 2,020 decode rows and 486 encode
rows are active; 31 encode rows remain planned. The separate target-only fault
lane contains 85 cases.

## JPEG progressive oversubscribed Huffman parity — 2026-10-24

Added active row
`decode:jpeg:malformed_markers_progressive_dht_oversubscribed`. It mutates
`progressive_gray.jpg` by replacing only its DC0 DHT with three one-bit codes,
preserving the other tables. Input SHA-256 is
`883582bb9b9d43c06f950dca99782441350fa25262ad30247a0d87d2d7bff1b3`. Pinned
Pillow 12.2.0 verifies the file and reports
`OSError("broken data stream when reading image file")` on decode. The selected
public parity row passed 1/1.

Coverage MCP's gap query for the three-row report lists 38 unobserved branch
coordinates in `progressive_reconstruct`. The report has no verified
source/build identity or test attribution, so the row-level observations do
not establish marginal or aggregate coverage gain. No full coverage campaign
was run.

## JPEG scalar Huffman sentinel parity — 2026-10-24

Added active row
`decode:jpeg:malformed_markers_dht_oversubscribed_scalar_fallback`. The fixture
starts from `sampling_3x1.jpg`, retains its sibling Huffman tables, and
oversubscribes DC0. Its generator is
`scripts/generate_jpeg_oversubscribed_dht_fallback_fixture.py`; input SHA-256
is `7e895799133d2308b6e0eef0795c67da67675771693cde3aad1f49dc81bd6625`.
Pillow 12.2.0 verifies the input and returns the same malformed-stream decode
error; the selected public parity row passed 1/1.

Coverage MCP's selected query for `HuffTable::decode_slow` at
`src/codecs/jpeg/decode/huffman.rs:349` lists only the true outcome, showing
the false outcome in the invalid-table sentinel predicate was observed. Both
arms at line 360 remain missing in the three-row report. Source/build identity
and test attribution are unverified, so no marginal or aggregate coverage gain
is claimed. No full coverage campaign was run.

## AVIF auxiliary alpha sample-count parity — 2026-10-22

Added `decode:avif:sequence_alpha_sample_count_mismatch` for the pinned
`10bit.avif` sequence with only the alpha track's `stsc.samples_per_chunk`
field changed from five to four (input SHA-256
`89a13baeb503ace10bea0f07d9c2aa296235d2ca8ed0471f3ee99751deccafcf`). Pillow
12.2.0 verifies the file and decodes its first four 64×64 RGBA frames; loading
frame four raises `RuntimeError: Failed to decode frame 4: No images remaining`.
The first-frame reference is 16,384 bytes with SHA-256
`cef05e2501d6fe214a10be9acd4aeef15db8263529bcfb0111bf2cdc98285b57`.

The selected public matrix case passed 1/1. It exposed and fixed an early Rust
sample-count rejection that made inspection fail before Pillow's late sequence
error: inspection and first-frame decoding now retain the mapped samples, and
full-sequence decoding reports the malformed color/alpha count before building
frames. Coverage MCP identified the gap in sequence sample-count validation at
`src/codecs/avif/samples.rs:2671`. The selected LLVM report is
`target/release-evidence/selected/avif-alpha-sample-count-mismatch-v2-20261022.json`.
Its comparison with the saved full report is incomparable because baseline
branch detail does not reconcile with reported totals and neither report has a
verified source/build receipt. The selected gaps query shows the expected
unvisited time-scale and equal-count arms in
`src/codecs/avif/decode.rs:307,312-313`; no marginal or aggregate coverage gain
is claimed. No full coverage campaign was run.

The public fixture matrix now contains 2,532 total rows: 2,015 decode / inspect /
verify rows and 517 encode rows. Of those, 2,015 decode rows and 486 encode
rows are active; 31 encode rows remain planned. The separate target-only fault
lane contains 85 cases.

## AVIF 12-bit 4:2:0 alpha odd-edge parity — 2026-10-22

Added public decode parity row
`decode:avif:high_bitdepth_still_12bit_420_alpha_odd_edge_17x9` for a
17×9, full-range, 12-bit AV1 profile-2 all-lossless 4:2:0 image with 12-bit
auxiliary alpha. The fixture exercises odd right/bottom edges and distinct
low-nibble chroma. Its deterministic generator, pinned provenance, and exact
pixel digest are recorded in the AVIF fixture README.

The selected native coverage run passed all five decode rows and the allocation
fault contract. That fault contract now uses this fixture, so the injected
reservation failure traverses the new 12-bit route; the public result and
retry contract pass. The selected LLVM report is
`target/release-evidence/selected/avif-12bit420-alpha-odd-edge-20261022-final-r4/coverage.json`.
Coverage MCP reports 2,974/32,174 branch observations for this selected
workload and six unobserved true arms in the helper's defensive plane-shape
guards at `src/codecs/avif/decode.rs:939`. The report has no verified source or
build identity and no test attribution. Its baseline comparison is
incomparable because the saved baseline's branch detail does not reconcile
with its totals. These selected observations do not establish marginal,
aggregate, or full-suite coverage gains.

The public fixture matrix then contained 2,531 total rows: 2,014 decode / inspect /
verify rows and 517 encode rows. Of those, 2,014 decode rows and 486 encode
rows were active; 31 encode rows remained planned. The separate target-only fault
lane contained 85 cases.

## AVIF subsampled RGB output reservation fault contract — 2026-10-22

Added target-only case
`fault-contract:avif:avif_subsampled_rgb_output_reservation_failure` using the
active `decode:avif:baseline` 8-bit 4:2:0 input. The coverage-only failpoint
forces the subsampled RGB output reservation to fail without requesting a
large allocation. The public result must be a `Dimensions` error at
`StillDecode` with message `decode: unable to allocate AVIF subsampled RGB
output`; retry must match a fresh decode. The selected run passed 1/1 with
`oracle_status: not_applicable`, so this is fault-contract evidence, not Pillow
parity. Its report is
`target/release-evidence/selected/20261022T055920Z/coverage.json` (SHA-256
`f9e744f74dc6b44dca3f5809b311baf0d0580db96adec4a042cd617943b51878`).

Coverage MCP found no missing branch location at the output reservation check
in this selected report. Overall it reports 1,985/32,908 branch observations;
source/build identity is unverified and test attribution is unknown, so this is
selected-run exploration only. No verified marginal gain, aggregate coverage
gain, or full-coverage result is claimed.

## ICO embedded PNG partial-signature parity — 2026-10-22

Added active parity row `decode:ico:error_malformed_png_entry_signature`.
Its complete 2,690-byte ICO preserves the embedded PNG's four-byte dispatch
prefix and flips only the fifth signature byte; input SHA-256 is
`69864fbd1392610d04953ca7ff3de0b0e9643a24f3edeabee748e06fe251549e`.
Pinned Pillow 12.2.0 reports `OSError("Truncated File Read")` for inspection,
verification, decode, and sequence decode. The exact selected Rust parity run
on the current main base passed 1/1.

Coverage MCP's selected-test coordinate comparison against the prior PNG
selection found the false arm of the full PNG signature check at
`src/codecs/png/decode.rs:2036` as newly observed. The one-row report leaves the
true arm unobserved, as expected for this malformed input. Both reports have
unverified source/build identity and unknown MCP test attribution, so this is
coordinate evidence only; no verified marginal gain or full-coverage result is
claimed. The current-base local report is
`.coverage-mcp/local-evidence/20261022/ico-png-partial-signature-main/coverage.json`.

## Selected coverage artifact retention — 2026-10-21

The selected coverage runner now defaults to a unique
`.coverage-mcp/local-evidence/selected/<run-id>/` directory outside `target/`.
The report, captured command log, and local execution receipt remain adjacent.
Explicit `SELECTED_COVERAGE_REPORT` paths retain their repository containment
and no-overwrite checks.

The selected-only verification ran
`RUSTC_WRAPPER= PYTHON=/Users/lazytrot/work/image-slash-star/.oracle-venv/bin/python CARGO_NET_OFFLINE=true make coverage-selected COVERAGE_CASES='decode:webp:lossy_vp8'`
from source revision `65d094e0bf2024735f5db932f7e7621b034a9576`. It passed 1/1:
selected, executed, and passed IDs were all `decode:webp:lossy_vp8`; no row failed
or remained planned. The local receipt records a clean worktree before and after
the run, `source_changed_during_run: false`, and
`fixtures_changed_during_run: false`.

The retained report is
`.coverage-mcp/local-evidence/selected/20261021T125859.070656Z-544ea994e4de47c8b33548801213f084/coverage.json`
(SHA-256 `b40b8a5d2e0491dfd80ca5ae96f6efd47ae4397fbb1338b020f74d66a1cf5454`).
Its adjacent files are `coverage.log` and `coverage.local-evidence.json`.

| Selected-run metric | Covered / total | Percent |
| --- | ---: | ---: |
| Lines | 2,744 / 139,295 | 1.97% |
| Branches | 267 / 32,016 | 0.83% |
| Functions | 227 / 8,908 | 2.55% |
| Regions | 4,587 / 216,342 | 2.12% |

Coverage MCP `coverage_gaps` was queried with this report, `metric=branches`,
and `query=src/codecs/webp`. It returned `status=measured`, with 267/32,016
branches in the report and 757 matching groups (five shown on the first page).
Source identity is `unverified`, test attribution is `unknown`, and build ID and
recorded revision are null. These missing provider receipts prevent a
compatible incremental union or a marginal coverage claim. Missing observations
are selected-run locations, not regressions; selected reports do not establish
full coverage or enforce full-suite floors.

## GIF NETSCAPE loop-payload parity — 2026-10-21

Added active input-only parity rows `decode:gif:netscape_loop_split_subblocks`
and `decode:gif:netscape_loop_empty_payload`. Their deterministic inputs are
derived structurally from `animated.gif`, with SHA-256 values
`f37bed09e23a28e757415e9a2e238743ca8e14010bc8f2a9f4203560e193326a` and
`20dad1519e7cd65f7f2aff71ca773bfedea1eb4d475b9ac795d0b71eb26202db`.
Pillow 12.2.0 verifies and decodes both as two-frame 128x128 animations, leaves
loop metadata unspecified, and returns identical frame references: frame 0
(`P`, SHA-256 `5c14fc6f08efde212c935cb3f9e414e282357d8ceca54b8cb932023c15e4f5f9`)
and frame 1 (`Rgb8`, SHA-256
`589579c365ba1aee1b7f5eb46af957d2d5f9dc32c22bc35c8f5d331263ebb4fb`).

The first case splits the loop record across data sub-blocks. The second has an
empty first sub-block; a following zero padding marker lets Pillow finish its
extension-data loop and is ignored by the Rust GIF block parser. The decoder
now reads the first NETSCAPE2.0 sub-block, matching Pillow's loop metadata
behavior. The coverage harness checks exact frame pixels while preserving the
decoder's indexed frame representation.

The selected run passed 5/5 rows (the two new cases, two loop controls, and the
existing ANIMEXTS1.0 control). Its local LLVM report is
`.coverage-mcp/local-evidence/20261021/gif-netscape-loop-edge-parity-r6/coverage.json`
(SHA-256 `540056e37567817f86d1b1c13ae9976981b0a6735eb163ba0d496083ca2a7df0`).
Coverage MCP measured 142/32,016 branch observations overall and found no
missing observations under `read_first_sub_block_and_skip_rest`. Source identity
is unverified and test attribution is unknown, so this is selected-run evidence
only; no full-coverage result or aggregate gain is claimed.

## AVIF monochrome super-resolution parity — 2026-10-21

Added active case `decode:avif:superres_actual_upscaled_monochrome_17x16` for a
17x16, 8-bit YUV400 AVIF with denominator-9 super-resolution (coded width 16,
visible width 17), one tile, and luma SGR restoration. The deterministic
fixture generator uses the pinned libavif 1.4.1/libaom 3.13.2 fixture tool;
the AVIF SHA-256 is
`d2c485bd5da5d3ea1a6c7af4c95807176f71949c312ca37756caed3e2eae6446`.
Pinned Pillow 12.2.0 returns 816 RGB bytes with SHA-256
`90cb1ee739f1819fd58c121b50d8ed032e298361ec5a897a1b88b3e61d103dd6`.

The decoder now admits this bounded intra monochrome super-resolution
restoration profile. Its matrix-quantization guard remains closed for inter
reconstruction. The selected new row passed 1/1, and the existing monochrome
and super-resolution controls passed 3/3 on the final formatted source. Reports:
`target/release-evidence/selected/avif-mono-superres-new-combined-final.json` and
`target/release-evidence/selected/avif-mono-superres-controls-rechecked-final.json`.

Both Coverage MCP incremental branch comparisons (new parity and target-only
fault against the controls report) are incomparable because the baseline branch
detail does not reconcile with its totals. All selected reports have unverified
source identity, null build/revision receipts, and unknown test attribution, so
no union or branch gain is claimed. The new-row gaps query reports 55 missing
observations in `lossy_monochrome_superres_restoration_common` and six in its
closure; both result arms of the matrix guard at
`src/codecs/avif/av1/entropy.rs:17927` remain unobserved. The fault-report query
reports the true arms of the sequence-support and frame-dimension guards at
`src/codecs/avif/av1/mod.rs:503` and `:504` as unobserved; it reports no missing
branch at the plane-dimension comparison on line 512. These are selected-report
gaps only. No full coverage campaign was run.

## AVIF monochrome alpha-plane fault contract — 2026-10-21

Added target-only case
`fault-contract:avif:avif_monochrome_alpha_plane_length_mismatch` using the
active Pillow-backed limited-alpha monochrome 16x16 fixture. A `cfg(coverage)`
hook removes one sample from the validated alpha plane immediately before
`monochrome_alpha_matches`. The runner requires
`Unsupported(NotImplemented, StillDecode)` and then compares an uninjected
retry with a fresh decode. The selected target-only run passed 1/1 with
`oracle_status: not_applicable`; this result is diagnostic fault-contract
evidence, not Pillow parity. Its local report is
`target/release-evidence/selected/avif-mono-superres-alpha-fault-combined-final.json`.
The hook is recorded as `defensive_model` in the coverage-origin inventory,
which verifies 196 exact `cfg(coverage)` guards across 28 files.

## Adam7 RGB16 PNG parity — 2026-10-21

Added active public parity case `decode:png:interlace_adam7_rgb_16bit`. Its
9x9 RGB16 source serializes all seven Adam7 passes and has input SHA-256
`243f253a436c595237e54fddb58f7b5b9fa5fbf07a18b35531399a821cb7a5dc`. Pinned
Pillow 12.2.0 decodes it as RGB with 243 bytes and SHA-256
`fdd488e078dac07ca3813a65dce0146ee1b9fc2f451301d579356f95f512920e`.

The selected parity run passed 1/1 on the combined matrix. Coverage MCP
measured 167/31,900 branches and reported both `unpack_into` arms at
`src/codecs/png/decode.rs:1794` as missing from this selected report. Source/build
identity is unverified and test attribution is unknown. The saved full report
comparison was incomparable because its branch detail does not reconcile with
its totals; no aggregate coverage gain is claimed. The refreshed report is
`target/release-evidence/selected/png-adam7-rgb16-merged-main-20261021.json`. No
full coverage campaign or production decoder change was made.

## BMP top-down odd-width 24-bit parity — 2026-10-21

Added active parity case `decode:bmp:top_down_24_odd_width`. Its 194-byte
9x5 INFO-header BMP uses a negative height, 27-byte BGR pixel rows, and one
padding byte per row. The complete input SHA-256 is
`e33cd8541c6e05f96d02f256fdc94baa80d072ec4146f08129ef9f87053d0b19`.
Pinned Pillow 12.2.0 returns 135 exact RGB bytes with SHA-256
`3ea707e3df8c76a38c8c72d2c3e263806f92aef645741ed183127b35ce04f925`.

The exact selected parity run passed 1/1; its report records 94/31,900
branch observations. Coverage MCP found the bottom-up arm missing at
`decode.rs:710`. A follow-up run with the existing bottom-up BMP control
passed 2/2 and recorded 95/31,900 branch observations. The selected-report
comparison found one newly observed branch arm, but remains limited because
source/build identity is unverified and test attribution is unknown. Comparing
with the saved full report was incomparable: its branch detail is incomplete
and its denominator is 31,800. No marginal coverage gain or full coverage
result is claimed. The local reports are
`target/release-evidence/selected/bmp-top-down-odd-width-merged-main-20261021.json` and
`target/release-evidence/selected/bmp-top-down-odd-width-with-bottom-up-control-merged-main-20261021.json`.

The saved full BMP report still lists a missing false arm at
`decode.rs:801` in palette attachment. Existing no-palette BMP input is
rejected by Pillow as truncated, so the branch remains a reachability question
rather than a justified new parity case.

The current fixture matrix contains 2,530 total rows: 2,013 decode / inspect /
verify rows and 517 encode rows. Of those, 2,013 decode rows and 486 encode
rows are active; 31 encode rows remain planned. The separate target-only fault
lane contains 82 rows.

## TIFF 2x2 Predictor=2 odd-edge parity — 2026-10-22

Added active parity rows for a 5x4 TIFF with an odd-width padded clump and a
4x5 TIFF with a padded final clump row. Both use LZW, contiguous 2x2 YCbCr,
Predictor=2, and RowsPerStrip=4. Pinned Pillow 12.2.0 / libtiff 4.7.1 decodes
the inputs to RGB; input and pixel SHA-256 values are `3aba39db3897b58eebb6a54769267ca7c0c46c3111fd06748a08c258d862b86c` /
`6d8e08b0b2dd2e2620535846b713064c48abca47c6d57ad654433136c0735e70` and
`363e81d857d75ba4b3ecf5a7da2db8cb8e35ff825abcd2856f292eb1c902ddef` /
`a8f4c5318a22ef4da79c4febbf6d8d838ba9415afacc1fe55a568a4c93fcdc30`.

The Rust TIFF gate admits odd-edge 2x2 Predictor=2 geometry. The existing
ceiling-based clump and predictor-row calculations handle the padding. A focused
selected run passed
5/5 exact decode rows, including both new cases and three controls; the
control-only run passed 3/3. Coverage MCP selected-test comparisons reported
zero new branch and line coordinates. Both reports lack source/build receipts,
so the comparison is limited to coordinates and does not establish a verified
coverage gain. Reports:
`.coverage-mcp/local-evidence/selected/20261022/tiff-ycbcr-odd-predictor-2-controls/coverage.json`
and
`.coverage-mcp/local-evidence/selected/20261022/tiff-ycbcr-odd-predictor2-controls-only/coverage.json`.

## TIFF 1x2 Predictor=2 odd-height parity — 2026-10-22

Added active parity rows for 4x3 and 3x3 contiguous YCbCr TIFFs using LZW,
1x2 subsampling, and Predictor=2. Both images end with a padded clump row. The
4x3 fixture has an 8-byte predictor row, so Pillow/libtiff preserves its
residual bytes; the 3x3 fixture has a 6-byte predictor row and takes horizontal
predictor reversal. Pinned Pillow 12.2.0 / libtiff 4.7.1 produces RGB references.
Input / pixel SHA-256 values are `f6e68880226a3e0d995cc73a931cd5b2905709d0bea15299aa572b0c62eef046` /
`7e55ac763e029f17ce869538b69db5f9a3c61078802c4a0ce8a254c992cd11e3` and
`4dca2e6335e895e6b88a6f8caa8b5c6aece3c1dac43b568044f283f74afd6967` /
`c52188185662eb2a565288b837ba81c29535ca8ddfa9a1fdfd3eecd7b077dfd9`.

Both exact selected parity rows passed individually and together (2/2). Coverage
MCP found the missing false arm at `src/codecs/tiff/decode.rs:957` when measuring
the 3x3 selector alone. The paired LLVM report covers both arms there (false=4,
true=5), and the focused `coverage_gaps` query returns no missing group at that
line. Coverage MCP marks source identity unverified and test attribution unknown;
the incremental comparison is incomparable because the earlier LLVM branch
detail is incomplete. This evidence is limited to these selected rows and is
not a full-suite coverage claim. The paired report is
`target/release-evidence/selected/tiff-1x2-odd-height-predictor2-pair.json`.

## PNG chunk-type byte parity — 2026-10-22

Added malformed input `decode:png:error_malformed_structure_invalid_chunk_type_non_letter`
with a complete, CRC-correct `a@Cd` chunk before IDAT. Pinned Pillow 12.2.0
rejects the file during `Image.open` with `UnidentifiedImageError`. Its chunk
name check also accepts ASCII digits and underscores, so the positive control
`decode:png:pillow_tolerated_chunk_type_digit_underscore` covers `a1cd` and
`a_cd`; the Rust parser preserves those Pillow-observed outcomes. The malformed
and accepted fixture SHA-256 values are
`edac414911843c23dac2cba7f194ed34fc43addfd40877440e29b676dce55ae8` and
`92f4b333a2dc484ceae684e89dac4cac8d3c11c96c03e287b0e2a0e4a71785de`.

The shared chunk-type validator now runs in both decode iteration and PNG
inspection. The focused selected run passed 4/4 rows: the malformed case, the
digit/underscore control, and the existing lowercase-reserved-bit and
post-IDAT ancillary controls. Coverage MCP found no missing branch groups in
`validate_chunk_type`. Its report marks source identity unverified and test
attribution unknown; the local receipt records the four selected cases and no
source changes during the run, so this remains selected-scope evidence rather
than an aggregate coverage claim. Report:
`.coverage-mcp/local-evidence/20261022/png-chunk-type-parity-final/coverage.json`.

## AVIF full-width multi-tile monochrome super-resolution — 2026-10-21

Added public Pillow parity case
`decode:avif:multitile_monochrome_superres_288x64`. The pinned libavif 1.4.1 /
libaom 3.13.2 generator verifies two deterministic encodes and an actual
denominator-9 AV1 frame with a 256x64 coded plane, a 288x64 upscaled plane, and
two full-width tile columns. The source Y4M, AVIF, and Pillow RGB pixel hashes
are `c8ebdc997141ba1718abe3b81819e6383f2d3724f54a646525f98e3059c30b60`,
`cc88ed36062a37989bef826649d01c2fa872f41c3668c59509f310ff081a93d4`, and
`30ba32d9eda130257562b74f7fa6a49025176071264bfca20a3d87cae6951479`.

The Rust decoder now admits only two complete tile columns, one tile row,
monochrome 8/10/12-bit reconstruction, actual super-resolution, and neutral
CDEF, deblocking, restoration, and film-grain settings for this path. The
existing frame assembler combines coded tile planes before applying the shared
upscaler. The initial 256x64 candidate had a cropped second tile and differed
from Pillow in 171 bytes at the right edge; it was replaced and is not active.

The selected public case passed exact RGB parity. The focused controls
`decode:avif:multitile_monochrome` and
`decode:avif:superres_actual_upscaled_monochrome_17x16` also passed. The local
selected LLVM report is
`.coverage-mcp/local-evidence/selected/avif-mono-288-final-20261021/coverage.json`.
Coverage MCP observed the super-resolution true arm at
`src/codecs/avif/av1/frame.rs:1714`; its false arm was absent from this selected
run. MCP reports 1,785/31,970 branches in this small selected report, with
source identity unverified and test inventory unknown, so it is only
coordinate-level evidence and not an aggregate coverage claim.

## AVIF monochrome grid parity and reservation failure — 2026-10-21

Added active parity case `decode:avif:portable_lossless_monochrome_grid_73x64`.
Its deterministic 73x64 monochrome 2x1 AVIF input SHA-256 is
`3e69167291d846470673b297e9a30e51d7027e182918b0a8c1d95f24719e9828`; pinned
Pillow 12.2.0 produces the exact 18,688-byte RGBA reference with SHA-256
`a553e8c265e29b0644101cff0da8cb54ac351ac4541b86d095ad960445cea987`.

Added target-only contract
`fault-contract:avif:avif_monochrome_grid_cell_reservation_failure` using that
active parity input. The coverage-only failpoint forces the monochrome grid's
cell-vector reservation to fail; the runner checks the public StillDecode
error and a successful retry. The exact selected run passed both selectors
(1/1 parity and 1/1 fault contract, oracle `not_applicable`). Its local report
is `target/release-evidence/selected/avif-monochrome-grid-mono-fault-final-20261021.json`.
The report is local-only and has no Coverage MCP source/build receipt; no full
coverage claim is made.

## AVIF monochrome grid edge parity — 2026-10-21

Added active parity cases
`decode:avif:portable_lossless_monochrome_grid_no_alpha_73x64` and
`decode:avif:portable_lossless_monochrome_grid_bottom_clip_64x73`. The first
fixture has no auxiliary-alpha items, references, or properties; its input
SHA-256 is
`a33a40230675d388722a13e45849205e24ff7f36486502907472c5f9ebd719d0`. Pinned
Pillow 12.2.0 returns RGB 73x64 with 14,016 exact bytes and SHA-256
`1b4a716a8f76555d37d08901f0582e195b2bdcedad46b00f781995d42e4cb1b6`. The
second is a 1x2 monochrome grid clipped to 64x73; its input SHA-256 is
`453945fc2524dabc77162d97896b21d8399a7171a909d19babb908603c0a4201`. Pillow
returns RGBA with 18,688 exact bytes and SHA-256
`20b867857750340cae5d682bb6f5a162508395c242dfe331321cbff0bae90bb4`.

The deterministic generator checks the BMFF item graph and extents, encodes
both cell variants twice, and verifies the complete decoded pixels through the
pinned oracle. The selected run passed 2/2 new parity rows. Coverage MCP's
focused branch query for `validate_monochrome_grid_with_token` returned 30
remaining observations in the selected report: the no-alpha `still.alpha`
branch at line 693 and visible-height calculation at line 807 had no missing
locations; it still listed the first-leaf arm at line 625, the unsupported
monochrome-dimension arm at line 628, the reservation-failpoint arm at line
636, and both zero-visible-sample rejection arms at line 811. The report is
`target/release-evidence/selected/avif-grid-edge-cases-20261021.json`; Coverage
MCP marks its source/build identity unverified and test attribution unknown.
The prior selected local report was unavailable after cleanup, and Coverage MCP
could not resolve the recorded 2026-08 baseline snapshot (composite snapshot not
found), so no marginal coverage gain is claimed. No full coverage campaign or production decoder
change was made.

Fixture declarations, executed comparisons, source coverage, and release
publication have different identities and denominators. The latest local
all-feature report and the historical managed coverage baseline are recorded
separately; later entries retain the report-specific observations made when
each entry was recorded.

## JPEG row-upsample fault contracts and TIFF Predictor=2 parity — 2026-10-21

Added target-only fault contract
`fault-contract:jpeg:jpeg_fancy_upsample_row_invalid_geometry` using the
active Pillow-parity YCCK input. Its coverage-only fault sets the row
upsampler's source width to zero, checks the public malformed-decode error,
then verifies retry against a clean decode. The selected fault-contract run
passed 1/1 with `oracle_status: not_applicable`. Coverage MCP returned
`fancy_upsample_row` branch locations for the selected report, but its source
identity is unverified and test attribution is unknown; no coverage gain is
claimed.

Added three target-only fault contracts on the same active YCCK parity input
for zero source height, zero horizontal upsampling ratio, and zero vertical
upsampling ratio. Valid JPEG frame dimensions and sampling factors cannot
produce these internal states. Each coverage-only hook exercises one guard,
asserts the public malformed-decode result, and verifies a clean retry. These
rows report `oracle_status: not_applicable` and remain separate from Pillow
parity counts. The combined selected report
`.coverage-mcp/local-evidence/selected/jpeg-parity-and-faults-20261024.json`
passed all 286 active JPEG decode parity rows and all 13 JPEG fault contracts
(299/299). Coverage MCP reports no missing branch at the row-geometry guard
for this selection. Its source identity remains unverified and test attribution
unknown, so the selected report does not support an aggregate coverage-gain
claim.

Added target-only fault contract
`fault-contract:jpeg:jpeg_fancy_upsample_row_short_stride`, injecting a stride
one byte shorter than the component width. This state is unreachable through
valid JPEG component setup and must be rejected before any row slice can use
the undersized stride. The public malformed-decode error and retry against a
clean decode are checked using the same active Pillow-parity YCCK input.
The selected fault run passed 1/1. In its local LLVM report the short-stride
condition recorded true/false counts of 1/36; the selected valid YCCK row
recorded 0/72 for the same condition. Coverage MCP surfaced remaining missing
arms in the row helper. An incremental comparison of the earlier selected
fault report with the valid YCCK report returned `incomparable` because
baseline branch detail does not reconcile with report totals, so no aggregate
coverage gain is claimed. The current selected fault report is
`target/release-evidence/selected/20261020T201245Z/coverage.json`; comparison
inputs were `target/release-evidence/selected/20261020T200320Z/coverage.json`
and `target/release-evidence/selected/20261020T200430Z/coverage.json`.

Added target-only fault-contract case
`fault-contract:jpeg:jpeg_row_upsample_source_length_mismatch` on the active
`decode:jpeg:color_ycck_app14_transform_2` input. Its coverage-only one-shot
hook shortens the selected component sample slice below the computed row
requirement. The public result must be a malformed JPEG `StillDecode` error,
followed by a retry that matches a clean decode. The selected run passed 1/1;
its local receipt records the exact selected ID, `oracle_status: not_applicable`,
clean source state before and after, and no fixture changes.
At revision `936bc563111847de84d8926c4eae4f2ed9f80a4a`, the retained report is
`.coverage-mcp/local-evidence/selected/jpeg-row-source-length-mismatch-20261021-clean/coverage.json`
with SHA-256
`51a038f39db8f0c26f9acd3abf6b208e885a0e8e44f821cd5f25c063ed13d063`. Its
single-case totals are 1,891/138,817 lines (1.3622%), 274/31,906 branches
(0.8588%), 132/8,881 functions (1.4863%), and 3,155/215,542 regions (1.4638%).
Coverage MCP returned `measured`; the focused helper query listed 17 remaining
branch observations, while the line-filtered query for the source-length guard
returned no missing group. MCP source identity is unverified, test attribution
is unknown, and build ID and recorded revision are absent. No compatible full
baseline was queried for this source revision, so no aggregate gain is claimed. No full coverage run
was made.

Added active parity case
`decode:tiff:ycbcr_subsampled_lzw_2x2_two_strips_predictor_2`. The 224-byte
little-endian input SHA-256 is
`bf76628b4a022f6c6891f65fc572d3af75760777dab85e70ef1e0e933072c6c1`; pinned
Pillow 12.2.0 produced the exact 48-byte RGB output SHA-256
`7a2b02a725a8988a4e716873f80fe8290c26e274536c6716b4474c31b8379df4`. The
selected parity run passed 1/1. Coverage MCP returned both branch arms at
`decode_ifd:323`, but source identity is unverified and test attribution is
unknown, so no coverage gain is claimed. The reports are
`target/release-evidence/selected/20261020T194137Z/coverage.json` and
`target/release-evidence/selected/20261020T194437Z/coverage.json`. No full
coverage campaign was run.

Added Pillow-backed 1-by-2 YCbCr LZW Predictor=2 parity cases. The 4-by-4
fixture has 8-byte predictor scanlines, so libtiff reports
`horAcc8: (cc%stride)!=0.` and leaves the residual bytes untouched; the 3-by-4
fixture has 6-byte scanlines, which libtiff reverses normally. With pinned
Pillow 12.2.0 / libtiff 4.7.1, the 235-byte input and 48-byte RGB output hashes
are `e5dd405646c7e6f44ea0c82e5f01cd770cd395ad38b93bb1011ccc176c17a095` and
`b394ef5f5e2680867b09884393d2ee73a2ce9491a21c905c9b40d99074d9671a`; the
224-byte input and 36-byte RGB output hashes are
`b1ebb064f9f7f330ce1a1e0b7e8abdab9919791f6c364067542dc656e09e6681` and
`384d5eeb323cb3f719d2203a0602ba2fcd42f95cd981568a4e9c4c2b084de219`. One
selected run passed both new rows and the neighboring 1-by-2 Predictor=1 and
2-by-2 Predictor=2 rows (4/4). Coverage MCP still lists
branch arms at `decode_ifd:323`, but the selected report records source identity
as unverified and test attribution as unknown. Comparing it with the saved full
report was incomparable because baseline branch detail does not match its
reported totals; no coverage gain is claimed. The selected report is
`target/release-evidence/selected/20261020T213209Z/coverage.json`. No full
coverage campaign was run.

The fixture matrix now contains 2,487 total rows: 1,972 decode / inspect /
verify rows and 515 encode rows. Of those, 1,972 decode rows and 483 encode
rows are active; 32 encode rows remain planned. The separate target-only fault
lane contains 78 rows. The coverage-origin inventory records 195 exact
`cfg(coverage)` guards across 28 Rust files.

## TIFF unaligned subsampled YCbCr Predictor=2 parity — 2026-10-21

Added active public parity rows for 2-by-2 and 1-by-2 subsampled YCbCr LZW
strips with `Predictor=2` and `RowsPerStrip=3`. The 238-byte 2-by-2 input
SHA-256 is
`45aef5c7eea83c4600240c0d4836d826b910723dc205b8f224187edcb40f63c0`; Pillow
returns 48 RGB bytes with SHA-256
`ee406bf055bf628b9b0a3b6d3ea29a5f32d97e963d3eb4964d5942c37f04881a`. The
253-byte 1-by-2 input SHA-256 is
`047e56f7916f1dca93eb034f622171ec39da0881f4679dc7f1681b075f0d243f`; Pillow
returns 48 RGB bytes with SHA-256
`83c2ac29ff7d103f160792e45e42d13c3cd1c5b36f7b70b62e3ed5117f207666`. In the
1-by-2 case, libtiff leaves the 8-byte predictor scanline residuals untouched;
in both layouts, Rust reverses eligible prediction in the existing strip
buffer before the unaligned conversion path, without adding an allocation.

The selected run passed five Pillow parity rows and the existing unaligned
strip reservation fault contract (1/1; oracle not applicable). Coverage MCP
returned both outcomes around `decode_ifd:323`, but reports source as
unverified and test attribution as unknown. Comparing to the saved full report
was incomparable because branch details differ; no coverage gain is claimed.
The local report is
`target/release-evidence/selected/tiff-unaligned-ycbcr-predictor-parity-20261021.json`.
No full coverage campaign was run.

## TIFF tiled subsampled YCbCr parity and resource bounds — 2026-10-21

Added Pillow-backed TIFF cases for patterned 2-by-2 YCbCr clumps across four
16-by-16 Deflate tiles, an accepted 15-by-16 clipped tile with compressed
output ending after the visible clumps, mismatched tile byte-count arrays, and
an out-of-bounds tile offset. Added a typed `not_applicable` fault contract for
the bounded decoded-prefix failure path alongside the existing tile-buffer
reservation fault. The padded tile prefix is capped at the maximum RGB byte
count permitted by TIFF's existing decompression-bomb pixel limit.

The selected coverage run passed four parity rows and both TIFF fault
contracts. Coverage MCP reported no missing branches in
`decode_subsampled_ycbcr_tiles` for this measurement. The local report is
`.coverage-mcp/local-evidence/20261021/tiff-ycbcr-tiles-fault-prefix.json`;
its source identity is unverified and test attribution is unknown, so this is
localized evidence only. No full coverage campaign was run.

Before the follow-up cases above, the fixture matrix contained 2,479 total
rows: 1,964 decode / inspect / verify rows and 515 encode rows. Of those,
1,964 decode rows and 483 encode rows were active; 32 planned encode rows
remained. The target-only fault lane contained 75 rows. The coverage-origin
inventory recorded 192 exact
`cfg(coverage)` guards across 28 Rust files.

## WebP VP8 method-zero sampling parity — 2026-10-20

Corrected libwebp parity in VP8 coefficient adaptation: method 0 samples the
first 50 macroblocks through 200 macroblocks, then samples one quarter of the
frame; skip probability uses the full frame macroblock count. Token-buffer
methods no longer signal skip probability, and an Intra4 macroblock skipped
after zero residuals preserves the Y2 contexts as libwebp does. These changes
reuse the existing macroblock decisions and add no extra per-macroblock pass.

Added Pillow-backed parity rows `enc_lossy_quality_0_method_0_capped_statistics`
and `enc_lossy_quality_0_method_0_large_frame_skip_sample`. The first uses the
existing 128x128 `rgb.png` (64 macroblocks) and matches a 698-byte WebP
(SHA-256 `66771f67573da043e1b72af996adf0d8b694ae86e03332369361be65ff305860`).
The second uses `large.png` at 513x257 (561 macroblocks; 140 sampled) and
matches a 506-byte WebP (SHA-256
`79e192628a654aa8745d528b5eef7537cfcb9463fafc24ddf7f28afe5bdc1a96`).

The selected all-feature run passed 10/10 existing and new WebP encode rows,
with 40 public encode calls and no panics. The capped-statistics row also
passed alone (1/1, four public calls). Reports are
`target/release-evidence/selected/webp-skip-method0-sampling-final-20261020.json`
(SHA-256 `61db78820e9506dfd16c93731b5f4fccb3b1480bd5b6dc2533dab0c13f748621`)
and
`target/release-evidence/selected/webp-method0-capped-statistics-only-20261020.json`
(SHA-256 `80227fb8b4a5ad8149831f684c6810e077de62a9e9beb506cd68ac1922b33fa7`).

Coverage MCP's focused branch query returned missing outcomes in
`adapt_coefficients` and the VP8 residual skip branch at line 531. These are
selected-report locations only: source/build identity is unverified and test
attribution is unknown. The attempted incremental comparison is incomparable
because the prior report's detailed branch inventory does not reconcile with
its totals. No aggregate coverage gain or full-suite claim is made, and no
full coverage campaign was run. These are ordinary input parity cases, not
target-only fault contracts.

The follow-up row `enc_lossy_quality_0_method_0_sampling_boundary_204_macroblocks`
uses a 192x272 RGB source (204 macroblocks). Method 0 samples 51 decisions at
this size, the first sample count above the 50-macroblock cap; the 51st sampled
macroblock has a deterministic high-frequency pattern. The source is generated
by `scripts/generate_webp_method0_sampling_boundary_fixture.py` from the pinned
`rgb.png` input. Pillow 12.2.0 emits 2,218 bytes (SHA-256
`02b3bde54171c43bee921746dd3398a6f0a35649f0735f2a376b31a9640749b3`), and the
selected all-feature run passed this row alone (1/1, four public encode calls,
zero panics). Its one-row LLVM report observed 7,099/140,521 lines,
609/32,044 branches, 481/8,971 functions, and 11,236/218,427 regions. Coverage
MCP identified the `total > 200` false arm as unobserved in this one-row
selection, while the prior 10-row selected report observed both arms. The
selected-tests comparison found no newly observed branch locations; the
incremental comparison is incomparable because the earlier report's branch
detail does not reconcile with its totals. Both reports have unverified source
identity and unknown test attribution. Keep this row for the exact public
sampling-boundary regression contract; it does not establish marginal or
aggregate coverage gain. The report is
`target/release-evidence/selected/webp-method0-sampling-boundary-204-20261020.json`
(SHA-256 `048ed9d35829a3ed4da16db92e2610bdf5c7b172934727a9d28246d082996548`).

The current public fixture matrix contains 2,471 total rows: 1,957 decode /
inspect / verify rows and 514 encode rows. Of those, 1,957 decode rows and 482
encode rows are active; the other 32 encode rows are planned. The separate
fault-contract lane contains 72 rows.

## ICO/CUR entry-selection parity — 2026-10-20

Added five active Pillow parity rows covering ICO directory selection and CUR
hotspot handling: equal-area lower-depth preference, stable equal-depth ties,
both zero-bit-count depth fallbacks, equal-size CUR entries with different
y-hotspots, and a CUR entry whose width and height both increase. The CUR
equal-size witness uses a 1x11 canvas, keeping y-hotspots 10 and 2 in bounds.
The larger-dimensions CUR witness orders a 1x1 green entry before a 2x2 red
entry and confirms Pillow selects the later entry.

The ICO decoder and inspector now share area-then-depth selection for ICOs,
including Pillow's palette-derived `ceil(log2(color_count))` fallback and the
maximum-depth fallback when the palette count is absent. CUR selection follows
Pillow's raw directory rule: a later entry replaces the current one only when
both raw dimensions are strictly larger. ICO bit-depth bytes are not treated
as CUR hotspot coordinates.

The focused all-feature branch-coverage run passed all seven selected rows:

```text
RUSTC_WRAPPER= PYTHON=.oracle-venv/bin/python make coverage-selected COVERAGE_CASES='decode:ico:cursor_equal_size_different_y_hotspots decode:ico:cursor_larger_dimensions decode:ico:multi_res_multi_descending decode:ico:multi_res_equal_area_bit_depth_tiebreak decode:ico:multi_res_equal_area_same_depth_stable decode:ico:multi_res_zero_bpp_depth_fallbacks decode:ico:cursor_cursor_24bit' SELECTED_COVERAGE_REPORT=.coverage-mcp/local-evidence/20261020/ico-selection-branches-final/coverage.json
```

Coverage MCP reports no missing branches in `entry_color_depth`,
`prefers_entry`, or `prefers_cursor_entry` for this measurement. The selected
report is `.coverage-mcp/local-evidence/20261020/ico-selection-branches-final/coverage.json`;
its source identity is unverified and test attribution is unknown, so this is
localized branch evidence, not an aggregate coverage-gain claim. These cases
are reachable through normal image inputs and remain Pillow parity rows; no
fault-contract row is appropriate.

The fixture matrix contains 2,468 rows: 1,957 decode / inspect / verify rows
and 511 encode rows. Of those, 1,957 decode rows and 479 encode rows are active;
the other 32 encode rows are planned. The separate fault-contract lane contains
72 rows.

## WebP method-zero quality-window parity — 2026-10-20

Added active Pillow encode rows `enc_lossless_method_0_quality_40` and
`enc_lossless_method_0_quality_60` using the existing 128x128 `lossless.webp`
source (SHA-256
`20eb48b4ef2a42d4842684930c2023f7d14ac343b825d19a185e8dda5b25f8b2`). Both
select `lossless: true` and `method: 0`, at qualities 40 and 60. With Pillow
12.2.0, Rust encoding matched the exact reference streams: 3,358 bytes at
quality 40 (SHA-256
`21af1c4ce50d605d46ea838a65660bb4219b9bee2d47fb21deb1718ea69d13d5`) and
3,276 bytes at quality 60 (SHA-256
`d9d39a23393d2418af4eea5bf80cac9fbcfff5a5ede79a126ec600c5bc3a18c9`). Both
re-decoded to the exact 128x128 RGB pixel reference.

The focused two-row selection passed both new cases and made eight public
encode calls. A follow-up selection including the existing quality-80 control
passed 3/3 rows and made 12 public encode calls; its report is
`target/release-evidence/selected/webp-method0-quality-windows-control.json`
(SHA-256
`362613df5133960f80abccdeb71bdc91e3449dc917c0a3c503a4a43992a45e98`). The
active LLVM function record counted both outcomes at `backward_refs.rs:307`
(6 true, 12 false) and `:309` (6 true, 6 false), plus six true outcomes at
`:311`. Coverage MCP also returned missing groups for duplicate zero-hit
function records at these coordinates. Its comparison with
`target/release-evidence/coverage-main-full-20261020.json` was incomparable
because the baseline branch detail does not match its reported totals; its
source/build identity is unverified and test attribution unknown. These remain
selected parity and branch-location evidence, not an aggregate coverage-gain
claim.

## WebP method-zero quality-25 window parity — 2026-10-20

Added active Pillow encode row `enc_lossless_method_0_quality_25` on the same
128x128 RGB `lossless.webp` source. At `lossless: true`, `method: 0`, and
`quality: 25`, Pillow 12.2.0 produces a 3,638-byte WebP with SHA-256
`90f3b669c6d09675a56cadb39a9bb794baa9899595fb09f6e4d8b787e485f588`; the
re-decoded RGB reference is 49,152 bytes with SHA-256
`8a1d6fcc36f5b5e70fbf949d4e612630a1931279383e7c21db27cd3cbad98131`.

The focused selected case passed 1/1 with exact encoded-byte and pixel parity.
Its report is `target/release-evidence/selected/webp-method0-quality-25.json`
(SHA-256
`466d08a74507cb1010c8474a1bb2f07470337c6884bec70009ae91840975e351`). The
active `fill_hash_chain_impl::<true>` record counted six false outcomes at the
`quality > 25` predicate on `backward_refs.rs:311`, selecting the `width << 4`
search window. Coverage MCP returned that false outcome as observed in one
function record while retaining duplicate zero-hit groups; its incremental
comparison with the saved full report was incomparable. This is local selected
evidence only.

## Direct odd-width 1-bit PNG decode parity — 2026-10-20

Added Decode row `decode:png:depth_1_1bit_odd_width` for the existing complete
9x3 mode `1` `1bit_odd_width.png` fixture, previously used only for encoding.
The source SHA-256 is
`ab470446e941a51011e157e861e3f920b911686d3e592e1b7cb1d6a683bdd675`; its six
Pillow pixel bytes have SHA-256
`36e3c5d288415910fd696272299e53b64a80b66782e965c506b57e7dd2d8d203`. The
odd row width exercises public unpacking of the ninth 1-bit pixel before the
seven unused tail bits in each row.

The selected Decode row and the existing `encode:png:enc_1bit_odd_width` row
both passed exact pixel parity. The command and report are
`PYTHON=.oracle-venv/bin/python make coverage-selected COVERAGE_CASES='decode:png:depth_1_1bit_odd_width encode:png:enc_1bit_odd_width' SELECTED_COVERAGE_REPORT=target/release-evidence/selected/png-one-bit-odd-width-roundtrip.json`
and `target/release-evidence/selected/png-one-bit-odd-width-roundtrip.json`
(SHA-256
`7aac9adf9c0bc71f044c3cc5b8d98400914c95485aed2788aa9cba63d181022f`). Coverage
MCP returned no missing branch groups for `unpack_into` at line 1774. Its query
for `pack_one_bit` at line 1900 reported a duplicate zero-hit function record,
while the selected LLVM export contains nonzero branch counters for an active
function record there. The selected report and saved full baseline have no
source/build receipts, and their incremental comparison is incomparable; this
is localized evidence, not an aggregate coverage-gain claim.

## JPEG progressive correction-buffer parity — 2026-10-20

Added the public encode row
`encode:jpeg:enc_progressive_correction_buffer_flush_256x256`. Its source is a
256x256 L8 PNG whose 256-byte rows repeat `[0, 32, 64, 96, 128, 160, 192, 224]`
32 times. The generated PNG is 371 bytes with SHA-256
`a50cef6ddd8ace094ab3db04f0d205abbd8d6a70b2b618d93fb03e494c47eca9`. Pillow
12.2.0 encodes it as JPEG with quality 75, progressive scans enabled, and
optimization disabled. The exact 3,228-byte JPEG has SHA-256
`7bf0311dd950a11befc00859db41ae78298875a015be7958dcdafaf71c091b18`.

The selected all-feature branch-coverage case passed 1/1 and made five public
encode calls, including sink and token parity. The local LLVM report is
`.coverage-mcp/local-evidence/20261020/jpeg-progressive-correction-256/coverage.json`
(SHA-256
`6cfefcf4cb26ef7e31efcf10cdf0669668b58cf9fb36226b7e6400a105064487`). Its
branch counters recorded six true evaluations of
`correction_bits.len() > 937` for `NoopProgressiveScanCheckpoint` and two for
`TokenProgressiveScanCheckpoint`; the `eob_run == 0x7fff` condition remained
false in this case. Coverage MCP marks source identity unverified and test
attribution unknown, and incremental comparison with the saved full baseline
is incomparable because baseline detail does not match its reported totals.
This records selected-path evidence only and makes no aggregate coverage-gain
claim.

The branch is reached through ordinary public encoding, so this case remains
in Pillow parity and does not need a target-only fault-contract row. The
fixture matrix now contains 2,468 total rows: 1,957 decode / inspect / verify rows and 511 encode rows.
Of those, 1,957 decode rows and 479 encode rows are active; the other 32 encode
rows are planned. The separate fault-contract lane contains 72 rows.

## WebP lossy quality-zero bitstream parity — 2026-10-20

Added two active Pillow encode rows for lossy WebP at quality 0 and method 0.
The solid 32x32 RGB source is `(54, 83, 112)` throughout; its exact 64-byte
WebP has SHA-256
`6f7a1d39acaa69a091365e2f0f40056123e3e32557716ff641b00c74908d2467` and its
decoded RGB reference has SHA-256
`400ee6b557d526d51bf44bf9bb5a134f1c4180f2735f5024e6a1b4caafd902f7`. The
nonuniform 17x17 RGB source is the existing `webp_subtract_green.png` fixture;
its exact 158-byte WebP has SHA-256
`dd356bbf484bad7659696b03b904f19eda8cf6a78f243ac2b2ff03ebdf09e054` and its
decoded RGB reference has SHA-256
`0e2113aada69ce5552318e82fb2a0187b4e90328b59cecd40323d9c389028100`.

Exact byte parity required matching libwebp's flat-border Intra16 mode choice,
the sharpness-zero filter-delta cap, and skip-probability signaling plus
zero-coefficient macroblock context resets. Skip counts are collected during
the existing coefficient-statistics traversal. Both quality-zero rows and the
quality-100 control passed the selected public encode run: 3/3 rows, 12 encode
calls, no panics. The report is
`target/release-evidence/selected/webp-q0-nonuniform-20261020.json` (SHA-256
`37315b6153679a21dd847a27dca58940882fabf12742ebe93c0cd7286476d466`).
Coverage MCP found no missing branch groups in `skip_probability` or
`flat_border_mode` for this selected slice. The report remains local evidence:
Coverage MCP marks source identity unverified and test attribution unknown, so
it does not support an aggregate coverage claim.

These branches are reachable through ordinary public inputs and remain Pillow
parity cases. No target-only fault contract is appropriate for them; the
separate fault-contract lane remains at 72 rows.

## AVIF odd-size I420 film-grain chroma parity — 2026-10-20

Added active Pillow parity row
`decode:avif:portable_lossless_filmgrain_420_chroma_from_luma_65x65`. The
fixture uses the pinned lossless 4:2:0 chroma-from-luma grain recipe at 65x65.
Its 33x33 chroma planes end in a one-pixel grain block. The final chroma
sample maps to the last luma row and column, and its horizontal interpolation
neighbor clamps at the right image edge. The
8,189-byte AVIF has SHA-256
`5f881e4c6eed5bb4c554004d72ca5e992f6b704bc39672b01d8b3a3236bb2330`; Pillow
12.2.0 returns 12,675 RGB bytes with SHA-256
`d3c620ef7742e8702d1351c33b4ae9386a67fa1a761b6c4b9ea695a6fe08f0c9`.

The selected coverage-matrix run passed 1/1 on `aarch64-apple-darwin` with all
features and branch instrumentation using:

```text
PYTHON=./.oracle-venv/bin/python make coverage-selected COVERAGE_CASES='decode:avif:portable_lossless_filmgrain_420_chroma_from_luma_65x65' SELECTED_COVERAGE_REPORT=.coverage-mcp/local-evidence/20261020/avif-filmgrain-odd-i420/coverage.json
```

Its LLVM report is retained at
`.coverage-mcp/local-evidence/20261020/avif-filmgrain-odd-i420/coverage.json`
(SHA-256
`69ebe07ee9fe26967cc7a2e0c5d7470ad7409c715e7bff11a077744182d5496a`). The
local receipt records the exact selected row passing and fixture hashes
unchanged during the run. Coverage MCP reports 1,928/32,484 branch
observations for this selected report and still lists missing outcomes in
`film_grain::apply_plane` and `apply_with_sampling` specializations. Source
identity is unverified and test attribution is unknown; the comparison with
the saved full baseline is incomparable because its detailed branch data does
not match its reported totals. This run records selected-path evidence only;
it does not establish aggregate coverage gain or complete branch closure.

The row exercises behavior reachable through a normal image decode, so it
remains in the Pillow parity lane. No target-only fault-contract case is
needed. The fixture matrix now contains 2,456 total rows: 1,951 decode / inspect /
verify rows and 505 encode rows. Of those, 1,951 decode rows and 473 encode
rows are active; the other 32 encode rows are planned. The separate
fault-contract lane contains 72 rows.

## AVIF auxiliary-alpha duration parity — 2026-10-20

Added active Pillow parity row
`decode:avif:sequence_alpha_duration_mismatch`. The input is derived from the
existing 10-bit AVIF sequence by changing only the auxiliary alpha track's
sample duration from 1 to 2; its sample payloads and color track are unchanged.
The generated input is 2,267 bytes with SHA-256
`b70e900cbe163b4e394aaa0bab6f283fec8f7f0093cc8dbf9ea790b85103902d`.
Pillow 12.2.0 accepts the 64×64 RGBA sequence and yields five 16,384-byte
frames, each with a 1,000 ms duration. The public decoder previously rejected
this input solely because corresponding color and auxiliary-alpha sample
durations differ; the decoder now keeps the sample-count and track-timescale
checks while allowing Pillow-compatible duration differences.

The exact selected parity case passed 1/1 after the fix using:

```text
PYTHON=.oracle-venv/bin/python make coverage-selected COVERAGE_CASES='decode:avif:sequence_alpha_duration_mismatch' SELECTED_COVERAGE_REPORT=.coverage-mcp/local-evidence/20261020/sequence-alpha-timing-final.json
```

The selected LLVM report is retained at
`.coverage-mcp/local-evidence/20261020/sequence-alpha-timing-final.json` (SHA-256
`ed93cf3ead23150f44100c598cbb2a7227d7425eec6b2d16cb6681b519efabcd`).
The local receipt records the selected case passing; Coverage MCP labels source
identity unverified and test attribution unknown, so this report is not used to
claim aggregate coverage gain. Incremental comparison with the retained full
baseline is incomparable because baseline branch detail does not match its
reported totals. Pillow supplies exact frame pixels and the independent
libavif observation supplies loop semantics. This parity case tests accepted
public input behavior; it does not claim support for every AVIF track
arrangement.

The current fixture matrix contains 2,455 total rows: 1,950 decode / inspect /
verify rows and 505 encode rows. Of those, 1,950 decode rows and 473 encode
rows are active; the other 32 encode rows are planned. The separate
fault-contract lane contains 72 rows. The coverage-origin inventory records
189 exact `cfg(coverage)` guards across 28 Rust files.

## JPEG marker-tail and YCCK singleton parity — 2026-10-20

Added active Pillow-backed rows
`decode:jpeg:baseline_444_single_scan_com_tail_junk`,
`decode:jpeg:baseline_444_single_scan_com_eoi_marker_fill`,
`decode:jpeg:baseline_444_single_scan_com_ff00_tail`,
`decode:jpeg:baseline_444_single_scan_two_trailing_com`, and
`decode:jpeg:ycck_h2v2_single_sample_1x1`. The COM-tail rows derive from the
8×8 baseline COM control by inserting a `00` junk byte, a legal `FF`
marker-fill byte, or an `FF00` stuffed pair before EOI; the fourth case adds a
second valid COM segment before EOI. Pillow 12.2.0 verifies and decodes all
four to the control's same 192 RGB bytes. The FF00 candidate SHA-256 is
`7ee7bd8d0fd0734f5b51cc0cd29e8fae262b29f0d6366ab5dcf081ea787e5dc3`;
its pinned base SHA-256 is
`cf8e4443dcff4df7858b6573664aa3fe73dd401827aa293d0eca84eb0667f666`. The YCCK
case changes only the SOF0 dimensions from 2×2 to 1×1; Pillow returns one CMYK
pixel, `ff ff fd ff`. The second-COM candidate SHA-256 is
`c23f18f246085b1b088d82f9c660e1939d9d3198dda3c05ea5fe1dcf961a78e8`.

The COM-tail rows exposed that the baseline decoder's validated single-scan
fast path rejected bytes after a trailing marker, then sent the valid scan
through generic extraction with the post-scan marker bytes included as entropy.
It now skips intervening bytes and FF00 stuffed pairs only after a valid
allow-listed trailing segment and bounds repeated marker-fill scanning at
parsed EOI. `parse_jpeg` guarantees the initial cursor is at an FF marker, so
the previously uncovered pre-segment non-marker guard was unreachable for
parser-produced input and was removed instead of adding an artificial test.
This preserves the entropy boundary and accepts the Pillow-observable inputs.

The exact selected regression run passed 7/7 rows on `aarch64-apple-darwin`
with all features and branch instrumentation, including the existing trailing
COM control and DQT marker-fill case:

```text
PYTHON=.oracle-venv/bin/python make coverage-selected COVERAGE_CASES='decode:jpeg:baseline_444_single_scan_trailing_com decode:jpeg:baseline_444_single_scan_com_tail_junk decode:jpeg:baseline_444_single_scan_com_eoi_marker_fill decode:jpeg:baseline_444_single_scan_com_ff00_tail decode:jpeg:baseline_444_single_scan_two_trailing_com decode:jpeg:baseline_444_single_scan_dqt_marker_fill decode:jpeg:ycck_h2v2_single_sample_1x1' SELECTED_COVERAGE_REPORT=.coverage-mcp/local-evidence/20261020/jpeg-tail-parity-seven/coverage.json
```

The selected LLVM report is retained at
`.coverage-mcp/local-evidence/20261020/jpeg-tail-parity-seven-final-code/coverage.json`
(SHA-256 `16086ef71e98c042dec010bedb8d59be50ee21aecd5d3bc83e87edfc850f3932`);
its local execution receipt records all seven cases passing with no fixture
changes during the run. Coverage MCP reports no missing branches at the
post-segment byte check (line 2766) or FF00 condition (line 2789) in this
report. The selected report totals 408/32,484 observed branches, while Coverage
MCP marks source identity unverified and test attribution unknown. Incremental
comparison is incomparable because the saved full baseline has incomplete
branch detail, so this run does not establish an aggregate coverage gain or
regression.

## Signed-tag TIFF and singleton YCCK coverage — 2026-10-22

Added active public parity rows `decode:jpeg:ycck_h2v1_single_sample_1x1` and
`decode:tiff:grayscale_unknown_signed_private_tags`. The JPEG fixture changes
only the SOF0 dimensions of the pinned 2×2 H2V1 YCCK input; its SHA-256 is
`6ef8f392eb8f93b3afccc85b36981cbad1c080d92c81ce99a9bce32c80f5ca40`. Pillow
12.2.0 verifies it and returns one CMYK pixel, `fb ff fd ff` (SHA-256
`fe4cfd863d3977883fd52b5794e14c8ea1cadd264db6925f83b128d1df2ce3e3`).

The TIFF fixture adds private SBYTE, SSHORT, and SLONG fields 65000–65002,
each with value −1, to the pinned grayscale input. Its SHA-256 is
`5e7b1dfc00abf38207e020dc3b39d6fe37fb32e77c7b5382227590057bc63c38`. Pillow
12.2.0 reads all three signed values and produces the same 128×128 grayscale
pixels as `gray.tiff` (SHA-256
`1928fc10cb7c056f9c5de7ee534445e876fac9fd1ea050bc3c8b6eb29371c707`).

Both selected public parity runs passed 1/1 on `aarch64-apple-darwin` with all
features and branch instrumentation:

```text
PYTHON=.oracle-venv/bin/python make coverage-selected COVERAGE_CASES='decode:jpeg:ycck_h2v1_single_sample_1x1' SELECTED_COVERAGE_REPORT=.coverage-mcp/local-evidence/selected/jpeg-ycck-h2v1-singleton-20261022/coverage.json
PYTHON=.oracle-venv/bin/python make coverage-selected COVERAGE_CASES='decode:tiff:grayscale_unknown_signed_private_tags' SELECTED_COVERAGE_REPORT=.coverage-mcp/local-evidence/selected/tiff-signed-private-tags-20261022/coverage.json
```

Coverage MCP's selected-test comparison against the active 2×2 YCCK control
observed the previously missing false arm of `upsample_h2v1_row` at
`src/codecs/jpeg/decode/upsample.rs:436`. Comparing the TIFF row with the
existing `grayscale` row observed six false-arm branch observations at
`src/codecs/tiff/decode.rs:3915–3917`, where the signed private tags take the
non-YCbCr path. Both comparisons are coordinate-only: the local reports have
no source revision or build identity, and Coverage MCP marks test status
unknown. They establish the selected inputs' observed paths, not an aggregate
coverage gain or regression.

## JPEG post-scan junk followed by another marker — 2026-10-22

Added active parity row
`decode:jpeg:baseline_444_single_scan_com_tail_junk_then_com`. The 750-byte
JPEG is derived from the pinned single-scan COM fixture by adding one junk byte
and a second COM segment before EOI. Its SHA-256 is
`1cc70897b39aad9b13eade241695dd0628c4aade3f660ab4d8557d013ea7952c`. Pillow
12.2.0 verifies and decodes it as an 8×8 RGB image. The 192 output bytes match
the existing post-scan COM reference exactly (SHA-256
`7382706a9ecb0f6acae26e7c1c9a9be0e9471233c1a0c0da48d08a458115be2f`).

The new row and the existing two-trailing-COM control each passed 1/1 in
separate selected coverage runs. Coverage MCP's selected-test comparison
observed three additional branch coordinates: the post-scan marker search in
`decode.rs:2766` and `decode.rs:2771`, plus junk skipping in
`parser.rs:107`. The reports use the same branch denominator, but have no
source revision or build receipt and Coverage MCP reports test status unknown;
the comparison is coordinate-only and does not establish an aggregate gain.
The parser's false arm remains an open candidate for a different input.

## AVIF version-1 item-reference alpha parity — 2026-10-20

Added active public parity row
`decode:avif:iref_version_1_alpha_auxl`. The generated fixture widens
`alpha.avif`'s existing `auxl` reference from 16-bit to 32-bit item IDs,
preserving the item 2 → item 1 relationship and encoded AV1 payloads. The
version-1 `iref` grows metadata by four bytes; both file-backed `iloc` extents
are rebased by four bytes. The 6,445-byte input SHA-256 is
`b6f3b78c98b33b32b679d07a91b3f3030aba7c49def8bc8bad8b64243eb2c710`. Pinned
Pillow 12.2.0 produces the exact 64×64 RGBA reference: 16,384 bytes with SHA-256
`7b62ff36a098d8bdea49e0b72c834b6fd3e4e04f70d884eee274fc69ed869ddd`.

The exact selected coverage row passed 1/1 using:

```text
PYTHON=.oracle-venv/bin/python make coverage-selected COVERAGE_CASES='decode:avif:iref_version_1_alpha_auxl' SELECTED_COVERAGE_REPORT=.coverage-mcp/local-evidence/20261020/avif-iref-v1/coverage.json
```

The LLVM report is retained at
`.coverage-mcp/local-evidence/20261020/avif-iref-v1/coverage.json` (SHA-256
`453f03ef1136e645062c84287a3153301381ad7f7e3f18ce5bd04ac02d4fb289`). The
local receipt confirms the exact row passed and fixtures stayed unchanged
during the run; Coverage MCP still labels source/build identity unverified and
test attribution unknown. Its selected report has 2,063/32,138 branches. At
the `from_id` and `to_id` width checks in `container::parse_iref` (lines 1131
and 1141) and `samples::parse_iref` (lines 1028 and 1039), the selected report
no longer lists the false arm as missing in one parser group at each site,
consistent with the version-1 path being exercised. Duplicate groups still
list both arms as missing. The saved full-baseline query lists the false arm
as missing at all four sites. Coverage MCP's incremental comparison is
incomparable because baseline branch detail is incomplete or inconsistent with
its 31,800-branch summary; no aggregate gain or regression is claimed.

The case reaches the version-1 reference parser through public image decode
and retains exact Pillow pixel parity. The remaining missing clone groups do
not establish additional missing input variants.

## Separate-planar TIFF Deflate faults and WebP quality-50 parity — 2026-10-20

Added two public TIFF parity inputs derived from the active separate-planar
RGB tiled Deflate fixture. The invalid-zlib-header input
`planar_separate_rgb_tiled_deflate_invalid_stream.tiff` has SHA-256
`fb0a43f73dc8a957f59f56cbe6d924c449ad4d0d8306649b1ab2f97c33b79ba5`; Pillow
12.2.0 verifies the TIFF header and raises `OSError: decoder error -2` while
loading pixels. The short-output input
`planar_separate_rgb_tiled_deflate_short_tile.tiff` has SHA-256
`2756d3f21cda37ea4aa2b5f02b6f40511e3ce0654bdf1a922a928e64c5ab4454` and has
the same Pillow decode error. It exposed a decoder panic when a valid Deflate
stream produced only three bytes; the decoder now returns a structured
malformed-input error after checking the requested sample range.

Added `encode:webp:enc_lossless_quality_50` on the existing lossless WebP
source. The exact Pillow encoded reference is 3,584 bytes with SHA-256
`46dc365fb19ed82270953c05d810df4c43e0aea7d5bff836f60e271de9e861a5`; its
re-decoded 128×128 RGB reference SHA-256 is
`8a1d6fcc36f5b5e70fbf949d4e612630a1931279383e7c21db27cd3cbad98131`.

The three selected parity cases passed. The report is retained at
`.coverage-mcp/local-evidence/20261020/tiff-webp-deflate-outcomes/tiff-webp-deflate-outcomes-final.json`
(SHA-256
`a47a5359edcaaa554578fa4613302c918dacdd19a879afad9aaf7cd2f505230f`). Its
local receipt confirms source and fixture hashes stayed fixed during the run.
Coverage MCP marks source/build identity unverified and test status unknown.
The LLVM report records the separate-planar `decode_block` error-mapping
closure and truncated-output guard. It also records the quality-50 path through
the `quality > 50` false edge. The full-baseline comparison remains
incomparable because baseline branch detail does not reconcile with aggregate
totals; no aggregate coverage gain is claimed.

These inputs reach behavior through public Pillow-compatible calls, so they
remain parity cases. The 72 typed target-only fault-contract rows remain in
their separate `not_applicable` oracle lane.

## WebP VP8L run-cap parity and benchmark path filter — 2026-10-20

Added active Pillow parity row `encode:webp:enc_lossless_method_0_run_cap`.
Its 4,356×1 RGB source contains 4,099 repeated white pixels followed by 257
distinct colors, keeping the input off the small-palette path. With lossless
method 0, the public encoder reaches the VP8L hash-chain checkpoint at an
internal run of 4,096 and clamps the run to 4,095. The PNG source SHA-256 is
`038d4a47bb2b7f22b62eaac823a897c74607b20ee0ab6999e30b5dc84f6a1a93`; the
13,068-byte Pillow-decoded RGB reference SHA-256 is
`3206f3f36cf23cf01f6a3f2b696cb407eee7c80c81c05a0373154b4a165a6016`; the
186-byte encoded WebP SHA-256 is
`d99c88b7b040f8f34dd8048f54ef0fcb4db4f43a7ffa1a048ddd9008dc863f7c`.

The selected parity run passed 1/1 and re-decoded to 4,356×1 with exact pixel
parity. Its report is retained at
`.coverage-mcp/local-evidence/20261020/webp-run-cap-62daa243/coverage.json`
(SHA-256
`c86f0bc2a039065003df89e788f7def9b9f26812d0939d963bbeb03e8342c16f`). In the
active `fill_hash_chain_impl::<true>` specialization, the report records both
outcomes at the periodic-run check (48 / 13,206) and maximum-run check (3 / 9).
The local receipt marks source/build identity unverified and named test
attribution unknown. Coverage MCP groups untouched sibling monomorphizations
separately, and its comparison with the full baseline is incomparable because
the baseline branch detail does not reconcile with its aggregate totals. These
are local path observations, not an aggregate coverage gain claim.

This input reaches the behavior through normal public encoding, so it remains
Pillow parity. The 72 typed target-only fault-contract rows remain in their
separate lane with oracle status `not_applicable`.

## WebP method-six hash-chain run-cap parity — 2026-10-22

Added active Pillow parity row `encode:webp:enc_lossless_method_6_run_cap`.
It reuses the 4,356×1 RGB run-cap source with lossless quality 80 and method 6,
which selects `fill_hash_chain_impl::<false>`. The selected all-feature
coverage run passed 1/1 and re-decoded to exact 4,356×1 RGB pixels. Pillow's
616-byte WebP reference has SHA-256
`8abd0df9e4f0d5292902f75bc9e0be22261b7045b2266ebf895d0c17a46e00d4`; its pixel
reference is the existing 13,068-byte source result with SHA-256
`3206f3f36cf23cf01f6a3f2b696cb407eee7c80c81c05a0373154b4a165a6016`.

The selected report is retained at
`.coverage-mcp/local-evidence/20261022/webp-method-6-run-cap/coverage.json`
(SHA-256
`8264ac4c3cbfdd41fb53820c6c4221533a7bd1f877ef493a77cf9fc47e4f6ad9`). In the
executed false specialization, the periodic-run check recorded 57 true and
15,453 false outcomes; the `run > MAX_LENGTH` check recorded 3 true and 12
false outcomes. The source file hash at measurement was
`b9cb0e990625fe385ce49f4d96e84acc609242353e949d89be17093939e787cc`. The
runner log records the selected test passing. Coverage MCP marks the report's
source/build identity unverified and test attribution unknown; comparison
with the older selected report is limited. These observations do not establish
an aggregate coverage gain.

The x86 benchmark workflow's pull-request path filter now includes
`src/cancel.rs`, which the benchmark uses for cancellation-aware encodes. Its
release-policy regression test and the local selected parity test passed. The
CI filter fix is committed on `main` as `62daa2436d7802294ed253f2e54953862a6509aa`;
the GitHub CI, documentation, and paired SSE2/AVX2 benchmark runs were triggered
on that commit.

The current fixture matrix contains 2,444 total rows: 1,941 decode / inspect /
verify rows and 503 encode rows. Of those, 1,941 decode rows and 471 encode
rows are active; the other 32 encode rows are planned. The separate
fault-contract lane contains 72 rows. The coverage-origin inventory records
189 exact `cfg(coverage)` guards across 28 Rust files.

## Full coverage baseline and AV1 film-grain overlap parity — 2026-10-20

Refreshed the local all-feature coverage baseline on source revision
`34c3930f2e7f6fdad102a421e15170fc2e202688`. The report is retained at
`.coverage-mcp/local-evidence/20261020/main-full-34c3930f/coverage.json`
(SHA-256
`0b28133d95eb8c736f83dccc87cfbcf7eaea57a89c9031dade323c2e8716383b`). The
run used:

```text
CARGO_INCREMENTAL=0 RUSTC_WRAPPER= PYTHON=.oracle-venv/bin/python CARGO_BUILD_JOBS=2 make coverage COVERAGE_REPORT=target/release-evidence/coverage-main-full-20261020.json
```

The all-feature coverage gate passed. The run passed 57/57 coverage-matrix
tests, 7/7 decode-policy tests, 1/1 determinism test, 68/68 feature-gate tests,
4/4 animation-loop tests, and the capability-table test. The full report
measures 91,909/138,417 lines (66.4001%), 18,108/31,800 branches (56.9434%),
5,079/8,855 functions (57.3574%), and 140,584/214,895 regions (65.4199%).
Coverage MCP labels its source identity unverified and test attribution
unknown; this is a fixed local exploration baseline, not a registered
source/build receipt.

Coverage MCP located uncovered outcomes in
`src/codecs/avif/av1/film_grain.rs::overlapped_grain`. Added the public parity
row `decode:avif:portable_lossless_filmgrain_i444_overlap_64x64` by enabling
only the inspected AV1 overlap flag in the existing 64x64 all-lossless I444
source. The generator confirms unchanged BMFF sample extents and all 15,555
tile-payload bytes. The input is 15,960 bytes with SHA-256
`345a3e16a1d9574a6ac39c4b14eaa93ba70b5d49714cae2188fdff03c252e03a`; its
12,288-byte Pillow RGB reference has SHA-256
`46bfc877578f36fc8d3b376c3ac41e6be9ff98bb14fda447a0b0baeeba0b97d7`.

The selected coverage-matrix run passed 1/1. Its LLVM report is retained at
`.coverage-mcp/local-evidence/20261020/avif-filmgrain-overlap-34c3930f/coverage.json`
(SHA-256
`418c268e65838ac45a232cc52b119b894843c94b84f321721b5eb1fbafbe72b8`). The
hit-bearing `overlapped_grain` specialization recorded 36,864 calls and both
outcomes at its branch conditions. Coverage MCP still reports missing branch
coordinates for another linked specialization. The incremental comparison
with the full baseline is incomparable because the baseline's detailed branch
data does not reconcile with its aggregate totals. No aggregate coverage gain
or complete branch-closure claim is made.

The parity matrix now contains 2,439 total rows: 1,938 decode / inspect /
verify rows and 501 encode rows. Of those, 1,938 decode rows and 469 encode
rows are active; the other 32 encode rows are planned. The separate
fault-contract lane contains 72 rows. The coverage-origin inventory records
189 exact `cfg(coverage)` guards across 28 Rust files.

## Unaligned TIFF YCbCr strips and CI gate hardening — 2026-10-20

Added a Pillow-parity fixture for subsampled YCbCr TIFF strips whose
`RowsPerStrip` boundary cuts through a vertical clump. The public decoder now
converts each strip using its own clump rows. A separate typed fault-contract
row `fault-contract:tiff:tiff_unaligned_strip_assembly_reservation_failure`
injects strip-output reservation failure on this path and checks the structured
error followed by a successful retry; its oracle is
`not_applicable`, so it is excluded from Pillow parity totals. The workflow
policy checker now rejects `continue-on-error` on required CI jobs and their
steps, with regression mutations for both levels.

The TIFF input SHA-256 is
`fec83d5c16fb6b391c5bb7bf3273a3f9e009dcd6937ba4ce795f1ad086f84a4d`; its exact
48-byte RGB output SHA-256 is
`ee406bf055bf628b9b0a3b6d3ea29a5f32d97e963d3eb4964d5942c37f04881a`. The
selected coverage run passed 1/1 parity rows and 1/1 fault-contract rows. Its
report is `target/release-evidence/selected/tiff-unaligned-ycbcr-fault-contract-final.json`
(SHA-256 `498567163da32491699c14b375c48d4d7030002a7be9c7f73c7ba31ca661cc3b`).
Coverage MCP reports the unaligned-strip predicate's false arms as missing in
this selection and no missing branch outcomes at the reservation hook. The
report has unverified source/build identity and unknown named-test attribution,
so this is coordinate evidence only. No full coverage campaign was run.

`cargo fmt --all -- --check`, strict all-target/all-feature Clippy,
`make workflows-check`, the focused workflow-policy regression test, fixture
integrity, and the typed fault-contract validator passed.

The current fixture matrix contains 2,439 total rows: 1,938 decode / inspect /
verify rows and 501 encode rows. Of those, 1,938 decode rows and 469 encode
rows are active; the other 32 encode rows are planned. The separate
fault-contract lane contains 72 rows. The coverage-origin inventory records
189 exact `cfg(coverage)` guards across 28 Rust files.

## AV1 odd-width I420 super-resolution parity — 2026-10-20

Added `decode:avif:superres_actual_upscaled_i420_17x17`, a lossy 17x17 AVIF
still that signals super-resolution denominator 9. Its coded luma width is 16
and upscales to 17; ceil-rounded I420 chroma width grows from 8 to 9. The
fixture generator validates the AV1 geometry, repeat-encodes byte-identically,
and checks Pillow 12.2.0 `verify()`/`load()` plus the exact RGB output. Input
SHA-256:
`5824f2bd3c3fa431a0c0c57509f34e11ec148e36bac9531bff083394eab59d2f`; the
867-byte decoded pixel SHA-256 is
`0140c8dab75efad711db2af0b7f1f837e76e2a8d578ac43052fea15b7693d926`.

The selected public parity row passed 1/1. Coverage MCP lists branch
coordinates in `upscale_subsampled_leaf` and `resize_plane`, including the
unobserved injected header-mismatch arm and invalid-plane-geometry arms. It
also lists the unsampled non-subsampled width arms at `resize.rs:223` and
`resize.rs:228`; this fixture follows the 4:2:0 subsampled arms. The report is
local-only with unverified source/build identity and unknown test attribution,
so these are coordinate observations rather than a marginal coverage claim.
Separately, the existing target-only
`fault-contract:avif:avif_superresolution_header_geometry_mismatch` row passed
1/1 with oracle status `not_applicable`; it remains outside Pillow parity
counts. The gap query for that selected report omits the injected mismatch arm
at `resize.rs:206`, while still listing non-subsampled width and invalid-plane
geometry arms. Its report is also local-only with unverified source/build
identity and unknown named-test attribution, so this does not establish an
aggregate or marginal coverage gain. No full coverage campaign was run.

The selected report is
`target/release-evidence/selected/20261020T000909Z/coverage.json` (SHA-256
`ad5903cec36bcdcb8f8c9d7a219c2e902d2fd5d8e571de08ac50c0f6943b8c3d`).
The selected fault-contract report is
`target/release-evidence/selected/20261019T234822Z/coverage.json` (SHA-256
`f102498b91b8b11e8034b16ec7315cc4edf802e9998d0608dd02f2639676feba`).

## JPEG trailing JFIF APP0 parity — 2026-10-19

Added `decode:jpeg:baseline_444_single_scan_trailing_app0`, a complete baseline
4:4:4 JPEG with a valid JFIF APP0 segment after its only entropy scan. Pinned
Pillow 12.2.0 accepts the file in both `verify()` and `load()`; it decodes to
8×8 RGB and the exact 192-byte pixel output matches the existing trailing-COM
and trailing-APP1 cases. The input SHA-256 is
`507c2cf54fb8d6c38bf75020b6371df6e74b639eefc5c8e7091b6657b1c9e646`; the
pixel SHA-256 is
`7382706a9ecb0f6acae26e7c1c9a9be0e9471233c1a0c0da48d08a458115be2f`.

The JPEG parser already accepts APP0 segments. The baseline single-scan fast
boundary recognizer now treats APP0 like the supported APP1, COM, DHT, and DQT
segments, keeping valid metadata after the scan out of entropy extraction.
This is a bounded marker match with no extra allocation or pixel-loop work.
The selected coverage run passed 1/1 and checked that decoded opaque metadata
retains the exact APP0, COM, APP0 order and bytes. Coverage MCP's line-filtered
branch query reports only the invalid-marker (`true`) arm at
`decode.rs:2774` as missing in this selection, consistent with APP0 taking the
accepted (`false`) arm. The incremental comparison with
`coverage-main-20261019.json` is incomparable because its branch detail does
not reconcile with the totals. Both MCP measurements lack source/build
receipts, and MCP test attribution is unknown. The local selected-run receipt
records the exact case, source and fixture hashes, and passing test process.
No aggregate coverage gain or full-suite claim is made; no full coverage
campaign was run.

The focused report is
`target/release-evidence/selected/20261019-jpeg-trailing-app0-as-chunks-f4e98437/coverage.json`
(SHA-256 `eec4c738e6ef1eba9d853d47ce6e88682bba93a3413a534fd4f8b827329b3c75`).

## TIFF RGB16 Deflate-compressed contiguous strips — 2026-10-19

Added `decode:tiff:rgb16_le_deflate_contiguous_strips` for a little-endian
16-bit RGB TIFF with two independently Deflate-compressed, one-row contiguous
strips. It relies on the default unsigned sample format and omits the default
contiguous planar configuration and predictor tags. Pinned Pillow 12.2.0
opens it as 2×2 RGB and returns the exact pixel bytes
`12569aab00ff0080ffff017f`. The fixture generator verifies the RGB
photometric tag, compression, strip offsets and lengths, and the omitted
default tags. Input SHA-256:
`b3a35c9135e45daf9016229f803eaade5923b506830b491cd001c9db1899e369`; output
SHA-256:
`d7973d0ab7bfd18e2fe3666bcdb5367d84d3b854b58bab793799d0803fb63c1b`.

The generic TIFF strip decoder already bounds each strip, decompresses to the
expected row size, and converts RGB16 samples to RGB8. The inspection
predicate now accepts this contiguous Deflate layout, with no pixel-loop or
allocation change. The final selected coverage run passed 1/1. Coverage MCP's
line query lists two missing observations at `inspect.rs:396`. The report's
raw branch records show ten executions of the `planar == 1` and `!tiled`
outcomes, but zero counts for a wider joined span on the same line. MCP marks
the report's source unverified and test attribution unknown, so this mapping
remains a coordinate lead and does not establish marginal coverage. Its
incremental comparison with `coverage-main-20261019.json` was incomparable
because baseline branch detail does not reconcile with its totals. The local
receipt records the selected case, passing process, matching source-file
hash, and fixture hashes; no aggregate coverage gain or full-suite claim is
made. No full coverage campaign was run.

The focused report is
`target/release-evidence/selected/20261019-tiff-rgb16-deflate-strips-b3c311c69-clippy/coverage.json`
(SHA-256 `ed984dbed71d978fab31b2f11b4d81055c7049ab36f371fdb4617e4bab258544`).

## TIFF unsigned RGB16 strip parity and allocation fault contract — 2026-10-19

Added little- and big-endian, uncompressed 2×2 RGB16 TIFF fixtures with two
one-row strips. The little-endian input relies on TIFF's default unsigned
sample format, while the big-endian input stores `SampleFormat=1` for all
three channels. Pinned Pillow 12.2.0 converts each unsigned sample to RGB8 by
retaining its high byte; both files produce the exact 12-byte pixel sequence
`12569aab00ff0080ffff017f`. The input SHA-256 values are
`aebc7d664d848fe2d4e5054c4926ea6b38b99665d2d31bd6c86419aa07b7874b` (little
endian) and
`9b91240255da2d849d12be62e59c3d868721112253ed46f1284578597567b0e3` (big
endian).

The selected matrix run passed both parity rows and the target-only
`fault-contract:tiff:tiff_rgb16_output_reservation_failure` row. The fault
returns the structured TIFF RGB16 output allocation error, then a retry
matches a fresh decode. This contract has `oracle_status: not_applicable` and
is reported outside Pillow parity. Its source case is the little-endian parity
fixture.

The selected branch report is
`target/release-evidence/selected/20261019T040515Z/coverage.json` (SHA-256
`249d565c2daacb13aa16001744aa85e33a07f4ba451c30bded8cbf94f9c69ef9`). Coverage
MCP records 174/31,674 observed branches and no missing branch groups in the
RGB16 conversion or output-reservation helpers; it reports 24 missing branches
in the shared TIFF layout function. The report has unverified source/build
identity and unknown test attribution, so these are coordinate-only
observations and do not establish an aggregate coverage gain. The focused
runner log records 2/2 Pillow decode rows and 1/1 fault contracts passing. No
full coverage campaign was run.

## TIFF subsampled YCbCr checkpoints and odd clump edges — 2026-10-19

Added five Pillow-backed decode rows for 1×2 and 4×2 subsampling. The three
32×32 inputs exercise the 1,024-pixel conversion checkpoint through the
specialized 1×2 path, the general 4×2 path, and custom-coefficient tables. Two
33×33 4×2 inputs add odd final clumps; both clip the final row and column while
still reaching the checkpoint. Pinned Pillow 12.2.0 generated the exact RGB
references for all five cases.

Selected branch runs passed all five rows: the individual 1×2, default 4×2,
and custom 4×2 reports are under
`target/release-evidence/selected/tiff-ycbcr-checkpoint-1x2.json`,
`target/release-evidence/selected/tiff-ycbcr-checkpoint-4x2-retry1.json`, and
`target/release-evidence/selected/tiff-ycbcr-checkpoint-custom-4x2-retry1.json`;
the two odd-edge rows passed in
`target/release-evidence/selected/tiff-ycbcr-checkpoint-4x2-odd-edges.json`.
Coverage MCP finds no missing branch groups at the general converter's current
checkpoint and clipping conditions (`decode.rs:2124`, `2131`, `2155`) in the
combined odd-edge report. Its selected-tests comparison with the even 4×2
report observes 13 new branch coordinates, including both true clipping arms.
The MCP marks the comparison limited because source/build identity is
unverified and test attribution is unknown. Its incremental comparison cannot
combine the selected reports because exact detail does not reconcile with the
reported totals. The 1×2 report's raw LLVM record shows both checkpoint
outcomes (5 and 5,115), while MCP still lists that specialized line as missing;
therefore no aggregate gain or regression is claimed. No full coverage
campaign was run.

## AVIF mixed-segment inter parity and TIFF YCbCr fault contract — 2026-10-18

Added the active public parity case
`decode:avif:animated_mixed_lossless_inter_b32_i420_64x64`. Its two-frame
64×64 8-bit I420 sequence contains both lossy and qindex-zero lossless
segments in a full-visible inter-coded B32x32 leaf. The selected Pillow 12.2.0
comparison passed 1/1 with exact 12,288-byte RGB parity. The fixture hash is
`ecb244cfa33b1737f09246cf87ad20dbd8156108dc9b93da76a8bbe885007257`; frame
reference hashes are `90ce523631e9fd385bcae414317ca29e655e9fa26a022e09d50795e71b32d634`
and `aa3d0199115d1734da9065059f33f648e4f90151fbce35dc3cd670a5f6b479f4`.

Added target-only
`fault-contract:tiff:tiff_ycbcr_general_rgb_output_reservation_failure` for
the generic 4×2 subsampled converter. Its coverage-only injection reuses the
existing reservation fault point; production builds reserve the original
exact output length. The selected contract passed 1/1, returning the public
typed allocation error and then matching a fresh decode on retry. The
Pillow-backed source parity control also passed 1/1. This row has
`oracle_status: not_applicable` and remains outside Pillow parity totals.

Selected coverage reports are
`target/release-evidence/selected/20261018-avif-mixed-lossless-inter-b32-final.json`
(SHA-256 `b49001beeaeca0880ad6b39743aac23eff5424ca507195f15ee1ed776f91f60f`),
`target/release-evidence/selected/20261018-tiff-ycbcr-general-reservation-fault-final.json`
(SHA-256 `8c48cebf2b1553581f10db116bc537da851d3341be80e8a4d5bdbb2ca452dcf7`),
and the two existing parity controls in
`target/release-evidence/selected/20261018-avif-tiff-edge-controls.json`
(SHA-256 `c408ded30954b71475dca52e5c2136d60d40bf59b9d082b3efa5fc9209b44689`).
Coverage MCP's selected-tests comparison reported 285 newly observed branch
coordinates for the AVIF entropy path and one for the generic TIFF reservation
hook. These measurements have unverified source/build identity and unknown
test attribution, so they are coordinate-only evidence; no aggregate coverage
gain or regression is claimed. No full coverage campaign was run.

At the time this 2026-10-18 entry was recorded, the fixture matrix contained
2,434 total rows: 1,934 decode / inspect / verify rows and 500 encode rows. Of
those, 1,934 decode rows and 468 encode rows were active; the other 32 encode
rows were planned. The separate fault-contract lane contained 71 rows. The
coverage-origin inventory recorded 188 exact `cfg(coverage)` guards across 28
Rust files.

## BMP decoded-pixel reservation fault contract — 2026-10-17

Added the target-only row
`fault-contract:bmp:bmp_decoded_pixels_reservation_failure`, which reuses the
active `decode:bmp:depth_24` Pillow input. The 24-bit decoder now reserves its
RGB output with `try_reserve_exact`; the coverage-only fault makes that
reservation fail and the public API returns a structured dimensions error.
The normal retry is compared with a fresh decode. The selected fault contract
passed 1/1 with `oracle_status: not_applicable`, outside Pillow parity, and the
ordinary `depth_24` parity row passed unchanged.

The selected branch report is
`.coverage-mcp/local-evidence/20261017/bmp-decoded-pixels-fault/coverage.json`
(SHA-256
`1929fb205c059466e7957c8713a82cea5533f3afa4bc42bff20608727e1e481c`). It
contains 51/31,478 observed branches overall. Coverage MCP reports no missing
branch group spanning `src/codecs/bmp/decode.rs:696` or
`src/codecs/bmp/decode.rs:705`. The report has no source/build receipt and
unknown test inventory, so these are coordinate-only observations; the
selected runner output is the evidence for the one fault case. The incremental
comparison with the saved full report was incomparable because exact branch
detail did not reconcile with its totals; no coverage union or regression
claim is made. No full coverage campaign was run.

## GIF pre-frame zero padding parity — 2026-10-17

Added `decode:gif:pre_frame_zero_padding` for a complete 1x1 indexed GIF with
a zero padding block before its first image descriptor. Pinned Pillow 12.2.0
verifies and loads the input as one `P` frame with pixel `00`; the fixture SHA-256
is `4c25c0da8eb0126a59c3b171a3316a9d8fed08697074a75f19dc0add1bb9a044` and the
exact pixel-reference SHA-256 is
`6e340b9cffb37a989ca544e6bb780a2c78901d3fb33738768511a30617afa01d`. The
selected matrix run passed with the post-frame padding control and no-frame
inputs. It also exercises basic inspection and the exact 34-byte metadata
limit. A separate zero-padding-before-trailer input still matches Pillow's
no-image error.

The selected branch report is
`.coverage-mcp/local-evidence/20261017/gif-pre-frame-zero-padding-selected/coverage.json`
(SHA-256
`ac416dd62e5b1f608917d2c903e52feffa2903bf4682319cad7148e6ac28c12f`). It
contains 110/31,476 observed branches overall. Coverage MCP reports no missing
branch group spanning `src/codecs/gif/decode.rs:175`,
`src/codecs/gif/decode.rs:420`, or `src/codecs/gif/inspect.rs:143`, covering
still/sequence parsing, metadata measurement, and inspection padding paths.
The report has no source/build receipt and unknown test inventory, so these
remain coordinate-only observations. The current fixture matrix contains 2,404 total rows: 1,904 decode / inspect / verify rows and 500 encode rows.
Of those, 1,904 decode rows and 468 encode rows are active; the other 32 encode
rows are planned.

## TIFF equivalent-default YCbCr rational parity — 2026-10-18

Added `decode:tiff:ycbcr_equivalent_default_rational_lzw_2x2`, a complete
4×4 LZW TIFF with explicit coefficient and reference fractions that equal the
YCbCr defaults but use noncanonical numerator/denominator pairs. Pinned Pillow
12.2.0 accepts the input as RGB and returns the same exact 48 bytes as the
default-tag fixture. The input SHA-256 is
`06a8a28ba0ebe1359d107ca2d7c442eb02dcc27cba8b85b06c16eaa610e7fa65`; the
pixel-reference SHA-256 is
`af15457a9372a158dd276ced4109b650b99def573093a49f70dbb469c2262668`.

The focused selected matrix run passed 1/1. Its report is
`target/release-evidence/selected/ycbcr_equivalent_default_rational_final.json`.
Coverage MCP reports 234/31,634 observed branches for that selected report.
The selected-tests comparison with three existing YCbCr rows found zero newly
observed branch coordinates and three coordinates not observed by the new
case. Both reports are source-unverified with unknown test attribution, so
this is limited coordinate evidence, not a measured aggregate coverage gain.
The old full report predates the current source and is not combined with this
selection. The case remains useful as a distinct parity contract for explicit
fractions equivalent to defaults; no fault injection or full coverage run was
needed.

## Codec edge and progressive JPEG restart parity — 2026-10-18

Added public Pillow parity rows for OS/2 core 4-bit BMP padding, GIF Plain Text
Extension handling, two-page transparent TIFF output, 16-bit partial-frame
APNG composition, and a trailing DHT after a single-scan baseline JPEG. Added
progressive JPEG restart-row parity for RGB, grayscale, and CMYK, plus a 33x33
4:2:0 input that ends on partial MCUs. Two malformed trailing-DHT cases verify
that Pillow accepts the JPEG structure but rejects truncated Huffman counts or
symbol payloads during pixel loading.

The 33x33 case exposed missing dummy-block DC symbols in interleaved scans and
missing inherited DC refinement bits. The encoder now emits the padded MCU
events Pillow expects. The selected decoder matrix passed 5/5 format-edge rows
and the focused trailing-DHT matrix passed 3/3 rows. The selected progressive
JPEG matrix passed 4/4 rows with 20 public encode calls and zero panics. The
new partial-MCU output is 1,331 bytes; its source SHA-256
is `afdec16d964c39b1d8f8329f72d798669ce507bf0904f96388fd273ddf65cc45` and
its Pillow encoded-reference SHA-256 is
`0185f9326f806ea684d69619abaa48e1abebf62d1d511ba0ca349017a8566f29`.

The selected target-only JPEG coefficient-reservation fault contract also
passed 1/1 with `oracle_status: not_applicable`, separately from Pillow parity.
Coverage MCP reports 726/31,634 branch observations in the final format-edge
decoder report, 292/31,634 in the trailing-DHT report, and 715/31,634 in the
progressive JPEG report. Its targeted query found no missing branch arms at
the new partial-MCU block-bound checks. It still lists the internal malformed
DHT bounds at `decode.rs:2596` and `:2602` as unobserved, despite the public
error-parity rows; those coordinates need further reachability analysis. The
reports have unverified source identity and unknown test attribution, so these
are exploration coordinates, not measured coverage gains. Reports:
`target/release-evidence/selected/20261018-edge-decode-final.json`,
`target/release-evidence/selected/20261018-progressive-jpeg-restarts-final.json`,
`target/release-evidence/selected/20261018-jpeg-trailing-dht-boundaries.json`,
and `target/release-evidence/selected/20261018-jpeg-fault-contract-coverage.json`.
No full coverage campaign was run.

## TIFF YCbCr rational edge parity — 2026-10-18

Added public Pillow parity cases for negative-equivalent `SRATIONAL` coefficient
and reference values, plus exact-count `DOUBLE` coefficients. Pillow displays
the negative-equivalent pairs as positive ratios, while libtiff consumes their
stored numerator as signed and denominator as unsigned during YCbCr conversion;
the fixtures retain the exact pixel bytes from that behavior. A `DOUBLE`
coefficient tag with the required three values opens and verifies but fails
during pixel decoding, while unsupported coefficient counts continue to use
libtiff defaults. These are
Pillow-reachable input cases, so they remain ordinary parity rows rather than
target-only fault contracts.

The final selected TIFF YCbCr rational matrix passed 25/25 active parity rows
in `target/release-evidence/selected/tiff-ycbcr-rational-edges-final-v3.json`.
Coverage MCP reports 260/31,512 branch observations for that report and lists
remaining coordinates in `ycbcr_parameters` and `read_ycbcr_rationals` around
`inspect.rs:441`, `:465`, `:469`, `:506`, and `:520`. The report has no recorded
revision or build ID, an unverified source state, and unknown test attribution;
these are exploration coordinates only and do not establish a measured source
coverage gain.

After the reference-limit comparison was rewritten with checked arithmetic to
satisfy strict Clippy, the accepted and rejected boundary rows were rerun and
passed 2/2 in
`target/release-evidence/selected/tiff-ycbcr-reference-limit-checked-arithmetic-v1.json`.
Coverage MCP lists ten missing branch coordinates in
`ycbcr_reference_exceeds_conversion_limit`, including spans around
`decode.rs:2332` and `:2337`. This selected report also has no source/build
receipt or test attribution, so the coordinates remain exploratory rather
than proof of current-source coverage.

## TIFF LZW mid-stream CLEAR parity — 2026-10-18

Added `decode:tiff:lzw_midstream_clear_width_reset`, a complete grayscale TIFF
whose LZW codes cross the 9-to-10-bit early-change boundary, issue CLEAR at
10 bits, and continue with 9-bit literals. The generator verifies the code
width at the boundary and after CLEAR, then checks Pillow 12.2.0's mode `L`,
330×1 dimensions, exact pixels, and SHA-256
`a50858b23acd18e82d5012a8a115b2a905f22610822f86e5622aef99ebfa3cf4`.
The selected public parity row passed 1/1.

Coverage MCP's selected LLVM report records 193/31,586 branch observations
overall and has no missing branch groups on the LZW CLEAR conditions at
`decode.rs:2960` and `decode.rs:3055`. The report has unverified source and
unknown test attribution. Its incremental comparison to the saved full report
is incomparable because the baseline lacks compatible detail; no aggregate
coverage gain or full-suite result is claimed. Report:
`target/release-evidence/selected/20261018T035838Z/coverage.json`.

## TIFF YCbCr FLOAT and DOUBLE parity — 2026-10-18

Added five valid public parity rows for IEEE FLOAT/DOUBLE coefficient and
reference-black-white tags, including an accepted infinite DOUBLE coefficient.
Four error-parity rows cover NaN coefficients and infinite reference values.
The generated TIFFs contain real IEEE payloads;
the malformed existing DOUBLE-coefficient row, which retains RATIONAL bytes,
is unchanged. Pinned Pillow 12.2.0 produced the exact RGB bytes and decoder
errors. These are ordinary parity cases because complete TIFF inputs reach each
behavior; no target-only fault contract was needed for these paths.

The focused run selected all nine new parity rows, the retained malformed
DOUBLE-coefficient row, and the five existing TIFF YCbCr fault contracts. All
10/10 parity rows and 5/5 target-only contracts passed.
The LLVM report is
`.coverage-mcp/local-evidence/20261018/tiff-ycbcr-float-double-v3/coverage.json`.
Coverage MCP reports 277/31,568 branch observations in this selected report,
but marks source identity unverified and test attribution unknown; it is not an
aggregate coverage gain or full-suite claim. Its remaining
`floating_value` index-bound arm is defensive: `read_ycbcr_rationals` checks
exact field counts before requesting each array index, so no public TIFF input
reaches an out-of-range call.

## TIFF YCbCr subsampling and positioning parity — 2026-10-18

Added seven public Pillow parity inputs for the supported 2x1, 4x1, 4x2, and
4x4 clump layouts, including odd image edges and custom coefficients. Added
three more inputs showing that libtiff ignores malformed YCbCr positioning
metadata (tag 531) during RGB conversion. The selected run passed all 10 rows
with pixel parity. The four existing TIFF YCbCr fault-contract rows also passed
4/4 with `oracle_status: not_applicable`.

The selected public report is
`target/release-evidence/selected/20261017T223455Z/coverage.json`; the selected
fault-contract report is
`target/release-evidence/selected/20261017T223916Z/coverage.json`. Coverage MCP
reports no missing branch groups for the general converter's odd-edge clipping
checks at `decode.rs:2011` and `:2018`. It still lists uncovered defensive
validation arms for zero clump factors and mismatched sample lengths in the
general converter. Both reports have no source/build receipt and unknown test
inventory; they are selected-test exploration evidence and do not establish an
aggregate coverage gain or full-suite coverage claim. The baseline comparison
was incomparable because the saved baseline lacks matching coverage detail and
verified receipts.

## APNG duplicate control before the default image — 2026-10-17

Added the public `decode:png:apng_duplicate_actl_before_idat` row. Its
CRC-checked fixture duplicates the `acTL` chunk before `IDAT` from
`apng_rgba_controls.png`; the input SHA-256 is
`18b489f5cea0c7e3e58550afcc2ce169a12d377d5f7e357b1de2714ab5c67022`.
Pinned Pillow 12.2.0 warns that the APNG is invalid, then verifies and loads
the static default image as one 4x4 RGBA frame. The exact pixel reference SHA-256
is `21b7dac1be9dacf604fd4475304d962dc50a83b6b9623340f59de886d398b8c6`.
The selected public matrix run passed for this case and the existing duplicate
`acTL` after `IDAT` control.

The selected branch report is
`.coverage-mcp/local-evidence/20261017/png-apng-pre-idat-duplicate-actl/coverage.json`
(SHA-256
`b25cc6e2ac8dc97e8521c5a0dfe4e53e99a5758c925582ecd4998e1b7c4a32de`). It
contains 249/31,482 observed branches overall. Coverage MCP reports no missing
branch group spanning `src/codecs/png/decode.rs:1032`, the duplicate-control
fallback arm. The report has no source/build receipt and unknown test
inventory, so this is coordinate-only reachability evidence. The current
fixture matrix contains 2,296 total rows: 1,802 decode / inspect / verify rows and 494 encode rows.
Of those, 1,802 decode rows and 462 encode rows are active; the other 32 encode
rows are planned.

## WebP oversized alpha ANMF height parity — 2026-10-17

Added the public `pillow_tolerated_malformed_animated_alpha_anmf_height_mismatch`
row. Its deterministic 64x64 animated alpha WebP declares the first ANMF frame
as 64x65 while retaining the original nested ALPH and VP8 bitstream. Pinned
Pillow 12.2.0 verifies and decodes it as two 64x64 RGBA frames; the exact frame
references match the existing oversized-width control. The selected
`test_decode_matrix_webp` parity run passed 1/1.

A selected branch-coverage report at
`.coverage-mcp/local-evidence/20261017/webp-anmf-height-selected/coverage.json`
has SHA-256
`148e3c20d0ed7b8964e2538635f3d20aab480d348faa00d1a592e6bf41ec4b3a`.
Coverage MCP reports both outcomes at
`src/codecs/webp/inspect.rs:310` in this selected report. The report has no
source/build receipt and test inventory is unknown; it is coordinate-only
reachability evidence. The incremental comparison with the saved 2026-10-16
full report is incomparable because its branch details do not reconcile with
its totals. No aggregate coverage gain or full-suite regression conclusion is
claimed. The current fixture matrix contains 2,288 total rows: 1,794 decode /
inspect / verify rows and 494 encode rows. At that point, 1,794 decode rows
and 462 encode rows were active; the other 32 encode rows were planned.

## WebP VP8L alpha hint parity — 2026-10-17

Added a paired VP8X/VP8L alpha-flag parity case. The
`pillow_tolerated_malformed_vp8x_vp8l_alpha_flag_mismatch` fixture sets the
VP8L alpha hint while keeping the VP8X alpha bit clear, without changing the
image bitstream. Pinned Pillow 12.2.0 accepts it as an opaque 128x128 RGBA
image; the exact pixel reference SHA-256 is
`6b4e0e27ac0a042958c1a976561c6d890fe2042e3e54c8e898cd859c87db332b`. The
complementary `pillow_tolerated_malformed_vp8x_alpha_flag_without_vp8l_hint`
fixture sets only the VP8X alpha bit; Pillow reports RGB and the exact pixel
reference SHA-256 is
`8a1d6fcc36f5b5e70fbf949d4e612630a1931279383e7c21db27cd3cbad98131`.

The first selected Rust matrix run failed because inspection reported RGB. The
public codec now takes still VP8L alpha metadata from its parsed image header,
and the selected pair and complete WebP matrix tests pass. A selected
branch-coverage report is at
`.coverage-mcp/local-evidence/20261017/webp-vp8x-alpha-pair-selected/coverage.json`;
its SHA-256 is
`0631eeae65b1d0a7b8abcc1d49fe4a0aa4b5d675393e59282405af5fca40223e`.
Coverage MCP reports no missing branch group at
`src/codecs/webp/inspect.rs:247` and one unobserved false outcome at
`src/codecs/webp/native/decoder.rs:329`. The report contains 256/31,460
observed branches, has no source/build receipt, and has unknown test inventory;
it is coordinate-only reachability evidence. The selected-test absence at line
329 is not a regression. Comparison with the saved 2026-10-16 full report is
incomparable because branch details do not reconcile with its totals. No
aggregate coverage gain or full-suite regression conclusion is claimed. The
current fixture matrix contains 2,291 total rows: 1,797 decode /
inspect / verify rows and 494 encode rows. Of those, 1,797 decode rows and 462
encode rows are active; the other 32 encode rows are planned.

## WebP extended duplicate image chunk rejection — 2026-10-17

Added four malformed extended-container parity rows for duplicate VP8L,
duplicate VP8, and both VP8L/VP8 mixed-chunk orders, plus a simple WebP case
where Pillow tolerates a duplicate VP8 chunk. The inputs are derived from
pinned fixtures and have their RIFF extents updated. Pillow 12.2.0 recognizes
each malformed WebP signature, then rejects inspection, verification, still
decode, and sequence decode with `OSError: could not create decoder object`.
The simple duplicate-VP8 file remains accepted. All five public matrix rows
pass their recorded operation outcomes. Full inspection and the native
extended decoder reject a second image chunk while the simple WebP path keeps
Pillow's tolerant behavior; VP8L headers continue to be read through a
chunk-bounded reader.

The selected five-row WebP branch report is at
`.coverage-mcp/local-evidence/20261017/webp-extended-image-chunk-parity/coverage.json`;
its SHA-256 is
`fbc64e64fb1f0e56ec79f2687180d49128133a222695fe2ff45d993b459b8a09`.
Coverage MCP shows the compound inspector guard's true path and the duplicate
VP8 decoder error path were reached; the simple duplicate-VP8 path is included
in this selected run. The report observes 313 of 31,460 branch coordinates;
false short-circuit outcomes remain among the reported inspector gaps.
Source/build identity is unverified and test inventory is unknown, so this is
coordinate-only reachability evidence.

A Coverage MCP incremental regional comparison used the duplicate-VP8L and
alpha-flag rows as a three-row baseline and the four additional chunk cases as
the new batch. It returned `limited`: 2,998/212,096 baseline regions, 4,993
batch regions, 3,447 newly observed coordinates, and 1,546 already covered.
The projected union is 6,445/212,096 regions (+1.625 percentage points on
that denominator). Both reports lack source/build identity and test inventory,
so the union is exploratory coordinate evidence, not a verified suite gain.
Branch and line incremental comparisons were `incomparable` because detailed
locations did not reconcile with report totals. The baseline report is
`.coverage-mcp/local-evidence/20261017/webp-image-chunk-baseline/coverage.json`
(SHA-256 `37f1f634d90a92a5e9446aa507a38318f494a7e18190ce9600eb8b68e89b3b8f`);
the new-batch report is
`.coverage-mcp/local-evidence/20261017/webp-image-chunk-incremental/coverage.json`
(SHA-256 `f23fdb42cb9f503ea3a35cb88b4c9938a297ccff35dabda99cee1108c88a056b`).
No full coverage run or full-suite regression conclusion is claimed. The current fixture matrix
contains 2,295 total rows: 1,801 decode / inspect / verify rows and 494 encode rows.
Of those, 1,801 decode rows and 462 encode rows are active; the other 32 encode
rows are planned.

## AVIF 12-bit I444 auxiliary-alpha parity — 2026-10-17

Added the public `high_bitdepth_still_12bit_444_alpha_9x9` decode row. The
deterministic 9x9 full-range profile-2 fixture contains 12-bit I444 color and a
monochrome 12-bit auxiliary alpha item. Pinned libavif 1.4.1/libaom 3.13.2
produce two all-lossless one-tile frames; Pillow 12.2.0 verifies and loads the
input as one RGBA8 frame. The 484-byte input SHA-256 is
`f7eb5b9edfdd7b11d00094aadb91320e7cc6cc01961470e3c2040d91e68610aa`; the
exact 324-byte Pillow reference SHA-256 is
`4a644e661e4cc9eb2ab3b4548d7f2275061b058e14a9370cffb63826f5c7a014`.

The selected `test_decode_matrix_avif` parity run passed. A selected
`cargo +nightly-2026-07-16 llvm-cov` report at
`target/release-evidence/coverage-avif-12bit-444-alpha-9x9.json` was queried
with Coverage MCP. Its report contains 1,829/31,302 observed branches overall;
the source and build are unverified and test attribution is unknown. The
report shows the new case reaches both the eight-lane converter batches and
the scalar tail. Coverage MCP also reports ten unobserved branch outcomes in
the separate 10-bit alpha converter and eight in the full-resolution RGB
shape checks. These are observations from the selected report, not claims
about the full suite. An incremental union with the saved 2026-10-16 full
baseline is unavailable: its detailed observations do not reconcile with its
reported totals, so Coverage MCP returns `incomparable`. No aggregate gain or
full-suite regression conclusion is claimed.

## JPEG empty DC Huffman table grayscale fast-path parity — 2026-10-17

Added the public `empty_dc_huffman_table_grayscale_fast` decode row. Its full
3,887-byte baseline grayscale JPEG replaces DC Huffman table 0 with a valid
zero-symbol table. Pinned Pillow 12.2.0 verifies and decodes it as one 128x128
`L` image. The complete input SHA-256 is
`7fda920713bb5982e3dc04a8f1dc695189e8b62a22f2118e72a22a5efd5f96b8`; the
exact 16,384-byte Pillow pixel reference SHA-256 is
`f1dad3cb56362cf934f41278fe27f561f388fe8284f70dda72f98035de1f4f2f`.

The selected `test_decode_matrix_jpeg` run passed exactly this row. A second
selected run under `cargo +nightly-2026-07-16 llvm-cov` passed the same row and
produced `target/release-evidence/coverage-jpeg-empty-dc-huffman-grayscale-fast-20261017.json`
(SHA-256 `c43650e94a75e035742262fbb95f59c1322d99445ff798fa6e29c39c7b9975f3`).
Coverage MCP reports 258/31,396 observed branches in this selected report and
shows the optimized `decode_slow_fast` path reaching the overlong-code and
empty-table recovery branches at lines 389–391. The saved 2026-10-16 full
report lists `arm=true` at line 390 as missing; the selected report's LLVM
branch data records the empty-values `arm=true` as taken. Both reports have
unverified source/build identity and unknown test attribution, so this is
coordinate-level reachability evidence only. The old report is stale and its
observations do not reconcile with its totals; no aggregate coverage gain is
claimed.

## Selected 10-bit I444 alpha coverage — 2026-10-17

The existing `high_bitdepth_still_10bit_444_alpha_lossy_16x16` Pillow parity
row also passed alone under `cargo +nightly-2026-07-16 llvm-cov`. Its report is
`target/release-evidence/coverage-avif-10bit-444-alpha-16x16.json` and records
2,426/31,302 observed branches. Coverage MCP's `selected_tests` comparison
with the selected 12-bit I444 alpha report found five branch arms newly
observed at coordinates in `convert_full_resolution_rgb_10bit_alpha`, with
five outcomes still unobserved there. The comparison is limited because both
reports have unverified source/build identities and unknown test attribution;
it describes report-coordinate differences, not a verified gain. The
incremental union was incomparable because the baseline's detailed
observations do not reconcile with its totals. No aggregate coverage claim is
made.

## Latest full all-feature coverage — 2026-10-16

One complete local `make coverage` run on source commit
`7fe6010a91b4b1732bf3f1f6fac99cdc418b4340` passed all 136 executed tests:
3 unit, 57 coverage-matrix, 7 decode-policy, 1 determinism, and 68
feature-gate tests. The run used:

```text
env RUSTC_WRAPPER= PYTHON=.oracle-venv/bin/python CARGO_BUILD_JOBS=2 make coverage COVERAGE_REPORT=target/release-evidence/coverage-main-full-20261016.json
```

The report is retained at
`.coverage-mcp/local-evidence/20261016/main-full-7fe6010a/coverage.json`
and was also written to
`target/release-evidence/coverage-main-full-20261016.json`. Its SHA-256 is
`8425ebb15b6b2c2e2f2022afc4bf3d813c30eda6325ab723fcab936ac39dbafd`.

| Metric | Covered / total | Percent |
| --- | ---: | ---: |
| Lines | 89,333 / 135,876 | 65.745974% |
| Branches | 17,470 / 31,282 | 55.846813% |
| Functions | 5,003 / 8,694 | 57.545434% |
| Regions | 136,514 / 210,887 | 64.733246% |

The run passes the owner-approved alpha floors of 59% lines, 46% branches,
52% functions, and 58% regions. It does not meet the separate 100% goal.
Coverage MCP can read the report, but its record has unverified source
identity, no registered build receipt, and unknown test attribution. Keep this
local exploration report separate from the historical managed baseline; no
verified regression or aggregate incremental-gain claim follows from comparing
them.

## Local artifact availability and exporter investigation — 2026-10-05

An owner-confirmed routine cleanup removed the local `target/` directories,
including the raw profiles, compiled objects and receipts cited below. Their
recorded digests remain historical identities; unavailable artifacts have not
been recreated or represented as surviving originals. Fresh measurements are
required for further comparisons. New essential receipts are being retained
outside build directories.

Before cleanup, a native exporter investigation found positive raw-profile
counts for 64 API functions reported as uncovered in the last full JSON.
Reordering the identical complete object set changed the exported function
coverage. The pre-cleanup AVIF table below preserves the original exporter output, but these
mapping losses prevent interpreting its changes as missing execution or a
test regression. The diagnosis retained after cleanup is a context
reconstruction, and a native repair must be validated with fresh artifacts.
The repair must prefer real mappings over unused mappings, including real
functions with zero executions, while retaining genuinely unused functions
and every reported source. No corrected result was admitted at that
investigation checkpoint; the fresh validation below has its own artifacts.

## Scoped JPEG grayscale and portable progressive parity — 2026-10-05

The maintained input generator adds 11 complete JPEG decode cases and 15
grayscale encode cases. Sizes 17x9, 128x128 and 512x512 cover omitted,
4:4:4, 4:2:2 and 4:2:0 sampling options, with optimized, progressive and
restart variants. Two complete inputs redefine DQT or DHT after the first
scan. Pinned Pillow 12.2.0 supplies all encoded-byte and decoded-pixel
references through the normal producer.

The encoder preserves explicit grayscale SOF sampling descriptors while
retaining one block per scan MCU. Non-1x1 singleton baseline scans use the
existing sequential reconstruction path. Unsnapshotted quantization tables
borrow the unchanged frame tables; missing captured tables remain errors.
An explicit Huffman snapshot flag distinguishes uncaptured tables from
captured absence, preserving first-scan provenance after later definitions.
Lookup `Option`s retain absence semantics and checked error conversion;
omitted encode options retain the grayscale default.

WASI exposed a preceding progressive decoder bug: non-DC-only scalar IDCT
received coefficients without dequantization. The correction multiplies in
the existing reorder loop on portable targets. Native ARM/x86 expressions
and the already dequantized DC-only path remain unchanged. The original
main attempt passed 212/227 WASI decode cases and 92/96 encode cases; both
failed receipts remain preserved. An isolated preceding-source reproduction
also fails the old progressive case against its exact Pillow reference.

Fresh main acceptance passes all 14 formatting, verification, strict native,
coverage-hook and WASI gates. Native and WASI each pass 227/227 JPEG decode
and 96/96 encode cases, with 454 public encode calls and zero panics. Existing
JPEG feature contracts pass 68 native and 66 WASI tests, with zero failures
or ignored tests. All 4,212 prior input/reference payloads, old ordinary row
objects and 26 fault rows remain exact. Fault contracts were not requested
in this normal scoped run. The main receipt SHA-256 is
`ab2a02836cd8ed6f280b74055be6d326e52dbb5ce784a73bf7c6646fcc84823f`;
independent acceptance review is
`c6b5c05ba60d992b58850fc0fd65fd87f36be552aeaa09767ca8bbbfd4c04aaa`.
Proofs are outside `target/`, under the recovery root recorded below.

Before the portable correction, the frozen grayscale candidate was measured
with 297 prior JPEG controls and then only 26 new ordinary cases, resetting
profiles between runs. Native counters record ten newly exercised branch
arms: five in JPEG and five in PNG deflate reached through the complete PNG
inputs. Coverage MCP's line comparison is limited; branch comparison is
incomparable because aggregate and normalized detail denominators differ.
No aggregate incremental union or full-suite regression is claimed. The
selected receipt SHA-256 is
`2b7abd4fc08c566ff3faf2eac61cd7af94cc34f1f31454706d2007281d924744`.
The full totals below belong to their earlier source. Paired throughput
timing remains pending.

## Managed full-coverage baseline — 2026-10-05 (historical)

The fresh full run on `ede1d7fa` passes `make coverage-with-exporter` and all
seven existing test binaries (3/4/1/57/7/1/68), with zero failures or ignored
tests. Its ordinary inventory is 2,138 rows: 1,675 active decode, 431 active
encode and 32 planned encode. All active rows pass. The 26 target-only fault
contracts pass separately with oracle status `not_applicable`. There are
1,647 public encode calls and zero encode panics.

| Metric | Covered / total | Percent |
| --- | ---: | ---: |
| Lines | 89,212 / 135,769 | 65.708667% |
| Branches | 17,401 / 31,246 | 55.690328% |
| Functions | 4,989 / 8,685 | 57.443869% |
| Regions | 136,469 / 210,864 | 64.718966% |

All alpha floors pass. The separate strict 100% verifier exits 1 because
every metric remains incomplete. This is a native all-feature test and
coverage run; the preceding scoped target checks retain their own evidence.
No matching x86 throughput measurement or codec speed claim is added.

Proof paths below are relative to
`/private/tmp/image-slash-star-evidence-recovery-a761-20261005/`.
The fresh JSON is
`maintained-exporter-canonical-main-ede1d7fa-v5/maintained-producer.json`,
SHA-256 `48ba281a9429f6610d526e791a44319d6ab3e493bd46506bd6d900bb5a581e49`.
The LCOV SHA-256 is
`0de32869f420a49de6b849a69442c088e6320293088027c3ae63064bfd156b79`.
Independent completion audit has SHA-256
`f73865355296d835be2da2650c7c634d8e81e41b0dac0ef90e115e2bd0ce7eb5`.
Its source, tool, log and report guards bind 5,388 source and fixture files,
113 compiled artifacts, seven fresh raw profiles and one merged profile.
All eight profile copies are preserved outside build directories. Empty
compiler wrappers, two build jobs, offline Cargo and disabled incremental
compilation are explicit. Objects and profiles remain fixed across exports.
This evidence prose is updated after the measured source freeze is released.

Coverage MCP reads byte-exact copies of the native reports and returns
remaining JPEG, TIFF and AVIF branch locations. Its provider source and
build identities are unregistered and test status is `unknown`; the local
audit supplies the separate execution binding. Previous full source differs,
so no incremental union or verified full-suite regression comparison is
claimed from those queries. The failed preceding full attempt remains
recorded below, without an accepted report.

## Earlier full all-feature coverage at 2,134 rows — 2026-10-05

The implementation at the recorded 2,134-row checkpoint was measured through
`make coverage-with-exporter`,
which delegates to the existing full `make coverage` command with the
independently audited LLVM 22.1.8 v3 exporter and pinned Rust `llvm-profdata`.
The JSON is `maintained-exporter-canonical-main-0460-v3/maintained-producer.json`,
SHA-256
`24a01a758cbdb7dbf3a1c3d0444a18352cf9a5cc001733ec8912d0435563c7c5`.
All fresh proof paths in this section are relative to
`/private/tmp/image-slash-star-evidence-recovery-a761-20261005/` and are outside
the removable build-target directories. Historical cleaned artifacts remain
unrecovered.

| Metric | Covered / total | Percent |
| --- | ---: | ---: |
| Lines | 89,211 / 135,769 | 65.707930% |
| Branches | 17,399 / 31,246 | 55.683928% |
| Functions | 4,989 / 8,685 | 57.443869% |
| Regions | 136,465 / 210,864 | 64.717069% |

All alpha floors pass. The separate strict verifier exits 1 because every
100% requirement remains unmet. The full run passes all seven existing test
binaries (3/4/1/57/7/1/68), with zero failures or ignored tests. That run
measured an ordinary inventory of 2,134 rows: 1,671 active decode, 431 active encode
and 32 planned encode. TIFF has 264 decode rows; AVIF remains at 525 and JPEG
at 216. All 26 target-only fault contracts retain separate results and oracle
status `not_applicable`. The malformed ledger contains 592 entries and the
static origin registry has 141 guards across 20 files.

The preceding same-code campaign explicitly binds 5,297 source, fixture, script, configuration,
benchmark and documentation files, 113 compiled artifacts and seven fresh raw
profiles. An empty `RUSTC_WRAPPER`, two build jobs and parent
`CARGO_INCREMENTAL=0` are recorded. Native reports use exactly the same
objects and profiles; source and artifact hashes stay fixed through all
exports. The full receipt has SHA-256
`943194502182041a23802e5bcddb6a4398ebb47c3753d9d3d0dc5627b195580d`.
This evidence prose was updated after measurement. Its source and fixture
identity remains bound to that recorded inventory; later additions have
separate evidence.

All eight distinct formatting/strict-Clippy gates pass, including debug,
release, coverage hooks, JPEG benchmark, SSE2 and AVX2 cross-compilation.
All-feature check, strict rustdoc, doctests and existing tests pass. All 33
feature lanes pass: 11 native runtime, 11 WASI runtime and 11 browser
compile/strict-Clippy/rustdoc lanes. The quality receipt has SHA-256
`91fb461c28aa400ea37503763afa15706d772f0819c66176fb85981d6f29d62b`.
The verbose matrix log supplies the lane records; its nonverbose summary list
is empty. Cross-compilation supplies no matching x86 throughput measurement.
This slice adds no private unit tests, unsafe code, broad lint exceptions,
source exclusions or ignored cases.

### TIFF ExtraSamples and grayscale-alpha parity

The maintained input-only generator covers four families (RGBX and grayscale
unspecified extras, associated LA and straight LA), contiguous/separate
storage, strips/tiles, and raw/Deflate/LZW/PackBits compression: 64 complete
17x9 inputs with padded tile edges and alpha boundaries. Fresh pinned Pillow
12.2.0 owns all expected outcomes: 34 successful pixel references, 28 open
failures and two successful opens followed by materialization failures. The
30 valid but unsupported layouts are classified `ambiguous` in the malformed
ledger. They are ordinary oracle cases, separately from fault contracts.

A fresh isolated public comparison observes 56/64 mismatches in the preceding
implementation and zero in this candidate; eight controls match both. All
264 TIFF cases pass. Every prior ordinary row, 25 prior fault contracts,
200 TIFF input/reference records and old encoded-input/raw-reference byte
remains exact. Historical managed measurements and identities are preserved.
The combined preparation receipt has SHA-256
`4cc13292522e82dcf0fc0f7a59f692baabd4a9ccd649db240b95b048545eda49`.

Inspection and decode share the ExtraSamples layout decision and raw ignored
band-count validation. Compressed ignored planes are omitted from payload
decoding. Retained samples are compacted in the existing checked assembly
buffer with its allocation and capacity preserved; cancellation remains
bounded. Optional alpha, color-map and extra-sample metadata retain genuine
absence semantics. The existing private fast-path `Option` categories retain
their checked fallbacks. New fallible conversion/compaction helpers return
`Result`. No timing result is claimed for these changes.

### AVIF SGR source reservation contract

`avif_sgr_source_reservation_failure` reuses the complete ordinary
`animated_lossy_inter_420_superres_sgr_8bit_160x56` input and introduces the
typed `av1.restoration.sgr_source_reservation` fault point. Under coverage it
requests `usize::MAX` through the actual `Vec<u16>::try_reserve` operation;
normal builds use the original sample count. The public contract checks the
structured `SequenceDecode` dimensions error and exact message, one-shot
fault reset, and two fresh successful retries with complete sequence equality.
The hook is absent from normal builds. This is target-only fault evidence;
the adjacent ordinary source keeps its independent Pillow comparison.

On fixed source and instrumentation, the selected control run passes 201
ordinary cases (200 prior TIFF cases plus the AVIF source) and 25 prior
faults. The next run passes only the new 64 TIFF cases and one fault. Profiles
are reset between runs; all actual objects and all four native denominators
stay fixed. The selected receipt has SHA-256
`3aea95a9a9e4749b0ec8e1782be610b0b7eeaad54212dd4146fdcbaa2bba39ae`.

Coverage MCP's unmodified LCOV comparison observes 56 new line coordinates,
55 in TIFF and one at the coverage-only SGR reservation hook, for the selected
union 34,518 -> 34,574 / 131,268. Its status is `limited`: provider source,
build and test contexts remain unbound, although the independent local receipt
records fixed hashes and successful executions. The response SHA-256 is
`a094bb469b4a603957f6977ce0af8d312507214b953bccbebe0a321560e5413f`.
Both native JSON incremental queries are `incomparable` because summary and
normalized detail counts differ. Reports and counters were preserved. These
selected coordinates do not establish full-suite regression status or the
full campaign's coverage delta.


### Native exporter provenance and reporting boundaries

The v3 native patch is pinned to Rust LLVM commit
`52ed14fcd56afc30f9cccd8ca8ce237c2eef7e04`; patch SHA-256 is
`c79225c56592e5305169c006f5f17d432e80a11f0e9463eff04b387434061b91`.
The preserved exporter SHA-256 is
`80b04f713478755ad311865a82ed2a3c49c7441f9398cfea37c06326d684e768`.
No installed toolchain files are replaced.

A fresh control on the preceding `056c502b` source reproduces the original
unpatched coverage totals. On identical objects and profiles the native repair
restores 64 real functions whose exact mangled-name/CFG-hash profile tuples
have positive counts. It retains all 110 source files and 397 unused-only
function entries at zero, including 333 entries in measured source files.
A separately labeled genuine capability-test profile proves that two real
unexecuted mappings retain their branch records at count zero. Unused
payloads contain the same coordinates with structural Zero counters, so LLVM
folds and omits their exported branches; real CounterRef/expression payloads
retain them. The full branch denominator increases by 14 for the seven
`SourceColor::is_empty` branch regions. This producer correction is separate
from execution gains caused by new cases.

The preceding native audit receipt has SHA-256
`0b48887f8613126abf4d7d9d5054af7e0ab3afad3ca5b606ac2631c5fabdbb71`;
its payload-wording correction has SHA-256
`464f449829a1829a65b04f18c9095f6c2e35e15b24ed75ae660ee88377b21957`.
The original strict Cargo/native JSON identity failure is preserved: Cargo
adds metadata and slightly reserializes percentage floats. Native integer
counters and literal LCOV fields agree; no original report is corrected.
Natural unmatched-unused mappings with a positive foreign-hash profile record
were not demonstrated. That edge retains source-policy evidence only.
Admission is bounded to the pinned native macOS ARM64 producer and recorded
artifacts.

An independent audit of the current combined campaign passes 34 direct native
JSON/LCOV exports across 17 distinct complete nine-object orders. Every source,
function, region, branch, instantiation and count field is retained and order
invariant. Frozen source, full and incremental objects, profiles, tools and
original reports/logs remain byte exact. The current audit receipt has SHA-256
`87db2f9469abc3be885755ac7a7b9c2b869355da889a58a3534211f5ac95253d`.
Its 34 Cargo/native percentage-only scalar differences remain separately
recorded. The first audit preparation fails on an incorrect assumption that
the new sorted fault row is last; the corrected exact-ID guard succeeds.
That failed preparation is preserved separately.

### Maintained producer setup and canonical command

The opt-in `coverage-exporter-setup` and `coverage-with-exporter` targets now
maintain the exact native patch and toolchain descriptor in the repository.
The original coverage/check/complete recipe bytes remain unchanged. A narrow
Git whitespace attribute preserves the serialized patch's blank context
markers; its reviewed patch hash remains exact. The optional launcher preserves
spaces and apostrophes in report names and rejects characters the inner recipes
cannot represent literally. Final controlled checks pass 90/90; an independent
outer-to-inner check passes 40/40, including GNU Make 3.81's `-i` behavior.
Ignored shell errors can mask the Python rejection's outer exit status, but no
coverage/context work runs for those rejected values. Dollar and line-feed
guards fail during Make expansion. Earlier failed checks remain preserved.

A genuinely fresh maintained v3 setup uses the exact prepared LLVM source,
two build jobs and an isolated prefix outside `target/`. Configure and all
759 build steps pass, followed by actual no-op and two concurrent receipt
reuses. Its executable SHA-256 is
`045227d446bfb7241b8e6febd2707ce82c6aee9aedb8cb725cb4d828bcec7004`;
its identity receipt is
`49ff1dd42ff0afbae1745cb002c9c55c3ee256357c0c7cf970e66887c8fedb42`.
All 34 native JSON/LCOV exports across 17 complete object orders match the
previously audited producer's fields. The actual setup/export verdict has
SHA-256
`db4e20acd783ca834861b25742bcb6c0fc452fc0baf75a041bc32f9128bdff9d`.
The first-source-fetch branch remains unexercised. Native admission and the
foreign-hash limitation described above retain their recorded scope.

The integrated canonical command passes `make verify`, all seven existing
test binaries, 1,671 active decode rows, 431 active encode rows and all 26
target-only fault contracts. Its 1,647 counted public encode calls have zero
panics. The 32 planned encoder rows are reported separately from the active
denominator. All alpha floors pass; the separate strict verifier exits 1.
The integer totals above reproduce the preceding audited full campaign exactly.

The measurement runner's bookkeeping assertion incorrectly included planned
skips in that active denominator and failed after the real commands exited 0.
Its original failed receipt remains unchanged, SHA-256
`01a969d5e64f4f0a54693aa95fe6e6f6fb68600718adc0346d901fb23e30a8d3`.
An independent completion supplement checks the actual runner source and
literal results, then performs only a normal LCOV report export and strict
verification. It reruns no tests and rewrites no counters or original receipt.
Its SHA-256 is
`54c7affcf9587c894a3580fd3bf4d38875a6bf572846e9f25b323a556f5977f0`.
The LCOV SHA-256 is
`363c6078af91b6907c56a1843fa5f1c6eba4f215540bfdfc420a61830d1426ef`.

The canonical run freezes 5,301 source/configuration files, 113 compiled
artifacts plus the root executable used by the nine-object export, seven raw
profiles and one merged profile. Its completion audit archives and checks
these artifacts, original reports/logs, producer identities and all 110 measured
sources before and after report-only work. The export's raw-profile argument
list is captured after the export, explicitly as a later capture. Human evidence
and command-reference prose are updated after measurement; codec, test,
fixture and producer recipe contents remain unchanged. No provider receipt is
invented, and this reporting repair is separate from a new-case coverage gain.

## TIFF ignored-sample row boundary parity — 2026-10-05

Three complete raw RGB TIFFs retain an unspecified fourth sample in rows of
255, 256 and 257 pixels. Their stored row sizes are 1,020, 1,024 and 1,028
bytes. The normal input-only generator and pinned Pillow 12.2.0 reference
producer preserve all previous 64 generator outputs, 2,134 ordinary rows and
26 separately typed fault contracts. Exact RGB observations contain 765, 768
and 771 bytes. The codec implementation is unchanged. The existing public
cache check uses four cloned handles sequentially on the maintained WASI
runner, which lacks thread spawning; native checks retain four worker threads
and the same complete image, pointer-identity and cache-state assertions.

The fixed-source incremental run, preceding that harness portability update,
selects all 264 previous public TIFF rows, then only the three new rows.
Both selections pass without failures or
ignored tests, with zero fault contracts selected or executed. Both native
reports retain identical denominators: 135,754 lines, 31,228 branches, 8,682
functions and 210,844 regions. The previously missing true arm of
`compact_extra_samples` at `tiff/decode.rs:1517` changes from 0 to 1 in the
native file and function records; the new row crosses the 1,024-byte token
checkpoint within a row. This is no cancellation-response or throughput claim.

Fresh evidence resides in
`tiff-extra-sample-boundary-cases-8abb-20261005/` below the recovery directory
named above. The original incremental receipt has SHA-256
`271b2e8da2b981fa6456594e77b9089326ff4be2a5af933b42a9467e7701bb92`;
independent completion has SHA-256
`1110d55f680fe9d0b182f7213a7f33b94c126a86ad0dba9cd338d14f825f576d`.
Preserved raw profiles re-merge byte-identically. Direct native exports match
all counters, coordinates, names and shapes; distinct outputs retain only
explicitly reported percentage display differences within two ULPs and one
terminal LF in LCOV. Earlier strict reviewer attempts remain preserved.

Coverage MCP returns the missing branch for the controls and none at that
location for the new-only report. Its LCOV line union records zero new lines.
Its branch union is incomparable because reported totals and detail differ;
provider source/build receipts remain unregistered. No report fields were
rewritten. These selected results do not replace the last full all-feature
measurement above or establish a full-suite regression check. The inventory
at this checkpoint is 2,137 ordinary rows, including 267 TIFF decode rows; the 26
fault contracts remain outside Pillow parity totals.

After the harness update, all ten scoped commands pass: formatting, fixture
verification, generator byte checks, strict native and WASI all-target/
all-feature Clippy, coverage-configured strict Clippy, native TIFF decode and
encode, the normal WASI matrix build, and its three selected ordinary rows.
Native parity passes 267/267 decode and 59/59 encode, with 205 public encode
calls and zero panics. WASI passes 3/3 selected rows. No test is ignored.
The completion receipt is
`main-scoped-acceptance-portable-v3-completion/completion.json`, SHA-256
`7ff0067677967b6f9f1141336412b050f411d3d183c03eeca62f50bfa88c4a55`.
Its final guards bind 5,374 source and fixture files and the rebuilt native
and WASI binaries. The original failed bookkeeping receipt is preserved:
it incorrectly expected a coverage-only fault summary from a normal binary,
whose fault dispatcher is empty. No fault selector was requested or counted
as Pillow parity. Four earlier WASI malformed-range classifications remain
outside this selected scope; this is no full WASI or 33-lane campaign claim.

## TIFF payload-range classification on 32-bit WASI — 2026-10-05

Four checked strip/tile offset additions now classify an unrepresentable
payload range as `malformed`, using the adjacent out-of-bounds payload
message. This matches the existing complete Pillow error fixtures for
`planar_separate_rgb_tiled_oob_tile`, `planar_separate_rgb_oob_strip`,
`tiled_palette4_oob_tile` and `error_bad_ifd_oob_strip`. Geometry errors and
checked arithmetic remain unchanged. The correction has no dependency on
the separate large-row diagnostic, which has not executed. Native 64-bit
behavior remains unchanged; a representable incremental retry minimum on
64-bit can still differ from a terminal unrepresentable range on 32-bit.

All fourteen scoped commands pass on the candidate following `f04ba130`.
Native and WASI each pass the full TIFF public matrix: 267/267 decode and
59/59 encode, with 205 public encode calls and zero panics per target.
The existing TIFF-only feature tests pass 67 native and 65 WASI tests, with
zero failures or ignored tests. Formatting, fixture verification, native and
WASI all-target/all-feature strict Clippy, coverage-configured strict Clippy,
and WASI TIFF-only strict Clippy also pass. All prior fixture bytes and
observations are unchanged; no fault selector is requested or counted.

The receipt is `tiff-range-only-f04ba130-scoped-acceptance-v1/receipt.json`
below the recovery directory above, SHA-256
`392f5ba40a496f99b47fe00caefcc8488f49e7726fcde11fc772649024eade5d`.
Final guards bind 5,374 source files, tools, rebuilt test binaries and the
two unrelated untracked files. This resolves the four classification failures
left outside the preceding three-row WASI selection. It is no complete
33-lane campaign or fresh full coverage result.

## Animated AVIF primary-item rotation parity — 2026-10-05

`animated_primary_item_irot` adds one essential primary-item rotation property
to the existing complete five-frame AVIF. The input-only generator grows the
file from 1,235 to 1,245 bytes, rebases its item and track offsets, and retains
every coded sample byte. Pinned Pillow 12.2.0 observes the same five 150x150
RGB frames and 33-ms durations. Two fresh observations through the maintained
native observer record repetition count zero. That observer uses the exact
surviving ABI header and pinned Pillow shared library; no full clean native
source checkout is claimed.

The isolated ordinary public comparison passes 1/1. Its fixed-source
incremental run passes one existing animated control, resets raw profiles,
then passes only the new row. No fault contract is selected or executed.
All 63 compiled objects and all four native denominators remain identical:
135,754 lines, 31,228 branches, 8,682 functions and 210,844 regions. The true
arm at `avif/decode.rs:349` changes from 0 to 1 in file and function records;
literal LCOV records agree. Coverage MCP returns the missing true arm for the
control and the missing false arm for the new-only run. Its incremental union
is incomparable because normalized detail and aggregate inventories differ.
Reports retain all fields; no MCP union or full-suite gain is claimed.

The candidate preserves all 2,137 previous ordinary observations, 26 separate
fault objects, old input/raw bytes and loop facts. Appending the fresh native
loop observation changes the whole-index digest, so 132 old-row index-SHA
fields are explicitly refreshed; those row objects are not byte-identical.
The final handoff is
`animated-avif-primary-irot-f04ba130-20261005/final-handoff-v1.json`, SHA-256
`2509e87af0b47c2ade917d0edff80bcd473039598120e410a1e56310c57c6973`,
under the recovery directory above. The original selected-run bookkeeping
failure is preserved and qualified by its actual command exit zero and
literal 1/1 result. This update changes no Rust codec or test harness. The
current inventory contains 2,138 ordinary rows, including 526 AVIF decode
rows; fault contracts remain outside Pillow parity totals.

Normal main acceptance passes formatting, the pinned input-generator check,
526/526 native AVIF rows and the one selected WASI row, with zero failures or
ignored tests. Existing executables are reused for actual current inputs;
there is no recompile claim. The outcome receipt is
`animated-avif-primary-irot-f04ba130-20261005/main-normal-public-outcomes-v1.json`,
SHA-256 `70594fd65d6370650667f3a7eea64fd85ce62240e81e91ce77bee2b9b5697e28`.
The initial fixture verification fails on two stale current roadmap AVIF
counts; its original receipt and log remain preserved. Those derived fields
are corrected to 526, with historical coverage records unchanged, and
verification is rerun separately. The administrative correction changes no
codec, test, fixture or generator byte measured by those runtime comparisons.

### Full-campaign AVIF witness registration correction

The fresh full attempt on `7edbc034` passes all 1,675 ordinary decode rows,
431 active encode rows and 26 separate fault contracts, with 1,647 public
encode calls and zero encode panics. It fails the existing complete native
repetition witness test on its stale 285-artifact assertion: the current
index contains 291 artifacts. This failed attempt produces no accepted
coverage report. Its seven raw profiles, original receipt and log remain
preserved in `maintained-exporter-canonical-main-7edbc034-v4/`; the failure
audit has SHA-256
`0ed60c841df220fd55a7736cfcfc58ec6e60674d63d484e8f8388b9fea69ed00`.

The public witness harness now requires exactly 291 artifacts, appends the
new case to its exact name list and resolves its source through the existing
matrix sequence observer. All previous 91 case objects and 285 artifact
bytes remain unchanged. Complete-input hashes, native repetition facts,
every frame's pixels and timing, and public error checks remain enforced.
The focused existing test passes 1/1 with zero failures or ignored tests,
covering all 92 complete cases. Fresh pinned Pillow observes the new five
frames and two native observations match the retained index. Formatting,
strict all-feature public-test Clippy and fixture verification also pass.

The terminal review is
`animated-avif-primary-irot-f04ba130-20261005/witness-registration-fix-7edbc034-v1/completion-review.json`,
SHA-256
`9a57980be1c9ee0445fda3e13ca7258f011f3e3ecdc52e402f7b5da5554aae73`.
Its guards bind 5,388 source and fixture files, tools, logs, the actually
recompiled normal test binary and the two unrelated untracked files. Only
the three harness registrations change. This focused correction does not
replace the last full coverage measurement above.

## Pre-cleanup AVIF full coverage — 2026-10-05

The last recorded full local report was
`target/release-evidence/coverage-avif-derived-skip-refs-v2-20261005.json`, SHA-256
`32b12a55dddb5a2488f8c09bfb67a02feb42dab40fa0ac21697c0b151983e0e3`.
`make coverage` uses `nightly-2026-07-16`, an empty `RUSTC_WRAPPER`, two build
jobs, `CARGO_INCREMENTAL=0` and a fresh
`target/llvm-cov-avif-derived-skip-refs-v2-full-20261005` target.

| Metric | Covered / total | Percent |
| --- | ---: | ---: |
| Lines | 88,832 / 135,656 | 65.483281% |
| Branches | 17,359 / 31,196 | 55.644954% |
| Functions | 4,921 / 8,682 | 56.680488% |
| Regions | 135,986 / 210,661 | 64.552053% |

All alpha floors pass. The separate strict verifier exits 1: all four 100%
requirements remain unmet. All seven test binaries pass 3, 4, 1, 57, 7, 1 and
68 tests, with zero failures or ignored tests. The measured AVIF checkpoint ordinary inventory
contains 2,070 rows: 1,607 active decode, 431 active encode and 32 planned
encode. AVIF has 525 decode rows, JPEG 216 and TIFF 200. All 25 target-only
fault contracts retain oracle status `not_applicable` and separate results.
The malformed-input ledger remains at 562 entries.

The incremental and full campaigns preserve all 5,190 measured
source/configuration/script/fixture hashes: 5,189 repository files and one
ignored ambient Finder file. The repository-local count includes the ignored
`.cargo/config.toml`; 5,188 measured files are tracked in Git. The local
configuration sets `CARGO_INCREMENTAL=0` for child commands and selects a
compiler wrapper. The campaign explicitly sets the parent Cargo environment
to `CARGO_INCREMENTAL=0` and overrides the wrapper with an empty
`RUSTC_WRAPPER`. Both ignored files retain
separately verified hashes. The full campaign has its own 113 instrumented
artifacts and seven fresh profiles, unchanged across JSON and LCOV exports.
The selected campaign's 63 artifacts and final profiles remain unchanged.
Different test scopes use separately bound objects; no cross-scope object
identity is claimed. The original full source receipt retains precommit base
revision `61f4dfdc145f2a35526588667368d5873df07280` and SHA-256
`dc6f39d09a0a1424857fd8c72ebb435bd084f6870cd47a752d8cdbf9549cf582`.
Human evidence documentation is outside that measured inventory.

All eight formatting/strict-Clippy gates pass, including coverage-nightly,
JPEG benchmark and SSE2/AVX2 cross-compilation. Their v2 receipt has SHA-256
`7ecdc21d94aa08a4c3f9ba76c947fdc2c3eed1e1607d9fbf2823c94aeafb892e`.
All 33 feature lanes pass: 11 native runtime, 11 WASI runtime and 11 browser
compile/strict-Clippy/rustdoc lanes. The v2 feature receipt has SHA-256
`5b5451ff599288230aeaae13b032bde06b0f5d7d6fea48065417d98d695da871`.
These cross-target checks provide compilation evidence, without matching x86
runtime or throughput measurements for this change. The origin registry
retains 140 guards across 20 files. This change adds no private unit tests,
unsafe code, lint exceptions, coverage exclusions or ignored cases.

### Complete AVIF disabled-skip reference parity — 2026-10-05

Three complete ordinary lossless animations exercise I420, I422 and I444
sampling with five 64×64 frames and fixed 32×32 blocks. At the observed inter
context, the header retains a derived skip-reference pair while skip mode is
disabled. The reconstruction gate now permits that unused pair, retaining
the existing exclusion of enabled skip mode and all reference, quantizer,
geometry and safety checks. The change removes one context predicate and
adds no pixel allocation, raster copy or block-loop operation. It has no
measured speed claim. Execution evidence covers bounded reconstruction for
these three complete inputs.

| Ordinary input | Bytes | Complete input SHA-256 |
| --- | ---: | --- |
| `animated_lossless_inter_420_derived_skip_refs_b32x32_64x64` | 6,130 | `50d8bdd33c5a5e958d5991d25767748088ba3354d35d4b809d52e84a45713c15` |
| `animated_lossless_inter_i422_derived_skip_refs_b32x32_64x64` | 7,530 | `0eae260be09a74b98b1f525261a237ef3783105870de44224de91dd65ee5d168` |
| `animated_lossless_inter_i444_derived_skip_refs_b32x32_64x64` | 10,135 | `3769c145bb4d97f357a8b3c4660fb8a2f976c682643304e64833d06515901009` |

The maintained `scripts/generate_avif_derived_skip_reference_fixtures.py`
runs through the normal AVIF asset hook. Recipes retain stimulus and complete
input hashes; fresh pinned Pillow 12.2.0 owns expected pixel and lifecycle
observations. Two separate output-directory generations and `--check`
reproduce all three input files. The generator receipt has SHA-256
`a618607002514ab86a69a686d24ecb6030c5c952678296526341600f9fb8d305`.
The normal native loop producer independently observes all five displayed
frames, exact pixels, 100 ms timing and loop zero. Its index grows from 88
to 91 cases and 267 to 285 artifacts. All previous cases and artifact bytes
remain exact. The new index SHA-256 is
`38e8bf690190b189c26fb93876ef7c7b47b67ca57fa4c99c49b819b0ef58f13d`.

Canonical regeneration preserves all 522 prior AVIF ordinary outcomes and
every other format/encode block. Only the native index digest changes in the
63 existing AVIF sequence provenance fields in each of the matrix and output
JSON. All 25 fault contracts and historical managed-ledger measurements,
run identities and measured-input hashes remain exact. An isolated public
probe also matches seven complete ordinary inputs, 29 sequence frames and
seven first images against fresh Pillow observations; its receipt has
SHA-256 `9774993f56582350a1c7ce44e02295a14087d47f97d9cf069477223d1ee56a23`.
Independent source review finds no required repairs and binds the retained
skip-mode exclusion, all 18 new first-image/sequence references, old outcomes
and native artifacts. Its receipt has SHA-256
`c8e17b450b2ba14ec348b861c1ac9ad5461babb3605475f18102637e437ee504`.

The first full attempt fails on the existing native observer's stale
267-artifact assertion. The repair updates that observer to 285 artifacts,
appends the three case registrations, shifts five origin line coordinates by
one and refreshes the ledger's current origin digest. A subsequent manual
formatting attempt also fails `rustfmt`; its unsuccessful evidence is kept.
The authoritative `cargo fmt` output is used by every successful v2 campaign.
The original incremental/feature successes, failed full attempt and formatting
failures remain separate from the passing v2 receipts.

#### Incremental measurements and coverage limits

On the fixed v2 source, the baseline executes 522 prior ordinary AVIF rows
and 24 applicable AVIF fault contracts, reported separately. Three subsequent
runs execute one new ordinary input each through the existing matrix runner.
All four runs pass, resetting raw profiles between runs and preserving all
63 actual instrumented objects. The incremental receipt has SHA-256
`301313e5e2c007b55b3fdb88420d94e4abafd83e19a40649a73c3f8f3efc1609`.
The read-only producer audit binds original report hashes, 110 measured source
files, all 5,190 frozen files, actual objects and passing selected-row logs.
Its SHA-256 is
`d67650d5986a300b9038b0a18c77b0129a8bec462009fc5c37fd65e444b00104`.

Coverage MCP verifies the LCOV line-coordinate union
**43,973 → 44,032 / 131,155**, with marginal gains of 58, one and zero.
The 58 coordinates are in existing code: 40 forwarding-wrapper lines in
`block.rs` and 18 compound-reconstruction lines in `entropy.rs`. The I422-only
gain is a short-circuit right-hand evaluation in
`bounded_reference_mode_supported`; it does not show enabled skip mode.
The zero-gain I444 case retains independent parity value. Native LF/LH totals
and the full report's aggregate metrics retain their separate denominators.
JSON region and branch comparisons are `incomparable`: region summary/detail
covered counts disagree, and native branch detail loses four identities.
No JSON producer context, replacement counters or synthetic identities are
introduced. The verbatim query receipt has SHA-256
`0dee6bf2dc3a8842f191176cc4ddfed258f9018aae122e132168b9ba47263f53`.

All four native full covered counts are lower than the preceding TIFF report.
Source, fixture inventory and build identity differ, so these reports do not
provide an exact full-suite regression comparison. The 59 selected line gains
are not a claim that 59 preceding full-suite gaps disappeared. The current
full report retains 46,824 missing lines, 13,837 missing branch outcomes,
3,761 missing functions and 74,675 missing regions. Proofs are under
`target/release-evidence/avif-derived-skip-v2-61f4dfdc-20261005/`, with original
reports and receipts beside it; canonical producer evidence is under
`target/release-evidence/avif-derived-skip-canonical-61f4dfdc-20261005/`.

## Preceding TIFF associated-alpha full campaign — 2026-10-05

The preceding TIFF full local report is
`target/release-evidence/coverage-tiff-associated-alpha-20261005.json`, SHA-256
`97b19bdff5d96774f0b4499a7177bbc25d0746eb51477cda829f201d44056e87`.
`make coverage` uses `nightly-2026-07-16`, an empty `RUSTC_WRAPPER`, two build
jobs and the dedicated `target/llvm-cov-tiff-associated-alpha-20261005` target.

| Metric | Covered / total | Percent |
| --- | ---: | ---: |
| Lines | 89,044 / 135,657 | 65.639075% |
| Branches | 17,363 / 31,216 | 55.622117% |
| Functions | 4,984 / 8,682 | 57.406128% |
| Regions | 136,223 / 210,663 | 64.663942% |

All alpha floors pass. The separate 100% verifier exits 1; all four completion
requirements remain unmet. All seven test binaries pass 3, 4, 1, 57, 7, 1 and
68 tests, with zero failures or ignored tests. The ordinary inventory has
2,067 rows: 1,604 active decode, 431 active encode and 32 planned encode.
TIFF has 200 decode rows, JPEG 216 and AVIF 522. The 25 target-only fault
contracts remain separately reported with oracle status `not_applicable`.

All 5,094 source/configuration/script/fixture hashes remain exact across the
fixed-source incremental and full campaigns: 5,093 repository files and one
ignored ambient Finder file. The original full source receipt preserves its
precommit base revision `3489d205f3acd4698d4c336eadfd1e756e40ff7a`; receipt
SHA-256 is `009eff76925febd98014667645a7fae0233f87eaa02020fde320d48d56999d32`.
Human evidence documentation is outside that inventory. Native full aggregate
totals and selected-report coordinate totals remain distinct.

All eight formatting/strict-Clippy gates pass, including coverage-nightly,
JPEG benchmark and SSE2/AVX2 cross-compilation. All 33 feature lanes pass with
all 5,004 captured file hashes unchanged: 11 native runtime, 11 WASI runtime
and 11 browser-target compile/Clippy/rustdoc lanes. The feature receipt has
SHA-256 `b8773060f79d62247a44b732187028308116bbee26c47677a238778d41c910ba`.
The origin inventory remains 140 guards across 20 files. Private unit tests,
unsafe code, coverage exclusions and ignored cases are absent from this change.

### Complete TIFF associated-alpha parity — 2026-10-05

Twenty-two complete ordinary inputs cover contiguous/separate sample planes,
multiple strips, clipped edge tiles, raw/Deflate/LZW/PackBits compression and
per-plane horizontal prediction. The maintained
`scripts/generate_tiff_associated_alpha_fixtures.py` is called by the normal
TIFF asset hook. It includes nonzero hidden RGB at alpha zero, stored colors
above alpha, alpha 1/2/17/254, opaque samples and payloads crossing cancellation
checkpoints. Two output-directory generations and `--check` reproduce every
input byte. All 180 previously tracked TIFF inputs and 178 prior decode rows,
59 encode rows, encoded reference JSON, other-format rows and 25 faults remain
exact. Inputs contain storage samples and selectors; the live oracle owns all
normalized expected pixels and failures.
The final 22-input regeneration receipt has SHA-256
`183af3b4daf63b2158bd59f8ea44dce8bc526c94a8b388c0ae5e39267adb6407`.

Pinned Pillow 12.2.0 independently establishes inspection/verification success
for all 22 inputs, 18 exact pixel successes and four materialization failures.
The raw separate importer accepts three declared RGB planes with the alpha
plane omitted, retaining colors with zero alpha. Declaring the first alpha
block instead fails with `ValueError: unknown raw mode for given image mode`.
These boundary inputs retain every payload and byte-count byte; only the
offset count changes. They are ordinary parity cases. Their malformed-ledger
specification status is `ambiguous`, because the complete TIFF storage is valid
and the reference's raw band importer lacks that decoder. The ledger preserves
558 prior classes and adds four, for 562.

Associated RGBA normalization now runs in place: transparent pixels clear,
opaque pixels retain their channels, and intermediate alpha uses truncated
`channel * 255 / alpha`, clipped to 255. The source descriptor still records
`SourceAlpha::Premultiplied`. Raw separate planes without an alpha block retain
their reference-defined transfer samples. A shared pixel helper and 1,024-byte
cancellation chunks add no pixel allocation or raster copy. One local Clippy
expectation documents the exact arithmetic invariant: byte multiplication is
at most 65,025 and the division arm's alpha is 1..=254. It excludes no source
from coverage. This correction has no throughput claim.

The preserved old binary mismatches all 18 initial inputs: 16 wrong normalized
rasters and two incorrectly successful raw planar decodes. Two further first
alpha-block inputs also expose the error; two omitted-plane controls already
match and preserve that leniency. The fresh public probe matches all 22 final
cases, with all inspection/verification boundaries retained. The canonical
matrix supplies wrapper, cancellation, metadata, sequence and structured-error
checks. The focused green receipt has SHA-256
`cdaa78609447990e5f58d2a92592c96257668cdcdd6adb3b3cd35360ee510e7b`.
The primary behavior is Pillow's `Unpack.c` RGBa unpacker and `TiffDecode.c`
separate-plane completion, pinned to tag 12.2.0 and preserved in ignored proof.
The tag resolves to commit `3c41c095064200a02672d89cc5ff629eaf4b0d4f`;
the saved commit-addressed source text is provenance and was neither compiled
nor executed.

#### Incremental measurements and producer receipts

Changed source is measured afresh: 178 prior ordinary TIFF rows, then batches
of eight contiguous, eight compressed separate and six raw boundary rows.
Every run passes with all 387 instrumented artifact hashes fixed. Raw profiles
are reset independently and fresh nonempty profiles are required. Initial
Cargo execution including compilation takes 124.120 seconds; later batches
take 0.843–1.003 seconds under concurrent checks. These are workflow timings.

Audited optional producer sidecars bind original report hashes, all 110 measured
source files, the frozen build identity and passing canonical row logs.
Coverage MCP verifies region union **6,116 → 6,153 / 210,643**: gains are 28,
one and eight. The separately audited LCOV line union is verified
**3,967 → 3,999 / 131,156**. These selected denominators differ from full
aggregate totals and do not establish a full-suite regression comparison.
JSON line and branch unions return `incomparable`; branch detail contains
31,210 coordinate arms against 31,216 native arms. Those responses are kept
without replacing native totals or inventing missing branch identities.

The full run still has 46,613 missing lines, 13,853 missing branch outcomes,
3,698 missing functions and 74,440 missing regions. In particular, selected
gains include newly implemented conversion behavior and are not a claim that
37 historical full-suite gaps disappeared. Historical managed ledger metrics,
run/snapshot IDs and measured-input hashes remain unchanged. Evidence is under
`target/release-evidence/tiff-planar-alpha-3489d205-20261005/`, with reports and
feature receipts beside it. Wider ExtraSamples-zero and LA layout differences
from the exploratory 128-input family remain unimplemented ordinary gaps.

## Preceding JPEG scan-declaration full campaign — 2026-10-05

The preceding JPEG full local report is
`target/release-evidence/coverage-jpeg-sos-20261005.json`, generated by
`make coverage` with `nightly-2026-07-16`, an empty `RUSTC_WRAPPER`, two
Cargo build jobs and the dedicated `target/llvm-cov-jpeg-sos-20261005` target.
Its SHA-256 is
`5b6ca500bae7c9d3d2add1b9145c7ccf15afbc7879056e43853388a7cff67b0d`.

| Metric | Covered / total | Percent |
| --- | ---: | ---: |
| Lines | 88,995 / 135,609 | 65.626175% |
| Branches | 17,343 / 31,196 | 55.593666% |
| Functions | 4,983 / 8,681 | 57.401221% |
| Regions | 136,168 / 210,608 | 64.654714% |

All alpha floors pass. The separate strict verifier exits 1 because all four
100% requirements remain unmet. All seven test binaries pass 3, 4, 1, 57, 7,
1 and 68 tests, with no failures or ignored tests. Its report-era ordinary inventory
contains 2,045 rows: 1,582 active decode, 431 active encode and 32 planned
encode rows. JPEG has 216 ordinary decode rows and AVIF retains 522. All 25
target-only fault contracts remain separately counted, with their reference
status `not_applicable`.

All 5,053 recorded source/configuration/script/fixture hashes are unchanged
between the fresh incremental and full campaigns. This inventory contains
5,052 repository files and one ignored ambient `tests/fixtures/.DS_Store`.
The original source receipt retains precommit base revision
`455d66aa3a8aa7e2bf6ac1da8c75198c8d96f316`; its SHA-256 is
`fa9e7c99f5d287bd28509077d2c6bd57608c0657e2f98b343b580aeeb5492977`.
Human evidence documentation is outside that measured inventory.

All eight formatting/strict-Clippy gates pass, including debug, release,
coverage-nightly, JPEG benchmark and SSE2/AVX2 cross-target checks. All 33
feature lanes pass with 4,959 recorded file hashes and capability artifacts
unchanged. Native/WASI lanes execute tests; browser WASM lanes compile and
run strict Clippy/rustdoc. Cross-target SIMD checks establish compilation and
lint evidence, without x86 throughput measurements for this change. The
origin registry retains 140 guards across 20 files. No private unit tests,
unsafe code, source exclusions, ignored cases or lint exceptions are added.

Coverage MCP reads the full report with source unverified and tests unknown.
It groups 74,440 missing regions into 6,647 missing-region groups, with AV1
inter-translation instantiations still the largest displayed groups. Missing
counts also include 46,614 lines, 13,853 branch outcomes and 3,698 functions.
Local command/hash receipts do not create a managed MCP source/build binding
or named-test attribution. Historical managed ledger metrics and identities
remain unchanged; current integrity hashes and inventory counts are separate.

### Complete JPEG scan-length and component-count parity — 2026-10-05

Eleven complete public inputs preserve the source entropy, scan order and
final EOI bytes, changing only the selected SOS length and, where declared,
component count. The maintained `generate_jpeg_sos_edge_fixtures.py` uses the
normal JPEG asset hook, pinned source/output hashes, bounded shared marker
walking and fresh pinned Pillow lifecycle calls. Two separate output-directory
runs and `--check` reproduce all eleven bytes exactly; all 216 prior JPEG
input files remain unchanged. Inputs contain no Rust outcomes.

| Ordinary case | Bytes | Complete input SHA-256 |
| --- | ---: | --- |
| `error_sos_length_0` | 4,564 | `729419da5fe9727a0f6d751a6865c55e44bcd95094bf153c06cc7e382ce2e4d0` |
| `error_sos_length_1` | 4,564 | `a744fc417d07b52a16f2dd1f3ac0f7e784803c64b88423c2342f07c1d1ff7223` |
| `error_sos_length_2` | 4,564 | `066f03bcef810cb346ff6f55ffdb1cbab465b375d1832e894a542d2706062929` |
| `error_sos_length_11` | 4,564 | `5f235865d0e748c1f67a12d63ac883a43c73eea66cd255ae44f7e47a1534e960` |
| `error_sos_length_13` | 4,564 | `121633f20c3b9b7e4fc33e80ea817fbf6d7e4cab6a6eab2d4ab791cad029ed3d` |
| `error_sos_length_65535` | 4,564 | `63c54401762bd11614feb80554c736d1d8c80ba8be003c9d570224deefd47edb` |
| `error_sos_zero_components_length_6` | 4,564 | `538b1e21c0c658ef42b5d053ff6c379c0f7ea9a5f40ee2f53140ae3a4f8754fa` |
| `error_sos_five_components_length_16` | 4,564 | `e192d29496bf30ab3f4ea34e2f8dc5fb3608ad66942475153695dc10b5d2c393` |
| `error_sos_255_components_length_516` | 4,564 | `fffe6aba1f8e5ed5d232db2bc6eded2ddaaf20c4b4b13c11d1577cf6dbd3e4b8` |
| `error_progressive_second_sos_length_7` | 3,939 | `eb1f33e9234cb7b8ab4ebabfb199c3eb255107a67b3e9ce468eaf81b07c0298e` |
| `error_baseline_multiscan_second_sos_length_7` | 649 | `d5dd86cf6d57ea40449b16e137de0a9403512c831f5d4bcee6c995ea8ee6c232` |

Pinned Pillow 12.2.0 with bundled libjpeg-turbo 3.1.4.1 detects every input as
JPEG. Ten inputs open, inspect and verify with the unchanged source metadata,
then both pixel workflows fail with `builtins.OSError` and
`broken data stream when reading image file`. The UINT16_MAX declaration
instead fails at open with `Truncated File Read`. Three complete valid source
controls and additional valid one-/four-component controls retain exact
Pillow pixels.

The old target incorrectly materialized all six length mutations and both
later-scan mutations. Lengths 0/1 also incorrectly failed inspection and source
opening for verification. The three count mutations already failed as malformed
input; they now validate the reference's scan-shape boundary before component
reads. They are regression cases, not additional claimed old mismatches.

Inspection now stops at SOS lengths 0/1 after the bounded length read, matching
Pillow's open behavior. Normal payload bounds, truncated length fields and
other markers retain their existing handling. Pixel parsing reads the length
and count, preserves the existing zero-component error, then requires at most
four components and exactly `2 * count + 6` bytes before allocation or table
reads. The saturated u16 arithmetic is exact for every byte-sized count
(maximum 516); strict Clippy passes without a new allowance. The check runs
once per scan and adds no pixel-loop work or copies. This is a compatibility
and validation fix, without a measured throughput claim.

The exact primary source is tag 3.1.4.1, commit
`9217719d3a58633923b096af4c1d50d304768a64`. Stored tag and commit-addressed
`jdmarker.c` bytes both have SHA-256
`1dd922aff8f92ece4f8f2b11eea08cd49b2e232de797ee98db9e331f6d0e3296`;
its `get_sos` reads length and count before validating the shape. Independent
review reproduced the complete inputs and verified all entropy/tails without
calling the repository generator or either codec.

The preserved red probe matches the root dependency lock, including wide 1.7.0.
The fresh green probe matches all eleven ordinary outcomes, preserves exact
Debug/display diagnostics, stages, offsets and identities of all seventeen
prior SOS rows, and preserves complete metadata/pixels for five normal
controls. Static JPEG sequence failures retain their existing `StillDecode`
stage. Initial observer field/lock/tooling failures and the first green
observer's executable-mode failure remain unsuccessful separate evidence;
no result from them becomes a passing receipt. The final green receipt is
`public-green-receipt-v2.json`, SHA-256
`13a8f62b4ed4346a1178bcbbdd8d16a343e7347f2adf3c6198dbb214469d2d79`.
Canonical Pillow regeneration preserves all 205 old JPEG decode rows, 81 encode
rows, encoded/raw bytes, every other format row and all 25 fault rows. The
malformed ledger preserves 547 prior classes and adds eleven, for 558 total.

#### Incremental execution and Coverage MCP limits

Changed source requires a fresh baseline. Twelve selections use the same
finished source and identical hashes for all 387 compiled artifacts: the
previous 205 ordinary JPEG rows plus its one existing target-only allocation
fault, followed by the eleven ordinary cases individually. Every selection
passes. The fault remains `oracle_status: not_applicable`; no new injected
failure is needed for input-reachable scan validation.

Each selection resets only raw profiles, uses `--no-clean`, and requires fresh
nonempty profiles before reporting. The first selection takes 95.456 seconds
including the build; subsequent Cargo executions take 0.789–0.828 seconds.
These are workflow timings with concurrent checks, not codec throughput.

Coverage MCP and the independent LCOV union agree on 5,630 to 5,634 covered
line coordinates out of 131,108. The first length-zero case adds four; the
other ten retain distinct boundary/later-scan regression value with zero
additional line-coordinate gain. This selected coordinate denominator differs
from the full aggregate line denominator above. No earlier-source report or
selected-test absence is used to claim a regression.

Both LCOV and LLVM JSON branch-union requests return `incomparable` because
normalized baseline detail does not match reported branch totals. Those
responses are preserved; no aggregate branch union is claimed. Bounded raw
parser/header observations and the full four-metric report remain separate
from this provider limit. The original line query was limited, with source
unverified and tests unknown. A subsequent read-only audit adapted the immutable
producer receipts into optional sidecars before the TIFF source change.
Coverage MCP then verified **9,155 → 9,157 / 210,588 regions** for all eleven
JPEG cases, with no report or source mutation. Branch union remained
incomparable: 31,190 coordinate arms versus 31,196 native arms. This historical
fixed-source result and its producer audit are preserved in the JPEG bundle;
it is not a comparison against the later TIFF source.

The ignored evidence bundle is
`target/release-evidence/jpeg-sos-455d66aa-20261005/`, with the original failed
attempts, pinned primary source/inputs, generator and independent reviews,
red/green outcomes, preservation/accounting receipts and strict checks.
Incremental/full LLVM reports, source receipts, MCP responses and the 33-lane
feature receipts are beside it in `target/release-evidence/`.

### Rejected JPEG packed quantizer correction deletion (2026-10-05)

A separate worktree at `78fc8c8c24126b6c095f3746569dba0bfaf94921`
tested removing the ceiling-reciprocal correction from the private packed
four-block JPEG quantizer. An independent review checked all 13 byte-origin
callers, the clamped quantization tables, pinned `wide 1.7.0` arithmetic, and
the reciprocal bound. The arbitrary-i32 quantizer retained its correction.
The candidate passed strict Clippy and formatting, 81/81 public JPEG encode
rows (379 calls, zero panics), 205/205 decode rows, and all 20 complete-byte
baseline/candidate output comparisons before each timing campaign.

The fixed `paired-five-rounds` and `paired-five-rounds-reversed` campaigns
(local sessions 53108 and 36102, both exit 0) each completed 40 whole-public-call
workloads across 20 configurations and five rounds, with opposite starting
order and the unchanged TurboJPEG 3.2.0 oracle following each Rust variant.
Each campaign saved 800 raw records. Source, binary, and oracle-library hashes
were unchanged. The Apple M3 Pro ARM64 host used macOS 15.7.7 and Rust 1.96.1
with LLVM 22.1.2; the Rust builds used ordinary release settings and the C
harness used `-O3` with TurboJPEG SIMD enabled.

Ratios below are candidate/baseline median paired times, first/reversed
campaign. Values above 1 mean slower complete calls.

| Encode workload | Raw ratio | Oracle-normalized ratio |
| --- | --- | --- |
| RGB 128x128 Q10 4:2:0 | 1.0727 / 1.0729 | 1.0727 / 1.0725 |
| RGB 512x512 Q10 4:2:0 | 1.0685 / 1.0825 | 1.0729 / 1.0886 |
| RGB 1024x1024 Q85 4:2:0 | 1.0290 / 1.0433 | 1.0323 / 1.0411 |
| RGB 128x128 Q85 4:2:2 | 1.0572 / 1.0597 | 1.0552 / 1.0324 |
| Grayscale 128x128 Q85 | 1.0504 / 1.0628 | 1.0550 / 1.0586 |
| Grayscale 512x512 Q85 | 1.0628 / 1.0749 | 1.0610 / 1.0728 |

Of the 17 configurations admitted to the changed encoder path, 15 had slower
raw medians in both campaigns and 14 had slower normalized medians in both.
None improved both raw and normalized medians in both campaigns. CMYK128's
small raw improvement reversed after normalization; RGB512 Q85 changed
direction between campaigns. Three unaffected encode configurations and all
20 decode workloads remain separately reported controls, not a combined score.

No repository Cargo, test or profiling jobs overlapped the timing window. The
first-start process snapshot showed unrelated Python jobs and macOS media
analysis consuming CPU. Those
heavy jobs were absent from the reverse-start snapshot, which retained smaller
background activity. Some controls also varied, including progressive decode;
these measurements do not establish a physically idle host or attribute every
timing change to the quantizer. Repeated material encoder penalties and the
absence of a repeatable acceptable benefit justified rejection. Static ARM64
instruction deletion did not establish lower runtime cost. No numeric SSE2,
AVX2, or world-fastest result is claimed, and the candidate was not applied to
main.

The ignored evidence bundle is
`target/release-evidence/jpeg-packed-quantizer-78fc8c8c-20261005/`: both raw
campaigns and summaries, `comparison.json`/CSV, exact JPEGs, source snapshots,
patch, binaries, build/check/range-review receipts, hardware/load snapshots,
and the rejection and independent measurement review. The baseline encoder
SHA-256 was
`5451cb6570679fe9ea13b385900e5270581220e35cf1993c09eeb0b1a799afab`;
candidate encoder
`df73cfae0d0accc85e54a2e2d11412915d26a54d4778b38153797e59356ae7a7`;
candidate patch
`85548d5ab3a7ee901d022b86a5ccf0e6acb25e2e0646ef934c1c28629a10c5e1`.

## Preceding AVIF container and multi-tile full campaign — 2026-10-05

This preceding full local report is
`target/release-evidence/coverage-avif-multitile-container-v2-20261005.json`,
generated by `make coverage` with `nightly-2026-07-16`, an empty
`RUSTC_WRAPPER`, two Cargo build jobs and the dedicated
`target/llvm-cov-multitile-container-v2-20261005` coverage target. Its SHA-256 is
`027665b7e2cb4e54182b7929ecd568ff83941b839f3708d29277747c4720103e`.

| Metric | Covered / total | Percent |
| --- | ---: | ---: |
| Lines | 88,986 / 135,603 | 65.622442% |
| Branches | 17,338 / 31,192 | 55.584765% |
| Functions | 4,983 / 8,681 | 57.401221% |
| Regions | 136,157 / 210,598 | 64.652561% |

All alpha floors pass. The separate strict verifier exits 1 because all four
100% requirements remain unmet. All seven test binaries pass 3, 4, 1, 57, 7,
1 and 68 tests, with no failures or ignored tests. They exercise 522 ordinary
AVIF decode rows, 25 separate target-only fault contracts and 88 native loop
witnesses. At that report’s source, the ordinary inventory contains 2,034 rows: 1,571 active
decode, 431 active encode and 32 planned encoder rows.

All 5,041 recorded local source/configuration/script/fixture hashes remain
unchanged between the fresh incremental and full campaigns. That inventory
contains 5,040 files committed in the checkpoint and one ignored ambient
`tests/fixtures/.DS_Store`; it is not a count of newly created source files.
The original local receipt preserves its precommit base revision. A separate
Git binding checks the measured files against committed blobs without
rewriting that receipt. Documentation changed after execution is outside this
measured source inventory.

All eight formatting/strict-Clippy gates pass, including debug, release,
coverage-nightly, JPEG benchmark, SSE2 and AVX2 cross-target checks. All 33
feature lanes pass with the 4,940 recorded local file hashes unchanged and
capability artifacts unchanged. Native/WASI lanes run tests; browser WASM
lanes compile and run strict Clippy/rustdoc. Cross-target SIMD checks establish
compilation/lint evidence. They supply no x86 throughput measurement. The
coverage origin registry has 140 guards across 20 files, with no source
exclusions. No new private unit tests, unsafe code, ignored cases or lint
exceptions are introduced.

Coverage MCP reads this full report with source unverified and tests unknown.
It groups 74,441 missing regions into 6,647 missing-region groups; these are
not counts of wholly uncovered functions. Missing counts also include 46,617
lines, 13,854 branch outcomes and 3,698 functions. The largest displayed groups
remain AV1 inter-translation instantiations. Local source/command/hash receipts
do not create a managed MCP source/build binding or named-test attribution.

### Complete AVIF container and multi-tile sequence cases — 2026-10-05

Thirteen new ordinary cases give 522 AVIF decode rows and 2,034 ordinary rows
(1,571 active decode, 431 active encode, 32 planned encode). The 25 target-only
fault contracts remain separate. These are current inventory/reference counts;
execution and coverage claims require completed campaign receipts.

The maintained container-edge and multi-tile-motion generators use the normal
AVIF asset hook, valid bounded BMFF boxes and byte-identical repeated inputs.
Pinned live Pillow 12.2.0, with libavif 1.4.1/dav1d 1.5.3/libaom 3.13.2,
supplies all public outcomes and raw pixels. No Rust outputs become ordinary
inputs or references. Format detection succeeds for every input.

| Ordinary case | Bytes | Complete input SHA-256 | Live Pillow outcome |
| --- | ---: | --- | --- |
| `animated_tkhd_version_zero` | 1,223 | `eb7b131fb8288fbe8e67daf9e41e7f80bdf3f923d474177fac84a0b58550b674` | Open, inspect, verify, decode and sequence decode succeed; all five frames, metadata and timing match the source. |
| `animated_tkhd_version_zero_unknown_duration` | 1,223 | `ca1834317cd4b5b0db00130370cf3db3c94426ff7479c6414c4da2cabfef2a09` | Open, inspect, verify, decode and sequence decode succeed; all five frames, metadata and timing match the source. |
| `ipma_optional_zero_index` | 5,907 | `590514b40d0df05335861bf4345eb99068dc1264d38715f23f8e931c5376aaf3` | Open, inspect, verify, decode and sequence decode succeed with unchanged still pixels and metadata. |
| `error_ipma_essential_zero_index` | 5,907 | `b4f6f62f44d85c84a27bd98c1ad9529453c7ce31fa88602fc88eae0641b06c80` | Open fails with `PIL.UnidentifiedImageError`; inspect, verify, decode and sequence decode report malformed input. |
| `error_ipma_property_index_out_of_bounds` | 5,907 | `ca46ccb69a865f237092cf3ed7fb77862fa3faff112d311f860ecfb91eec77dd` | Open fails with `PIL.UnidentifiedImageError`; inspect, verify, decode and sequence decode report malformed input. |
| `iloc_file_data_reference_1` | 3,077 | `d4b06f63b3bd3ce2bd409930b7c2892236000ff4583e70e771f4a21573dbb6af` | Open, inspect, verify, decode and sequence decode succeed with unchanged still pixels and metadata. |
| `iloc_file_data_reference_65535` | 3,077 | `a9d263c7af3c3d1219cf2187bef5916ace90bf1018dfb6fa58650f99f51be0f7` | Open, inspect, verify, decode and sequence decode succeed with unchanged still pixels and metadata. |
| `iloc_idat_data_reference_1` | 5,906 | `b0689513b0693085ef11397fa967bcc56741ae79b80d8a81d4ef424ce561bea2` | Open, inspect, verify, decode and sequence decode succeed with unchanged still pixels and metadata. |
| `iloc_idat_data_reference_65535` | 5,906 | `756ea5b7a21c472cee85913fbb64260fccd5267f98900dd2b815760738f07eb4` | Open, inspect, verify, decode and sequence decode succeed with unchanged still pixels and metadata. |
| `animated_motion_multitile` | 1,264 | `c248f019a008e9dea4425e1d5f44d1418b44929604312d95a69a0a3ac5d4709f` | Open, inspect, verify, decode and sequence decode succeed; four exact RGB 256×128 frames, each lasting 100 ms. |
| `animated_motion_multitile_split_groups` | 1,280 | `eb19bcebc0698dfa3149c76fb6828705b22e37f45e213b70541a61b3abd20b89` | Open, inspect, verify, decode and sequence decode succeed; four exact RGB 256×128 frames, each lasting 100 ms. |
| `animated_lossless_motion_multitile` | 1,327 | `24612186624c6b470a50a549d876469c8265976633afee87fa030d910cd0532b` | Open, inspect, verify, decode and sequence decode succeed; four exact RGB 256×128 frames, each lasting 100 ms. |
| `animated_lossless_motion_multitile_split_groups` | 1,342 | `04b61cf8b3e9887ee651cec5dc055b5a102198de9f69e2ad64b4e183639cf596` | Open, inspect, verify, decode and sequence decode succeed; four exact RGB 256×128 frames, each lasting 100 ms. |

The two association errors retain `cannot identify image file <bytes>`.
The essential-zero input asset is `ipma_essential_zero_index.avif`.
Track mutations narrow tkhd version 1 to version 0 and rebase the file extent
and chunk offset by twelve bytes, preserving other fields and media bytes.
Both versions, including the `UINT32_MAX` duration sentinel, retain five
frames and native one-play repetition. Association/data-reference mutations
retain every media byte. Split-group variants preserve each encoded tile and
parsed frame-header field, repacking complete sample extents and track tables.

#### Ordinary compatibility fixes

The Q100 pair exposed a real lossless sequence gap. Before the fix, public
still decoding succeeded while both sequence decodes returned
Unsupported/SequenceDecode/NotImplemented with
`decode sequence: AVIF sequence validation failed: AVIF sequence rendering has no completed color surface`.
Pillow decoded four RGB 256×128 frames. Source inspection traced the gate to
comparing a full 256-pixel reference width with a checked 128-pixel tile width;
the public probe alone did not locate the failing sample.

The generic lossless inter gate now admits validated, unreduced full references
shared by checked tiles and preserves the previous admitted-width alternative.
`ScaleFactors` still uses full current/reference dimensions; `scaled == false`
requires exact dimensional equality. Depth/layout, surface, tile bounds and
tool checks remain; local reconstruction/global prediction coordinates are
unchanged. Fixed public sequence decodes of both Q100 variants match all four
Pillow frames exactly, and still pixels equal frame zero. The Q80 source/split
pair retains the same complete parity. This proves the bounded witnesses, not
every possible multi-tile block grammar.

The four nonzero data-reference files exposed another ordinary gap: Pillow
accepts 1 and 65,535 for construction method 0/file and 1/idat; the preceding
Rust probe rejected all four at inspection, still decode and sequence decode.
Pinned libavif `read.c` reads the field without storing/using it; `idatStored`
selects local bytes. Both Rust parsers and the independent full sample inspector
now consume the fallible u16 without rejecting its value. Reserved bits,
supported methods, checked arithmetic and existing file/idat bounds remain.
The field cannot select an external resource. Fixed public still/sequence
pixels match the live Pillow witnesses without a fault label or private
comparator.

#### Native provenance and preservation

Six new native observations cover both tkhd variants and all four motion
inputs. All 82 prior record dictionaries and native artifact records/bytes
remain exact. The 88-record bundle has 80 accepted complete decodes and eight
malformed inputs; 267 artifacts total 9,816,480 bytes, including 9,293,948
Pillow frame bytes. Additional collections bind their actual compiler and
observer-binary identities, preserving original metadata. Native repetition
is 0 for both tkhd variants (one total play), and -1 for the motion inputs
(infinite). The native index SHA-256 is
`38b94714d241f811741a4116bb80cbb9e1479feb8360da99a001bad37037b159`.

Canonical generation preserves all 509 prior AVIF operation observations,
other-format observations and AVIF encode rows. Only
`sequence.loop_evidence.index_sha256` refreshes in 57 output rows and their
57 matrix rows. Existing native identities, input hashes, loop counts, pixels,
durations, errors and flags stay unchanged. All prior 24 fault rows and
1,876 raw files retain exact values/bytes. The 37 new ordinary canonical raws
match stored live Pillow bytes in full. Another 32 new files are six native
witness inputs plus 26 Pillow frames. Only two association-error classes
extend malformed accounting: all 545 prior class dictionaries remain exact,
giving 547 classes. Later count/integrity updates are separately recorded
accounting metadata.

#### Capacity-growth-only allocation contract

`avif_retained_temporal_sample_reservation_failure` is a separate target-only
case using ordinary `animated_motion_multitile_split_groups`. Its
`av1.frame.retained_temporal_sample_reservation` hook consumes only when a
partial group's nonempty sample count exceeds spare vector capacity. Empty
or already-fitting groups cannot consume it. `usize::MAX` additional
nonzero-sized samples causes an actual deterministic `try_reserve` capacity
failure without host OOM. Normal builds retain the actual sample request,
existing error closure and ordering before pending-state publication.

The generic shared runner requires AVIF Dimensions at SequenceDecode and
`decode sequence: AVIF sequence validation failed: unable to reserve retained AV1 temporal-MV samples`.
Existing thread-local scoping consumes once and restores state on return or
unwind. A successful retry must equal a fresh uninjected Rust sequence decode;
its ordinary source independently checks exact Pillow frames. Injection/retry
has `oracle_status: not_applicable`, not Pillow allocation attribution. The
new hook is `defensive_model`; current origins total 140 guards across 20 files.

#### Identity and claim limits

Clean pinned libavif commit is
`6543b22b5bc706c53f038a16fe515f921556d9b3`; `read.c` SHA-256 is
`d2e4062f7030ffa5843a3c9cc4984e382a07782ce2241d8b5354fda888266e85`.
The Pillow handshake checks wheel/runtime pins. Public-probe receipts retain
actual source/input hashes and locked dependency identities. Independent
canonical/source audits are retained under
`target/release-evidence/avif-multitile-current-78fc8c8c-20261005/`; generator,
native and before/after proofs are under the companion
`next-public-gaps-78fc8c8c-20261005/` and
`avif-multitile-inter-candidate-78fc8c8c-20261005/` directories.

The existing native parity test now pins 267 artifacts and all 88 names,
including the six new matrix source mappings. Its four existing test functions
and preceding helper/assertion bodies are preserved. Fresh ignored native
collections have their own build identity and ordering; publishing additions
preserves the accumulated canonical index's record order and provenance.

#### Fresh incremental campaign and failed-run preservation

The first 15 selected runs pass on their recorded source. The subsequent full
run fails only the old 235-artifact assertion in the existing native parity
test. Its failed terminal receipt and log remain preserved; no successful full
report is attributed to that run. The existing test's six new witness names
and source mappings are added with the 267-artifact assertion. Source changed,
so a fresh baseline and all 14 additions are remeasured in the separate `v2`
campaign; reports from the two source inventories are not combined.

The fresh baseline executes 509 preceding ordinary AVIF rows plus 24 preceding
faults. Thirteen ordinary selections and one target-only fault then run
separately. All 15 selections pass. Each case resets only profiles with
`cargo +nightly-2026-07-16 llvm-cov clean --profraw-only`; the normal combined
LCOV invocation uses `--no-clean`, followed by a separate JSON report. The
receipt requires fresh nonempty profiles and keeps all 387 compiled-file hashes
identical between selections. Profile absence cannot reuse stale profdata.
The complete campaign takes 106.450857 seconds including its first build;
subsequent selected Cargo calls take 0.809–0.866 seconds on this host. These
are workflow timings, with no codec throughput claim.

The independent LCOV union and all 14 Coverage MCP incremental comparisons
agree: 45,247→45,290 observed lines out of the fixed 131,102 selected-report
coordinate denominator. Marginal gains in campaign order are
16, 3, 0, 0, 10, 1, 6, 1, 1, 0, 0, 0, 0 and 5 lines. The two Q100 sources and
four data-reference inputs remain regression witnesses despite zero marginal
lines after earlier selections. This selected denominator remains distinct
from the full report's aggregate line total.

The retained allocation-error closure at `frame.rs:1598–1602` has three LLVM
regions, each with baseline count zero, selected fault count one and full-suite
count one. The fault's structured error and retry assertions pass separately
from its ordinary source's exact Pillow parity. Selected-report absences are
not regressions. MCP retains limited coordinate evidence with source unverified
and tests unknown. The 100% and fastest-codec goals remain open.

The `avif-multitile-container-v2-*` local receipts, immutable full source receipt,
independent coverage audit and `coverage-mcp-multitile-v2-*` summaries retain
these results. The current malformed and claim-integrity accounting is updated
before the fresh campaigns, preserving all historical managed measurement IDs,
counts, inputs and preceding report observations.

### Preceding full AVIF location and temporal coverage — 2026-10-05

The preceding full local report is
`target/release-evidence/coverage-avif-iloc-temporal-20261005.json`, generated by
`RUSTC_WRAPPER= CARGO_BUILD_JOBS=2 COVERAGE_REPORT=target/release-evidence/coverage-avif-iloc-temporal-20261005.json make coverage`
with `nightly-2026-07-16`. It records 88,946/135,595 lines (65.596814%),
17,313/31,190 branches (55.508176%), 4,982/8,681 functions (57.389702%), and
136,103/210,590 regions (64.629375%). All alpha floors pass. The strict 100%
verifier exits with status 1 with all four requirements still unmet; that goal remains
open. The report SHA-256 is
`1de66283960b30d4afbf90213e54a0a111663a48a5d59ca37a341fe7a9de0f7f`.

All seven test binaries pass: 3, 4, 1, 57, 7, 1 and 68 tests, with no failures
or ignored tests. The matrix harness covers 509 ordinary AVIF decode rows,
24 separate target-only fault contracts and the native repetition bundle's
82 cases. The ordinary decode/encode inventory contains 2,021 rows: 1,558
active decode rows, 431 active encode rows and 32 planned encoder rows.
All 505 preceding AVIF observations, other-format observations and 23 preceding
fault rows remain unchanged.

The full local receipt records unchanged hashes for 4,957 local files:
4,956 tracked source, configuration, script and fixture files,
plus one ignored ambient `tests/fixtures/.DS_Store` file. The expanded inventory
includes 109 existing committed scripts and five new fixture files; it does
not mean that 114 source files were created. The full receipt separately records
the generated `malformed_ledger.json` and its `claim_ledger.json` integrity
refresh after the incremental campaign. All executable source, input/reference
and matrix hashes remain identical between those campaigns.

Relative to the preceding full report, covered counts increase by 36 lines,
eight branch outcomes, four functions and 42 regions. Source totals increase
by 30 lines, eight branches, two functions and 40 regions. Those changes span
the generic inspection/decode staging fix and the new coverage hook; they are
descriptive counts for different source. They remain separate from the 14 new
line observations isolated by the same-source incremental campaign below.
No source exclusions are used.

`make fmt`, `make lint` and `make lint-x86-simd` pass all eight recorded
formatting/strict-lint gates with identical hashes for 153 Rust/configuration
files. The SSE2 and AVX2 gates are cross-target compile/lint checks. All 33
feature lanes pass with all 4,855 recorded local hashes unchanged, including
4,698 codec fixture/observation files and the separate ambient Finder file.
Native and WASI lanes execute runtime checks; browser WASM lanes compile and
run strict Clippy/rustdoc checks. Capability artifacts remain unchanged.
The two accounting-file updates made after these campaigns are recorded
separately. The exact `cfg(coverage)` inventory is 139 guards across 20 files.
No new private unit tests, unsafe code, ignored cases or lint exceptions are
introduced.

Coverage MCP reads the full report with source unverified and test attribution
unknown. It groups the 74,487 repository-wide missing regions into 6,648
missing-region groups; these are not counts of wholly uncovered functions.
There are also 46,649 missing lines, 13,877 missing branch outcomes and 3,699
missing functions. The largest displayed groups remain repeated AV1
inter-translation instantiations. An AVIF display filter retains the global
totals. Local command/hash receipts do not create a managed MCP source/build
binding or named-test attribution.

### Complete AVIF location cases and error staging — 2026-10-05

Four ordinary cases extend the existing indexed-idat input. All are complete
files with valid enclosing BMFF box sizes. The maintained idat generator and
normal AVIF asset hook construct their inputs; pinned live Pillow 12.2.0
produces the operation observations and raw reference. Its runtime is libavif
1.4.1, dav1d 1.5.3 and libaom 3.13.2. Repeated generation is byte-identical.

| Ordinary case | Bytes | Complete input SHA-256 | Live Pillow outcome |
| --- | ---: | --- | --- |
| `iloc_idat_split_extents` | 5,918 | `b92d22f904a48d380a99f7b91ff78aa73ab4fc73e9d2f8b71697d843881aa1bd` | Open, inspect, verify, decode and sequence decode succeed with the original pixels and frame metadata. |
| `error_iloc_truncated_extent_index` | 5,896 | `62f5d6dd3b3cfb368030407977a2af434659c178c1412844781e43c03e25d8ed` | Open fails with `PIL.UnidentifiedImageError`; inspect, verify, decode and sequence decode report malformed input. |
| `error_iloc_index_size_three` | 5,906 | `53be83151f9031fbc56c0f3bc91fd50f5c2b7b3376ac760fb662b458eadc572b` | Open fails with `PIL.UnidentifiedImageError`; inspect, verify, decode and sequence decode report malformed input. |
| `error_iloc_idat_extent_out_of_bounds` | 5,906 | `1e0f61d11eb4cf291f4af89d3636ff874f517358e2eecb20d8b92d1a4a05e46b` | Open, inspect and verify succeed; decode and sequence decode fail with `builtins.SyntaxError`. |

Format detection succeeds for all four. The two open failures retain the
normalized Pillow message `cannot identify image file <bytes>`. The overrun
retains the exact decode message
`Failed to decode frame 0: BMFF parsing failed`. Its successful header is RGB,
128×128, one frame, so an inspection failure would violate the public contract.

The split input retains the nonzero base offset of eight. Its two zero-index
extents use relative offsets eight and 1,405, with lengths 1,397 and 1,398;
they concatenate to the original 2,795 AV1 bytes. The truncated-index case ends
its bounded `iloc` after two of the four declared index bytes. The width case
declares an unsupported three-byte index. The overrun changes only the extent
length from 2,795 to 2,796: its relative end exceeds the 2,811-byte idat payload
by one while remaining inside the outer file. Adjacent bytes cannot satisfy
that item's source bound. All `ftyp`, original unreferenced `mdat`, non-`iloc`
metadata children and idat/AV1 bytes remain unchanged. The AV1 payload SHA-256
is `be0bf650b8612533577e4b47a60989bb3f092cfb256b81f3bd84e3b5ce8ea199`.
The split row compares all 49,152 decoded bytes with the independent reference.

The generic Rust inspection path now validates container/configuration metadata
while retaining declared idat spans with checked arithmetic. Decode extraction
checks every item's file/idat bound before using encoded payloads. Grid-descriptor and
retained opaque-metadata reads remain bounded to the selected source; opaque
extents are checked before reserving their declared size. File-backed items
and sequence sample-table spans retain their existing bounds checks. This
preserves Pillow's lazy item-payload failure stage without permitting a read
outside idat or adding a case-specific comparator.

Independent bit-depth enrichment now uses `inspect_color_configurations` to
read bounded item properties or track sample descriptions without reading item
extents. The full sample inspector remains strict and rejects the overrun with
`item extent exceeds source bytes`. All 29 preceding inspector function bodies
and all 470 successful preceding inspection-depth values are unchanged.
`generate_decode_refs.py` catches only public Pillow open failures when
recording inspection errors; an independent enrichment failure now aborts
generation instead of being mislabeled as a Pillow outcome.

During ordinary input/reference generation, 4,690 of 4,694 preceding fixture
files stay byte-identical. The ordinary generator changes only the three AVIF
aggregate JSON/index files; the concurrently changed origin registry belongs
to the separate fault work. Four complete inputs and one raw reference are
added, with no prior files deleted. All 505 preceding AVIF row dictionaries,
other-format matrix rows and 23 preceding fault rows are unchanged. Later
count/integrity updates are accounting metadata, separately recorded.

### Nonempty temporal-sample allocation contract — 2026-10-05

The target-only row `avif_tile_group_temporal_sample_reservation_failure`
reuses the ordinary `animated_motion_chroma` input. Its coverage-only point,
`av1.frame.tile_group_temporal_sample_reservation`, is consumed only when a
reconstructed tile has nonempty temporal-MV samples. Empty intra-frame work
cannot consume it. A real `try_reserve(usize::MAX)` fails deterministically;
the production request remains the actual sample count, with the same existing
Dimensions error closure.

The shared public runner requires AVIF SequenceDecode and the exact message
`decode sequence: AVIF sequence validation failed: unable to allocate AV1 tile-group temporal-MV samples`.
It then checks a successful uninjected retry against a fresh Rust sequence
decode, including all returned content. The ordinary source row separately
supplies independent Pillow sequence/pixel parity. One-shot fault state is
cleared before the retry. This row remains in the shared matrix's
`oracle_status: not_applicable` lane; all 24 fault contracts are separate from
the 2,021 ordinary rows. Ordinary image bytes cannot deterministically force
this allocation failure.

### Same-source incremental observations and accounting — 2026-10-05

The six selected runs first execute all 505 preceding AVIF rows plus 23
preceding fault contracts, then each of the four new ordinary rows and the new
fault row separately. All selections pass. All 4,957 recorded local hashes
remain unchanged throughout execution and the subsequent MCP comparisons.
The independently checked LCOV coordinate union is
45,228→45,230→45,233→45,234→45,237→45,242 observed lines out of 131,094:
gains of two, three, one, three and five lines, in the table order above followed
by the temporal-sample fault. No coordinates are added or removed. This fixed
selected-report denominator differs from the full report's 135,595 lines.

The truncated-index selection observes the error-return region at
`samples.rs:1122` in both inspection/decode instantiations: preceding counts
zero/zero become four/two. The temporal-sample fault observes the existing
allocation-error closure at `frame.rs:2941–2943` once, from zero. Those exact
regions are also observed in the full report. MCP's raw LCOV/LLVM results retain
source unverified and tests unknown. These coordinate observations do not
establish full-suite regressions, managed source binding or named-test
attribution.

The preliminary `make verify` catches the stale generated malformed ledger.
After all incremental tests and MCP comparisons finish, the maintained
generator adds exactly the three ordinary AVIF error classes, increasing the
ledger from 542 to 545 classes while preserving all preceding class values.
Only that ledger and its current claim-ledger integrity digest change between
the incremental and full receipts. Rust runtime tests do not read the malformed
ledger; executable source, inputs, references and the shared matrix stay
unchanged. Historical managed run/snapshot identities, metrics and measured
inputs are preserved. The later verification passes. Receipts are retained in
`target/release-evidence/avif-iloc-edge-cases-d9efb879-20261005/`, the
`avif-iloc-temporal-*` campaign paths and the corresponding `coverage-mcp-iloc-*`
summaries.

### Rejected aligned JPEG 4:2:2 conversion fusion — 2026-10-05

An isolated experiment based on `d9efb879` fused RGB conversion with h2v1
chroma downsampling for no-token Cs422 inputs whose width is divisible by 16.
It removed two full-size chroma allocations while preserving saturation before
averaging. Both frozen variants passed 205 JPEG decode and 81 JPEG encode public
matrix rows, strict Clippy and all 20 exact JPEG benchmark-encoded-byte comparisons.

The append-based variant took 1.0191× the baseline's time on the affected RGB 128
Q85 Cs422 encode workload. A direct-store refinement measured 0.9958× and 0.9927×
in opposite-start five-round campaigns, while its oracle-normalized direction
crossed (1.0029× / 0.9974×). Normal RGB 512 Q10 Cs420 encodes measured 1.0148× / 1.0089×
and Q85 Cs420 encodes 1.0039× / 1.0139×, with higher normalized medians in both runs.
Every complete 40-workload campaign and raw sample remains in the ignored
`target/release-evidence/jpeg-next-performance-d9efb879-20261005/` evidence.

Both variants were rejected and reverted. No JPEG optimization or speed claim
was retained; these local ARM64 results do not establish x86 performance.

## Preceding indexed-idat, color CDEF and H4 full campaign — 2026-10-05

The preceding full local report is
`target/release-evidence/coverage-avif-indexed-idat-cdef-20261005.json`, generated by
`RUSTC_WRAPPER= CARGO_BUILD_JOBS=2 COVERAGE_REPORT=target/release-evidence/coverage-avif-indexed-idat-cdef-20261005.json make coverage`
with `nightly-2026-07-16`. It records 88,910/135,565 lines (65.584775%),
17,305/31,182 branches (55.496761%), 4,978/8,679 functions (57.356838%), and
136,061/210,550 regions (64.621705%). Every alpha floor passes. The strict
verifier still fails all four 100% requirements; that goal remains open.

All seven test binaries pass, including 57 coverage-matrix harness tests,
505 AVIF decode rows, 23 separate target-only fault contracts, the native
repetition bundle's 82 cases, seven decode-policy tests and 68 feature-gate
tests. The ordinary matrix contains 2,017 rows. All preceding Pillow case
observations and 21 preceding fault rows remain unchanged. The local full
receipt records unchanged hashes for 4,843 local files, including 4,842 tracked
source, configuration and fixture files plus one ignored Finder metadata file.
The report SHA-256 is
`6a4d058c0d3a2c6dad13eea36f71da69bfcce4bcf69b9151f2b54c9bbb95db0e`.

Relative to the preceding full report, covered counts increase by 18 lines,
six branch outcomes, two functions and 24 regions. Source totals decrease by
362 lines, 14 branches, 13 functions and 470 regions across the H4 cleanup and
new coverage hooks. These source-total changes remain separate from the
eight additional line observations isolated by the same-source incremental
campaign below. No source exclusions are used.

`make fmt`, `make lint`, `make lint-x86-simd`, `make verify` and all 33 native,
WASI and WASM feature lanes pass. Strict lint retains identical hashes for
153 Rust/configuration files; the feature campaign retains all 1,334 input
hashes without capability-artifact drift. The two new defensive fault points
increase the exact `cfg(coverage)` inventory from 136 to 138 guards across
20 Rust files. Existing guard origins are retained. No new unit tests,
unsafe code, ignored cases or lint exceptions are introduced.

Coverage MCP reads the full report as source-unverified with test attribution
unknown. It retains 74,489 missing regions and 13,877 missing branch outcomes.
The largest groups remain repeated AV1 inter-translation instantiations; the
retained luma-only H4 fallback still has 24 unobserved branch outcomes.
Local command/hash receipts do not create managed MCP evidence. This
checkpoint makes no measured performance or fastest-codec claim.

### AVIF per-tile CDEF region-map reservation fault contract — 2026-10-16

`fault-contract:avif:avif_tile_cdef_region_map_reservation_failure` reuses the
active `decode:avif:multitile_color_split_groups` Pillow input, SHA-256
`654ef88dd8ebc71a554c03f721ad97581fcf2d2df0d825020d267939eb8f5e34`. Its
coverage-only fault point forces the per-tile CDEF region-map reservation in
`entropy.rs` to fail before resizing. The selected target-only contract passed
1/1: the public AVIF `StillDecode` `Dimensions` error has message
`decode: AVIF AV1 validation failed: unable to allocate AV1 CDEF region map`,
and retry output equals a fresh uninjected decode. `oracle_status` is
`not_applicable`; the separate fault result does not contribute to Pillow
parity counts. The hook is compiled only for coverage builds.

Coverage MCP finds no missing branch observation at `entropy.rs:11308` in the
selected report. The report has no source/build receipt and test attribution is
unknown. The incremental comparison with the saved full baseline is
`incomparable` because exact branch detail is incomplete, so this remains
selected coordinate evidence and makes no aggregate coverage claim.

Report:

- `target/release-evidence/coverage-avif-tile-cdef-reservation-selected-20261016.json`

### Indexed AVIF item, color CDEF faults and luma-only H4 — 2026-10-05

The ordinary parity row `iloc_idat_indexed_extent` adds a four-byte zero extent
index to the pinned method-one idat input. Its version-one `iloc` declares
`index_size = 4`; construction method one leaves that index unused. Base and
extent offsets remain eight bytes each, and all 2,795 AV1 bytes, idat payload,
`ftyp`, original `mdat` and non-`iloc` metadata children remain unchanged.
Bounded box reconstruction adds four bytes to `iloc` and `meta`. The complete
5,906-byte input has SHA-256
`d90e970b5b9570c5091df64871d58a8f500c72df294c456037fad90cc1d92aac`.

Live pinned Pillow 12.2.0 with libavif 1.4.1, dav1d 1.5.3 and libaom 3.13.2
accepts the indexed input with exactly the source image's pixels, metadata and
frame observations. The maintained generator now verifies both idat variants
before writing either and retains its original returned path. The normal AVIF
asset hook and independent reference generator use the same workflow. Both
repeated generation runs and `generate_decode_refs.py --format avif` pass.
During input/reference generation, 4,689 of 4,692 preceding fixture files
remain byte-identical; only the three aggregate AVIF JSON/index files change,
and the new complete input and 49,152-byte raw reference are added. Subsequent
current-count and integrity-hash updates affect only accounting metadata.
Historical coverage metrics, measured inputs and managed identities remain
unchanged.

Two new target-only rows,
`avif_assembled_cdef_region_map_reservation_failure` and
`avif_assembled_cdef_active_map_reservation_failure`, reuse the active
`multitile_color_split_groups` parity input. Ordinary image bytes cannot
reliably force either allocation failure. Each coverage-only point requests
`usize::MAX` capacity at one fallible vector reservation, takes the existing
structured AVIF still-decode Dimensions error, and then checks a successful
uninjected retry against a fresh Rust decode. The existing ordinary source row
separately supplies independent Pillow pixel parity. Error messages and
production reservation counts remain unchanged; restoring the normal aliases
reconstructs `frame.rs` byte-for-byte to the preceding production source.
These rows stay in the shared matrix's separate `oracle_status: not_applicable`
lane and do not increase Pillow parity totals.

The former `decode_following_vertical_without_chroma` has one caller: the
third child of the closed H4 compositor, always 16x4 with no Full-resolution
edges. Its locally owned state remains 4:2:0, while this child parses and
publishes Monochrome syntax/output because it owns only luma samples.
The new private `decode_following_horizontal_four_without_chroma` keeps sample
checks, geometry consumption, quantization/CDEF bookkeeping, error ordering,
palette rejection, the active H4 reconstruction and byte-identical output
contexts. Four disconnected private helpers and unused edge preparation are
removed after independent caller/reference review. All other surviving
function bodies are byte-identical apart from the sole compositor call.
`block.rs` shrinks by 422 lines, with final SHA-256
`0f18d590882c560b9770503795b3a0c3520dd03c1fbd554bf0ef7930767abb00`.
The shared reconstruction, complete partition decoder and checked H4 fallback
remain intact.

The incremental campaign separately runs all 504 preceding AVIF rows plus
21 preceding fault contracts, then the new parity row and each new fault row.
Every selection passes with no failures or ignored tests. All 4,843 recorded
local file hashes stay unchanged within the campaign. Coverage MCP's LCOV union
estimates 45,192→45,194→45,197→45,200 observed lines out of 131,064: gains of
two, three and three lines, respectively, with zero added or removed
coordinates. Successful extent-index reads at `container.rs:659` and
`samples.rs:1110` are observed; their error-return outcomes remain missing.
The color CDEF allocation error closures at `frame.rs:1993` and `:2010` are
observed in their selected reports. This subset denominator differs from the
full report; the union is limited coordinate evidence and does not establish
full-suite regressions or verified named-test attribution.

The preliminary verifier catches stale current provenance counts in the
roadmap/documentation. They are synchronized to 138 before the successful
final verification. The local full receipt explicitly records the later
claim-ledger integrity refresh; all Rust and test input/reference hashes
remain identical to the incremental campaign. Independent source, input,
registry and numerical reviews have no actionable findings. Proofs and
receipts are retained under
`target/release-evidence/avif-h4-no-chroma-specialization-0899215a/`,
`target/release-evidence/avif-idat-indexed-20261005/` and the corresponding
`avif-indexed-idat-cdef-*` campaign paths.

### Preceding AVIF idat and following-chroma H4 campaign — 2026-10-05

The preceding full local report is
`target/release-evidence/coverage-avif-h4-idat-20261005.json`, generated by
`RUSTC_WRAPPER= CARGO_BUILD_JOBS=2 COVERAGE_REPORT=target/release-evidence/coverage-avif-h4-idat-20261005.json make coverage`
with `nightly-2026-07-16`. It records 88,892/135,927 lines (65.396867%),
17,299/31,196 branches (55.452622%), 4,976/8,692 functions (57.248044%), and
136,037/211,020 regions (64.466401%). All alpha floors pass; the strict report
verifier still fails all four 100% requirements, so that goal remains open.

Every test binary passed, including all 57 coverage-matrix harness tests,
504 AVIF decode rows, the native repetition bundle's 82 cases, all 21
target-only fault contracts, seven decode-policy tests, all 68 feature-gate
tests, determinism, animation, and runtime suites. The ordinary decode/encode
inventory now contains 2,016 rows; previous Pillow observations are unchanged.
The local source receipt confirms unchanged Rust, Cargo and input hashes
throughout this full campaign.

Relative to the preceding full report, covered counts increase by 15 lines,
four branch outcomes and 26 regions, with the function numerator unchanged.
The same-source incremental comparison below isolates the new idat row's
line observations. Source totals decrease by 6,732 lines, 1,038 branches,
388 functions and 9,914 regions after the H4 specialization supported by
caller proof and removal of its disconnected code. These source-total changes are kept
separate from executed coverage; no source exclusions are used.

Coverage MCP reads the report as source-unverified with test attribution
unknown. It retains 74,983 missing regions and 13,897 missing branch outcomes.
The largest groups remain repeated AV1 inter-translation instantiations.
The retained following-H4 fallback still has uncovered outcomes; its
reachability and checked fallback remain intact.

`make fmt`, `make lint`, `make lint-x86-simd`, `make verify`, and all 33 native,
WASI, and WASM feature lanes pass. Lint and feature receipts confirm unchanged
source/input hashes. The origin inventory remains 136 exact `cfg(coverage)`
guards across 20 Rust files. No new Rust unit tests, unsafe code, source
exclusions, ignored cases or lint exceptions are introduced.

### AVIF idat item and following-H4 specialization — 2026-10-05

The ordinary parity row `iloc_idat_primary_item` relocates the pinned baseline's
primary AV1 item into a complete `meta/idat` box. Its `iloc` version-one entry
uses construction method one, data-reference index zero, and nonzero base and
extent offsets of eight bytes each. Both container inspection and sample
extraction must resolve the item relative to the `idat` payload. The sixteen
padding bytes precede the unchanged 2,795-byte AV1 payload; the original
`mdat`, `ftyp`, and non-`iloc` metadata children remain byte-identical. The
5,902-byte complete fixture has SHA-256
`cc020ae1e91a2cad8c4f66b10165702bf11abc39a462a06b9665664f3c63c337`.

Live pinned Pillow 12.2.0 with libavif 1.4.1, dav1d 1.5.3, and libaom 3.13.2
accepts the relocated item with the same complete image and frame observations
as the source. The maintained reference generator produces exact pixels and
operation contracts independently of Rust. Generate or verify the input with
`target/oracle-staging/pillow122/bin/python scripts/generate_avif_idat_fixture.py`;
the normal AVIF asset hook invokes it too. Repeated generation preserves the
same bytes and refuses to replace a differing fixture. The input/reference
generation step leaves all 4,687 preceding nonaggregate fixture files
byte-identical, with previous JSON case observations and ordinary matrix rows
preserved. Subsequent count and integrity-hash synchronization changes only
accounting metadata; historical measured inputs and managed coverage identities
remain unchanged. The AVIF decode inventory grows from 503 to 504; all 21
target-only fault rows retain their separate `oracle_status: not_applicable`
lane.

The H4 refactor uses the following caller invariants. Both calls to the former `decode_following_vertical` belong to one
locally owned `Lossy420Decoder::with_qindex` state, which fixes sampling to
4:2:0. Each receives 16x4 dimensions and no Full-resolution edges. Pending
geometry is consumed by each successful syntax attempt, so both following
leaves have parent `Horizontal16x4` geometry. Encoded TX8x4 or TX4x4 child splits
do not change that parent. The callback can access only the range decoder.
The private entry is now `decode_following_horizontal_four_chroma`.

Sample-depth and geometry checks, quantization/CDEF bookkeeping, syntax/error
sequencing, palette rejection before reconstruction, the active H4
reconstruction, and visible output/context updates are retained. The complete
partition decoder, normalized Full/high-depth path and H4 compositor remain.
The source proof disconnects 63 private helpers together with their unused
aliases, constants, neighbor type and twelve unread fields. Four already
disabled private tests used only removed helpers; they are retired. A mixed
edge test retains its live right-edge assertion. No public parity case is
removed. `block.rs` shrinks by 8,365 lines; the final source SHA-256 is
`c389592f0ba11d479ceb3ff0277b6148d7a3b94093ff7d0449828b0362f586c7`.
No coverage exclusions, unsafe code, ignored cases or new lint exceptions are
introduced.

Structured caller/reference proofs, preserved baseline/candidate source,
lint and feature receipts with source hashes, and the live input/reference receipts
are retained under `target/release-evidence/avif-h4-following-specialization-3e4ab357/`,
`target/release-evidence/avif-idat-primary-20261005/`, and the corresponding
`avif-h4-idat-*` campaign paths.

The incremental campaign runs all 503 preceding AVIF rows and the new row
separately on identical source; both selections pass. Its local receipt confirms
unchanged Rust, Cargo and input hashes. Coverage MCP's LCOV union estimates
43,755→43,770 observed lines out of 131,419, a fifteen-line gain
(0.011414 percentage points), with zero added or removed coordinates. The new
row observes 14,754 lines; 14,739 overlap the preceding subset. The previously
missing Idat selection at `container.rs:642` and relative-span calculation at
`samples.rs:1124` are observed in its selected LLVM report. This subset
denominator differs from the full campaign. MCP still reports source/build
identity unverified and test attribution unknown; the local command/hash
receipt does not create managed evidence or establish full-suite regressions.

### Preceding Full-chroma H4 cleanup campaign — 2026-10-05

The preceding full local report is
`target/release-evidence/coverage-avif-h16x4-cleanup-20261005.json`, generated by
`RUSTC_WRAPPER= CARGO_BUILD_JOBS=2 COVERAGE_REPORT=target/release-evidence/coverage-avif-h16x4-cleanup-20261005.json make coverage`
with `nightly-2026-07-16`. It records 88,877/142,659 lines (62.300311%),
17,295/32,234 branches (53.654526%), 4,976/9,080 functions (54.801762%), and
136,011/220,934 regions (61.561824%). All alpha floors pass; the strict report
verifier still fails all four 100% requirements, so that goal remains open.

Every test binary passed, including all 57 coverage-matrix harness tests,
503 AVIF decode rows, the native repetition bundle's 82 cases, all 21
target-only fault contracts, seven decode-policy tests, all 68 feature-gate
tests, determinism, animation, and runtime suites. The 2,015 ordinary
decode/encode rows and their Pillow references are unchanged.

The covered numerators are identical to the preceding full campaign. The
caller-proven dead-code cleanup removes 457 reported lines, 84 branches,
15 functions, and 713 regions from the source totals; no coverage exclusion
or synthetic case produces this change. Coverage MCP reads the report as
source-unverified with test attribution unknown. Its totals retain 84,923
missing regions and 14,939 missing branch outcomes. Uncovered paths in the
retained vertical fallback remain open.

`make fmt`, `make lint`, `make lint-x86-simd`, `make verify`, and all 33 native,
WASI, and WASM feature lanes pass. The origin inventory remains 136 exact
`cfg(coverage)` guards across 20 Rust files. No Rust unit tests, unsafe code,
source exclusions, or new lint exceptions are introduced by the cleanup.

### Preceding CDEF fault-contract campaign — 2026-10-05

The preceding full local report is
`target/release-evidence/coverage-cdef-region-map-20261005.json`, generated by
`RUSTC_WRAPPER= COVERAGE_REPORT=target/release-evidence/coverage-cdef-region-map-20261005.json make coverage`
with `nightly-2026-07-16`. It records 88,877/143,116 lines (62.101372%),
17,295/32,318 branches (53.515069%), 4,976/9,095 functions (54.711380%), and
136,011/221,647 regions (61.363790%). All alpha floors pass; the strict report
verifier still fails all four 100% requirements, so that goal remains open.

Every test binary passed, including all 57 coverage-matrix harness tests,
503 AVIF decode rows, the native repetition bundle's 82 cases, all 21
target-only fault contracts, seven decode-policy tests, all 68 feature-gate
tests, determinism, animation, and runtime suites. The 2,015 ordinary
decode/encode rows and their Pillow references are unchanged by this slice.
Fault results remain a separate `oracle_status: not_applicable` lane.

Coverage MCP reads this full report as source-unverified with test attribution
unknown. It reports 85,636 missing regions and 15,023 missing branch outcomes.
The monochrome CDEF region-map allocation error closure is now covered.
The same-source incremental line comparison below isolates the new case's
coordinate observations; full-report denominator changes are reported intact.

`make fmt`, `make lint`, `make lint-x86-simd`, `make verify`, and all 33 native,
WASI, and WASM feature lanes pass. The origin inventory records 136 exact
`cfg(coverage)` guards across 20 Rust files. No source exclusions, Rust unit
tests, unsafe code, or lint exceptions were added in this slice.

### Preceding AVIF edge campaign — 2026-10-05

The preceding full local report is
`target/release-evidence/coverage-avif-edges-20261005.json`, generated by
`RUSTC_WRAPPER= COVERAGE_REPORT=target/release-evidence/coverage-avif-edges-20261005.json make coverage`
with `nightly-2026-07-16`. It records 88,864/143,107 lines (62.096194%),
17,293/32,316 branches (53.512192%), 4,975/9,095 functions (54.700385%), and
136,002/221,642 regions (61.361114%). All alpha floors pass (59%, 46%, 52%,
and 58% respectively). The strict report verifier still fails all four 100%
requirements; that goal remains open.

Every test binary passed, including all 57 coverage-matrix harness tests,
503/503 AVIF decode rows, the native repetition bundle's 82 complete cases,
all twenty target-only fault contracts, seven decode-policy tests, determinism,
all 68 feature-gate tests, animation, and runtime suites. The matrix contains
2,015 decode/encode rows. The unchanged fault contracts remain a separate
lane with `oracle_status: not_applicable`, outside Pillow parity counts.
The initial full run caught the native repetition harness's stale 232-artifact
inventory; its exact inventory now includes the new sequence and all 235
artifact hashes, and the repeated full campaign passes.

Coverage MCP reads the final report as source-unverified with test attribution
unknown. It reports 85,640 missing regions and 15,023 missing branch outcomes.
The largest groups remain AV1 inter-translation and vertical decode
instantiations in `block.rs`. Repeated generic instantiations dominate; these
coordinates are leads for source review, not direct fixture prescriptions.
The final same-source incremental comparison is recorded below. Source and
harness edits changed full-report denominators, so differences from the
preceding full report are not an isolated execution gain.

`make lint`, `make lint-x86-simd`, `make fmt`, `make verify`, and all 33 native,
WASI, and WASM feature lanes pass. The origin inventory remains 135 exact
`cfg(coverage)` guards across 20 Rust files, with no source exclusions.
No new Rust unit tests, unsafe code, or lint exceptions are introduced.

### Preceding complete-source campaign — 2026-10-05

The preceding full local report is
`target/release-evidence/coverage-current-source-accounting-20261005.json`,
generated by
`RUSTC_WRAPPER= COVERAGE_REPORT=target/release-evidence/coverage-current-source-accounting-20261005.json make coverage`
with `nightly-2026-07-16`. It records 88,864/143,127 lines (62.087517%),
17,279/32,344 branches (53.422582%), 4,974/9,095 functions (54.689390%), and
136,005/221,676 regions (61.353056%). The project alpha floors pass (59% lines,
46% branches, 52% functions, 58% regions), while the separate 100% requirement
still fails and remains open.

All test binaries in the campaign passed, including all 57 coverage-matrix
harness tests, seven decode-policy tests, determinism, all 68 feature-gate
tests, animation, and runtime suites. The shared matrix contains 1,995
decode/encode rows and twenty target-only AVIF/JPEG fault contracts in a
separate lane with `oracle_status: not_applicable`; all twenty contracts
passed. The full AVIF decode matrix passed 483/483, including both lossy 4:2:0
four-Square8 parity cases, the skipped color/alpha CDEF case, and the 33×17
I422 super-resolution case's Pillow odd-width right-edge chroma behavior.

Coverage MCP reads the full LLVM report as source-unverified with test
attribution unknown because it carries no source/build receipts. Its
region and branch totals report 85,671 missing regions and 15,065 missing
branch observations. The largest groups are AV1 inter-translation and vertical
decode instantiations in `block.rs`; repeated generic instantiations dominate
the result, so the coordinates are leads for source review rather than a direct
case prescription. The targeted branch scan still reports missing outcomes in
`Lossy420Decoder::decode_following_vertical` (`block.rs:67556`); neither lossy
Square8 parity row adds observations there. The `frame.rs` branch scan also
reports 35 missing outcomes in `assemble_monochrome_tiles`, including
inconsistent or absent per-tile filter metadata paths. MCP does not attribute
observations to tests. The full report includes all twenty fault-contract
rows; source/build receipt-bound attribution remains unavailable.

The current coverage-origin inventory records 135 exact `cfg(coverage)`
guards across 20 Rust files. All source `coverage(off)` attributes have been
removed. Rust's strict Clippy policy, coverage-hook Clippy configuration, and
x86 SSE2/AVX2 lint targets pass without new lint exceptions. Target fault
injection remains compiled only for coverage builds.

### AVIF configuration precedence and film-grain edges — 2026-10-05

Twenty new complete-file manifest cases use live pinned Pillow 12.2.0,
libavif 1.4.1, dav1d 1.5.3, and libaom 3.13.2. Fourteen succeed completely,
five fail during opening or first-frame decoding, and one succeeds on frame
zero before rejecting an unlisted grain reference in frame one. The ordinary
public runner executes all twenty successfully after the fixes. These are
Pillow parity inputs; the twenty target-only fault contracts remain a
separate unchanged lane.

The configuration generator preserves BMFF lengths, item extents, and AV1
media bytes while changing redundant declarations or a later duplicate
property. Seven cases establish that decoded sequence fields take precedence
over differing profile, level, tier, monochrome, subsampling, and chroma
position declarations. Two depth disagreements between the selected `pixi`
and `av1C` properties require opening errors. Two later duplicate `pixi`
properties retain the first property's precedence. Two matching ten/twelve-bit
container declarations retain the actual eight-bit sequence pixels, and one
accepted twelve-bit declaration has `highBitdepth` clear. Declared depth
metadata remains independently identified; decoded storage and conversion
use the validated AV1 sequence depth.

Before fixes, the original nine configuration cases failed, both duplicate
property cases failed, both declared-versus-sequence depth cases failed, and
the tolerated twelve-bit flag case failed. Rust now validates the selected
first `pixi` against the cached `av1C` depth and follows libavif's twelve-bit
flag precedence. The redundant configuration/sequence equality check is
removed. Bounded container framing, actual AV1 syntax, repeated sequence
consistency, sample bounds, and checked plane/conversion validation remain.
No additional pixel allocation or repeated configuration payload parse is
introduced.

The grain generator adds exact I422 pixels, Y-point-count 15 and UV-count 11
rejections, unequal I420 U/V presence, an unlisted reference rejection, and
valid reuse of an inactive reference. The last case reproduced a sequence
decode failure before the fix. A listed, existing reference with inactive
grain now inherits inactivity; empty or unlisted references still fail.
`Option<FilmGrain>::None` retains its documented meaning of `apply_grain=false`.
The independent header inspector follows the same native semantics. The
refreshed native repetition bundle contains 82 cases; all 81 preceding case
records are unchanged, with one new inactive-reference sequence observation.

Reproduce inputs with
`target/oracle-staging/pillow122/bin/python scripts/generate_avif_config_disagreement_fixtures.py`
and
`target/oracle-staging/pillow122/bin/python scripts/generate_avif_filmgrain_edge_fixtures.py`.
The existing asset-generation hook invokes both. Exact outputs and operation
contracts are generated through `scripts/generate_decode_refs.py --format avif`;
no expected pixels or failures are copied from Rust.

The incremental campaign executes the preceding 483 AVIF rows and the twenty
new rows in separate runs on identical final source, features, and LLVM flags.
Both subsets pass and their local receipt confirms unchanged Rust inputs and
matrix hashes throughout. Coverage MCP's LCOV coordinate union reports
43,734→43,749 observed lines out of 138,301: fifteen additional lines, with no
source coordinates added or removed. The twenty-row subset observes 22,283
lines; 22,268 were already observed by the old AVIF subset. The I422 grain
helper has seven missing regions in the baseline and none in the new subset.
This subset denominator differs from the full all-test campaign.

Coverage MCP labels both raw reports source-unverified and test attribution
unknown. The local receipt supports this same-source comparison but does not
turn it into managed MCP evidence or a full-suite regression result. JSON
branch/region coordinates are useful for review; no numeric incremental branch
or region union is claimed. Reports are `avif-edges-baseline-20261005.lcov` and
`avif-edges-selected-20261005.lcov`, with companion JSON reports,
`avif-edges-receipt-20261005.json`, and
`coverage-mcp-avif-edges-final-20261005.json`, under `target/release-evidence/`.

Local red/green logs: `config-disagreement-red-20261005.log`,
`avif-15-red-20261005.log`, `duplicate-pixi-red-20261005.log`,
`sequence-depth-red-20261005.log`, `depth-flags-red-20261005.log`, and
`avif-20-green-20261005.log`, all under `target/release-evidence/`.

### Rejected JPEG sparse chroma masks — 2026-10-05

A separate worktree at `d5ddebbe83d067be16f49eb13c4f29603faa55d5`
implemented fused chroma nonzero masks and a separate sparse Cb/Cr entropy
loop for complete I420 MCUs without low-quality trimming. The prototype is
rejected and reverted; the main encoder remains unchanged. Its exact source
and patch are preserved locally for review of the rejected prototype.

The ARM64 macOS 15.7.7 campaign used rustc 1.96.1, ordinary Cargo release
builds, and the pinned TurboJPEG 3.2.0 oracle. No LTO, target-cpu, PGO, or
TurboJPEG SIMD disabling was added. Single-threaded complete public calls use
the existing twenty configurations, forty encode/decode workloads, and Rust
harness's 100-call warmup; per-invocation iteration counts and timings are
retained. Five rounds alternate baseline/candidate order, with the unchanged
oracle following each variant. All 800 raw records retain commands, timing,
output hashes, order, and background-load observations. Source, binary, and
oracle hashes remained unchanged. CPU brand/model queries were unavailable;
recorded background load was nonzero, so this is a scoped host observation.

All seven admitted encode configurations regress in every paired round.
The table gives medians of paired candidate/baseline time ratios; values above
one are slower. Oracle normalization retains the same regression decision.

| Encode workload | Candidate / baseline | Oracle-normalized ratio |
| --- | ---: | ---: |
| 32×32 Q85 I420 | 1.1585 | 1.1587 |
| 128×128 Q50 I420 | 1.0494 | 1.0413 |
| 128×128 Q85 I420 | 1.1222 | 1.1243 |
| 128×128 Q100 I420 | 1.2138 | 1.1838 |
| 512×512 Q85 I420 | 1.2125 | 1.2083 |
| 1024×1024 Q85 I420 | 1.2366 | 1.2138 |
| 128×128 Q85 I420, restart interval 4 | 1.1085 | 1.1325 |

Controls varied: odd 63×65 encoding was 0.9805, I444 encoding 1.0008,
512×512 Q10 encoding 1.0927, and 1024×1024 decoding 0.9928. These controls
are retained alongside the regressions. Mask-construction overhead and lost
six-chain instruction overlap are plausible causes, without stage-level
proof. No encoder, decoder, SSE2, AVX2, or universal speedup is claimed.

Correctness still passed: emitted JPEG bytes matched the baseline for all
twenty configurations; the existing public manifest passed 81/81 encode rows
(379 calls, zero panics) and 205/205 decode rows; strict all-feature/all-target
Clippy passed.

The preserved candidate source SHA-256 is
`283be8d25af57b67396f3a08742586532c539593cbb5095cd6b0dcc78e912321`;
patch SHA-256 is
`b541f59801c0a84d7ae4510ab292943a28779fc3046661896b9543a247558556`.
Local evidence is under `target/release-evidence/jpeg-sparse-chroma-d5ddebbe/`:
`rejection.json`, `candidate-source.rs`, `candidate.patch`, build/check logs,
and `paired-five-rounds/{metadata.json,raw.jsonl,summary.json,summary.csv,exact-encode-equality.json}`.
These local artifacts are not shipped release benchmark results. The next
candidate must preserve instruction overlap or identify a different measured
cost, then pass the same correctness and complete-call measurement gates.

### Rejected JPEG bounded entropy arithmetic — 2026-10-05

An isolated candidate based on `e070f9cef79005c5b832ebb78db0fee3eb192cf6`
replaced three saturating operations with locally bounded arithmetic: available
reservoir space, the post-capacity-check word count, and the AC zero run.
The source-bound ARM64 object showed saturation selects for these expressions,
but did not quantify their share of elapsed time. Six-chain scheduling,
quantization, storage, and output contracts were unchanged.

Two whole-call campaigns each completed forty workloads and five balanced
rounds, with the second reversing the starting order: 1,600 raw records in
total. Both retained exact equality for all twenty encoded benchmark JPEGs.
Strict Clippy and the public Pillow manifest passed 81/81 encode rows and
205/205 decode rows. The ordinary release builds used Rust 1.96.1 on macOS
15.7.7 ARM64 and the pinned TurboJPEG 3.2.0 timing oracle, with single-threaded
operation loops. CPU brand/model and memory queries were unavailable; nonzero
background load is recorded in each campaign. Inputs, source,
executables, and oracle hashes stayed unchanged during each campaign.

Each table value is the median of five paired round ratios. Candidate/baseline
ratios below one mean lower elapsed time. Normalized ratios also divide each
target measurement by its following oracle measurement.

| RGB I420 encode workload | First ratio | Reversed ratio | Normalized first / reversed |
| --- | ---: | ---: | ---: |
| 63×65 Q85 | 1.0092 | 1.0122 | 1.0067 / 1.0095 |
| 128×128 Q100 | 1.0214 | 1.0135 | 1.0198 / 1.0337 |
| 512×512 Q85 | 1.0169 | 1.0048 | 1.0122 / 0.9981 |
| 1024×1024 Q85 | 1.0167 | 1.0131 | 0.9955 / 1.0129 |
| 512×512 Q10 | 0.9852 | 0.9479 | 0.9839 / 0.9511 |

Regular RGB inputs repeatedly regressed despite gains on some low-quality and
CMYK inputs. Decode controls also varied. The combined candidate is rejected;
these results do not isolate an individual expression's cost or establish an
SSE2/AVX2 result. The encoder was restored byte for byte, and the clean managed
worktree was archived after independent preservation checks. No candidate
source reached main.

The retained bundle `target/release-evidence/jpeg-bounded-entropy-e070f9ce/`
contains both executables, source and patch snapshots, harness files, inputs,
both complete campaigns, `comparison.json`, `comparison.csv`, `rejection.json`,
and `preservation.json`. Candidate source SHA-256:
`71942749a3545ed9323573abc8a60c9c9a59ca6a9234f8e992798ed3a375c36f`;
restored encoder SHA-256:
`5451cb6570679fe9ea13b385900e5270581220e35cf1993c09eeb0b1a799afab`.

### SSE2/AVX2 runtime evidence and JPEG profile — 2026-10-05

At pushed revision `d5ddebbe83d067be16f49eb13c4f29603faa55d5`,
[CI run 37263569622](https://github.com/appunni-m/image-slash-star/actions/runs/37263569622)
completed successfully. Both SSE2 and AVX2 public JPEG/AVIF encode/decode
parity lanes passed with branch coverage; aggregate coverage floors, strict
lint, feature/target checks, and dependency checks also passed. The x86
coverage artifact retains eight reports. Their numerical totals have not been
inspected locally, so this establishes runtime parity and passed floors.

[Benchmark run 37263569657](https://github.com/appunni-m/image-slash-star/actions/runs/37263569657)
also passed five rounds each of SSE2 and AVX2 on the same Ubuntu host, covering
20 configurations and 40 workloads. All 40 comparisons retained equal output
lengths and hashes. Downloading its timing artifact requires authenticated
access (HTTP 401), so no relative speed or speedup is claimed from this run.

A source-bound ARM64 profile at that revision sampled the complete 1024×1024
RGB Q85 I420 encode call for 12 seconds. Its receipt records source hashes,
binary SHA-256, build flags, and the owned process; the source remained
unchanged through sampling. Streaming encoding represented 84.56% of observed
top-of-stack samples, RGB conversion 9.11%, and external FDCT 6.20%. Exact
binary/object/DWARF matching maps representative streaming addresses to Cb AC
coefficient reads, zero-run handling, and the bit reservoir. Collapsed samples
prevent assigning a precise entropy fraction. Sampling is not a timing
benchmark.

The profile motivated collecting sparse chroma AC masks during quantization
and enumerating their zigzag positions. The experiment above rejects that
prototype after complete-call measurement. Any future implementation must
preserve ZRL/EOB, DC prediction, byte stuffing, and restart semantics, then
pass exact public parity and balanced complete-call measurements. Dense-image
mask overhead is part of the measured rejection above; no optimization is
retained from this profile. Halving the measured conversion bucket would permit approximately
1.048× overall speedup, while the sparse-scan candidate's affected fraction is
unknown.

Local receipts: `target/release-evidence/jpeg-profile-d5ddebbe/receipt.json`,
`analysis.md`, `analysis-offset-mapping.json`, and `analysis-ci-terminal.json`;
remote benchmark metadata: `target/release-evidence/simd-remote-d5ddebbe/`.

### Unreachable AVIF Full-chroma Horizontal16x4 cleanup — 2026-10-05

Both current callers of `Lossy420Decoder::decode_following_vertical` belong to
the H4 composer. Each receives a local `with_qindex` state initialized to
`Subsampled420`, dimensions `(16, 4)`, and absent full-edge state. Intervening
methods do not change sampling; the callback receives only the range decoder.
The sole Full-sampling setter, `decode_origin_full`, is outside this local path.
The Full-chroma `Horizontal16x4` predicate therefore cannot succeed for any
current caller, independently of coverage observations.

The removed cluster consists of that reconstruction arm,
`reconstruct_following_lossy_full_vertical_16x4_leaf`,
`reconstruct_lossy_full_16x4_chroma_normalized_target`, and their exclusively
owned `right_edge_4_for_16x4` accessor. It removes 580 source lines and three
separator blanks. Exact comparison against
`f46efa4b3eee82b9c33e5cd3fe1911a7ad2d16f5` confirms only those four ranges were
deleted: every retained byte is unchanged. The live 4:2:0 arm, entire vertical
decoder, H4 composer, and their safety checks remain. This proof does not
justify deleting the complete fallback method.

The source proof is retained in
`target/release-evidence/avif-vertical-h16x4-cleanup-f46efa4b/proof.json`.
No cfg guard or source coverage exclusion changed.
The full all-feature campaign, strict debug/release/coverage and x86 SIMD
Clippy gates, formatting, static verification, and all 33 feature lanes pass.
Ordinary Pillow rows and target-only fault rows are unchanged. This source
cleanup is not a measured AVIF throughput improvement.

### Complete source accounting and unreachable Square8 cleanup — 2026-10-05

The preceding full report still contained 42 source `coverage(off)` attributes.
This cleanup removes all 42, including defensive production checks, and the
unused nightly coverage-attribute feature gates. `make verify` now rejects
source coverage exclusions in Rust files under `src` and `tests`, preventing
those attributes from silently shrinking later denominators.

Source review proves the older lossy four-Square8 fallback is unreachable:
public root contexts retain level zero or one; its exact frame class is absent
from `closed_class` admission, and the alternative vertical-pair admission
requires context level three. The complete frame decoder already handles the
two public Square8 inputs. The obsolete predicate, dispatch branch, and
composer are deleted, removing 208 lines and one coverage guard. Shared
vertical helpers and the H4 callers remain.

The subsequent full campaign passes all public parity and twenty target-only
fault-contract rows. `make lint`, `make lint-x86-simd`, and all 33 feature/target
lanes also pass. Coverage is rebaselined at the four totals above; source
accounting and deletion changed the denominators, so their difference from the
previous report is not an incremental execution gain. The 100% target remains
open. These edits introduce no codec optimization or new runtime speed claim.

The x86 CI benchmark lane also gains an explicit Linux POSIX feature macro
for its strict C11 timing harness. The native strict C build and a timing smoke
invocation pass on ARM64 macOS; Linux compilation and SSE2/AVX2 runtime results
remain pending the pushed workflow. The fixture encoder patch retains its
semantics after whitespace normalization; reverse application checks pass
against the pinned patched libavif tree.

Report: `target/release-evidence/coverage-current-source-accounting-20261005.json`.

### AVIF skipped CDEF color/alpha public parity — 2026-10-05

`decode:avif:multitile_skipped_cdef_color_alpha` is a complete 128×64 RGBA
input with two tile columns in each color and monochrome-alpha item. The
independent libaom default-CDF writer selects unpartitioned 64×64 DC blocks
with skipped transforms. Frame headers retain enabled CDEF syntax with zero
strengths, while skipped blocks omit the region index. Pinned Pillow 12.2.0
accepts the file and supplies exact pixels; the public matrix runner passes
all declared operations. No target fault injection or Rust implementation
change is involved.

The new one-case selection passes against the same 481-case AVIF baseline
used for the gradient below. Coverage MCP's incremental LCOV line comparison,
including the gradient as a previously accepted batch, observes 20 newly
covered coordinates and 11,454 overlapping coordinates. The estimated gain is
0.014490 percentage points over the 138,027-line selected-report denominator.
The gains include the inactive-block and absent-region-index skips in both
color and monochrome tile assembly (`frame.rs:2063,2099,2411,2462`).
Missing source/build receipts and test attribution limit this to coordinate
evidence, without a full-suite regression claim. This ordinary-input case
belongs to Pillow parity; the twenty typed target-only fault contracts remain
a separate outcome lane.

The subsequent full campaign passes all tests and records 18 additional
covered lines, 24 branches, zero functions, and 24 regions with unchanged
denominators relative to the preceding full report. MCP's full-report branch
comparison remains limited by unverified source/build identity; its `frame.rs`
filter observes seven newly covered outcomes, including all four targeted
assembly skips. The current report records both outcomes at each target:
color active-map 256/4,352, color region-index 68/4, monochrome active-map
256/1,536, and monochrome region-index 24/4. These are report counts, not
receipt-bound per-test attribution. `make fmt`, `make verify`, `make lint`,
and `make lint-x86-simd` also pass. The native fixture writer now compiles with
`-Wall -Wextra -Werror`; both generator profiles reproduce their pinned hashes.
The SSE2/AVX2 gates are cross-compilation checks on this ARM64 host, with no
new runtime speed measurement. The fixture additions change no codec hot path.

Reports:

- `target/release-evidence/coverage-avif-existing-minus-square8-gradient-20261005.info`
- `target/release-evidence/coverage-avif-square8-gradient-new-20261005.info`
- `target/release-evidence/coverage-avif-skipped-cdef-color-alpha-new-20261005.info`
- `target/release-evidence/coverage-current-skipped-cdef-20261005.json`

### AVIF lossy Square8 gradient public parity — 2026-10-05

`decode:avif:portable_lossy_420_square8_gradient` adds a 16×16 full-range
quality-99 RGB gradient with four terminal Square8 leaves and disabled
screen-content tools. The pinned libavif 1.4.1/libaom 3.13.2 generator produces
identical repeated encodes; pinned Pillow 12.2.0 supplies the exact RGB
reference. The public manifest runner passes all declared operations.

The focused LLVM report records three complete lossy-partition decodes,
twelve streamed intra leaf decodes, and nine following-leaf decodes. It records
zero hits in `decode_four_lossy_420_leaves` and
`Lossy420Decoder::decode_following_vertical`. The complete frame decoder handles
this input before the legacy fallback. Source review also finds that the
fallback's `closed_class` admission omits the lossy square-split predicate;
the public root context has level zero or one, while its vertical-pair
alternative requires level three. This case therefore protects the complete
frame's public pixels without providing a witness for the older square helper.

Both the 481-case existing AVIF selection and the new one-case selection pass.
Coverage MCP cannot union these LLVM JSON details with the full report because
the detailed observations do not reconcile with the reported totals. Matching
LCOV selections instead provide an unverified incremental line estimate:
zero newly covered lines, with all 12,177 new-selection line coordinates
already covered by the 138,027-line existing-selection inventory. The case is
retained for its distinct public gradient regression contract. Missing
source/build receipts and test attribution limit the comparison to coordinate
evidence; it makes no full-suite regression claim.

Reports:

- `target/release-evidence/coverage-avif-square8-gradient-new-20261005.json`
- `target/release-evidence/coverage-avif-existing-minus-square8-gradient-20261005.info`
- `target/release-evidence/coverage-avif-square8-gradient-new-20261005.info`

### AVIF monochrome CDEF region-map reservation fault contract — 2026-10-05

`fault-contract:avif:avif_monochrome_cdef_region_map_reservation_failure`
reuses the existing `decode:avif:multitile_monochrome_split_groups` complete
128×64 Pillow-success input, SHA-256
`57490ef5f80626a3218b96cae9f3e9601ff9db685fc608cd02ad39213638b24c`.
The ordinary input requests two CDEF region slots. It cannot deterministically
choose allocator failure, and Pillow cannot receive the target's injection.

The coverage-only point makes the existing `try_reserve_exact` request
`usize::MAX`, causing its real capacity failure before resizing the map.
The runner requires the public AVIF `StillDecode` dimensions error and exact
message `decode: AVIF AV1 validation failed: unable to allocate assembled monochrome CDEF map`,
then requires a successful retry equal to a fresh uninjected decode. The row
uses the existing `decode_error_then_retry_succeeds` contract and
structured diagnostics requirement, with `oracle_status: not_applicable`.
Production builds keep the original reservation count and error path.

The twenty existing contracts and the new one-case selection each pass.
Their local receipt confirms unchanged source and Cargo inputs between runs.
Coverage MCP's matching LCOV comparison observes five newly covered lines,
30,769→30,774/138,310, a 0.003615 percentage-point incremental estimate.
The selected LLVM region scan finds no remaining gap in the allocation error
closure. MCP still labels source/build identity unverified and test attribution
unknown; the coordinate union does not establish a full-suite coverage result.
The added hook brings the origin inventory to 136 exact `cfg(coverage)` guards
across 20 Rust files, without source exclusions.

Reports:

- `target/release-evidence/cdef-region-map-baseline-twenty-20261005.lcov`
- `target/release-evidence/cdef-region-map-selected-one-20261005.lcov`
- `target/release-evidence/cdef-region-map-selected-one-20261005.json`
- `target/release-evidence/cdef-region-map-incremental-receipt-20261005.json`
- `target/release-evidence/coverage-mcp-cdef-region-map-incremental-20261005.json`

### AVIF monochrome CDEF active-map reservation fault contract — 2026-10-05

`fault-contract:avif:avif_monochrome_cdef_active_map_reservation_failure`
reuses the active `decode:avif:multitile_monochrome_split_groups` Pillow
stimulus, which enables CDEF for a split monochrome frame. Its coverage-only
failpoint targets the later `cdef_active.try_reserve_exact` allocation, after
the CDEF region map and loop-filter metadata reservations. The public AVIF
`StillDecode` dimensions error is checked, then a fresh retry must match an
uninjected decode. The row uses `oracle_status: not_applicable`; the injected
failure remains outside Pillow parity counts.

The new fault case and the nineteen-case baseline each passed through
`test_fault_contract_matrix`. Coverage MCP's LCOV incremental line comparison
observed five newly covered lines in `frame.rs` and estimated a 0.003622
percentage-point gain over the 138,027-line selected-report denominator.
MCP marks the estimate limited because source/build receipts and test
attribution are missing. The branch comparison is incomparable because exact
branch detail is unavailable, so this is selected-run coordinate evidence,
not a full-suite coverage claim.

Reports:

- `target/release-evidence/coverage-fault-contracts-existing-nineteen-minus-cdef-20261005.info`
- `target/release-evidence/coverage-fault-contract-monochrome-cdef-active-map-new-20261005.info`
- `target/release-evidence/coverage-current-fault-contracts-20261005-cdef-active-map.json`

### AVIF grid and monochrome assembly fault contracts — 2026-10-05

Two target-only rows use existing active Pillow parity inputs as reproducible
stimulus. `fault-contract:avif:avif_grid_cell_reservation_failure` uses
`decode:avif:grid`, an 80×80 color grid with per-cell alpha. Grid assembly now
uses fallible `try_reserve` instead of `Vec::with_capacity`, preserving the
normal reservation size while returning the public AVIF `StillDecode`
dimensions error on allocation failure. Its coverage-only failpoint forces
that error, and the runner confirms a fresh retry matches an uninjected decode.

`fault-contract:avif:avif_monochrome_loop_filter_metadata_reservation_failure`
uses `decode:avif:multitile_monochrome_split_groups`. It forces the assembled
monochrome loop-filter metadata reservation to fail, checks the public
`StillDecode` dimensions error, and verifies retry output against a fresh
decode. Both rows use `oracle_status: not_applicable`; their injected outcomes
remain separate from Pillow parity counts.

The 17-case existing-fault baseline and the two-case new selection each passed
through `test_fault_contract_matrix`. Coverage MCP's LCOV incremental line
comparison observed 923 new line coordinates, a 0.668744 percentage-point
gain over the 138,020-line selected-report denominator; 15,044 coordinates
were already covered. MCP marks this estimate limited because the reports
have no source/build receipts or test attribution. Its branch comparison was
incomparable because exact branch detail was unavailable, so these figures
are selected-run coordinate evidence, not full-suite coverage claims.

Reports:

- `target/release-evidence/coverage-fault-contracts-seventeen-baseline-20261005.info`
- `target/release-evidence/coverage-fault-contracts-two-new-20261005.info`

### AV1 projected temporal-field reservation fault contract — 2026-10-05

The target-only row
`fault-contract:avif:avif_projected_temporal_field_reservation_failure` reuses
`decode:avif:animated_motion_chroma`, whose inter frames enable reference-frame
MV projection. Its `cfg(coverage)` failpoint forces
`ProjectedTemporalField::new`'s reservation to fail and asserts the exact AVIF
`SequenceDecode` dimensions diagnostic, then verifies a fresh retry matches an
uninjected sequence decode. The new contract passed 1/1; the existing sixteen
contracts passed 16/16 in the selected baseline. Its oracle is
`not_applicable`, so it remains outside Pillow parity totals.

Coverage MCP compared the selected LCOV reports in incremental line mode: four
newly observed coordinates (0.002898 percentage points), with 19,478 already
covered. In the sixteen-case baseline, the allocation-error mapping at
`motion.rs:2999` was missing; it is observed by the new contract. MCP marks the
estimate limited because source/build receipts and test attribution are
unavailable.

Reports:

- `target/release-evidence/coverage-fault-contracts-existing-sixteen-20261005.info`
- `target/release-evidence/coverage-fault-contract-projected-field-new-20261005.info`
- `target/release-evidence/coverage-full-active-goal-20261005-projected-field.json`

The public row
`decode:avif:coverage_lossy_420_square8_four_leaves_01` pins a lossy 4:2:0
16×16 AVIF with a four-Square8 AV1 partition trace and exact Pillow RGB output.
It passed 1/1, and its same-target baseline comparison against the other 480
active AVIF decode rows found zero newly covered lines. The frame's
`allow_screen_content_tools` flag fails the specialized closed-frame gate, so
this row is retained as a public parity regression only; it is not claimed as
coverage for `decode_four_lossy_420_leaves` or `decode_following_vertical`.
MCP's selected report remains source-unverified and test attribution unknown.

Reports:

- `target/release-evidence/coverage-avif-existing-480-20261005.info`
- `target/release-evidence/coverage-avif-lossy-square8-new-20261005.info`

### AV1 tile assembly and partition-node reservation fault contracts — 2026-10-05

Two target-only cases exercise deterministic AV1 allocation failures through
the shared fault-contract runner. The assembled loop-filter metadata contract
reuses `decode:avif:multitile_color_split_groups`; the partition-node contract
reuses `decode:avif:baseline`. Each case checks the public AVIF `StillDecode`
dimensions error for its named allocation failure, then verifies that a fresh
decode retry matches the clean result. Both use
`oracle_status: not_applicable` and remain excluded from Pillow parity counts.

The selected runner passed both new contracts (2/2); a same-source selected
baseline run passed the other fourteen contracts (14/14). Coverage MCP compared
their LCOV reports in incremental line mode: 7 newly observed line coordinates,
14,410 already covered, and a limited 0.005072 percentage-point estimate. MCP
marks the comparison limited because source/build receipts and test
attribution are unavailable. Its selected report has no remaining line gaps in
`PartitionWalker::push`, and no region gap at the assembled metadata error
mapping or branch gap at its injection check. The rest of
`assemble_color_tiles` retains unrelated uncovered regions.

Reports:

- `target/release-evidence/coverage-fault-contracts-existing-fourteen-20261005.info`
- `target/release-evidence/coverage-fault-contracts-new-two-20261005.info`
- `target/release-evidence/coverage-fault-contract-assembly-partition-20261005-confirmed.json`

### AV1 tile block metadata reservation fault contract — 2026-10-05

The target-only row
`fault-contract:avif:avif_tile_block_metadata_reservation_failure` reuses the
active `decode:avif:baseline` input. Its coverage-only failpoint forces the
per-tile decoded-block metadata reservation to fail and checks the public AVIF
`StillDecode` dimensions error, then verifies that an uninjected retry matches
a clean decode. The selected contract passed 1/1; all twelve earlier fault
contracts also passed in their selected batch, and the full run passed all
thirteen. Its oracle status is `not_applicable`, so it remains separate from
Pillow parity counts.

Coverage MCP's same-target LCOV incremental comparison starts from the
56-test coverage-matrix baseline without its fault-contract runner, then unions
the twelve existing fault contracts and this new row. It reports four newly
observed line coordinates (0.002899 percentage points) in `tile_state.rs`.
MCP marks the result limited because the reports have no source/build receipts
or test attribution. The full all-feature report has no remaining region gap
at the new reservation error path.

### AV1 chroma tile-cell reservation fault contract — 2026-10-05

The target-only row
`fault-contract:avif:avif_chroma_tile_cell_reservation_failure` reuses the
active `decode:avif:baseline` input. Its coverage-only failpoint runs after the
luma cell allocation succeeds and forces chroma-cell reservation to fail. The
contract checks the distinct public AVIF `StillDecode` dimensions diagnostic,
then verifies an uninjected retry matches a clean decode. The selected case
passed 1/1; its oracle status is `not_applicable` and it does not contribute to
Pillow parity counts.

The same-target LCOV incremental comparison adds this row after the thirteen
existing fault contracts and reports three newly observed line coordinates
(0.002174 percentage points) in `tile_state.rs`. MCP marks the comparison
limited because the local reports have no source/build receipts or test
attribution. The full report has no missing branch at the injection check and
no remaining region at the public error mapping.

### AV1 film-grain parity and restoration fault contracts — 2026-10-05

The public parity row `decode:avif:animated_filmgrain_reference_reuse_i444_64x64`
uses a two-frame full-resolution I444 sequence. The first key frame refreshes
film-grain parameters into reference slot 0; the second displayed inter frame
sets `update=false`, reuses slot 0, and supplies a new seed. Pinned Pillow
12.2.0/libavif 1.4.1 decodes both frames. The selected Rust parity run passed
1/1, including exact per-frame pixel references.

The public row
`decode:avif:portable_lossless_filmgrain_420_chroma_from_luma_64x64` adds a
lossless I420 key frame with `chroma_scaling_from_luma=true`, non-empty Y
scaling points, and empty U/V point arrays. The generator preserves the tile
payload and item extent, validates the rewritten syntax with the independent
OBU inspector, and pins the exact Pillow RGB output. Its selected Rust parity
run passed 1/1. Coverage MCP reports no remaining branch gap at the
`read_film_grain` chroma-scaling gate (`frame.rs:4277`) in the full AVIF matrix.

The public row
`decode:avif:portable_lossless_filmgrain_420_zero_y_points_64x64` exercises a
valid 8-bit I420 key frame whose film-grain syntax has `num_y_points == 0` and
CFL disabled. AV1 infers empty U/V point tables and omits luma AR coefficients
for this 4:2:0 syntax. The deterministic generator preserves the tile payload
and item extent, validates the rewritten OBU independently, and pins Pillow's
exact 12,288-byte RGB output
(`5d35ab50438b9f1536ef7de3cbca8ac3d01360efa9d77446dfe0d69786d2fb62`).
The selected public parity row passed 1/1. The decoder now accepts this legal
case, retains a zero luma-grain LUT for active chroma consumers, and skips Y
plane writes when no Y points are present. With no active chroma points, it
returns after geometry and parameter validation without allocating grain LUTs.

This case exposed an admission gap: lossless non-super-resolution inter
reconstruction rejected all film-grain frames before producing a color
surface. The decoder now admits only bounded full-resolution I444 grain for
that lossless profile. Grain remains display-only and retained reference
samples remain ungrained. The bounded geometry and depth checks match the
existing display materializer.

The target-only row
`fault-contract:avif:avif_sgr_intermediate_reservation_failure` injects a
deterministic reservation failure in SGR intermediate allocation while
decoding the active
`decode:avif:animated_lossy_inter_420_superres_sgr_8bit_160x56` input. It
asserts the public allocation error classification and retry behavior; its
oracle status is `not_applicable`. The selected new fault case passed 1/1.
The target-only row
`fault-contract:avif:avif_restoration_stripe_scratch_reservation_failure` uses
the active `decode:avif:high_bitdepth` sequence. Independent OBU inspection
confirms a 64×64, non-super-resolution, single-tile frame with restoration
enabled, which reaches striped restoration. The reservation hook forces the
scratch reservation to fail; ordinary allocation failure now reports the
typed `Dimensions` resource error rather than malformed input. The contract
asserts the exact `SequenceDecode` diagnostic and verifies retry against a
fresh sequence decode. Its oracle status is `not_applicable`, and the selected
fault case passed 1/1.

The target-only row
`fault-contract:avif:avif_restoration_stripe_output_reservation_failure` uses
the active `decode:avif:high_bitdepth` sequence. A `cfg(coverage)` failpoint
forces the striped-restoration output reservation to fail before stripe
scratch is allocated. The public result must be a typed `Dimensions` error at
`SequenceDecode`, and a fresh retry must decode the same reference sequence.
Its oracle status is `not_applicable`, and the selected fault case passed 1/1.
Coverage MCP's selected-test branch comparison against the scratch-reservation
control found one newly observed arm at the output-reservation failpoint
(`restoration.rs:221–223`). The scratch-control arm at `restoration.rs:254–256`
was not observed in the output-failure run because that failure returns before
scratch reservation. MCP source/build identity remains unverified, so these
are coordinate observations rather than a verified aggregate coverage gain.

The target-only row
`fault-contract:avif:avif_tile_cell_reservation_failure` uses the active
`decode:avif:baseline` input. A `cfg(coverage)` failpoint forces the AV1 tile
cell reserve to fail; the contract checks the exact public `Dimensions`
diagnostic at `StillDecode`, then confirms a fresh retry matches a clean
decode. Its oracle status is `not_applicable`, and the selected contract
passed 1/1. The coverage-origin inventory now records 128 exact guards across
20 Rust files; the new reservation hook is classified as `defensive_model`.

The focused film-grain parity rows passed 3/3; all twelve target-only fault
contracts passed 12/12 in the full all-feature campaign. The complete AVIF
decode matrix passed 480/480, and the AVIF edit-list repetition witness passed
after its artifact index was updated to include the new verified loop sample.
The current full report is
`target/release-evidence/coverage-zero-y-filmgrain-tile-cell-20261005.json`.

Coverage MCP `scope: incremental`, metric `lines`, reports five newly observed
coordinates for the zero-Y parity row (0.003624 percentage points) and three
for the tile-cell fault contract (0.002174 percentage points). The comparisons
use the same-source selected reports
`target/release-evidence/coverage-baseline-zero-y-filmgrain-final-20261005.info`,
`target/release-evidence/coverage-filmgrain-zero-y-i420-final-20261005.info`,
`target/release-evidence/coverage-baseline-fault-contract-tile-cell-20261005.info`,
and
`target/release-evidence/coverage-fault-contract-tile-cell-final-20261005.info`.
MCP marks them source-unverified with unknown test attribution, so both gains
are limited coordinate estimates. Branch-detail unions are incomparable.
Invalid reference and point syntax remains outside the positive Pillow parity
lane; the twelve remaining `read_film_grain` branch observations need separate
input-reachability review. The current full report also leaves normal SGR
regions uncovered, including 36 in `sgr_intermediates` and 23 in
`restore_sgr_plane`; the striped restoration contract covers its allocation
failure outcome only. The earlier film-grain, SGR, and stripe reports are
preserved as historical measurements. Earlier reports:
`target/release-evidence/coverage-baseline-filmgrain-existing-20261005.json`,
`target/release-evidence/coverage-filmgrain-reference-reuse-20261005.json`,
`target/release-evidence/coverage-filmgrain-cfl-i420-20261005.json`,
`target/release-evidence/coverage-baseline-faults-existing-20261005.json`, and
`target/release-evidence/coverage-fault-contract-sgr-intermediate-20261005.json`.

A prior full all-feature report and its dated observations are retained below
for historical context.

A later `coverage_matrix_tests` integration-target run completed 56/56 tests on
the 1,981-row matrix. Its parity-target report is
`target/release-evidence/coverage-parity-matrix-final-20261004.json`: 85,100 /
142,526 lines (59.7084%), 16,699 / 32,240 branches (51.7959%), 4,652 / 8,990
functions (51.7464%), and 130,211 / 220,521 regions (59.0470%). This report is
separate from the broader all-feature report above.

### AVIF display-plane allocation fault contract — 2026-10-04

`fault-contract:avif:avif_display_plane_copy_allocation_failure` injects a
one-shot allocation failure immediately before AV1 display-plane copy
reservation while decoding the existing `decode:avif:baseline` input. It
asserts the public `Dimensions` / AVIF / still-decode error and a successful
retry. The case is indexed in the shared matrix with a not-applicable oracle;
it does not add to Pillow parity counts. The selected
all-feature coverage run passed 1/1 and wrote
`target/release-evidence/coverage-fault-contract-avif-20261004.json` (1,972 /
32,244 branches for that selected run). Coverage MCP reports no remaining
branch gap at `src/codecs/avif/av1/surface.rs:174`.

For Coverage MCP incremental comparison, a same-source baseline was captured
with `test_fault_contract_matrix` skipped; its remaining 56 integration tests
passed. The compare call is `incomparable`: LLVM detailed observations do not
reconcile with report totals, and these reports have no source/build receipts
or test attribution. No marginal coverage gain is claimed from that comparison.

`fault-contract:avif:avif_sequence_frames_reservation_failure` drives the
public sequence decoder with the active `decode:avif:animated_motion_chroma`
fixture (32×32, four frames). The coverage-only failpoint asks
`Vec::try_reserve` for an impossible capacity, which deterministically reaches
the existing reservation-error conversion without allocating; the public call
must return `Dimensions` at `SequenceDecode`, and retry must return all four
frames. The selected all-feature run passed both fault contracts (2/2 cases).
Against the same-source 56-test parity baseline at
`target/release-evidence/coverage-parity-baseline-postfmt-20261004.info`, the
selected combined fault report
`target/release-evidence/coverage-fault-contracts-postfmt-20261004.info` adds 23
line coordinates (0.016688 percentage points). Coverage MCP reports no
remaining branch outcomes at `src/codecs/avif/decode.rs:231` or `:241`, the
failpoint and reservation-error conversion, nor at
`src/codecs/avif/av1/surface.rs:174` for the existing still-image fault.
Branch-level incremental union is incomparable because LLVM detail does not
reconcile with aggregate totals. The reports have no source/build receipts or
test attribution, so the line result is a limited estimate rather than a
verified causal claim.

### AV1 split monochrome tile reservation fault contract — 2026-10-04

`fault-contract:avif:avif_monochrome_tile_reservation_failure` uses the active
`decode:avif:multitile_monochrome_split_groups` Pillow parity input. Its first
tile group exercises the existing incremental monochrome tile-state reserve.
The coverage-only fault requests an impossible vector capacity, which reaches
the existing `Dimensions` error conversion without allocating. The selected
target-only contract checks `StillDecode` error classification and that a
fresh retry returns the complete decoded image. The injected result remains
`oracle_status: not_applicable` and outside Pillow parity totals.
The focused five-case fault-contract lane passed 5/5 contracts, and its
instrumented JSON report is
`target/release-evidence/coverage-fault-contracts-five-20261004.json`.
Coverage MCP reports no remaining branch outcomes at the new failpoint on
`src/codecs/avif/av1/frame.rs:1577`. The report has no source/build receipts or
test attribution, so this is limited coverage-coordinate evidence.

For the new case alone, a same-source 56-test parity baseline skipped
`test_fault_contract_matrix`; those tests passed 56/56. Coverage MCP's
incremental comparison against
`target/release-evidence/coverage-parity-baseline-monochrome-fault-20261004.info`
estimates 21 newly observed line coordinates, or 0.015235 percentage points.
The selected report is
`target/release-evidence/coverage-fault-contract-monochrome-tile-20261004.info`.
It remains a limited estimate because source/build identity and test
attribution are unavailable.

### AV1 super-resolution and JPEG multi-scan allocation fault contracts — 2026-10-04

`fault-contract:avif:avif_superresolution_positions_reservation_failure` uses
the active `decode:avif:animated_lossy_inter_420_superres_sgr_8bit_160x56`
sequence. The inter frame reaches AV1 super-resolution after the first frame
has been decoded. A coverage-only one-shot fault requests `usize::MAX`
positions from the existing `try_reserve_exact` call, producing its normal
`Dimensions` / AVIF / `SequenceDecode` error without attempting a large
allocation. Retry must return the matrix canvas and frame count, and its full
result must equal a fresh uninjected decode.

`fault-contract:jpeg:jpeg_multiscan_coefficient_reservation_failure` uses the
active `decode:jpeg:baseline_444_multiscan_duplicate_component` input to enter
multi-scan coefficient retention. Its coverage-only fault forces the actual
`try_reserve_exact` capacity-overflow path; the public result must be
`Dimensions` / JPEG / `StillDecode`, and retry must equal a fresh uninjected
decode. Both cases use `oracle_status: not_applicable` and remain outside
Pillow parity totals.

The focused all-feature baseline passed 56/56 integration tests with the fault
runner skipped. The selected four-case fault lane then passed 4/4 contracts
(the two existing AVIF contracts plus these additions). Its LCOV reports are
`target/release-evidence/coverage-fault-parity-baseline-20261004.info` and
`target/release-evidence/coverage-fault-contracts-new-cases-20261004.info`.
Coverage MCP's incremental line comparison is limited because the reports
have no source/build receipts or test attribution: it estimates 29 newly
observed line coordinates, or 0.021040 percentage points. The path-filtered
observations include the JPEG coefficient reservation and AV1 position
reservation error paths. Branch comparison is incomparable because the
available detail does not reconcile with report totals; no branch gain or
regression claim is made.

### JPEG default Huffman table slot reservation fault contract — 2026-10-14

`fault-contract:jpeg:jpeg_huffman_table_slot_reservation_failure` reuses the
active `decode:jpeg:baseline_444_multiscan_late_dht_after_default_scan` input.
Its first baseline scan reaches the default Huffman table slot reservation
before the later DHT marker. A coverage-only one-shot fault requests
`usize::MAX` slots from the existing `try_reserve_exact` call, taking the
capacity-overflow error path without a large allocation. The public result must
be `Dimensions` / JPEG / `StillDecode` with message
`decode: unable to allocate JPEG Huffman table slots`; retry must equal a fresh
uninjected decode. The fault row uses `oracle_status: not_applicable` and is
excluded from Pillow parity totals; only its ordinary source row carries Pillow
parity. The selected fault-matrix run passed 1/1 case; its report is
`.coverage-mcp/local-evidence/20261014/jpeg-huffman-slot-fault/coverage.json`.
Coverage MCP found no remaining region or branch location at `parser.rs:414` in
that selection. The report has no build/source receipt and test attribution is
unknown, so these are limited selected-coordinate observations; they do not
establish an aggregate coverage gain or full-suite coverage.

### JPEG YCCK output reservation fault contract — 2026-10-19

`fault-contract:jpeg:jpeg_ycck_output_reservation_failure` reuses the active
odd-sized subsampled YCCK input `decode:jpeg:color_ycck_app14_transform_2`.
The coverage-only one-shot fault requests `usize::MAX` output bytes from the
fallible YCCK result reservation, exercising the public `Dimensions` / JPEG /
`StillDecode` failure without attempting a large allocation. Retry must match
the clean Pillow-parity decode. This target-only contract is recorded with
`oracle_status: not_applicable`; the source image remains the input-driven
Pillow parity case.

### JPEG h1v2 upsampler edge and allocation contracts — 2026-10-19

The public parity row `decode:jpeg:baseline_420_chroma_component_sampling_fallback`
exercises the 1×2 chroma upsampling ratio and passes exact 16×16 RGB parity.
Four target-only fault contracts reuse that input to check zero-height and
zero-width geometry, a short source plane, and output-plane reservation
failure. Each returns the expected public JPEG decode error, then retry matches
a clean decode. These rows use `oracle_status: not_applicable` and remain
outside Pillow parity counts.

The selected run passed all five rows and is recorded at
`target/release-evidence/selected/20261019-jpeg-h1v2-helper-edges-final/coverage.json`.
Coverage MCP reports no missing branch groups for
`h1v2_fancy_upsample`; it still reports 18 unobserved regions in that function.
The report has no source/build receipt and MCP test attribution is unknown, so
it is selected-coordinate evidence only. No aggregate coverage gain or
full-suite coverage claim is made, and no full coverage campaign was run.

Six YCCK parity rows also pass in the selected report
`target/release-evidence/selected/20261019-ycck-selected-final/coverage.json`.
They cover odd- and even-height 4:4:0 vertical row upsampling, luma and K-plane
sampling differences, and single-sample h2v1/h2v2 edges. The 17×10 parity case
checks the clamped final source row. Coverage MCP reports 3,975/223,112 regions
observed overall and 186 unobserved regions in the separate full-plane
`h1v2_fancy_upsample` helper for this YCCK-only selection. This report also
lacks source/build identity and test attribution, so it does not establish an
aggregate coverage gain. No full coverage campaign was run.

### AV1 super-resolution output-plane reservation fault contract — 2026-10-04

`fault-contract:avif:avif_superresolution_plane_reservation_failure` reuses
the active 160×56 AV1 super-resolution sequence after the positions vector has
been built. A coverage-only fault requests `usize::MAX` output samples from
the existing plane `try_reserve_exact`; the public sequence result must be
`Dimensions` / AVIF / `SequenceDecode`, and retry must equal a fresh clean
decode. The fault is recorded with `oracle_status: not_applicable` and does
not contribute to Pillow parity counts.

The final-source baseline selected 472 existing AVIF decode rows and five
prior fault contracts; all 57 coverage-matrix harness tests passed. The
selected sixth-contract run also passed all 57 tests. Coverage MCP reports the
sample-reservation error region at `resize.rs:398` as missing in the baseline
and absent from the selected-run gap list. Its incremental comparison is
incomparable because LLVM detail does not reconcile with report totals. Reports
are
`target/release-evidence/coverage-avif-superres-faults-postfmt-baseline-20261004.json`
and
`target/release-evidence/coverage-fault-avif-superres-plane-postfmt-new-20261004.json`.
They lack source/build receipts and test attribution, so the coordinate change
does not establish a numeric marginal gain or a verified regression result.

### AV1 temporal-motion-field reservation fault contract — 2026-10-05

`fault-contract:avif:avif_temporal_motion_field_reservation_failure` injects a
capacity-overflow failure at the existing AV1 temporal-motion-field reservation
while decoding the active `decode:avif:baseline` Pillow input. The selected
contract checks the public AVIF `Dimensions` / `StillDecode` error and its full
diagnostic (`decode: AVIF AV1 validation failed: unable to allocate AV1 temporal
motion field`), then requires a retry to equal a fresh uninjected decode. The
injected outcome is indexed as `oracle_status: not_applicable` and does not
contribute to Pillow parity counts.

The six existing fault contracts passed in the selected baseline run, and the
new contract passed in its one-row run. Coverage MCP showed the allocation
error region as missing in the earlier AVIF report at `motion.rs:3113`; it is
absent from the selected run's gap list at `motion.rs:3122`. The region-level
incremental comparison against the six-case run estimates six newly observed
regions (0.002720 percentage points). This evidence is limited: reports have
no source/build receipts or test attribution. Line comparison is incomparable,
and branch detail does not reconcile with report totals. The selected reports
are
`target/release-evidence/coverage-fault-contracts-six-before-temporal-motion-20261005.json`
and
`target/release-evidence/coverage-fault-contract-avif-temporal-motion-field-v2-20261005.json`.

### AV1 split color-tile reservation fault contract — 2026-10-05

`fault-contract:avif:avif_color_tile_reservation_failure` uses the active
`decode:avif:multitile_color_split_groups` Pillow parity input. The dedicated
`scripts/generate_avif_color_tile_split_fixture.py` derives it from the pinned
`multitile.avif`, preserving frame-header and tile payload bytes while placing
the two color tiles in separate tile-group OBUs. It checks the split ranges
`(0, 0)` and `(1, 1)`, then requires Pillow 12.2.0 to return the same format,
mode, size, metadata, and pixels as the source. The generated input hash is
`654ef88dd8ebc71a554c03f721ad97581fcf2d2df0d825020d267939eb8f5e34`; its
98,304-byte RGB reference hashes to
`8fddfd016bbc17e2f00a5154ee6e50c8f1d9d6a8254279ddf57be490ecdf9e44`.

The selected Pillow parity row passed. The target-only coverage case injects a
one-shot failure at the existing incremental color-tile reservation and checks
the public `Dimensions` / AVIF / `StillDecode` error, including the diagnostic
`decode: AVIF AV1 validation failed: unable to reserve reconstructed AV1 tile
state`; retry must equal a fresh uninjected decode. Its oracle remains
`not_applicable`, outside the Pillow parity result count. The focused fault run
passed 1/1 and wrote
`target/release-evidence/coverage-fault-contract-color-tile-20261005.json`.

Coverage MCP reports the reservation-error closure at
`src/codecs/avif/av1/frame.rs:1574` as missing in the same-source AVIF parity
baseline, and absent from the selected fault report. The JSON region comparison
is incomparable because its detailed LLVM observations do not reconcile with
the reported totals. An LCOV incremental line comparison against
`target/release-evidence/coverage-avif-color-tile-parity-baseline-20261005.info`
reports 29 newly observed line coordinates (0.021037 percentage points); the
selected report is
`target/release-evidence/coverage-fault-contract-color-tile-final-20261005.info`.
The LCOV branch comparison remains incomparable because branch detail does not
reconcile with totals. Neither report has source/build receipts or test
attribution, so the line result is an unverified coordinate estimate, not a
causal coverage claim.

The full `scripts/generate_test_assets.py --format avif` command currently
stops earlier at its ICC fixture hash assertion. The dedicated split fixture
generator and Pillow reference generation completed independently.

### AV1 SGR restoration allocation classification fault contract — 2026-10-05

`fault-contract:avif:avif_sgr_restoration_output_reservation_failure` reuses
the active `decode:avif:animated_lossy_inter_420_superres_sgr_8bit_160x56`
sequence input to inject a deterministic failure at SGR output-plane
reservation. It protects a distinct public diagnostic: allocation failures
must remain `Dimensions` errors, not be mislabeled as malformed AV1 input.
The runner checks the full `SequenceDecode` diagnostic and that retry matches
a fresh uninjected sequence decode. The matching AVIF source row remains the
Pillow parity case; the injected result is `oracle_status: not_applicable`.

The focused target-only run selected, executed, and passed all nine fault
contracts declared in the matrix at that point. Its report is
`target/release-evidence/coverage-fault-contracts-nine-20261005.json` and
records 4,878/32,254 branches for this selected run. Coverage MCP found no
missing branch observation at the SGR allocation error conversion in
`restoration.rs:688`. The report has no source/build receipt or test
attribution, so its coverage coordinates are limited evidence; the runner
output is the evidence that the nine selected contracts passed.

### AV1 equal-width super-resolution parity — 2026-10-04

The active `decode:avif:superres_equal_width_16x16` case uses a pinned lossy
16×16 I420 still whose AV1 sequence enables super-resolution and whose frame
header signals denominator 9 while coded and upscaled widths both remain 16.
The fixture generator checks that syntax, repeats the encode, and verifies the
pinned Pillow 12.2.0 RGB pixels (768 bytes; SHA-256
`d5b4e270a08e4f03c6de84f4488420658a3ab62b00a954aeca14d50f58bb0eef`). The
encoded asset SHA-256 is
`2de74d720f8863be43049a3df776337c1cde494122a57093ffad532cea19a004`.

The selected parity row passed within the 57-test coverage-matrix harness. In
the final-source 472-row AVIF baseline, Coverage MCP lists the true arm at
`resize.rs:208` as missing; in the one-row measurement, that arm is absent from
the gap list while the false arm remains missing. The incremental comparison
is incomparable because LLVM detail does not reconcile with report totals.
Reports are
`target/release-evidence/coverage-avif-superres-faults-postfmt-baseline-20261004.json`
and
`target/release-evidence/coverage-avif-superres-equal-width-postfmt-new-20261004.json`.
The reports lack source/build receipts and test attribution, so this confirms
the selected coordinate change without a numeric marginal percentage claim.

### AV1 equal-width I444 super-resolution parity — 2026-10-05

The active `decode:avif:superres_equal_width_i444_16x16` row adds the 8-bit
4:4:4 path to the existing 4:2:0 equal-width case. The deterministic Y4M
source is pinned at
`0ed9bb725d8f4eb3ae33998b97f064ef9486683156befa28e3c572f652d42788`; the
fixture generator repeats the pinned encode and checks AV1 I444 and
equal-coded/upscaled-width syntax. The fixture hash is
`cd02b86f213b96f9d58bec3781f8855f325aa7b4c867a6fbf76f5802886420a4`. Its
768-byte Pillow RGB reference hashes to
`9cebc72915c26c647a920455c639b606976a1bbc0221ff9bf10cc0c9441864f9`.

The selected public parity run passed 1/1. Coverage MCP branch gaps at
`resize.rs:210` and `:215` now list only the `true` arms, so this I444 row
observes the previously missing `false` arms. The JSON incremental comparison
is incomparable because LLVM detail does not reconcile with the baseline
totals. The LCOV line union estimates 113 newly observed coordinates
(0.081973 percentage points) against
`target/release-evidence/coverage-avif-color-tile-parity-baseline-20261005.info`.
Both reports lack source/build receipts and test attribution, so this is a
limited coordinate estimate rather than a verified causal gain. The selected
reports are
`target/release-evidence/coverage-avif-i444-equal-width-selected-20261005.json`
and
`target/release-evidence/coverage-avif-i444-equal-width-selected-20261005.info`.

### JPEG optimized Huffman length-limit parity — 2026-10-05

The active `encode:jpeg:enc_optimize_huffman_length_limit` row encodes a
deterministic 2504×288 L8 PNG at quality 1 with optimized baseline Huffman
tables. Its balanced frequencies across seventeen AC symbols and EOB force
the JPEG 16-bit code-length limiter to rebalance the generated table. The
integer-basis fixture generator pins the source pixels and PNG hashes to
`8c5f7f43d4a78548fe6c8054b4c6a74279da5d46afe367adbd95a1ec8da92a9c` and
`21d7fefde68d0e4c9ee627c2222831fce271230e3365a8a921393bf37b554338`.
Pillow 12.2.0 with JPEG 6.2 returns the 8,167-byte output pinned at
`4f8d2339e8eb0053fc2190cbbdd7e10a99f74e4139edd7be16805a669db7d668`; public
Rust encode bytes match exactly, and decoded L8 pixels match the Pillow image.

The selected public parity run passed 1/1. Coverage MCP reports no missing
regions, branches, or lines in `jpeg::encode::huffman::optimal_table` for this
selected report. Incremental LCOV comparison is incomparable because the
measurement includes coordinates outside the older baseline inventory; the
JSON detail comparison is also incomparable. Reports lack source/build
receipts and test attribution, so no aggregate percentage gain is claimed.
The selected reports are
`target/release-evidence/coverage-jpeg-huffman-length-limiter-selected-20261005.json`
and
`target/release-evidence/coverage-jpeg-huffman-length-limiter-selected-20261005.info`.

### Target-only fault-contract lane — 2026-10-05

At this report snapshot, the shared matrix had nine active AVIF/JPEG fault
contracts, kept separate from Pillow parity with `oracle_status: not_applicable`. An earlier selected
lane run, recorded before the SGR restoration contract was added, executed
eight contracts and passed all eight public error and retry checks. Coverage
MCP reported no remaining branch gaps at the AVIF color-tile reservation fault
point in `frame.rs:1574` or its reservation error conversion at `:1577`. That
selected report records 4,876/32,252 branches; it is not full-suite coverage.
It has no source/build receipts or test attribution, so those totals are
limited selected-run evidence. Against
`target/release-evidence/coverage-avif-color-tile-parity-baseline-20261005.info`,
the incremental LCOV comparison estimated 1,366 newly observed line
coordinates (0.990925 percentage points). This remains an unverified estimate,
not a receipt-bound causal claim or full-suite gain. The historical reports
are `target/release-evidence/coverage-fault-contracts-eight-agent-20261005.json`
and `target/release-evidence/coverage-fault-contracts-eight-agent-20261005.info`.

That nine-case lane subsequently passed 9/9, including the SGR
restoration allocation-classification contract. Its report is
`target/release-evidence/coverage-fault-contracts-nine-20261005.json` and
records 4,878/32,254 branches. Coverage MCP reports no remaining branch gap at
the SGR allocation error conversion in `restoration.rs:688`. This selected
report also has no source/build receipts or test attribution; the runner output
establishes the nine passing contracts, while its coverage coordinates remain
limited evidence.

### Progressive CMYK without APP14 parity — 2026-10-04

The active `progressive_cmyk_no_app14` row strips the single Adobe APP14
segment from the existing progressive CMYK input. The 11,859-byte fixture has
SHA-256
`abe44e8567c1cbf463ba0e99cd8d96598fad6de8291e8723f3abd00d82ce769c`; Pillow
12.2.0 returns 65,536 CMYK bytes with SHA-256
`ff97c9d811b513694fa089a582cdf540e4f17c26bfa087cd9df458dd0f610385`. The
initial selected parity run found all channels complemented: the progressive
path inverted CMYK samples only when APP14 existed, unlike the baseline path
and Pillow's four-component output convention. The decoder now applies the
same inversion for all four-component progressive JPEGs and no longer stores
an unused APP14 transform byte. The targeted parity run passed for the new
case and the existing baseline and progressive APP14 CMYK neighbors. Its
matrix case is included in the same-source parity baseline; it therefore adds
no further marginal line coverage in a separate selected-row comparison.

### AV1 duplicate tile-group-start parity — 2026-10-04

The active `error_tile_group_start_order` AVIF row changes the second split
tile-group header from tile 1 to a duplicate tile 0, with zero alignment
padding. The 316-byte fixture hashes to
`68781acc246e2946c962c33990c53b50ed308a06fcdec6e421a38dbf353de4cf`.
Pillow reports `RuntimeError: Failed to decode frame 0: Decoding of color
planes failed`; the selected Rust/Pillow parity row passes 1/1 on the local
Pillow 11.3.0 host (the repository pins 12.2.0). The isolated report
`target/release-evidence/coverage-avif-tile-order-final.json` and the refreshed
parity-target report cover the missing true arm at
`src/codecs/avif/av1/frame.rs:1482`. Coverage MCP's selected-test comparison
observes one new coordinate, but remains limited without source/build receipts.
The deeper `accept_tile_group` ordering guard at line 1545 is rejected earlier
by `read_tile_group`, so this row claims only the public path it reaches.

### GIF metadata-limited decode parity — 2026-10-04

The active `palette_absent` case also exercises the public metadata-limited
decode path on its three Pillow-valid GIF inputs, with a 13,772-byte cap above
their metadata extents. The normal decode assertion remains active, and the
policy decode must return exact reference pixels. Its selected matrix row passes
1/1; the isolated report and Coverage MCP comparison observe the false outcome
at `src/codecs/gif/decode.rs:369`. The full comparison also observes the true
outcome at line 400 for the local-table variants. Pillow pixel references and GIF
assets were unchanged. The host Pillow version is 11.3.0 while the repository
pins 12.2.0, so this Rust-only policy-field change did not regenerate oracle
outputs.

### Progressive JPEG restart-overrun parity — 2026-10-04

The active decode row `progressive_restart_overrun` mutates the second
progressive scan in `progressive_restart.jpg` by appending sequence-correct
RST7/RST0 markers at its entropy boundary, before the following DHT and DRI
markers. Its 5,655-byte input hashes to
`509d436c87d82c880128aeffe1b41a39819dc7030141964bd302a6e4039f3c86`; pinned
Pillow 12.2.0 returns the same 128×128 RGB pixels as the unmutated source, with
reference SHA-256
`e92abdc1f9d14fd2491920de98bdcee028788715bebc6ba5d21b20d89b59fd76`. The
selected public parity row passes 1/1. Its isolated LLVM report records 4 true
and 5,632 false outcomes at
`src/codecs/jpeg/decode/progressive.rs:832`, exercising the extra empty segment
past the 256-MCU scan. Coverage MCP has no source/build receipt or test
attribution for this selected report; the named-row result and branch counts
are local execution evidence.

### JPEG low-quality 4:2:0 edge parity — 2026-10-04

### Monochrome AV1 film-grain parity — 2026-10-04

The active decode row `portable_lossless_filmgrain_monochrome_64x64` is a
6,210-byte 64×64 8-bit all-lossless monochrome AV1 primary still with frame
film grain. Its AVIF input SHA-256 is
`0bc3fe81f320d7f55853d53ec7b7fa20f099bf8af7e5e7ccbaa69556ccd980d4`; Pillow
12.2.0's 12,288-byte RGB reference hashes to
`853bfb557b4ab4960d708f4ecfeda145ed9feab8c987214d82ee6a20665e93f4`. The
selected public parity row passes. The decoder now admits monochrome primary
sequences that declare film-grain support because selected-display
materialization applies the frame's grain before checking the complete plane.
The auxiliary-alpha admission rule remains unchanged.

Coverage MCP compares eight newly observed branch coordinates in
`film_grain::apply_monochrome` with the earlier full report; eight validation
arms remain unobserved. Both reports lack source/build receipts and test
attribution, so this is limited coordinate evidence, not a verified causal
claim.

### JPEG low-quality 4:2:0 edge parity — 2026-10-04

The active encode row `enc_sub_420_q10_odd_streaming_edge` combines quality 10,
4:2:0 sampling, and a 33×33 source, exercising partial right and bottom MCU
edges on the low-quality streaming path. Its existing source asset hashes to
`afdec16d964c39b1d8f8329f72d798669ce507bf0904f96388fd273ddf65cc45`; Pillow
12.2.0's 33×33 RGB reference is 3,267 bytes with SHA-256
`9ee98db427d2e8ac625f6eb0e09e79f96ae13ff7c975297d537fe0e1725c27e4`, and the
encoded JPEG reference is 777 bytes with SHA-256
`db364d028ea02c8c5e3548e22df6c1d4a74466e9afb06cf952bfe0e47e71419e`. The
selected public matrix row passes. Its selected LLVM report records the true
arm of `present[0]` at `jpeg/encode/mod.rs:3925`; the remaining listed arms are
structural states, and the full coverage totals remain unchanged. MCP reports
no source/build receipt and unknown test attribution, so the branch observation
is coordinate evidence rather than a receipt-verified causal claim.

### GIF reserved disposal values — 2026-10-04

The active rows `enc_animated_disposal_8_reserved_bits` and
`enc_animated_disposal_255_u8_limit` exercise GIF sequence encoding with the
full public `u8` disposal range. Pillow 12.2.0 shifts the supplied value into
the graphic-control packed byte: disposal 8 produces `0x20` / `0x21`, while
255 produces `0xfc` / `0xfd`. The exact two-frame Pillow outputs hash to
`8a8954d61effd22db2292078adda561beb35102d50c36d2cd438094787afc190` and
`727785b857f0bbb52cddea5cbbad4a14b8721f68914eb31cc557b92344a69305`.
Both selected public parity rows pass. Coverage MCP returns no missing branch
group for `disposal_code`; its report has no source/build receipt and unknown
test attribution, so this is coordinate evidence rather than a verified
causal claim.

### Public parity rows added — 2026-10-04

The active AVIF rows `error_sequence_frame_id_width_exceeds_16` and
`error_sequence_identity_color_matrix_profile2_8bit` mutate the pinned
error-resilient animation's sequence header. The first changes its three-bit
additional frame-ID length from 0 to 2, declaring a 17-bit frame ID. The
1,084-byte input hashes to
`6970dc8e41a8861097dff0fbd8577b84dde72ec2018d3f2f75169bf7d78a123e`. Pillow
12.2.0 opens and verifies the container, then reports
`RuntimeError: Failed to decode frame 0: Decoding of color planes failed` on
load. The selected public decode row passes. The selected LLVM branch report
observes the true arm of the 16-bit guard at
`src/codecs/avif/av1/sequence.rs:266`, which the preceding full Coverage MCP
report listed as missing. Coverage reports lack source/build receipts and test
attribution, and the selected comparison is not aggregate-comparable, so this
is coordinate-level evidence. The second changes the same sequence from Main
profile to profile 2 while retaining 8-bit depth, then sets the CICP matrix to
the identity value. Its input hash is
`478d360b9c47d12f32c1a2f6ee9da0a483f7b6ac87b1ea93deda0e786dea07f6`; Pillow
12.2.0 opens and verifies it before reporting the same decode error. Its
selected branch report enters identity-matrix parsing, but raw counts at
`sequence.rs:354` show its `profile != 1` true arm and both nested
`profile == 2 && bit_depth == 12` outcomes remain unobserved. This malformed
row therefore does not close the profile/depth guard.

The active rows `baseline_reserved_ac_zero_size_eob` and
`baseline_multiscan_reserved_ac_zero_size_eob` cover Pillow's handling of a
reserved zero-size AC symbol (0x10) as end-of-block. Their 137-byte grayscale
and 165-byte 4:4:4 multiscan inputs hash to
`278b5b7d9a5d56799646149baec5c89be8d1f1bc1d39c74f0ab02b16a53c774d` and
`8aa96a1a0a6e8edd37fafbe7be424c3b3691deb16923a469c46718730a10e7bc`.
Pillow 12.2.0 yields 8×8 grayscale and RGB images respectively, all pixels
128; both selected public decode rows pass exact parity. The grayscale row
reaches the single-scan fast path, and the three-scan RGB row dispatches through
the scalar baseline block decoder. Zero-size EOB leaves the DC-only output
optimization available.

The active JPEG rows `baseline_420_restart_row_aligned` and
`baseline_420_restart_overrun` add an exact DRI=8 4:2:0 control and a
Pillow-tolerated surplus-restart mutation. The 5,954-byte control hashes to
`a0ea540bcedbb4fdb195055eee29aa0e3f5b9a856e6bea195002be0d8f7d5f4c`; the
5,958-byte candidate hashes to
`d52d934b119504b84ee1697bec2ce78077788e3a98ff8073a371a2a42b26715c`. Pillow
12.2.0 loads and verifies both as 128×128 RGB with the same pixel SHA-256
`e92abdc1f9d14fd2491920de98bdcee028788715bebc6ba5d21b20d89b59fd76`. The
candidate inserts `FF D0 FF D1` before EOI. With 64 MCUs and DRI 8, the decoder
expects eight entropy segments; the mutation supplies ten, reaching the
direct-path segment-count fallback at
`src/codecs/jpeg/decode/decode.rs:1180`. The focused public row passes 1/1.
Coverage MCP's refreshed full report has no missing branch group at that
location. MCP still lacks source/build receipts and test attribution, so this
is coordinate evidence rather than a receipt-verified causal claim.

The active JPEG row `baseline_444_single_component_scan_cb` retains only the
first Cb scan from `baseline_444_three_sequential_scans` and appends EOI. The
640-byte input hashes to
`6ab4a1f95019f910ed48b81d5889ff9a464904c9ba158232ab290e41dfe16231`; Pillow
12.2.0 opens, verifies, and decodes it as 8×8 RGB with pixel SHA-256
`3100a6a318864cd8fff4192c9acc15b51f2219a0195043761c7a38caf5b3963a`. This
single-scan route reaches the `scan_components.len() != 3` fallback guard in
`src/codecs/jpeg/decode/decode.rs:1590`, then decodes through the general
checked path. The public matrix test and complete `make coverage` campaign
pass. Coverage MCP no longer lists that outcome as missing, but cannot attribute
the hit to a test without source/build receipts. No codec or SIMD implementation
changed in this fixture batch.

The active JPEG row `baseline_cmyk_single_component_scan` retains only the
first component scan from `baseline_cmyk_13x9.jpg`. Its 13×9 CMYK input hashes
to `81109c230ad68dc6ddfccb25a441f9c0ceb2b5d336a3e948b2496628849c1c7f`, and
Pillow 12.2.0's decoded CMYK pixels hash to
`1ca1b4e0d3309f8b0e4d84ddc4fd20c8fc98827a62e0b4d3713ef3017f52f7b4`. The
focused public JPEG matrix passes 202/202 rows. This input reaches the
single-component fallback in `src/codecs/jpeg/decode/decode.rs:963`; the
coverage campaign below measures it in the current report.

The active AVIF row `animated_lossy_inter_420_superres_sgr_8bit_160x56`
extends the 10-bit super-resolution/restoration witness to 8-bit I420. Its
frame 1 uses largest-transform mode, 16×16 partitions, super-resolution from
128×56 to 160×56, and SGR restoration on all three planes. The 1,612-byte
timestamp-normalized input hashes to
`c294163610d4a45852fe374e0345c878979bb81e5ea94596960ef64411180fd7`.
Pillow 12.2.0 RGB frame hashes are
`69db76fd576323ba0a068072e8473ed16740c868e8c8c7199063056f97b4f9ed` and
`0bb7e3278f7cad0535a87e433d644434074b207396bf668ab3f5007dfca98061`; the
selected public sequence decode matches them byte-for-byte. The fixture uses
the pinned encoder compatibility patch documented in
`scripts/avif_fixture_oracle/README.md`.

The active AVIF rows `error_av1c_invalid_marker` and
`error_av1c_twelve_bit_without_high_bit_depth` each mutate one field in the
3,077-byte baseline file. Their input SHA-256 values are
`8e183203e849b4a7fd01bf5662b585a42b745b01b18a62a5c11cb64d550dbae9` and
`68e3963bfc3437156e871b5b48e279e1450fa8d67f22b03644925190b8b03695`. Pillow
12.2.0 reports `PIL.UnidentifiedImageError`; the Rust parity contract reports
`malformed`. The AVIF decode matrix passes 442/442 cases, and Coverage MCP now
returns no missing branch observations at `samples.rs:724` and `samples.rs:732`.

The active AVIF row `error_animated_duplicate_sample_size` retypes the animated
fixture's `stss` box as a second `stsz`, places it before `stco`, and makes the
following chunk-offset table incomplete. Its input SHA-256 is
`99400ac1612db40de4af63b07e7511483381590689be3cc0ed1607df63e44959`.
Pillow reports `PIL.UnidentifiedImageError`, and the Rust parity contract
reports `malformed`. The AVIF decode matrix passes 443/443 cases. Coverage MCP
reports no missing branch observation at `samples.rs:2189`; the report still
lacks a source/build receipt, so this remains coordinate-level evidence.

The active AVIF row `animated_lossy_inter_420_superres_sgr_10bit_160x56` adds a
full-range two-frame 10-bit I420 sequence. Its inter frame is coded at
128×56, super-resolves to 160×56, and uses SGR restoration on all three planes;
the bitstream uses the supported largest-transform mode. The 1,366-byte
timestamp-normalized input hashes to
`016e4a8433002b60899744fba6f26a7c10af82c192f65b4c5c19233b15c3cb11`. Both
public Rust decode APIs match Pillow 12.2.0 byte-for-byte: frame hashes are
`69db76fd576323ba0a068072e8473ed16740c868e8c8c7199063056f97b4f9ed` and
`199a031bed1879efe5b17d90bacd44226ec2c05da120f4e981326695e6d493d0`, each
with a 33 ms duration. The selected manifest parity row passes 1/1. Coverage
MCP's full-report coordinate comparison shows this case alongside other
all-feature work exercising 61 additional outcomes in the high-depth
super-resolution/restoration admission function; the function now has 65
unobserved branch outcomes. MCP reports no source/build receipt or test
attribution, so the comparison is not a verified causal claim.

The active AVIF row `animated_lossy_inter_420_superres_select_10bit_160x56`
adds a companion full-range 10-bit I420 sequence. Its inter frame uses
TX_MODE_SELECT with the same 128×56 to 160×56 super-resolution geometry and no
active restoration. The 1,311-byte timestamp-normalized input hashes to
`04e31a3ac36c25ef77061a2ed09b79fd8fb885a5ff14d6ab3a82e2d32ad3b02e`. Public
Rust decode and sequence decode match Pillow 12.2.0 byte-for-byte; the RGB
frame hashes are `69db76fd576323ba0a068072e8473ed16740c868e8c8c7199063056f97b4f9ed`
and `7aa2455358aa4a1d51b3a4a263a425229b1be524efd68979a26dad18c8250fef`, both
with a 33 ms duration. The focused row passes 1/1, and native repetition is
confirmed as infinite. Coverage MCP's selected comparison against the SGR row
shows seven newly observed coordinates in
`complete_high_depth_inter_reconstruction_context`; the all-feature report
also adds one covered branch relative to the prior full report. Those reports
lack source/build receipts and test attribution, so these remain coordinate
signals rather than verified attribution.

The active ICO row `error_short_header_metadata_preflight` uses a four-byte
recognized signature without the remaining directory header. Its input SHA-256
is `6b1e73a0094b7b812d3b9e22cffb4f8239319847522c4fa103753b6950020f93`; the
manifest also compares `decode_with_policy` with a zero metadata-byte limit.
Pillow reports `PIL.UnidentifiedImageError`, and the Rust parity contract
reports `malformed`. The ICO decode matrix passes 57/57 cases, and Coverage MCP
returns no missing branch observation at `ico/decode.rs:123`.

The active WebP encode row `enc_lossless_sampling_row_compaction_checkpoint`
uses a deterministic 512×1,024 RGB input whose two VP8L histogram bands compact
to multiple rows. The token-aware encode shard passes 47/47 cases, including
byte-for-byte Pillow parity for the new row. The input SHA-256 is
`36d8d0a1048480054a0d99cb503cab17e299d0f6fa1f9afdbee81e1568e43478`; its
395,738-byte Pillow WebP output has SHA-256
`5d216ba74978fb381c518359f11db60861b8ce6a25ab2c30edd4bdcd8e1ed012`. The
selected Coverage MCP report has no missing branch at
`webp/native/encoder.rs:1454`, the nonzero-row token checkpoint. Its report
still lacks a source/build receipt, so this is coordinate-level coverage
evidence.

### Token-aware VP8L sampling checkpoint parity — 2026-10-04

The active WebP encode row `enc_lossless_sampling_equal_rows_checkpoint` uses
an 8,192×16 RGB fixture with 511 unique colors. Repeated histogram
distributions across tile rows reach the token-aware row and column sampling
checkpoints without selecting the palette transform. The selected public row
passes Pillow byte parity, including the token-aware Rust output. The input
SHA-256 is
`cda7a74a4e7b6e8414374a82307919b65426a6c24b252055e6f89c0cc230650a`; its
134,180-byte Pillow WebP output has SHA-256
`3da23b0660aba09c176a17b552913c14e33f2ac943a265582bdbfb5a0529b92f`.
Coverage MCP reports no missing branch outcomes at
`webp/native/encoder.rs:1378` and `:1418`, the 1,024-comparison row and column
checkpoints. The incremental LCOV estimate reports four newly covered lines
and a 0.002902 percentage-point gain against the pre-row baseline. That
increment is coordinate evidence only because source/build and test receipts
are unavailable; branch-level incremental unions remain incomparable.

The current matrix contains 2,233 total rows: 1,753 decode / inspect /
verify rows and 480 encode rows. Of those, 1,753 decode rows and 448 encode
rows are active; 0 decode rows and 32 encode rows are planned. TIFF has
288 decode rows. The 31 fault contracts are tracked separately from the
Pillow parity totals. Full
and selected MCP reports lack source/build receipts and test attribution, so
MCP locations are coordinate evidence rather than verified named-test
attribution.

### PNG Paeth tie parity — 2026-10-15

Two active public encode rows now target Paeth predictor tie behavior.
`enc_paeth_tie_precedence` uses a 2x7 grayscale raster to exercise the
predictor return arms and equal adaptive filter scores.
`enc_paeth_emitted_above_upper_left_tie` uses a 3x2 grayscale raster where
Paeth wins adaptive selection and distinct, equally close above and
upper-left samples make its tie choice visible in exact encoded bytes. The
pinned Pillow 12.2.0 references and round-trip pixels are generated from the
public encoder. The second reference decompresses to filter bytes `[0, 4]`
with Paeth residuals `[1, 1, 0]`. The selected public matrix run passed both
rows (2/2), made eight public encode calls, and had no panics.

Coverage MCP returned no missing branch groups for
`src/codecs/png/encode.rs:275` or `:339` in the selected report
`target/release-evidence/coverage-png-paeth-tie-final.json`. That report
observed 269/31,320 branch outcomes. It has no source/build receipt and test
attribution is unknown. Incremental comparison with
`coverage-png-matrix-current.json` is incomparable because the branch detail
differs (31,320 versus 31,314 outcomes); no marginal gain or full-suite
coverage claim is made. The saved full baseline remains unchanged.

### JPEG Huffman edge rows — 2026-10-14

Two generated JPEG parity rows extend the active matrix. `long_huffman_code_slow_path`
is an 8x8 grayscale SOF0 image whose one-symbol DC and AC tables use valid
11-bit codes; pinned Pillow 12.2.0 decodes all 64 samples as 128. The row
exercises the >10-bit lookup fallback through public decode operations.
`baseline_444_unused_custom_dc_chroma_table` adds unused DHT selector 2 with
standard chroma DC code counts and swapped symbol values. Pillow 12.2.0
returns the same 128x128 RGB pixels as the baseline image, while Rust takes
the generic custom-table path.

The saved full branch report
`.coverage-mcp/local-evidence/20261013/main-full-baseline/coverage.json`
lists the false arm of `standard_huff_table` at `huffman.rs:89` as missing.
The selected public row passed 1/1. Its focused report is
`.coverage-mcp/local-evidence/20261014/jpeg-unused-custom-dc-chroma/coverage.json`;
Coverage MCP no longer lists the values-comparison false arm at line 89, and a
selected-row comparison against `jpeg-empty-dc-huffman/coverage.json` reports
three newly observed branch coordinates, including that arm. The comparison
is `limited`: source/build identities are unverified and test attribution is
unknown. This is targeted coordinate evidence, not a verified aggregate gain.
No full coverage run was made.

### Current AVIF location generator observations — 2026-10-05

The current AVIF location inputs come from
`scripts/generate_avif_idat_fixture.py` through the normal asset hook. They
exercise a truncated extent index, an unsupported three-byte index width,
assembly from two indexed idat extents, and an extent ending one byte beyond
idat. Pinned Pillow 12.2.0 rejects the first two at open, preserves the split
item's complete image result, and accepts inspection and verification of the
overrun before frame decoding rejects it as malformed. The canonical reference
generator reads configuration depth through the bounded configuration-only
reader in `scripts/inspect_avif_bitstreams.py`; its full sample inspector keeps
extent validation. Independent enrichment failures abort generation. These
ordinary cases remain separate from
the 24 target-only fault contracts.

### Token-aware PNG reserved-distance parity — 2026-10-04

The active row `error_token_aware_reserved_deflate_distance` reuses the pinned
`zlib_reserved_distance_symbol.png` input through the public
`decode_with_token` API and checks the same Pillow 12.2.0 malformed-decode
contract as ordinary decode. Its 67-byte fixture has SHA-256
`222fcf6a5fc0b121582aa779cd86a7e77e35948b779e71151313d9ced571b7b1`; the
selected public parity row passes 1/1.

The selected LLVM report records one hit on the reserved-distance true arm at
`src/codecs/compression/deflate.rs:539`, which the earlier full report lists as
uncovered. The other outcome remains uncovered in this one-row selection. The
full-versus-selected MCP comparison is incomparable because baseline branch
detail is incomplete; both reports lack source/build receipts and test
attribution, so this is coordinate-level evidence only. No codec or SIMD
implementation changed.

### AVIF profile-2 12-bit identity-CICP parity — 2026-10-04

The active row `profile2_identity_12bit_444` uses a reproducible 16×16 full-range
4:4:4 AVIF with profile 2, 12-bit samples, and CICP 1/13/0. Its 1,188-byte
fixture SHA-256 is
`1ce2aae5539074e3a65092f185c4db34537462d637859d4fd85449bb15557931`; the
768-byte Pillow RGB8 reference SHA-256 is
`45114693c3cde135ae810d49fbfb8bcf54204da4b9bf11fa3094b5e177686343`. The
selected public parity row passes 1/1. The decoder maps AV1's Y/Cb/Cr identity
planes to G/B/R and truncates validated 12-bit samples to RGB8; existing YUV
conversion kernels are unchanged.

The fresh selected LLVM report records the true outcomes of all three guards at
`src/codecs/avif/av1/sequence.rs:354` (three hits each). The separately selected
8-bit malformed identity-CICP row records the false depth outcome (three hits),
so the two public rows exercise both depth outcomes. Coverage MCP reports the
opposite outcomes missing in their respective single-row selections. The
`profile != 1` false arm and `profile == 2` false arm remain unobserved. MCP
reports lack source/build receipts and test attribution; its selected-test
comparison is limited coordinate evidence, not a verified full-suite gain.

### VP8 loop-filter mode-delta parity — 2026-10-04

The active WebP row `lossy_vp8_mode_filter_delta` uses a deterministic
128×128 RGB keyframe with a +5 VP8 loop-filter mode delta. The 3,312-byte
fixture SHA-256 is
`86b97b8489b9ec2318858bef5d91be138e1222398dd2b8ffbde6dded0e598b55`; its
Pillow RGB pixel SHA-256 is
`ce195285d0ae374bc2ae5538da590658fbe26846b27cf2b0d2300979319f4c99`. The
selected public decode parity row passes 1/1. Its fixture generator applies the
encoder-only patch to a temporary copy of pinned libwebp 1.6.0 and checks the
Pillow output hashes.

The selected LLVM report records true hits at
`src/codecs/webp/native/vp8.rs:1158` (3), `:1283` (3), and `:1913` (192); the
mode check at `:1915` observes both outcomes (177 true, 15 false). Coverage MCP
confirms the opposite outcomes at 1158, 1283, and 1913 remain unobserved in this
selected-only report. The earlier full report listed these true outcomes as
missing, but neither report has source/build receipts or test attribution, so
the comparison remains coordinate-level evidence.

## WebP malformed chunk extents and GIF89a exact metadata budget — 2026-10-04

The active WebP row `error_truncated_chunk_header_metadata_preflight` retains
the valid VP8X and EXIF chunks from `exif.webp`, removes its VP8L image chunk,
and adds one byte inside the corrected RIFF extent. Pillow rejects the
resulting malformed file; the Rust row exercises the metadata preflight path
with a zero-byte metadata limit. The full parity matrix passes, and Coverage
MCP no longer lists the short-chunk-header branch at
`src/codecs/webp/decode.rs:203`. The report is source-unverified and its test
attribution is unknown, so this is coordinate evidence.

The active WebP row `error_chunk_exceeds_riff_metadata_preflight` uses the same
extended metadata container with the EXIF chunk length raised past the RIFF
extent. Pillow and the Rust public operations reject it, and the zero-byte
metadata-limit call reaches the chunk-extent error at
`src/codecs/webp/decode.rs:223`. Coverage MCP no longer lists either line 203
or line 223. Its two remaining WebP metadata branches, at lines 185 and 186,
reject bad RIFF/WEBP signatures before public metadata scanning can be entered.

The active GIF row `gif89a_exact_metadata_limit` checks that the existing
GIF89a `lzw_kwkwk.gif` fixture decodes at its exact 39-byte metadata limit.
Its Pillow pixel parity passes. This case adds a metadata-policy regression
check but does not change aggregate coverage; the remaining reported GIF
signature arm corresponds to invalid signatures, which public format
detection rejects before metadata scanning.

An earlier local full report recorded 86,813/142,471 lines (60.9338%),
16,768/32,250 branches (51.9938%), 4,849/8,990 functions (53.9377%), and
132,773/220,396 regions (60.2429%). The GIF exact-budget case adds a parity
regression check without changing that report's aggregate coverage. The report
and MCP view lack source/build receipts and test attribution, so their source
locations are coordinate evidence rather than verified causal attribution.

The active AVIF row `iref_future_version` adds an empty version-2 `iref`
FullBox and preserves exact Pillow 12.2.0 metadata and pixels. Its selected
public Rust parity row passes.

The active JPEG row `baseline_444_multiscan_final_empty_entropy` derives an
8×8 baseline 4:4:4 image with an empty final component scan immediately before
EOI. Pinned Pillow 12.2.0 decodes the fixture and the selected Rust parity row
passes. The refreshed local LLVM report with both rows records
86,819/142,504 lines (60.9239%), 16,751/32,252 branches (51.9379%),
4,847/8,990 functions (53.9155%), and 132,753/220,401 regions (60.2325%).
Coverage MCP no longer reports missing branch observations at
`avif/samples.rs:982` or `jpeg/decode/decode.rs:2473`. The local report has no
source/build receipt or test attribution, so these locations are coordinate
evidence rather than verified causal claims.

MCP still reports the true outcome for the right-hand comparison at
`tiff/decode.rs:1872` and `:1965`. The active invalid-first-code TIFF row makes
the left side of `code >= CLEAR || output.len() >= expected` true, so the
right side is short-circuited. Public TIFF inputs keep `expected` positive and
the decoder stops when it reaches that size; no public Pillow-parity input can
take the right-hand true outcome. This is not a missing public fixture.

The active WebP row `enc_lossy_method_6_token_checkpoint` exercises the
token-aware lossy VP8 method-6 trellis path on `lossless.webp`. Its 128×128 RGB
input SHA-256 is
`20eb48b4ef2a42d4842684930c2023f7d14ac343b825d19a185e8dda5b25f8b2`; its
49,152-byte Pillow RGB reference hashes to
`2c759de6a1ae9096f129744a6c04d24f1a611bcd8841b26810cc1c583b6a1525`, and its
3,570-byte Pillow WebP output hashes to
`ba524d40418e1bd254e437cb4cf51d98e10d31df9fb5a163938840b9e3fddf4f`. The
token-aware WebP encode shard passes 47/47 cases, including byte-for-byte
parity for this row. Coverage MCP's focused report no longer lists a missing
group at `webp/encode/vp8/quant.rs:238`, and the raw LLVM export records 10,240
executions of the token-specific quantizer specialization. The aggregate
branch percentage did not change; the report has no source/build receipt or
test attribution, so these are coordinate and specialization observations,
not receipt-verified causal attribution.

A parity-only coverage-matrix run passed 38 public parity-target tests with 18
internal/admin tests filtered out. Its selected report,
`target/release-evidence/coverage-public-parity-20261004-final6.json`, records
83,386/142,489 lines (58.5210%), 16,208/32,252 branches (50.2542%),
4,554/8,987 functions (50.6732%), and 127,597/220,381 regions (57.8984%).
Compared with final5, raw covered totals rise by 50 lines, 12 branches, six
functions, and 81 regions. Coverage MCP's selected comparison observes six
new `parse_pasp` branch coordinates: the valid false outcomes in both parser
implementations. Across the three TIFF
increments, Coverage MCP observes the zero next-IFD arm at `decode.rs:46`, three
`metadata_bytes` outcomes, and the `inspect_basic` arm at `inspect.rs:47`.
Current filtered gap queries list no missing TIFF outcomes in
`metadata_bytes`, `decode_page`, or `inspect_inner`. MCP marks the comparisons
limited because source/build receipts are unavailable; they are coordinate
evidence rather than verified causal or regression claims. The incremental
comparison to final5 is limited because source/build identity is unverified.
These selected-target totals describe the public parity test target and do not
replace the full release coverage campaign.

The active AVIF row `primary_item_pasp_4x3` adds the exact 342-byte input
`primary_item_irot_pasp_4x3.avif` (SHA-256
`2acc2e59572718a5fd1247aef7d18ab7666943533f068c221117df05378c4564`). It
associates a valid 4:3 `pasp` property with the existing 270-degree primary-item
rotation fixture; the property and association add 17 metadata bytes, and the
`iloc` extent is relocated from byte 285 to 302. Pillow 12.2.0 verifies and
loads it as 2×3 RGB with the same info and all 18 pixel bytes as the source.
The focused public AVIF parity target passes 1/1. Coverage MCP reports
`target/release-evidence/coverage-avif-pasp-20261004.json` and
`target/release-evidence/coverage-public-parity-20261004-final6.json` leave only
the three invalid-value arms in each `container::parse_pasp` and
`samples::parse_pasp` unobserved; the valid false/success outcomes are present.
Both reports lack source/build receipts and test attribution, so this is
coordinate evidence, not a verified suite-wide coverage gain.

The active AVIF row `clap_zero_width_ignored` adds an essential clean-aperture
property with a zero width numerator to the 3,077-byte baseline AVIF. The
3,118-byte input hashes to
`ce95ee92b5498964ff6e3e0bd383608c559fafcafe7bd110e5f2101d10734572`;
Pillow 12.2.0/libavif 1.4.1 verifies and decodes it as 128×128 RGB with the
unchanged pixel SHA-256
`f1a2555b1c61036af2bd1d3d125a6a1343993873d9e5d58e3caee79989095dcf`. Rust's
inspector and decoder now preserve this property as opaque metadata and do not
apply a crop, matching Pillow.

The active AVIF row `clap_zero_width_denominator_ignored` adds an essential
clean-aperture property with a nonzero width numerator and a zero width
denominator to the same 3,077-byte baseline. The 3,118-byte input hashes to
`8641c0eff26bfdf83f998e2a13737eeea8dbff30d04c818109a358d3a267b4e1`.
Pinned Pillow 12.2.0/libavif 1.4.1 verifies and decodes it as 128×128 RGB with
the unchanged pixel SHA-256
`f1a2555b1c61036af2bd1d3d125a6a1343993873d9e5d58e3caee79989095dcf`.

Six further active rows exercise the other ignored clean-aperture cases. The
3,118-byte zero-height-numerator, zero-height-denominator, zero-horizontal-
offset-denominator, and zero-vertical-offset-denominator inputs hash to
`ec0031d6d620d303a05937051504d621498dfe73920c51fb072e03c7746fbe2c`,
`1004a811270a222f0072637fafc2b1004f25084a6f8f1b34976696595fefefab`,
`9d67878e798b93a6fe551fdd492701a501d4744d847f26c670e5c793c9a367c3`, and
`662fca402bf3a78537aaaadbe101c5ceaee736a064ddcaa46ff5931ec17971c2`.
The trailing-payload rows contain one and 16 bytes and hash to
`ece2fc8f98cc42f31a8806690ac415460d5eadf96c93492ba2d710c0b2e22bde` and
`93038a983d242ee2204d866b63a1811573acc7f8e38699595d328ccdf49cf2d7`.
Pinned Pillow 12.2.0/libavif 1.4.1 verifies and decodes all six rows as
128×128 RGB with the unchanged pixel SHA-256 above. Both AVIF parsers now
preserve these unusable clean-aperture declarations as opaque metadata. The
full AVIF matrix passes 467/467, and Coverage MCP reports no missing branch
group in either `parse_clap`; its source identity and test attribution remain
unverified.

The active GIF row `lzw_long_expansion_token_checkpoint` is a 1,024×515 Pillow
12.2.0 palette image. Its 1,909-byte input hashes to
`60bfab0d96d4fcc742246b440f337bc183e1e1ead86dfa02a8fb4013f07e571a`; its
527,360-byte decoded reference has SHA-256
`8245292ed38da63c136ed67020067c8dad124da8cadacdb8cad7489f914fa01b`. Its
valid LZW phrase reaches the cancellable expansion checkpoint, and the new GIF
fixture passes exact public pixel parity. A separate
`error_lzw_end_only_token_checkpoint` row checks the immediate-end-code error
through the token-aware public decoder; it was split out of the aggregate
malformed group so the token-aware assertion applies only to the relevant
input. Two more dedicated error rows cover an invalid first LZW code and a
future dictionary reference. The selected four-row GIF shard passes 4/4. The
full selected Coverage MCP report returns no missing branch groups for
`decode_lzw_with_token` or `append_code_with_token` in
`src/codecs/gif/decode.rs`; this remains coordinate evidence.

Four existing Pillow rows now also exercise public `decode_with_policy` at the
measured metadata boundary: BMP `depth_24` at 54 bytes, ICO `single_icon` at 22,
WebP `icc_profile` at 69, and TIFF `tiled_missing_byte_counts` at 146. The
focused BMP/ICO/WebP selector passes 3/3 and the TIFF selector passes 1/1; all
four retain exact Pillow pixel references. Against the prior selected report,
MCP's filtered gap listings now show one missing BMP `metadata_bytes` outcome
instead of two, no ICO `metadata_bytes` group instead of one, six WebP
`metadata_bytes` outcomes instead of 19, and three TIFF outcomes instead of 10.
The TIFF check exercises the unequal offset/count length path at
`tiff/decode.rs:150`. The cycle and empty-directory policy rows exercise the
remaining TIFF `metadata_bytes` exits; the final selected report has no missing
branch groups for that function. MCP's aggregate incremental comparison remains
incomparable and its reports lack source/build receipts and test attribution,
so these are coordinate-level observations rather than verified test
attribution.

The TIFF `cyclic_ifd_leniency` row exercises metadata policy on the
self-referential `cyclic_ifd.tiff` with its measured inclusive limit of 140
bytes. A dedicated `error_empty_ifd_chain_metadata_policy` row exercises
`empty_ifd_chain.tiff` with a zero limit and retains Pillow's malformed error.
Together they cover the cycle stop, matching strip offset/count arrays, and
no-directory error path. The final selected report has no remaining TIFF
`metadata_bytes` branch gaps; source/build identity is unverified.

The TIFF `single_page` row requests page index 1 from `single.tiff`. Pinned
Pillow 12.2.0 reports `builtins.EOFError`, and the public owned and borrowed
`decode_frame` calls are pinned to Rust's `parameter` error. This reaches the
zero next-IFD termination arm in `tiff/decode.rs:46`; the preceding selected
report listed that arm as missing. The final parity-only run passed with the
frame request included, and Coverage MCP no longer lists that arm. The report
has no source/build receipt or test attribution. The row also exercises public
`inspect_basic`; the parity harness checks that its complete page count matches
the full inspection and Pillow reference, covering the `basic` path in
`tiff/inspect.rs:47`.

The active TIFF row `error_lzw_invalid_first_token_checkpoint` checks the
invalid-first-code error through the token-aware public decoder as well as the
ordinary decode path. It is retained for that API parity contract, not claimed
as a new branch gain. A fresh one-row LLVM report,
`target/release-evidence/coverage-tiff-invalid-first-token-selected.json`, records
the invalid `code >= CLEAR` arm four times in ordinary decode and twice in
token-aware decode at `tiff/decode.rs:1872` and `:1965`. The RHS
`output.len() >= expected` is short-circuited and has no counter in this
selection. Coverage MCP reports the missing RHS outcomes in the one-row view;
this does not identify a missing public input, because the first code is already
invalid and a valid first code starts with zero output against a positive
expected length. The same fresh report lacks source/build receipts and test
attribution.

The final5 TIFF branch query has four remaining outcomes: the invalid-signature
arm in `parse_header` at `decode.rs:1005`, the output-full RHS in ordinary and
token-aware LZW at `decode.rs:1872` and `:1965`, and the empty-input arm in
`encode_lzw` at `encode.rs:723`. Public format dispatch rejects signatures
before `parse_header`; TIFF validation requires positive decoded dimensions and
skips zero-row compressed strips before LZW; and public encoder validation
passes nonempty pixel data to `encode_lzw`. These defensive arms have no known
public Pillow-compatible parity witness under the current APIs. This is a
source-level reachability assessment; the MCP report lacks source/build receipts
and test attribution.

### Public parity rows added — 2026-10-03

The active JPEG row `baseline_444_restart_marker_fill` adds three legal `0xFF`
fill bytes before the restart markers of a Pillow 12.2.0 baseline 4:4:4 JPEG.
Its input is 2,593 bytes (SHA-256
`c5369d29b879ca0d40ac01c927c4a5d845d3fc9f59ae07ccb7cb5947b0e5fee2`); its
2,835-byte Pillow RGB reference has SHA-256
`0276fe6a4734a34cee3009e2c6aca9c36323785712622759f3a8b9e835453895`. The
selected public parity run passes. The pre-row full report listed the true arm
of the repeated-fill loop at `jpeg/decode/decode.rs:2484`; the selected report
observes it, and the refreshed full report has no gap at that location.

The active AVIF row `no_intrabc_repeated_patch_monochrome_256` pairs the
existing monochrome IntraBC witness with an all-lossless frame that disables
IntraBC. Its input SHA-256 is
`34321df1afb253b459f20169ddc4d3b588e517b4832c2c2460d6900ca647c7a3`; its
196,608-byte Pillow RGB reference has SHA-256
`ed729898cd63496b4a1aa628686ebff2dd7a0f383dfdae854f7cd77dae54ce22`, and the
selected public parity row passes. Coverage MCP still reports the false arm of
`!header.superres_enabled` at `av1/frame.rs:3346`; this row tests the valid
IntraBC-disabled bitstream state and is not credited with closing that
superres branch. Selected and full reports have no source/build receipts and
unknown test attribution, so the branch observations are coordinate evidence.

The selected and refreshed full reports show no remaining branch gap at
`jpeg/decode/decode.rs:2484`, `av1/frame.rs:3647`, or
`webp/native/decoder.rs:226`. MCP incremental comparison is unavailable because
the full report's detailed baseline branch inventory is incomplete; no
four-metric union or receipt-verified causal attribution is claimed. The full
report still has 15,726 unobserved branch outcomes, so the 100% goal remains
open.

The existing WebP rows `enc_lossless_indexed` and
`enc_lossless_indexed_alpha` now also request public `encode_with_token` parity.
The 9,190-byte opaque Pillow reference has SHA-256
`fc35d688c00c25e149daee2fc8716ca3e6fcd671a67430db0169671c0dc39b34`; the
2,022-byte alpha reference has SHA-256
`5e8dfcc6d3fab8655405b166a951cbb1885400c7b6966945e112cc0473d0c339`. Both
token-aware calls match those exact bytes, and the indexed-alpha call also
retains its 65,536-byte RGBA reference (SHA-256
`f622a7e1dbe429f8414c6081c3d79c93573e37b18538dc828c497f13fa8b3659`). The
selected MCP report and refreshed full report no longer list either outcome of
`has_alpha` in the token-aware `expand_indexed` path at
`webp/encode/mod.rs:663`. MCP source/build receipts and test attribution are
unavailable, so this is coordinate evidence.

The active WebP error row `error_truncated_riff_extent_metadata_preflight`
adds a reproducible 16-byte RIFF prefix declaring 32 container bytes while
providing only the 16-byte header. Pillow 12.2.0 detects WebP and rejects the
input with `OSError: could not create decoder object`; the selected public
parity row passes. The matrix also repeats public still decode with a zero-byte
metadata limit and checks the resulting error kind and format against the same
Pillow error contract. The full Coverage MCP report no longer
lists a missing branch at `webp/decode.rs:193`, the declared-extent guard. MCP
source/build receipts and test attribution remain unavailable, so the
coordinate mapping is not receipt-verified.

The WebP decode row `lossy_vp8_segmentation_feature_data_disabled_16x16`
(input SHA-256
`b1fce88cb549bb4429cada0271752681530e8a86bc1c76256c57e581cf7c93ad`) matches
its pinned Pillow RGB reference and passes selected public parity. A Coverage
MCP comparison with the saved pre-row report observes the false arm at
`src/codecs/webp/native/vp8.rs:1177`; the refreshed filtered query has no missing
branch there. The reports lack source/build receipts and test attribution, so
this is coordinate evidence, not receipt-verified causal attribution.

The GIF encode row `enc_rgb_balanced_checkerboard_token_split` uses source
SHA-256 `d5045ccb8a0f05d35a07e300642c7501d4349af24aee2157b0b6886a0e3a32d4`
and matches the 78-byte Pillow GIF reference with SHA-256
`edcba5284f8c6f34ffb4de0f7c6012a90b7a650c3c693ac97efe6ad176c74f6d`. Its
selected public parity passes. The full comparison shows no new branch
coordinate: the false arm for `split > 0` at
`src/codecs/gif/encode.rs:1832` remains missing, while this input exercises the
false outcome of the later equality operand. The normal 22-row color
quantization partition also leaves that arm unobserved. Retain this row for
public token-encoding parity, not as a coverage gain; MCP source/build identity
is unverified.

The additional GIF row `enc_rgb_257_color_low_green_dominant` exercises a
skewed 257-color RGB distribution (source SHA-256
`221a0b156a315d1113a1d491c7fe497b21253a21ff17ad28c9ac7dd6f3c7c0e7`) and
matches the 1,124-byte Pillow GIF reference with SHA-256
`f63195630ba5ccc8c680101828adceb54944bfa35dd235d48ae7a7921cf070cd`. Its
selected public token-encode row passes. The full branch comparison shows zero
new coordinates, and the refreshed query still reports the false arm of the
`split < sorted.len()` condition at `src/codecs/gif/encode.rs:1814`. The selected
input reaches the final axis entry but exits through the median-threshold
`break`; treating the condition's false arm as unreachable for valid public
inputs is a source inference, not receipt-verified Coverage MCP evidence. Keep
the row for its exact palette-encoding parity value, not as a coverage gain.

The AVIF decode row `animated_lossless_inter_420_clipped_b32x32_10bit_28x28`
uses input SHA-256
`f9d829f20968483dbd078a47f136b9c413205ce78d423fb0b5f1bdb2d2c0cce4`. Pinned
Pillow 12.2.0 returns two 28×28 RGB frames at 100 ms each, with hashes
`925479a7e1eb832f5ce02848b2fb4df84769979f770e056ac398bf856cdcca1f` and
`e8e893f5c8a17b1d22653f7d92adfed2021901ab37d9bcbc72debccfba3645bd`; the
selected public parity row passes. The selected report still shows missing
outcomes in `decode_inter_transform_size` at `av1/entropy.rs:6927` and `:6938`,
so this row is retained for high-bit-depth clipped-frame parity rather than as
a coverage gain. The report has no source/build receipt or test attribution.

Three WebP encode rows cover compressed, raw, and equal-sized alpha candidates.
`enc_lossy_rgba_alpha_compressed` (source SHA-256
`bdf14b68c3d95d39080728931fee6002ddb27624e816df41a1120ba497ee2a81`) matches
the 140-byte Pillow output SHA-256
`9528b8edc7bbf5d2010bcaa1492f07feeef7d9004073c4ef3fdf8127f5a5a307`.
`enc_lossy_rgba_alpha_raw` (source SHA-256
`e1105dfac182e64e01ef20cd87bbe44ffa4a187666161e1d54858afaf947ed36`) matches
the 4,210-byte output SHA-256
`c3fed186ee97cc2418fd1815880646118e73fdaa3fed896bf977118972b13e4d`.
`enc_lossy_rgba_alpha_size_tie` uses a 9×1 constant-alpha RGBA source (source
SHA-256 `6e7e517108ef0e25c4f6c4257318ebf06f0abb2bea133804ea2417883e586d14`)
and matches Pillow's 100-byte output SHA-256
`f99e6b76bea04b1263728c6f4c6d275db57a7e9e1636887f6da8df0f1d84b594` through
both ordinary and token-aware public encode calls. The exact tie exposed a
parity bug: Pillow keeps the compressed candidate on equal lengths. Both Rust
paths now choose raw only when it is strictly shorter, retaining the existing
compressed buffer on a tie. The selected row passes with five public encode
calls and no panics. Coverage MCP reports both outcomes of the ordinary
Noop-writer size decision covered at `webp/native/encoder.rs:2750`; its
token-specialized sibling arms remain unobserved in that block because the
specialization receives `Some(token)`. MCP source/build receipts and test
attribution are unavailable, so the coverage mapping remains coordinate
evidence.

The AVIF matrix now includes two size-zero box cases. `size_zero_top_level_free`
appends a size-zero top-level `free` box and payload to the baseline (input
SHA-256 `0390a339020a54aa4ddb7600b988407c10efdc21ec0bdff762a793b7a1217e6c`).
Pinned Pillow 12.2.0 returns the same 128×128 RGB pixels as the baseline: 49,152
bytes with SHA-256
`f1a2555b1c61036af2bd1d3d125a6a1343993873d9e5d58e3caee79989095dcf`. The
`error_meta_nested_zero_size_ipma` case sets the final nested `ipma` box size
to zero (input SHA-256
`01ef1212f06626031f796c5268e49065a340ab0308a0cc7db4a0fdbafcfdbc65`); Pillow
rejects it as malformed, and its active public row passes. A Coverage MCP
coordinate comparison with the pre-case full report shows one additional
observation at `samples::next_box` (`samples.rs:264`), and the refreshed
filtered query reports no remaining branch gap there. The comparison has no
source/build receipt or test attribution, so it cannot bind that observation to
these rows. `avif/container.rs:449` still has an unobserved nested-size
rejection arm: public validation rejects the nested box in `samples::validated`
before reaching the duplicate container parser.
The `error_meta_duplicate_primary_item` AVIF row changes the baseline's nested
`iloc` FourCC to a second `pitm` without moving box data (input SHA-256
`3badc9ce02273d7ac0210aa6bcc3634e75cfed2531b30427bf721620452ab2c3`). Pillow
rejects the malformed meta box and the selected public row passes. Compared
with the full report captured before this row, Coverage MCP reports one new
observation at `samples::parse_meta` (`samples.rs:471`); the refreshed filtered
query has no gap there. The report comparison is coordinate-only because it
has no source/build receipt or test attribution.
The `error_meta_nonzero_version` AVIF row changes the top-level `meta` FullBox
version from zero to one (input SHA-256
`81cbc07cf4cad9653c58c47c7b88fcd3c323d7f749e2c47cbb39995a3a411b9b`). Pinned
Pillow rejects it as malformed, and the selected public AVIF row passes. The
current filtered Coverage MCP query reports no remaining branch gap at
`samples.rs:449`; source/build receipts and test attribution are unavailable.
Two more AVIF error rows relabel a later `meta` child to exercise duplicate
tables. `error_meta_duplicate_item_info` changes `iprp` to a second `iinf`
(input SHA-256
`51045cb251dc8cbbdfa245be7893f8fa54735e16414a71cb77abd0708e4f7aec`), and
`error_meta_duplicate_item_location` changes `iinf` to a second `iloc` (input
SHA-256 `8139b4019d1bdaad53622eaa137d0b8987b29b06a1d360df2c1dc0218d546f76`).
Pinned Pillow rejects both, and both selected public AVIF rows pass.
Compared with the saved report before these rows, Coverage MCP shows two new
coordinates at `samples::parse_meta` (`samples.rs:478` and `:499`); the filtered
queries have no remaining gaps there. The comparison has no source/build
receipts or test attribution.
The JPEG row `baseline_444_multiscan_duplicate_dqt_after_snapshot` duplicates
the 69-byte DQT marker between the first two component scans (input SHA-256
`a73504634df562811870132182af22730318fa2f4416347770499aa412c71e56`). Pillow
12.2.0 returns the 8×8 RGB reference (192 bytes, SHA-256
`983025493b251f11b379bbbf15a92cde4e5b9177e63626ff035e0049cc6779b4`), and the
selected public JPEG row passes. Coverage MCP reports one new coordinate at
`jpeg/decode/parser.rs:431`, with no remaining filtered gap. This comparison
also lacks source/build receipts and test attribution.
The JPEG row `baseline_444_multiscan_duplicate_dht_after_snapshot` repeats the
first DHT marker twice between the first two component scans (input SHA-256
`60ce19025b6c5e78ff18659ffea26dc97a0c628bb29dbc9c78c4d653c8c708f6`). Pillow
12.2.0 returns the same 8×8 RGB pixels as the three-scan source (192 bytes,
SHA-256 `0d4dccc69dc0ee4ae956be8c624186ddb2b5f3d411671361056c51a7ea7f9e66`),
and the selected public JPEG row passes. Comparison with the saved report before
this row shows three new branch coordinates at `jpeg/decode/parser.rs:442`;
the current filtered query has no gap there. This remains coordinate-only
evidence without source/build receipts or test attribution.
The TIFF parity matrix adds `tiled_gray2_deflate_short_tile` (input SHA-256
`1fb7f7d7d6a281cb667c365fe097bc868d841ec246783bd0b5014f4417127d8`) and
`tiled_palette4_oob_tile` (input SHA-256
`175f8b4e1c2df6e59d8a758892b8430ddc03cf23c24e26db752faff754435cb6`). The
first has a valid Deflate stream that expands to a truncated packed tile; the
second has a tile offset outside the file. Pinned Pillow opens both headers and
rejects pixel loading with decoder/truncation errors, and both active public Rust
decode rows pass the expected malformed-input parity. The full all-feature
coverage campaign passes with these rows in the matrix.

The `tiled_deflate_no_predictor` row adds a 128×128 RGB TIFF using Deflate tiles
without a Predictor tag (input SHA-256
`fddee4f848bfcde2bf9f8064016278ba0d8a755d7f0fc3598c756755e2399f73`). Pinned
Pillow 12.2.0 produces the exact 49,152-byte RGB reference with SHA-256
`8a1d6fcc36f5b5e70fbf949d4e612630a1931279383e7c21db27cd3cbad98131`; its active
public matrix row passes. The full local LLVM report records 64 false-arm hits at
`decode_ifd:454`, and the filtered Coverage MCP query no longer lists that
predictor branch.

The packed-tile helpers no longer repeat bounds and sample-width checks already
guaranteed by their callers; checked bit-offset arithmetic remains. Strict Clippy
and the full all-feature suite pass. Removing the two duplicated layout guards
reduced the local branch inventory from 32,264 to 32,254 arcs. Filtered MCP
queries no longer report gaps at `decode.rs:290`, `:454`, or `:1141`; they still
show one header-magic error arm and two LZW output-length guard outcomes. The
active `error_bad_ifd_lzw_invalid_first` row emits code 258 and exercises the
separate `code >= CLEAR` arm in both ordinary and token-aware public decode paths.
The remaining `output.len() >= expected` operands protect an internal state
which public TIFF dimensions and capped output writes do not allow. The header
magic arm is also unreachable through normal public dispatch: `detect_format`
recognizes only signatures whose magic `parse_header` considers registered.
These MCP locations remain coordinate evidence because source/build receipts
and test attribution are unavailable.

On this report, the filtered branch query
shows no missing branch groups in
`src/codecs/jpeg/encode/encode_baseline_422_block_row_streaming`. The selected
`enc_sub_422_streaming_exif_restart_rows` public parity row passes and its
selected report observes the true EXIF arm at `jpeg/encode/mod.rs:1958`; the
opposite arm remains unobserved in that selection. The report lacks
source/build receipts, so it does not bind the observed arm to a source
revision or prove test attribution.

The JPEG decode matrix now has 161 active rows. The new
`baseline_cmyk_aligned_entropy_truncated_tail_64` case uses a 32×8 MCU-aligned
CMYK image with 64 entropy bytes removed before EOI (input SHA-256
`51f4f76d8a6144572eb0880c5722f290840db23c6fa5a83148ac1a1947d7445c`). Its
exact 1,024-byte Pillow output has SHA-256
`89b4bd202579d6c4c3497dc124d1d45795ffb3dd74bb25a024a40d66f094c718`. Public
parity passes, and the selected report observes the incomplete-data arm at
`jpeg/decode/decode.rs:916`; the full Coverage MCP query lists no remaining gap
there. MCP source/build receipts are absent and test attribution is unknown,
so those findings remain coordinate evidence.

The `progressive_dqt_redefined_between_scans` JPEG row inserts an identical
DQT segment after the first DC scan of the 128×128 progressive fixture. The
5,579-byte input has SHA-256
`353d4f1b20a5b73e473deef0d401dd91a9d2973c6065b0dfddd005edf21785da`; its exact
49,152-byte Pillow output has SHA-256
`e92abdc1f9d14fd2491920de98bdcee028788715bebc6ba5d21b20d89b59fd76`. The
public JPEG decode row passes. Local full LLVM coverage now records three hits
for the previously unseen `!progressive` false outcome at
`jpeg/decode/parser.rs:431`; the filtered MCP query no longer lists that span,
though a later conjunction outcome on the same line remains uncovered. MCP
source/build receipts are absent and test attribution is unknown.

The `animated_lossless_inter_420_b8x16` AVIF row adds a two-frame 8×16 I420
sequence whose public decode selects a lossless B8×16 inter transform. Its
1,152-byte input SHA-256 is
`9e3fbac5e42c61413ec8c1f7f4f72bc1338fb7cfd21587e30826855732e369c0`; exact
384-byte frame references have SHA-256
`201fc960015c0e0974cb9b6aab74c9147694ddfd7876da531b696fc3639257e6` and
`aebfeaa0e20ab2e7aad44ca321ed0bdebaea085045a02954e55eb63505342659`. Both
frames have 100 ms durations, and the pinned native loop oracle records
infinite repetition. The selected public parity run and refreshed full local
report hit the `lossless_rect` and exact B8×16 outcomes at
`av1/block.rs:56172`. Coverage MCP still lists sibling outcomes and other
decoder instantiations at that line; source/build receipt and named test
attribution are unavailable.

The active `enc_animated_rgba_transparency_composite` row decodes a two-page
RGBA TIFF (input SHA-256
`445c160373a2391d3e090179cf9e5b3959382c1753b9ed762b0a653ec81bf5e6`) and
encodes both frames through the public GIF sequence API. Its 90-byte Pillow
reference has SHA-256
`e6fbf77ce827bd01c711a70ea9b48468b3ddf10dcca7bc9635e96eceeee63a7f`. The
focused GIF matrix test passes, and the fresh filtered Coverage MCP result no
longer lists the false arc in `composite_image` at `encode.rs:710`. Coverage
MCP's report remains coordinate-only because source/build receipts are absent
and test attribution is unknown.

The active `enc_error_oversized_dimensions` BMP row supplies a valid packed L1
image of width 2,147,483,648 (268,435,456 bytes) through the public encoder
matrix. Pillow 12.2.0 raises `OverflowError: signed integer is greater than
maximum`; the focused BMP matrix and full coverage campaign pass. The fresh
filtered Coverage MCP query no longer lists the width-failure arm in
`bmp_file_fits`. The height-failure arm remains unobserved: a valid 1×2,147,483,648
packed L1 image requires a 2 GiB pixel buffer, so the matrix retains the
existing Pillow error row without constructing that buffer. Coverage MCP has
no source/build receipt and unknown test attribution, so these locations are
coordinate evidence.

The active `enc_sub_420_q10_small_image_threshold` JPEG row encodes the
existing 17×17 RGB input (source SHA-256
`cb1632ddf154ea044863205d6b6f6e8249c70791a044debdc2b80b8fae92622e`) with
quality 10, baseline 4:2:0 subsampling, and optimization disabled. Pillow 12.2.0
produces a 690-byte reference (SHA-256
`ff93ff388fa786b630cff562f4052d31de74e9f4d2cdd076b1377f4cf83a8cba`). The
focused public matrix case passes exact byte parity. Its selected report is
`target/release-evidence/coverage-jpeg-q10-sub420-small-20261003.json`; the
full-report query at `jpeg/encode/mod.rs:713` now has no missing branch group.
The selected report observes the small-image false arm, but the MCP source and
test receipts remain unverified/unknown.

The packed FDCT encoder now multiplies all four rounded coefficient numerators
by their shared reciprocal with safe `u32x4::widening_mul`. Both the packed and
scalar paths apply the exact possible one-step correction by subtracting the
boolean as 0 or 1. The full public JPEG encode matrix passes exact Pillow byte
parity. Coverage MCP's filtered full-report queries list no missing branch
groups at `jpeg/encode/mod.rs:3042` or `:3212`; the report still has no
source/build receipt and unknown test attribution. Three five-round public
benchmark runs measured the 512×512 RGB quality-10 4:2:0 case 7.6–8.1% faster
than the scalar baseline, with identical encoded bytes and hashes. The
full-matrix geometric mean stayed within 1% of baseline, so this is a targeted
low-quality workload gain rather than a general JPEG speedup. Artifacts are in
`target/benchmarks/jpeg/arm64-goal-quantize-simd-{baseline-20261003-b,candidate-20261003-a,candidate-20261003-b,branchless-20261003-c}/`.

Coverage MCP still lists the true checkpoint arms at
`src/codecs/webp/native/encoder.rs:1378` and `:1418` as missing in this full
report. The experimental large lossless input did not observe either arm and
failed exact Pillow parity, so it is not a retained matrix case.
The preceding full report's filtered queries found no remaining branch gaps at
`container.rs:608` or `:622`, or at `jpeg/encode/mod.rs:2201`, `:2241`, and
`:2283`. An earlier filtered query on
`coverage-after-gif-parity-20261003.json` reported no remaining branch gaps in
the WebP grayscale/alpha scan. The grayscale RGBA input has 1,024 opaque
pixels before varying alpha, so its token-aware scan crosses one checkpoint
before selecting the per-tile predictor; alpha classification now shares that
grayscale traversal. Two 32×32 partial-alpha RGBA rows exercise the red/green
and green/blue early exits. Both match Pillow byte-for-byte at 942 and 954
bytes, respectively, with ordinary and token-aware public encoding. The opaque
grayscale shortcut remains available for fully opaque input.

The active `decode:gif:valid_frame_missing_trailer` row exercises Pillow's
acceptance of a complete one-frame GIF that ends at a top-level block boundary
without a trailer. Its ordinary and metadata-limited public decode paths match
Pillow's 1×1 palette image. The focused GIF matrix passes 80/80, strict lint
passes, and the full coverage run above passes. Coverage MCP's filtered branch
query reports no missing branch groups in `src/codecs/gif/inspect.rs`; the
report lacks source/build receipts, so this is coordinate evidence only.

Coverage MCP also lists AV1 `decode_following_vertical` at `block.rs:67554`,
but the `full_strict` outcome is unreachable through today's public decoder:
all four callers use `Lossy420Decoder` constructors fixed to 4:2:0. The true
arm at `block.rs:21111` is ordinary nonzero-plane decoding, already represented
by active I444 residual rows. Neither coordinate provides a distinct public
parity case; the report remains unverified coordinate evidence.
## Selected parity and coverage — 2026-10-03

The JPEG public decode matrix passes 174/174 active rows, including valid cases
where Cb and Cr use different sampling factors. Those cases exposed a panic in
the generic reconstruction path: it cropped Cr using Cb's plane dimensions.
Reconstruction now crops and upsamples each chroma plane from its own sampling
factors. The existing baseline 4:2:0, 4:2:2, 4:4:4, grayscale, and CMYK direct
decoders still return before this generic path.

The four selected `baseline_420_chroma_component_sampling_fallback` rows pass
4/4. Their LLVM report is
`target/release-evidence/coverage-jpeg-chroma-component-sampling-all-variants-20261003.json`.
Coverage MCP's older full report showed missing true arms for the Cb/Cr
horizontal and vertical sampling guards at `decode.rs:1170-1173`; the selected
report now observes those true arms. The false arm at line 1173 remains
uncovered in that selected run. This is coordinate evidence from reports with
no source/build receipts; MCP lists test attribution as unknown.

The selected 4:2:2 sampling, CMYK sampling, and restart rows pass 7/7. Their
LLVM report is
`target/release-evidence/coverage-jpeg-sampling-fallback-guards-20261003.json`.
Coverage MCP shows the selected 4:2:2 cases reach the component sampling
fallback guards at `decode.rs:1430-1433` and the CMYK cases reach the
non-1x1 sampling guard at lines 962-965. The report does not observe every
opposite arm in this selection; source/build receipts are absent and MCP lists
test attribution as unknown.

The AVIF `ipma_wide_property_associations` row passes its selected public parity
case (1/1). The selected LLVM report is
`target/release-evidence/coverage-avif-wide-associations-20261003.json`.
Coverage MCP reports 2,142/32,164 branches observed in that selected test and
leaves the false arm of `parse_ipma` at `container.rs:1077` uncovered. The
earlier full report listed the true arm at that coordinate as missing. This is
consistent with the selected row exercising the wide-association arm. Both
reports lack source/build receipts and MCP lists test attribution as unknown;
the incremental comparison is incomparable, so no combined coverage gain is
claimed.

The active `compression_lzw_lzw_long_phrase_checkpoint_1024` TIFF row adds a
Pillow-exact 525,825-pixel grayscale stream whose growing LZW phrase reaches
1,025 bytes and crosses the token-aware output checkpoint. Its input SHA-256 is
`0e85b9b6a8f064755ce9f1934d838c5d64c2e8fb5b453a2fe072ba347f2f0`; the Pillow
pixel reference SHA-256 is
`e32ad4269a71ed42a3d18578f1e3cbd8ec3a0b026b8215287483b0d47fa9e7bd`. The
selected TIFF matrix suite and full coverage run pass. Coverage MCP's filtered
query reports no remaining branch group in `append_lzw_with_token`; its
coordinate comparison observes the checkpoint's true arm once. The report has
no source/build receipt and test attribution is unknown, so this remains local
coordinate evidence rather than a managed Coverage MCP claim.

Coverage MCP still reports eight uncovered branch observations in WebP's main
`optimize_sampling` and one in its cross-color sampler, plus two observations
each in GIF's `split_median_box` and token-aware variant. A 16,320×576
token-aware WebP candidate reached 641 row comparisons but still missed the
main sampler's row-checkpoint true arm and column pass; it encoded to 222,328
Rust bytes versus 222,334 Pillow bytes, so it remains outside the parity
manifest. This report has no source or build receipt and test attribution is
unknown, so Coverage MCP comparisons are source-coordinate diagnostics rather
than verified coverage gains. For the GIF misses, a 2×1 two-color Pillow
control reaches the equal-value fallback, but public-path branch counters
confirm that the initial split loop's exhausted condition and the fallback's
`split > 0` false arm remain unobserved. With positive, nonsaturated per-color
counts, the midpoint loop breaks on the last color and the fallback stops at a
prior distinct color; no public fixture witness for either false arm was found.

The same full report has a PNG chunk-iterator stop observation. Its raw LLVM
branch table identifies the missing arm as `Chunks::failed`; the position-at-EOF
arm is already exercised by the active missing-IEND input. Every public chunk
consumer propagates a chunk error immediately, so none requests another item
after `failed` becomes true. TIFF's packed-tile extent checks follow validated
tile geometry and bounded source rows. BMP signature guards and ICO size guards
are preempted by public dispatch/source validation; reaching the BMP file-size
limit with a valid image would require impractically large pixel storage. These
remain defensive branches without valid public Pillow parity witnesses, rather
than reasons to add private coverage-only tests.

Coverage MCP's TIFF LZW gap at `decode_lzw:1888` is the right side of the
`||` check (`output.len() >= expected`). The active
`error_bad_ifd_lzw_invalid_first` row passes public parity but returns on the
left side (`code >= CLEAR`) before evaluating it. Successful output writes
return as soon as they reach `expected`, so the right-side true outcome remains
a defensive state without a valid public input witness.

The new `tiled_rgb1_packed_multisample_rejected` row preserves Pillow's
malformed-error parity for packed 1-bit RGB tiles. It does not reach the
`decode_ifd` tile-sample guard at `decode.rs:289-291`: `layout_and_palette`
rejects RGB below 8 bits earlier, and every currently supported packed layout
uses one stored sample. Its selected LLVM report records no branches at that
guard, so the row is an error-parity edge rather than a coverage gain there. A
fresh selected run passes 1/1 and is recorded at
`target/release-evidence/coverage-tiff-packed-tile-control-20261003.json`;
Coverage MCP likewise reports no observations at that guard or in
`copy_packed_tile_into`/`copy_packed_tile_row`. The report is selected-test
evidence with no source/build receipt and unknown test attribution.

Coverage MCP's remaining JPEG encoder groups include zero-dimension and
unsupported-sampling guards in `downsample_without_checkpoint`,
`downsample_two_to_one`, and `downsample_with_checkpoint`. Public image
validation rejects zero dimensions, and `JpegSubsampling` exposes only 4:4:4,
4:2:2, and 4:2:0; no-token 4:2:0 uses a separate pre-downsampled path. The
`encode_six_coefficient_major_edge_raw_blocks` helper also receives chroma
presence flags set to true at both public call sites, while its absent-luma
edge makes the uncovered `present[3]` outcome unreachable. Existing exact
parity rows cover the supported sampling and clipped-edge paths.

The JPEG direct-decoder bailout audit found a valid baseline multi-scan edge:
Pillow accepts a 4:4:4 image that encodes Cb, Y, and Cr in separate
non-interleaved sequential scans. The active
`baseline_444_three_sequential_scans` fixture is generated by splitting a
single 8×8 interleaved Pillow JPEG. The resulting file has SHA-256
`b9e3e277c0d05b2df169804236d5f9b72d3a1c8beda33e845f9c6ffd436f424b`, and its
decoded pixels match the original Pillow JPEG exactly (pixel SHA-256
`0d4dccc69dc0ee4ae956be8c624186ddb2b5f3d411671361056c51a7ea7f9e66`). The
active `baseline_444_three_scans_dqt_redefined` variant inserts a new
8-bit Cb quantization table between the first and second scans. Pillow's
decoded pixels differ from the original when the final table changes (input
SHA-256 `74d02103e151831846019cbcf009f2d17912988045e8d82e850828e18d1724f7`,
pixel SHA-256
`983025493b251f11b379bbbf15a92cde4e5b9177e63626ff035e0049cc6779b4`), so
each component latches its quantization table at its first scan; scans retain
their SOS Huffman and restart settings. The decoder handles non-interleaved
block geometry and resets DC predictors at scan and restart boundaries.
Single-scan baseline decoding keeps its existing dispatch and fast paths, and
the parser defers per-scan table copies until a later scan or table update
requires them.

Coverage MCP surfaced three additional Pillow-tolerated SOF0 cases. The
`baseline_444_multiscan_ss_nonzero` row sets Ss to 1 in the first sequential
scan; Pillow returns the same pixels as the ordinary three-scan input (input
SHA-256 `7a36ce48ba3d15bcefb15ad70b1a5e3f0c70523c84d2cdadade2ed6531e0094d`, pixel SHA-256
`0d4dccc69dc0ee4ae956be8c624186ddb2b5f3d411671361056c51a7ea7f9e66`).
`baseline_444_multiscan_duplicate_component` changes the final scan's component
ID from Cr to the already-seen Cb (input SHA-256
`76b4d1d36238ff9f92d63a44bb6dc1bcde040e99e0217f15b1c96f4a624668cc`, pixel
SHA-256 `2b73ccd3b62ddd2c645e63cd0d65ed016fc25a461d82e161514ac0d48fb2fb52`).
Pillow maps that scan back to Cb and leaves the omitted Cr plane neutral. Its
coefficient buffer retains earlier AC values where a repeated sequential scan
encodes zeros, so the Rust path stores coefficient history only for components
that repeat and merges the later block coefficients. This follows libjpeg-turbo's
multiscan coefficient buffers and Huffman decoder ([`jdcoefct.c`](https://github.com/libjpeg-turbo/libjpeg-turbo/blob/main/src/jdcoefct.c),
[`jdhuff.c`](https://github.com/libjpeg-turbo/libjpeg-turbo/blob/main/src/jdhuff.c)).
The `baseline_444_multiscan_missing_component` row omits the final Cr scan and
leaves its neutral plane initialized (input SHA-256
`d91a46924a2310aae6a9b7c16fd7f091eaf21bff8a8b74dc99c388d1983e6a8a`, pixel
SHA-256 `669b0f32cd296cd0742d7e9b8e9c25cf7a4f114b6a5788f83af5ed0e89f409b7`).
The `baseline_444_multiscan_empty_entropy` row removes the first component
scan's entropy while retaining its SOS; Pillow accepts the file and leaves that
component neutral (input SHA-256
`f8bba1d8f8e8f631f5487349550f64a3bd3438a9789ed293619d7e2553be6192`, pixel
SHA-256 `1b343e02a958d0137fd239618dd0332265457fc011296714fe1a42cf0e35d49d`).
`baseline_444_multiscan_interleaved_restart` combines Y and Cb in one scan,
leaves Cr in a singleton scan, and places a restart marker between its two
MCUs (input SHA-256
`ddbcf29eb1549aae0356661ff2b879b7b2b23b7aae924adbb424af22246e20d7`, pixel
SHA-256 `f83545d43c6939ec393b6b8310959b6174fd764b08a12fc22d908408a7e6a43e`).
The `baseline_444_multiscan_restart_overrun` variant adds two trailing restart
markers; the first creates an empty segment after the final MCU. Pillow accepts
it and returns the same pixels (input SHA-256
`1aac34523a7e6491744764d84c9c0de21ef9a308d532924e121c7c36229defe6`, pixel
SHA-256 `f83545d43c6939ec393b6b8310959b6174fd764b08a12fc22d908408a7e6a43e`).
`baseline_444_multiscan_duplicate_component_dc_only` repeats Cb with blocks
containing only DC coefficients, reaching the repeated-component DC-only path
(input SHA-256
`3a316bfffa1834975e775d7a4dd62f0766919eef85bdd5a44f0401f12f17f429`, pixel
SHA-256 `fa7b78cc215df21d7ce54d8c3c6637c326dab95c10fbc12263101365973f4268`).
All seven tolerance cases match Pillow. The JPEG decode matrix passes 188/188.
The active `entropy_all_ones_huffman` row is a grayscale baseline JPEG whose
SOS prefix is followed by two stuffed all-ones entropy bytes and EOI. Pillow
12.2.0 decodes it; the input SHA-256 is
`bef219c962289debcec499023a86acf162a5e51c0ccd78309162f845a2544456`, and the
exact 128×128 L8 reference SHA-256 is
`f9db8abaac585e058a892932da246a2cd05aec42b862dd931ffee9891577c4de`. The
fixture reaches the fast decoder's overlong-code sentinel with length 17.
Coverage MCP no longer lists the overlong-length guards in either the scalar
or fast decoder in the current full report. The active
`empty_dc_huffman_table` row starts from `sampling_3x1.jpg` and replaces DC0's
DHT entry with a zero-symbol table. Pillow 12.2.0 decodes the resulting 617-byte
input (SHA-256
`c28fa453e06196681ad2815bfe6dd320ee2239e19d9efba3999d0bdc9c5c8d14`) to a
24×8 RGB image (pixel SHA-256
`65f0bcf872b036301186859680a42be71467f6fd402c47ab2cc20c68e10dee44`). Rust
now follows IJG's fake-zero recovery for scalar and fast overlong codes while
preserving the error for its internal invalid-table sentinel. The empty-table
fixture reaches scalar `decode_slow` at length 17. Remaining filtered gaps are
fast `ensure` true at `huffman.rs:373`, fast `values.is_empty()` true at line
390, scalar `ensure` true at line 329, the invalid synthetic sentinel at line
349, and the missing-symbol arm at line 360. MCP reports source/build identity
as unverified and test attribution as unknown.
Coverage MCP reported remaining unobserved branches in
`reconstruct_baseline_multi_scan`; its local report has no source/build receipt
and unknown test attribution, so it is coordinate-level guidance rather than a
managed coverage claim. A header-only SOS component-ID swap remains rejected by
Pillow as a broken data stream; no parity case is claimed for that mutation.
The final full `make coverage` run passes all tests and repository floors:
86,169/142,520 lines (60.4610%), 16,492/32,268 branches (51.1095%),
4,810/8,989 functions (53.5098%), and 131,806/220,404 regions (59.8020%).
Coverage MCP now reports one branch at `decode.rs:2268`: an empty extracted
segment set with nonempty scan bounds. The JPEG parser ends each entropy range
at the first non-stuffed, non-restart marker, so this error arm appears
unreachable through manifest-visible inputs. The LLVM report has no source or
test receipt; MCP marks both as unverified/unknown.

Coverage MCP also pointed to a valid baseline JPEG with a DQT marker after its
only SOS entropy scan and before EOI. Pillow 12.2.0 accepts the full 8×8 4:4:4
input and decodes it identically to the source without the trailing marker
(input SHA-256
`1b4519e48e1ce711cd1cbb44c3173ef3c97f222b8aa1686f9437df5e5f906372`, pixel
SHA-256 `7382706a9ecb0f6acae26e7c1c9a9be0e9471233c1a0c0da48d08a458115be2f`).
The public `baseline_444_single_scan_trailing_dqt` row exercises this case.
Single-scan reconstruction now stops entropy extraction at the scan boundary
and uses the quantization-table snapshot captured before the marker. Pillow
also accepts an empty trailing DQT (input SHA-256
`bedd3492ec593158787385c58ec04055ecedd0beb334d1dbcae55716cb3ed9a3`) and
marker fill before a nonempty trailing DQT (input SHA-256
`6f4e22755f714d4fe3dab67e8d9e0154abc33cd80714ba4cd7ba4792abfa18e4`); both
produce the same pixel hash above. The public
`baseline_444_single_scan_empty_trailing_dqt` and
`baseline_444_single_scan_dqt_marker_fill` rows cover those forms. The code
keeps the existing single-scan dispatch and fast paths. The focused public rows
and full JPEG decode matrix pass; these are previously unsupported valid
inputs.

Coverage MCP identified the false arm of the second comparison at
`src/codecs/jpeg/decode/huffman.rs:410`: a valid AC DHT has the standard luma
code counts but a different symbol order. The public
`baseline_444_unused_custom_ac_table` fixture inserts an unused AC table 2 into
the complete `baseline_444.jpg` input, swapping the first two symbols assigned
to one code length. Its generator pins the source and generated-input SHA-256
values and verifies Pillow 12.2.0's decoded RGB SHA-256. Pillow accepts the
9,329-byte input as 128×128 RGB and preserves the baseline pixels exactly;
Rust matches all 49,152 Pillow bytes. The selected public matrix row passes
1/1. Its LLVM branch report changes the targeted arm from zero observations
for `decode:jpeg:subsampling_444` to four observations for the new row. The
Coverage MCP selected-test comparison reports the line-410 arm as newly
observed, but remains limited because source/build receipts are absent. The
selected report is `target/jpeg-coverage-probe/coverage-public-row.json`; this
is coordinate-level evidence, not a full-suite coverage claim.

## JPEG direct-safe SOF component-count invariant — 2026-10-04

Coverage MCP also showed missing true arms for `components.len() != N` in the
grayscale, CMYK, 4:2:0, 4:2:2, and 4:4:4 baseline direct-safe paths. Those arms
are unreachable from public JPEG inputs: `parse_sof0` builds the component
vector from the SOF count, the parser derives `num_components` from that vector,
and its sole `JpegInfo` construction stores both values together. The five
redundant hot-path checks were removed and the invariant is documented on
`JpegInfo`; no private unit tests or coverage exclusions were added. The public
JPEG decode matrix passes 203/203 rows. The selected LLVM report is
`target/release-evidence/coverage-jpeg-direct-safe-guards-20261004.json`; MCP
has no source/build receipts, so it is row-local evidence and does not support
a verified aggregate coverage-gain claim.

Coverage MCP also lists repeated AV1 specialization observations in
`decode_inter_translation_impl` and `decode_following_vertical`; specialization
counts are not distinct source branches. A selected run of the clipped
`animated_lossless_inter_420_clipped_b32x32_64x60` row passes public parity and
reaches the true outcome at lines 54836–54838 in one of eight compiled
`decode_inter_translation_impl` specializations. Its report has no source/build
receipts, so retain this as coordinate evidence.

The 32×32 `animated_lossless_inter_420_b32x32_10bit` row records no branch
counts at lines 54836–54845 across eight compiled specializations. A separate
selected run of
`animated_lossless_inter_420_b32x32_10bit_64x64` passes public parity and
records six false outcomes at `block.rs:54838` (`tools.sample_depth ==
SampleDepth::EIGHT`) in one specialization, with the preceding 32×32
`LosslessGrid` match reached. This existing full-size case already witnesses
the sample-depth false arm. The 28×28 10-bit clipped fixture uses B8×8 leaves;
high-depth clipped B32 geometry is rejected by the current planner before
this block evaluator, so it does not need a new fixture to cover the false arm.
The selected 64×64 coverage report is
`target/release-evidence/coverage-avif-10bit-64x64-target-20261004.json`.
Coverage MCP still lists other missing specialization observations, and its
incremental comparison with the full baseline is incomparable because exact
baseline branch detail is incomplete; this does not claim a marginal full-suite
coverage increase.

`decode_following_vertical` has no observed calls in this report; its
production caller requires a lossy 4:2:0 16×16 split in a closed frame
context. A 16×16 q76 noisy-quadrant AVIF probe decoded in Pillow but returned
`NotImplemented` from the Rust public decoder, so it was not added. No valid
Pillow-parity witness for this remaining gap was established. The reports have
no source/build receipts; Coverage MCP marks source unverified and test
attribution unknown, so these are source-coordinate diagnostics only.

Coverage MCP reports no remaining branch gap in `decode_portable` at lines 492,
496, 535, 555, 588, and 606 after adding 10-bit alpha 4:2:0, 4:2:2, and 4:4:4
public parity rows. It reports no remaining gap at
`webp::inspect::validate_animation_frame` line 311 or in the token-aware GIF
coalescing group. These are source-coordinate diagnostics; the report has no
source/build receipt and marks test status unknown.

Seven public limited-range monochrome parity rows now cover RGB and auxiliary
alpha output at 8, 10, and 12 bits, plus BT.2020/PQ CICP. Their selected rows
match exact Pillow output and the full AVIF decode matrix passes. The CICP row
shows monochrome admission no longer needs a 1/13/6 color declaration. Coverage
MCP reports no branch gaps in `limited_luma_to_u8`; remaining AVIF groups are
plane-shape guards at `decode_portable` lines 457–463 and
`decode_monochrome_portable` lines 676–684, plus the false
`subsampling_x/subsampling_y` check at line 410 of
`monochrome_primary_sequence_supported`. The shape guards follow validated
plane assembly, and AV1 monochrome sequences have fixed 4:0:0 subsampling, so
these arms have no known public encoded-input witness. The report is
unverified for source/build identity and test attribution.

The active `enc_rgba_octree` and `enc_rgba_octree_sorted` GIF rows now also
call the public `encode_with_token` API on their existing RGBA inputs. Each
token-aware result is compared byte-for-byte with both the exact Pillow
reference and ordinary Rust encoding. Both selected parity rows passed, and
Coverage MCP reports no remaining branch gaps in
`gif::encode::apple_qsort_buckets_with_token` in the refreshed full report.

The active `animated_lossless_inter_420_clipped_b32x32_185x64` AVIF case adds
a 185×64 two-frame, lossless 4:2:0 B32 edge input. Its final block has 25
visible columns; the input is pinned by SHA-256
`701d247ebbe2666b1f28f447fa1a086573e7dc6d88b0b29405f9f7a6f9460b59`, and its
two Pillow RGB frames are pinned by SHA-256
`ac76c404cf3001f4e087307516e596223e3e5f1c26d7eaf8860bc710b88761a1` and
`336f948700529f81199b9d8fb0adcc8da79dcc00b06ffffa5b66d5dee255199e`. The
selected parity row and full AVIF matrix pass. This edge does not make the
AV1 partial-grid false arm observable: the relevant decoded extent is MI
padded to a multiple of four.

The active `animated_lossless_inter_420_clipped_b32x32_64x60` AVIF row adds a
two-frame bottom-edge crop case with exact Pillow frame parity. Its selected
LLVM run reports `selected=1 active=1` and passes the row. The report records 11
calls to `inter_lossless_grid_clipped_b32_geometry_supported`; one B32x32 call
passes the 32-pixel width check and takes the 28-pixel visible-height arm.
`decode_inter_transform_size` runs 11 times, but the later clipped-plan arm at
line 6927 remains unobserved in this selected run. Coverage MCP marks the
report's source unverified and test attribution unknown, so these local
counters are scoped diagnostics, not managed coverage attribution.

Two active TIFF parity inputs set the separate-planar RGB tile width or height
to zero. Pillow opens and verifies both headers, then raises
`ValueError: tile cannot extend outside image` while decoding pixels. Their
input SHA-256 values are `f8265499d2ee3d4e63e9525a76e70aa88cd9840891267c47aaa840928056617a`
and `1dc16a0bd0c6b6574ae38c9ed163bf931e343736e41a41f2d81a24331bd08cc0`. The
public parity run matches all four lifecycle results. These inputs cover both
zero-dimension exits in `verify_separate_planar_offset_count`; the remaining
mismatched-tag operand was removed because the preceding directory check
rejects that state before this helper runs. Coverage MCP's filtered query now
returns no branch gaps for the helper, with the same source/build and test
attribution caveat noted above.

The JPEG encode matrix now has 74 passing active rows. The new
`enc_sub_444_unaligned_subthreshold` and
`enc_sub_444_aligned_subthreshold` rows pin exact Pillow bytes for 4:4:4
inputs below the row-streaming size threshold; together they exercise the
three-component independent-entropy predicate and the dimension-alignment
alternative. The token-aware GIF LZW and BMP raw-pixel paths also run through
public parity: GIF passes 79/79 decode rows and BMP passes 106/106. The
selected report `target/release-evidence/coverage-public-parity-edges-20261003.json`
records the JPEG three-component arm and both token-aware BMP read paths. Its
aggregate covers only the four selected rows, and Coverage MCP reports no
source/build receipt and unknown test attribution. A seven-row token-aware GIF
selection passes 7/7, covering the existing patterned frame, four LZW boundary
cases, and invalid minimum code sizes on both sides of the accepted range. Its
report is
`target/release-evidence/coverage-public-gif-token-lzw-edges-20261003.json`;
Coverage MCP reports no remaining branch observations at the invalid-size
guard in `gif/decode.rs:556` or the phrase traversal in
`append_code_with_token` at line 695 for this selection. The full GIF decode
matrix passes 79/79. This report also has unverified source/build identity and
unknown test attribution, so it supports a selected-path observation rather
than a full-suite coverage claim.

The VP8L structural inspector now follows nested `VP8L` chunks in `ANMF`
frames when no top-level bitstream exists. The property map records the nested
16×16 frame header and color-indexing transform for
`pillow_tolerated_malformed_animated_vp8l_anmf_width_mismatch`; the malformed
outer width remains covered by Pillow parity, not by the VP8L parser.

## Public parity coverage edges — 2026-10-02

Active Pillow parity cases target codec edges surfaced by Coverage MCP.
The WebP row `error_malformed_container_animated_frame_bottom_outside`
complements the existing horizontal frame-overflow case by moving the ANMF top
offset down two pixels. Pillow 12.2.0 reports
`OSError: could not create decoder object`, and the selected public parity row
passes. The isolated LLVM report
`target/release-evidence/webp-frame-bottom-outside-isolated.json` records four
false outcomes for the horizontal operand and four true outcomes for the
vertical operand at `webp/inspect.rs:311`. This is selected local evidence only;
it has no managed Coverage MCP source/build receipt or test attribution, so it
does not establish full-suite coverage.

The `fdat_without_actl_before_idat` PNG case uses a valid-CRC `fdAT` chunk
before `IDAT` with no `acTL`; Pillow rejects it during inspection, and Rust now
rejects it in the independent PNG inspector as well as the decoder. Its input
is pinned by SHA-256
`6d1a1d4a5fb1e4185b39fe44bd13e489d9a58680b3a5f7490ffcad4854148c4d`. The
`apng_truncated_fdat_before_idat` case puts a three-byte `fdAT` after valid
`acTL` and `fcTL` chunks but before `IDAT`; Pillow raises
`ValueError: APNG contains truncated fDAT chunk`. Its CRC-correct fixture is
pinned by SHA-256
`4f34e6caa28789baa349f5b3ada75f0ad7aae29f617717da9dffd6f906d2312b`. The
PNG parity matrix passes all 175 active rows, including both malformed cases
and the existing valid sequence-only APNG case.

An earlier all-feature `make coverage` run passed every suite and repository
floor with
`target/release-evidence/coverage-full-webp-tiff-jpeg-20261002.json`:
85,519/142,085 lines (60.1886%), 16,254/32,178 branches (50.5128%),
4,788/8,972 functions (53.3660%), and 130,592/219,584 regions (59.4725%).
The PNG parity harness decodes each successful row through `decode_with_token`
and compares its exact pixels with Pillow. The existing WebP `enc_exif`,
`enc_xmp`, `enc_icc`, and `enc_lossy_alpha_exif` cases now also run through
`encode_with_token`; the harness checks exact bytes against Pillow and the
ordinary Rust encoder. The WebP decode parity lane passes all 230 active rows.
Four complete VP8X mismatch files exercise width and height rejection for both
VP8 and VP8L, including `inspect_basic`. The three new files derive from
valid generated WebP inputs by changing only one 24-bit VP8X canvas dimension;
Pillow reports `OSError: could not create decoder object`, and Rust reports a
structured malformed-image error. The VP8L decoder now checks its signature,
dimensions, and version in the frame header and returns typed errors rather
than panicking on malformed input. These checks run once before pixel
reconstruction; the pixel loops and SIMD kernels are unchanged. Coverage MCP's
filtered branch queries no longer list missing branches in
`inspect_extended_basic` or `LosslessDecoder::read_frame_header`. Other groups
remain in WebP inspection and lossless decoding. It still
reports the false arm of `encoded.len() >= 30` at
`src/codecs/webp/encode/mod.rs:912`; valid public output already contains the
12-byte RIFF header and 18-byte VP8X chunk, so that shorter-buffer arm is
unreachable there. The AVIF query still reports missing branch observations at
`src/codecs/avif/av1/block.rs:54833`; the 10-bit fixture validates its sample
but does not complete sequence surface reconstruction. These Coverage MCP
queries are diagnostic: the full report has no source/build receipt and MCP
reports test attribution as unknown. The 100% coverage goal remains open.

The new `lossless_copy_out_of_bounds` WebP decode row reaches the lossless
copy-length bounds rejection: a singleton VP8L copy asks for 25 pixels when
only 16 remain. Pillow reports `OSError: failed to read next frame`; the Rust
decoder matches the malformed-input outcome. Its 8×4 fixture is pinned by
SHA-256 `abb0a3d2b630b23fc5bdd476bb59e9b7790a461ae60522ddc282fd53b27ef075`.
`enc_wide_scanline_checkpoint` uses a 1,025×1 RGB BMP row to reach the
post-pixel checkpoint at index 1,024 in `bmp::encode::write_rows`; its exact
3,130-byte BMP output is pinned in `tests/fixtures/outputs/jsons/Encode.bmp.json`.
`enc_lossless_palette4_box_chain` uses a 4×4,100 four-color WebP source whose
packed palette stream reaches the 4,095-sample box-chain match boundary; its
exact 62-byte output is pinned in `tests/fixtures/outputs/jsons/Encode.webp.json`.
`enc_sequence_raw_final_aligned` uses two 4×3 RGB TIFF pages and produces an
exact 352-byte encoded sequence ending on a 16-byte boundary.
`enc_animated_reserved_disposal` re-encodes a two-frame GIF with disposal
values 4 and 2; its exact 13,328-byte Pillow output exercises the GIF writer's
valid `FrameDisposal::Reserved(4)` conversion.
`enc_sub_420_aligned_full_mcu_rows` encodes a 32×32 RGB source through the
complete 4:2:0 MCU-row converter; its exact 1,264-byte Pillow output is pinned
with SHA-256 `644ec00d071d47f21042a0548a7c2d89387c43039be6af375f7b087ef7c0f3a0`.
`lossy_vp8_delta_segment_values` decodes a four-segment VP8 frame whose
quantizer and filter strengths use delta syntax. Stock libwebp always emits
absolute segment values, so `scripts/build_webp_delta_cwebp.py` patches a
temporary copy of pinned libwebp 1.6.0 and
`scripts/generate_test_assets.py` verifies the resulting stream against its
byte and decoded-pixel hashes. Pillow 12.2 decodes the delta and stock
absolute-mode streams to the same 49,152 RGB bytes. The input PPM is pinned by
SHA-256 `9efd98332fada058f0ec106ac5c714b7898d640379b3739a22a1185397604421`,
the WebP asset by `9f6a1b274d9faf180fd024b962b068d6c4db5fe29e619868e7e4f9e91f12adbb`,
and decoded RGB by
`6b2c202c55d0bf70d9f32b74c1b8705232ae41e0ed24203ea2b1b138dfd914fe`.
The `portable_lossless_monochrome_alpha_17x17` AVIF row adds a 17×17 lossless
monochrome color item with an auxiliary alpha plane. Its 900-byte input is
pinned by SHA-256
`573db25044c9dcbb03f00d623d20a30639d3334ba35feae6d57f115e208741bd`; Pillow
returns exact RGBA pixels in 1,156 bytes with SHA-256
`d0201688c025cda34f274e3187240f35090e3366cb8883d3a3f04c45d03e0319`. The
case targets the `has_alpha` path in `decode_monochrome_portable`; its primary
and auxiliary AV1 items are both monochrome. The AVIF decode parity matrix
passes all 402 active rows. Coverage MCP no longer lists the true arm at
`src/codecs/avif/decode.rs:699` in the refreshed full run; source identity is
unverified and test attribution is unknown, so this is diagnostic coverage
evidence alongside the local parity result.

The `bmp_8bit_default_palette` ICO row sets the embedded 8-bit DIB's
`colors_used` field to zero while retaining its full palette. Its 1,374-byte
input is pinned by SHA-256
`4915bb486fdd9b56d0bb200d43dfeb80efe82762a6e02485390b22cabc7f22ad`; Pillow
returns 16×16 RGBA pixels in 1,024 bytes with SHA-256
`d3b82ecad923e39c5007fdae2b42acb59a6dd9d36419a4e1329d53859b824ff5`. The ICO
decode parity matrix passes all 55 active rows. Coverage MCP's refreshed full
report no longer lists the true arm at `src/codecs/ico/decode.rs:465`; source
identity is unverified and test attribution is unknown, so this is diagnostic
coverage evidence alongside the local parity result.

The `enc_cmyk_small_unaligned_fallback` JPEG encode row uses the existing
13×9 CMYK source, below the streaming threshold and outside its alignment
constraints. It exercises the baseline generic encode path and matches Pillow
12.2.0 byte-for-byte: 634 bytes, SHA-256
`0d8885a6009b46e1f8f066fe90abadf5dac3153dcd11c5c63a8f62c15bf9b03a`.

The `enc_cmyk_aligned_subthreshold` 32×8 row takes the aligned baseline
streaming path with only 256 pixels, below the 1,024-pixel threshold; its exact
759-byte Pillow output is pinned by SHA-256
`226d495545b556a72558ba6f9b768ef382372b2729eaa6481410313426c18fba`. The
`enc_cmyk_aligned_width_partial_height` 32×9 row reaches the aligned-width,
partial-height fallback; its exact 809-byte output is pinned by SHA-256
`aa5e354fe0ae1c8d072642ea27501f72252648fc1bb52a03bf3ff61d1730709c`. Together
with the 13×9 unaligned row, these three cases cover both alignment outcomes
inside the baseline CMYK dispatch expression.

The `enc_cmyk_optimized` and `enc_grayscale_optimized` rows take the generic
optimized entropy paths; Pillow's exact outputs are 10,325 bytes (SHA-256
`7bb93dc5c122a85aa555e03c9912da5f22d8f5377226d7cb16ceb9af84070d4e`) and
3,185 bytes (SHA-256
`5612f6c2f016150cde09d9d35f336c8cf9115178772bff3d143a64804908b9ff`),
respectively. The JPEG encode parity matrix passes with all five rows active.
Coverage MCP no longer lists branch gaps at `src/codecs/jpeg/encode/mod.rs`
lines 607, 608, or 631 in
`target/release-evidence/coverage-full-jpeg-optimized-paths-20261002.json`.
That full report passes every suite and repository floor: 85,184/142,042 lines
(59.97%), 16,037/32,156 branches (49.87%), 4,780/8,972 functions (53.28%), and
130,166/219,556 regions (59.29%). Its source/build identity is unverified and
test attribution unknown, so the branch results are diagnostic evidence beside
the local parity results.

The `error_malformed_structure_apng_fdat_payload_before_idat` PNG row places
frame-data payload after an `fdAT` sequence number before the default image's
`IDAT`. Pillow raises `OSError: broken data stream when reading image file`
when decoding and `SyntaxError: broken PNG file (chunk b'\x00\x00\x00\x01')`
when verifying. Its input is pinned by SHA-256
`07d84a0ef147069484e49bdb3a63aca3a28d5e863d6af033708c3b186434d783`. Parity
exposed a static PNG decoder path that previously ignored this invalid APNG
state; the decoder now validates the sequence-only exception and rejects early
frame payload before IDAT.

The `compression_deflate_deflate_predictor_checkpoint_343x2` TIFF row uses
343×2 RGB pixels with Deflate and horizontal Predictor 2. A 1,029-byte scanline
reaches the predictor's 1,024-byte cancellation checkpoint. The 194-byte input
is pinned by SHA-256
`1a0c5c7ce1a80fa071619c138e8702854ece6d7101fcd16cb93db4b8de899cdf`; Pillow's
2,058-byte RGB reference has SHA-256
`ab69d4afd329ce8d28339663557fdc4a3c66844a0ddde05f87bdf61f1b75574f`.

Two AVIF malformed rows cover required `meta` child validation:
`error_meta_first_child_not_hdlr` changes the first child box away from `hdlr`,
and `error_meta_hdlr_non_pict` declares a non-picture handler. Pillow raises
`UnidentifiedImageError` for both. Their input SHA-256 values are
`21062fa34dab7a259cc7aa47f7960a5c2a0b1b39d28396f361c46c5e4a754a9c` and
`39d963ae504465535d09d7341de78c1788a752cc528ff5d888a873a5478e85e4`.

`error_meta_missing_primary_item` and `error_meta_missing_item_info` retag the
respective required `pitm` and `iinf` children as `free`. Pillow raises
`RuntimeError: Failed to decode image: Missing or empty image item` for both;
their input SHA-256 values are
`e7eb26add7703e8973c243e22f7b9225ead5292e8e1f051a09ada938824c0ffa` and
`8551acfec0234028ec989a52f1ceea4ff5666a3bc929e00a5a4946f902ae6dc6`.
`error_meta_missing_item_properties` retags the required `iprp` child as
`free`; Pillow raises `UnidentifiedImageError`, and the input SHA-256 is
`5339cf9635130eee4dcfd7801dd25563b591332b243b137b077a67576ee69436`.
`unknown_extended_size_uuid_box` appends a valid top-level extended-size UUID
box to the 64×64 I444 fixture. Pillow returns the same 12,288 RGB bytes as the
base image; its input SHA-256 is
`270ba9556bd21bd85c56b5f7609a5d48e9a255ffc3ec4d1b1fa4c1885a70847d` and its
pixel SHA-256 is
`5e74b862ca69314d8ae9a3aa4b8f3af5651dfbd238ce922a7973c193e338f64a`.

`error_bad_ifd_zero_tile_height` sets TIFF `TileLength` to zero while retaining
a nonzero width. Pillow opens and verifies the header, then raises
`ValueError: tile cannot extend outside image` on decode; the input SHA-256 is
`6c83b3e663f810d18db3d9a6a623eeac6ee7f21407d9dd22ad7084f0f626e208`.
`enc_grayscale_small_fallback` encodes a 13×9 grayscale JPEG source below the
streaming threshold and outside its alignment constraints. Its exact 422-byte
Pillow output is pinned by SHA-256
`05fae511efea0f239c6d7c59ee24d0e52f993f6d6581ee842b2397ec06cb3442`.
`enc_grayscale_aligned_width_partial_height` and
`enc_grayscale_aligned_subthreshold` encode 32×1 and 32×8 grayscale sources.
Both stay below the 1,024-pixel threshold with aligned width; their heights
take the false and true outcomes of the 8-pixel alignment check. Pillow's
exact outputs are 349 bytes (SHA-256
`1467cdfb18e2f59ce73814ebfeb2882f6bef77c97990b4b16c3f4182e217b241`) and
465 bytes (SHA-256
`29c71e3206d0eeebe5428504e2432d5810631508a82d761f7fd3db9fbab3ffec`).
`enc_sub_422_aligned_width_partial_height` and
`enc_sub_422_aligned_subthreshold` encode 64×1 and 64×8 RGB sources with
4:2:2 subsampling. They stay below the 1,024-pixel threshold with aligned
width; their heights take the false and true outcomes of the 8-pixel alignment
check. Pillow's exact outputs are 691 bytes (SHA-256
`4afe5fc617557765f13a6806683d757d6bdab6815240556c4192f4f357dc7ea5`) and
1,026 bytes (SHA-256
`3c6dcb8056b151eb832db29ec8a337e4edaf76a423ba0aefdf4573466d557a01`).
The JPEG encode parity lane passes all 72 active rows.

The `palette_absent_animated_no_palette_second_frame_no_local_table` GIF row
removes the second frame's local color table while retaining its two-frame
palette-less structure. Pillow returns an 8×8 luminance image (64 bytes) with
SHA-256 `c2a74daea21f6caad6b7794dcfd121cbaa0bddc3407ce5f5fdc1914bd5e7ff90`;
the input SHA-256 is
`440eb36917bab98cd88a53534d87aadb611865999898bc9bbc3503a62a2c9654`.

The `error_malformed_short_dib_payload_below_eight` ICO row contains a
seven-byte embedded DIB payload, exercising the short-payload rejection before
the DIB header can be read. Pillow raises `OSError: Truncated File Read`; the
input SHA-256 is
`585f1feefb2f364c4ecb0b50ac8e3ff414771fde088ec23fe39b44e652caa981`.

The `incomplete_image_data_no_iend` PNG row contains a valid IHDR and IDAT
chunk, but its truncated zlib stream does not provide the complete image and
the file ends without IEND. Pillow raises `OSError: image file is truncated`
when loading it (and `OSError: truncated PNG file` during `verify`). Its
1×2 fixture is pinned by SHA-256
`b4c5b83118403fbff8bfa3fe0d4d85f094a79e21a8421de87bd7695c9b2e56e7`. The PNG
decode parity matrix passes with the row active, and Coverage MCP no longer
lists the no-IEND `NeedMore` mapping at `src/codecs/png/decode.rs:678` in the
refreshed full report. Source identity and test attribution remain unverified.

The public parity matrix compares every eligible successful non-direct-still
row through its shape-matched sink API: stills through `encode_to_sink`, and
animations plus all TIFF cases through `encode_sequence_to_sink`. Each sink
result must match the same pinned Pillow bytes and the regular encoder result.
At the time of the selected LLVM report below, the public matrix had 1,334
active decode rows across eight format tests and 386 active encode rows across
19 `coverage_matrix_tests` functions. All 27 public decode/encode matrix tests
passed. The selected LLVM report,
`target/release-evidence/coverage-public-parity-matrix-vp8-delta-20261002.json`,
records 14,277/32,072 branches (44.52%) for that earlier matrix. It predates
the TIFF rows recorded below, is not a full-suite measurement, and Coverage MCP
marks source identity unverified and test attribution unknown. Coverage MCP
could not calculate an incremental union with the earlier report because exact
baseline branch detail is incomplete, so no marginal percentage gain is
claimed.

Coverage MCP lists no remaining branch gaps at WebP
`Vp8Decoder::read_quantization_indices` line 1119 after the delta-mode row was
added; the row-only report covers the `true` outcome, while the full matrix
report covers both outcomes. It also lists no remaining gaps for PNG
`validate_color_chunk_structure`, TIFF `decode_packbits`, or JPEG
`convert_rgb_420_mcu_row`, including its aligned fast path. The earlier
edge-case report still predates the delta-mode row. The targeted report
`target/release-evidence/coverage-webp-copy-overrun-row-20261002.json` records
the `<4>` lossless decoder taking the true bounds-rejection outcome at
`lossless.rs:766`; its filtered MCP query no longer lists that arm as a gap.
Coverage MCP marks these reports' source/build identity unverified and test
attribution unknown, so only the observed locations and aggregates are
reported. The latest full local `make coverage` campaign passes all 56
`coverage_matrix_tests`, including all eight decode lanes and the 402/402 AVIF
parity rows. Its report,
`target/release-evidence/coverage-full-avif-jpeg-alignment-edges-20261002.json`,
records 85,328/142,054 lines (60.0673%), 16,076/32,164 branches (49.9813%),
4,785/8,972 functions (53.3326%), and 130,382/219,569 regions (59.3809%).
Coverage MCP reports no remaining branch group at
`src/codecs/avif/container.rs:441` or `:444`,
`src/codecs/avif/samples.rs:513`, `src/codecs/jpeg/encode/mod.rs:632` or `:654`,
or `src/codecs/tiff/decode.rs:265`. The report has no source/build receipt and
test status is unknown, so those coordinates are diagnostic evidence alongside
the passing public parity lanes. The 100% coverage goal remains open.

## Packed low-bit TIFF tiles — 2026-10-02

Three active Pillow parity rows now cover 1-bit black-is-zero grayscale,
Deflate-compressed 2-bit grayscale, and 4-bit palette tiles. Each uses a
spec-compliant 16×16 tile over a 17×13 image, so the decoder reconstructs both
partial right and bottom edges. Pillow 12.2.0 supplies the exact references.
Three more rows use a Pillow-accepted 9-pixel tile width over a 17×1 image,
covering unaligned second-tile offsets for 1-bit, 2-bit, and 4-bit samples.
Those nonstandard widths record Pillow compatibility, not TIFF-spec
conformance.
Byte-aligned sample placement stays on a bulk byte-copy path; packed samples
with unusual bit offsets use a checked merge path that preserves neighboring
samples in the destination byte. No private unit tests were added.

The complete TIFF decode matrix now has 167 active rows. The broader 100%
coverage goal remains open. The refreshed local full LLVM run at
`target/release-evidence/coverage-full-png-incomplete-no-iend-20261002.json`
passes repository floors: 85,181/142,042 lines (59.97%), 16,027/32,156
branches (49.84%), 4,780/8,972 functions (53.28%), and 130,159/219,556 regions
(59.28%). Coverage MCP no longer lists the unaligned-copy arm at
`src/codecs/tiff/decode.rs:1199` as a gap; it still lists packed-row bounds
checks at line 1193 and the defensive sample-width guard at line 1122. MCP
marks report source identity unverified and test attribution unknown, so these
branch locations remain diagnostic observations rather than source-bound
coverage claims. The older full report below predates these rows and the
packed-tile decoder.

## Separate-planar TIFF parity — 2026-10-02

Manifest parity now covers PlanarConfiguration 2 RGB strips and tiles, including
raw data, Deflate with and without horizontal prediction, partial right/bottom
tiles, missing final strips and tiles that Pillow zero-fills, one-sample
grayscale, and 1:1 YCbCr strips and tiles. The YCbCr cases preserve Pillow's raw
component triplets in RGB mode. Malformed rows cover excess strip/tile offsets,
out-of-bounds strip/tile payloads, zero rows per strip, invalid planar value 3,
and missing, empty, zero, or mismatched compressed byte counts. Pillow accepts
inspection for invalid planar value 3 but fails at pixel load; Rust matches that
lazy failure. Pillow also rejects extra separate-planar tile offsets during
open, while a short final raw tile remains a successful zero-filled decode.

At that measurement, the TIFF decode matrix passed all 161 active rows. The full
`make coverage` campaign passes all suites and repository floors with
`target/release-evidence/coverage-full-planar-tiles-20261002.json`: 84,990/141,861
lines (59.91%), 15,994/32,130 branches (49.78%), 4,768/8,959 functions
(53.22%), and 129,857/219,262 regions (59.22%). Coverage MCP returns no
remaining branch gaps for `decode_separate_planar_tiles`; report source identity
is unverified and test attribution unknown, so its location results are
diagnostic rather than source-bound claims. The current matrix has 1,367 active
decode rows and 392 active encode rows at that point. Strict debug, release,
coverage-configured, x86-64 SSE2-baseline, and AVX2 Clippy checks pass. The x86
SIMD commands are compile checks and do not measure runtime performance.

`pillow_tolerated_post_idat_color_chunks` appends PLTE and tRNS chunks after
the 1×1 RGB PNG's IDAT data; Pillow and the Rust decoder retain the same exact
three pixel bytes. The existing `size_1x1` row also exercises
`decode_with_policy` at its exact 57-byte non-pixel metadata budget. The public
`apng_animated` row decodes with a 169-byte metadata budget and compares every
frame with its pinned Pillow reference; a 168-byte budget returns
`LimitExceeded(MetadataBytes)` with an observed extent of 169 bytes. The final
matrix report above includes these rows. Its Coverage MCP source and test
attribution remain unverified.

`apng_orphan_fdat_missing_control` keeps the orphaned `fdAT` sequence number
valid, so `decode_sequence` reaches the missing-`fcTL` outcome at
`src/codecs/png/decode.rs:1007`. Pillow 12.2 decodes the default image to the
exact pinned 49,152 RGB bytes, then reports `OSError: Truncated File Read` when
animation playback seeks the orphaned frame data. The paired
`apng_orphan_fdat_invalid_sequence` case retains the original bad sequence
number and checks Pillow's `SyntaxError: APNG contains frame sequence errors`
in ordinary decode; it covers the sequence-consumption error at
`src/codecs/png/decode.rs:568`. The short-`fdAT` row continues to verify
truncated-chunk handling before either sequence outcome.

`apng_empty_fdat_before_idat` adds a sequence-only `fdAT` before `IDAT` while
preserving Pillow's exact default-frame pixels and both decoded frames. Rust
verification rejects the misplaced chunk; ordinary decode retains the default
image, and sequence decode consumes the sequence number while ignoring the
empty frame-data payload. The follow-up PNG-only public parity run is
`target/release-evidence/coverage-png-apng-orphan-sequence-parity-20261002.json`;
its `test_decode_matrix_png` integration test passes. Coverage MCP reports no
remaining branch observations at `decode.rs:562` or `decode.rs:568`. The
`parse_apng` length check at `decode.rs:1003` still lacks its non-empty
pre-IDAT `fdAT` outcome; moving compressed frame data before `IDAT` caused
46,906 of 49,152 decoded bytes to differ, so that case cannot be added as a
pixel-parity witness. This selected-test report has an unverified source/build
identity and unknown test attribution; its global branch denominator is not a
whole-suite measurement.

Coverage MCP also found the unobserved COREHEADER depth-selection arm at
`src/codecs/bmp/inspect.rs:66`. The new `error_unsupported_core_depth` row uses a
complete 1×1 OS/2 COREHEADER with unsupported 2-bit samples, a four-entry
palette, and a padded row; Pillow reports `Unsupported BMP pixel depth (2)`.
The selected public parity run now observes both outcomes at that branch.

The TIFF decode matrix now invokes both public `decode_with_token` and
`decode_prefix` for each active TIFF input. Successful token results are
checked against the Pillow pixel reference; terminal errors retain the Pillow
contract, and resumable results must match the prefix API's exact retry
minimum. This keeps the ordinary `decode` route as the terminal Pillow
error-parity assertion. It also exposed two Deflate `NeedMore` results inside
strips whose byte counts already bound the compressed input. Those errors are
now terminalized in the TIFF Deflate path, while genuinely truncated TIFF
structures and strip spans retain resumable status.

Public boundary-sized TIFF cases cover token-aware sample-conversion
checkpoints: the existing 128×64 WhiteIsZero bilevel image contains exactly
1,024 packed bytes; new 32×32 WhiteIsZero Gray8 and Gray2 images reach their
1,024-sample checkpoints; and a new 32×32 YCbCr image reaches its
1,024-byte checkpoint. Coverage MCP reports no remaining branch outcomes in
`convert_pixels_with_token`. The new `lzw_extra_zero_output_strip` fixture
contains one LZW strip that decodes to `A` plus an extra strip whose geometry
requires zero output; pinned Pillow returns the exact one-byte image. The
decoder now skips unused compressed strips after checking their declared byte
range, while retaining the Pillow error for uncompressed extra strips. The
LZW first-code output-length guard at `decode.rs:1377` remains unobserved by
public inputs because zero-row compressed strips are now skipped before
inflation.

## TIFF YCbCr LZW parity — 2026-10-17

The public TIFF matrix adds three Pillow 12.2.0 inputs for contiguous 8-bit
YCbCr with default 2-by-2 subsampling: the 4×4 Kata candidate, a 3×3 image
that clips the right and bottom clump edges while omitting default tags 530
and 531, and a 4×4 image split across two aligned LZW strips with centered
positioning. Their input SHA-256 values are respectively
`d9c3e4b7111658f3b86fdb1f94bf04e03aebe7cb4fbd01a98708ebfd27e65a6c`,
`67df0dce7b4cab6af8d43f258f7eb1b77ba119e096ad242083baec83ebd585e9`, and
`1dd542a0a6e22d922f66e0f19e92421dd15fd1108f726af38810a11a6bf13487`.
An additional full-resolution 1-by-1 LZW row guards the compressed path while
the existing uncompressed 1-by-1 rows retain Pillow's RGBX raw behavior; its
input SHA-256 is `c3ebf5fdd35da227f9ef92cf45b81d726c908a0be1fe3e52c1bd3abfe699f9b6`.
The 32×32 subsampled control reaches the conversion loop's 1,024-pixel
cancellation checkpoint; its input and RGB reference hashes are
`655b9b0172e44433b222cc93acdd2e14ca296e22f373e5f073e2e24d33b5111f` and
`c5fc0bdafed814b597605fb7d00ce7510c60e1b45e774d0c71c4850ec39b4ded`.

The Kata candidate is 186 bytes and decodes to the exact Pillow RGB byte
sequence recorded in `scripts/generate_tiff_ycbcr_subsampled_fixtures.py`
(SHA-256 `af15457a9372a158dd276ced4109b650b99def573093a49f70dbb469c2262668`).
For compressed full-resolution data, the decoder converts each YCbCr triple;
the four 2-by-2 cases read six-byte clumps (`Y00,Y01,Y10,Y11,Cb,Cr`), check
clump and output geometry with checked arithmetic, clip odd image edges, and
apply libtiff's fixed-point default conversion. Libtiff's RGBA path applies a
clump's chroma to all four luma samples for either legal positioning value.
The subsampled layout supports contiguous strips with predictor 1 and aligned
two-row strip boundaries. Uncompressed files retain Pillow's RGBX raw path,
including the existing 1-by-1 rows. The fixture generator pins input hashes,
tag claims, and oracle output hashes, and is part of `make verify`.

The active `ycbcr_subsampled_lzw_1x2` row adds Pillow-compatible horizontal
full-resolution and vertical 2:1 subsampling. Its 203-byte complete TIFF has
input SHA-256
`d659443b0f3eb3c6de349a45d3e007e482b6727c4682f82545df0ac5b49971c9`; pinned
Pillow 12.2.0 decodes it to 4×4 RGB with reference SHA-256
`1ddadfe41e070962f7991dda8e8d26efc8c43a7352227b45154387a9991c5fb6`. Rust
accepts `[1, 2]` subsampling and keeps the established `[2, 2]` converter on its
existing specialized path. The selected public parity row passes. Two separate
target-only fault-contract rows exercise clump-length rejection and RGB output
reservation failure on the 1-by-2 path; both declare Pillow `not_applicable`
and are reported outside the parity totals.

The active `ycbcr_subsampled_lzw_1x2_odd_height` row is a 4×3, 203-byte TIFF
with the same 1-by-2 LZW layout. Its input SHA-256 is
`00013d30c2155603f592722acf82b52154eb031774453023612d93a8983e2185`; pinned
Pillow produces 36 RGB bytes with SHA-256
`c71479e1cfff0dd7c76fbe5df2c61e80c75405df52c1aa2413e2f4d08a99f1fb`. It
reaches and passes the new converter's bottom-edge clipping condition. The
selected public parity row passes. The focused report
`target/release-evidence/coverage-tiff-ycbcr-1x2-odd-final.json` records both
outcomes at `decode.rs:2124` (20 clipped and 60 in-bounds observations in the
exercised specialization). Coverage MCP calls hung in this run, so the
selected report is not unioned with a baseline and establishes no aggregate
coverage change.

The full all-format public decode matrix passes all eight integration tests.
Its report,
`target/release-evidence/coverage-public-decode-matrix-parity-final-20261002.json`,
records 11,386/32,252 branches (35.30%) for this selected decode run. Coverage
MCP reports both outcomes covered at BMP `inspect.rs:66` and no remaining
outcomes at TIFF `decode.rs:490` for the zero-row compressed-strip path,
`decode.rs:599` for bounded Deflate handling, or in `convert_pixels_with_token`.
The report has no source/build receipt and unknown test attribution; its
incremental comparison with the earlier selected report is unavailable
because that baseline lacks complete branch detail. These measurements do not
replace the separate full-suite coverage snapshot or claim 100% coverage.

The [AV1 encoder entropy corpus](../tests/fixtures/outputs/av1_encoder/index.json)
retains eleven complete files from standalone pinned libavif/libaom builds, with
Pillow/dav1d decoded pixels. It contains thirteen independently inspected tile
payloads: monochrome, 4:2:0/4:2:2/4:4:4, alpha, two-column tiling, lossless,
and 8/10/12-bit samples. Original and instrumented builds each encode every
file twice with identical results. All thirteen tile payloads bind uniquely to
finished native writers; unmodified libaom replay matches every logical state,
CDF update and final byte. This is native encoder evidence, not Pillow `save()`
parity or a Rust execution result.

The 83 hashed artifacts total 1,618,610 bytes. Gzip traces retain expanded
lengths and hashes; prepared planes, replay inputs, syntax reports, complete
files, pixels, instrumentation patch and build identities remain separate.
Committed tiles exhibit 292 normalization flushes and 103 carry changes,
including a two-byte carry. Separately labelled native models cover alphabets
2–16, probability endpoints, CDF plateaus, frozen adaptation, counter
thresholds and empty/short termination. They are not full-file syntax claims.
The generator is `scripts/generate_av1_encoder_refs.py`; its C observers under
`scripts/av1_encoder_oracle/` are development tools, excluded from the crate.
The Rust entropy/tile writer, image analysis and full AV1 compressor remain
unimplemented. The 16x16 10-bit I420 case also supplies the active public AVIF
decoder parity row `high_bitdepth_still_10bit_420_lossy_16x16` in the
[coverage matrix](../tests/fixtures/coverage_matrix.json), with generator and
fixture provenance in the [AVIF fixture README](../tests/fixtures/input/images/avif/README.md).
Its selected Pillow 12.2.0 check passed for 16x16 RGB8 output
([reference bytes](../tests/fixtures/outputs/raws/Decode.avif_high_bitdepth_still_10bit_420_lossy_16x16_avif.bin), 768 bytes,
`55820e29e26b25634c402e57e8b743bb9560666861f5ea07c14a607f31eea38f`). A
same-source AVIF selected-test Coverage MCP comparison reports 73 newly
observed branch coordinates, including seven outcomes in
`complete_high_depth_420_reconstruction_context`. Its source/build receipts are
unverified, so this is limited selected-test coordinate evidence and does not
claim a full-suite coverage-percentage change. The active matrix row is decoder
evidence; it does not promote a Rust AV1 encoder roadmap item.

The 2026-09-17 private still-container candidate has a
[28-file native mux corpus](../tests/fixtures/outputs/avif_mux/index.json).
Pinned Pillow/libavif/libaom observations repeat exactly, including complete
encoded bytes and decoded pixels. Sixteen files correspond to registered
planned still rows; twelve supplemental files cover orientation, combined
alpha/metadata and actual media reuse within and across item boundaries.
The corpus has 105 hashed artifacts totaling 314,731 bytes and retains clean
libavif source, native binary, generator, source image and manifest identities.

The Rust writer consumes semantic descriptors and encoded/metadata payloads;
expected serialized boxes and output offsets are excluded from its inputs.
Deferred tests compare complete containers, output limits and separate model
error/interruption states. The source-derived 10/12-bit descriptor handling
has no native encode witness yet; all 28 files are 8-bit. AV1 compression and
metadata preparation remain unimplemented, so public encoding is still
unavailable and no matrix row is promoted. Rust behavioral execution and
managed coverage remain deferred; historical measurements below are unchanged.
All 16 strict compile configurations pass across native and both WASM targets,
with AVIF-only/all features and ordinary/coverage modes, plus native default
and no-codec lanes. Warnings-as-errors rustdoc, formatting and static checks
pass. The coverage-origin manifest now classifies 111 exact guards across 16
files; these are provenance records, not coverage exclusions or a new
measurement.

## Historical release 0.1.2

Version 0.1.2 passed [main CI](https://github.com/appunni-m/image-slash-star/actions/runs/35007807946)
and [release CI](https://github.com/appunni-m/image-slash-star/actions/runs/35010129246)
at `70190214a0711223302c76ab58e76c097288d80b`. The published crate matches the
GitHub checksum manifest, with SHA-256
`e53037e57d0c5cae052ba94851c8cf72a80b9dfe195cd21b166506ab7bdbeb3b`.

The 2026-09-15 all-feature release coverage run recorded 95,473/161,451 lines,
14,912/32,258 branches, 4,859/9,244 functions, and 140,609/241,503 regions.
The release floors are respectively 59%, 46%, 52%, and 58%. The complete target
retains 100%; a floor pass is not complete coverage. All executed comparisons
remain mandatory.

## Maintained fixture matrix

The retained matrix contains 1,798 total rows: 1,370 decode /
inspect / verify rows and 428 encode rows. Of those, 1,370 decode rows and 396 encode rows are
active; 32 encode rows remain planned. Planned rows stay outside executed
parity numerators. Active rows have operation-specific outcomes, including
not-applicable results.

[Generated capabilities](capabilities.md) distinguish declarations and actual
native/all-feature fixture observations. A WASM cross-compile does not extend
that pixel-evidence scope.

## Decoder validation — 2026-09-20

The full public matrix passes all 46 tests, including 343 AVIF rows and exact
native comparisons of complete animations. All native, WASI and browser-WASM
feature lanes pass. High-depth, HDR and animation additions are on `main`,
awaiting the next release; the 32 AVIF encoder rows remain planned.

A fresh local `make coverage` run passes every executed test and the unchanged
release floors: 99,349/163,373 lines (60.8112%), 15,524/32,418 branches
(47.8870%), 5,052/9,372 functions (53.9053%), and 145,689/243,793 regions
(59.7593%). This is LLVM evidence, not a new managed Coverage MCP snapshot.
The changed-line review observes 274/294 executable lines in the seven changed
AV1 modules. Unobserved lines include defensive failures and the Wiener stripe
branch; this is not complete line or branch coverage.

The local reports live under `target/release-evidence/`: `coverage.json`,
`coverage.lcov`, and `avif-fix-coverage.json`. The last receipt retains source
and input hashes and the exact uncovered line list. CI produces a fresh report
for each pushed commit and retains it as the `llvm-coverage` artifact.

## Codec parity and coverage — 2026-10-03

At that point, the matrix contained 1,927 total rows: 1,474 decode /
inspect / verify rows and 453 encode rows. Of those, 1,474 decode rows are active;
421 encode rows are active and 32 are explicitly planned. It includes 440 active
AVIF decode rows. Twelve new error rows pass the selected public matrix runs:
`short_marker_missing_eoi` (JPEG, input SHA-256
`869e30d73346ca502d81c1a04881b3ca8cbb64d980cb73b73cf856d54a8bdcbc`),
`error_ispe_nonzero_version` (AVIF, `9caaefec8072bd826607867f9d80c8ce2a10d67f1638bf8c1ac382055d75a791`),
`error_ispe_zero_width` (AVIF, `3524b652351b5a668a7a91a7dcd10a376205d5771858d11e073f61bdf96de3da`),
`error_iloc_version_3` (AVIF, `9ce231f76eac5fb15f9da9d12cc34b6e6a59d7eeee8bcad4101a451ee9648dd2`),
`error_sequence_zero_timing_rate` (AVIF, `055dea40a69ca9eb7a60a499524a7e563990bccb708f6a9992c1691f304f15e2`),
`error_inconsistent_operating_point_idc` (AVIF, `3c81896058305429697219be55ddcaf4fcf5293aca48570c1e7e22f6ebcd0a07`),
`error_sequence_zero_time_scale` (AVIF, `92d7882e439adf1430d60460d488d0df2b7feb9f6ba3bb7931898c4f70f25ac0`),
`error_inconsistent_operating_point_idc_low_byte` (AVIF, `d49e5403f421c1f71140c7442ba4493e51317bd1c9776d0aaf19069e9002fd71`),
`error_ispe_zero_height` (AVIF, `e93421df7d5d3564f7d297c4deb09a765be702e16aa86de41c3705974d059024`),
`error_vp8_color_space_invalid` (WebP,
`ce6b3a9b8e69194d02cdc0739b9db9cdc668554abc0fe29e28fa902e49eb6e45`),
`error_malformed_container_animated_all_frame_chunks_unrecognized` (WebP,
`cde9d77559e01ef3324fb73f6541c4cd6257b8563595a4ca397fc0ab52f69886`), and
`error_truncated_riff_extent_metadata_preflight` (WebP,
`e84c137505358f37c528aa349b7541971603dae587e5dbfd15e67792ce72da09`).
The pinned Pillow 12.2.0 oracle rejects each case under the matrix's malformed
decode contract. The selected rows and full coverage campaign pass. Coverage
MCP reports nine newly observed branch coordinates at AVIF `samples.rs:656`,
`:661`, `:1030`, `sequence.rs:179`, and `:223`, JPEG `parser.rs:609`, and WebP
`vp8.rs:1268`; a filtered follow-up reports no missing branch groups at the
targeted AVIF height, timing, and IDC operands. The comparison has no
source/build receipts and test attribution is unknown, so these are coordinate
differences rather than verified causal gains or regressions. The active
`uniform_tile_rows_monochrome_256` AVIF row uses a 344-byte constant grayscale
still with one tile column and four uniform tile rows (input SHA-256
`782568b37a63b44e632aee68896bcee3c59c5492081c7ac2ac6a31fc0ca1df37`). Its
Pillow reference is 196,608 RGB bytes with SHA-256
`d5039cbfb3fb9a915614802a721cba155442563d36a6bbf73e582e65f5290890`; the
selected public parity row passes. The pre-row full report showed the true arm
of `read_tiling` at `av1/frame.rs:3647` missing, and the selected report leaves
only the false outcome of the tile-row bit read. The active WebP row
`error_malformed_container_animated_all_frame_chunks_unrecognized` changes both
nested VP8L frame FourCCs in a valid two-frame lossless animation to `JUNK`
while preserving RIFF lengths (input SHA-256
`cde9d77559e01ef3324fb73f6541c4cd6257b8563595a4ca397fc0ab52f69886`). Pillow
reports `could not create decoder object`; its selected public matrix row passes
the malformed WebP contract. The selected report covers the previously missing
true arm of `WebPDecoder::new` at `webp/native/decoder.rs:226`. Coverage MCP
reports missing source/build receipts and unknown test attribution for these
selected runs, so the arm mappings remain coordinate evidence. The new
`animated_lossless_inter_420_b32x32_10bit_64x64` AVIF sequence adds a
`animated_lossless_inter_420_b32x32_10bit_64x64` AVIF sequence adds a
deterministic 64×64 10-bit 4:2:0 inter-frame case while preserving the prior
32×32 fixture. Its pinned dav1d trace confirms four 32×32 leaves in the second
frame; the selected public parity run passed and checked exact Pillow frame
bytes. The selected LLVM report records three hits on the previously missing
false arm in one `decode_inter_translation_impl` specialization at
`src/codecs/avif/av1/block.rs:54836`. The fresh full report adds one aggregate
branch observation. Coverage MCP comparison remains limited because these
reports have no source/build receipts and MCP cannot attribute tests. The active
`enc_lossless_sampling_multi_group_checkpoint` WebP
row encodes an 8192×16 RGB PNG with lossless quality 80, method 4, and a
cancellation token. Its deterministic 32×1 metadata map alternates two retained
Huffman histogram groups. Pinned Pillow 12.2.0 produces the committed 98,388-byte
reference (SHA-256
`cc92a5c28e780be64ef34a104ddeeb92e76c967f8e3f8f1a16fb42a01ca907a4`), and the
focused selected row and full encode matrix pass. The current full local report
still lists the true checkpoint arms at `encoder.rs:1378` and `:1418`; the
filtered query at `:1421` has no missing branch group. Because the report lacks
source/build receipts and test attribution, this is a fixture-parity result plus
coordinate coverage evidence, not source-bound proof that those checkpoint arms
were executed. The active `enc_animated_non_centisecond_delays` GIF row uses
two WebP frames with 17 ms and 33 ms source durations. Rust now stores their GIF
delays as 1 and 3 centiseconds, matching Pillow's 10 ms and 30 ms output; the
129-byte encoded reference SHA-256 is
`52f23eb3646581d4982bed87c11bf63315ddf2e3a4cdf5d73a5b0b80c44e8509`. The
`enc_animated_large_fraction_delay` row sets the first duration to the unreduced
`u64::MAX / u64::MAX` rational (one second), exercises GIF's wide scaling path,
and matches Pillow's `[100, 3]` centisecond delays. Its 129-byte reference SHA-256
is `df760f2efb3378fd2eb3c46660ebaf5399b11086838ec28f99b5faf5e9a15792`. The
focused GIF encode parity suite and full coverage run pass. Coverage MCP's
filtered `gif_delay` query now leaves only the zero-denominator arm at line 869;
the wide-scaling arm is no longer missing. The local report has no source/build
receipt and test attribution is unknown, so this is coordinate evidence. A
zero-denominator value is not a valid Pillow frame duration and has no public
Pillow-backed fixture witness. The new `primary_item_irot` row decodes the independently
generated 325-byte mux fixture with an associated 270-degree primary-item
rotation. Its Pillow 12.2.0 output is 2×3 RGB (18 bytes,
`efbe5247163f007591d148c7975ad74429fd2282feb0bdec402838155763da38`), and the
Rust decoder reports the matching counterclockwise 270-degree transform. The
Seven `portable_lossless_monochrome_limited_*` rows cover limited-range
monochrome RGB and RGBA at 8, 10, and 12 bits, plus a BT.2020/PQ CICP case.
Their selected public parity
rows pass against exact Pillow bytes; the preceding all-feature AVIF decode lane
included all 415 active rows. Coverage MCP reports no
remaining branch gap at `container.rs:1367` in
`coverage-avif-primary-irot-current.json`; that selected report has no
source/build receipt and labels test status unknown, so this is bounded path
evidence only.

The new active `ipma_version_1_wide_item_ids` still row widens the primary
item ID in AVIF's `ipma` FullBox from 16 to 32 bits and adjusts the following
absolute `iloc` extent. The 3,079-byte fixture is generated from Pillow's
baseline still and hashes to
`90ac8f0230ddede24a72b73fb120bd46680962d4c0a7facab31e77b088cd82c5`. Pillow
12.2.0 verifies and decodes it to the same 128×128 RGB pixels, metadata, and
info as the baseline. The focused public AVIF decode matrix passes 416/416
rows. The full local `make coverage` campaign passes every executed test and
reports 85,737/142,131 lines (60.3225%), 16,350/32,170 branches (50.8237%),
4,797/8,975 functions (53.4485%), and 131,045/219,720 regions (59.6418%);
all four release floors pass and the 100% target remains open. Its LLVM report
is `target/release-evidence/coverage-full-ipma-v1-20261003.json`. Coverage MCP's
coordinate comparison lists two newly observed branch arms in `parse_ipma`,
the version-zero arms in `container.rs` and `samples.rs`. That report has no
source/build receipt and MCP reports test attribution as unknown, so the
comparison is coordinate evidence; the local matrix and full coverage commands
themselves passed.

The active `iloc_version_2_wide_item_ids` AVIF row derives from
`baseline.avif`, widening the `iloc` item count and ID to 32 bits, adding
construction method zero, and relocating the absolute extent after the
six-byte box growth. The generated input SHA-256 is
`39a494da43edb3e25dcc60c1c51c987452c66a2590e5ac9563d416d922199d87`. Pinned
Pillow 12.2.0 accepts it and produces the same 128×128 RGB pixels and info as
the baseline (49,152 bytes; output SHA-256
`f1a2555b1c61036af2bd1d3d125a6a1343993873d9e5d58e3caee79989095dcf`). The
selected public row passes, and the full AVIF matrix passes 418/418 rows. Its
selected LLVM report records four visits to the version-2 path at
`container.rs:608`; the full report has no remaining branches at lines 608 or
622. Coverage MCP reports no gaps at those coordinates, but has no source/build
receipt and test attribution is unknown, so that result is coordinate evidence.

The active `size_1x1_local_palette_only` GIF row moves the exact six-byte
palette from the Logical Screen Descriptor's global table into the image's
local table, leaving no global table. Its 37-byte input SHA-256 is
`2d029f561c59b4bf8c23b46e1acf61a144e16d7ec46901e0c428914801363146`. Pillow
12.2.0 returns the one-byte P image `00` and the six-byte RGB palette with SHA-256
`b0f66adc83641586656866813fd9dd0b8ebb63796075661ba45d1aa8089e1d44`; the
focused public parity row passes detect, inspect, verify, decode, and sequence
comparisons. Pillow omits `info["background"]` for this input, so the matrix
records the Logical Screen Descriptor's palette index with
`specification_reference` provenance. Metadata-byte scanning runs only when a
caller sets a `DecodePolicy` metadata limit, so the parity-only row does not
claim coverage for that policy path.

The ordinary and token-aware GIF LZW paths reject zero-sized frames before
decoding and return as soon as the output reaches its expected length. This
makes the first-code output-full guards and end-code exact-length success arm
unreachable; those redundant conditions are removed. All 79 active
GIF parity rows pass. The refreshed Coverage MCP GIF query no longer lists
`decode_lzw`; it still lists the metadata scanner's global-table false arm at
line 366 and local-table true arm at line 393. That scanner only runs when a
caller sets a metadata-byte policy limit. The new full report has no
source/build receipt, so these filtered results are source-coordinate leads.

The active `high_bitdepth_still_10bit_420_alpha_lossless_16x16` AVIF row
exercises the public 10-bit 4:2:0 color path with an auxiliary 10-bit alpha
plane. Its 1,653-byte input SHA-256 is
`8c3cb86055e1003d91897d986995dd2ee030d0278b82d3d7c9ddc783189eb76f`; pinned
Pillow returns 16×16 RGBA8 (1,024 bytes) with SHA-256
`a3eec8f3c1e9539ccee71ec2e9d514a46d20d0b93a7b8e5d562379ef4a794dd9`. The
selected public parity row passes. Its isolated branch report records the
10-bit alpha path at `decode_portable` lines 492, 535, and 555. The refreshed
full report shows no remaining branch gap at those locations.

Two more active 10-bit alpha stills exercise the AV1 4:2:2 and 4:4:4 decode
paths. The all-lossless 16x16 4:2:2 input hash is
`eef1ffae7961354032dad642c1ebfc146cbcb28f7147c4dce6d9b8a4de17d392`; Pillow
RGBA8 output hash is
`b7731cca14b3da564883533a5cfd6ddc75aba189b60dbe7795de93711237a457`. The
16x16 lossy 4:4:4 input hash is
`468bd5af2f96693582d6618572dc353cac4ee16719cebebb8e20ff7e81049548`; Pillow
RGBA8 output hash is
`02e871ddbaa576836e9d486cf36be09efce4f2fef164c7f66af2a579dfb1e67a`. Both
selected public parity rows pass. Coverage MCP's selected report reaches the
10-bit 4:2:2 sampler path at lines 588 and 606 and the full-resolution alpha
conversion at line 496. That report has no source/build receipt and marks test
status unknown; the local selected matrix command supplies the pass result.

The active `portable_lossless_monochrome_1x1` row exercises a
1×1 8-bit lossless monochrome still. Pillow returns RGB `7f7f7f`; the selected
public parity test passes, and the Rust reconstruction crops its coded block
to the visible canvas. The row's pinned input SHA-256 is
`6c4212de07ead445c0b468c39b77f099cc8555e99edd6460806407d7536be305`.
The WebP decode lane passes all 230 active rows. Two generated 1×1 VP8L cases
exercise transformed opaque RGB reconstruction and singleton RGBA color-cache
insertion. Their Pillow outputs are RGB `0a141e` and RGBA `0a141e80`. Coverage
MCP reports no remaining branch group at `webp/native/lossless.rs:763`; it still
reports the alpha-used false arm at line 155.

Two JPEG fixtures truncate 64 entropy bytes before EOI in baseline 4:2:2 and
4:4:4 inputs; both selected public parity rows pass. Coverage MCP reports no
remaining branch group at the direct-row fallback locations
`jpeg/decode/decode.rs:1504` and `:1697` in the latest full report.

The metadata-policy manifest adds public TIFF cases for a cyclic IFD, empty
tile byte counts, and an empty IFD chain. The policy test checks successful
metadata accounting and malformed-scan propagation through inspect, still
decode, and sequence decode. Coverage MCP reports no remaining branch groups
at `tiff/decode.rs:133`, `:150`, or `:160`.

These filtered MCP observations have no source/build receipt or per-test
attribution; they are bounded path evidence. The full-report comparison lists
seven newly observed branch coordinates at these TIFF, JPEG, and WebP paths,
but MCP marks the result limited because source/build identity is unverified.
The full local report still passes every repository coverage floor.

The active GIF row `enc_rgb_high_color_token_checkpoint` adds a 33×33 RGB PNG
with 1,089 distinct colors, forcing the token-aware median-cut quantizer across
its 1,024-pixel checkpoints in all three mapping passes. The source PNG SHA-256
is `a4c488ae9683a7ae782017bf8fc334d67ec43e0f0430a0c2442cfa426dcc82e6`; its
Pillow GIF reference is 1,862 bytes with SHA-256
`7bbedc5d1161b6b8b00a41b9b5f4dc52e7cc2d33a4cc63b30320876a4165e43f`. The
selected public parity row passed byte-for-byte comparison against Pillow and
ordinary Rust encoding. Its isolated LLVM run records one checkpoint hit and
1,087 non-hits at each pass; the fresh full report records no remaining branch
gap for `quantize_rgb_nearest` in Coverage MCP. Both reports lack source/build
receipts and test attribution, so local test results and counters are scoped
evidence rather than managed coverage attribution.

The active GIF row `enc_rgba_high_color_token_checkpoint` exercises transparent
pixel normalization in token-aware RGBA quantization with 1,088 opaque pixels
and one fully transparent pixel. Its 33×33 PNG SHA-256 is
`1939c2816f1f56c21c3e672d03df8341a6dcf16181cbaa8035b93ad0e3ae9896`; the
1,936-byte Pillow GIF reference has SHA-256
`ee584ccbbe4b55eb06a9e2d3ad52ae037d74f2d3ad545cf0e954ef555e8c4e34`. The
selected public parity test passes exact Pillow byte parity. Its isolated LLVM
run records the normalization checkpoint and both alpha outcomes; the full
report has no remaining branch group for `quantize_rgba` in Coverage MCP. Those
reports also lack source/build receipts and test attribution.

The active GIF row `enc_animated_rgb_full_palette_token` uses a lossless,
opaque two-frame WebP animation with all 256 grayscale values in each 16×16
frame; the second frame swaps the first and last pixels. The WebP SHA-256 is
`eda7f7272532e97d362a46a905d3f80dc857c5a5602232dde366f885d9838946`. The row
requests GIF local color tables and the public token-aware sequence encoder;
its Pillow GIF reference is 2,963 bytes with SHA-256
`881dfb2f5a8b81575a20ac40350ccb6124bf3ef95df6b9b15c9a9cc5cb98c6f4`. The
selected parity row passes for both ordinary and token-aware sequence encoding.
Its isolated LLVM run records three visits to the false arm at
`gif/encode.rs:472`, where the full 256-entry palette prevents adding a
transparency slot. The report has no source/build receipt or test attribution.

The WebP lane's public basic-inspection
parity assertion now covers VP8 canvas width and height mismatches and VP8L
canvas width and height mismatches. The generated VP8L files are complete
`VP8X, VP8L, EXIF` RIFF images derived from `exif.webp`; their Pillow outcome is
`OSError: could not create decoder object`. The VP8 width fixture is pinned by
SHA-256 `9213d677987c5dd684e0d07d09efcb3922a624b8188706f2879b3dbfe385d7ea`,
the VP8 height fixture by
`8d9ebf1b4656176dae2627363d7f73cc99a27b4fdcfc74f14db917def973bf14`, the
VP8L width fixture by
`51127d31ba0f68d73000ffea0a8d962cc5408db4dea2df6188a70302e7b10fa4`, and the
VP8L height fixture by
`79a1c86ebd90824274d792f602d980e6dc1c01bb83a969ceb4e1ffb2b057c3a5`. Two
more full-file mutations start from the valid two-frame
`animated_sequence_rgba_keyframes.webp`: one clears the first nested VP8L
signature byte, and one sets its unsupported version bit while preserving
dimensions. Pillow returns the same decoder-object error for both. The public
decode matrix passes both rows across detection, inspection, verification,
still decode, and sequence decode; their input SHA-256 digests are
`a40cd40a7f492abe7c086323aefd25edd04e73291618154285e83b85d3ca4a4c` and
`6e51fc2edee8208762d8faa8e292993f165d715a3ba2e7a5cdaf9e629c2f75c8`.
Coverage MCP's full report filtered to WebP inspection and VP8L lossless
decoding contains no missing group in `inspect_extended_basic` or
`LosslessDecoder::read_frame_header`, though other WebP groups remain.
Source/build identity and test attribution are unverified in that report, so
the query is diagnostic evidence alongside the local parity pass.
The new `portable_i444_quality100_64x64` case checks a 64×64 8-bit 4:4:4
quality-100 frame with fixed 64-pixel partitions. The selected public
matrix row passes exact Pillow parity for all 12,288 RGB bytes; its input SHA-256
is `5ceb66b47bda43bec5c421222eeb3ef858c62702bc41833986fdf678f6867737` and its
output SHA-256 is `5e74b862ca69314d8ae9a3aa4b8f3af5651dfbd238ce922a7973c193e338f64a`.
The new `animated_lossy_global_halfblend_spatial_i444_b256x256`
row pairs identity and RotZoom global references in compound spatial neighbors.
Its four complete RGB frames and 100ms timing match Pillow, and the independent
loop bundle records an infinite-play sequence. Selected LLVM coverage records
five visits to each outcome at `motion.rs:1707`; the full report records 30/10.
Coverage MCP returns no missing branch group at that coordinate. These local
reports have no source/build receipt and list test status as unknown; the public
matrix row and full campaign pass. The new `coverage_h16x4_third_leaf_no_chroma_tx8x4_01` row pins
a 16×16 4:2:0 H4 with two TX8×4 luma residuals in its third no-chroma leaf
against Pillow output and passes the matrix. Coverage MCP still shows 26 missing
branch observations in
`decode_following_vertical_without_chroma`: production decodes this input with
the complete partition walker, so the row guards that public reconstruction
path without covering the narrower first-partition fallback. Its report has no
source/build or test receipt, so those locations remain unverified.
Three further AVIF parity rows add a 64x64 TX64-root split and two opposite
512x128 projected-motion window misses. All eleven decoded frames and native
loop observations match their pinned references. Coverage MCP reports no
missing branch group at `entropy.rs:8000` for the TX64 child offsets or at
`motion.rs:3351` for the temporal x-window checks. These reports have no
source/build receipt and label test status unknown; the successful full local
coverage run supplies execution evidence.
One more public sequence adds a textured 184×64 all-lossless 4:2:0 input with
fixed 32×32 AV1 partitions. Its final-frame edge block at pixel `(160,0)` has
a 24×32 visible extent and selects the clipped-lossless plan. The input hash is
`0a8c935fe67694fe576e7f21064eec4c428cffc2a05a3ff2d0be33e401a20c1d`; both
Pillow RGB frame hashes are retained in the AVIF loop oracle and exact public
sequence parity passes. The selected LLVM report observes the true outcome at
`entropy.rs:6925`; Coverage MCP reports the false outcome missing in that
single-row report. Its source/build receipt is unverified and test status is
unknown. The complete coverage run passes, with branch totals unchanged.
Two public parity rows exercise the AVIF ICC `prof` and `rICC`
properties; Pillow 12.2.0 accepts both fixtures with matching RGB pixels and
preserved profile bytes. Two further rows assert associated BT.2020/D65 `mdcv`
fields against the property definition while Pillow verifies identical pixels
and profile preservation, including when one bounded trailing metadata byte is
present. A new two-frame 60×64 lossless I420 sequence pins a B32×32 leaf at
pixel `(32,32)` with 28×32 visible pixels at the right edge; both complete
frames match Pillow exactly. New JPEG parity cases cover progressive
AC-first run overshoot, AC-refinement EOB runs and scan tails, repeated sparse refinement, a malformed
DC Huffman category-16 symbol, and out-of-range spectral starts and ends
rejected by Pillow during pixel loading, an oracle-accepted out-of-range DC
coefficient case matching signed JCOEF narrowing, plus two DC-only progressive cases
that exercise smoothing with ordinary and Pillow-tolerated zero quantization
tables. The matrix also includes a 33×33 baseline 4:2:0 encode with partial
MCU edges. Five additional encoder rows exercise the fused 4:2:0
batch path at 16×2, 16×1, 17×17, and 17×2 sizes, plus the generic 4:2:2 RGB
batch tail at 17×17. Four more JPEG fixtures cover partial baseline grayscale
and CMYK blocks, plus progressive 4:2:2 scans at 17×13 and 1×8. The odd 4:2:2
case exposed scan iteration over padded storage blocks; progressive scans now
use the logical component block extent while retaining the padded buffer
stride. Successful cases compare exact pixels or bytes against
Pillow. At that time, the all-feature JPEG decode lane passed all 154 active rows. The new
`baseline_ac_pair_tail` fixture exercises a baseline AC run whose first
coefficient position exceeds 63; its exact Pillow pixels pass the public
matrix. The bounded overrun path consumes the symbol amplitude and writes to
coefficient 63, matching libjpeg-turbo's padded zigzag behavior for corrupted
streams. Coverage MCP's query for `decode.rs:145` has no remaining missing
branch group in the full JPEG lane report. A selected incremental branch union
was incomparable because its exports lacked complete branch detail; the
parallel region comparison found 10 newly observed regions in
`decode_block_fast` against the other 153 JPEG decode rows. The reports are
`target/release-evidence/coverage-jpeg-baseline-excluding-ac-pair-tail.json`
and `target/release-evidence/coverage-jpeg-ac-pair-tail-selected.json`; the
full-lane report is `target/release-evidence/coverage-jpeg-ac-pair-tail-final.json`.
These reports
have no source/build receipts, so MCP marks their test status unknown; local
selected and full JPEG decode runs passed.

Three more Pillow-generated baseline JPEG rows exercise public restart-marker
fallbacks: 4:2:0 with DRI 3, which is not aligned to its eight-MCU row, and 4:2:2
and 4:4:4 with DRI 1. The fixture generator uses Pillow's
`restart_marker_blocks` option; the full 128×128 sampling grids avoid partial
MCU edges. All 157 active JPEG decode rows pass exact pixel parity. Coverage MCP
reports no remaining branch gaps at `decode.rs:1083`, `1330`, or `1494` in
`coverage-jpeg-restart-fallbacks-current.json`. That selected report has no
source/build receipt and marks test status unknown, so the result is bounded
path evidence; the local test output confirms 157/157.

A fresh selected all-feature run passes all 60 active JPEG encode rows,
including progressive CMYK and the CMYK EXIF cases below.

The public `enc_cmyk_progressive` row closes a Pillow-supported combination
that previously returned an unsupported error. The general progressive scan
script now covers each component of non-YCbCr images, and the optimized CMYK
row-streaming path remains restricted to baseline output. The exact Pillow
reference is 10,364 bytes with SHA-256
`53d7b0a56e5156b5b9a7ea14ef13fe5fc0fe88274b1d86efa5e6668fa1592814`; all 60
active JPEG encode rows pass. Coverage MCP reports no remaining branch-gap group
in `default_progression_script` for
`coverage-jpeg-progressive-cmyk-current.json`. Its source/build receipt is
unverified and test status unknown, so this does not claim a managed coverage
delta. The baseline CMYK streaming path remains available unchanged for
non-progressive output.

JPEG parity now also covers baseline CMYK streaming with valid and oversized
EXIF, restart-marker emission, the 65,536-MCU restart-interval clamp, and restart
rows above Pillow's signed limit, alongside the existing row without EXIF. The
full all-feature JPEG encode lane passes all 60 active rows. Coverage MCP reports
no missing branch at `restart_interval_from_rows`, and no missing region at the
APP1 error-propagation site on line 4813, the restart-marker emission site on
line 4866, or in the CMYK streaming function. A 4,096-row setting clamps to DRI
65,535; a setting of 2,147,483,648 omits DRI, matching Pillow. Pillow rejects the
65,534-byte EXIF payload with `ValueError: EXIF data is too long`, matching the
Rust parameter error. The current report has no source/build receipt and MCP
marks test status unknown; the local test output confirms 60/60. MCP findings are
limited to this JPEG lane and do not establish a full-repository coverage change.
At that point, the active JPEG decode inventory had grown to 158 rows. Its new
`baseline_cmyk_entropy_truncated_tail_64` case truncates 64 bytes of baseline CMYK
entropy while retaining EOI. Pillow returns the exact 13×9 CMYK output (468 bytes);
the direct-decoder fallback at `decode.rs:1065` is covered by the public row.
Coverage MCP returns no missing branch group at that coordinate in the current
full report, which has no source/build receipt and reports test status unknown.

The JPEG decode inventory now has 159 active rows. The new
`baseline_cmyk_height_partial_aligned_width` fixture is 8×9 CMYK: its width is
MCU-aligned while its final MCU row is partial. Pillow's exact 288-byte output
has SHA-256 `ac10bdd2898d8df3462b1594b15ade2e2a429a61ca2908a31102c64f27b579b9`.
Exact parity passes, and both outcomes of the AArch64 height-alignment check at
`decode.rs:1023` appear in the full local report. Coverage MCP returns no
remaining gap at that line; the LLVM report has no source/build receipt and MCP
labels test status unknown.

The `idct_high_horizontal_frequencies` row uses a restart-separated, fully
valid baseline grayscale JPEG. Six blocks place AC coefficients at horizontal
frequencies 3 through 7 and at vertical frequency 1. Its exact Pillow pixel
reference passes the public decode matrix. The selected LLVM report observes
both outcomes at each scalar IDCT row-shortcut predicate on lines 191–195;
Coverage MCP returns no remaining gap for those selected coordinates. The
local report has no source/build receipt, so this is selected-path evidence and
does not establish a complete-suite coverage delta by itself.

The 2026-10-01 all-feature `make coverage` run passes every executed test and
the release floors: 106,527/162,369 lines (65.607967%), 17,345/32,236 branches
(53.806304%), 5,341/9,371 functions (56.994985%), and 156,010/242,524 regions
(64.327654%). Its LLVM report is `target/release-evidence/coverage.json`, measured
on macOS ARM64 with `nightly-2026-07-16` against the current working tree. It
has no source/build receipt: Coverage MCP reports source identity unverified
and test status unknown, so its totals cannot establish a source-bound delta or
per-row coverage gain. Coverage MCP groups the remaining branch observations in
the AV1 lossless partial-grid predicate at `block.rs:54830-54831`; the targeted
`decode_following_vertical_without_chroma` query still returns 26 observations.
A selected 2026-10-04 run of the public 64×64 10-bit fixed-32-partition row
records six false outcomes at the current `block.rs:54838` sample-depth check,
so that outcome is reachable through a valid full-size `LosslessGrid`. This
selected witness does not identify the older report's remaining observations
at lines 54830–54831; both reports lack source/build receipts, so their line
coordinates cannot be treated as source-bound branch identities. The 100% goal
remains open, with 55,842 lines, 14,892 branches, 4,030 functions, and 86,514
regions uncovered.

While validating the three-frame I444 parity row, a DAV1D trace exposed a
transposed compound-reference CDF lookup. Rust indexed the AV1 `[group][context]`
tables as `[context][group]`, selecting `Last3` where the stream selected `Last`.
The decoder now indexes group then context; the selected animated I444 parity
row and the full test campaign pass. Coverage MCP reports 207 newly observed
branch coordinates against the prior report, but source/build receipts are
missing, so MCP marks that comparison limited rather than a verified gain.
Its highest missing group remains
`Lossy420Decoder::decode_inter_translation_impl`, with 1,842 repeated
coordinates across monomorphizations, not 1,842 distinct source branches. The
100% goal remains open.

Two odd-sized baseline color JPEG rows now cover right and bottom edges in
separate sampling modes: 13×9 4:4:4 and 17×9 4:2:2. Both compare exact decoded
pixels with the pinned Pillow oracle. The 4:4:4 row also drives a short RGB
conversion tail through the direct row kernel, extending coverage beyond the
existing odd-sized progressive 4:2:2 case.
The AVIF additions include four-frame 4:2:2 and 4:4:4 sequences. The 4:2:2
case moves a small block across horizontal chroma boundaries; the 4:4:4 case
translates a periodic full-frame chroma pattern. A three-frame 4:2:0 sequence
with forced 4x4 inter partitions and a small luma/chroma change exercises the
subsampled block-width ownership edge. Exact Pillow frame references and pinned
libavif loop witnesses pass the public sequence checks.

A 96×96 untransformed VP8L lossless case repeats a four-pixel RGB phrase at
short distances. Its exact Pillow reference passes public decode parity. The
isolated and full local branch reports observe the short non-overlapping copy
arm; Coverage MCP reports new branch coordinates for this input, with source
and build identity unverified.

A deterministic 128×64 VP8L stream exercises color-cache lookahead after a
cached color, a new literal, and a repeated cached color. The exact RGB output
matches Pillow, and the isolated public report observes both outcomes at the
three lookahead branches in `lossless.rs:828-830`; Coverage MCP reports no
remaining gap for those selected coordinates, but source/build receipts are
unverified. The new active row is `lossless_color_cache_lookahead`.

The public GIF encode matrix now includes a one-frame sequence with exactly one
total play. Its Pillow byte reference is unchanged because no GIF loop extension
is required. The selected parity row reaches the `Finite { total_plays: 1 }`
mapping in `encode.rs:1734`; the selected report has no remaining gap at that
coordinate, with source/build identity unverified.

Three complete 3×1, 4-bpp ICO fixtures pin Pillow's grayscale-palette rules:
identity grayscale becomes raw luminance bytes, non-identity grayscale remains
indexed, and a black/white two-entry palette uses the one-bit interpretation.
All three decode, inspect, and verify through the public manifest parity path.
The identity fixture's input SHA-256 is
`de23f5d6d7ec9047cc6103e2991db5b62fde8cbd9b3d95e078ccf3960d1f5b29`, and its
Pillow RGBA reference SHA-256 is
`45add7ffebacd45767e5608c10f43c1aab79268a2645bfe9f5ab759656af4c22`. The
selected LLVM report observes both outcomes at the odd-nibble guard; Coverage
MCP reports no remaining gap there, with source/build identity unverified.

A new two-frame 16×16 4:2:0 AVIF sequence moves a 4×4 patch by one pixel.
Both frames match Pillow exactly. Its inter frame is all-lossless and selects
the specialized `LosslessB16I420` reconstruction path; a branch-instrumented
public decode observed the previously missing false arm at
`decode_inter_transform_size`'s small-block eligibility check.

A four-frame 256×256 4:2:0 AVIF sequence with moving inter-frame content covers
the large-block side of that eligibility check. Its frames match Pillow and
pinned native loop witnesses. The row also exposed a decoder-state bug: a
skipped transform block inside a non-skipped coding block must propagate its
own `txb_skipped` state to the wide-luma terminal decoder. Passing the enclosing
block state could consume coefficient symbols for an empty transform and
overread the tile; the corrected path passes the per-transform state.

A new 16×16 lossy WebP fixture uses luma-balanced red and green 4×4 tiles.
It reaches the VP8 Y2 residual path with no coefficients, so both neighboring
macroblocks receive a zero Y2 complexity context. The exact Pillow RGB output
matches the public Rust decoder, and targeted LLVM evidence observes the
previously missing false arm at `read_residual_data`.

A 32×32 four-quadrant lossy WebP row extends the public VP8 chroma decode
corpus with a 2×2 macroblock layout and passes exact Pillow output parity. Its
selected run reaches the production chroma DC prediction path. The saved full
LLVM report `coverage-after-adam7-gray16.json` already records hits for that
arm, and the MCP incremental comparison is incomparable, so no coverage gain
is claimed; the row is retained as a decode regression case.

Three further 32×32 lossy WebP rows target VP8 chroma prediction with mixed
neighboring blocks, a horizontal color gradient, and a vertical color
gradient. All three pass exact Pillow RGB parity. Their isolated reports reach
the production `Vp8Decoder<Take<&mut Cursor<&[u8]>>>` specialization's
TrueMotion, Vertical, and Horizontal predictor arms. The aggregate coverage
counts did not increase because the existing full matrix already covered those
arms; the cases add focused regression inputs, not a claimed coverage gain.
Coverage MCP has no source/build receipt for these local reports, so its source
identity is unverified and selected-test status is unknown.

The malformed WebP row
`error_malformed_container_alpha_uncompressed_truncated_payload` starts from
`alpha_uncompressed.webp`, shortens the raw ALPH plane from 4,096 samples to
one, and updates both chunk and RIFF lengths so the container stays complete.
Pinned Pillow 12.2.0 opens and structurally verifies it, then reports
`OSError: failed to read next frame` during load. The public Rust parity row
passes with the matching malformed error category. It exercises the
`Take<&mut Cursor<&[u8]>>` production specialization at
`read_alpha_chunk`'s `read_exact` in `extended.rs:442`; the prior full report
listed that exact region as missing. The current full report and selected
report show it executed. The row-only LLVM report is
`target/release-evidence/coverage-webp-alpha-truncated.json`. MCP comparison
remains coordinate evidence only because its source/build identity is
unverified and test status is unknown.

The companion malformed row
`error_malformed_container_alpha_lossy_gradient_empty_compressed_payload`
keeps the compressed ALPH header and intact lossy VP8 image while removing the
compressed alpha bitstream, then repairs the chunk padding and RIFF length.
Its input SHA-256 is
`dbacedb09b0aec8375255d8d5b509fde9063b018b3497764769e8ea5aa93ba52`.
Pinned Pillow 12.2.0 opens it as 64×64 RGBA and raises
`OSError: failed to read next frame` on load. Its public parity row passes with
the malformed category, and a selected LLVM report records the production
`Take<&mut Cursor<&[u8]>>` specialization without a remaining gap. Comparing
the old and refreshed full reports observes one new region at
`read_alpha_chunk`'s lossless decoder call in `extended.rs:433`; that MCP result
is limited coordinate evidence because source/build receipts are absent.
The selected report is
`target/release-evidence/coverage-webp-alpha-lossy-truncated.json`.

A further malformed WebP row,
`error_lossless_vp8l_short_backreference_truncated_tail`, starts from the active
96×96 RGB `lossless_short_backreference.webp` stream and removes its final two
VP8L payload bytes while repairing both the chunk and RIFF sizes. The complete
13,924-byte container has SHA-256
`6c5dac54aae71b6c8e086096acfd00e4370689f955aa66bbe0227eb06198dd83`.
Pinned Pillow 12.2.0 opens it as RGB and raises `OSError: failed to read next
frame` on pixel load; the public Rust parity row passes. The full coverage
totals did not change. MCP's full-report comparison identifies one newly
observed region in the direct RGB VP8L pixel path at `lossless.rs:754:72`, but
the report has no source/build receipt and the result remains coordinate-only.
The row is retained as a malformed-input regression for a valid header and
short-copy stream that fails during pixel reconstruction, not as an aggregate
coverage increase. Its full report is
`target/release-evidence/coverage-after-webp-avif-shortbackref.json`.

The AVIF compound-reference investigation found a CDF table-axis error that
could panic on a valid context-2 stream. The correction indexes each CDF by
branch group then context. A deterministic full-file AVIF mutation preserves
the compound-reference prefix and damages only a later coded sample; Pillow
and Rust both reject sequence decoding as malformed, while the first image
still matches Pillow exactly. An isolated public `decode_sequence` trace stops
at the corrected backward-reference CDF lookup with group 0/context 2; the old
transposed access would index beyond that table's two group rows. The selected
parity row then returns its structured malformed result. This access occurs in
an earlier AV1 sample than the mutation at byte 16098, so it proves the public
prefix exercises the fix but does not attribute the eventual sequence error to
the changed byte or frame. The valid DIFFWTD stream still reaches other AV1
decoder limitations, so it is not counted as a successful parity case.

An odd-sized 9×9 4-bit grayscale PNG uses Adam7 with all seven passes carrying
samples. Exact Pillow L8 parity exercises packed-sample reconstruction in the
interlaced pass callback, complementing the existing non-interlaced low-depth
fixtures.

A three-page RGB TIFF sequence now uses 9×7, 9×7, and 5×11 pages. The public
sequence decoder matches Pillow for all three frames, and the encoder matches
Pillow's complete byte stream. This extends IFD traversal and inter-page
alignment coverage through a third page with a different extent. The two new
rows add regression coverage but leave the full LLVM totals unchanged; the
Coverage MCP incremental comparison is incomparable because the prior report
does not retain compatible branch detail.

A deterministic 128×64 monochrome AVIF with two tiles now passes exact Pillow
RGB pixel parity in both one-group and split-group forms. The split case sends
tile 0 and tile 1 in separate tile-group OBUs and exercises the accumulated
pending-plus-trailing tile path. A new two-tile AVIF with a monochrome alpha
item and CDEF disabled reaches the CDEF-off reconstruction branch; the complete
RGBA output matches Pillow byte-for-byte.
The new row `multitile_monochrome_alpha_zero_loop_filter` keeps that two-tile
monochrome alpha layout but makes its alpha frame all-lossless, with zero Y/U/V
loop-filter levels. Its generator checks both tile indices, CDEF, lossless state,
and filter levels before accepting the fixture. Exact Pillow RGBA parity passes;
the selected LLVM report observes the missing false arm at
`assemble_monochrome_tiles` in `frame.rs:2246`. That report has no source/build
receipt, so the location remains coordinate evidence. The selected report is
`target/release-evidence/coverage-avif-monochrome-alpha-zero-loop-filter.json`.

A sparse grayscale progressive JPEG pins a 64-ZRL AC-refinement scan with
coefficient correction bits and passes exact Pillow L8 pixel parity. A second
8×8 grayscale row, `progressive_ac_refinement_refine_zrl`, includes refinement
symbols with `r=15, size=0` and also matches Pillow exactly. Its selected
nightly LLVM report records 8 true and 16 false executions at
`ac_refine_block`'s `r != 15` branch; Coverage MCP lists no remaining gap for
the production `FastBitReader` specialization. The full ARM64 report still
lists the generic `BitReader` monomorphization, which this target does not use
for production JPEG decode. The isolated report has no source/build receipt,
so its branch location is coordinate evidence; the local public parity run
passed. This edge case adds regression value without increasing the aggregate
branch count.

An earlier full all-feature `make coverage` run passes every executed test and
remains above all project floors: 100,225/163,401 lines (61.3368%),
15,892/32,398 branches (49.0524%), 5,085/9,372 functions (54.2574%), and
146,750/243,772 regions (60.1997%). The local LLVM report is
`target/release-evidence/coverage-goal-lossless-inter-420-final.json`; it is
not a managed Coverage MCP snapshot. It was measured on the macOS ARM64
working tree on 2026-09-30 with `nightly-2026-07-16`, all Cargo features, and
the complete `make coverage` test campaign; it is not bound to a clean commit
or a source/build receipt. Compared with the prior full local report
`target/release-evidence/coverage-goal-cdef-disabled.json`, the measured totals
show +171 covered lines, +70 branches, +11 functions, and +203 regions. The
new lossless inter-frame AVIF parity row passes the full Pillow pixel check and
observes the eligible lossless transform-size branch in the public decoder.
Coverage MCP's report-coordinate comparison lists 173 newly observed branch
locations across monomorphizations; its reports lack source/build receipts and
test status, so the comparison is limited and does not establish verified
attribution or regressions. The local full campaign itself reports all tests
passing. The 100% target remains open.

The preceding all-feature `make coverage` run passed 118 library unit tests, 3
binary tests, 4 animation-loop tests, 1 capability test, 59 coverage-matrix
tests, 7 decode-policy tests, 1 determinism test, and 68 feature-gate tests.
It reports 106,798/165,795 lines (64.4157%), 16,435/32,508 branches
(50.5568%), 5,578/9,617 functions (58.0015%), and 156,910/247,495 regions
(63.3993%). The local LLVM report is
`target/release-evidence/coverage-goal-jpeg-jcoef-dc-shortcut-final.json`,
measured on macOS ARM64 with `nightly-2026-07-16` and all features. Unit tests
were included after enabling the existing library test target. Coverage MCP
queries on this report no longer list branch gaps for JPEG `range_limit`,
`ycc_to_rgb_batch`, `reconstruct_baseline_444_direct_safe`, or
`jpeg_idct_islow`. The largest remaining group is AV1
`decode_inter_translation_impl`: MCP repeats 4,063 observations at the same
source spans across specialized monomorphizations, not 4,063 distinct source
statements. MCP marks source as unverified and test status unknown, so these
are report-coordinate findings; the local Cargo campaign itself passed. All
four floors pass, while 100% remains open.

An earlier all-feature `make coverage` run passes 119 library unit tests, 3
binary tests, 4 animation-loop tests, 1 capability test, 59 coverage-matrix
tests, 7 decode-policy tests, 1 determinism test, and 68 feature-gate tests.
It reports 106,929/165,923 lines (64.4450%), 16,441/32,508 branches
(50.5752%), 5,580/9,619 functions (58.0102%), and 156,956/247,537 regions
(63.4071%). The local LLVM report is
`target/release-evidence/coverage-goal-av1-has-chroma-final.json`, measured on
macOS ARM64 with `nightly-2026-07-16` and all features. The new table-driven
AV1 test covers monochrome, 4:4:4, 4:2:2, and 4:2:0 ownership at odd/even
origins and one-MI versus larger block axes. Coverage MCP has no remaining
branch gaps for `inter_block_has_chroma`; the rule is `#[inline]` in release
builds. Its next AV1 branch group is `lossy_i422_narrow_mode0`, the
4:2:2-only 4×8/4×16 mode-0 specialization. That group has 1,916 repeated
observations across specialized monomorphizations; they are not distinct
source statements. The full report improves by 131 covered lines, 6 branches,
2 functions, and 46 regions over the preceding full report. MCP reports source
as unverified and test status as unknown, so its gap listings are
report-coordinate findings; the local Cargo campaign itself passed. All four
floors pass, while 100% remains open.

An earlier selected LLVM run of the 59-test `coverage_matrix_tests`
integration target records 97,978/163,390 lines (59.9657%), 15,457/32,404
branches (47.7009%), 4,867/9,369 functions (51.9479%), and 143,525/243,760
regions (58.8796%). It clears the line, branch, and region floors and is five
functions short of the 52% function floor. Every successful JPEG decode input
row also passes through the public token-aware decoder and matches its Pillow
pixel reference. Every successful JPEG encode row passes through both ordinary
and token-aware encoding, and both byte streams match the committed Pillow
output exactly. Every successful JPEG encode roundtrip also passes its encoded
output through the token-aware decoder and compares its pixels with the Pillow
reference, including the 1,024-MCU checkpoint in the 2048×1024 grayscale case.
Coverage MCP reports that checkpoint branch observed. Its selected report
also listed the false arm of the AArch64 dequantization-overflow fallback.
Source review shows full-file parsing bounds progressive coefficients to i16
and DQT entries to u16; the maximum product, 2,147,450,880, fits in i32. That
fallback protects internal states outside the parser path and has no valid
full-file JPEG parity witness. The selected report has no branch-gap groups for
`baseline_frequencies_with_checkpoint`, `append_ac_refine_events`, or
`encode_baseline_420_mcu_pair`.
The MCP report has no source/build receipt and marks test status unknown, so
these are report-coordinate observations; the local Cargo command itself
reported 59/59 passing. A Coverage MCP selected comparison against the previous
selected report shows seven newly observed branch observations, including both
`find_entropy_end` arms, the generic CMYK branch in `reconstruct_image`, and
PNG opaque-chunk/reserved-bit paths. The source/build receipts are unavailable,
so these are coordinate-only comparisons. The selected target does not describe
whole-workspace coverage. The 100% target remains open.

The selected JPEG additions cover baseline CMYK reconstruction with restart
markers and two missing-EOI tails: a dangling `0xFF` and a stuffed `0xFF00` at
EOF. Pillow error contracts remain exact, and the selected comparison observes
both `find_entropy_end` arms plus the generic four-component branch.

The same selected run adds Pillow-parity PNG cases for duplicate gAMA, sRGB,
cHRM, and iCCP chunks; invalid and short sRGB payloads; malformed short cHRM;
an empty-key iCCP chunk; and valid, invalid, and edge compressed text metadata.
It also covers a Pillow-tolerated unknown ancillary chunk with a lowercase
reserved bit in static PNG and APNG inputs, preserving exact pixels and frames.
`text_chunks.png` now contains tEXt, zTXt, and iTXt. All active PNG decode rows
pass exact pixel comparison. Typed source-color assertions cover valid values
and duplicate chunks where Pillow and Rust expose the same declared color.
Malformed short sRGB and empty-key iCCP rows assert pixels only because their
Pillow metadata differs from the decoder's spec-aware typed `SourceColor`.
Coverage MCP's selected comparison reports 10 newly observed branch
observations in `retain_color_chunk`, with three false arms still uncovered;
its gap query lists no remaining branch group for
`invalid_compressed_metadata_identity`. The remaining color-chunk arms require
short gAMA data, an iCCP keyword without a NUL terminator, or an iCCP method
without profile bytes. Pinned Pillow rejects those images, so they cannot be
added as parity fixtures. The coverage report lacks source/build receipts and
MCP reports test status unknown; the local run passed 59/59. This remains
selected-target evidence, not whole-workspace coverage.

Coverage MCP reports no remaining branch-gap groups in
`encode_baseline_420_mcu_pair` after the edge row and removal of redundant
first-column checks implied by the MCU loops. The all-feature run's gap query
listed 12 remaining AC-refinement branch observations for `BitReader`; the
`FastBitReader` group had disappeared after the repeated sparse-refinement row
and once-per-scan bound check. AV1 `decode_inter_translation_impl` groups
remained the largest branch gap in that run. After adding the AC-first narrow-tail fixture and matching
libjpeg-turbo's padded-table write behavior, Coverage MCP no longer lists the
`FastBitReader` AC-first or AC-refinement gap groups. The malformed
category-16 fixture also closes the fast-reader `dc_first_block` error group.
The selected ARM report still shows generic `BitReader` groups for those
functions; the public JPEG path selects `FastBitReader`. This is selected-run
evidence, not a full-suite result. The new `Ss=64` and `Se=64` JPEG rows
reach both scan-range error paths. The larger 128×128 JPEG scan-tail row adds
multi-block parity coverage. MCP reports have no source/build receipts, so
their comparisons and gap listings are report-scoped evidence, not standalone
source-provenance claims.

The selected AVIF report passed all active AVIF matrix rows. Coverage MCP's
query at `block.rs:55046` dropped from ten to nine missing specialization
groups, and the previously unobserved block-width branch recorded both sides
in the selected AVIF lane. The local report is
`target/release-evidence/coverage-parity-avif-current.json`; MCP reports lack
source/build receipts and report test status as unknown, so this remains scoped
coverage evidence rather than a full-suite claim.

The row-selected JPEG encoder report exercised the new micro-image cases.
Coverage MCP compares the five-case report with the earlier two-case batch
report and reports five newly observed branch arms: the aligned/even-height
decision, the 4:2:0 row remainder and even-height fallback, and both arms of
the generic RGB conversion loop. Gap queries show no remaining observations at
those locations; the fused helper's two zero-dimension guard arms remain.
Public zero-sized JPEG inputs fail validation before this helper. These local
LLVM reports have no source/build receipts and MCP test status is unknown, so
their branch comparison is limited selected-run evidence, not an aggregate
coverage or regression claim. Direct all-feature parity runs passed both JPEG
decode and encode matrix tests. Reports are under
`target/release-evidence/coverage-parity-jpeg-kernel-edges-final.json` and
`target/release-evidence/coverage-parity-jpeg-420-batch-cases.json`.

The selected decode report's largest group is AV1
`decode_inter_translation_impl`. Raw LLVM data shows ten monomorphized closure
instantiations at the reported location, with execution recorded in two for
this run. Coverage MCP's 1,928 observations therefore span unvisited specialized
paths; they are not 1,928 distinct source statements. A useful next parity pass
needs to identify the missing supported AV1 syntax paths before selecting or
generating fixtures.

Two local release A/B runs per source version exercised the public JPEG
benchmark matrix on an Apple M3 Pro, with three alternating rounds per run.
Across 63×65, 127×129, 128², 512², and 1024² RGB 4:2:0 encodes, the median of
the run medians was 5.3–7.8% lower after the MCU-check cleanup; Rust output
lengths and hashes matched before and after in all five cases. One baseline run
started with a 1-minute load average of 10.7, so these are directional local
measurements rather than a cross-machine or release speed claim.

A separate release profile used the public Rust runner for a 1024×1024 RGB
4:2:0 encode (2,000 measured iterations after 100 warmups) on the same M3 Pro.
The median was 6.806 ms and P95 was 7.200 ms. A five-second `sample` capture
recorded 3,866 stacks; `rgb_to_ycbcr_420_packet` appeared in 340 stacks (8.8%)
and the four-block FDCT in 221 (5.7%). This is a single-workload diagnostic
that points to color conversion for follow-up profiling. It is not a paired
speedup measurement, and it provides no SSE2 or AVX2 runtime evidence.

A same-checkout A/B isolated `#[inline(always)]` on
`rgb_to_ycbcr_420_packet`. Seven alternating release-runner pairs on the M3 Pro
measured 128², 512², and 1024² RGB 4:2:0 encodes. The candidate medians were
2.61%, 0.34%, and 3.18% lower, respectively; every encoded length and hash
matched. These small local gains support retaining the hint. The run does not
measure SSE2 or AVX2 execution. Raw paired results are in the ignored local
artifact `target/benchmarks/jpeg/goal-continuation-2026-09-29/packet-inline-controlled-ab.json`.

A follow-up experiment replaced four overlapping 8-pixel conversions in each
16-pixel RGB row with three 16-byte loads and mask-based channel deinterleaving.
The public JPEG encode parity lane passed and all 20 encoded outputs matched the
stored baseline hashes and lengths. However, a five-round full JPEG matrix on
the M3 Pro measured an encode median ratio of 1.0031 and a geometric-mean ratio
of 1.0067 versus baseline; RGB 512² and 1024² 4:2:0 encodes regressed 5.15% and
3.69%. The packet-body change was reverted. This result is directional ARM64
evidence only, not an SSE2 or AVX2 runtime measurement. The local candidate
report is `target/benchmarks/jpeg/goal-20261001-arm64-3load-candidate`.

A bounded public-path CMYK experiment streams each decoded component block into
its final channel for aligned baseline 4:4:4 images on AArch64. Five
alternating release rounds reduced median latency by 21.13% at 512×512
(6.187 ms to 4.880 ms) and 18.24% at 128×128 (0.254292 ms to 0.207917 ms);
output hashes matched in every round. The helper keeps the existing packet path
for partial edges and restart scans, and is not compiled for x86, where the
existing SIMD path is unchanged. The selected Pillow decode matrix passed all
three active CMYK cases, including the aligned fast-path case, odd partial
blocks, and restart rows. `cargo fmt --check`, strict release library Clippy,
and `git diff --check` passed. Round data and the exact candidate patch are in
`target/jpeg-cmyk-fusion-experiment/`; these are local Apple M3 Pro measurements,
not SSE2/AVX2 runtime evidence.

The preceding full local `make coverage` campaign on 2026-09-30 passed every
executed test and the unchanged release floors: 100,431/163,617 lines
(61.3818%), 15,947/32,444 branches (49.1524%), 5,097/9,395 functions
(54.2523%), and 147,132/244,178 regions (60.2560%). Its LLVM report is
`target/release-evidence/coverage.json`; it was measured on macOS ARM64 with
`nightly-2026-07-16` and is not bound to a clean commit. The fresh Coverage MCP
gap query still lists AV1 `decode_inter_translation_impl` as the largest
branch group. MCP source/build receipts are unavailable and test status is
unknown, so these locations are report-scoped observations rather than verified
test attribution or a managed snapshot.

The AV1 gap query at `block.rs:55105` returns repeated
`decode_inter_translation_impl` observations across specializations. The
apparent lossy I422 narrow path requires transform mode 0 and a non-lossless
segment. Valid frame parsing sets mode 0 only when `all_lossless`, and derives
that flag as the conjunction of every segment's lossless state, so no valid
frame can satisfy the helper's `!segment_lossless && transform_mode == 0`
predicate. This is an unreachable implementation branch, not a missing public
fixture; no parity row should be invented to exercise it. A separate reachable
candidate is diff-weighted compound prediction at `block.rs:57292` and
`:57310`. No candidate is accepted without independent syntax inspection and
exact Pillow/native frame parity.

An earlier full `make coverage` run passes all executed tests and project
floors: 101,330/163,618 lines (61.9308%), 16,104/32,444 branches (49.6363%),
5,124/9,395 functions (54.5396%), and 148,460/244,179 regions (60.7997%).
This run includes the Adam7 4-bit grayscale PNG and large-motion 4:2:0 AVIF
parity rows. The AVIF sequence row exposed and now covers propagation of the
per-transform skip flag to the wide-luma terminal decoder; all active AVIF
decode rows pass. The 59-test integration target, 68 feature-gate tests,
7 decode-policy tests, 4 animation-loop tests, 3 binary tests, and the
determinism test pass. The local report is
`target/release-evidence/coverage.json`, from macOS ARM64 with
`nightly-2026-07-16`; it is not bound to a clean commit. Coverage MCP reports
16,340 uncovered branch observations and keeps AV1
`decode_inter_translation_impl` as the largest group, with repeated hits
across monomorphizations. It reports no remaining gap at the large-block
eligibility check in `motion.rs:2278`. MCP source/build receipts are absent and
test status is unknown, so these are report-coordinate findings; the local
full campaign supplies the passing test evidence. The 100% target remains open.

The preceding full `make coverage` campaign on 2026-09-30 passes all executed
tests and the release floors: 104,438/163,657 lines (63.8152%),
16,770/32,444 branches (51.6891%), 5,251/9,398 functions (55.8736%), and
152,993/244,210 regions (62.6481%). The local report is
`target/release-evidence/coverage.json`, measured on macOS ARM64 with
`nightly-2026-07-16`; it is not bound to a clean commit. Coverage MCP reports
15,674 missing branch observations, with the largest group at AV1
`decode_inter_translation_impl` (`block.rs:55105`); repeated specialization
records are observations, not distinct source branches. At the WebP Y2
`read_residual_data` false edge, LLVM records 57 executions in a covered
specialization and a duplicate zero-count generic record, while Coverage MCP
still lists missing branch records at that coordinate. Keep that edge marked
ambiguous in MCP rather than calling its gap closed. MCP reports source/build
identity as unverified and test status as unknown, so its results are
report-coordinate findings; the local full campaign supplies the passing test
evidence. `make lint`, `make lint-x86-simd`, `make test-feature-matrix`, and
`make verify` also pass. SSE2/AVX2 checks validate formatting and compilation;
they do not establish runtime speedups. The 100% coverage and fastest-decoder
goals remain open.

A Coverage MCP query against that earlier report found no supportable new
non-AV1 parity fixture. Its JPEG `ac_refine_block<BitReader>` gaps are for the
x86 decoder specialization; this ARM64 run uses `FastBitReader`, whose public
progressive ZRL cases are already in the parity corpus. The reported
`progressive_dequantize_block` fallback only follows a DC-times-quantizer
overflow, which valid parser bounds prevent. These are target-specific or
unreachable observations, not reasons to add duplicate or invalid fixtures.

The AV1 `decode_following_vertical` gap was checked with selected public parity
rows on 2026-09-30. `coverage_h4_horizontal_bands` and
`coverage_h16x4_predictor_adst_dct_01` both pass the matrix parity test, but a
fresh branch-instrumented run of the latter records zero calls to
`Lossy420Decoder::decode_following_vertical`. The public decoder therefore
does not exercise this bounded first-leaf helper for those H4 streams, despite
their encoded topology. Source inspection shows its only callers are the
bounded 4:2:0 split helpers. Those callers fix chroma sampling to 4:2:0 and pass
no full-resolution edges, so the method's Full-only entry checks cannot be
covered by a valid public fixture on the current dispatch path. Coverage MCP
also reports both outcomes missing for the `full_strict` condition at
`block.rs:67502`: the 4:2:0 leaf helpers hardcode `Subsampled420`, and their
closed-frame callers require 8-bit input. The generic decoder's Full/high-depth
following path uses `decode_following_from_edges` instead. Keep this gap open
until a supported public path can exercise it or the obsolete path is removed
with a behavior-preserving refactor; do not add a parity fixture that merely
matches the topology label. Selected reports are
`target/release-evidence/coverage-h4-horizontal-bands.json` and
`target/release-evidence/coverage-h16x4-predictor.json`; Coverage MCP marks their
source identity unverified and test status unknown.

A fresh 2026-10-01 Coverage MCP read of `target/release-evidence/coverage.json`
reports 1,512 missing-branch groups and 14,972 missing branch observations.
The `decode_following_vertical` group has 342 repeated observations across
monomorphizations; filtering to `block.rs:67501` isolates the two missing
outcomes of its `full_strict` condition. Fresh selected parity profiles for
the H4/H16 fixtures and the 16x16 4:2:0 SPLIT fixtures all passed, yet recorded
zero executions across the method and its 102 closure records. This confirms
that their encoded partition labels do not reach this helper on the public
dispatch path. Coverage MCP still marks report source identity unverified and
test status unknown, so these are coordinate-only observations, not a verified
coverage gain. No duplicate fixture was added.

On 2026-09-30, a portable `u32x4::widening_mul` experiment for four-lane JPEG
reciprocal quantization preserved Rust output lengths and hashes across all 40
rows of the public JPEG benchmark matrix. Two five-round full-matrix repeats
per implementation, run in opposite order on the aarch64-apple-darwin host,
showed the candidate's 1024×1024 RGB 4:2:0 encode median 9.63% and 14.02%
slower than baseline; the median slowdown across the 20 encode workloads was
10.69% and 15.37%. The candidate was removed. Results are local directional
evidence only and do not measure SSE2 or AVX2 runtime performance. Raw reports
are under the ignored local artifacts
`target/benchmarks/jpeg/goal-quantizer-{baseline,candidate,candidate2,baseline2}-2026-09-30/`.

A subsequent all-feature `make coverage` campaign on 2026-09-30 passes every
executed test and the release floors: 104,471/163,650 lines (63.8381%),
16,807/32,432 branches (51.8223%), 5,251/9,398 functions (55.8736%), and
153,027/244,201 regions (62.6644%). The local report is
`target/release-evidence/coverage.json`, measured on macOS ARM64 with
`nightly-2026-07-16`; it is not bound to a clean commit or a managed Coverage
MCP receipt. The 100% target remains open.

The WebP matrix adds a 32×16 public parity row with a flat macroblock beside a
deterministic high-frequency macroblock. Its exact Pillow output passes the
selected decoder matrix test. The selected LLVM report
`target/release-evidence/coverage-webp-macroblock-candidate.json` invokes the
`Cursor<&[u8]>` specialization of `Vp8Decoder::read_residual_data` and records
both outcomes at the plane-selection and luma complexity-update branches.
Separate selected runs of the existing public rows
`lossy_vp8_lossy_macroblock_residual_context_edges_32x16_q75_m4` (input
SHA-256 `9bbe53e9597854a1f5412ab40b1ee9f54bcf56f6c8513d254d69dee85fa74abb`)
and `lossy_vp8_lossy_y2_empty_ac_16x16_q90_m4` (input SHA-256
`e5e6cc56cf73aded4521a16de4ef0d08ff4066db75cdcd5527b15e6210d4ea98`) both
pass exact Pillow pixel parity. Their row-only LLVM reports record `n=true`
and `n=false`, respectively, at `src/codecs/webp/native/vp8.rs:1583-1584` in
the production `Vp8Decoder<Take<&mut Cursor>>` specialization. The reports
have SHA-256 `4723f808dd1331cf4b0cd92639aaab47958c5f0dee08a0ef356da1dacd146109`
and `7c0769e6f8fa66ab048c442e6552b865f128bfabd60d670e2590d4430064f1b9`.
The aggregate MCP miss is in the separate `Vp8Decoder<Cursor<&[u8]>>`
specialization; MCP source/build identity is unverified and selected-test
status is unknown, so this is coordinate guidance rather than a source-bound
coverage claim. No private-only coverage test was added.

A 128×64 lossless RGB WebP row, `lossless_color_cache_lookahead`, covers the
VP8L color-cache lookahead branches through the public decoder. Its deterministic
512-color input has SHA-256
`920481d0986bf9034730ebdd4678c65540c985dc00c25c0cc293d45167b3e541`; the
Pillow 12.2.0 RGB8 reference has SHA-256
`d35118946b17473c290fc38351acb637a8887d4248de5052a3e6e2f305ddd35c`. The
independent VP8L inspector finds one untransformed stream, cache width 10, and
6,252 cache lookups. The selected all-feature matrix row passes exact Pillow
parity. Its isolated local LLVM report records both outcomes at
`lossless.rs:828` (10,572/93), `:829` (10,113/459), and `:830` (8,091/2,022),
true/false respectively. This is selected-row evidence, not a refreshed
Coverage MCP measurement or complete coverage claim.

The public VP8L cache-boundary corpus also includes a 2×1 RGBA input with
`color_cache_bits = 11`. Its dynamic green tree emits a literal whose cache hash
is 2047, then performs a real lookup of that highest entry. The independent
structural inspector records both the width and referenced cache index; Pillow
12.2 decodes the exact pixels. The focused matrix test passes. Coverage MCP
reports the focused branch profile as 135/31,268 observations, with source
identity unverified and test status unknown. Incremental comparison with the
saved full baseline is incomparable because its exact branch detail does not
match the summary totals, so this case is recorded as a maximum-boundary parity
witness rather than a claimed marginal branch-coverage gain.

The same change simplifies AV1 lossless inter-transform selection to use its
prevalidated geometry predicate directly, removing a repeated exact-geometry
check and duplicate block/layout guards. The matrix adds a two-frame 8-bit
monochrome lossless inter sequence with a translated patch. Its generator
disables warped motion to stay within the decoder's admitted reconstruction
profile; exact Pillow frame/timing parity and pinned native loop evidence pass.
Coverage MCP reports no missing branch observation at
`decode_inter_transform_size` line 6950; the true arm at line 6951 remains
missing at that checkpoint. The report's source/build identity is unverified
and test status is unknown, so this is coordinate evidence rather than an
attributed MCP gain.
Strict Clippy now denies `missing_errors_doc`; the eight
previously undocumented public `Result` APIs have `# Errors` sections, and
`make lint` passes. This lint and documentation add no runtime work. No SSE2 or
AVX2 runtime benchmark was available on this ARM64 host, so no x86 speedup is
claimed.

The 8-bit color lossless inter-transform gap has public parity fixtures:
`animated_lossless_inter_420_b32x32.avif` uses a translated 4x4 patch inside an
exact-visible 32x32 block and is pinned to input SHA-256
`65e8617044f7f12db081f65276235296f8dc208a006451210ebb9603f27f20a7`; the new
`animated_lossy_split_inter_420_b16x16.avif` exercises a lossy B16x16 transform
split and is pinned to input SHA-256
`9eb50f5a45dc2eb549c62ac9bcb67abd931a977076685bcf3c0f69e6827ce323`.
`animated_lossy_wide_monochrome_b128x128.avif` is a four-frame 8-bit
monochrome sequence with three lossy B128x128 inter blocks using switchable
transform splits; it is pinned to input SHA-256
`eb8dda5000882ffd03c182a817e08fc944ef110afa1e78aaf1678bef5efc65bf`. All
three fixtures match Pillow frame pixels exactly, and native sequence evidence
records each displayed frame.

The three-frame `animated_lossy_b16_mixed_topology_inter_420_b16x16` sequence
adds an inter B16x16 whose top-right TX8 child splits while the other three
children stay unsplit. The deterministic Pillow 12.2.0/libavif 1.4.1 generator
is `scripts/generate_avif_b16_mixed_topology_fixture.py`; its 1,186-byte input
has SHA-256
`3264482e2a50d80bd39be178b843fdfe9997e947d54b4466c745751837821d74`. The exact
Pillow/Rust frame hashes are
`7f3e5e4e65eca4390e9242558012bc9bdad133d7ac9f6aed53fa156a2288f73b` (frames 0
and 1) and
`e195092d99a776d7638ff75a30825447f6dffd85a04fa845750617ce04a98a79` (frame 2).
The selected public row passed 1/1 under branch-enabled isolated coverage in
`target/release-evidence/coverage-avif-b16-mixed-topology-row-20261004.json`.
Its `entropy.rs:7778` branch counts are true=1, false=0, proving the
`any(child_split)` true arm; at line 7775 they are true=0, false=1, leaving the
all-children-split true arm unobserved. The line 7778 false arm is also still
unobserved. Coverage MCP comparison to the older full AVIF report is limited
and shows zero aligned coordinate gains because the receipts and inventories
differ, so this selected-row observation is not an aggregate coverage gain.

The 10-bit public row `animated_lossless_inter_420_b32x32_10bit` now materializes
its sequence instead of returning the former Rust-only `Unsupported` result.
Its header permits warped motion, but the checked-in 32×32 stream has one
top-left B32×32 leaf and no overlappable neighbor; the frame flag therefore adds
no local-warp mode syntax for this input. The high-depth admission predicate no
longer rejects that capability flag, while per-block mode and geometry checks
remain in `decode_inter_leaf`. The existing row passes exact Pillow parity for
both RGB frames and sequence timing/loop metadata; no new fixture or unit test
was added.

The 2026-10-02 macOS ARM64 all-feature, branch-enabled `make coverage` run passes
all executed tests and the repository floors: 85,551/142,082 lines (60.2124%),
16,248/32,170 branches (50.5067%), 4,791/8,972 functions (53.3995%), and
130,676/219,579 regions (59.5121%). Its report is
`target/release-evidence/coverage-full-avif-high-depth-parity-20261002.json`.
Coverage MCP still reports unvisited branches in
`decode_inter_transform_size`; its source/build receipt is absent, so its
comparison is coordinate guidance and does not establish a verified coverage
delta. The 100% target remains open.

The public row `animated_opidc_0x101` transforms the pinned animated AVIF
fixture (input SHA-256
`25b79a856ea2767e02e5a509f72e7af3f9563202301779be65b0745724ab23be`) to use
operating-point IDC `0x101` and explicit temporal-0/spatial-0 extension headers
for its seven layer-specific OBUs. Its five 150×150 RGB frames match Pillow
byte for byte, and the native loop witness confirms all five displays. The
selected public parity command reports `1/1` active rows passed; the native
repetition witness test also passes. Before this row, Coverage MCP listed the
false arm at `src/codecs/avif/av1/frame.rs:952` as missing. After the full
coverage run, its line-filtered branch query returns no missing group at 952;
the function still has gaps at lines 947 and 964. MCP marks source/build
identity unverified and test status unknown, so this is coordinate guidance;
the local instrumented campaign is the executed test evidence.

A fresh all-feature `make coverage` run passes every executed test and the
release floors: 104,619/163,698 lines (63.9098%), 16,854/32,444 branches
(51.9480%), 5,262/9,405 functions (55.9490%), and 153,241/244,291 regions
(62.7289%). Compared with the preceding full report, it observes four
additional lines, four branches, and four regions; function coverage is
unchanged. The new 32×32 lossy I444 AVIF parity row exercises mode-2 and positive
qindex outcomes before its geometry excludes the separate 64-axis split path.
The full report still has 247 uncovered branch observations in
`decode_inter_transform_size`. Coverage MCP marks source/build identity
unverified and test status unknown, so its gap coordinates are guidance. The
report SHA-256 is
`79cc1056db899cd493dea18492556626d95c8e91bab4da1250ddf96d77d58149`.
Raw LLVM counters observe six true and 24 false executions at
`src/codecs/avif/av1/block.rs:65010` in a wide-chunk decoder specialization,
and the true arm at `block.rs:55789`
was reached three times in the lossy decoder specialization. Coverage MCP still
lists misses in other monomorphizations. The largest general AV1 gap remains at
`block.rs:55105`, which prior parsed-state analysis found unreachable for
legal lossy input. JPEG progressive gaps belong to the non-active `BitReader`
monomorphization on this ARM64 host; the active public `FastBitReader` path is
covered, and parsed JPEG state cannot reach the reported dequantization
overflow fallback. MCP reports no remaining ICO decode branch group. The WebP
cost/quant interpretation in that report depended on private coverage-hook
monomorphizations that have since been removed; those old coordinates are not
current evidence. The current public parity report above still lists WebP VP8
segment-delta and VP8L bounds outcomes. Coverage MCP marks source/build identity
`unverified` and test status `unknown`, so the results are coordinate guidance
only. The 100% target remains open.

ICO adds four 3×1 public 4-bpp parity rows for the black-and-white white-bit
arm, two distinct two-entry palette predicate failures, and an out-of-range
palette reference. Pillow decodes the missing palette index as black; the Rust
decoder now uses a fixed 16-entry table with black-filled unused entries. That
removes the separate palette-reference scan while preserving the existing
short grayscale-palette errors. The selected rows pass 4/4, and Coverage MCP
reports no missing decode branch in `src/codecs/ico/decode.rs` for the full
report.

JPEG adds a minimal public parity fixture,
`progressive_ac_refinement_eobrun_correction_bit`, for a 16×8 two-block
grayscale progressive stream. Its repeated 8×8 source pattern is pinned to
input SHA-256 `d1823abbbd3f9b78b440eb25abbb38b73db7dc30be1457d013131d481038c3eb`;
the selected public decode row matches Pillow pixels exactly. The isolated
instrumented run reaches the AC-refinement EOBRUN correction arm through
`FastBitReader`. The full local report remains unchanged because the remaining
true arm is the separate `BitReader` instantiation used on x86, which this
ARM64 host does not execute. The existing x86 CI lane runs the public JPEG
matrix and retains branch reports; no x86 runtime result is claimed here.

A fresh all-feature `make coverage` campaign on 2026-09-30 passes every
executed tests and the release floors: 104,452/163,691 lines (63.8105%),
16,752/32,454 branches (51.6177%), 5,254/9,404 functions (55.8698%), and
153,135/244,275 regions (62.6896%). It used `nightly-2026-07-16` on macOS
ARM64. The report is local to the dirty worktree and contains no revision or
build receipt. Coverage MCP therefore labels its source identity unverified
and test identity unknown; the successful `make coverage` exit is the local
test-run evidence. These floor passes do not meet the separate 100% target.
The current report leaves 59,239 lines, 15,702 branch observations, 4,150
functions, and 91,140 regions uncovered. `make fmt`, `make lint`,
`make lint-x86-simd`, and `make test-feature-matrix` also pass. The matrix
passes native and WASI runtime lanes plus browser-WASM compile/documentation
lanes; its capability tables agree. SSE2-baseline and AVX2 x86 builds pass
strict Clippy, which validates compilation but is not an x86 runtime benchmark
or speed claim.

Two new public parity rows extend the corpus. JPEG
`progressive_ac_refinement_zrl_symbol` exercises a ZRL across coefficients
1–16 before introducing coefficient 17; its selected row matches Pillow and
the instrumented ARM64 run covers both outcomes in the `FastBitReader`
specialization. Coverage MCP still reports the portable `BitReader` false
arm at `progressive.rs:209`, which the ARM64 public matrix does not exercise. AVIF
`animated_lossy_wide_i444_b128x128` adds exact two-frame parity for a lossy
4:4:4 sequence with 128×128 superblocks. Its selected run does not satisfy the
size predicate at `block.rs:65010`, so it does not cover the specialized wide
chunk branch. The remaining `decode_following_vertical` observations are in a
diagnostic fallback that the current public 4:2:0 decode path cannot reach;
no synthetic coverage-only input was added.

On 2026-10-01, the active public row
`progressive_ac_refinement_zrl_duplicate_tail` passed exact Pillow pixel parity
(1/1). Its repeated AC-refinement scan is the targeted edge case for the
correction-bit no-op at `progressive.rs:226`. Coverage MCP reports no remaining
branch group there for the ARM64 `FastBitReader` specialization; the listed
missing outcomes belong to the portable `BitReader` monomorphization, which
this target's public matrix does not exercise. The selected-format report has
no source/build receipt, so the mapping is local coordinate evidence rather
than a verified full-suite comparison. This active row already covers the
reachable path, so no additional fixture was justified. x86 runtime parity and
performance still require matching hardware.

Coverage MCP reports no missing branch observations in JPEG
`ycc_to_rgb_batch` or AV1 `inter_intra_context` in this report. The public
`baseline_444_partial_blocks` JPEG row exercises the scalar RGB tail. The new
deterministic 17×17 two-frame 4:2:0 sequence has exact Pillow references for
both frames. Differential decoding exposed and fixed two sequence-path
defects: one-intra/two-neighbor inter-intra context selection, which previously
overread tile padding, and final-column odd-width chroma upsampling, which
blended an out-of-edge sample. Both frame hashes match Pillow after the fixes.

Coverage MCP on the current local report lists 191 missing branch observations
in `decode_inter_transform_size`. Its current full report observes the
`transform_mode == 0` arm and the TX64 `offset_x == 0` decision; clipped-geometry
and high-depth guards remain open. Pinned AOM searches still find input
geometries that the safe Rust sequence decoder rejects, so those candidates
remain outside the Pillow parity corpus. The largest
AV1 groups remain the I444 split predicate at `block.rs:54939` and the vertical
I422 TX4 chroma grid at `block.rs:54973`; observations repeat across decoder
monomorphizations. The new I444 parity row covers the predicate true arm for
some instantiations, while the separate terminal body and other
monomorphizations remain open. The existing active public row
`animated_motion_chroma_422` reaches the vertical TX4 predicate's true arm in
one `decode_inter_translation` specialization; its row-filtered LLVM report
records 3 true and 0 false evaluations with exact four-frame Pillow parity.
Other decoder monomorphizations remain uncovered, so no duplicate I422 fixture
was added.

Coverage MCP also reports 114 missing branch observations in
`high_depth_lossy_i444_superres_restoration_supported`, beginning at
`entropy.rs:13514`. The existing 8-bit I444 animated parity fixtures exercise
the generic I444 operand, but not the high-depth operand. The pinned
Pillow/libavif AOM encoder rejects a 10-bit animated probe, and Pillow's AVIF
save interface does not expose a bit-depth option, so there is no reproducible
oracle-backed public candidate to add yet.
At the time of this historical report, an 8-bit I420 super-resolution/
restoration fixture was blocked because stock pinned libavif rejected the
required encoder controls. On 2026-10-04, a reproducible fixture-only patch
under `scripts/avif_fixture_oracle/` exposed those controls and enabled a
public parity case. The active case uses largest-transform mode and 16x16
partitions; its two exact Pillow frame hashes are recorded in
`tests/fixtures/input/images/avif/README.md`. This does not close the separate
high-depth I444 restoration gap.
Coverage MCP reports four missing branch observations in the defensive
empty/zero-size guard at `decode.rs:1049` inside the 4:2:0 sampler. No public
AVIF fixture has been identified that reaches this helper with an empty plane
or zero geometry; the gap remains open instead of adding a private-only test.
It also reports repeated observations around `decode_following_vertical` at
`block.rs:67502`. Source review found its four direct callers inside two
bounded 4:2:0 helpers. Both constructors hardcode 4:2:0, and their closed-frame
callers require 8-bit input; generic Full/high-depth following blocks use a
separate decoder path. The `full_strict` branch is therefore not reachable
through today's public decoder call graph. These MCP coordinates are
unverified because the local report has no source/build receipt. The 100%
target remains open.

### Previous full-suite snapshot before parity-only cleanup — 2026-10-02

The last recorded all-feature `make coverage` run before removing private
coverage sweeps passed its then-current tests and release floors: 108,537/162,403
lines (66.8319%), 17,646/32,238 branches (54.7366%), 5,456/9,374 functions
(58.2035%), and 159,061/242,607 regions (65.5632%). It ran with
`nightly-2026-07-16` on macOS ARM64 and wrote
`target/release-evidence/coverage.json`. This report predates the current source
and fixture set and is historical, not a measurement of the current tree.
Coverage MCP reports its source identity as unverified and test status as
unknown because the report has no source/build receipt. Its branch observations
cannot be attributed to current public parity rows. The selected public matrix
measurement above is current but does not establish full-suite floors or 100%
coverage.

That earlier MCP report listed a missing outcome in an AV1 decoder
specialization. Its coordinate (`block.rs:21108`) predates source cleanup. The
format rule still requires `delta_q_present = 0` when `CodedLossless = 1`, so a
conforming parity input cannot exercise the opposite state
([AV1 bitstream semantics](https://github.com/AOMediaCodec/av1-spec/blob/master/07.bitstream.semantics.md#codedlossless-semantics)).
No synthetic case was added to claim that unreachable state.

The active `portable_lossless_filmgrain_i444_64x64` row exercises a 64×64
8-bit all-lossless I444 AV1 still with film grain. The full-resolution I444
admission uses the existing bounded grain-dimension check, then the public
decode path applies the grain kernel. The row is one of 393 active AVIF decode
cases and passes the current public matrix with exact Pillow parity. Its RGB
reference is 12,288 bytes with SHA-256
`44f85ee642e036ae8646b40b2a71643f1e74a27d2a5a6af719c938d8aba2dceb`, and its
AVIF source SHA-256 is
`5e40ca71068fdd232d11f356103f53c1fdd7b94356fa91b0004d7307de4cbd25`. The
historical full coverage run recorded 859 more covered lines, 110 more
branches, 64 more functions, and 1,335 more regions than its predecessor. The
report has no source/build receipt, so that coordinate change is not a
test-attributed gain.

Progressive JPEG reconstruction now selects the existing safe fixed-point
`wide` IDCT on x86_64 as well as AArch64. The x86_64 baseline uses SSE2; the
AVX2 configuration is a separate compile-time target-feature build. The
all-feature JPEG decode matrix passed all 163 active rows on this ARM64 host,
which exercises the AArch64 path only. `make lint-x86-simd` passed strict
Clippy for x86_64 SSE2 and AVX2 builds. CI runs the public JPEG decode and
encode parity matrices with branch reports on both x86 configurations, and the
benchmark workflow measures both variants on the same AVX2-capable host. No
x86 runtime parity or speedup result is claimed until those artifacts are
available.

A separate three-round, 40-workload pre-candidate ARM64 JPEG diagnostic
compared the then-current build with TurboJPEG. RGB 4:2:0 encode medians were 1.82× slower at 512×512
quality 85 and 1.89× slower at 1024×1024 quality 85; their output lengths and
hashes matched. The run is a baseline, not an A/B candidate comparison or an
x86 measurement. Its artifacts are local under
`target/benchmarks/jpeg/goal-baseline-20261002/`; the results identified the
RGB 4:2:0 encode path for further full-workload profiling.

A 3-second macOS `sample` capture during 2,000 public 1024×1024 quality-85
RGB 4:2:0 encodes observed 2,320 stack samples: 1,953 ended in
`encode_baseline_420_mcu_row_streaming`, 215 in aligned RGB conversion, and 148
in packed FDCT. The optimized runner reported a 6.569 ms median and 7.004 ms
p95, with the same 793,139-byte output and hash `d748543a168ce6aa`. The row
pipeline inlines its transform, quantization, and entropy work, so this sample
does not separate those costs enough to justify a stage-specific SIMD change.
The run and sample are under
`target/benchmarks/jpeg/goal-encode-rgb1024-q85-arm64-debuginfo-20261002.*`;
they are profiling evidence, not a controlled performance comparison.

The aligned RGB 4:2:0 MCU-row fast path reuses the existing packet conversion
kernel and keeps the generic edge-replication path for partial dimensions. Its
public JPEG encode parity matrix passes all 61 active rows. A 32×33 RGB case
reaches the streaming converter with aligned width and odd height; Coverage MCP
no longer reports missing branches in this converter. The unreachable padded
width comparison was removed because `ceil(width / 16) * 16` equals `width`
whenever the preceding width-alignment condition is true. A balanced local
baseline/candidate/candidate/baseline comparison, with five rounds per build
and the full 40-workload matrix on the same ARM64 Mac15,7, measured a 2.13%
lower geometric-mean latency across encode workloads and a 1.65% lower mean
across the 512×512 quality-10/85 and 1024×1024 quality-85 RGB 4:2:0 cases.
Decoder latency changed by -0.10%; input and Rust output length/hash matched
for all 40 workloads. The measurements are directional ARM64 evidence, not an
x86 SIMD result or a cross-machine performance claim. Raw runs are under
`target/benchmarks/jpeg/goal-aligned-ab-{b1,c1,c2,b2}-20261002/`.

The JPEG quantizer now omits its upward quotient correction: prepared JPEG
divisors are 8–2040 and use `ceil(2^32 / divisor)`, so multiply-high cannot
underestimate the quotient. The single downward correction remains, and the
public JPEG encode parity matrix passes all 61 active rows. Ten alternating
500-encode captures of the public 1024×1024 RGB quality-85 4:2:0 workload on an
Apple M3 Pro measured a 1.14% lower median of run medians (6.271 ms candidate,
6.343 ms baseline); six of ten paired runs were faster. Output size and FNV-1a
hash were identical at 793,139 bytes and `d748543a168ce6aa`. This is a small,
directional ARM64 result; raw records are in
`target/benchmarks/jpeg/goal-quantizer-20261002/`, and no x86 runtime speedup is
claimed.

Four exact public AVIF sequences now select non-wedge inter-intra mask modes
0, 1, 2, and 3. The mode-3 case has a pinned dav1d syntax trace for the
final-frame B16x16 at `(16,16)`, matches all three Pillow RGB frames and
100 ms durations, and is retained in the 60-case, 167-artifact AVIF loop
bundle. The current full LLVM report observes both outcomes of
`mode == 2` at `mc.rs:778` 768 times each, and Coverage MCP reports no missing
branch at that line. Six missing observations remain in
`fill_inter_intra_mask`'s invalid-mode and invalid-geometry guards; valid AV1
syntax cannot produce those states, so they remain uncovered without
private-only tests.

The new active `animated_lossy_wide_i444_mode2_unsplit_b128x128` case is a
two-frame 256×256 I444 sequence encoded with fixed 128×128 partitions. Its
mode-2 unsplit inter leaf is the public input needed to take the previously
unobserved `topology.is_none()` arm at `block.rs:63734`. The selected public
row passes exact Pillow RGB parity; frame hashes are
`e9a4e243aaa2832b61a0f8d362ee44e06d67acf653cf0e687c0d3e33099d8e` and
`cd57a87fb5d4d0d896d8dd50b80583ca80780eee160f5d59cd4699283937c4dc`, each
with a 100 ms duration. Its complete-file loop observation is infinite.
The selected LLVM report records one true and three false evaluations at
that branch. The full report now records two true and 60 false evaluations in
the public `decode_inter_leaf` specialization (the preceding full report had
zero true and 54 false); other monomorphizations still have uncovered
observations there. Coverage MCP marks the report source unverified and test
status unknown because no source/build receipt accompanies the local report.

The companion `animated_lossy_wide_i444_mode2_split32_b128x128` fixture uses a
flat gray key frame followed by a row-alternating 32-pixel sinusoid. Its
1,739-byte input hashes to
`f48a6d235c7ab245e8d57aedcc6135b897cea1bcbc579c3d630aa7c20e1a61aa`, and both
Rust-decoded frames match the pinned Pillow bytes. The selected public row
passes 1/1; its branch report reaches the true `all_roots_split &&
all_children_unsplit` `LossyWideMode2Split32` arm at `entropy.rs:7109` and the
true `all_roots_split && all_children_split` `LossyWideMode2Deep16` arm at
`entropy.rs:7115`. Coverage MCP's selected-test comparison reports six newly
observed branch coordinates in `entropy.rs`, including both arms, but marks
the comparison limited because source/build identity and test receipts are
unavailable. The fresh full AVIF decode-matrix report passes 466/466 and
records 9,528/32,200 branch
observations; this selected AVIF report does not represent full-project
coverage. Reports are
`target/release-evidence/coverage-avif-mode2-split32-row-20261004.json` and
`target/release-evidence/coverage-avif-goal-full-20261004-rev2.json`.

The mode-zero condition in `decode_inter_transform_size` is covered, but its
nested `clipped_lossless_grid` false arm at `entropy.rs:6927` remains missing:
the fresh full report records six true and zero false outcomes. The public
17×17 clipped-grid row now changes its bottom-right pixel in frame two and
passes exact Pillow parity. Its selected LLVM report exits through the earlier
complete-grid guard at line 6881 and records neither outcome at line 6927, so
it is retained as an edge-pixel parity regression case rather than coverage
evidence for that arm. The selected batch of ten lossless inter rows records
two true and zero false outcomes there. Full aggregate coverage remains
unchanged at lines 85,551/142,082, branches 16,248/32,170, functions
4,791/8,972 and regions 130,676/219,579. Coverage MCP reports the full-report
comparison as limited because source/build receipts are absent. The reports
are `target/release-evidence/coverage-avif-clipped17-edge-20261002.json`,
`target/release-evidence/coverage-avif-lossless-inter-batch-20261002.json` and
`target/release-evidence/coverage-full-bottom-b32-speed8-edge-20261002.json`.
Pinned AOM searches found inputs for some clipped B32x16/B64x128 geometries
that Rust rejects; no rejected or mismatching file was promoted to the public
parity corpus.

The public `interlace_adam7_gray_16bit` PNG row adds a 9×9 grayscale fixture
with 16-bit samples across all seven Adam7 passes. The selected PNG coverage
report has no remaining region observations at `decode.rs:1654–1655`. A full
comparison against the preserved pre-row report lists seven newly observed
coordinates in that span, with limited status because source/build identity is
unverified. Full-suite totals are unchanged, and the incremental comparison
could not combine the selected PNG and full-suite report inventories. No
aggregate coverage increase is claimed from these reports.

In this snapshot, `decode_inter_translation_impl` still has its largest grouped
branch observations at `block.rs:54939` and `block.rs:54973`, corresponding to
the small I444 chroma-split predicate and vertical I422 TX4 chroma-grid path.
Coverage MCP repeats observations across decoder monomorphizations, so group
counts are not unique source branches. The deterministic public AVIF input
`animated_lossy_inter_i444_split_b16x32_mode2` now passes exact two-frame Pillow
pixel parity. Its SHA-256 is
`ad7ce564a11440b91237e0dfffedbde1053bc663ae0e9bf0894f87e05e669bd0`. The
public decoder trace selects `SplitB16x32Deep` with transform mode 2 and the
LLVM report records the true arm of the small I444 predicate at `block.rs:54939`.
This input does not reach the later small-chroma terminal body; other decoder
monomorphizations remain uncovered. A Coverage MCP coordinate comparison
against the saved pre-rerun report observes 45 additional AV1 block branch
observations, including 27 in `decode_inter_translation_impl`; source/build
receipts are absent, so MCP does not verify attribution. A row-filtered run of
the existing public `animated_motion_chroma_422` case
records 3 true and 0 false evaluations at `block.rs:54973` in one decoder
specialization. Its active input SHA-256 is
`5a83530dcc60f75f0bec7d36cb4db67ae7258b2acc481cbb08cc0987247ea66a`; the
focused report SHA-256 is
`efe7104321bb07819efe9a9d17194236102e0122594e145aee3056adf76e89e7`. This
confirms the geometry is publicly reachable, while the remaining repeated MCP
observations belong to other monomorphizations. The lossy mode-zero plan
families formerly reported as narrow I422 mode-zero cases were removed
after source and AV1 syntax analysis showed that they required both a lossy
segment and frame transform mode 0. AV1 uses ONLY_4X4 only for coded-lossless
frames ([AV1 specification](https://aomediacodec.github.io/av1-spec/)), while
the parser sets mode 0 only when every segment is lossless. The legitimate
lossless plans and mode-zero parsing remain.

A temporary public parity profile of staged 16×16 and 32×16 I444 candidates
matched their Pillow frame pixels and durations, but neither reached the
plane-nonzero terminal fallback at `block.rs:58149`; the LLVM report records
zero executions for both arms across the eight emitted decoder
specializations. A staged 16×32 candidate failed frame-1 pixel parity and was
excluded. The candidates did not justify another matrix row; the 100% target
and the terminal-body gap remain open.

The thin-64 lossy qindex guard at `block.rs:55045` remains without a public
parity row. A 128-case staging sweep used the pinned Pillow 12.2.0, libavif
1.4.1, dav1d 1.5.3, and AOM 3.13.2 stack across exact 16×64 and 64×16
canvases, monochrome and 4:2:0, four content patterns, quality 75 and 50, and
partition settings from 8/16 minimum to 64/128 maximum. The sweep index is
`target/oracle-staging/thin64-lossy-search/quality-partition-sweep-index.json`;
none of its candidates selected a thin-64 inter block. The closest
mode-2/non-lossless candidates with positive qindex matched all Pillow frames
exactly but selected 16×16, 16×32, or 32×16 inter leaves. This search does not
prove the decoder path unreachable for every valid AV1 bitstream, so no
coverage-only or synthetic test was added and the gap remains open.

A separate 36-case qindex-zero sweep over 16×64 and 64×16 4:2:0/monochrome
sequences also produced no thin-64 inter leaf: every quality-100 segment was
lossless with `only_4x4`, while a quality-99 control used inter qindexes 2 and
4. The pinned three-frame qmatrix-at-qindex-zero input is
`target/oracle-staging/thin64-zero-qindex/candidates/16x64_420_bands_q100_min64_max64.avif`
(SHA-256 `da19be87bf0952334f6d613e5ad1b15381ecc04f41c3e952dacde25f9cd288ac`).
A two-frame derivative at
`target/oracle-staging/thin64-zero-qindex/two-frame-qmatrix/16x64_420_bands_2frame_q100_qm15.avif`
(SHA-256 `80b7e4469118c276fb0478e4f97ea0b319d1b3feb88c9f70d9eb22f3b5cc2f53`)
uses the same first two AV1 sample payloads and isolates the qmatrix inter
frame. Both kept `segment_lossless=true` and passed independent Pillow/dav1d
decoding; the three-frame input also passed Rust still decode. Its first shown
key frame reconstructed, but the second
shown all-lossless I420 inter frame was rejected by the initial reconstruction
admission gate because `complete_lossless_inter_color_reconstruction_context`
requires `!quantization.using_matrix`. The qmatrix-off control reconstructed
that frame, then its third frame was rejected because the same profile excludes
`skip_mode_references`. The AV1 specification assigns lossless segments
`SegQMLevel=15` and applies matrix scaling only below level 15
([AV1 specification, §§5.9 and 7.12.3](https://aomediacodec.github.io/av1-spec/av1-spec.pdf));
this looks like a future profile-expansion candidate. However, the repo's
capability contract declares AVIF `sequence_decode` not implemented, so the
probe does not justify a public parity row or changing that decoder gate under
the current scope. `deltaq-mode=1` is suppressed at qindex zero by the encoder
and its three-frame parity control still selected 16×16/16×32 inter leaves.
No bitstream surgery or coverage-only fixture was added.

Earlier coverage measurements reported the true arm of the mode-zero fallback
in `decode_inter_transform_size` at `entropy.rs:6888`. It can be reached only
when `lossless_grid_geometry` is false, because the preceding geometry path
returns a lossless plan or an unsupported result. The public 16×16 lossless
inter row and staging-only 17×17, 23×21, and 33×33 lossless edge probes took
the geometry path; the odd-size probes passed exact public parity in shadow
harnesses but did not cover the fallback. They do not prove that every valid
lossless syntax/geometry is covered, so this guard remains open. Three
additional monochrome lossless sequences—7×9, 65×67 translated noise, and a
65×67 edge-residual variant—also passed exact public pixel, timing, and loop
parity in staging. Their isolated reports still record 0/0 at line 6888; the
lossless-grid guard at line 6799 was true for every inter-transform call
(true/false counts 1/0, 72/0, and 16/0). These candidates therefore took the
complete lossless-grid plan before the fallback. Candidate files and reports
remain under ignored `target/oracle-staging/mode0-public-{7x9,65x67,edge-65x67}/`;
they do not prove every valid lossless geometry is covered, and no new row was
added. The compound-reference CDF lookup now follows the declared
`[group][context]` table order, including the backward table's two group rows;
its parity correction passes the complete matrix and coverage campaign.

The existing public row `animated_odd_dimensions_inter_intra_boundary_420`
also passes an isolated all-feature parity run on the ARM64 host. Its selected
LLVM report leaves both outcomes of `transform_mode == 0` at
`entropy.rs:6888` unobserved, while the current full report still lists the
true arm as missing. The row therefore does not reach this fallback, and no
duplicate fixture was added. The selected report
`target/release-evidence/av1-odd-interintra-mode0-arm64.json` has no
source/build receipt; Coverage MCP marks its source unverified and test status
unknown.

Two additional active AVIF parity cases close decoder divergences found while
examining public edge inputs. The 20x20 all-lossless I444 sequence exposed an
incorrect B8x8 inter-intra size group; it now matches dav1d's group 1 and
passes exact two-frame Pillow parity at clipped edges. The three-frame lossy
I444 sequence exposed single-reference motion-vector collection from the
matching lane of a compound neighbor, plus a mixed B16x32 split that was
incorrectly rejected whenever OBMC was present. The decoder now selects the
first matching neighbor lane without allocation, accepts the existing
OBMC-capable topology reconstruction path, and uses AV1's 8x8 paired-MI
metadata rule in the OBMC syntax precheck. The normative check examines the
right/lower metadata owner in each 4x4 pair ([AV1 specification, §7.10.3](https://aomediacodec.github.io/av1-spec/av1-spec.pdf)). Its three
frames match Pillow exactly. The loop bundle now contains 49 complete
observations: 41 accepted inputs and eight malformed inputs, with native
repetition and complete frame hashes retained.
Coverage MCP's remaining true arm in `has_overlappable_neighbor` at
`entropy.rs:5608` is defensive: its public caller invokes the helper only when
both block axes are at least two MIs. No private-only test was added for this
unreachable guard.

A four-frame 256×256 4:4:4 affine-motion AVIF row exercises `RotZoom` global
models on a compound-reference path. Its frame-3 mismatch traced to a second
application of `compound_mode_context`: `find_reference_mvs` had already
reduced the reference stack to the final 0–7 CDF context. The decoder now uses
that context directly. All four Rust frames match the pinned Pillow RGB
references byte for byte at 90, 110, 130, and 150 ms, and the native libavif
loop witness confirms infinite repetition and repeat equality. This fixture
also exercises compound spatial-neighbor and affine-motion state through the
public decode path.

An earlier local Coverage MCP query on `target/release-evidence/coverage.json`,
before the all-feature snapshot above, reported 17,239/32,240 branch
observations covered, with 15,001 missing across 1,513 groups. Its largest
`decode_inter_translation_impl` groups reported 1,870 repeated misses at
`block.rs:54830–54832`; those counts span decoder monomorphizations and are not
unique source branches. Coverage MCP marked that report's source unverified and
test identity unknown because it had no source/build receipt, so the counters
were coordinate diagnostics rather than attributed test coverage. An earlier
coordinate comparison reported 84 newly observed coordinates against
`target/release-evidence/coverage-after-compound-reference-cdf.json`, but that
baseline file is no longer present in the workspace and the reports lacked
receipts. The earlier local test run passed all 59 coverage-matrix tests.

The active `intrabc_spatial_neighbor_monochrome_256` fixture now witnesses the
IntraBC spatial-neighbor path. Its pinned Pillow 12.2.0/libavif 1.4.1/AOM 3.13.2
input is a deterministic 256×256 monochrome lossless AVIF with neighboring
8×8 IntraBC leaves (input SHA-256
`c77c448e3668e90a4de1228c8b44a54fd47caae8e7538e314a1597dcc961795f`). Public
`decode` and `decode_sequence` both match all 196,608 Pillow RGB bytes (SHA-256
`ed729898cd63496b4a1aa628686ebff2dd7a0f383dfdae854f7cd77dae54ce22`). The
fixture exposed that AV1 infers both subsampling flags as true for monochrome;
the bounded profile gate now checks those inferred values, and the fallback
motion-vector row test is tile-relative. A DAV1D trace confirms the second leaf
uses its IntraBC spatial neighbor. The full-report Coverage MCP query still
lists three missing observations in `add_spatial_candidate` at `motion.rs:1675`,
`1678`, and `1707`; this fixture does not claim to cover every outcome there.
That report has no source/build receipt and labels test status unknown, so the
remaining locations are diagnostic rather than verified per-row attribution.
No coverage-only test was added.

The targeted temporal-projection candidate is already exercised by the active
`animated.avif` parity row (SHA-256
`2f8683d21725261f37f86e115f0c212cc52d0fefd3a2ddfcc4fa648c1859906d`).
Independent OBU inspection confirms order hints and reference-frame MVs are
enabled, including an inter frame whose future ALT slot differs from GOLDEN.
The selected public-row LLVM profile invokes `load_projected_temporal_field`
five times and records both outcomes for the ALT/GOLDEN hint comparison (2/3)
and positive temporal-distance selector (4/11); the existing row already pins
five complete Pillow RGB displays and sequence metadata. The selected report
has no source/build receipt, so these are diagnostic counters rather than
Coverage MCP attribution. No duplicate parity row was added.

Boundary-focused monochrome probes also passed public pixel and duration
parity at 69×73 with 128×128 superblocks, 4×4, 5×8, 8×5, and 5×5. Their
isolated profiles took the complete lossless-grid path for every inter
transform call and recorded no execution at `entropy.rs:6842`. The candidates
remain under ignored `target/oracle-staging/mode0-edge-non4/`; no parity row
was added because none reached the fallback.

Two additional staging-only 64×64 monochrome lossless inter-sequences matched
Pillow's decoded RGB frames and frame durations; the second frame in each
sequence was inter-coded. Their isolated LLVM profiles recorded no executions
of the large-transform helper or either outcome at `block.rs:21105`. Pillow's
AVIF info does not expose the loop field, so the Rust `loop=0` result has no
independent Pillow assertion. These candidates do not justify a public parity
row, and no row was added.

A later bounded probe exercised four deterministic 64×64 monochrome lossless
images and four RGBA images with monochrome lossless alpha items. All eight
public decodes matched Pillow exactly, while isolated LLVM profiles still
recorded 0/0 at line 21105 and did hit a separate monochrome plane-geometry
branch. The target function's production caller is a fallback reached only
when the complete streamed monochrome validator declines; these probes
followed that complete validator instead. Encoder-option attempts did not find
a valid candidate that passes the fallback's narrower admission checks. The
route remains open for a bitstream where the first validator declines and the
fallback accepts; no coverage-only input or parity row was added.

The selected public rows `animated_lossless_inter_420_b16x16` and
`animated_lossy_split_inter_420_b16x16` passed their Pillow parity checks but
did not exercise the luma-extension condition at `block.rs:48287`. Coverage
MCP reports both branch outcomes missing in the selected report
`target/release-evidence/coverage-goal-vertical420-probe.json`; the full and
other selected AVIF profiles also show no observations for the helper's
extension/fallback arms. No new row was added from these cases.

Coverage MCP also identifies a WebP `Vp8Decoder<Cursor<&[u8]>>` specialization
gap at `vp8.rs:1566`. Production decode call sites pass bounded readers
(`Take`); the direct `Cursor` specialization is instantiated only by private
decoder tests. The selected public `lossy_vp8_lossy_macroblock_residual_context_edges_32x16_q75_m4`
parity row exercised the `Take` specialization six times and did not call the
direct specialization. Since a new public row follows the same production
reader path, it cannot cover this gap; no parity row was added for it.

The new public encode row `enc_lossy_noise` re-encodes the 64×64 high-entropy
`lossless_noise.webp` source as lossy WebP through both ordinary encoding and
`encode_with_token`. The 2,888-byte Pillow 12.2.0 output matches exactly
(SHA-256 `4989ffb95ab4b7bd4f1d5feb61d80d9fe256278a1e8d90070835db628dbb3912`).
Its pre-cleanup selected LLVM report, `coverage-webp-token-noise-selected.json`,
shows the `TokenPartitionCheckpoint::flush_with_checkpoint` `output.last_mut()`
`Some(previous)` arm 15 times in `encode_bool` and once in the final flush at
`bool_enc.rs:169`. A selected comparison against the q50 token-aware row reports
three newly observed branch outcomes at lines 168, 169, and 173. The quality,
alpha, palette, grayscale, CMYK, and predictor probes did not hit this carry
path. The earlier full profile's eight `None` outcomes came from the
coverage-only hook's empty output vectors, which have since been removed. This
public parity row supplies the complementary prior-byte carry case. Its old
branch coordinates and the 17,536/32,238 aggregate belong to a pre-cleanup
report; they are not current full-suite coverage. Coverage MCP reports no
source/build receipt and unknown test status, so the selected comparison is
limited coordinate evidence rather than a managed coverage delta.

### Public parity rows added — 2026-10-01

Three active decode rows add exact edge-case parity from Coverage MCP findings.
`unknown_ancillary_after_idat` inserts a valid `abCd` ancillary chunk after PNG
image data; Pillow 12.2.0 and Rust return identical 128×128 RGB pixels
(49,152 bytes, SHA-256
`8a1d6fcc36f5b5e70fbf949d4e612630a1931279383e7c21db27cd3cbad98131`).
`bmp_1bit_default_palette` sets ICO's embedded 1-bit DIB `colors_used` field to
zero and matches Pillow's 16×16 RGBA output (1,024 bytes, SHA-256
`1a9eee34c466acd78abb5ad2db2467d3fae0931df64b402f1eaf35a65463b842`).

`compression_packbits_packbits_zero_strip_byte_count` sets TIFF tag 279
(`StripByteCounts`) to zero. Pillow still returns the exact 128×128 RGB pixels
above. The public parity test exposed Rust rejecting this input as a truncated
PackBits stream; compressed strips with a zero byte count now derive their
bounded payload span from the next strip or file boundary, while positive
declared counts keep their existing path. The all-feature TIFF decode matrix
passes 129/129 rows.

The PNG, TIFF, and ICO focused decode matrices pass 163/163, 129/129, and 54/54
rows respectively. Coverage MCP reports no remaining branch gaps at
`png/decode.rs:347`, `tiff/decode.rs:1180`, or `ico/decode.rs:699` in their
selected reports. Those reports have no source/build receipts and test status
is unknown, so these are bounded path observations and do not update the
source-bound full-repository coverage totals.

### Selected parity coverage probes — 2026-10-03

The existing public GIF row `enc_rgba_mixed` now also invokes
`encode_with_token` and compares exact Pillow bytes and ordinary/token-aware
output. Its selected Coverage MCP report no longer lists the token-present arm
in `compact_rgba_palette`. That 4×2 input is too short to reach the remap
checkpoint at 1,024 pixels, so a separate public row
`enc_rgba_mixed_token_compact_checkpoint` uses a 33×33 PNG cycling the same
eight-value mixed-alpha pattern. The selected matrix run reports exactly one active
row passed, and Coverage MCP's selected comparison observes the true
checkpoint arm at `encode.rs:2027`. The two reports have no source/build
receipts and unknown test attribution; this is local coordinate evidence, not
a full-suite coverage claim. The remaining `has_holes` true arm has no public
witness under the current FASTOCTREE palette ordering: used indices form a
dense prefix, with unused padding at the end.

The public row `pillow_tolerated_malformed_missing_iend` verifies that PNG
decode succeeds while Pillow-style verification reports a truncated-file
error. Its selected run exposed a coverage-matrix lifecycle assertion that
always required `EncodedImage::verify()` to succeed for the first selected row
of a format. The parity harness now checks the source verification result
against that row's recorded Pillow status and error contract. The selected PNG
row passes 1/1. Its LLVM report records `Chunks::next`'s clean-EOF condition
true 7 times and false 12 times; the other true arm on the same source line,
`self.failed`, remains unobserved. Coverage MCP groups those two short-circuit
branches at one line and still lists a true-arm gap. Chunk errors propagate to
the current iterator consumers, so a later `next()` call after `self.failed`
has no known public parity input. The selected report has no source/build
receipt and unknown test attribution; these are local path observations, not a
full-suite coverage claim.

The selected BMP rows `error_not_bmp` and `error_related_os2_signatures` both
pass public error parity, but their selected reports do not enter
`bmp::decode()`'s signature branch at `decode.rs:363`: format detection rejects
those signatures before dispatch. Adding another row with the same dispatch
path would not cover that decoder guard. Their reports likewise have no
source/build receipts or test attribution.

The active WebP row
`pillow_tolerated_malformed_animated_alpha_anmf_width_mismatch` keeps the
nested VP8 and alpha streams at 64×64 while declaring the ANMF frame width as
16,385. Pillow 12.2.0 decodes two 64×64 frames; the previous ALPH path
trusted the declared width and returned `ImageTooLarge`. The decoder now reads
the bounded VP8 bitstream first and uses its validated dimensions for alpha
expansion and compositing, retaining the 16,384 limit on the decoded frame. The
selected public parity row passes 1/1. Its report
`target/release-evidence/coverage-webp-alpha-frame-too-large-20261003-after.json`
still lists the guard's `false` arm as unobserved at `decoder.rs:705`; Coverage
MCP has no source/build receipt and unknown test attribution, so this is not a
verified full-suite coverage gain. Incremental comparison with
`coverage-full-gif-lzw-invariants-20261003.json` is incomparable because its
baseline branch detail is incomplete; no marginal-coverage union is claimed.

The active JPEG row `baseline_420_singleton_chroma_width_fallback` uses a 1×1
baseline JPEG whose Cb and Cr sampling factors differ. Pinned Pillow 12.2
decodes the pixel as RGB `(128, 128, 128)`, and the selected public parity row
passes 1/1 without a decoder change. Its LLVM report
`target/release-evidence/coverage-jpeg-singleton-h2v2-20261003.json` shows the
previously missing false arms at `upsample.rs:85` and `:105` hit; the opposite
true arms remain missing. Coverage MCP reports no source/build receipt and
unknown test attribution, so this records the selected input's paths rather
than a suite-wide gain.

The active WebP row
`pillow_tolerated_malformed_animated_vp8l_anmf_width_mismatch` declares an
ANMF width of 16,385 around a 16×16 VP8L stream. Pillow 12.2 decodes two
64×64 RGBA frames. The decoder now reads the bounded VP8L header and uses its
intrinsic dimensions before allocating and compositing the frame. The selected
row passes 1/1. The active rows `error_animated_vp8l_short_header` and
`error_animated_vp8l_short_header_oversized_width` cover four-byte nested
VP8L payloads with normal and oversized ANMF declarations. Pillow 12.2 rejects
both with `OSError: could not create decoder object`; both selected public
parity rows pass 1/1, including the oversized-frame guard's `ImageTooLarge`
path. The WebP decode matrix passes 237/237. Rows
`error_animated_vp8l_frame_outside_right` and
`error_animated_vp8l_frame_outside_bottom` shift a valid 64×64 nested frame two
pixels beyond each canvas edge. Pillow 12.2 rejects both; the selected public
parity rows pass 2/2 and exercise both short-circuit arms of the frame-bounds
guard. The complete decoder matrix ran eight public format tests and passed
1,419/1,419 active decode rows. Coverage MCP report
`target/release-evidence/coverage-public-decode-matrix-20261003-verified.json`
records 11,766/32,174 crate-wide branch counters for that decode-only run.
The report has no source/build receipt and unknown test attribution, so use it
as a path inventory rather than a full-suite coverage measure. Coverage MCP no
longer lists gaps in the new WebP VP8L header-size or canvas-bounds paths, or
in the JPEG singleton-width upsampler. Its remaining WebP `read_frame` gap at
`decoder.rs:734` is the VP8 dimension guard; valid VP8 widths and heights are
14-bit values and cannot exceed its 16,384 limit.

The latest public parity batch adds three malformed AVIF meta cases: duplicate
`iprp` (`9a245809b8f3a78aec952c15ee9703973a5d407bbaffe17580c4b6b3aca26ee8`),
duplicate `iref`
(`ec6f361b37447d4abeb047508863c59e60d3aa73721adb4b5a76e7e159295f3e`), and
empty `idat` (`281c5cf87f6d34f77635924a13384d107111870db7600e10636e4a2735d2c1c7`).
It also adds JPEG `progressive_com_tail_junk`
(`70dd4ed893d23e30d3e5c27ef1ac3f55e6a1a98e3aad166610db48754ee43ed2`), which
opens and verifies in Pillow but fails during load as a truncated progressive
stream. All four selected public parity rows pass. The full `make coverage` run
passes every executed test and the local floors. Coverage MCP reports seven new
branch coordinates compared with the saved pre-batch report: five under
`samples::parse_meta` and two in JPEG `find_next_marker`. The comparison remains
limited because source/build identity and test attribution are unverified; these
are coordinate changes, not source-bound coverage claims. The 100% goal remains
open.

## Clipped AV1 lossless inter-grid parity — 2026-10-04

The public AVIF matrix now includes clipped B32 lossless inter-grid witnesses
for I444 right and bottom edges, I422 right-edge cases, and I422/I444 dual-edge cases. The new
`animated_lossless_inter_i444_clipped_b32x32_64x56.avif` has SHA-256
`9004ec840b2a413c0e15294f52c723ebe5c3689cbe3decbd4afd5575cad78e23`; its
bottom B32 block exposes 32x24 pixels, and its two decoded RGB frames match
Pillow exactly. The selected public row and the complete AVIF native-loop
parity test pass. The 52x64 I422 case also passes exact sequence parity with a
20x32 clipped right block. The two 52x60 dual-edge cases pass exact sequence
parity with 20x28 luma visibility; I422 chroma visibility is 10x28. For odd-width I422 at 49x64, key-frame pixels match
Pillow and sequence decoding keeps an explicit `Unsupported` expectation
because the final chroma sample geometry is not parity-safe. The AVIF matrix
now has 454 active decode rows; the full fixture matrix has 1,950 rows.

`make coverage` passed all executed tests and the local floors. Its LLVM report
records 86,804/142,471 lines (60.9275%), 16,766/32,250 branches (51.9876%),
4,849/8,990 functions (53.9377%), and 132,767/220,396 regions (60.2402%).
Coverage MCP identified six missing branch outcomes in
`inter_lossless_grid_clipped_b32_geometry_supported` near
`src/codecs/avif/av1/entropy.rs:5395-5397`. The report has no source/build
receipt and MCP reports `source=unverified`, `tests=unknown`, and no build or
revision ID, so these coordinates do not establish named-test attribution or
a verified coverage delta. Those false arms are unreachable for valid B32 leaves: tree traversal skips
zero-area blocks, and a visible extent over 32 contradicts the coded block
size. Malformed inputs may exercise upstream rejection, but defensive-only
states do not become Pillow parity fixtures. Strict SSE2-baseline and AVX2 Clippy builds pass,
but they are compile checks, not runtime speed measurements. The 100% goal
remains open.

The refreshed MCP branch report's largest group is the `lossless_partial_grid`
predicate at `block.rs:54838-54847`. Source review found seven zero-hit
callback-specialization clones; the remaining live clone already observes both
outcomes of the clipped-versus-full extent predicate. Existing active parity
rows cover admitted clipped B32 grids across I420, I422, and I444. The rejected
zero-hit clones and the depth/alignment rejection arms do not provide a new
valid Pillow-parity input, so no redundant AVIF row was added. A bounded probe
for the 64-axis lossy transform mapping at `block.rs:57818` and its topology
variant at `block.rs:57852` also produced no bitstream that both selected the
missing plan and passed public parity. MCP still has no source/build receipts;
these are coordinates, not verified test attribution.

## PNG post-IDAT ancillary CRC parity — 2026-10-04

The public PNG matrix adds `pillow_tolerated_post_idat_ancillary_bad_crc`, an
unknown ancillary chunk after IDAT with one corrupted CRC byte. Pillow 12.2.0
still decodes the original RGB pixels (SHA-256
`8a1d6fcc36f5b5e70fbf949d4e612630a1931279383e7c21db27cd3cbad98131`), while
`verify()` reports `SyntaxError: broken PNG file (bad header checksum in
b'abCd')`. The selected public decode-matrix test passes. Coverage MCP could
not union its selected report with the saved full baseline: branch totals differ
(32,250 versus 32,324), and both reports lack source and build receipts, so no
verified coverage gain is claimed.

## GIF89a metadata-limit parity — 2026-10-04

The active `palette_index_leniency` row also decodes its 35-byte GIF89a input
with a public metadata limit of exactly 33 bytes; its two compressed image-data
bytes are excluded from the metadata extent. The targeted matrix row and full
coverage campaign pass, with exact Pillow pixel parity. Coverage MCP now leaves
only the malformed-signature outcome at `src/codecs/gif/decode.rs:361`; public
format detection rejects that signature before metadata preflight, so it does
not justify another parity fixture. The report remains unbound to source/build
receipts and has unknown test attribution.

## JPEG 4:2:0 generic small-image encoder parity — 2026-10-17

The active `enc_sub_420_generic_mcu_aligned_16x16` row encodes a deterministic
16×16 RGB PNG at quality 85 with baseline 4:2:0 subsampling and Huffman
optimization disabled. Its PNG input is 852 bytes with SHA-256
`427e0a7d6532ab94abf4ecf3d96d891719c060780eaa9243d161f278d97e9eef`; the
Pillow JPEG reference is 818 bytes with SHA-256
`f8f11806f51915cf5673c8f5732280b24fe53ee7b3c5ef6669ac17ace9e97031`, and the
roundtrip RGB reference hash is
`6fb0c4384dd488fb72663f0799d7591a1fe701165b1ac614a289974139f9696a`.
The source forms a complete 2×2 luma-block grid below the 1024-pixel
streaming threshold. The selected public encode row passes. The focused report
`target/release-evidence/coverage-jpeg-420-generic-16x16-final.json` records
the selected row's aligned-grid outcomes at
`src/codecs/jpeg/encode/mod.rs:4611–4614` (three taken observations per
guard). Coverage MCP calls hung in this run, so the selected report is not
unioned with a baseline and establishes no aggregate coverage change.

## Small JPEG chroma and WebP palette encode parity — 2026-10-04

Two active public encode rows add exact Pillow references using existing source
assets: `enc_subsample_422_tiny_edge` encodes the 1x8 RGB JPEG as baseline 4:2:2
at quality 85, and `enc_lossless_palette16` losslessly re-encodes the 64x64 RGB
WebP fixture with exactly 16 colors. They cover small-image chroma edge
replication and the WebP 5–16-color packed-palette case. The selected public
matrix passes 2/2; the full all-feature coverage campaign also passes all 56
matrix tests, 7 decode-policy tests, and 68 feature-gate tests.

Coverage MCP still reports the defensive invalid-geometry arm at
`src/codecs/jpeg/encode/mod.rs:1507`; validated public dimensions cannot reach
that arm. The earlier full report for this review recorded 86,804/142,471 lines,
16,766/32,250 branches, 4,849/8,990 functions, and 132,767/220,396 regions; it
predates the latest full report at the top of this file. Neither report carries
source/build receipts for MCP attribution. The 100% coverage goal remains open.

## Historical claim ledger

The following source-bound baseline was measured on 2026-08-27. Its historical
percentages must not be relabeled as current-release coverage. The validator
checks the historical manifest/matrix bytes at the measured Git revision,
Coverage MCP identities, and separately checks current fixture integrity. The public rendering is centralized here
so copied prose cannot drift across guides.

After regenerating the manifest, coverage matrix, coverage origin manifest,
unreachable contract manifest, or roadmap, run `make refresh-fixture-integrity`
locally. It updates current receipt hashes and the current-integrity line below,
then runs `make verify-fixture-integrity`. It preserves the measured revision,
original manifest and matrix hashes, and coverage MCP identity. GitHub CI runs
the read-only verifier and never refreshes receipts automatically.

<!-- current-claim-ledger:begin -->
Current claim-ledger baseline (not current `HEAD`):
- Measured revision: `93ec80ec99c42671dce6cf70694bce27ad8a2ef4`.
- Coverage MCP run: `ec4c4bbd-dbda-4e49-8109-d7da07722dc0`; snapshot: `7665cda3-f4a7-4568-b871-a9d34afaa92c`.
- Coverage: 100,389/110,015 lines (91.2503%), 12,861/14,246 branches (90.2780%), 5,125/5,794 functions (88.4536%), and 150,221/166,375 regions (90.2906%).
- Measured manifest SHA-256: `c1a1cccd485d066ffbe206a6e1577a1788aff8d4f288e4e8f8a933fa3c62ae7b`; measured matrix SHA-256: `f26151b3811aaab58556da422f476b714b5fac5925ff5b97807904096b4d2d58`.
- Current fixture integrity only: manifest `94ff9e2db79279ca40a92b8afedff3fd7d8c07983ec09ce13a47710564e27e52`; matrix `c4d559500ad94c2739e8c5dae830c7857a8cb8e9a424f8f66853b97587c97389`.
<!-- current-claim-ledger:end -->

The larger current source denominator and the historical source denominator
are not interchangeable. Selected incremental runs cannot substitute for a full
fresh report. Keep the original raw reports and context rather than attaching
old identities to new measurements.

The ledger's fixture-manifest hashes track current-file integrity. Updating
those hashes does not extend the historical coverage run to new code, guards,
or unexecuted regressions. The 2026-09-17 AVIF temporal-candidate repair remains
unverified by Rust execution, as recorded in the roadmap.

The 2026-09-17 coverage-fixture maintenance passes strict all-feature Clippy
with `--cfg coverage` on native and `wasm32-unknown-unknown` targets. Native
coverage builds also pass with no codecs, default codecs, and each codec
individually. Ordinary native Clippy, both WASM library feature matrices, and
all-feature rustdoc pass. Fixture setup retains explicit failure assertions,
checked bounds, and the original malformed inputs; feature-specific exercises
follow the same feature gates as their implementation. The Rust behavioral
test suite and coverage measurements remain deferred. The documentation
checker inadvertently executed the README quickstart during this maintenance;
that example passed, but is not evidence of roadmap or parity completion. The
historical coverage totals and open capability statuses remain unchanged.

The 2026-09-17 HDR color candidate has independent full-file native evidence:
dav1d and libavif agree on 240,000 YUV bytes, while pinned scalar libyuv,
libavif and Pillow agree on 120,000 RGB bytes. The deterministic observation
is recorded in the [HDR oracle index](../tests/fixtures/outputs/avif_hdr_color/hdr/index.json).
This witnesses the native conversion of 10-bit full-range I444 CICP 9/16/9
without alpha. The safe-Rust converter and its full-plane/tail regressions are
implemented; their behavioral execution and managed coverage are deferred.
Strict Clippy passes for native, `wasm32-unknown-unknown` and `wasm32-wasip1`
with AVIF-only and all features, both ordinary and coverage configurations.
Native and coverage-enabled unknown-WASM checks include all targets; WASI
checks compile the library. Warnings-as-errors rustdoc, formatting and static
provenance/roadmap checks also pass.
The HDR public row remains planned and historical coverage is unchanged.

The 2026-09-17 high-depth color candidate adds the exact 12-bit limited-range
I422 CICP 2/2/2 declaration with auxiliary alpha from `10bit.avif`. Independent
scalar dav1d and libavif agree on 81,920 color-plane bytes and 40,960 alpha-plane
bytes across all five frames. Pinned scalar libyuv, libavif and Pillow agree on
81,920 RGBA bytes, with repeatable native planes, pixels and timing. Source,
compiler, default I601 build flags and artifact hashes are retained in the
[native index](../tests/fixtures/outputs/avif_sequence_color/high_bitdepth/index.json).
Partial-alpha pixels witness straight output; zero-alpha pixels are absent.

The shared declaration selector and Rust color conversion are implemented,
with deferred regressions for every native frame, 25 full-row slices and
separate internal admission/sample-buffer checks. Strict Clippy passes all
12 native/WASM, AVIF-only/all-feature, ordinary/coverage lanes used for the
HDR candidate, and warnings-as-errors rustdoc, formatting and static checks
pass. This is compile-only Rust verification. Public reconstruction, sequence
presentation, Rust pixel execution and managed coverage remain unverified;
the existing sequence gates, planned matrix rows and historical coverage
totals are retained.

The 2026-09-17 sequence presentation candidate processes matching color and
alpha samples with persistent reference state, converts each complete display
before advancing, and publishes frames only after all samples succeed. It
reserves later output transfer bytes before reconstruction and requires the
inspected frame count, output mode and actual frame geometry to agree. Missing
reconstruction surfaces remain capability gaps; malformed tile envelopes and
empty reference slots retain precedence. That revision retained the frame-ID
sequence presentation gate; the later candidate below removes it.

The [sequence native index](../tests/fixtures/outputs/av1_sequence/animated/index.json)
retains five complete RGB displays, six decoded frames, two hidden frames and
one show-existing display from the unchanged `animated.avif`. Unmodified and
instrumented pinned scalar dav1d agree on 168,750 YUV bytes; repeated native
libavif/Pillow observations agree on 337,500 RGB bytes. Native durations are
exactly 1/30 second; Pillow reports rounded 33 ms durations.
Independent libavif repetition observations distinguish this
one-play animation from the infinitely repeating high-depth fixture. Pillow
omits its loop field for both. The [bundle notes](../tests/fixtures/outputs/av1_sequence/README.md)
retain the native metadata origin and required matrix reconciliation.

Deferred regressions compare every public display, first-image pixels,
timing, native repetition and output-budget boundaries for both fixtures.
Internal defensive cases remain Rust model assertions. All 12 strict Clippy
lanes described above, warnings-as-errors rustdoc, formatting and static
provenance/roadmap checks pass. Rust behavioral execution and managed coverage
remain deferred; no row or finding is promoted. Output transfer limits do not
bound retained AV1 references or scratch memory, and variable frame geometry
and primary-item/track declaration disagreements remain unsupported.

The later 2026-09-17 frame-ID candidate corrects interleaved reference index and
delta parsing in Rust and the syntax inspector. Seven pinned native reads
establish the ordering and values independently of either implementation.
The [frame-ID oracle](../tests/fixtures/outputs/av1_sequence/error_resilient/index.json)
retains both native RGB displays, native YUV noninterference, exact timing and
repetition, a full-file 32767-to-0 wraparound success and a full-file reference
delta rejection. The latter preserves the first image in both native/Pillow
observations and Rust's deferred regression. Native CLI diagnostics and actual
frame outputs establish rejection even though that CLI returns exit code 0.
Strict Clippy passes in all 12 native/WASM, AVIF-only/all-feature,
ordinary/coverage lanes. Warnings-as-errors rustdoc, formatting, artifact
hashes and static provenance/roadmap checks also pass. These are compile-only
and static Rust checks; the native observer runs execute only pinned oracles.

The frame-ID-only presentation gate is removed, while repeated-current-ID
validation retains precedence over reference-delta failures. Behavioral Rust
execution and managed coverage remain deferred. Short-signaling fallback,
stale-reference behavior, broader reconstruction and native-versus-Pillow loop
reference reconciliation remain unfinished. The bundle's README separates
these limitations from the bounded evidence; no matrix row or finding closes.

The 2026-09-17 short-reference candidate removes the no-future rejection and
corrects the Python inspector's maximum-distance tie handling. The shared
production helper preserves native fallback order, duplicate anchors and
repeated earliest-slot reuse. Its normal parser wrapper still requires all
eight reference headers.

The [short-reference oracle](../tests/fixtures/outputs/av1_short_references/index.json)
contains six complete AVIF mutations with independently repeated native traces,
4,608 YUV bytes and 9,216 RGB bytes. Unmodified and instrumented dav1d agree
byte for byte. Pillow/libavif observations repeat with exact native timing,
and the corrected inspector agrees on all selected indices. Equal reference
pixels make index comparisons necessary; the deferred coverage test invokes
the same production selection helper. Invalid adapter arguments are separately
identified as internal model assertions. No coverage exclusions were added.

All 12 strict native/WASM, AVIF-only/all-feature, ordinary/coverage compile
lanes pass. Rust behavior and managed coverage remain deferred. This corpus
does not establish every mixed-distance history or complete sequence/resource
parity; planned rows, finding counts and historical coverage remain unchanged.

The 2026-09-17 repetition candidate aligns the container parser with native
edit-list handling and replaces the matrix's absent-Pillow-loop inference with
explicit native provenance. The [loop index](../tests/fixtures/outputs/avif_loops/index.json)
retains 28 complete inputs, 41 hashed artifacts and 420,956 RGB/RGBA bytes.
Twenty accepted observations preserve all frames; eight malformed files fail
native parsing and Pillow opening. All observations repeat identically.

The native signed-count boundary preserves 2,147,483,648 total plays as finite,
then normalizes larger values to infinite. Absent edit lists stay unspecified;
nonrepeating lists stop after flags. Repeating lists validate count, version
and nonzero segment duration but ignore media fields. Color repetition wins
over a different alpha value, while malformed alpha metadata still rejects.

The generator and Rust matrix harness bind loop evidence to the complete
input, case, native source, index hash and normalized value. The Python static
validator rejects a valid witness substituted from a different file. Existing
row statuses and deferred Rust capability contracts remain unchanged pending
the final behavioral campaign. All 16 strict native/WASM compile configurations,
warnings-as-errors rustdoc and static provenance checks pass. The guard inventory
remains 540 across 89 files. No current-source coverage claim is made.

The later 2026-09-17 display-retention candidate replaces the per-sample vector
of shown completions with one selected completion. Online maximum selection
preserves temporal-unit/spatial/temporal precedence and latest-tie behavior.
Losing candidates still commit their reference/CDF/current-ID state. A hidden
sample cannot reuse an earlier display as its current presentation. Failed
flushes preserve the last completed-frame commit, not a whole-sample rollback.

Two deferred internal ownership regressions use `Weak` references to check
immediate release of losing/superseded surfaces and survival of independently
referenced surfaces. They also cover selection axes, exact ties, a winning
missing-surface gap, hidden-unit filtering, pending frames and counter overflow.
Existing complete native animation/high-depth/frame-ID witnesses retain pixel
authority. No input acceptance boundary or oracle output is changed. Rust
behavior and managed coverage remain deferred; aggregate memory limits and
peak-allocation measurements remain unfinished.
All 12 strict native/WASM, AVIF-only/all-feature, ordinary/coverage compile
configurations pass, along with rustdoc, formatting and static provenance gates.
The coverage-origin inventory records 147 exact guards across 24 Rust files.

### TIFF FillOrder rational field parity — 2026-10-13

The TIFF decode matrix now includes complete-file Pillow cases for SHORT,
LONG, BYTE and RATIONAL tag 266 values. The RATIONAL cases cover valid 1/1 and
2/1 values, first-value behavior with a later 0/0 value, zero-count defaulting,
zero denominators, fractional values, and unsupported 3/1. BYTE(2) and invalid
rational values match Pillow's open-time rejection. These are public input
parity cases; no target-only fault contract was added, and the 27 typed fault
contracts remain separately reported.

The focused TIFF run selected 14 matrix rows, including the default
FillOrder-absent miniswhite row, all seven new FillOrder rows, and the existing
strip and tiled cases. It passed (`1 passed; 0 failed; 56 filtered out`).
Formatting and strict all-target/all-feature Clippy also pass. The focused
Coverage MCP report is
`.coverage-mcp/local-evidence/20261013/tiff-fillorder2/focused.json`;
`coverage_gaps` reports no missing branch outcomes in
`validate_fill_order` and summarizes the selected report as 201/31,308 branch
observations. Coverage MCP marks source identity unverified and test attribution
unknown. Its incremental comparison with
`.coverage-mcp/local-evidence/20261013/main-full-baseline/coverage.json` is
incomparable because the reports' branch details do not match their totals, so
this focused run does not establish a suite-wide gain or regression. The last
full baseline remains 89,277/135,839 lines, 17,418/31,268 branches,
4,992/8,690 functions and 136,575/210,976 regions; no full coverage campaign
was run for this change.

### AV1 film-grain output allocation fault contract — 2026-10-13

The target-only row
`fault-contract:avif:avif_film_grain_output_reservation_failure` uses the
active Pillow parity input `decode:avif:portable_lossless_filmgrain_i444_64x64`
as deterministic stimulus. A coverage-only reservation-size override reaches
the real `Vec::try_reserve_exact` error path; the public result is
`ImageError::Dimensions` for AVIF at `StillDecode`, and a clean retry must
match a fresh uninjected decode. The row is recorded with
`oracle_status: not_applicable`, separate from Pillow parity. In normal builds
the fault hook is absent and the reserve path has no additional branch; the
behavior change classifies allocation failure as a dimensions/resource error
instead of malformed input.

The focused target-only run selected and passed this one contract. Coverage MCP
reports no remaining region at `apply_plane` line 1059; the historical full
report showed two unobserved regions in the prior allocation-error mapping at
line 1048. Its incremental comparison is `incomparable` because the historical
baseline detail differs from its totals. Both reports are source-unverified
with test attribution unknown, so this is focused coverage evidence rather
than a quantified suite-wide gain. No full coverage campaign was run.

The adjacent target-only row
`fault-contract:avif:avif_film_grain_points_reservation_failure` injects a
failure into AV1 film-grain point-array reservation while parsing the same
active Pillow parity fixture. It expects the decoder's existing structured
`ImageError::Malformed` result at `StillDecode`, then verifies that an
uninjected retry matches a fresh decode. The fault row is marked
`oracle_status: not_applicable`; it is not Pillow parity. Coverage MCP's saved
full report marks the error arm of the increasing-point validation in
`read_points` as unobserved. The reservation fault returns before that check,
and the focused selected report still shows that arm as unobserved, so this row
does not claim to close that branch gap. The incremental comparison with the
full report is incomparable because baseline branch details differ from its
totals. Both reports lack source receipts and have unknown test attribution;
the focused run verifies this fault contract only. No full coverage campaign
was run for this row.

### AVIF `stts` timing fallbacks — 2026-10-13

Six active parity rows mutate the pinned `animated.avif` source: the original
zero-`sample_delta` case plus empty, zero-count, under-filled, over-filled, and
wrapped-count timing tables. The deterministic writer pins each output hash
and preserves all media payload bytes. The four incomplete-table mutations
keep the complete file length; the empty table is a size-16 `stts` box followed
by an 8-byte `free` sibling. The wrapped-count case inserts an additional run,
updates enclosing sizes and `stco`/`iloc` offsets, and grows the input from
1,235 to 1,243 bytes. Pillow 12.2.0 returns five 150×150 RGB frames with
unchanged pixels for every mutation. The zero-delta case has five zero-ms
durations, the four incomplete-table cases have five 33-ms durations, and the
wrapped-count case has durations 33, 67, 67, 67, and 67 ms. Pinned libavif 1.4.1
parses each file and decodes all five frames.

The Rust sequence decoder now mirrors libavif's `uint32_t` timing-run
accumulation, final-run fallback, and one-tick empty-table default while
preserving zero deltas. The wrapped-count row reaches the defined `u32` sum
wrap on all Rust targets. The decoder still checks conversion of Rust sample
indices and downstream timing arithmetic. These are public Pillow parity
cases, not target-only fault contracts.

The five new incomplete- and wrapped-table rows passed one focused public
decode-matrix run (5/5 selected rows; one integration test, 56 filtered).
Its Coverage MCP report is
`.coverage-mcp/local-evidence/20261013/avif-stts-incomplete/current-selected.json`,
SHA-256 `9b27edbd93dacf007c9d6cb7492d56078c558b9bf40382f02feeca38cb9bc50c`.
The selected report records 3,297/31,302 branches and 20,014/135,914 lines;
Coverage MCP found no missing branch observations at `samples.rs:2503` for
this selection. MCP marks the report source unverified and test attribution
unknown. The earlier split-run reports remain in the same local directory;
their comparison with the saved full baseline is `incomparable` because
baseline branch detail differs from its reported totals. No suite-wide
coverage gain or regression is claimed. No full coverage campaign was run.
Exact commands, local test output, report digests, and oracle identities are
in `.coverage-mcp/local-evidence/20261013/avif-stts-incomplete/receipt.md`.

### AVIF metadata reservation faults and iinf entry-count parity — 2026-10-13

Two target-only contracts now exercise AVIF metadata-table reservations for
`iinf` items and `iloc` locations. The coverage-only fault hooks force the
real `Vec::try_reserve` capacity-error path; normal builds contain no
injection branch. Each selected fault row returns a structured AVIF
`ImageError::Dimensions` at `StillDecode`, and a clean retry matches a fresh
decode. These rows use `oracle_status: not_applicable` and stay outside
Pillow parity.

The public parity row `error_iinf_entry_count_exceeds_children` changes only
the v0 `iinf.entry_count` in `baseline.avif` from one to two while retaining
the single valid `infe` child. Its fixture SHA-256 is
`651cb3bd4e3d0f6202d1b9caddd11470f518d05c978e5d38cc5d66423a60dde8`.
Pillow 12.2.0 detects the AVIF signature, then reports
`PIL.UnidentifiedImageError: cannot identify image file <bytes>` for
inspection, verification, decode, and sequence decode. It reaches the normal
missing-child parse error and is not a target-only fault case.

The complementary public row `error_iinf_entry_count_zero_with_child` changes
that count from one to zero while retaining the same child. Its full-file
fixture SHA-256 is
`951efe4031639fc3cda52ba994de66b93b028892cae96e4e0efb2f83d79677ef`. The
pinned Pillow 12.2.0 reference generator classifies inspection, verification,
decode, and sequence decode as malformed
`builtins.RuntimeError: Failed to decode image: Missing or empty image item`.
The single-row coverage run passed 1/1. Coverage MCP's selected report
`.coverage-mcp/local-evidence/20261013/avif-iinf-zero-count/coverage.json`
(SHA-256
`45c831e568a85c30b2895f0ad7343d523bafe0b7138f7ac78b73f59f876c0bf8`)
lists the `false` arm as missing at `src/codecs/avif/samples.rs:599`; the
trailing-child `true` arm is observed in this selection. The test command ran
from source revision `43a94a55ea39cb53115749a60ebef757ea6e3b48` with the
generated manifest and references. The report has no build ID or recorded
revision, and MCP labels source identity unverified and test attribution
unknown. Incremental comparisons with the selected iinf report and saved full
baseline are incomparable because their exact branch detail is incomplete;
no aggregate coverage gain is claimed.

For the count-exceeds selection, the focused public run passed 1/1 row and
Coverage MCP returned no missing branch location at
`src/codecs/avif/samples.rs:589`; that path is distinct from the trailing-child
branch measured by the zero-count selection above. The selected fault run
passed the baseline plus both contracts (3/3 rows), with no missing branch observations
at the `iinf` reservation hook (`samples.rs:576`) or the exercised
`parse_iloc::<true>` hook (`samples.rs:1095`). Both reports are selected-only;
MCP labels source identity unverified and test attribution unknown. The full
baseline comparison is incomparable because its exact branch detail is
incomplete, so no aggregate gain or regression is claimed. No full coverage
campaign ran. Reports and receipts are in
`.coverage-mcp/local-evidence/20261013/avif-iinf-parity/` and
`.coverage-mcp/local-evidence/20261013/avif-metadata-fault-contracts-selected/`.

`make fmt` and `make verify` pass; strict all-feature Clippy passed with
`RUSTC_WRAPPER=` after the default `sccache` wrapper was denied by the local
sandbox. The broad asset command
`.oracle-venv/bin/python scripts/generate_test_assets.py --format avif` still
stops at its existing pinned ICC fixture-hash check before reaching the new
generator block. The new fixture bytes were produced from the pinned baseline
with the single-field mutation and verified against the recorded digest.

## Diagnostic provenance

The separate defensive-model contract has 61 diagnostic cases: 38 use committed bytes that also have a Pillow parity row;
23 cases construct runtime mutations. Diagnostic identities and messages remain
Rust model observations, not Pillow parity fields. The provenance verifier checks
the original input hashes and supporting baseline route for every case.

## Contract catalog: behavior Pillow cannot prove

This is the separate Rust-only list. “Cannot prove” means Pillow may have a
similar idea internally, but it cannot return this crate's exact field, token,
target, sink, or typed result for comparison.

The bounded v1 map is machine-checked by
`make verify` against
`tests/fixtures/unreachable_contract_manifest.json`. `covered` means that the
manifest names an existing fixture-backed integration contract or fixture
verifier; `planned` means that no such contract is claimed yet. The map is an
evidence index, not a claim that every legal format state is implemented.

The verifier also parses this ten-row table. It requires each row's status and
Pillow-parity column to match the manifest, every covered row to name the exact
manifest evidence paths, the release-package row to name its exact fixture
verifier, and the planned allocation/stack/coverage row to say that no
category-specific evidence is claimed while naming its bounded context paths.
This is documentation-integrity evidence only: it does not promote the planned
category to covered and does not add any Rust-only result to Pillow parity.

The separate `fault_contract_cases` index maps target-only injected failures
to their structured-diagnostics requirement and shared matrix selector. The
AVIF, BMP, JPEG, and TIFF fault cases reuse active Pillow parity inputs as
reproducible stimulus, while each injected result is recorded with
`verification: fault-contract` and `oracle_status: not_applicable`. The new AVIF
monochrome alpha case shortens one sample only under `cfg(coverage)`, expects
`NotImplemented` at `StillDecode`, and compares a clean retry with a fresh
decode. It is target-only diagnostic evidence, not Pillow parity. The runner
reports this lane separately from Pillow decode/encode counts. The TIFF ignored-sample contracts exercise zero row bytes and a short assembled buffer. The TIFF YCbCr contracts also exercise zero horizontal and vertical subsampling after valid-layout selection, then verify a clean Pillow-parity retry. The YCbCr
cases force clump-length failures in the 1-by-2, 2-by-2, and general
converters, full-resolution sample-length failures, and RGB-output reservation
failures in the 1-by-2, 2-by-2, and general converters. They assert the
expected typed decode error followed by a successful retry against the same
Pillow-backed input.

| Map ID | Rust-only contract | Why Pillow cannot prove it | v1 status | Separate evidence | Pillow parity |
| --- | --- | --- | --- | --- | --- |
| `decode-encode-policy-limits` | `DecodePolicy` and `EncodePolicy` limits | Pillow does not expose this crate's pre-detection, canvas, metadata, decoded-byte, encoded-output, or work-budget result with the same boundary/error fields | `covered` | Manifest evidence: `tests/decode_policy_tests.rs`, `tests/feature_gate_tests.rs` | `excluded` |
| `cancellation-work-budgets` | Cancellation and work budgets | Pillow has no caller-owned `CancellationToken`, checkpoint budget, or `EncodeWorkUnits` result | `covered` | Manifest evidence: `tests/feature_gate_tests.rs` | `excluded` |
| `output-sink-delivery` | `OutputSink` delivery | Pillow does not accept this crate's dependency-free sink, expose delivered prefixes, flush failures, or rollback semantics | `covered` | Manifest evidence: `tests/feature_gate_tests.rs` | `excluded` |
| `caller-owned-destination-buffers` | Caller-owned destination buffers | Pillow does not expose `decode_into` capacity, short-destination rejection, or no-partial-write guarantees | `covered` | Manifest evidence: `tests/feature_gate_tests.rs` | `excluded` |
| `source-provenance` | Source provenance | `SourceDescriptor`, FileTypeBox facts, AVIF item/property identity, raw source relationships, and declared-versus-confirmed fields are not Pillow result fields | `covered` | Manifest evidence: `tests/feature_gate_tests.rs` | `excluded` |
| `structured-diagnostics` | Structured diagnostics | Rust diagnostic kind, offset, consumed extent, recovery status, and provenance are not Pillow's ordinary return shape | `covered` | Manifest evidence: `tests/feature_gate_tests.rs`, `scripts/verify_diagnostic_provenance.py`; target-only fault cases: `fault-contract:avif:avif_segment_map_reservation_failure`, `fault-contract:avif:avif_10bit_i444_alpha_plane_length_mismatch`, `fault-contract:avif:avif_12bit_i444_generic_alpha_plane_length_mismatch`, `fault-contract:avif:avif_10bit_i444_alpha_invalid_sample`, `fault-contract:avif:avif_10bit_i444_alpha_output_reservation_failure`, `fault-contract:avif:avif_12bit_i444_generic_rgb_output_reservation_failure`, `fault-contract:avif:avif_subsampled_rgb_output_reservation_failure`, `fault-contract:avif:avif_12bit_i420_alpha_sample_count_mismatch`, `fault-contract:avif:avif_12bit_i444_generic_u_plane_length_mismatch`, `fault-contract:avif:avif_12bit_i444_generic_v_plane_length_mismatch`, `fault-contract:avif:avif_12bit_i444_generic_invalid_sample`, `fault-contract:avif:avif_10bit_i444_u_plane_length_mismatch`, `fault-contract:avif:avif_10bit_i444_v_plane_length_mismatch`, `fault-contract:avif:avif_display_plane_copy_allocation_failure`, `fault-contract:avif:avif_display_plane_copy_sequence_allocation_failure`, `fault-contract:avif:avif_film_grain_output_reservation_failure`, `fault-contract:avif:avif_film_grain_points_reservation_failure`, `fault-contract:avif:avif_temporal_motion_field_reservation_failure`, `fault-contract:avif:avif_sequence_frames_reservation_failure`, `fault-contract:avif:avif_superresolution_positions_reservation_failure`, `fault-contract:avif:avif_superresolution_plane_reservation_failure`, `fault-contract:avif:avif_superresolution_header_geometry_mismatch`, `fault-contract:avif:avif_superresolution_chroma_u_plane_reservation_failure`, `fault-contract:avif:avif_superresolution_chroma_v_plane_reservation_failure`, `fault-contract:avif:avif_sgr_restoration_output_reservation_failure`, `fault-contract:avif:avif_sgr_intermediate_reservation_failure`, `fault-contract:avif:avif_restoration_stripe_scratch_reservation_failure`, `fault-contract:avif:avif_restoration_stripe_output_reservation_failure`, `fault-contract:jpeg:jpeg_multiscan_coefficient_reservation_failure`, `fault-contract:jpeg:jpeg_huffman_table_slot_reservation_failure`, `fault-contract:jpeg:jpeg_ycck_output_reservation_failure`, `fault-contract:jpeg:jpeg_h1v2_upsample_plane_reservation_failure`, `fault-contract:jpeg:jpeg_h1v2_upsample_empty_geometry_rejection`, `fault-contract:jpeg:jpeg_h1v2_upsample_source_length_mismatch`, `fault-contract:jpeg:jpeg_h1v2_upsample_zero_width_rejection`, `fault-contract:jpeg:jpeg_fancy_upsample_row_invalid_geometry`, `fault-contract:jpeg:jpeg_fancy_upsample_row_zero_height_rejection`, `fault-contract:jpeg:jpeg_fancy_upsample_row_zero_horizontal_ratio_rejection`, `fault-contract:jpeg:jpeg_fancy_upsample_row_zero_vertical_ratio_rejection`, `fault-contract:jpeg:jpeg_fancy_upsample_row_short_stride`, `fault-contract:jpeg:jpeg_row_upsample_source_length_mismatch`, `fault-contract:avif:avif_monochrome_tile_reservation_failure`, `fault-contract:avif:avif_monochrome_group_buffer_reservation_failure`, `fault-contract:avif:avif_color_tile_reservation_failure`, `fault-contract:avif:avif_tile_cell_reservation_failure`, `fault-contract:avif:avif_tile_block_metadata_reservation_failure`, `fault-contract:avif:avif_partition_node_reservation_failure`, `fault-contract:avif:avif_partition_context_above_reservation_failure`, `fault-contract:avif:avif_chroma_tile_cell_reservation_failure`, `fault-contract:avif:avif_assembled_loop_filter_metadata_reservation_failure`, `fault-contract:avif:avif_projected_temporal_field_reservation_failure`, `fault-contract:avif:avif_grid_cell_reservation_failure`, `fault-contract:avif:avif_monochrome_grid_cell_reservation_failure`, `fault-contract:avif:avif_monochrome_loop_filter_metadata_reservation_failure`, `fault-contract:avif:avif_monochrome_cdef_region_map_reservation_failure`, `fault-contract:avif:avif_monochrome_cdef_active_map_reservation_failure`, `fault-contract:avif:avif_retained_temporal_sample_reservation_failure`, `fault-contract:avif:avif_tile_group_temporal_sample_reservation_failure`, `fault-contract:avif:avif_assembled_cdef_region_map_reservation_failure`, `fault-contract:avif:avif_tile_cdef_region_map_reservation_failure`, `fault-contract:avif:avif_tile_cdef_active_map_reservation_failure`, `fault-contract:avif:avif_assembled_cdef_active_map_reservation_failure`, `fault-contract:avif:avif_item_table_reservation_failure`, `fault-contract:avif:avif_item_location_table_reservation_failure`, `fault-contract:avif:avif_item_extent_span_reservation_failure`, `fault-contract:avif:avif_sgr_source_reservation_failure`, `fault-contract:avif:avif_monochrome_alpha_plane_length_mismatch`, `fault-contract:bmp:bmp_palette_reservation_failure`, `fault-contract:bmp:bmp_decoded_pixels_reservation_failure`, `fault-contract:bmp:bmp_rle_entry_work_budget_exceeded`, `fault-contract:bmp:bmp_rle_command_work_budget_exceeded`, `fault-contract:png:png_decoded_sample_buffer_reservation_failure`, `fault-contract:png:png_apng_sequence_counter_exhaustion`, `fault-contract:png:png_apng_inspection_sequence_counter_exhaustion`, `fault-contract:tiff:tiff_lzw_zero_expected_output_rejection`, `fault-contract:tiff:tiff_lzw_zero_expected_output_token_rejection`, `fault-contract:tiff:tiff_header_registered_magic_rejection`, `fault-contract:tiff:tiff_strip_assembly_reservation_failure`, `fault-contract:tiff:tiff_unaligned_strip_assembly_reservation_failure`, `fault-contract:tiff:tiff_rgb16_output_reservation_failure`, `fault-contract:tiff:tiff_ycbcr_clump_length_mismatch`, `fault-contract:tiff:tiff_ycbcr_general_clump_length_mismatch`, `fault-contract:tiff:tiff_ycbcr_general_rgb_output_reservation_failure`, `fault-contract:tiff:tiff_ycbcr_full_resolution_length_mismatch`, `fault-contract:tiff:tiff_ycbcr_full_resolution_length_mismatch_custom_tags`, `fault-contract:tiff:tiff_ycbcr_rgb_output_reservation_failure`, `fault-contract:tiff:tiff_ycbcr_custom_table_clump_length_mismatch`, `fault-contract:tiff:tiff_ycbcr_custom_table_rgb_output_reservation_failure`, `fault-contract:tiff:tiff_ycbcr_custom_general_clump_length_mismatch`, `fault-contract:tiff:tiff_ycbcr_custom_general_rgb_output_reservation_failure`, `fault-contract:tiff:tiff_ycbcr_custom_1x2_clump_length_mismatch`, `fault-contract:tiff:tiff_ycbcr_custom_1x2_rgb_output_reservation_failure`, `fault-contract:tiff:tiff_ycbcr_1x2_clump_length_mismatch`, `fault-contract:tiff:tiff_ycbcr_1x2_rgb_output_reservation_failure`, `fault-contract:tiff:tiff_ycbcr_tile_assembly_reservation_failure`, `fault-contract:tiff:tiff_ycbcr_tile_prefix_limit_failure`, `fault-contract:tiff:tiff_ignored_sample_row_bytes_zero`, `fault-contract:tiff:tiff_ignored_sample_assembly_length_mismatch`, `fault-contract:tiff:tiff_ycbcr_horizontal_subsampling_zero`, `fault-contract:tiff:tiff_ycbcr_vertical_subsampling_zero`, `fault-contract:tiff:tiff_ycbcr_custom_horizontal_subsampling_zero`, `fault-contract:tiff:tiff_ycbcr_custom_vertical_subsampling_zero`, `fault-contract:avif:avif_monochrome_portable_subsampling_x_rejected`, `fault-contract:avif:avif_monochrome_portable_subsampling_y_rejected`, `fault-contract:avif:avif_monochrome_portable_u_plane_nonempty_rejected`, `fault-contract:avif:avif_monochrome_portable_v_plane_nonempty_rejected`, `fault-contract:avif:avif_monochrome_portable_y_plane_length_mismatch`, `fault-contract:avif:avif_monochrome_portable_alpha_plane_length_mismatch`, `fault-contract:avif:avif_monochrome_portable_y_sample_out_of_depth`, `fault-contract:avif:avif_monochrome_portable_alpha_sample_out_of_depth`, `fault-contract:avif:avif_monochrome_portable_output_length_mismatch`, `fault-contract:webp:webp_hash_chain_index_overflow_rejected`, `fault-contract:webp:webp_riff_header_too_short_rejected`, `fault-contract:webp:webp_riff_signature_mismatch_rejected`, `fault-contract:webp:webp_form_type_mismatch_rejected` | `excluded` |
| `feature-target-capability` | Feature and target capability | Pillow does not model this crate's Cargo feature-disabled errors or native versus `wasm32-wasip1` capability table | `covered` | Manifest evidence: `tests/feature_gate_tests.rs`, `tests/capability_table.rs` | `excluded` |
| `cache-concurrency-api-lifecycle` | Cache/concurrency/API lifecycle | Pillow does not expose `EncodedImage` lazy-cache states, Rust clone sharing, bounded native concurrent verification, or this crate's frame/page lifecycle | `covered` | Manifest evidence: `tests/feature_gate_tests.rs` | `excluded` |
| `release-package-surface` | Release package surface | Pillow cannot inspect this crate's Cargo archive, included source/legal files, or deliberate exclusion of parity fixtures and repository-only integration targets | `covered` | Manifest evidence: `tests/fixtures/package_surface_manifest.json`, `scripts/verify_package_surface.py` | `excluded` |
| `allocation-stack-coverage-models` | Allocation/stack/coverage models | Pillow cannot witness Rust allocator checkpoints, stack measurements, or private defensive branches | `planned` | No category-specific evidence is claimed. Planned context: `scripts/benchmark_fixture_workloads.py`, `scripts/verify_coverage_origins.py`, `tests/fixtures/coverage_origin_manifest.json` | `excluded` |

Rust-only results do not belong in Pillow parity rows. The shared matrix has a
separate `fault_contracts` lane for deterministic target-internal failures that
ordinary inputs cannot trigger. Each such row is typed as `fault-contract`,
uses `oracle_status: not_applicable`, and shares the matrix selector and
case-index machinery without contributing to parity counts. Its input may be
an existing Pillow parity fixture; that keeps the stimulus reproducible but
does not turn the injected result into Pillow parity.

### AVIF monochrome group-buffer allocation fault contract — 2026-10-15

`fault-contract:avif:avif_monochrome_group_buffer_reservation_failure` reuses
the active `decode:avif:multitile_monochrome` Pillow parity input. The fixture
reaches the per-tile-group complete-monochrome buffer reservation, while the
coverage-only fault point forces its fallible reserve to report a structured
`Dimensions` error. The public fault-contract runner checks the AVIF format,
still-decode stage, exact diagnostic, and a successful retry matching a fresh
decode. The injected fault remains separate from Pillow parity and compiles
out of normal library builds.

The targeted parity and fault-contract runs passed 1/1 each. Coverage MCP's
LCOV incremental comparison found 52 newly observed lines overall, including
five in `src/codecs/avif/av1/frame.rs` at lines 3076 and 3085–3088 (0.0396
percentage points against the selected baseline). This is a limited coordinate
estimate: the local reports have no source/build receipts or test receipts.
Coverage MCP could not provide the branch-level delta because these reports do
not contain exact branch observations. No full coverage campaign was run.
Reports: `target/release-evidence/coverage-avif-monochrome-group-baseline-current.info`,
`target/release-evidence/coverage-avif-monochrome-group-fault-current.info`.
The coverage-origin inventory now records 149 exact guards across 25 Rust
files; this is a static provenance count, not a new full-coverage measurement.

### BMP RLE4 edge parity and palette-reservation fault contract — 2026-10-15

Two active Pillow parity rows cover BI_RLE4 odd-length absolute runs: one ends
at end-of-line with the omitted low nibble, and the other places the pixel
stream at an odd file offset to exercise word padding. Their input SHA-256
values are `e126b4422eb23399b411090b0a9928b7f4f46467ea6847fbd9c0da522b6a33fb`
and `39a7b1fbc8d09716958cd9ad55f72fb477e69c40d20afceabb357807d39fff03`.
Both selected public rows pass the pinned Pillow pixel parity. The BMP fixture
generator reproduces their inputs and reference provenance.

The target-only row
`fault-contract:bmp:bmp_palette_reservation_failure` reuses the first fixture.
Its coverage-only failpoint forces the palette's fallible reservation to fail
before palette entries are read. The public contract checks the BMP
`StillDecode` error `decode: unable to allocate BMP palette` and confirms that
a clean retry equals a fresh decode. The selected fault row passed 1/1; the
normal allocation path compiles without the fault hook. Coverage MCP found no
missing branch observation at `src/codecs/bmp/decode.rs:173`, but its selected
reports have no source/build receipts or test attribution, and branch-level
incremental comparison is incomparable because exact detail is incomplete.
No aggregate coverage gain or full-coverage result is claimed.
Selected reports: `target/release-evidence/coverage-bmp-rle4-odd-eol-current.json`,
`target/release-evidence/coverage-bmp-rle4-odd-offset-current.json`, and
`target/release-evidence/coverage-bmp-palette-fault-current.json`.

### JPEG SOS frame-order parity and fast-path predicates — 2026-10-15

The public parity row `decode:jpeg:baseline_420_reversed_chroma_sos_order`
uses a pinned baseline 4:2:0 input (`baseline_420.jpg`, SHA-256
`c669c42fcf388430afd4400bbdcbc66f30165d50e4b162ed77e1f67392cce4ef`) with
only the SOS component records changed. The candidate hash is
`d2c2d756ed55c4cf7b6af6ee4a614edd53b261d72d3b1c4beadcf784c20bd26d`. Pillow
12.2.0 opens and structurally verifies the input, then reports a broken data
stream when pixels are loaded. Rust now preserves marker-only inspection and
verification while returning a malformed decode error for the invalid
frame-order scan.

The current fixture matrix contains 2,243 total rows: 1,762 decode /
inspect / verify rows and 481 encode rows. Of those, 1,762 decode rows and
449 encode rows are active.

The selected control, malformed case, and multi-scan control passed 3/3; the
complete JPEG decode matrix passed 252/252. After the final CMYK direct-index
simplification, the CMYK control and partial-block cases passed 2/2. Coverage
MCP reports no missing branch observations in
`validate_scan_component_order` or the 4:2:0 direct reconstruction helper for
the complete JPEG workload. Its local report has no source/build or test
receipts, and comparison with the historical full-suite baseline is
incomparable because exact detail does not match. This evidence
does not claim an aggregate coverage gain or a full-suite coverage result.

### Pillow-tolerated iTXt without a keyword separator — 2026-10-16

The input-only row
`decode:png:pillow_tolerated_malformed_itxt_missing_keyword_separator` uses
a complete 1×1 RGB PNG with a valid-CRC iTXt chunk whose payload contains no
NUL byte. Its reproducible generator pins the fixture SHA-256 to
`ef5663b10dd1bef8481a1f73bee3283038fc41a3bd928cd0ba7945654efb98cd`.
Pillow 12.2.0 opens and verifies the file, loads the same three RGB bytes as
the base image, and omits the malformed metadata. The selected public parity
row passed 1/1 across detection, inspection, verification, decode, and
sequence decode. The fixture is an ordinary malformed-input case, not a
target-only fault contract; runtime code is unchanged.

Coverage MCP's focused report records 2,315/211,155 observed regions for the
selected test process and still reports a missing observation at
`src/codecs/png/decode.rs:119` in
`invalid_compressed_metadata_identity`. The incremental comparison with the
earlier PNG matrix report is incomparable because the selected report contains
observations outside the baseline inventory. Both local reports lack
source/build receipts and test attribution. No aggregate coverage gain or
full-suite result is claimed, and no full coverage campaign was run. The
selected report is
`target/release-evidence/coverage-png-itxt-missing-keyword-separator-selected.json`.

The same gap review identified redundant fast-path predicates. Component
order is checked once at decode materialization; per-component sampling
records determine their maxima, and the reconstruction table vector is built
once per frame component. The 4:2:0, 4:2:2, 4:4:4, grayscale, and CMYK direct
paths now avoid rechecking those derived facts. CMYK also uses the validated
frame order directly instead of rebuilding a scan-order map. The new JPEG
input remains ordinary Pillow parity evidence; it is not a target-only fault
contract. Reports:
`target/release-evidence/coverage-jpeg-direct-safe-cleanup-current.json` and
`target/release-evidence/coverage-jpeg-cmyk-order-regression-current.json`.

### JPEG active chroma AC Huffman table parity — 2026-10-16

The public row `decode:jpeg:baseline_444_custom_ac_chroma_values` changes two
unused codeword symbols in AC DHT table 1, which the first scan selects for
both chroma components. The generated input is pinned to SHA-256
`157f83e6e9a3f0d41dde66adf35dc156d1efaa1acfd77c64a83c782214059799`;
Pillow 12.2.0 returns the baseline 128x128 RGB pixels with SHA-256
`2288f8f3cb6dafaa523538760165f2a4634b9a269e03175b8fbc284158dbb94a`.
This is valid-input public parity, not a target-only fault contract. The
selected matrix test passed 1/1.

Coverage MCP's selected-row comparison with the `subsampling_444` control
observed two additional false-arm coordinates in
`standard_huff_table` (`huffman.rs:95`) and
`standard_ac_general_pair_table` (`huffman.rs:428`). The focused report is
`target/release-evidence/coverage-jpeg-active-ac-chroma.json`; the control is
`target/release-evidence/coverage-jpeg-baseline444-control.json`. The reports
lack source/build receipts and test attribution, so this is limited coordinate
evidence. Comparison with the saved full baseline is incomparable because its
branch inventory differs (31,268 versus 31,276 outcomes). No aggregate
coverage gain or full-suite result is claimed, and no full coverage campaign
was run.

### Pillow-tolerated zTXt without a keyword separator — 2026-10-16

The input-only row
`decode:png:pillow_tolerated_malformed_ztxt_missing_keyword_separator` uses a
complete 1×1 RGB PNG with a valid-CRC zTXt chunk whose payload contains no NUL
byte. The compressed text is valid, but the PNG text keyword separator is
absent. Pinned Pillow 12.2.0 opens and verifies the file, then loads the same
three RGB bytes as the unmodified input. The selected public parity row passes
1/1 across detection, inspection, verification, decode, and sequence decode.
The decoder already preserves this tolerated input; this change adds an
independent malformed-metadata parity witness without altering runtime code.

Coverage MCP's branch comparison with the earlier PNG matrix report is
incomparable because the exact branch detail differs. A focused incremental
comparison against the earlier full PNG matrix report is incomparable because
the selected report contains observations outside that baseline inventory.
Comparing the selected row with the invalid-compression control reports no new
region observations and 17 control observations absent from the selected report;
source/build identity is unverified, so those differences cannot establish a
coverage gain or regression. The selected report still shows unobserved PNG
regions, including in `invalid_compressed_metadata_identity`; no full-suite
coverage campaign was run. Reports:
`target/release-evidence/coverage-png-ztxt-invalid-compression-control-final.json`
and `target/release-evidence/coverage-png-ztxt-missing-separator-final.json`.

### Odd-width 1-bit PNG encode parity — 2026-10-16

The public row `encode:png:enc_1bit_odd_width` uses a complete 9×3 mode-1 PNG
saved by pinned Pillow 12.2.0. Its source SHA-256 is
`ab470446e941a51011e157e861e3f920b911686d3e592e1b7cb1d6a683bdd675`;
Pillow decodes pixels `5980a6000080` with SHA-256
`36e3c5d288415910fd696272299e53b64a80b66782e965c506b57e7dd2d8d203`. The
rows exercise unused packed tail bits while setting the ninth pixel to both
zero and one. The exact 74-byte encoded reference is byte-identical to
Pillow's output. The selected manifest parity test passed 1/1. Its fixture
generator checks the pinned PNG without importing Pillow; the optional
`--verify-pillow` path checks the pinned save and pixel roundtrip.

The selected branch report records 230/31,276 observations and reaches the
1-bit row packing plus non-palette encoder paths at `encode.rs:70` and
`:107`/`:429`. Coverage MCP's selected-test comparison reports 28 newly
observed branch coordinates, including the false arms at lines 107 and 429.
This is limited coordinate evidence: source/build receipts and test
attribution are unavailable. The saved PNG baseline is 559/31,314 branches;
its exact detail differs from the selected report, so the incremental union is
incomparable. No aggregate coverage gain, full-suite result, or full coverage
campaign is claimed. The selected report is
`target/release-evidence/coverage-png-1bit-odd-width-final.json`.

### PNG decoded-sample reservation fault contract — 2026-10-16

`fault-contract:png:png_decoded_sample_buffer_reservation_failure` reuses the
active `decode:png:size_1x1` Pillow input. `decode_scanlines` now reserves the
checked sample count fallibly, then initializes the buffer; a reservation
failure returns a structured PNG `Dimensions` error at `StillDecode`. The
one-shot `png.scanline.sample_buffer_reservation` injection exists only in
coverage builds. The shared runner verifies the error and confirms that retry
produces the same decoded image as a fresh decode. The ordinary parity row and
the selected fault row each passed 1/1; the injected result remains in the
`oracle_status: not_applicable` lane.

The ordinary-row baseline and fault-row reports were collected separately from
the same checkout. Coverage MCP reports 163/31,278 observed branch outcomes
for the selected ordinary row and 91/31,278 for the selected fault row. Its
incremental comparison is incomparable because exact baseline detail is
incomplete; both reports also lack source/build receipts and test attribution.
A focused gap query still identifies unobserved `decode_scanlines` branches at
`decode.rs:1583` and `decode.rs:1605`; the selected 1×1 case does not cover the
Adam7 path. No aggregate coverage gain or full-suite coverage result is
claimed, and no full coverage campaign was run. Reports:
`target/release-evidence/coverage-png-sample-buffer-baseline.json` and
`target/release-evidence/coverage-png-sample-buffer-reservation.json`.

### BMP RLE8 delta crossing the row boundary — 2026-10-16

`compression_rle8_rle8_delta_row_boundary` adds an input-only Pillow parity
case based on the existing 4×2 RLE8 fixture. Its compressed payload contains a
delta `(dx=5, dy=0)`, end-of-line, a four-pixel run, and end-of-bitmap; the
complete file declares its 1,088-byte size and 10-byte compressed payload. The
pinned Pillow 12.2.0 oracle opens and verifies it as a 4×2 palette image and
decodes eight zero indices. The selected matrix test passed 1/1.
The active matrix now contains 2,243 rows: 1,762 decode and 481 encode rows.

The selected branch report is
`target/release-evidence/coverage-bmp-rle8-delta-boundary.json`. Coverage MCP
reports 115/31,278 branch outcomes observed and groups 16 missing branches in
`decode_rle`. Its incremental comparison against
`.coverage-mcp/local-evidence/20261014/github-ci-613ac4e0/coverage.json` is
incomparable: both reports are source-unverified, test attribution is unknown,
and the baseline branch detail is incomplete. No aggregate coverage gain is
claimed; no full coverage campaign was run.

### TIFF normal-order byte-aligned 2-bit tile parity — 2026-10-16

The active row `decode:tiff:tiled_gray2_byte_aligned_edge` adds a normal-order
companion to the existing FillOrder=2 tiled 2-bit grayscale case. The pinned
input `tiled_gray2_byte_aligned_edge.tiff` has SHA-256
`f3fcb7652e0eda354ec21bee46fbfedbc4c0a47476d5ae9d5147ca382c3cbce7`. Pinned
Pillow 12.2.0 decodes it as a 16×1 L8 image; its 16-byte pixel reference has
SHA-256 `d6591e2f9c5c4f4156323d28bb19ac453d1ac5f4d0ede89805b7ce5316e6fc2e`.
The existing `scripts/generate_test_assets.py` generator builds it with
`write_low_depth_tiled_tiff` as two 8×1 tiles and checks equal Pillow pixels
against the FillOrder=2 counterpart.
The input-only parity row passed 1/1 across its selected matrix operations;
Rust implementation code is unchanged.

The current fixture matrix contains 2,244 total rows: 1,763 decode /
inspect / verify rows and 481 encode rows. Of those, 1,763 decode rows and
449 encode rows are active.

The selected branch report is
`target/release-evidence/coverage-tiff-gray2-byte-aligned.json` and records
156/31,278 observed branch outcomes. At `decode.rs:1339`, Coverage MCP reports
the `tail_bits != 0` arm as unobserved for `copy_packed_tile_row::<false>`, so
the byte-aligned false arm is present in this selected report. Both arms of
the `<true>` specialization remain unobserved under this selection. Comparison
with the saved local baseline is incomparable because branch detail is
incomplete; the reports also lack source/build receipts and test attribution.
No aggregate coverage gain or full-suite coverage result is claimed, and no
full coverage campaign was run.

### AVIF `iinf` wrong-child-kind parity — 2026-10-16

The active row `decode:avif:error_iinf_child_not_infe` adds a complete-file
malformed-input case for the `parse_iinf` child-kind rejection. The deterministic
generator `scripts/generate_avif_iinf_child_kind_fixture.py` checks the pinned
3,077-byte `baseline.avif` source
(`d4327b7ab11ed8f11d86978258fc04e5505bcfe511ca2c4efa4838c85d226fd2`), verifies
the sole version-zero `iinf` entry is an `infe` box, and changes only its type
to `free`. The output hash is
`ee95c49866ddcf2bf1fd4f2f3a0e4c1d2c768bb5ccbf8f675555691298225d76`. Pinned
Pillow 12.2.0/libavif 1.4.1 rejects the complete input as
`PIL.UnidentifiedImageError: cannot identify image file <bytes>`; the public
matrix records successful format detection and that normalized error for
inspection, verification, decode, and sequence decode.

The current fixture matrix contains 2,245 total rows: 1,764 decode /
inspect / verify rows and 481 encode rows. Of those, 1,764 decode rows and
449 encode rows are active.

The selected parity test passed 1/1. Coverage MCP's focused branch query at
`src/codecs/avif/samples.rs:590` lists only the `false` arm of
`child.kind != infe` as unobserved, so the selected report records the
wrong-child rejection arm. The report
`target/release-evidence/coverage-avif-iinf-child-wrong-kind.json` has SHA-256
`1c852014db08a06b706a2bffed3ec63230c032c6070a0fb86616dc6e26412edb` and
records 77/31,278 observed branch outcomes. Coverage MCP's incremental
comparison with `.coverage-mcp/local-evidence/20261013/avif-iinf-zero-count/coverage.json`
is incomparable because branch detail is incomplete and the inventories differ
(31,278 versus 31,306). Both reports lack source/build receipts and test
attribution. No aggregate coverage gain or full-suite result is claimed; no
full coverage campaign was run.

`make verify` now includes the generator's hash-only `--check` and passes;
the refreshed malformed-input ledger contains 609 classes across eight
formats.


### JPEG multiscan positive-size AC tail overrun parity — 2026-10-16

The active input-only row `baseline_multiscan_ac_pair_tail` adds a 172-byte
SOF0 4:4:4 JPEG with three single-component scans. Its Y scan places a positive
size-eight AC symbol after a run that advances past coefficient 63; the scalar
multiscan decoder consumes its amplitude and stores the value at the final
coefficient. The pinned Pillow 12.2.0 oracle verifies the input as 8×8 RGB. The input SHA-256 is
`ab2f907a724ae03917b169c72868ae43dfc9e6d38dece1c5521039bebc5f26f6`; the exact
192-byte RGB reference SHA-256 is
`ef3e84f55ebfea3a4470bedbe9341f5145b718d560f870428ab76bb6dc9f43dc`.

The focused public matrix test passed 1/1. It exercises detection, inspection,
verification, still decode and sequence decode against Pillow references. The
fresh GitHub [x86 CI artifact](https://github.com/appunni-m/image-slash-star/actions/runs/38834683027)
for source commit `7c32909e` recorded zero hits for the `k >= 64` true arm at
`src/codecs/jpeg/decode/decode.rs:89`; Coverage MCP groups that arm as missing.
The extracted CI report is
`target/release-evidence/coverage-jpeg-x86_64-ci.json` (SHA-256
`f8423fa474f19658377965edcb1ca63e3b69bf973389e06e34399f11105b6208`). The
selected local LLVM report records four true-arm and four false-arm hits, and
Coverage MCP finds no remaining gap at that location. Its report is
`target/release-evidence/coverage-jpeg-multiscan-ac-pair-tail.json` (SHA-256
`0cd67a75fadaa4237422b8a448b72274fcc781fd10b6f690507d2041ce86be07`). The
baseline and selected reports have different branch inventories (302 and 308),
and both lack source/build receipts and test attribution, so their incremental
comparison is incomparable. This is bounded selected-path evidence, not an
aggregate coverage gain or a full coverage result. No Rust decoder or SIMD code
changed.

The current fixture matrix contains 2,246 total rows: 1,765 decode /
inspect / verify rows and 481 encode rows. Of those, 1,765 decode rows and
449 encode rows are active. No full coverage campaign was run.

### JPEG multiscan invalid DC category parity — 2026-10-16

The active public error row `error_baseline_multiscan_dc_category_16` adds a
complete 165-byte SOF0 4:4:4 JPEG with three single-component scans. Its first
scan contains DC category 16, which is invalid for baseline JPEG. The input
`baseline_multiscan_dc_category_16.jpg` has SHA-256
`807a3da9cac05080e6182c047e551652eeee8e94653ef030a4c60775e4c4c4ea`. The
generator mutates the existing deterministic three-scan fixture; pinned Pillow
12.2.0 verifies the file structure and rejects pixel loading with
`OSError: broken data stream when reading image file`.

The focused public matrix test passed 1/1 for this row. Coverage MCP's selected
branch report is
`target/release-evidence/coverage-jpeg-multiscan-dc-category-16-local.json`
(SHA-256
`b3048e2376faa1a96b4c130653b6d6177540a1c502187c82d51064263ebcdcac`). It
records 160/31,278 observed branch outcomes. At
`src/codecs/jpeg/decode/decode.rs:57`, Coverage MCP lists only the `false` arm
of `dc_cat > 15` as unobserved, so this selected report observes the invalid
category arm in scalar `decode_block`. The previous selected multi-scan AC
report shows the complementary arm. The incremental comparison is
incomparable because exact branch detail is incomplete; both reports lack
source/build receipts and Coverage MCP test attribution. These are selected
path observations only, not an aggregate coverage gain or full-suite result.

The current fixture matrix contains 2,255 total rows: 1,774 decode /
inspect / verify rows and 481 encode rows. Of those, 1,774 decode rows and
449 encode rows are active. The regenerated malformed-input ledger contains
611 classes across eight formats. The Rust AV1 decoder gained the 12-bit clipped
B32 lossless inter path; no SIMD code changed, and no full coverage campaign
was run.

### APNG sequence-counter exhaustion fault contract — 2026-10-16

`fault-contract:png:png_apng_sequence_counter_exhaustion` reuses the active
`decode:png:apng_sequence_controls` Pillow fixture, which has five
sequence-bearing chunks. Its coverage-only virtual-prefix fault makes the
first control consume `u32::MAX`, then verifies that the following sequence
chunk returns `ImageError::Malformed` for PNG at `SequenceDecode` with
`decode sequence: APNG sequence number overflows`. The sequence counter now
uses an exhausted `Option<u32>` state, so accepting MAX cannot wrap to zero.
The shared runner also retries without injection and compares the result with
a fresh sequence decode. The injected outcome remains
`oracle_status: not_applicable`, separate from Pillow parity.

The selected public fault row and the unchanged APNG parity row each passed
1/1. Coverage MCP measured 154/31,280 branch outcomes in the selected report
and found no missing branch observation at the exhaustion guard in
`src/codecs/png/decode.rs:1233`; it still reports the independent sequence
mismatch arm at line 1238 as uncovered by this selected fault run. The
incremental comparison with the saved October 13 baseline is incomparable
because exact branch details differ; neither report has source/build receipts
or test attribution. No aggregate coverage gain or full-suite result is
claimed, and no full coverage campaign was run. Report:
`target/release-evidence/coverage-apng-sequence-counter-exhaustion.json`.
The static coverage-origin inventory now records 151 exact `cfg(coverage)`
guards across 26 Rust files; this does not change the historical full-coverage
baseline.

### APNG inspector sequence-counter exhaustion fault contract — 2026-10-17

`fault-contract:png:png_apng_inspection_sequence_counter_exhaustion` reuses
the active `decode:png:apng_sequence_controls` Pillow fixture. The coverage-only
fault models a valid prefix through `u32::MAX` before the inspector's next
frame-control chunk. It verifies a public `ImageError::Malformed` for PNG at
`Inspection` with `inspect: PNG animation sequence number overflows`, then
retries `img::inspect` without injection and compares its metadata with a
fresh inspection. The row uses `oracle_status: not_applicable`; the separate
decoder sequence-exhaustion contract remains unchanged.

The selected run included `decode:png:apng_sequence_controls`,
`fault-contract:png:png_apng_sequence_counter_exhaustion`, and this inspection
fault row. The matrix test binary passed. A fault-only selected rerun reported
two selected and executed fault contracts, both passing (2/2), with zero
failures and `oracle=not_applicable`. The selected branch report is
`.coverage-mcp/local-evidence/20261017/png-apng-inspect-selected/coverage.json`,
SHA-256
`c82b8cc6f2b90b1a75f5af5ca9bac0f281a8423c50602ecf875b5dae80c518c2`.
Coverage MCP found no missing branch observation at the exhaustion guard in
`src/codecs/png/inspect.rs:195`; the independent sequence-mismatch arm at line
200 remains unobserved. The report is source-unverified, has no build ID or
recorded revision, and reports test attribution as unknown. These selected
results do not establish aggregate coverage or a full-suite result. No full
coverage campaign was run.

### AV1 partition-context above reservation fault contract — 2026-10-16

`fault-contract:avif:avif_partition_context_above_reservation_failure` reuses
the active `decode:avif:baseline` Pillow input. A coverage-only one-shot fault
requests an impossible capacity for `PartitionContexts::new`'s `above` buffer.
The public decode returns `ImageError::Dimensions` for AVIF at `StillDecode`
with `decode: AVIF AV1 validation failed: unable to allocate AV1 partition
context`. The shared runner then verifies that an uninjected retry equals a
fresh decode. The injected result is `oracle_status: not_applicable` and stays
outside Pillow parity counts.

The selected fault-contract row passed 1/1. Its JSON report is
`target/release-evidence/coverage-avif-partition-context-above.json`, retained
for Coverage MCP inspection. No aggregate coverage gain is attributed to this
individual case. A later full all-feature local run and its attribution
limits are recorded above. The exact coverage-origin inventory now records
152 `cfg(coverage)` guards across 26 Rust files.

### BMP palette-less indexed encoding parity — 2026-10-16

`encode:bmp:enc_indexed_missing_palette` reuses the 67-byte PNG asset
`palette_missing_plte.png` (SHA-256
`6e78b381ae98aa9a5bdb1e50179a74073cdb0d567b2d55c6e48e4b6c8faeb7c3`). Pinned
Pillow 12.2.0 loads it with `PIL.ImageFile.LOAD_TRUNCATED_IMAGES` enabled as a
1×1 P image without a palette and preserves sample `00`. It saves a 58-byte
8-bit BI_RGB BMP with offset 54, both palette counts zero, and four zero
raster/padding bytes; the encoded SHA-256 is
`64850622123ceb124c80eca8ee8c8f4a3ec3026580501d9ba44a7c256e5ba747`. Pillow
reopens the file under the same load setting and preserves the P-mode sample.

The new public row first reproduced a Rust/Pillow mismatch (1,082 bytes versus
58). The BMP encoder now omits the absent palette table and copies indexes
unchanged. The decoder admits the corresponding 8-bit BI_RGB layout only for
INFO-or-later headers whose pixel offset equals the end of the DIB header;
other indexed BMP files retain their existing palette requirements. The
reference generator records and restores the row-scoped Pillow load setting
for both source and roundtrip images.

The selected all-feature matrix run passed seven rows: BMP decode `depth_8`,
`os2_v1`, and `error_malformed`, plus encode `enc_indexed_missing_palette`,
`enc_8bit`, `enc_grayscale`, and `enc_24bit`. Formatting and strict workspace
Clippy passed. Coverage MCP's selected report is
`target/release-evidence/coverage-bmp-no-palette-controls-20261016.json`; its
queries report no unobserved branch outcomes at the new palette omission
predicate and the encoder's optional-palette branch. The report has no source
or test receipt, and its incremental comparison with the saved historical
full report is incomparable because that report's branch detail is incomplete.
No aggregate coverage gain or full-suite claim is made; no full coverage
campaign or SIMD change was made.

### AVIF wide-I444 mode-2 mixed topology parity — 2026-10-16

The active `animated_lossy_wide_i444_mode2_mixed_b128x128` row adds a
two-frame 256x256 lossy I444 input with fixed 128-pixel partitions. Only the
top-left 128x128 mode-2 inter leaf has one TX64 root split once to TX32; its
other three roots and the remaining leaves stay unsplit. No TX32 child splits.
The pinned generator
double-encodes the input and checks its SHA-256
`0e72f7d47e8e83e0a6e13a76a816cb98974cdb2976fff72857901712e2633d61` and both
Pillow RGB frame hashes. The complete-file loop observer reports infinite
repetition. The selected public row passes exact Pillow parity 1/1.

The selected branch report records the mixed-plan fallback at
`entropy.rs:7152`. In `block.rs:57706`, one decoder monomorphization observes
both `topology.is_some()` outcomes (8 true and 24 false); the remaining six
monomorphizations have no hits in this selected-only run. Coverage MCP reports
3,308/31,296 branch observations for the selected measurement, with source
identity unverified and test attribution unknown because no source/build or
test receipt accompanies the LLVM report. This is selected-path evidence,
not an aggregate coverage gain or full-suite result. Report:
`target/release-evidence/coverage-avif-mode2-mixed-selected-20261016.json`.
No full coverage campaign or SIMD change was made.

At this 2026-10-16 checkpoint, the fixture matrix contains 2,262 total rows: 1,779 decode /
inspect / verify rows and 483 encode rows. Of those, 1,779 decode rows and
451 encode rows are active; 32 planned encode rows remain.

### AV1 segment-map reservation fault contract — 2026-10-17

The active `decode:avif:mixed_lossless_clipped_b32_i420_60x60` fixture is a
321-byte still AVIF whose key frame enables segmentation without temporal
segmentation. Pinned Pillow 12.2.0 decodes it to 60×60 RGB8 pixels (10,800
bytes). Its source SHA-256 is
`ea8d484e1cde6819d1c98a64ff58db1bec7937fd929a835cf9c13c2da8430d1b`; the
Pillow pixel reference SHA-256 is
`a3b4b27575bef66a5eb058529aee5cac545d4d43a9bace07421cea066b4d59b8`.

`fault-contract:avif:avif_segment_map_reservation_failure` reuses that public
parity input to reach `SegmentMap::new`. Its coverage-only hook forces
`try_reserve_exact` to reject an impossible capacity before the map is
initialized. The public result is `ImageError::Dimensions` with
`format: Some(Avif)`, message
`decode: AVIF AV1 validation failed: unable to allocate AV1 segment map`,
`stage: Some(StillDecode)`, `offset: None`, and `identity: None`. The shared
fault runner checks the structured result, then confirms retry succeeds and
matches a fresh decode. `oracle_status: not_applicable` keeps the injected
failure separate from Pillow parity.

The selected parity row and selected fault-contract row each passed 1/1.
Coverage MCP's branch-gap query for `src/codecs/avif/av1/entropy.rs:855`
returned zero missing groups for this selected report. The incremental
comparison with `coverage-main-full-20261016.json` was incomparable because
its baseline detail does not match the reported totals; neither report has
source/build receipts or test attribution. No aggregate coverage gain is
claimed and no broad codec or full-coverage campaign was run. Report:
`target/release-evidence/coverage-avif-segment-map-fault-current.json`.
The static coverage-origin inventory now records 161 exact `cfg(coverage)`
guards across 26 Rust files.
The current fixture matrix contains 2,269 total rows: 1,786 decode /
inspect / verify rows and 483 encode rows. Of those, 1,786 decode rows and
451 encode rows are active; 32 planned encode rows remain. The
separate target-only fault lane contains 45 rows.

### AVIF 10-bit I444 alpha plane-length fault contract — 2026-10-17

`fault-contract:avif:avif_10bit_i444_alpha_plane_length_mismatch` reuses the
active `decode:avif:high_bitdepth_still_10bit_444_alpha_lossy_16x16` Pillow
input. Its `cfg(coverage)` hook shortens only the full-resolution alpha plane
before the converter's plane-length guard. The existing `Option` failure path
returns `ImageError::Unsupported` with AVIF, `NotImplemented`, and
`StillDecode`, with diagnostic `decode: AVIF input is outside the supported
pure-Rust decode subset`. The contract checks that exact structured result,
then confirms an uninjected retry equals a fresh decode. The injected outcome
uses `oracle_status: not_applicable` and remains separate from Pillow parity.

The selected fault-contract case passed 1/1. Coverage MCP's selected branch
report, `target/release-evidence/coverage-avif-10bit-i444-alpha-mismatch-20261017-final.json`,
shows the alpha-plane mismatch outcome at `decode.rs:948` was observed. Its
function query still lists four missing branch outcomes: the `split_last`
empty-plane path, true outcomes for the preceding U/V length checks, and
invalid sample-depth rejection. The incremental comparison with
`target/release-evidence/coverage-avif-10bit-444-alpha-16x16.json` is
incomparable because baseline branch detail does not reconcile with totals;
both reports lack source/build receipts and test attribution. No aggregate
coverage gain or full-coverage claim is made, and no full coverage campaign or
SIMD change was run.

### AVIF 10-bit I444 U/V length and sample-domain fault contracts — 2026-10-17

The target-only selectors
`fault-contract:avif:avif_10bit_i444_u_plane_length_mismatch`,
`fault-contract:avif:avif_10bit_i444_v_plane_length_mismatch`, and
`fault-contract:avif:avif_10bit_i444_alpha_invalid_sample` reuse the active
`decode:avif:high_bitdepth_still_10bit_444_alpha_lossy_16x16` Pillow input.
Coverage-only hooks independently shorten the U or V plane, or inject an
out-of-range sample into the helper's existing sample validation. Each case
asserts the established `ImageError::Unsupported` / `NotImplemented` /
`StillDecode` result and exact diagnostic, then confirms that a clean retry
matches a fresh decode. Their oracle status is `not_applicable`; none is
reported as Pillow parity.

The selected three-case fault runner passed. Coverage MCP's branch query on
`target/release-evidence/coverage-avif-10bit-i444-three-faults-20261017.json`
no longer lists the U-plane, V-plane, or invalid-sample true outcomes as gaps.
It lists three remaining outcomes in this helper: the alpha-fault hook, the
alpha-plane length guard, and the output-length guard. A same-source selected
alpha control passed in
`target/release-evidence/coverage-avif-10bit-i444-alpha-control-20261017.json`.
The incremental comparison between those selected reports was incomparable
because branch detail was incomplete or differed; source/build receipts and
test attribution are absent, so no additive coverage gain is claimed.

The output-length guard at `decode.rs:1004` remains a coverage limitation:
the helper reserves exactly four bytes per sample and emits one RGB triplet
plus one alpha byte for each sample. Injecting a shortened local output would
test an artificial private state rather than a reachable decode contract.

### JPEG baseline post-scan COM parity — 2026-10-17

Added `decode:jpeg:baseline_444_single_scan_trailing_com`, a Pillow 12.2.0
parity row using the deterministic 8×8 baseline 4:4:4 input with a valid COM
segment inserted between the entropy scan and EOI. The fixture SHA-256 is
`cf8e4443dcff4df7858b6573664aa3fe73dd401827aa293d0eca84eb0667f666`; its
192-byte RGB reference SHA-256 is
`7382706a9ecb0f6acae26e7c1c9a9be0e9471233c1a0c0da48d08a458115be2f`.

The first focused run exposed a public decode failure: the single-scan fast
boundary check rejected COM as a non-DQT marker, then generic entropy
extraction treated COM as entropy and returned no segment. The parser had
already recorded the end of scan entropy. The decoder now reuses that
boundary for a single baseline scan when parsed COM or DQT segments precede
EOI. This keeps entropy extraction bounded at the scan and avoids additional
pixel processing. The selected matrix row passes.

The selected LLVM report is
`target/release-evidence/coverage-jpeg-postscan-com-selected.json`. Coverage
MCP reports 271/31,378 selected branch outcomes; its function query shows only
the input-rejection arms of the helper remain unobserved for this selection.
The incremental comparison with
`target/release-evidence/coverage-main-full-20261016.json` is incomparable
because the baseline branch detail does not reconcile with its totals. Both
reports lack source/build receipts and test attribution, so no aggregate
coverage gain is claimed.

The current fixture matrix contains 2,273 total rows: 1,790 decode /
inspect / verify rows and 483 encode rows. Of those, 1,790 decode rows and
451 encode rows are active; 32 planned encode rows remain. The separate
target-only fault lane contains 47 rows.

### JPEG entropy-boundary regression guard — 2026-10-17

GitHub CI #658 exposed two regressions in the initial COM fix on x86-64 and
Windows: a fill byte before an RST marker shortened entropy extraction, and
an unexpected `FF02` marker after entropy was accepted by the fast path. The
boundary helper now walks the post-scan tail and only accepts well-formed COM
and DQT segments. Restart markers and other marker codes fall back to generic
entropy extraction, which preserves RST splitting and malformed-marker
rejection.

The selected matrix run passed the COM parity case, the restart-marker-fill
case, and the malformed unexpected-marker case. Its LLVM report is
`target/release-evidence/coverage-jpeg-tail-regressions.json`; Coverage MCP
reports 367/31,392 branch outcomes for this selection. Its source receipt and
test attribution are absent, and comparison with the historical full report
is incomparable because the baseline detail does not reconcile with its
totals. This is selected-run evidence only; it does not establish aggregate
coverage change.

### BMP DIB header parity — 2026-10-17

Added three full-file BMP parity cases: reject unknown DIB size 41 and retain
Pillow-supported INFO-family sizes 52 and 56. The fixtures derive from a
1×1, 24-bit BMP and adjust the DIB size, pixel offset, and file size together.
Pillow 12.2.0 reports `Unsupported BMP header type (41)` for the 41-byte
fixture; the successful 52- and 56-byte references decode to the same
1×1 RGB pixel with SHA-256
`25ff297dd78c82468376d6e2c93b672b49c01fb125a2812b94f8f9693a51b67e`.
The 41-byte input SHA-256 is
`331c249550844cee20e0230baaceecbe2f8b58f2ddacbfefe5c8bf9b0f680295`.

Decoder and inspector now share an explicit check for Pillow's supported
Windows INFO-family DIB sizes (40, 52, 56, 64, 108, and 124). This check runs
once while reading the header; pixel loops are unchanged. The selected matrix
run passed all three rows. Coverage MCP reports 95/31,392 observed branch
outcomes in
`target/release-evidence/coverage-bmp-dib-variants-selected.json`. Source and
test receipts are absent, and the comparison with the historical full report
is incomparable because its branch detail does not reconcile with totals.
No aggregate coverage gain is claimed.

### PNG zero-width encoder parity — 2026-10-17

Added the active public row `encode:png:enc_error_zero_width`, using the
existing `rgb.png` source transformed to dimensions `[0, 1]`. The pinned
Pillow 12.2.0 oracle reports `ValueError: cannot write empty image`; the Rust
encoder reports the structured `Dimensions` error. The PNG reference generator
added the case to the input index and coverage matrix without adding a binary
asset. The focused selected row passed 1/1. The existing zero-height control
also passed 1/1.

Coverage MCP reports 120/31,392 branch observations for
`target/release-evidence/coverage-png-zero-width-20261017.json`. Its query for
`src/types/mod.rs:2727` identifies the missing outcomes in
`DecodedImage::validate`; the width predicate's true outcome is absent from the
missing branch list, so the zero-width input reaches that path. Comparison
against both the historical full report and the selected zero-height control
is incomparable because report identities and branch detail do not reconcile.
The reports have no source/build receipts or test attribution, so no aggregate
coverage gain is claimed. No full coverage campaign or Rust source change was
made.

The current fixture matrix contains 2,274 total rows: 1,790 decode /
inspect / verify rows and 484 encode rows. Of those, 1,790 decode rows and
452 encode rows are active; 32 planned encode rows remain. The separate
target-only fault lane contains 47 rows.

### TIFF big-endian Gray16 Predictor=2 parity — 2026-10-17

Added the active public row
`decode:tiff:compression_deflate_be_unsigned16_wraparound_predictor`. The
4×2 Deflate TIFF is unsigned 16-bit grayscale, big-endian, and uses horizontal
Predictor=2. Its rows include modulo-65,536 underflow and overflow deltas. The
input SHA-256 is
`465f00da37c237c0b94fd6537a6cd2888c1bb1aa88c95821dcda4f0c98bd36a8`.
Pinned Pillow 12.2.0 returns mode `I;16B` and 16 reference bytes with SHA-256
`b6e268aab4491c42fd095133f3ea44c6dd1c4c455c027a7571943f6e1c93259a`.

The public `ImageMode::L16` transfer contract stores samples little-endian and
the decoded source descriptor retains the TIFF's big-endian provenance. The
sample values have equal high and low bytes, so Pillow's raw `I;16B` reference
also matches the public little-endian `L16` bytes exactly. Their predictor
deltas still include non-symmetric `0xfeff` and `0x0102` values, so correct
big-endian delta reconstruction is required for the sample comparison to pass.
The selected public LLVM coverage run passed 1/1. Its report records one
execution of `reverse_horizontal_predictor_with_token` and observes the 16-bit
predictor path. The 1,024-byte checkpoint outcome is not exercised by this
small fixture. Coverage MCP's incremental comparison could not complete
because its local daemon was unhealthy. The saved full baseline also lacks
source/build receipts and test attribution, so no aggregate coverage claim is
made.

The current fixture matrix contains 2,275 total rows: 1,791 decode /
inspect / verify rows and 484 encode rows. Of those, 1,791 decode rows and
452 encode rows are active; 32 planned encode rows remain. The separate
target-only fault lane contains 47 rows.

### TIFF big-endian Gray16 LZW Predictor=2 parity — 2026-10-17

Added active public row
`decode:tiff:compression_lzw_be_unsigned16_predictor`. The fixture is a 4×2
big-endian unsigned Gray16 TIFF using LZW compression and horizontal
Predictor=2. The generated LZW stream contains the predictor bytes as literal
codes; exact Pillow decoding validates the TIFF LZW stream and predictor
reconstruction together. The input SHA-256 is
`3599d2593242e2176e074e192728c47b4c0fe22ca57ed1bf58533affcb93cee8`.
Pinned Pillow 12.2.0 reports mode `I;16B` and 16 output bytes with SHA-256
`b6e268aab4491c42fd095133f3ea44c6dd1c4c455c027a7571943f6e1c93259a`.

The palindromic samples preserve exact raw-byte parity between Pillow's
big-endian mode and the public little-endian L16 transfer representation; the
Predictor=2 deltas still exercise byte-asymmetric wraparound values `0xfeff`
and `0x0102`. The selected public parity row passed 1/1. Coverage MCP's
incremental comparison against the saved full report is incomparable: branch
inventories differ (31,282 versus 31,392) and source/build receipts are absent.
A selected-tests comparison against the matching big-endian Deflate case is
limited and observes 32 additional branch locations, including TIFF LZW
decode paths, but both reports have unverified source identity and unknown
test attribution. These coordinates do not establish an aggregate coverage
gain.

The current fixture matrix contains 2,277 total rows: 1,793 decode /
inspect / verify rows and 484 encode rows. Of those, 1,793 decode rows and
452 encode rows are active; 32 planned encode rows remain. The separate
target-only fault lane contains 49 rows.

### WebP lossless options — 2026-10-17

Added ten active Pillow-backed WebP encode rows for quality 0/100, method 0,
partial and transparent alpha, method 5 quality boundaries, and method 6. The
lossless path now forwards its quality and method options into the VP8L
encoder. A histogram clamp correction matches libwebp's single-tile rule for
small power-of-two frames. The exact selected option rows passed after
reference generation.

Coverage MCP identified the large-image histogram-growth path as an open gap.
The attempted method-5 wide high-entropy row differed from Pillow by eight
encoded bytes and was removed. A passing method-4 sampling fixture did not
exercise the growth body. Focused LLVM reports have unverified source identity
and unknown test attribution; they are useful for locating code only and do not
establish aggregate coverage gains.

The current fixture matrix contains 2,287 total rows: 1,793 decode /
inspect / verify rows and 494 encode rows. Of those, 1,793 decode rows and
462 encode rows are active; 32 planned encode rows remain. The separate
target-only fault lane contains 49 rows.


### AVIF generic 12-bit I444 alpha length guard — 2026-10-17

Added the target-only fault-contract row
`fault-contract:avif:avif_12bit_i444_generic_alpha_plane_length_mismatch`. It
reuses the active Pillow-backed input
`decode:avif:high_bitdepth_still_12bit_444_alpha_9x9`. A coverage-only hook
shortens a present full-resolution alpha plane by one sample immediately
before the generic plane-length guard. The shared runner checks the structured
`Unsupported` / `NotImplemented` result at `StillDecode` and then confirms that
a retry equals a fresh decode. The injected state is target-only
(`oracle_status: not_applicable`) and is not counted as Pillow parity.

The selected fault-contract test passed 1/1, and the selected Pillow-backed
source parity control passed 1/1. Their branch reports are
`target/release-evidence/coverage-avif-i444-generic-alpha-mismatch-verified-20261017.json`
and `target/release-evidence/coverage-avif-i444-generic-alpha-control-20261017.json`.
The selected fault report records the generic alpha-length predicate as one
true and two false branch outcomes across the injected decode, retry, and fresh
decode. Coverage MCP could not load because its local daemon did not become
healthy; there is no MCP gap or incremental comparison result for this run,
and no aggregate coverage gain is claimed.

The static coverage-origin inventory now records 165 exact `cfg(coverage)`
guards across 26 Rust files. The parity fixture matrix remains at 2,277 rows
(1,793 decode / inspect / verify and 484 encode); the separate target-only
fault lane now contains 49 rows.


### AVIF 10-bit I444 alpha output reservation fault — 2026-10-17

Added the target-only fault-contract row
`fault-contract:avif:avif_10bit_i444_alpha_output_reservation_failure`. It uses
the active Pillow-backed input
`decode:avif:high_bitdepth_still_10bit_444_alpha_lossy_16x16` and injects a
failure at the AVIF 10-bit I444 alpha RGBA output reservation. The public
contract is a structured `Dimensions` error at `StillDecode`; a subsequent
decode must succeed and equal a fresh decode. This deterministic allocation
failure is target-only (`oracle_status: not_applicable`) and is not counted as
Pillow parity. A real `try_reserve_exact` failure now preserves the same
structured allocation error.

The selected target-only fault-contract test passed 1/1, and the selected
Pillow-backed AVIF parity row passed 1/1. A grouped fault-control run also
passed for the new reservation case and the four existing I444-alpha
unsupported cases; its report is
`target/release-evidence/coverage-avif-10bit-i444-alpha-fault-controls-20261017.json`.
The first two reports are
`target/release-evidence/coverage-avif-10bit-i444-alpha-output-reservation-20261017.json`
and `target/release-evidence/coverage-avif-10bit-i444-alpha-parity-20261017.json`.
Coverage MCP's focused branch query finds the new reservation and error outcomes
observed at the updated conversion lines. The selected report still lists other
fault-injection branches in the conversion helper as missing. Source/build
identity is unverified and test attribution is unknown, so these reports are
coordinate evidence only; no aggregate coverage gain is claimed.

The current fixture matrix contains 2,277 total rows: 1,793 decode /
inspect / verify rows and 484 encode rows. Of those, 1,793 decode rows and
452 encode rows are active; 32 planned encode rows remain. The separate
target-only fault lane contains 48 rows.

### AVIF generic 12-bit I444 RGB output reservation fault — 2026-10-17

Added `fault-contract:avif:avif_12bit_i444_generic_rgb_output_reservation_failure`
using the active Pillow-backed source row
`decode:avif:high_bitdepth_still_12bit_444_alpha_9x9`. In coverage builds,
the one-shot fault point supplies `usize::MAX` as the reservation length so
`try_reserve_exact` itself reaches the same typed `Dimensions` error mapping
used for a real reservation failure. The retry contract then verifies that an
uninjected decode equals a fresh decode. Normal builds reserve the exact
output length, and allocation failure remains reported at `StillDecode`.
The injected fault is target-only with `oracle_status: not_applicable`; the
source parity row remains independently Pillow-backed.

The selected fault-contract row passed 1/1 under coverage instrumentation.
The selected Pillow parity row passed 1/1 both under coverage instrumentation
and in a normal all-feature build. Reports:

- `target/release-evidence/coverage-avif-12bit-i444-generic-rgb-output-reservation-20261017.json` (SHA-256 `0fdb2f13f84756b5cee435730e2b0555daf6a90c360021c072d92a4a22480d15`)
- `target/release-evidence/coverage-avif-12bit-i444-generic-rgb-parity-20261017.json` (SHA-256 `17d701a1015f4cb74011a93f0643f48f6a724b7ce8cad9a645c769a6f8415d40`)

| Selected report | Lines | Branches | Functions | Regions |
| --- | ---: | ---: | ---: | ---: |
| Fault contract | 10,302 / 136,668 | 1,629 / 31,444 | 667 / 8,735 | 13,045 / 211,995 |
| Pillow parity | 11,526 / 136,668 | 1,832 / 31,444 | 792 / 8,735 | 14,851 / 211,995 |

Coverage MCP's selected-tests branch comparison reports two newly observed
coordinates in `convert_full_resolution_rgb`: the injected reservation arm
and the `try_reserve_exact(...).is_err()` arm at lines 927 and 936. Its focused
query found no missing branch outcomes at line 936. Both reports have
unverified source/build identity and unknown test attribution, so this is
selected coordinate evidence only. The incremental comparison with the saved
2026-10-16 full report is incomparable because its detailed branch inventory
does not reconcile with its totals. No aggregate coverage gain or full-suite
coverage claim is made.

The coverage-origin inventory now records 166 exact `cfg(coverage)` guards
across 26 Rust files. The parity matrix remains at 2,287 rows (1,793 decode /
inspect / verify and 494 encode); its separate target-only fault lane contains
50 rows.

### AVIF generic 12-bit I444 U/V and invalid-sample fault contracts — 2026-10-17

Added target-only rows for generic I444 U-plane length mismatch,
V-plane length mismatch, and an out-of-range sample. They reuse the active
Pillow-backed source row
`decode:avif:high_bitdepth_still_12bit_444_alpha_9x9`. The U/V faults shorten
their respective slices inside `convert_full_resolution_rgb`, after container
validation, so each short-circuit predicate is reached independently. The
invalid-sample hook appends `u16::MAX` to the same iterator consumed by the real
`SampleDepth::validate` guard. All hooks are compiled only with `cfg(coverage)`;
the shared runner checks the public `Unsupported` / `NotImplemented` result at
`StillDecode` and verifies a clean retry against a fresh decode. These states
are target-only (`oracle_status: not_applicable`) and remain separate from
Pillow parity.

The selected fault-contract run passed 1/1 for all three rows. The selected
Pillow parity control passed 1/1 under coverage instrumentation. Reports are
`target/release-evidence/coverage-avif-i444-generic-guards-20261017.json` and
`target/release-evidence/coverage-avif-i444-generic-rgb-control-20261017.json`.
Coverage MCP found the U, V, and sample-validation true arms missing in the
control report, and no missing branch outcome at their source lines in the
fault report. A selected-tests comparison reported six newly observed branch
coordinates, but is limited because source/build identity and test attribution
are unverified. Incremental union is incomparable because the selected report
branch detail does not reconcile with aggregate totals. No aggregate coverage
gain or full-suite coverage claim is made.

The parity matrix remains at 2,287 rows (1,793 decode / inspect / verify and
494 encode); the target-only fault lane now contains 54 rows. The
coverage-origin inventory records 171 exact `cfg(coverage)` guards across 27
Rust files.

### AV1 super-resolution geometry and chroma allocation faults — 2026-10-19

Added three target-only fault-contract rows against the active Pillow-backed
AVIF sequence input
`decode:avif:animated_lossy_inter_420_superres_sgr_8bit_160x56`: decoded-leaf
geometry mismatch, chroma U output-plane reservation failure, and chroma V
output-plane reservation failure. Each verifies the public sequence-decode
error and a clean retry. The injected conditions are compiled only with
`cfg(coverage)` and are recorded with `oracle_status: not_applicable`; they do
not count as Pillow parity. The existing source row and luma/position fault
contracts remain controls.

The selected run passed the AVIF source parity row and all five selected fault
contracts (6/6 total). Its report is
`target/release-evidence/selected/20261019T_av1-superres-fault-contracts-final.json`.
Coverage MCP reports the selected report's source/build identity as unverified
and test attribution as unknown, so the result establishes test execution but
not an aggregate coverage gain. Its focused branch query still lists uncovered
branch coordinates in `upscale_subsampled_leaf`; those coordinates require a
later source-and-measurement-matched investigation before making a coverage
claim.

The parity matrix remains at 2,287 rows (1,793 decode / inspect / verify and
494 encode); its separate target-only fault lane now contains 65 rows. The
coverage-origin inventory records 184 exact `cfg(coverage)` guards across 27
Rust files.

### AVIF clipped mixed-lossless inter B32 parity — 2026-10-20

Added the public Pillow parity case
`decode:avif:animated_mixed_lossless_inter_b32_i420_60x60`. Its 60×60 I420
two-frame animation has a frame-1 inter B32 at the bottom-right, clipped to a
28×28 visible extent. The frame uses both qindex-zero lossless and lossy
segments with a spatial segment map update. The deterministic input is
`tests/fixtures/input/images/avif/animated_mixed_lossless_inter_b32_i420_60x60.avif`
(1,083 bytes; SHA-256
`618e40d92c2dd80005f841b5af4907e8f328512960621c6f1eb7e267c24b23ce`). Pinned
Pillow 12.2.0 produced two 10,800-byte RGB frame references with SHA-256
`a3b4b27575bef66a5eb058529aee5cac545d4d43a9bace07421cea066b4d59b8` and
`6b6f92c82d4962e2f59502cd76f454527f3a213fafa10130620e601812aa918c`.

The selected `make coverage-selected` parity run passed 1/1 for this row. Its
report is
`target/release-evidence/selected/avif-clipped-mixed-inter-b32-20261020.json`
(SHA-256
`c3db40108516a8db2f4c57b3241949671e944bb83da562b5e2812b6df4865824`), and
the local run receipt records `source_changed_during_run: false`. Coverage MCP
reports 3,070 / 32,044 branches covered and groups the largest missing AV1
entropy observations under `validate_complete_lossy_420_partition`,
`decode_inter_transform_size`, and `decode_inter_leaf`. MCP source/build
identity remains unverified and test attribution is unknown, so these are
coordinate-only gaps and do not establish a coverage gain. The incremental
comparison with
`target/release-evidence/coverage-main-full-20261020.json` is incomparable
because exact branch detail in the baseline does not reconcile with its
reported totals. No aggregate coverage gain is claimed.

An already-started selected run of the existing 64×64 companion row also
passed 1/1, but its receipt detected a source change during execution; that
report is excluded from comparisons.

### AVIF per-tile CDEF active-map reservation fault contract — 2026-10-20

`fault-contract:avif:avif_tile_cdef_active_map_reservation_failure` reuses the
active `decode:avif:multitile_color_split_groups` Pillow input. Its
coverage-only hook forces the per-tile CDEF active-map reservation to fail.
The selected target-only contract passed 1/1 with an AVIF `StillDecode`
`Dimensions` error and the exact message
`decode: AVIF AV1 validation failed: unable to allocate AV1 CDEF active map`.
Retry output matches a fresh uninjected decode. The case remains
`oracle_status: not_applicable` and is excluded from Pillow parity counts.

The selected report is
`target/release-evidence/selected/avif-tile-cdef-active-map-reservation-20261020.json`
(SHA-256
`030816355baa3a0fd76fd5d85dd2baf5ab2c14622deb271d773412dc570ef0f9`). Its
local receipt records `source_changed_during_run: false`. Coverage MCP's
branch query at `entropy.rs:11343` returned no missing branch locations for
the active-map hook in this selected report. Source/build identity remains
unverified and test attribution is unknown; no aggregate coverage gain is
claimed.

### TIFF separate-planar YCbCr subsampling error parity — 2026-10-20

Added `decode:tiff:ycbcr_separate_subsampling_unsupported_pair_lzw_1x1` using
the complete 253-byte LZW TIFF
`tests/fixtures/input/images/tiff/ycbcr_separate_subsampling_unsupported_pair_lzw_1x1.tiff`
(SHA-256
`7c8a656e6d7a72c19a284b19260f5bf021f406526a17606f4f36bfaa96426905`). The
fixture changes only tag 530 from `(1, 1)` to `(2, 2)` in a separate-planar
file. Pinned Pillow 12.2.0 opens and verifies the image, then reports
`builtins.OSError: decoder error -2` when pixels are loaded. The selected
public parity case passed 1/1 and matched the expected TIFF `malformed` error
contract.

The selected report is
`target/release-evidence/selected/tiff-separate-ycbcr-unsupported-pair-20261020.json`
(SHA-256
`75d3297b5ac77da0568eb9c3cf0fce33943776ae3990e0b450e50e5a4ef57860`). Its
local receipt records `source_changed_during_run: false`. Coverage MCP queried
`src/codecs/tiff/decode.rs:317`; its report remains source-unverified and does
not support aggregate or regression claims. The exact branch comparison with
`target/release-evidence/coverage-main-full-20261020.json` is incomparable
because the saved baseline's detailed branch observations do not reconcile
with its reported totals.

### Token-aware WebP method-0 run-cap parity — 2026-10-20

Added `encode:webp:enc_lossless_method_0_run_cap_token` using the existing
4,356×1 PNG run-cap source
`tests/fixtures/input/images/png/webp_lossless_run_cap_4356x1.png` (SHA-256
`038d4a47bb2b7f22b62eaac823a897c74607b20ee0ab6999e30b5dc84f6a1a93`). The
new row calls `encode_with_token` with a fresh non-cancelled token on the
4,096-pixel repeated run, exercises cancellation checkpoints, and matches the
same 186-byte Pillow output as the existing non-token row. The selected case
passed 1/1 and re-decoded to the expected 4,356×1 RGB pixels.

The selected report is
`target/release-evidence/selected/webp-lossless-run-cap-token-20261020.json`
(SHA-256
`5915498715e1c2e1e7271885fdd02b4dcce0eec2c963db1538c0bf2b067fc490`). Its
local receipt records `source_changed_during_run: false`. Coverage MCP's
query at `backward_refs.rs:241` returned no missing branch locations for the
token checkpoint call, while the run-check and cap conditions at lines 240
and 244 still have candidate gaps in other monomorphizations. MCP source/build
identity remains unverified and test attribution is unknown; no aggregate
coverage gain is claimed.

The same selected report for the existing WebP run-cap case and three GIF
median-cut parity rows passed 4/4. Their exact Pillow outputs were unchanged.
Coverage MCP still reports an unobserved `split > 0` outcome in GIF's median
tie fallback; the valid median-cut rows keep each candidate box nonempty, so
that remaining state is not represented as Pillow behavior.

The current fixture matrix contains 2,619 total rows: 2,091 decode / inspect /
verify rows and 528 encode rows. Of those, 2,091 decode rows and 497 encode
rows are active; 31 planned encode rows remain. The separate target-only fault
lane contains 67 rows. The coverage-origin inventory records 213 exact
`cfg(coverage)` guards across 28 Rust files.

## JPEG baseline grayscale nonzero SOS spectral-start parity — 2026-10-26

Added `decode:jpeg:grayscale_sampling_420_17x9_sos_ss_nonzero`. Its maintained
generator starts with the deterministic 17×9 grayscale 4:2:0 input and changes
only the baseline SOS `Ss` byte from zero to one. The complete 468-byte input
has SHA-256
`52dbda00c3f20563251d3db1f4ec77debe55b90fc8838dce9bfee05e5730e5d0`.
Pinned Pillow 12.2.0 accepts the input and returns the same 17×9 `L` pixels as
the zero-`Ss` source: 153 bytes with SHA-256
`6cb8bf5ca5f3ecb5e63af750426a1af7823ef95743d030651deb9e7b9366728c`. The
selected public parity case passed 1/1.

The selected report is
`.coverage-mcp/local-evidence/selected/jpeg-gray-spectral-start-20261026/coverage.json`
(SHA-256
`eeb0c61f1fe34f8299909c756dc70050cce65ed3435ee9faf99f4b364852ac8f`). The
active LLVM function instance records four true observations and zero false
observations for `scan.ss != 0` at `src/codecs/jpeg/decode/decode.rs:2676`; the
full baseline had zero true observations. Coverage MCP lists duplicate
zero-execution function instances for this location, and its incremental
comparison is incomparable because baseline branch details do not reconcile
with its totals. MCP source identity is unverified and test attribution is
unknown, so no aggregate coverage gain is claimed.

## JPEG grayscale SOS spectral-end and approximation parity — 2026-10-26

Added three public Pillow parity rows for grayscale 4:2:0 JPEGs derived from
the deterministic 17×9 source: `Se=62`, `Ah=1/Al=0`, and `Ah=0/Al=1`. The
generator changes only the corresponding SOS byte. Pinned Pillow accepts all
three inputs and returns the same 153-byte `L` pixel output as the source
(SHA-256
`6cb8bf5ca5f3ecb5e63af750426a1af7823ef95743d030651deb9e7b9366728c`). The
three new public parity cases passed 3/3; the combined selection including the
earlier nonzero-`Ss` row passed 4/4.

The selected LLVM report is
`.coverage-mcp/local-evidence/selected/jpeg-sos-boundary-fields-merged-main-20261026/coverage.json`
(SHA-256
`2d10a141a5273a83ff5f179e9a21709f148e373dfcd1d912f5a8477900573084`). Its
active grayscale direct-path function instance recorded true outcomes for
`scan.ss != 0` (4 true, 9 false), `scan.se != 63` (3 true, 6 false),
`scan.ah != 0` (3 true, 3 false), and `scan.al != 0` (3 true, 0 false) at
`decode.rs:2676–2679`. These are selected-case counters, not full-suite totals.
The earlier full LLVM report recorded 0 true / 74 false for each guard.

Coverage MCP marks this report's source unverified and test attribution
unknown. Its incremental comparison with the full baseline is incomparable
because the baseline branch details do not reconcile with reported totals; no
aggregate coverage gain is claimed. No Rust source or unit tests were added.

## JPEG baseline grayscale empty-entropy parity — 2026-10-26

Added `decode:jpeg:grayscale_sampling_420_17x9_empty_entropy`. Its maintained
generator starts from the deterministic 17×9 grayscale 4:2:0 JPEG and removes
only the entropy payload, leaving a complete SOS header followed immediately
by EOI. The generator is now part of `make verify`, which checks the full
fixture family for deterministic regeneration. The complete 330-byte input
has SHA-256
`142f0ed4c9dc5dc0210a866d14702644f8dc5ae756b9bd4442eba518a8089afc`. Pinned
Pillow 12.2.0 accepts it and returns 153 `L` bytes with SHA-256
`563114a366c030ebb8171aa3a92559ddbb650cebcee185eaa10c9bae576c44c5`. The
selected public parity case passed 1/1.

The selected LLVM report is
`.coverage-mcp/local-evidence/selected/jpeg-gray-empty-entropy-20261026/coverage.json`
(SHA-256
`da0b989df51a88089ce8dff61e90300ac6ec6da304afb9f5cdd4c85fbbd77609`). The
active grayscale direct-path function recorded four true observations and no
false observations for `scan.entropy_start == scan.entropy_end` at
`src/codecs/jpeg/decode/decode.rs:2681`. This confirms the edge input reaches
the empty-entropy guard. Coverage MCP reports the opposite arm as missing in
this one-row selection; the existing all-JPEG selected run recorded 60 false
observations. MCP source identity remains unverified and test attribution is
unknown, so no aggregate coverage gain is claimed.

## JPEG trailing DHT length-one parity — 2026-10-26

Added `decode:jpeg:baseline_444_single_scan_trailing_dht_length_one`, a
complete JPEG whose trailing DHT marker declares the invalid one-byte segment
length. The deterministic generator replaces the anchored trailing COM in the
8×8 baseline image. Pillow 12.2.0 verifies the file structurally and reports
`OSError: broken data stream when reading image file` during pixel decoding;
the selected Rust parity row passes with the same malformed result. The input
SHA-256 is
`ce200aef72bb9a259f48cf9f154a51ca8ba16280601b39c7076c8c4c6df07b0a`.

A clean full `make coverage` run on `origin/main` at
`0d72fbda5757d1ff878bfeaa9d9d56d62d3110c3` passed all executed tests and
reported 67.2685% lines, 58.0959% branches, 58.5237% functions, and 66.1997%
regions. Its report is
`.coverage-mcp/local-evidence/full-20261026-main/coverage.json` (SHA-256
`552996f85421077b702606d73e00d71ef2943b724c5f68a665c27306bb312931`). The
full report had no true outcome for the `segment_length < 2` branch at
`src/codecs/jpeg/decode/decode.rs:2900`. The exact selected run passed 1/1 and
its receipt records three true outcomes for that branch, with no source or
fixture change during execution. Its report is
`.coverage-mcp/local-evidence/selected/20261025T213051.396026Z-4fd74cd709074907b76d0e5e0dfb41ca/coverage.json`
(SHA-256
`97f41f835b3b1d5030c96f032235c4c3e7f465cf5084ecf3b90a13d13d237ccb`).

Coverage MCP could not provide a grouped or incremental result: its shared
daemon did not become healthy at `127.0.0.1:59471`. The retained LLVM report
and local receipt support this one source-bound case observation; no MCP
snapshot or aggregate coverage gain is claimed. No Rust source or unit tests
were added.

## WebP two-node Huffman color-cache lookahead parity — 2026-10-26

Added `decode:webp:lossless_vp8l_two_node_cache_peek`. Its maintained
generator writes a 4×1 RGB VP8L stream with a one-bit color cache and a dynamic
green tree containing only the one-bit symbols 280 and 281. The code sequence
280, 280, 280, 281 makes lookahead select both leaves in
`HuffmanTree::peek_symbol`. The 66-byte input has SHA-256
`388dc731d98c2f58a7e0842ee1efaeff168047f6a336539a29fbbc8e22001f19`. Pillow
12.2.0 returns twelve black RGB bytes with SHA-256
`15ec7bf0b50732b49f8228e07d24365338f9e3ab994b00af08e5a3bffe55fd8b`; the
selected public parity row passes. The independent VP8L parser reports a
one-bit cache, four lookups, and both cache indices.

The selected LLVM report is
`.coverage-mcp/local-evidence/selected/20261026T080421.416756Z-7e5ea112674c4bd4af32ee11130c7622/coverage.json`
(SHA-256
`5b76ebfcaf47b835f62b7014119afb8fbb77d1e5f4f40bb168f2b780c3cdab60`). Its
source-hashed local receipt records three observations for each branch outcome
at `src/codecs/webp/native/huffman.rs:505`; the preceding full WebP selection
recorded zero for both outcomes. The report is selected-case evidence rather
than a Coverage MCP snapshot or an aggregate coverage claim. No Rust runtime
code or unit tests were added.

## AV1 lossy intra I420 super-resolution parity — 2026-10-26

Added the public Pillow parity row
decode:avif:superres_actual_upscaled_i420_no_restoration_17x17. It retains
the existing 17x17 full-range 8-bit I420 input shape and denominator-9
super-resolution, with AV1 restoration disabled. The pinned fixture generator
checks both byte-identical encodes, AV1 geometry, absent restoration on all
three planes, and Pillow 12.2.0 RGB pixels. The input SHA-256 is
175f4eed6919458cd0f3af455f20e6a0be83bfd059b2e63e745ad8fab3e46c35; the
867-byte RGB reference SHA-256 is
36b2cbf4958b72a92a936b63d8b905bfd49dfb49b18511cc01723e880576b57b. The
selected public parity case passed 1/1.

Coverage MCP previously identified the unobserved true arm at
src/codecs/avif/av1/entropy.rs:10614 in the full report for source revision
3d918164137833338d5272430b17e224ceb4ea40. The exact selected LLVM report
records 3 true and 0 false observations for that arm; the full baseline
recorded 0 true and 166,368 false observations. The selected report is
.coverage-mcp/local-evidence/selected/avif-superres-i420-intra-no-restoration-20261026/coverage.json
(SHA-256
8ab75d7737b8becd18f26c4179e84ea225889e508155f608e8e39400230e8bf4).
Its local receipt records unchanged source and fixtures during execution.
This establishes the selected branch hit, not a refreshed full-suite
percentage or a Coverage MCP incremental snapshot. No Rust source or unit
tests were added.

## AV1 lossless qmatrix sequence parity — 2026-10-26

Added active row
`decode:avif:animated_lossless_inter_420_qmatrix_level15_qindex0_16x64`. The
maintained generator reproduces a two-frame 16x64 8-bit I420 AVIF sequence
with pinned libavif 1.4.1/libaom 3.13.2, qindex zero, qmatrix level 15 on all
planes, and all-lossless key/inter frames. The 6,144-byte input SHA-256 is
`b76924ec8cf9fa01ad27e48dec5990332b089d75460fa7bf37d9fa00108100de`; Pillow
12.2.0 reference frame hashes are
`44c3002ad58c2b063ac9c560c8baf279052e13d007eaeb84fef33c77a81b9d39` and
`e121e6212ae783e1d2a9c83c99f0fe931df1682b045e772cce7f60e693b3f7e5`, each
with a 100 ms duration.

The focused selected parity run passed the qmatrix case, the no-restoration
super-resolution case, three existing all-lossless inter controls, and three
existing mixed-lossless controls (8/8). It retains exact RGB output, including
3,072 bytes for the qmatrix sequence. The narrow decoder admission change
threads the parser-proven all-lossless frame flag into lossless inter
reconstruction; ordinary and mixed-segment paths keep matrix admission
disabled. Strict native Clippy and both x86-64 SSE2/AVX2 cross-Clippy lanes
pass.

The selected LLVM report is
`.coverage-mcp/local-evidence/selected-worktree/qmatrix-superres-20261026/coverage.json`
(SHA-256
`592a535961454465120de20c6230a95cd0d71454b521a291b5103fc58b8b8990`). Its
local receipt records all eight selected cases passing and unchanged source
and fixtures during execution. Coverage MCP read the selected report and
reported 4,452/45,193 branch observations for the selected binary; a gap query
at `src/codecs/avif/av1/block.rs:49247` shows unobserved generic
monomorphizations in this selected run. Source identity is unverified and test
attribution is unknown. Its incremental comparison to the older full baseline
timed out, so no aggregate coverage gain is claimed. No unit tests were added.

## AVIF lossy I422 split-transform neighbor context parity — 2026-10-26

Added active row
`decode:avif:animated_lossy_inter_422_tx_context_checker_b64x32`. The
maintained generator verifies the 64x32 two-frame lossy 4:2:2 input, pinned
Pillow 12.2.0 RGB frame hashes, and reproducible AVIF bytes. The input
SHA-256 is
`38c94eed681628437dd11423d62573fee3b1325a749ffc9f74dda1301c6acd56`; the
frame hashes are
`3268f29921f0c622303b8fcf16c10c8bcd25500338944422ea4c18ced0ae821e` and
`aac1dfe4688ff506cb0b3eca55ae4094f8bee4b12100c1b2d7c2b962cf90aa1f`. A
pinned native libavif observation supplies the sequence-loop evidence.

The decoder now records coded block dimensions for an inter neighbor's
`tx_intra` context while preserving the per-cell split-transform plan. The
selected public parity run included the existing 32x32 I422 control and passed
2/2 exact RGB rows; the full AVIF decode matrix passed 605/605. Strict
formatting, workspace Clippy, and the x86-64 SSE2 and AVX2 cross-Clippy lanes
passed.

The selected LLVM report is
`.coverage-mcp/local-evidence/selected/20261026T120515.229922Z-c65a47bf1a7d4389bb5433ac3fa13f53/coverage.json`
(SHA-256
`05482db667104e09730b44bf5cb0192e6bbb96165cbb0b0b4b94e3f35ba074c2`).
Coverage MCP measured 4,066/32,134 branch observations for this selected
binary and still reports uncovered branches in `TileState::commit_with_metadata`.
Source identity is unverified and named-test attribution is unknown, so this is
targeting evidence rather than an aggregate coverage claim. No unit tests were
added.

## AVIF lossy I422 B32x64 vertical-split parity — 2026-10-26

Added active public row
`decode:avif:animated_lossy_inter_422_vertical_split_b32x64`. The maintained
generator checks a deterministic two-frame 32x64 lossy 4:2:2 AVIF, pinned
Pillow 12.2.0 RGB output, and reproducible encoded bytes. The input SHA-256 is
`0eb45159064b3f1c30919d0062afa19056415f76be203278cd3ddd7174e8a798`; the
6,144-byte RGB frame hashes are
`f1aab4fdf740e5726556337b0287ec940079cc48a4f1995d5f2b8fb5fbc9348c` and
`c72b8082dac958d7aeda2637782f8226e7238711a7e967959100d25ad270e39d`. The
independent pinned libavif record observes both frames and infinite repetition.

The selected public parity and LLVM coverage run passed 1/1 and matched all
6,144 RGB bytes. Its report is
`.coverage-mcp/local-evidence/selected/avif-i422-vertical-split-final-2-20261026/coverage.json`
(SHA-256
`43b475948a8c5558b479d8a1a1b86e711cecadd194a5d2d81389df16f34f9f90`). Raw
branch counters at `src/codecs/avif/av1/block.rs:48348` recorded true-arm counts
of 2 and 6 in two selected specializations. The full run still has unobserved
arms in other specializations at that source span, so the template-wide gap
remains open.

A refreshed `make coverage` run passed its executed suites, including 57/57
coverage-matrix tests, 69/69 feature-gate tests, and the AVIF loop-evidence
integrity test. Its LLVM report is
`.coverage-mcp/local-evidence/full-20261026-avif-i422-vertical-split/coverage.json`
(SHA-256
`cd94d83e8f342f831f88b92523bc6329862e02f423581940b82d1678b7b6150e`). Totals
are 93,661/139,561 lines (67.1112%), 18,558/32,056 branches (57.8924%),
5,220/8,928 functions (58.4677%), and 143,043/216,607 regions (66.0380%).
Coverage MCP reports source identity unverified and test attribution unknown;
the full-report comparison is limited and the selected incremental comparison
is incomparable because baseline detail is incomplete. No aggregate coverage
gain is claimed. No production Rust code or unit tests were added.

## AVIF high-depth monochrome inter parity — 2026-10-27

Added the active public parity row
`decode:avif:animated_lossless_inter_monochrome_b32x32_10bit`. Its pinned
10-bit all-lossless key/inter sequence uses a full-size B32x32 monochrome leaf.
The 1,114-byte input SHA-256 is
`28427120f6a0e9de71d0abcfe36bbeaae4d701dacefb62f852dfd71eeb82d624`; Pillow
12.2.0 RGB frame hashes are
`9eb4dc1f2ea4eb4cd5bc4dca1bc035a1eeee1d08248a527348e3be8cb5d25129` and
`c40bb457a52128f753525937c909cd08ebd8f96d7a5431a4e9751392dc5b487f`. The
fixture generator checks deterministic bytes, monochrome AV1 sequence headers,
and exact Pillow frame pixels. The complete AVIF loop-witness parity test also
passed with the new two-frame case registered.

The focused `make coverage-selected` row passed 1/1. Its report is
`.coverage-mcp/local-evidence/selected/avif-hi10-monochrome-20261027/coverage.json`
(SHA-256
`c4bd568056ec0057874090638622796c26ed583d601551307bb18ac0f91263a7`). Raw
LLVM branch counters record one false outcome at
`src/codecs/avif/av1/entropy.rs:10787`, where the older full report recorded
zero. Coverage MCP still lists the span because the selected report also has a
zero-count branch record at that same coordinate. Both reports have unverified
source identity, and the incremental comparison is incomparable because the
baseline detail is incomplete; no aggregate coverage gain is claimed. No
production Rust code or unit tests were added.

At the monochrome parity row, the fixture matrix contained 2,641 total rows:
2,113 decode / inspect / verify rows and 528 encode rows. Of those, 2,113
decode rows and 497 encode rows were active; 31 planned encode rows remained.

## AVIF I422 compound-reference parity — 2026-10-27

Added the active public parity row
`decode:avif:animated_lossy_inter_422_compound_refs_b64x32`. Its deterministic
three-frame 64x32 8-bit I422 sequence uses compound prediction from both prior
references on the final inter frame. The pinned generator verifies repeated
encoding, AV1 4:2:2 headers, the selectable reference list, and exact Pillow
12.2.0 RGB frames. The 5,565-byte input SHA-256 is
`58179bf4eec6cd6a78dc575646dc3db0fa30b0d8c844f74a1dbf02694c785350`; frame
hashes are `aff58d48982f1c9f12bc4e1161fed09f6128f2f786b5d2c43c9f50f0fa0ec7b9`,
`b971221eb15826e56645181490b7db5eb89625680368ea7d9453809714e30ecb`, and
`df257674ce899a4c3fba6125261fd00827a9d1b551d4f8e1c55cc86f34b34491`. The
independent pinned libavif observation decodes all three frames and reports
infinite repetition.

The selected public parity and branch-coverage run passed 1/1, matching all
6,144 RGB bytes per frame. Its LLVM report is
`.coverage-mcp/local-evidence/selected/avif-i422-compound-20261027/coverage.json`
(SHA-256
`e5abd916ac03cf71c3656647b300abfa1b915b71c5ffcc3e505fdc13392e0bea`). Raw
branch counters at `src/codecs/avif/av1/block.rs:48348` record eight true-arm
hits in the `decode_inter_compound_translation` specialization; the saved full
baseline had zero true-arm hits there. This is branch-specific selected evidence.
Coverage MCP marks source identity unverified and test attribution unknown; its
incremental comparison is incomparable because baseline detail is incomplete.
No aggregate coverage gain is claimed. No production Rust code or unit tests
were added.

The current fixture matrix contains 2,653 total rows: 2,117 decode / inspect /
verify rows and 536 encode rows. Of those, 2,117 decode rows and 505 encode
rows are active; 31 planned encode rows remain.

## WebP alpha filter no-win parity — 2026-10-27

Added active encode parity row `encode:webp:enc_lossy_rgba_alpha_filter_no_win`.
Its deterministic 512x512 RGBA PNG uses a seeded 32-color alpha palette whose
sampled residuals favor horizontal filtering while the full-plane encoder
keeps the unfiltered alpha representation. The generator checks the PNG
bytes; the exact Pillow 12.2.0 encoded reference is 157,078 bytes. The input
SHA-256 is `343008603074288cbdad755679e6e6f0c78e9cb8b96b73cffcb7b316f7229210`,
and the encoded reference SHA-256 is
`715914879a0bcc255e9e736fbe69ce3f395be11bf7d9685cec7406079b7f5bac`.

The selected public parity run passed 1/1 and matched the decoded 512x512 RGBA
pixels. Its report is
`.coverage-mcp/local-evidence/selected/webp-alpha-filter-no-win-512-20261027/coverage.json`
(SHA-256
`3b123e8d36d2b00410ede33334638ec523e0e00a6d8be9520fa5076c7e1065ef`). Raw
LLVM branch counters at `src/codecs/webp/native/encoder.rs:3082` record 0 true
and 3 false outcomes for this row, reaching the full-plane no-win arm.
Coverage MCP returned `I/O error: No such file or directory` for this report
path, so this is local selected-run evidence without an MCP source/build
receipt.

The refreshed full `make coverage` run passed 57/57 coverage-matrix tests,
7/7 decode-policy tests, 1/1 determinism test, and 69/69 feature-gate tests.
Its retained report is
`.coverage-mcp/local-evidence/full-20261027-webp-alpha-filter-no-win/coverage.json`
(SHA-256
`33574fd55eee1fca407017c16864582f83583eb6e5327de32078e17cfda1982e`). Totals
are 93,818/139,562 lines (67.2232%), 18,627/32,042 branches (58.1331%),
5,224/8,928 functions (58.5125%), and 143,268/216,612 regions (66.1404%). At
line 3082, the full report records 23 true and 3 false outcomes. Relative to
the preceding retained full report with the same denominators, the local LLVM
totals add one covered line, branch, and region. Coverage MCP has not verified
that incremental comparison or source/build identity. All four repository
floors pass. No Rust implementation change or unit test was added.

## WebP VP8L box-chain max-match parity — 2026-10-27

Added active parity rows `encode:webp:enc_lossless_method_6_box_chain_max_match`
and `encode:webp:enc_lossless_method_6_box_chain_max_match_token`. Their pinned
PNG source contains three rows of six colors arranged so palette packing gives
the box-chain search a match longer than 4,095 packed pixels after a shorter
hash-chain match. The source SHA-256 is
`c07464a1965f2fd029193f05cb0f1da90740d046570b30a66bd86dd2d2ea5df3`; its RGB
pixels hash to
`809865969461d792ec552fbe8040f3f59abc39fa4f4da222d3b715d058f2dfe6`. Pillow
12.2.0 produces the same 76-byte encoded reference for both API paths, SHA-256
`d012caba3e51c61643abd86874b13d55c5a178de871c5469896f01bab7bc46c2`.

The selected public parity run passed 2/2 and re-decoded exact 8,196x3 RGB
pixels. Its LLVM report is
`.coverage-mcp/local-evidence/selected/webp-box-chain-max-match-3row-20261027/coverage.json`
(SHA-256
`75812713bbe7aaf9fac5e426cf8d037c620837a739d780a0af6fedca0fd62a60`). The
selected report records both outcomes for the token-aware and ordinary search
guards in `src/codecs/webp/native/encoder/backward_refs.rs`: line 655 has 4 true
and 28,677 false outcomes, and line 695 has 24 true and 172,062 false outcomes.
Coverage MCP returned `I/O error: No such file or directory` for this report,
so source/build identity and incremental attribution remain unverified. This is
local selected-case evidence; it does not establish an aggregate coverage gain.
No Rust implementation change or unit test was added.

The refreshed full `make coverage` run passed 57/57 integration tests,
7/7 decode-policy tests, 1/1 determinism test, and 69/69 feature-gate tests.
Its report is
`.coverage-mcp/local-evidence/full-20261027-webp-box-chain-max-match/coverage.json`
(SHA-256
`1dbec9f10af5d98aa82689d1af36930c2cc3ae62066689503fdd2d790b0eb500`). Totals
are 93,818/139,562 lines (67.2232%), 18,629/32,042 branches (58.1393%),
5,224/8,928 functions (58.5125%), and 143,268/216,612 regions (66.1404%). The
full report records 4 true / 53,426 false outcomes at line 655 and 24 true /
4,396,437 false outcomes at line 695. Against the preceding full report with
the same denominators, two additional branches are covered; the line, function,
and region totals are unchanged. All four repository floors pass. Coverage MCP
also returned `I/O error: No such file or directory` for this full report, so
the new totals remain local LLVM evidence without MCP source/build attribution.

The current matrix contains 2,649 rows: 2,117 active decode rows, 501 active
encode rows, and 31 planned encode rows.

## WebP token-aware input preparation parity — 2026-10-27

Added active parity rows `encode:webp:enc_lossless_bilevel_token`,
`encode:webp:enc_lossless_luminance_alpha_partial_token`, and
`encode:webp:enc_lossy_opaque_rgba_token`, reusing the pinned Pillow source
assets `1bit.png`, `gray_alpha_partial.png`, and `rgba.png`. The exact
Pillow 12.2.0 encoded references were generated; all three selected public
parity rows passed and re-decoded their expected pixels.

The selected LLVM report is
`.coverage-mcp/local-evidence/selected/webp-token-adapter-branches-20261027/coverage.json`
(SHA-256
`7e9e27eb60d88568a069c048b70c0c7dd11b3a00c7aa98e4fabb578daeeb398f`). It
records the token arms in `src/codecs/webp/encode/mod.rs`: line 533 has 1 true
and 3 false outcomes, line 620 has 1 true and 3 false, line 731 has 1 true and
3 false, and line 738 has 1 true outcome for partial-alpha expansion. Coverage
MCP returned `I/O error: No such file or directory` for this report, so these
remain local selected-case observations without MCP source/build attribution.
No codec implementation change or unit test was added; no aggregate coverage
gain is claimed from this selected run.

The current matrix contains 2,652 rows: 2,117 active decode rows, 504 active
encode rows, and 31 planned encode rows.

## WebP token-aware CMYK preparation parity — 2026-10-27

Added active row `encode:webp:enc_lossless_cmyk_token` using the existing
Pillow CMYK source `baseline_cmyk.jpg`. Its pinned Pillow 12.2.0 output matches
the ordinary lossless CMYK case exactly: 16,780 encoded bytes with SHA-256
`6f7509153181e2346a376b97c4b15e00ed5fe1fd0b19f74d25dcf93c5fe3ead1`.

The selected public parity run passed 1/1 and re-decoded 128x128 RGB pixels.
Its LLVM report is
`.coverage-mcp/local-evidence/selected/webp-cmyk-token-preparation-20261027/coverage.json`
(SHA-256
`4341b9d2378483979a675bbd833fdaa0f811ff9f85ea3264fe2e916184dc1829`). It
records the token arm in `src/codecs/webp/encode/mod.rs:1090` (1 true and 3
false outcomes). Coverage MCP still cannot access reports in this managed
worktree; local selected evidence has no MCP source/build attribution. No
codec implementation change or unit test was added, and no aggregate coverage
gain is claimed from this selected run.

The current matrix contains 2,653 rows: 2,117 active decode rows, 505 active
encode rows, and 31 planned encode rows.

The refreshed full `make coverage` run passed 57/57 integration tests,
7/7 decode-policy tests, 1/1 determinism test, and 69/69 feature-gate tests.
Its report is
`.coverage-mcp/local-evidence/full-20261027-webp-token-adapters/coverage.json`
(SHA-256
`14c849230bf7f07e8403ed3ce5f3920ec7cec658a2fdb8d1547bc9a2a94f80cc`). Totals
are 93,858/139,562 lines (67.2518%), 18,633/32,042 branches (58.1518%),
5,224/8,928 functions (58.5125%), and 143,363/216,612 regions (66.1842%). The
preceding full report has the same denominators; the four token parity cases
add 40 covered lines, 4 branches, and 95 regions. All four repository floors
pass. Coverage MCP still returns `I/O error: No such file or directory` for
reports under this managed worktree, so the refreshed full totals are local
LLVM evidence without MCP source/build attribution.

## WebP encoder dimension boundary parity — 2026-10-27

Added five active parity rows at the WebP encoder's maximum side dimension,
using deterministic one-row RGB PNGs. Both lossless and lossy Pillow 12.2.0
accept width 16,383 and reject 16,384; lossless also rejects 16,385. The Rust
encoder had accepted width 16,384 in both modes. A shared maximum of 16,383
now guards the lossless and lossy paths. The accepted output matches Pillow at
44 bytes for lossless (SHA-256
`8a4796055658062fb87856f6d048f1cf9b3944961b9e5b8e1d58357abcf1263f`) and
542 bytes for lossy (SHA-256
`e67238025de2835593afe703635fa8c58a220e7818affee2ce8a309b1a37608b`).

The final-source selected parity run passed 5/5 with no panic. Its LLVM report
is `.coverage-mcp/local-evidence/selected/webp-dimension-boundaries-final2-20261027/coverage.json`
(SHA-256
`4fef195cc7a038f679ac5013c39ee2d23326c4bf1b1a2f8db18db9a97ec8fe06`). The
lossless check at `src/codecs/webp/native/encoder.rs:2765` records 2 true and
3 false selected outcomes; the lossy check at
`src/codecs/webp/encode/vp8/encoder.rs:117` records 1 true and 3 false.

The final full `make coverage` run passed 57/57 coverage-matrix tests,
7/7 decode-policy tests, 1/1 determinism test, and 69/69 feature-gate tests.
Its LLVM report is
`.coverage-mcp/local-evidence/full-20261027-webp-dimension-boundaries-final/coverage.json`
(SHA-256
`0ccc09fbdd03827e4afa312b7373e8558dc5618a4528fe472d235b463fd65025`). Totals
are 93,875/139,578 lines (67.2563%), 18,639/32,050 branches (58.1560%),
5,225/8,929 functions (58.5172%), and 143,380/216,629 regions (66.1869%).
All four repository floors pass. Coverage MCP previously returned
`I/O error: No such file or directory` for a report under this managed
worktree, so these totals are local LLVM evidence without MCP source/build
attribution.

The current fixture matrix contains 2,673 total rows: 2,120 decode / inspect /
verify rows and 553 encode rows. Of those, 2,120 decode rows and 522 encode
rows are active; 31 planned encode rows remain.

## WebP lossless EXIF output-reuse parity — 2026-10-27

Added active row `encode:webp:enc_lossless_exif_metadata_reuse` using the
existing 1x1 PNG and EXIF bytes. The 74-byte output matches Pillow 12.2.0
(SHA-256
`10418c63af7178aff19eb7b9f6ebe473621543f4e3dadee3e839b3747814cd35`). The
selected public parity run passed 1/1, including sink output parity and exact
re-decode. Its LLVM report is
`.coverage-mcp/local-evidence/selected/webp-lossless-exif-reuse-20261027/coverage.json`
(SHA-256
`53c006d7abb0a28366c24fd8ef38b45a77ffc100275dbf3cea4bcc618cf9069b`). It
reaches the VP8L base-container fallback in
`src/codecs/webp/encode/mod.rs:919`; the full report now records both sides of
that `VP8X` check (3 true, 16 false).

The refreshed full `make coverage` run passed 57/57 coverage-matrix tests,
7/7 decode-policy tests, 1/1 determinism test, and 69/69 feature-gate tests.
Its report is
`.coverage-mcp/local-evidence/full-20261027-webp-metadata-reuse/coverage.json`
(SHA-256
`2931c9883d1de559c9fea7a00bd32fddf837d7a2f4bd159a2664a2d82563016d`). All
four repository floors pass. Coverage MCP has returned
`I/O error: No such file or directory` for worktree-local reports, so this is
local LLVM evidence without MCP source/build attribution.

## WebP VP8 plane-padding helper cleanup — 2026-10-27

Local LLVM coverage showed an unobserved `height == padded_height` outcome in
`pad_plane`. Its only caller, `pad_plane_with_token`, returns the input plane
when both dimensions are already padded and calls `pad_plane` only otherwise.
Renamed the helper to `pad_plane_unaligned` and removed that redundant guard;
the aligned direct-return path remains in the caller. The existing aligned
32x32 and unaligned 17x17 Pillow parity cases passed 2/2, including exact
encoded-byte parity, re-decode, and sink checks. Their selected LLVM report is
`.coverage-mcp/local-evidence/selected/webp-plane-padding-helper-final-20261027/coverage.json`
(SHA-256
`d4616e5f21d26ffdf915317e561643c5a783b48e0f2360f4de1e01b462b0b740`); its
local receipt says the source did not change during the run.

The refreshed full `make coverage` run passed 57/57 coverage-matrix tests,
7/7 decode-policy tests, 1/1 determinism test, and 69/69 feature-gate tests.
All four repository floors pass. The report is
`.coverage-mcp/local-evidence/full-20261027-webp-pad-plane-cleanup/coverage.json`
(SHA-256
`6f5d837cedc8937e160500758b142bde50e3c2e12f6685b94aa7d52be24355cf`). Totals
are 93,879/139,581 lines (67.2577%), 18,636/32,046 branches (58.1539%),
5,225/8,929 functions (58.5172%), and 143,377/216,624 regions (66.1870%).
Coverage MCP still cannot read reports under this managed worktree, so these
are local LLVM measurements without MCP source/build attribution. After the
source change, `make fmt`, `make lint`, and `make lint-x86-simd` passed;
`make test-feature-matrix` passed every native, wasm32-wasip1, and
wasm32-unknown feature lane, and the generated capability tables agreed.

An exploratory exact-parity row for quality 80/method 4 on the same 32x32
source initially exposed a VP8 encoder difference: Rust produced 76 bytes
while Pillow produced 80 bytes, although both decode to the same RGB pixel
SHA-256 (`0c1680535c7baa47105185e3f379d36376fd4b3be62cc9e76aceb696829f2d2d`).
That probe was not retained. An exhaustive public-API sweep over quality 0
through 100 and method 0 through 6 found no parameter pair matching Pillow's
output. The follow-up below fixed the mode decision and added two active
parity rows.

## WebP quality-80 method-4 flat-block parity — 2026-10-27

The mismatch came from libwebp's `PickBestIntra16` scoring rule: for a uniform
source macroblock, it doubles distortion and spectral distortion while the
quantized luma AC coefficients remain flat, then stops applying the rule once
a candidate is no longer flat. The Rust encoder omitted that ordered update
and could select Intra16 where Pillow/libwebp selected Intra4. The Rust path
now applies the method-3-or-higher rate-distortion rule in candidate order.

Added active Pillow rows
`encode:webp:enc_lossy_quality_80_method_4_uniform_rgb_32x32` and
`encode:webp:enc_lossy_quality_80_method_4_nonuniform_rgb_17x17`. The uniform
32x32 output is 80 bytes (SHA-256
`d5b33a6a4452d87928c7956be970a09f66bf33b024f91df67b61222017617f3d`); the
nonuniform 17x17 edge-padded output is 380 bytes (SHA-256
`9c01cd551254909ec5028d6470d2f83af7ab936fc0146c9b9738432d40f6df10`). Both
selected public parity rows passed, including exact reference bytes and
re-decode pixel parity: 2/2 rows, 8 encode calls, no failures or panics. The
selected LLVM report is
`.coverage-mcp/local-evidence/selected/webp-flat-intra16-q80-m4-final-20261027/coverage.json`
(SHA-256
`441c71287eb33384d45ac980f60ab81306037d03c68dff1672e878aabbf595ec`).
Coverage MCP still returns `I/O error: No such file or directory` for reports
inside this managed worktree, so the report is local LLVM evidence without
MCP source/build attribution.

## WebP shifted-row lossless cost-manager parity — 2026-10-27

Added active parity rows
`encode:webp:enc_lossless_cost_manager_shifted_rows` and
`encode:webp:enc_lossless_cost_manager_shifted_rows_token`. The deterministic
2,048x512 RGB input is generated by
`scripts/generate_webp_cost_manager_shifted_rows_fixture.py`; its PNG SHA-256
is `c190d493dc2f746a76d1b73be5aa7232ab2fddf4fbd496d54fc7691cc93a303f`.
Both Pillow 12.2.0 outputs are 5,336 bytes with SHA-256
`16cbe243b738c77fd6f21ac77157fbb1de47ce129d36d4bec7ef304f40c32240`.

The selected public parity run passed 2/2 rows, with 9 encode calls and no
panics. The selected LLVM report is
`.coverage-mcp/local-evidence/selected/webp-cost-manager-shifted-rows-20261027/coverage.json`
(SHA-256
`ad1166369ea05545d32d9ff20704b0688495712d57bdfa0d0061d3a2f56eb285`). A
local branch-coordinate comparison against the preceding full report found
one newly covered token-aware histogram checkpoint outcome at
`src/codecs/webp/native/encoder.rs:1684`. The input does not reach the
cost-manager 500-interval fallback at `backward_refs.rs:1604` or
`backward_refs.rs:1712`; the separate fragmented-stream run below also stays
below saturation. Coverage MCP cannot read reports in this managed worktree,
so this selected result has no MCP source/build attribution. No unit test or
codec implementation change was added.

## WebP fragmented-stream lossless cost-manager parity — 2026-10-27

Added active rows `encode:webp:enc_lossless_cost_manager_fragment_stream` and
`encode:webp:enc_lossless_cost_manager_fragment_stream_token`. Their seeded
1,024x1,024 RGB source is generated by
`scripts/generate_webp_cost_manager_fragment_stream_fixture.py`; the PNG SHA-256
is `a435566d1b22d8405e38ff5588213b5492050a180633586a3d67c673879aef40` and the
pixel SHA-256 is
`5afff9a4bc1b81d0f066a1f75fd2284cb1405cd28abfa0ffa01c17451e83146a`. Both
Pillow 12.2.0 outputs are 46,436 bytes with SHA-256
`466d1a901e783a9fda34b3af56767991c2fc2be0d4cfc055403bf594d7b6c9a8`.

The selected public parity run passed 2/2 rows, with 9 encode calls and no
panics. These cases do not reach the ordinary or token-aware 500-interval
fallback true arms. The counts previously reported at
`backward_refs.rs:1604` and `:1712` were executions of the below-saturation
false arms, not fallback hits. The selected LLVM report is
`.coverage-mcp/local-evidence/selected/webp-cost-manager-fragment-stream-final/coverage.json`
(SHA-256
`8166475636b977c51ddd13195d8914cebda29767b5240f0f380128f022dd461e`). Coverage
MCP still cannot read reports in this managed worktree, so this selected result
has no MCP source/build attribution.

Parity exposed an encoder limit mismatch: libwebp 1.6.0 caps
`MAX_COLOR_CACHE_BITS` at 10, while the Rust writer allowed 11. The writer and
palette candidate path now share the 10-bit cap, matching the upstream
definition in
[libwebp 1.6.0](https://github.com/webmproject/libwebp/blob/v1.6.0/src/enc/backward_references_enc.h#L28).
No unit tests were added.

## Full parity coverage refresh — 2026-10-27

The final full `make coverage` run passed 57/57 coverage-matrix tests,
7/7 decode-policy tests, 1/1 determinism test, and 69/69 feature-gate tests.
The retained LLVM report is
`.coverage-mcp/local-evidence/full-20261027-webp-cost-manager-final/coverage.json`
(SHA-256
`b7b2637a7f78c6134bf9d266c42be0a3ce1e6e7c2f6999d615d910aac86c22e`). Totals
are 93,914/139,613 lines (67.2674%), 18,647/32,056 branches (58.1701%),
5,229/8,933 functions (58.5358%), and 143,413/216,663 regions (66.1917%). All
four repository floors pass; the 100% coverage goal remains open.

The selected WebP fragmented-stream parity run does not reach either
cost-manager saturation arm at
`src/codecs/webp/native/encoder/backward_refs.rs:1604` or `:1712`; it reaches
only the below-saturation arms. The earlier claim that its 175,224 and 29,204
branch counts represented saturated fallback hits was incorrect. The existing
Rust feature-gate work-budget contract was rebased to the lower 6,547/6,548
boundary after the pinned libwebp cache limit was corrected. No unit tests
were added. `make fmt`, strict stable/nightly `make lint`,
`make lint-x86-simd`, and the native/WASI/WASM `make test-feature-matrix` all
passed. The SSE2 and AVX2 checks compile their target lanes; no new x86 runtime
benchmark was run.

## WebP Huffman, predictor-grid, and full coverage recheck — 2026-10-27

After adding the WebP singleton copy-symbol and one-cell predictor-grid parity
rows, the full `make coverage` run passed 57/57 coverage-matrix tests, 7/7 decode-policy
tests, 1/1 determinism test, 69/69 feature-gate tests, and all four repository
coverage floors. The report
`.coverage-mcp/local-evidence/full-20261027-predictor-grid-final/coverage.json`
has SHA-256
`985db53124f1c6f35102a8908ba2b98aa247750332487bffd7cb051973b01f10` and totals
93,966/139,636 lines (67.293535%), 18,674/32,068 branches (58.232506%),
5,230/8,934 functions (58.540407%), and 143,479/216,699 regions (66.211196%).
This report is local LLVM evidence; it has no Coverage MCP source/build
receipt.

The fragmented-stream pair was rerun with the exact selected-coverage command.
Both parity rows passed with 9 encode calls and no panics. Its report
`.coverage-mcp/local-evidence/selected/webp-cost-manager-fragment-stream-current-check/coverage.json`
has SHA-256
`26833aaf106f46d219020a6aa4260b4da989c3226a27140ca3f63e4a0207ce56`. Raw
LLVM branch counters show zero true-arm hits and 175,224 false-arm hits for the
ordinary saturation condition, and zero true-arm hits and 29,204 false-arm
hits for the token-aware condition. The full report likewise records zero
true-arm hits for both conditions. Coverage MCP identifies the token-aware
true arm as missing; source and test attribution remain unverified because no
MCP receipt is available. The 500-interval fallback therefore remains an
open coverage gap, and 100% coverage is not achieved.

The new `enc_lossless_tiny_predictor_grid` pair uses a pinned 8×3 RGBA PNG
with 24 colors to reach the one-cell predictor-grid case. Its selected parity
run passed 2/2 rows with 9 encode calls and no panics; both encoded references
are 74 bytes and decode to the expected RGBA pixels. Its local report
`.coverage-mcp/local-evidence/selected/webp-tiny-predictor-grid-final-20261027/coverage.json`
has SHA-256
`66e3ba306b74eb39836b9bd790f30dbab3dc3698878d1f6253d0662d72133369`.
Coverage MCP's branch query on the refreshed full report now shows the
one-cell `clamp_transform_bits` arm at line 497 as covered. The remaining
missing arms at lines 493 and 501 are defensive under the public method-five
and method-six configurations: they cap the initial bits at four and step
histogram bits down while the next finer grid would still contain one cell.
Under the parity-only test policy, these remain documented coverage gaps
rather than being relabeled as Pillow-reachable behavior. Coverage MCP still identifies
the ordinary and token-aware 500-interval saturation true arms at
`backward_refs.rs:1604` and `:1712` as missing. These reports have no MCP
source/build receipt; MCP results are limited to branch coordinates.

## AVIF zero decoder-model tick parity — 2026-10-27

Coverage-guided inspection identified the AV1 sequence-header branch that
rejects a zero decoder-model timing tick. Added active row
`decode:avif:error_sequence_zero_decoder_tick` and a deterministic full-file
mutation of the pinned `animated.avif` input. The generated AVIF SHA-256 is
`5287aa51ceac1e2128487a90caf3b5b12fcb30348a866ae700da42ccd4d08b49`.
Pinned Pillow 12.2.0 and Rust produced matching public decode failures; the
selected parity run passed 1/1.

The selected local LLVM report is
`.coverage-mcp/local-evidence/selected/avif-zero-decoder-tick-20261027.json`
(SHA-256
`865eefe0a3fb17031a01002584a8914babacf13c2fdc23154f4d907317802d5b`). Its
branch record for `src/codecs/avif/av1/sequence.rs:169` has 3 hits on the
zero-tick true arm. This is selected-run evidence: its receipt marks Coverage
MCP source/build attribution as unverified, and the full coverage suite was
not rerun. The last full report remains the 2026-10-27 baseline recorded above;
the 100% four-metric coverage goal remains open.

## WebP token-aware sampling row-mismatch parity — 2026-10-27

Added active row `encode:webp:enc_lossless_sampling_row_mismatch_early_stop`
with a deterministic 8×16,383 PNG source (SHA-256
`262b8efe57c954caf1068ea6e63969fbc205b667e85ce4788cc34aa5a75098d6`). The
selected parity run passed 1/1; Pillow and Rust agree on the 41,090-byte WebP
output and decoded pixels. The token-aware encoder enters `optimize_sampling`
and exercises both outcomes of the row-equality check before stopping on a
mismatch. It does not reach the 1,024-symbol compaction checkpoint; that
checkpoint remains an open coverage gap.

The selected report is
`.coverage-mcp/local-evidence/selected-20261027-webp-sampling-row-mismatch-final/coverage.json`.
Coverage MCP identifies the remaining `optimize_sampling` branch gaps, but
reports source attribution as unverified and test status as unknown. These
selected-run observations do not establish a full-suite coverage gain or
regression. An incremental comparison against the last full report was
incomparable because that baseline lacks exact branch detail. No Rust
production code or unit tests were added.

The current fixture matrix contains 2,674 total rows: 2,120 decode / inspect /
verify rows and 554 encode rows. Of those, 2,120 decode rows and 523 encode
rows are active; 31 planned encode rows remain.
