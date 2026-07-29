//! First closed AV1 leaf-block reconstruction class.
//!
//! This module is decoder-private codec machinery. It is deliberately not a
//! reusable transform, prediction, color-conversion, or raster-processing API.

use super::entropy::RangeDecoder;

type TransformCoefficients = [i32; 16];
type PlaneCoefficients = [TransformCoefficients; 16];

/// Exact syntax consumed for one supported leaf block.
struct BlockSyntax {
    luma_predictor: LumaPredictor,
    coefficients: [PlaneCoefficients; 3],
    transform_grid: TransformGrid,
}

/// Complete set of coded transform grids admitted by the closed AV1 class.
#[derive(Clone, Copy)]
pub(super) enum TransformGrid {
    Square8,
    Square16,
    Horizontal16x8,
    Vertical8x16,
}

impl TransformGrid {
    const fn properties(self) -> (usize, usize, [u16; 2]) {
        match self {
            Self::Square8 => (2, 2, [24_902, 0]),
            Self::Square16 => (4, 4, [20_360, 0]),
            Self::Horizontal16x8 => (4, 2, [23_374, 0]),
            Self::Vertical8x16 => (2, 4, [20_217, 0]),
        }
    }
}

#[derive(Clone, Copy)]
enum LumaPredictor {
    Dc,
    Vertical,
    Horizontal,
}

impl LumaPredictor {
    const fn sample(self) -> u16 {
        match self {
            Self::Dc => 128,
            Self::Vertical => 127,
            Self::Horizontal => 129,
        }
    }

    const fn cdf_index(self) -> usize {
        match self {
            Self::Dc => 0,
            Self::Vertical => 1,
            Self::Horizontal => 2,
        }
    }
}

#[derive(Clone, Copy)]
enum SpatialLumaContext {
    Origin,
    LeftVertical,
    LeftHorizontal,
    AboveVertical,
    AboveHorizontal,
}

impl SpatialLumaContext {
    const fn from_neighbor(orientation: SplitOrientation, predictor: LumaPredictor) -> Self {
        match (orientation, predictor) {
            (_, LumaPredictor::Dc) => Self::Origin,
            (SplitOrientation::Horizontal, LumaPredictor::Vertical) => Self::LeftVertical,
            (SplitOrientation::Horizontal, LumaPredictor::Horizontal) => Self::LeftHorizontal,
            (SplitOrientation::Vertical, LumaPredictor::Vertical) => Self::AboveVertical,
            (SplitOrientation::Vertical, LumaPredictor::Horizontal) => Self::AboveHorizontal,
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::Origin => 0,
            Self::LeftVertical => 1,
            Self::LeftHorizontal => 2,
            Self::AboveVertical => 3,
            Self::AboveHorizontal => 4,
        }
    }

    const fn accepts(self, predictor: LumaPredictor) -> bool {
        match self {
            Self::Origin => true,
            Self::LeftVertical | Self::LeftHorizontal => {
                matches!(predictor, LumaPredictor::Dc | LumaPredictor::Horizontal)
            }
            Self::AboveVertical | Self::AboveHorizontal => {
                matches!(predictor, LumaPredictor::Dc | LumaPredictor::Vertical)
            }
        }
    }
}

#[derive(Clone, Copy)]
enum CoefficientPolicy {
    DcOrSkipped,
    Skipped,
    SquareSkipped,
    BoundaryDc,
}

/// Frame-level coding tools that change one intra leaf's entropy syntax.
#[derive(Clone, Copy)]
pub(super) struct BlockTools {
    pub(super) allow_screen_content_tools: bool,
    pub(super) enable_filter_intra: bool,
}

/// Orientation of the smallest admitted two-child recursive split.
#[derive(Clone, Copy)]
pub(super) enum SplitOrientation {
    /// Two coded 8x8 children placed left-to-right.
    Horizontal,
    /// Two coded 8x8 children placed top-to-bottom.
    Vertical,
}

/// One codec-internal reconstructed plane in coded row-major order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::codecs::avif) struct ReconstructedPlane {
    pub(in crate::codecs::avif) samples: Vec<u16>,
}

/// The three planes reconstructed by the first supported AV1 leaf.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::codecs::avif) struct FirstLeaf {
    pub(in crate::codecs::avif) width: u32,
    pub(in crate::codecs::avif) height: u32,
    pub(in crate::codecs::avif) planes: [ReconstructedPlane; 3],
    #[cfg(coverage)]
    pub(in crate::codecs::avif) entropy_operations: Vec<crate::Av1EntropyOperationState>,
}

struct BlockCdfs {
    skip: [u16; 2],
    luma_mode: [[u16; 13]; 5],
    luma_angle: [[u16; 7]; 2],
    chroma_mode: [[u16; 13]; 3],
    palette_y: [u16; 2],
    palette_uv: [u16; 2],
    use_filter_intra: [u16; 2],
    coefficient_skip: [[u16; 2]; 2],
    trailing_coefficient_skip: [[u16; 2]; 2],
    double_neighbor_coefficient_skip: [[u16; 2]; 2],
    eob_bin_luma: [u16; 5],
    eob_bin_chroma: [u16; 5],
    eob_high_luma: [[u16; 2]; 2],
    eob_base_luma: [[u16; 3]; 3],
    eob_base_chroma: [u16; 3],
    base_luma: [[u16; 4]; 7],
    high_luma: [[u16; 4]; 11],
    high_chroma: [u16; 4],
    dc_sign: [[[u16; 2]; 3]; 2],
}

impl BlockCdfs {
    // ✅ VERIFIED: dav1d 1.5.3 src/cdf.c:113-138, 169-177, 412-414,
    // 719-843, 881-890, and 1313-1348 for q-context zero. Filter-intra is
    // indexed by block size at src/cdf.c:88-110.
    const fn defaults(use_filter_intra: [u16; 2]) -> Self {
        Self {
            skip: [1_097, 0],
            luma_mode: [
                [
                    17_180, 15_741, 13_430, 12_550, 12_086, 11_658, 10_943, 9_524, 8_579, 4_603,
                    3_675, 2_302, 0,
                ],
                [
                    20_752, 14_702, 13_252, 12_465, 12_049, 11_324, 10_880, 9_736, 8_334, 4_110,
                    2_596, 1_359, 0,
                ],
                [
                    22_716, 21_997, 10_472, 9_980, 9_713, 9_529, 8_635, 7_148, 6_608, 3_432, 2_839,
                    1_201, 0,
                ],
                [
                    22_745, 13_183, 11_920, 11_328, 10_936, 10_008, 9_679, 8_745, 7_387, 3_754,
                    2_286, 1_332, 0,
                ],
                [
                    20_155, 19_177, 11_385, 10_764, 10_456, 10_191, 9_367, 7_713, 7_039, 3_230,
                    2_463, 691, 0,
                ],
            ],
            luma_angle: [
                [30_588, 27_736, 25_201, 9_992, 5_779, 2_551, 0],
                [30_467, 27_160, 23_967, 9_281, 5_794, 2_438, 0],
            ],
            chroma_mode: [
                [
                    10_137, 8_616, 7_390, 7_107, 6_782, 6_248, 5_713, 4_845, 4_524, 2_709, 1_827,
                    807, 0,
                ],
                [
                    23_255, 5_887, 5_795, 5_722, 5_650, 5_104, 5_029, 4_944, 4_409, 3_263, 2_968,
                    972, 0,
                ],
                [
                    22_923, 22_853, 4_105, 4_064, 4_011, 3_988, 3_570, 2_946, 2_914, 2_004, 991,
                    739, 0,
                ],
            ],
            palette_y: [1_092, 0],
            palette_uv: [307, 0],
            use_filter_intra,
            coefficient_skip: [[26_876, 0], [22_807, 0]],
            trailing_coefficient_skip: [[10_833, 0], [2_526, 0]],
            double_neighbor_coefficient_skip: [[281, 0], [651, 0]],
            eob_bin_luma: [31_928, 31_729, 30_788, 27_873, 0],
            eob_bin_chroma: [29_521, 27_818, 23_080, 18_205, 0],
            eob_high_luma: [[15_807, 0], [15_545, 0]],
            eob_base_luma: [[14_931, 3_713, 0], [3_168, 1_322, 0], [1_924, 890, 0]],
            eob_base_chroma: [11_403, 2_742, 0],
            base_luma: [
                [28_734, 23_838, 20_041, 0],
                [14_686, 3_027, 891, 0],
                [20_172, 6_644, 2_275, 0],
                [23_322, 11_650, 5_763, 0],
                [26_460, 17_627, 11_489, 0],
                [30_305, 26_411, 22_985, 0],
                [12_101, 2_222, 839, 0],
            ],
            high_luma: [
                [18_470, 12_050, 8_594, 0],
                [20_232, 13_167, 8_979, 0],
                [24_056, 17_717, 13_265, 0],
                [26_598, 21_441, 17_334, 0],
                [28_026, 23_842, 20_230, 0],
                [28_965, 25_451, 22_222, 0],
                [31_072, 29_451, 27_897, 0],
                [18_376, 12_817, 10_012, 0],
                [16_790, 9_550, 5_950, 0],
                [20_581, 13_294, 8_879, 0],
                [23_592, 17_128, 12_509, 0],
            ],
            high_chroma: [16_801, 9_863, 6_482, 0],
            dc_sign: [
                [[16_768, 0], [19_712, 0], [13_952, 0]],
                [[17_536, 0], [19_840, 0], [15_488, 0]],
            ],
        }
    }
}

// ✅ VERIFIED: dav1d 1.5.3 src/recon_tmpl.c:49-57.
fn read_golomb(decoder: &mut RangeDecoder<'_, '_, '_>) -> u32 {
    let mut length = 0_u32;
    while !decoder.equal() && length < 32 {
        length = length.wrapping_add(1);
    }
    let mut value = 1_u32;
    for _ in 0..length {
        value = value
            .wrapping_shl(1)
            .wrapping_add(u32::from(decoder.equal()));
    }
    value.wrapping_sub(1)
}

fn decode_high_token(decoder: &mut RangeDecoder<'_, '_, '_>, cdf: &mut [u16; 4]) -> u32 {
    decoder.high_token(cdf)
}

fn decode_eob_bin(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    plane: usize,
    cdfs: &mut BlockCdfs,
) -> u32 {
    if plane == 0 {
        decoder.adaptive_symbol(&mut cdfs.eob_bin_luma, 4)
    } else {
        decoder.adaptive_symbol(&mut cdfs.eob_bin_chroma, 4)
    }
}

fn extend_high_token(decoder: &mut RangeDecoder<'_, '_, '_>, token: u32) -> u32 {
    if token == 15 {
        read_golomb(decoder).wrapping_add(15)
    } else {
        token
    }
}

fn dequantize_lossless_coefficient(token: u32, negative: bool) -> i32 {
    // ✅ VERIFIED: dav1d 1.5.3 src/dequant_tables.c q-index zero and
    // src/recon_tmpl.c:596-718. Eight-bit all-lossless dequant is four.
    #[expect(
        clippy::cast_possible_wrap,
        reason = "dav1d evaluates malformed coefficient syntax with wrapping C-width arithmetic"
    )]
    let magnitude = (token as i32).wrapping_mul(4);
    if negative {
        magnitude.wrapping_neg()
    } else {
        magnitude
    }
}

fn decode_dc_only_after_eob(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    plane: usize,
    sign_context: usize,
    cdfs: &mut BlockCdfs,
) -> Option<TransformCoefficients> {
    let base_token = if plane == 0 {
        decoder.adaptive_symbol(&mut cdfs.eob_base_luma[0], 2)
    } else {
        decoder.adaptive_symbol(&mut cdfs.eob_base_chroma, 2)
    };
    (base_token == 2).then_some(())?;
    let token = if plane == 0 {
        decode_high_token(decoder, &mut cdfs.high_luma[0])
    } else {
        decode_high_token(decoder, &mut cdfs.high_chroma)
    };
    let coefficient_context = usize::from(plane != 0);
    let negative = decoder.adaptive_bool(&mut cdfs.dc_sign[coefficient_context][sign_context]);
    // ✅ VERIFIED: dav1d 1.5.3 src/recon_tmpl.c:615-632. High tokens below
    // fifteen are complete magnitudes; only token fifteen has a Golomb
    // extension. Reading that extension for a direct token would consume the
    // following sign or coefficient syntax.
    let token = extend_high_token(decoder, token);
    let mut coefficients = [0_i32; 16];
    coefficients[0] = dequantize_lossless_coefficient(token, negative);
    Some(coefficients)
}

fn decode_luma_dc_after_ac(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    sign_context: usize,
    cdfs: &mut BlockCdfs,
    high_context: usize,
) -> Option<(u32, bool)> {
    // ✅ VERIFIED: dav1d 1.5.3 src/recon_tmpl.c:526-546. Once EOB is nonzero,
    // DC uses base_tok rather than eob_base_tok, and its high-token context
    // is derived from the preceding AC level neighborhood.
    let dc_base_token = decoder.adaptive_symbol(&mut cdfs.base_luma[0], 3);
    (dc_base_token == 3).then_some(())?;
    let dc_token = decode_high_token(decoder, &mut cdfs.high_luma[high_context]);

    // ✅ VERIFIED: dav1d 1.5.3 src/recon_tmpl.c:596-718. DC sign and a possible
    // DC Golomb extension precede every equiprobable AC sign and extension.
    let dc_negative = decoder.adaptive_bool(&mut cdfs.dc_sign[0][sign_context]);
    Some((extend_high_token(decoder, dc_token), dc_negative))
}

fn decode_luma_direct_ac_and_dc(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    sign_context: usize,
    cdfs: &mut BlockCdfs,
    ac_token: u32,
    ac_coefficient_index: usize,
) -> Option<TransformCoefficients> {
    // ✅ VERIFIED: dav1d 1.5.3 src/recon_tmpl.c:526-546 and the pinned Slice
    // 21 trace. The DC high-token context is derived from the preceding AC
    // magnitude. This closed class admits only direct AC token ten, which
    // selects context five.
    let (dc_token, dc_negative) = decode_luma_dc_after_ac(decoder, sign_context, cdfs, 5)?;

    let ac_negative = decoder.equal();
    let ac_token = extend_high_token(decoder, ac_token);

    let mut coefficients = [0_i32; 16];
    coefficients[0] = dequantize_lossless_coefficient(dc_token, dc_negative);
    coefficients[ac_coefficient_index] = dequantize_lossless_coefficient(ac_token, ac_negative);
    Some(coefficients)
}

fn decode_luma_direct_ac_token(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    cdfs: &mut BlockCdfs,
) -> Option<u32> {
    let ac_base_token = decoder.adaptive_symbol(&mut cdfs.eob_base_luma[1], 2);
    (ac_base_token == 2).then_some(())?;
    let ac_token = decode_high_token(decoder, &mut cdfs.high_luma[7]);
    (ac_token == 10).then_some(ac_token)
}

fn decode_luma_eob_one_after_eob(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    sign_context: usize,
    cdfs: &mut BlockCdfs,
) -> Option<TransformCoefficients> {
    // ✅ VERIFIED: dav1d 1.5.3 src/recon_tmpl.c:443-480 and pinned Slice 21
    // traces. EOB one uses context one, scan_4x4[1] == 4, and the high-token
    // context seven for its final AC base token.
    let ac_token = decode_luma_direct_ac_token(decoder, cdfs)?;
    decode_luma_direct_ac_and_dc(decoder, sign_context, cdfs, ac_token, 4)
}

fn decode_luma_eob_two_after_eob(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    sign_context: usize,
    cdfs: &mut BlockCdfs,
) -> Option<TransformCoefficients> {
    // ✅ VERIFIED: dav1d 1.5.3 src/recon_tmpl.c:407-564, src/scan.c:35-40,
    // and the pinned Slice 22 scalar trace. EOB-bin symbol two uses high-bit
    // context zero, scan_4x4[2] == 1, then visits scan_4x4[1] == 4 in reverse.
    (!decoder.adaptive_bool(&mut cdfs.eob_high_luma[0])).then_some(())?;
    let ac_token = decode_luma_direct_ac_token(decoder, cdfs)?;
    (decoder.adaptive_symbol(&mut cdfs.base_luma[1], 3) == 0).then_some(())?;
    decode_luma_direct_ac_and_dc(decoder, sign_context, cdfs, ac_token, 1)
}

fn decode_luma_eob_four_after_eob(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    sign_context: usize,
    cdfs: &mut BlockCdfs,
) -> Option<TransformCoefficients> {
    // ✅ VERIFIED: dav1d 1.5.3 src/recon_tmpl.c:403-545, src/scan.c:35-40,
    // and the pinned Slice 23 scalar trace. EOB-bin symbol three uses
    // high-bit context one and one equiprobable extra bit. This closed class
    // admits only EOB four and rejects EOB six before coefficient syntax.
    (!decoder.adaptive_bool(&mut cdfs.eob_high_luma[1])).then_some(())?;
    (!decoder.equal()).then_some(())?;

    // scan_4x4[4] == 5 is the final coefficient and uses EOB-base context
    // two. The reverse loop then visits raster 2, 1, and 4 with contexts
    // six, three, and three respectively.
    (decoder.adaptive_symbol(&mut cdfs.eob_base_luma[2], 2) == 2).then_some(())?;
    let coefficient_five_token = decode_high_token(decoder, &mut cdfs.high_luma[7]);
    (coefficient_five_token == 5).then_some(())?;
    (decoder.adaptive_symbol(&mut cdfs.base_luma[6], 3) == 0).then_some(())?;
    (decoder.adaptive_symbol(&mut cdfs.base_luma[3], 3) == 3).then_some(())?;
    let coefficient_one_token = decode_high_token(decoder, &mut cdfs.high_luma[10]);
    (coefficient_one_token == 5).then_some(())?;
    (decoder.adaptive_symbol(&mut cdfs.base_luma[3], 3) == 3).then_some(())?;
    let coefficient_four_token = decode_high_token(decoder, &mut cdfs.high_luma[10]);
    (coefficient_four_token == 5).then_some(())?;

    // The three direct AC magnitudes select DC high-token context six.
    // dav1d's nonzero link chain reads signs in raster order 4, 1, then 5.
    let (dc_token, dc_negative) = decode_luma_dc_after_ac(decoder, sign_context, cdfs, 6)?;
    let coefficient_four_negative = decoder.equal();
    let coefficient_four_token = extend_high_token(decoder, coefficient_four_token);
    let coefficient_one_negative = decoder.equal();
    let coefficient_one_token = extend_high_token(decoder, coefficient_one_token);
    let coefficient_five_negative = decoder.equal();
    let coefficient_five_token = extend_high_token(decoder, coefficient_five_token);

    let mut coefficients = [0_i32; 16];
    coefficients[0] = dequantize_lossless_coefficient(dc_token, dc_negative);
    coefficients[1] =
        dequantize_lossless_coefficient(coefficient_one_token, coefficient_one_negative);
    coefficients[4] =
        dequantize_lossless_coefficient(coefficient_four_token, coefficient_four_negative);
    coefficients[5] =
        dequantize_lossless_coefficient(coefficient_five_token, coefficient_five_negative);
    Some(coefficients)
}

fn decode_nonzero_lossless_transform(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    plane: usize,
    sign_context: usize,
    allow_luma_ac: bool,
    cdfs: &mut BlockCdfs,
) -> Option<TransformCoefficients> {
    match decode_eob_bin(decoder, plane, cdfs) {
        0 => decode_dc_only_after_eob(decoder, plane, sign_context, cdfs),
        1 if allow_luma_ac && plane == 0 => {
            decode_luma_eob_one_after_eob(decoder, sign_context, cdfs)
        }
        2 if allow_luma_ac && plane == 0 => {
            decode_luma_eob_two_after_eob(decoder, sign_context, cdfs)
        }
        3 if allow_luma_ac && plane == 0 => {
            decode_luma_eob_four_after_eob(decoder, sign_context, cdfs)
        }
        _ => None,
    }
}

fn decode_dc_coefficients(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    plane: usize,
    cdfs: &mut BlockCdfs,
    transform_grid_width: usize,
    transform_grid_height: usize,
) -> Option<PlaneCoefficients> {
    let transform_count = transform_grid_width.saturating_mul(transform_grid_height);
    let mut coefficients = [[0_i32; 16]; 16];
    let coefficient_context = usize::from(plane != 0);
    let first_zero = decoder.adaptive_bool(&mut cdfs.coefficient_skip[coefficient_context]);
    if first_zero {
        // ✅ VERIFIED: dav1d 1.5.3 src/recon_tmpl.c:754-838 and the pinned
        // Slice 12 scalar traces. With no top-left coefficient context, every
        // remaining transform in the 2x2 or 4x4 grid uses the base skip CDF.
        for _ in 1..transform_count {
            decoder
                .adaptive_bool(&mut cdfs.coefficient_skip[coefficient_context])
                .then_some(())?;
        }
        return Some(coefficients);
    }
    coefficients[0] = decode_nonzero_lossless_transform(decoder, plane, 0, false, cdfs)?;
    // ✅ VERIFIED: dav1d 1.5.3 src/recon_tmpl.c:754-838. A nonzero top-left
    // transform changes the coefficient context of exactly its right and
    // lower neighbors. All later row-major transforms return to the base CDF.
    for transform_index in 1..transform_count {
        let skipped = if transform_index == 1 || transform_index == transform_grid_width {
            decoder.adaptive_bool(&mut cdfs.trailing_coefficient_skip[coefficient_context])
        } else {
            decoder.adaptive_bool(&mut cdfs.coefficient_skip[coefficient_context])
        };
        skipped.then_some(())?;
    }
    Some(coefficients)
}

fn decode_boundary_coefficients(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    plane: usize,
    cdfs: &mut BlockCdfs,
    transform_grid_width: usize,
    transform_grid_height: usize,
) -> Option<PlaneCoefficients> {
    ((transform_grid_width, transform_grid_height) == (2, 2)).then_some(())?;
    let coefficient_context = usize::from(plane != 0);
    let first_skipped = decoder.adaptive_bool(&mut cdfs.coefficient_skip[coefficient_context]);
    let mut coefficients = [[0_i32; 16]; 16];
    if first_skipped {
        // ✅ VERIFIED: dav1d 1.5.3 src/recon_tmpl.c:59-105, 1282-1545 and
        // pinned Slice 21 traces. If the first boundary transform is skipped,
        // all four transforms retain the same base skip context.
        for _ in 1..4 {
            decoder
                .adaptive_bool(&mut cdfs.coefficient_skip[coefficient_context])
                .then_some(())?;
        }
        return Some(coefficients);
    }
    coefficients[0] = decode_nonzero_lossless_transform(decoder, plane, 0, true, cdfs)?;
    let sign_context = if coefficients[0][0].is_negative() {
        1
    } else {
        2
    };
    for coefficient in &mut coefficients[1..3] {
        (!decoder.adaptive_bool(&mut cdfs.trailing_coefficient_skip[coefficient_context]))
            .then_some(())?;
        *coefficient = decode_nonzero_lossless_transform(decoder, plane, sign_context, true, cdfs)?;
    }
    decoder
        .adaptive_bool(&mut cdfs.double_neighbor_coefficient_skip[coefficient_context])
        .then_some(())?;
    Some(coefficients)
}

fn decode_skipped_coefficients(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    plane: usize,
    cdfs: &mut BlockCdfs,
    transform_grid_width: usize,
    transform_grid_height: usize,
) -> Option<PlaneCoefficients> {
    let transform_count = transform_grid_width.saturating_mul(transform_grid_height);
    let coefficient_context = usize::from(plane != 0);
    for _ in 0..transform_count {
        decoder
            .adaptive_bool(&mut cdfs.coefficient_skip[coefficient_context])
            .then_some(())?;
    }
    Some([[0_i32; 16]; 16])
}

fn decode_syntax(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    cdfs: &mut BlockCdfs,
    transform_grid: TransformGrid,
    spatial_luma_context: SpatialLumaContext,
    coefficient_policy: CoefficientPolicy,
    tools: BlockTools,
) -> Option<BlockSyntax> {
    let (transform_grid_width, transform_grid_height, _) = transform_grid.properties();
    (!decoder.adaptive_bool(&mut cdfs.skip)).then_some(())?;
    let luma_predictor =
        match decoder.adaptive_symbol(&mut cdfs.luma_mode[spatial_luma_context.index()], 12) {
            0 => LumaPredictor::Dc,
            1 if !matches!(
                coefficient_policy,
                CoefficientPolicy::SquareSkipped | CoefficientPolicy::BoundaryDc
            ) =>
            {
                LumaPredictor::Vertical
            }
            2 if !matches!(
                coefficient_policy,
                CoefficientPolicy::SquareSkipped | CoefficientPolicy::BoundaryDc
            ) =>
            {
                LumaPredictor::Horizontal
            }
            _ => return None,
        };
    spatial_luma_context.accepts(luma_predictor).then_some(())?;
    if let LumaPredictor::Vertical | LumaPredictor::Horizontal = luma_predictor {
        let angle_index = luma_predictor.cdf_index().wrapping_sub(1);
        #[expect(
            clippy::cast_possible_wrap,
            reason = "the decoded symbol is bounded to 0..=6"
        )]
        let luma_angle_delta =
            (decoder.adaptive_symbol(&mut cdfs.luma_angle[angle_index], 6) as i32).wrapping_sub(3);
        (luma_angle_delta == 0).then_some(())?;
    }
    let predictor_index = luma_predictor.cdf_index();
    (decoder.adaptive_symbol(&mut cdfs.chroma_mode[predictor_index], 12) == 0).then_some(())?;
    if tools.allow_screen_content_tools {
        if matches!(luma_predictor, LumaPredictor::Dc) {
            (!decoder.adaptive_bool(&mut cdfs.palette_y)).then_some(())?;
        }
        (!decoder.adaptive_bool(&mut cdfs.palette_uv)).then_some(())?;
    }
    if matches!(luma_predictor, LumaPredictor::Dc) && tools.enable_filter_intra {
        (!decoder.adaptive_bool(&mut cdfs.use_filter_intra)).then_some(())?;
    }
    let decode_plane = |decoder: &mut RangeDecoder<'_, '_, '_>, plane, cdfs: &mut BlockCdfs| {
        match coefficient_policy {
            CoefficientPolicy::DcOrSkipped => decode_dc_coefficients(
                decoder,
                plane,
                cdfs,
                transform_grid_width,
                transform_grid_height,
            ),
            CoefficientPolicy::Skipped | CoefficientPolicy::SquareSkipped => {
                decode_skipped_coefficients(
                    decoder,
                    plane,
                    cdfs,
                    transform_grid_width,
                    transform_grid_height,
                )
            }
            CoefficientPolicy::BoundaryDc => decode_boundary_coefficients(
                decoder,
                plane,
                cdfs,
                transform_grid_width,
                transform_grid_height,
            ),
        }
    };
    let coefficients = [
        decode_plane(decoder, 0, cdfs)?,
        decode_plane(decoder, 1, cdfs)?,
        decode_plane(decoder, 2, cdfs)?,
    ];
    Some(BlockSyntax {
        luma_predictor,
        coefficients,
        transform_grid,
    })
}

// ✅ VERIFIED: dav1d 1.5.3 src/itx_1d.c:1066-1080.
fn inverse_wht_4(values: &mut [i32; 4]) {
    let input_zero = values[0];
    let input_one = values[1];
    let input_two = values[2];
    let input_three = values[3];
    let temporary_zero = input_zero.wrapping_add(input_one);
    let temporary_two = input_two.wrapping_sub(input_three);
    let temporary_four = temporary_zero.wrapping_sub(temporary_two) >> 1;
    let temporary_three = temporary_four.wrapping_sub(input_three);
    let temporary_one = temporary_four.wrapping_sub(input_one);
    *values = [
        temporary_zero.wrapping_sub(temporary_three),
        temporary_three,
        temporary_one,
        temporary_two.wrapping_add(temporary_one),
    ];
}

// ✅ VERIFIED: dav1d 1.5.3 src/itx_tmpl.c:184-207.
fn inverse_wht_4x4(coefficients: TransformCoefficients) -> [i32; 16] {
    let mut values = [0_i32; 16];
    let columns = [[0, 4, 8, 12], [1, 5, 9, 13], [2, 6, 10, 14], [3, 7, 11, 15]];
    for (row, indices) in values.chunks_exact_mut(4).zip(columns) {
        let mut vector = [
            coefficients[indices[0]] >> 2,
            coefficients[indices[1]] >> 2,
            coefficients[indices[2]] >> 2,
            coefficients[indices[3]] >> 2,
        ];
        inverse_wht_4(&mut vector);
        row.copy_from_slice(&vector);
    }
    for indices in columns {
        let mut vector = [
            values[indices[0]],
            values[indices[1]],
            values[indices[2]],
            values[indices[3]],
        ];
        inverse_wht_4(&mut vector);
        for (index, value) in indices.into_iter().zip(vector) {
            values[index] = value;
        }
    }
    values
}

fn reconstruct_transform(predictor: u16, coefficients: TransformCoefficients) -> [u16; 16] {
    let residual = inverse_wht_4x4(coefficients);
    let mut samples = [0_u16; 16];
    for (sample, value) in samples.iter_mut().zip(residual) {
        let reconstructed = i32::from(predictor).saturating_add(value).clamp(0, 255);
        #[expect(
            clippy::cast_sign_loss,
            reason = "the reconstructed eight-bit sample is explicitly clamped to 0..=255"
        )]
        let reconstructed = reconstructed as u16;
        *sample = reconstructed;
    }
    samples
}

// ✅ VERIFIED: dav1d 1.5.3 src/recon_tmpl.c:1176-1545. For the accepted
// first-frame leaves, dav1d visits the 2x2, 4x2, 2x4, or 4x4 transform grid in
// row-major order. The first DC-only transform reconstructs one constant
// value. With every trailing transform skipped, the accepted DC, vertical,
// and horizontal predictors propagate that value through the remaining coded
// plane.
fn reconstruct_coded_plane(
    predictor: u16,
    coefficients: PlaneCoefficients,
    transform_grid: TransformGrid,
) -> ReconstructedPlane {
    let (transform_grid_width, transform_grid_height, _) = transform_grid.properties();
    let coded_width = transform_grid_width.saturating_mul(4);
    let coded_height = transform_grid_height.saturating_mul(4);
    let transform_count = transform_grid_width.saturating_mul(transform_grid_height);
    let mut coded_samples = vec![0_u16; coded_width.saturating_mul(coded_height)];
    let mut propagated_predictor = predictor;
    for (transform_index, coefficient) in coefficients.into_iter().take(transform_count).enumerate()
    {
        let transform = reconstruct_transform(propagated_predictor, coefficient);
        if transform_index == 0 {
            propagated_predictor = transform[0];
        }
        let transform_x = transform_index.rem_euclid(transform_grid_width);
        let transform_y = transform_index.div_euclid(transform_grid_width);
        for (row_index, row) in transform.chunks_exact(4).enumerate() {
            let output_start = transform_y
                .saturating_mul(4)
                .saturating_add(row_index)
                .saturating_mul(coded_width)
                .saturating_add(transform_x.saturating_mul(4));
            let output_end = output_start.saturating_add(4);
            coded_samples[output_start..output_end].copy_from_slice(row);
        }
    }
    ReconstructedPlane {
        samples: coded_samples,
    }
}

fn visible_plane(
    coded: &ReconstructedPlane,
    coded_width: usize,
    width: u32,
    height: u32,
) -> ReconstructedPlane {
    // ✅ VERIFIED: dav1d retains the top-left declared rectangle from the
    // padded 8x8, 16x8, 8x16, or 16x16 reconstruction for the accepted
    // dimensions.
    let visible_width = width as usize;
    let visible_height = height as usize;
    let mut samples = Vec::with_capacity(visible_width.saturating_mul(visible_height));
    for coded_row in coded.samples.chunks_exact(coded_width).take(visible_height) {
        samples.extend_from_slice(&coded_row[..visible_width]);
    }
    ReconstructedPlane { samples }
}

struct ClosedLeaf {
    luma_predictor: LumaPredictor,
    planes: [ReconstructedPlane; 3],
}

fn reconstruct_leaf(syntax: BlockSyntax, predictors: [u16; 3]) -> ClosedLeaf {
    let planes = [
        reconstruct_coded_plane(predictors[0], syntax.coefficients[0], syntax.transform_grid),
        reconstruct_coded_plane(predictors[1], syntax.coefficients[1], syntax.transform_grid),
        reconstruct_coded_plane(predictors[2], syntax.coefficients[2], syntax.transform_grid),
    ];
    ClosedLeaf {
        luma_predictor: syntax.luma_predictor,
        planes,
    }
}

fn origin_predictors(luma_predictor: LumaPredictor) -> [u16; 3] {
    [luma_predictor.sample(), 128, 128]
}

fn neighbor_edge_sample(plane: &ReconstructedPlane, orientation: SplitOrientation) -> u16 {
    let index = match orientation {
        SplitOrientation::Horizontal => 7,
        SplitOrientation::Vertical => 56,
    };
    plane.samples[index]
}

fn neighbor_predictors(first: &ClosedLeaf, orientation: SplitOrientation) -> [u16; 3] {
    [
        neighbor_edge_sample(&first.planes[0], orientation),
        neighbor_edge_sample(&first.planes[1], orientation),
        neighbor_edge_sample(&first.planes[2], orientation),
    ]
}

fn bottom_edge(plane: &ReconstructedPlane) -> [u16; 8] {
    [
        plane.samples[56],
        plane.samples[57],
        plane.samples[58],
        plane.samples[59],
        plane.samples[60],
        plane.samples[61],
        plane.samples[62],
        plane.samples[63],
    ]
}

fn right_edge(plane: &ReconstructedPlane) -> [u16; 8] {
    [
        plane.samples[7],
        plane.samples[15],
        plane.samples[23],
        plane.samples[31],
        plane.samples[39],
        plane.samples[47],
        plane.samples[55],
        plane.samples[63],
    ]
}

fn dc_predictor(top: [u16; 4], left: [u16; 4]) -> u16 {
    let sum = top
        .iter()
        .chain(&left)
        .fold(0_u32, |sum, sample| sum.saturating_add(u32::from(*sample)));
    u16::try_from(sum.saturating_add(4).div_euclid(8)).unwrap_or(128)
}

fn reconstruct_dc_transform(
    top: [u16; 4],
    left: [u16; 4],
    coefficients: TransformCoefficients,
) -> [u16; 16] {
    reconstruct_transform(dc_predictor(top, left), coefficients)
}

fn reconstruct_boundary_plane(
    above: &ReconstructedPlane,
    left: &ReconstructedPlane,
    coefficients: PlaneCoefficients,
) -> ReconstructedPlane {
    let top = bottom_edge(above);
    let left = right_edge(left);
    let top_left = reconstruct_dc_transform(
        [top[0], top[1], top[2], top[3]],
        [left[0], left[1], left[2], left[3]],
        coefficients[0],
    );
    let top_right = reconstruct_dc_transform(
        [top[4], top[5], top[6], top[7]],
        [top_left[3], top_left[7], top_left[11], top_left[15]],
        coefficients[1],
    );
    let bottom_left = reconstruct_dc_transform(
        [top_left[12], top_left[13], top_left[14], top_left[15]],
        [left[4], left[5], left[6], left[7]],
        coefficients[2],
    );
    let bottom_right = reconstruct_dc_transform(
        [top_right[12], top_right[13], top_right[14], top_right[15]],
        [
            bottom_left[3],
            bottom_left[7],
            bottom_left[11],
            bottom_left[15],
        ],
        coefficients[3],
    );
    let mut samples = Vec::with_capacity(64);
    for (first, second) in top_left.chunks_exact(4).zip(top_right.chunks_exact(4)) {
        samples.extend_from_slice(first);
        samples.extend_from_slice(second);
    }
    for (first, second) in bottom_left
        .chunks_exact(4)
        .zip(bottom_right.chunks_exact(4))
    {
        samples.extend_from_slice(first);
        samples.extend_from_slice(second);
    }
    ReconstructedPlane { samples }
}

fn reconstruct_boundary_leaf(
    syntax: BlockSyntax,
    above: &ClosedLeaf,
    left: &ClosedLeaf,
) -> ClosedLeaf {
    let planes = [
        reconstruct_boundary_plane(&above.planes[0], &left.planes[0], syntax.coefficients[0]),
        reconstruct_boundary_plane(&above.planes[1], &left.planes[1], syntax.coefficients[1]),
        reconstruct_boundary_plane(&above.planes[2], &left.planes[2], syntax.coefficients[2]),
    ];
    ClosedLeaf {
        luma_predictor: syntax.luma_predictor,
        planes,
    }
}

fn decode_following_square_syntax<F>(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    cdfs: &mut BlockCdfs,
    transform_grid: TransformGrid,
    spatial_luma_context: SpatialLumaContext,
    coefficient_policy: CoefficientPolicy,
    tools: BlockTools,
    between_leaves: &mut F,
) -> Option<BlockSyntax>
where
    F: FnMut(&mut RangeDecoder<'_, '_, '_>) -> Option<()>,
{
    between_leaves(decoder)?;
    decode_syntax(
        decoder,
        cdfs,
        transform_grid,
        spatial_luma_context,
        coefficient_policy,
        tools,
    )
}

fn visible_leaf(
    leaf: ClosedLeaf,
    transform_grid: TransformGrid,
    width: u32,
    height: u32,
) -> FirstLeaf {
    let (transform_grid_width, _, _) = transform_grid.properties();
    let coded_width = transform_grid_width.saturating_mul(4);
    let planes = leaf
        .planes
        .each_ref()
        .map(|plane| visible_plane(plane, coded_width, width, height));
    FirstLeaf {
        width,
        height,
        planes,
        #[cfg(coverage)]
        entropy_operations: Vec::new(),
    }
}

/// Decode and reconstruct the first complete closed leaf syntax class.
pub(super) fn decode_first_lossless_444_leaf(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    width: u32,
    height: u32,
    transform_grid: TransformGrid,
    tools: BlockTools,
) -> Option<FirstLeaf> {
    let (_, _, use_filter_intra) = transform_grid.properties();
    let mut cdfs = BlockCdfs::defaults(use_filter_intra);
    let syntax = decode_syntax(
        decoder,
        &mut cdfs,
        transform_grid,
        SpatialLumaContext::Origin,
        CoefficientPolicy::DcOrSkipped,
        tools,
    )?;
    let predictors = origin_predictors(syntax.luma_predictor);
    let leaf = reconstruct_leaf(syntax, predictors);
    Some(visible_leaf(leaf, transform_grid, width, height))
}

fn compose_split_plane(
    first: &ReconstructedPlane,
    second: &ReconstructedPlane,
    orientation: SplitOrientation,
) -> ReconstructedPlane {
    let samples = match orientation {
        SplitOrientation::Horizontal => {
            let mut samples = Vec::with_capacity(128);
            for (first_row, second_row) in first
                .samples
                .chunks_exact(8)
                .zip(second.samples.chunks_exact(8))
            {
                samples.extend_from_slice(first_row);
                samples.extend_from_slice(second_row);
            }
            samples
        }
        SplitOrientation::Vertical => {
            let mut samples = Vec::with_capacity(128);
            samples.extend_from_slice(&first.samples);
            samples.extend_from_slice(&second.samples);
            samples
        }
    };
    ReconstructedPlane { samples }
}

fn compose_square_plane(
    top_left: &ReconstructedPlane,
    top_right: &ReconstructedPlane,
    bottom_left: &ReconstructedPlane,
    bottom_right: &ReconstructedPlane,
) -> ReconstructedPlane {
    let top = compose_split_plane(top_left, top_right, SplitOrientation::Horizontal);
    let bottom = compose_split_plane(bottom_left, bottom_right, SplitOrientation::Horizontal);
    compose_split_plane(&top, &bottom, SplitOrientation::Vertical)
}

/// Decode and compose the smallest closed two-leaf recursive split.
pub(super) fn decode_two_lossless_444_leaves<F>(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    width: u32,
    height: u32,
    orientation: SplitOrientation,
    tools: BlockTools,
    between_leaves: F,
) -> Option<FirstLeaf>
where
    F: FnOnce(&mut RangeDecoder<'_, '_, '_>) -> Option<()>,
{
    let transform_grid = TransformGrid::Square8;
    let (_, _, use_filter_intra) = transform_grid.properties();
    let mut cdfs = BlockCdfs::defaults(use_filter_intra);

    let first_syntax = decode_syntax(
        decoder,
        &mut cdfs,
        transform_grid,
        SpatialLumaContext::Origin,
        CoefficientPolicy::DcOrSkipped,
        tools,
    )?;
    let first_predictors = origin_predictors(first_syntax.luma_predictor);
    let first = reconstruct_leaf(first_syntax, first_predictors);

    between_leaves(decoder)?;

    let spatial_luma_context = SpatialLumaContext::from_neighbor(orientation, first.luma_predictor);
    let second_syntax = decode_syntax(
        decoder,
        &mut cdfs,
        transform_grid,
        spatial_luma_context,
        CoefficientPolicy::Skipped,
        tools,
    )?;
    let second_predictors = neighbor_predictors(&first, orientation);
    let second = reconstruct_leaf(second_syntax, second_predictors);

    let coded_width = match orientation {
        SplitOrientation::Horizontal => 16,
        SplitOrientation::Vertical => 8,
    };
    let coded_planes = [
        compose_split_plane(&first.planes[0], &second.planes[0], orientation),
        compose_split_plane(&first.planes[1], &second.planes[1], orientation),
        compose_split_plane(&first.planes[2], &second.planes[2], orientation),
    ];
    let planes = coded_planes
        .each_ref()
        .map(|plane| visible_plane(plane, coded_width, width, height));
    Some(FirstLeaf {
        width,
        height,
        planes,
        #[cfg(coverage)]
        entropy_operations: Vec::new(),
    })
}

/// Decode and compose the first closed four-leaf square recursive split.
pub(super) fn decode_four_lossless_444_leaves<F>(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    width: u32,
    height: u32,
    tools: BlockTools,
    mut between_leaves: F,
) -> Option<FirstLeaf>
where
    F: FnMut(&mut RangeDecoder<'_, '_, '_>) -> Option<()>,
{
    let transform_grid = TransformGrid::Square8;
    let (_, _, use_filter_intra) = transform_grid.properties();
    let mut cdfs = BlockCdfs::defaults(use_filter_intra);

    let top_left_syntax = decode_syntax(
        decoder,
        &mut cdfs,
        transform_grid,
        SpatialLumaContext::Origin,
        CoefficientPolicy::DcOrSkipped,
        tools,
    )?;
    let top_left_predictors = origin_predictors(top_left_syntax.luma_predictor);
    let top_left = reconstruct_leaf(top_left_syntax, top_left_predictors);

    let top_right_context =
        SpatialLumaContext::from_neighbor(SplitOrientation::Horizontal, top_left.luma_predictor);
    let top_right_syntax = decode_following_square_syntax(
        decoder,
        &mut cdfs,
        transform_grid,
        top_right_context,
        CoefficientPolicy::SquareSkipped,
        tools,
        &mut between_leaves,
    )?;
    let top_right_predictors = neighbor_predictors(&top_left, SplitOrientation::Horizontal);
    let top_right = reconstruct_leaf(top_right_syntax, top_right_predictors);

    let bottom_left_context =
        SpatialLumaContext::from_neighbor(SplitOrientation::Vertical, top_left.luma_predictor);
    let bottom_left_syntax = decode_following_square_syntax(
        decoder,
        &mut cdfs,
        transform_grid,
        bottom_left_context,
        CoefficientPolicy::SquareSkipped,
        tools,
        &mut between_leaves,
    )?;
    let bottom_left_predictors = neighbor_predictors(&top_left, SplitOrientation::Vertical);
    let bottom_left = reconstruct_leaf(bottom_left_syntax, bottom_left_predictors);

    let bottom_right_syntax = decode_following_square_syntax(
        decoder,
        &mut cdfs,
        transform_grid,
        SpatialLumaContext::Origin,
        CoefficientPolicy::BoundaryDc,
        tools,
        &mut between_leaves,
    )?;
    let bottom_right = reconstruct_boundary_leaf(bottom_right_syntax, &top_right, &bottom_left);

    let coded_planes = [
        compose_square_plane(
            &top_left.planes[0],
            &top_right.planes[0],
            &bottom_left.planes[0],
            &bottom_right.planes[0],
        ),
        compose_square_plane(
            &top_left.planes[1],
            &top_right.planes[1],
            &bottom_left.planes[1],
            &bottom_right.planes[1],
        ),
        compose_square_plane(
            &top_left.planes[2],
            &top_right.planes[2],
            &bottom_left.planes[2],
            &bottom_right.planes[2],
        ),
    ];
    let planes = coded_planes
        .each_ref()
        .map(|plane| visible_plane(plane, 16, width, height));
    Some(FirstLeaf {
        width,
        height,
        planes,
        #[cfg(coverage)]
        entropy_operations: Vec::new(),
    })
}

#[cfg(coverage)]
#[coverage(off)]
pub(super) fn __coverage_exercise_private_branches() {
    let invalid_input = [0_u8; 64];
    let invalid_spans = [super::super::samples::ByteSpan {
        start: 0,
        end: invalid_input.len(),
    }];
    let invalid_data =
        super::bit_reader::SegmentedData::new(&invalid_input, &invalid_spans).unwrap();
    let mut invalid_decoder =
        RangeDecoder::new(&invalid_data, 0, invalid_input.len(), false).unwrap();
    let (_, _, invalid_filter_intra_cdf) = TransformGrid::Square8.properties();
    let mut invalid_cdfs = BlockCdfs::defaults(invalid_filter_intra_cdf);
    let _ = decode_boundary_coefficients(&mut invalid_decoder, 0, &mut invalid_cdfs, 1, 2);
    for fill in 0..=u8::MAX {
        let input = [fill; 64];
        let spans = [super::super::samples::ByteSpan {
            start: 0,
            end: input.len(),
        }];
        let data = super::bit_reader::SegmentedData::new(&input, &spans).unwrap();
        let mut decoder = RangeDecoder::new(&data, 0, input.len(), false).unwrap();
        let _ = read_golomb(&mut decoder);
    }
}
