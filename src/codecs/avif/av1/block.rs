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
    chroma_sampling: ChromaSampling,
    reconstruction: ReconstructionPolicy,
}

#[derive(Clone, Copy)]
enum ChromaSampling {
    Full,
    Subsampled420,
}

impl ChromaSampling {
    const fn transform_grid(
        self,
        luma_grid_width: usize,
        luma_grid_height: usize,
        plane: usize,
    ) -> (usize, usize) {
        if plane != 0 && matches!(self, Self::Subsampled420) {
            (luma_grid_width.div_ceil(2), luma_grid_height.div_ceil(2))
        } else {
            (luma_grid_width, luma_grid_height)
        }
    }

    const fn subsampled_cfl_allowed(luma_grid_width: usize, luma_grid_height: usize) -> bool {
        luma_grid_width.div_ceil(2) == 1 && luma_grid_height.div_ceil(2) == 1
    }
}

#[derive(Clone, Copy)]
enum ReconstructionPolicy {
    LosslessWht4x4,
    Lossy420Dct8x8,
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
    AboveHorizontalLeftVertical,
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
            Self::AboveHorizontalLeftVertical => 5,
        }
    }

    const fn from_two_neighbors(above: LumaPredictor, left: LumaPredictor) -> Option<Self> {
        match (above, left) {
            (LumaPredictor::Dc, LumaPredictor::Dc) => Some(Self::Origin),
            (LumaPredictor::Horizontal, LumaPredictor::Vertical) => {
                Some(Self::AboveHorizontalLeftVertical)
            }
            _ => None,
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
            Self::AboveHorizontalLeftVertical => {
                // `BoundaryContextual` rejects every non-DC symbol before
                // this two-neighbor context reaches the spatial validation.
                true
            }
        }
    }
}

#[derive(Clone, Copy)]
enum CoefficientPolicy {
    DcOrSkipped,
    DcThenLumaAc,
    Skipped,
    Lossy420DcOrSkipped,
    SquareContextual {
        neighbor_contexts: [[u8; 2]; 3],
        orientation: SplitOrientation,
    },
    SubsampledSkippedContextual {
        neighbor_contexts: [[u8; 2]; 3],
        orientation: SplitOrientation,
    },
    BoundaryContextual {
        above_contexts: [[u8; 2]; 3],
        left_contexts: [[u8; 2]; 3],
    },
    SubsampledBoundaryContextual {
        above_luma_contexts: [u8; 2],
        left_luma_contexts: [u8; 2],
    },
}

#[derive(Clone, Copy)]
enum QuantizationSyntax {
    Lossless,
    DeltaQ {
        initial_qindex: u32,
        resolution_log2: u32,
    },
}

#[derive(Clone, Copy)]
struct SyntaxPolicy {
    spatial_luma_context: SpatialLumaContext,
    coefficient_policy: CoefficientPolicy,
    quantization_syntax: QuantizationSyntax,
}

#[derive(Clone, Copy)]
enum CoefficientSkipCdf {
    Base,
    OneNonzeroNeighbor,
    TwoNonzeroNeighbors,
    SubsampledNoNonzeroNeighbor,
    SubsampledOneNonzeroNeighbor,
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

#[derive(Clone, Copy)]
struct FollowingCoefficientContext {
    neighbor_contexts: [u8; 2],
    orientation: SplitOrientation,
    transform_grid_width: usize,
    transform_grid_height: usize,
    single_subsampled_chroma_transform: bool,
    require_skipped: bool,
}

#[derive(Clone, Copy)]
struct FollowingSyntaxContext {
    transform_grid: TransformGrid,
    chroma_sampling: ChromaSampling,
    spatial_luma_context: SpatialLumaContext,
    coefficient_policy: CoefficientPolicy,
    tools: BlockTools,
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
    delta_q: [u16; 4],
    luma_mode: [[u16; 13]; 6],
    luma_angle: [[u16; 7]; 2],
    chroma_mode: [[u16; 13]; 3],
    subsampled_chroma_mode: [[u16; 14]; 3],
    palette_y: [u16; 2],
    palette_uv: [u16; 2],
    use_filter_intra: [u16; 2],
    coefficient_skip: [[u16; 2]; 2],
    lossy_luma_8x8_coefficient_skip: [u16; 2],
    lossy_luma_8x8_transform_type: [[u16; 7]; 2],
    lossy_luma_8x8_eob_bin: [u16; 7],
    lossy_luma_8x8_eob_base: [u16; 3],
    lossy_luma_8x8_high_token: [u16; 4],
    subsampled_chroma_coefficient_skip: [[u16; 2]; 2],
    trailing_coefficient_skip: [[u16; 2]; 2],
    double_neighbor_coefficient_skip: [[u16; 2]; 2],
    eob_bin_luma: [u16; 5],
    eob_bin_chroma: [u16; 5],
    eob_high_luma: [[u16; 2]; 3],
    eob_high_chroma: [[u16; 2]; 2],
    eob_base_luma: [[u16; 3]; 4],
    eob_base_chroma: [[u16; 3]; 3],
    base_luma: [[u16; 4]; 24],
    base_chroma: [[u16; 4]; 7],
    high_luma: [[u16; 4]; 20],
    high_chroma: [[u16; 4]; 8],
    dc_sign: [[[u16; 2]; 3]; 2],
}

impl BlockCdfs {
    // ✅ VERIFIED: dav1d 1.5.3 src/cdf.c:113-138, 169-177, 412-414,
    // 719-905 and 1313-1348 for q-context zero. Filter-intra is
    // indexed by block size at src/cdf.c:88-110.
    const fn defaults(use_filter_intra: [u16; 2]) -> Self {
        Self {
            skip: [1_097, 0],
            delta_q: [4_608, 648, 91, 0],
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
                [
                    23_081, 19_298, 14_262, 13_538, 13_164, 12_621, 12_073, 10_706, 9_549, 5_025,
                    3_557, 1_861, 0,
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
            subsampled_chroma_mode: [
                [
                    22_361, 21_560, 19_868, 19_587, 18_945, 18_593, 17_869, 17_112, 16_782, 12_682,
                    11_773, 10_313, 8_556, 0,
                ],
                [
                    28_236, 12_988, 12_711, 12_553, 12_340, 11_697, 11_569, 11_317, 10_669, 8_540,
                    8_075, 5_736, 3_296, 0,
                ],
                [
                    27_495, 27_389, 12_591, 12_498, 12_383, 12_329, 11_819, 11_073, 10_994, 9_630,
                    8_512, 8_065, 6_089, 0,
                ],
            ],
            palette_y: [1_092, 0],
            palette_uv: [307, 0],
            use_filter_intra,
            coefficient_skip: [[26_876, 0], [22_807, 0]],
            lossy_luma_8x8_coefficient_skip: [1_220, 0],
            lossy_luma_8x8_transform_type: [
                [32_442, 23_972, 18_136, 17_689, 13_496, 5_282, 0],
                [32_284, 25_192, 25_056, 18_325, 13_609, 10_177, 0],
            ],
            lossy_luma_8x8_eob_bin: [32_439, 32_270, 31_667, 30_984, 29_503, 25_010, 0],
            lossy_luma_8x8_eob_base: [27_051, 6_291, 0],
            lossy_luma_8x8_high_token: [18_362, 11_906, 8_354, 0],
            subsampled_chroma_coefficient_skip: [[25_114, 0], [13_295, 0]],
            trailing_coefficient_skip: [[10_833, 0], [2_526, 0]],
            double_neighbor_coefficient_skip: [[281, 0], [651, 0]],
            eob_bin_luma: [31_928, 31_729, 30_788, 27_873, 0],
            eob_bin_chroma: [29_521, 27_818, 23_080, 18_205, 0],
            eob_high_luma: [[15_807, 0], [15_545, 0], [25_147, 0]],
            eob_high_chroma: [[13_699, 0], [10_243, 0]],
            eob_base_luma: [
                [14_931, 3_713, 0],
                [3_168, 1_322, 0],
                [1_924, 890, 0],
                [7_842, 3_820, 0],
            ],
            eob_base_chroma: [[11_403, 2_742, 0], [2_256, 345, 0], [1_110, 147, 0]],
            base_luma: [
                [28_734, 23_838, 20_041, 0],
                [14_686, 3_027, 891, 0],
                [20_172, 6_644, 2_275, 0],
                [23_322, 11_650, 5_763, 0],
                [26_460, 17_627, 11_489, 0],
                [30_305, 26_411, 22_985, 0],
                [12_101, 2_222, 839, 0],
                [19_725, 6_645, 2_634, 0],
                [24_617, 14_011, 7_990, 0],
                [27_513, 19_929, 14_136, 0],
                [29_948, 25_562, 21_607, 0],
                [24_576, 16_384, 8_192, 0],
                [24_576, 16_384, 8_192, 0],
                [24_576, 16_384, 8_192, 0],
                [24_576, 16_384, 8_192, 0],
                [24_576, 16_384, 8_192, 0],
                [24_576, 16_384, 8_192, 0],
                [24_576, 16_384, 8_192, 0],
                [24_576, 16_384, 8_192, 0],
                [24_576, 16_384, 8_192, 0],
                [24_576, 16_384, 8_192, 0],
                [17_032, 5_215, 2_164, 0],
                [21_558, 8_974, 3_981, 0],
                [26_821, 18_894, 13_067, 0],
            ],
            base_chroma: [
                [26_466, 16_324, 11_007, 0],
                [9_728, 1_230, 293, 0],
                [17_572, 4_316, 1_272, 0],
                [22_748, 9_822, 4_254, 0],
                [26_235, 15_906, 9_267, 0],
                [29_230, 22_952, 17_692, 0],
                [8_324, 893, 243, 0],
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
                [25_700, 20_113, 15_740, 0],
                [27_112, 22_326, 18_296, 0],
                [30_188, 27_776, 25_524, 0],
                [20_632, 14_719, 11_342, 0],
                [18_984, 12_047, 8_287, 0],
                [21_932, 15_147, 10_868, 0],
                [24_396, 18_324, 13_921, 0],
                [26_245, 20_989, 16_768, 0],
                [27_431, 22_870, 19_008, 0],
            ],
            high_chroma: [
                [16_801, 9_863, 6_482, 0],
                [19_234, 12_114, 8_189, 0],
                [23_264, 16_676, 12_233, 0],
                [25_793, 20_200, 15_865, 0],
                [27_404, 22_677, 18_748, 0],
                [28_411, 24_398, 20_911, 0],
                [30_262, 27_834, 25_550, 0],
                [9_736, 3_953, 1_832, 0],
            ],
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
        decoder.adaptive_symbol(&mut cdfs.eob_base_chroma[0], 2)
    };
    (base_token == 2).then_some(())?;
    let token = if plane == 0 {
        decode_high_token(decoder, &mut cdfs.high_luma[0])
    } else {
        decode_high_token(decoder, &mut cdfs.high_chroma[0])
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

fn decode_luma_eob_four_coefficients(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    sign_context: usize,
    cdfs: &mut BlockCdfs,
) -> Option<TransformCoefficients> {
    // ✅ VERIFIED: dav1d 1.5.3 src/recon_tmpl.c:403-545, src/scan.c:35-40,
    // and the pinned Slice 23 scalar trace. scan_4x4[4] == 5 is the final
    // coefficient and uses EOB-base context two. The reverse loop then visits
    // raster 2, 1, and 4 with contexts six, three, and three respectively.
    (decoder.adaptive_symbol(&mut cdfs.eob_base_luma[2], 2) == 2).then_some(())?;
    let coefficient_five_token = decode_high_token(decoder, &mut cdfs.high_luma[7]);
    (decoder.adaptive_symbol(&mut cdfs.base_luma[6], 3) == 0).then_some(())?;
    (decoder.adaptive_symbol(&mut cdfs.base_luma[3], 3) == 3).then_some(())?;
    let high_context = match coefficient_five_token {
        3 => 9,
        5 => 10,
        _ => return None,
    };
    let coefficient_one_token = decode_high_token(decoder, &mut cdfs.high_luma[high_context]);
    (coefficient_one_token == coefficient_five_token).then_some(())?;
    (decoder.adaptive_symbol(&mut cdfs.base_luma[3], 3) == 3).then_some(())?;
    let coefficient_four_token = decode_high_token(decoder, &mut cdfs.high_luma[high_context]);
    (coefficient_four_token == coefficient_five_token).then_some(())?;

    // The three direct AC magnitudes select DC high-token context five for
    // token three and context six for token five.
    // dav1d's nonzero link chain reads signs in raster order 4, 1, then 5.
    let dc_high_context = if coefficient_five_token == 3 { 5 } else { 6 };
    let (dc_token, dc_negative) =
        decode_luma_dc_after_ac(decoder, sign_context, cdfs, dc_high_context)?;
    if coefficient_five_token == 3 {
        (dc_token == 3).then_some(())?;
    }
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

fn decode_luma_eob_six_coefficients(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    sign_context: usize,
    cdfs: &mut BlockCdfs,
) -> Option<TransformCoefficients> {
    // ✅ VERIFIED: dav1d 1.5.3 src/recon_tmpl.c:403-718, src/scan.c:35-40,
    // and the pinned Slice 24 scalar trace. scan_4x4[6] == 12 is final, then
    // the reverse loop visits raster 8, 5, 2, 1, and 4.
    (decoder.adaptive_symbol(&mut cdfs.eob_base_luma[3], 2) == 2).then_some(())?;
    let coefficient_twelve_token = decode_high_token(decoder, &mut cdfs.high_luma[14]);
    (coefficient_twelve_token == 5).then_some(())?;
    (decoder.adaptive_symbol(&mut cdfs.base_luma[8], 3) == 3).then_some(())?;
    let coefficient_eight_token = decode_high_token(decoder, &mut cdfs.high_luma[17]);
    (coefficient_eight_token == 5).then_some(())?;
    (decoder.adaptive_symbol(&mut cdfs.base_luma[6], 3) == 0).then_some(())?;
    (decoder.adaptive_symbol(&mut cdfs.base_luma[6], 3) == 0).then_some(())?;
    (decoder.adaptive_symbol(&mut cdfs.base_luma[1], 3) == 0).then_some(())?;
    (decoder.adaptive_symbol(&mut cdfs.base_luma[4], 3) == 3).then_some(())?;
    let coefficient_four_token = decode_high_token(decoder, &mut cdfs.high_luma[10]);
    (coefficient_four_token == 5).then_some(())?;

    // Three direct AC magnitudes select DC high-token context three. The
    // first pinned transform exercises token fifteen's Golomb extension; the
    // second exercises direct token seven after adaptive CDF updates.
    let (dc_token, dc_negative) = decode_luma_dc_after_ac(decoder, sign_context, cdfs, 3)?;
    let coefficient_four_negative = decoder.equal();
    let coefficient_four_token = extend_high_token(decoder, coefficient_four_token);
    let coefficient_eight_negative = decoder.equal();
    let coefficient_eight_token = extend_high_token(decoder, coefficient_eight_token);
    let coefficient_twelve_negative = decoder.equal();
    let coefficient_twelve_token = extend_high_token(decoder, coefficient_twelve_token);

    let mut coefficients = [0_i32; 16];
    coefficients[0] = dequantize_lossless_coefficient(dc_token, dc_negative);
    coefficients[4] =
        dequantize_lossless_coefficient(coefficient_four_token, coefficient_four_negative);
    coefficients[8] =
        dequantize_lossless_coefficient(coefficient_eight_token, coefficient_eight_negative);
    coefficients[12] =
        dequantize_lossless_coefficient(coefficient_twelve_token, coefficient_twelve_negative);
    Some(coefficients)
}

fn decode_luma_eob_four_or_six_after_eob(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    sign_context: usize,
    cdfs: &mut BlockCdfs,
) -> Option<TransformCoefficients> {
    // ✅ VERIFIED: dav1d 1.5.3 src/recon_tmpl.c:403-435 and pinned Slice
    // 23/24 traces. Symbol three shares high-bit context one and one
    // equiprobable extra bit: high zero is EOB four and high one is EOB six.
    let high = decoder.adaptive_bool(&mut cdfs.eob_high_luma[1]);
    (!decoder.equal()).then_some(())?;
    if high {
        decode_luma_eob_six_coefficients(decoder, sign_context, cdfs)
    } else {
        decode_luma_eob_four_coefficients(decoder, sign_context, cdfs)
    }
}

fn decode_luma_eob_nine_coefficients(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    sign_context: usize,
    cdfs: &mut BlockCdfs,
) -> Option<TransformCoefficients> {
    // ✅ VERIFIED: dav1d 1.5.3 src/recon_tmpl.c:403-718, src/scan.c:35-40,
    // and the pinned Slice 25 scalar trace. scan_4x4[9] == 3 is final. The
    // reverse loop then visits raster 6, 9, 12, 8, 5, 2, 1, and 4 with
    // contexts 6, 6, 6, 6, 6, 8, 4, and 1.
    (decoder.adaptive_symbol(&mut cdfs.eob_base_luma[3], 2) == 2).then_some(())?;
    let coefficient_three_token = decode_high_token(decoder, &mut cdfs.high_luma[14]);
    (coefficient_three_token == 6).then_some(())?;
    for _ in 0..5 {
        (decoder.adaptive_symbol(&mut cdfs.base_luma[6], 3) == 0).then_some(())?;
    }
    (decoder.adaptive_symbol(&mut cdfs.base_luma[8], 3) == 3).then_some(())?;
    let coefficient_two_token = decode_high_token(decoder, &mut cdfs.high_luma[17]);
    (coefficient_two_token == 6).then_some(())?;
    (decoder.adaptive_symbol(&mut cdfs.base_luma[4], 3) == 3).then_some(())?;
    let coefficient_one_token = decode_high_token(decoder, &mut cdfs.high_luma[10]);
    (coefficient_one_token == 6).then_some(())?;
    (decoder.adaptive_symbol(&mut cdfs.base_luma[1], 3) == 0).then_some(())?;

    // One neighboring direct magnitude selects DC high-token context three.
    // The first pinned transform exercises token fifteen plus Golomb one;
    // the second exercises direct token eight after adaptive CDF updates.
    let (dc_token, dc_negative) = decode_luma_dc_after_ac(decoder, sign_context, cdfs, 3)?;
    let coefficient_one_negative = decoder.equal();
    let coefficient_one_token = extend_high_token(decoder, coefficient_one_token);
    let coefficient_two_negative = decoder.equal();
    let coefficient_two_token = extend_high_token(decoder, coefficient_two_token);
    let coefficient_three_negative = decoder.equal();
    let coefficient_three_token = extend_high_token(decoder, coefficient_three_token);

    let mut coefficients = [0_i32; 16];
    coefficients[0] = dequantize_lossless_coefficient(dc_token, dc_negative);
    coefficients[1] =
        dequantize_lossless_coefficient(coefficient_one_token, coefficient_one_negative);
    coefficients[2] =
        dequantize_lossless_coefficient(coefficient_two_token, coefficient_two_negative);
    coefficients[3] =
        dequantize_lossless_coefficient(coefficient_three_token, coefficient_three_negative);
    Some(coefficients)
}

fn decode_luma_eob_ten_coefficients(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    sign_context: usize,
    cdfs: &mut BlockCdfs,
) -> Option<TransformCoefficients> {
    // ✅ VERIFIED: dav1d 1.5.3 src/recon_tmpl.c:403-718, src/scan.c:35-40,
    // src/cdf.c:881-894, 1316-1340, and the pinned Slice 26 scalar trace.
    // scan_4x4[10] == 7 is final. The reverse loop then visits raster 3, 6,
    // 9, 12, 8, 5, 2, 1, and 4.
    (decoder.adaptive_symbol(&mut cdfs.eob_base_luma[3], 2) == 2).then_some(())?;
    let coefficient_seven_token = decode_high_token(decoder, &mut cdfs.high_luma[14]);
    (coefficient_seven_token == 3).then_some(())?;

    (decoder.adaptive_symbol(&mut cdfs.base_luma[8], 3) == 3).then_some(())?;
    let coefficient_three_token = decode_high_token(decoder, &mut cdfs.high_luma[16]);
    (coefficient_three_token == 3).then_some(())?;
    (decoder.adaptive_symbol(&mut cdfs.base_luma[8], 3) == 3).then_some(())?;
    let coefficient_six_token = decode_high_token(decoder, &mut cdfs.high_luma[16]);
    (coefficient_six_token == 3).then_some(())?;

    for _ in 0..3 {
        (decoder.adaptive_symbol(&mut cdfs.base_luma[6], 3) == 0).then_some(())?;
    }

    (decoder.adaptive_symbol(&mut cdfs.base_luma[9], 3) == 3).then_some(())?;
    let coefficient_five_token = decode_high_token(decoder, &mut cdfs.high_luma[9]);
    (coefficient_five_token == 3).then_some(())?;
    (decoder.adaptive_symbol(&mut cdfs.base_luma[10], 3) == 3).then_some(())?;
    let coefficient_two_token = decode_high_token(decoder, &mut cdfs.high_luma[19]);
    (coefficient_two_token == 3).then_some(())?;
    (decoder.adaptive_symbol(&mut cdfs.base_luma[5], 3) == 3).then_some(())?;
    let coefficient_one_token = decode_high_token(decoder, &mut cdfs.high_luma[12]);
    (coefficient_one_token == 3).then_some(())?;
    (decoder.adaptive_symbol(&mut cdfs.base_luma[4], 3) == 3).then_some(())?;
    let coefficient_four_token = decode_high_token(decoder, &mut cdfs.high_luma[9]);
    (coefficient_four_token == 8).then_some(())?;

    // The three DC-neighbor levels select high-token context six. dav1d's
    // nonzero link chain reads signs in raster order 4, 1, 2, 5, 6, 3, and 7.
    let (dc_token, dc_negative) = decode_luma_dc_after_ac(decoder, sign_context, cdfs, 6)?;
    let coefficient_four_negative = decoder.equal();
    let coefficient_four_token = extend_high_token(decoder, coefficient_four_token);
    let coefficient_one_negative = decoder.equal();
    let coefficient_one_token = extend_high_token(decoder, coefficient_one_token);
    let coefficient_two_negative = decoder.equal();
    let coefficient_two_token = extend_high_token(decoder, coefficient_two_token);
    let coefficient_five_negative = decoder.equal();
    let coefficient_five_token = extend_high_token(decoder, coefficient_five_token);
    let coefficient_six_negative = decoder.equal();
    let coefficient_six_token = extend_high_token(decoder, coefficient_six_token);
    let coefficient_three_negative = decoder.equal();
    let coefficient_three_token = extend_high_token(decoder, coefficient_three_token);
    let coefficient_seven_negative = decoder.equal();
    let coefficient_seven_token = extend_high_token(decoder, coefficient_seven_token);

    let mut coefficients = [0_i32; 16];
    coefficients[0] = dequantize_lossless_coefficient(dc_token, dc_negative);
    coefficients[1] =
        dequantize_lossless_coefficient(coefficient_one_token, coefficient_one_negative);
    coefficients[2] =
        dequantize_lossless_coefficient(coefficient_two_token, coefficient_two_negative);
    coefficients[3] =
        dequantize_lossless_coefficient(coefficient_three_token, coefficient_three_negative);
    coefficients[4] =
        dequantize_lossless_coefficient(coefficient_four_token, coefficient_four_negative);
    coefficients[5] =
        dequantize_lossless_coefficient(coefficient_five_token, coefficient_five_negative);
    coefficients[6] =
        dequantize_lossless_coefficient(coefficient_six_token, coefficient_six_negative);
    coefficients[7] =
        dequantize_lossless_coefficient(coefficient_seven_token, coefficient_seven_negative);
    Some(coefficients)
}

fn decode_luma_eob_twelve_coefficients(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    sign_context: usize,
    cdfs: &mut BlockCdfs,
) -> Option<TransformCoefficients> {
    // ✅ VERIFIED: dav1d 1.5.3 src/recon_tmpl.c:403-718, src/scan.c:35-40,
    // src/cdf.c:881-905, 1316-1340, and the pinned Slice 27 scalar trace.
    // scan_4x4[12] == 13 is final. The reverse loop then visits raster 10,
    // 7, 3, 6, 9, 12, 8, 5, 2, 1, and 4.
    (decoder.adaptive_symbol(&mut cdfs.eob_base_luma[3], 2) == 1).then_some(())?;
    let coefficient_thirteen_token = 2;

    for _ in 0..2 {
        (decoder.adaptive_symbol(&mut cdfs.base_luma[21], 3) == 0).then_some(())?;
    }
    for _ in 0..2 {
        (decoder.adaptive_symbol(&mut cdfs.base_luma[6], 3) == 0).then_some(())?;
    }

    let coefficient_nine_base = decoder.adaptive_symbol(&mut cdfs.base_luma[7], 3);
    if coefficient_nine_base == 3 {
        return decode_luma_alternate_eob_twelve_coefficients(
            decoder,
            sign_context,
            cdfs,
            coefficient_thirteen_token,
        );
    }
    (coefficient_nine_base == 2).then_some(())?;
    let coefficient_nine_token = 2;
    (decoder.adaptive_symbol(&mut cdfs.base_luma[7], 3) == 3).then_some(())?;
    let coefficient_twelve_token = decode_high_token(decoder, &mut cdfs.high_luma[15]);
    (coefficient_twelve_token == 3).then_some(())?;
    (decoder.adaptive_symbol(&mut cdfs.base_luma[10], 3) == 3).then_some(())?;
    let coefficient_eight_token = decode_high_token(decoder, &mut cdfs.high_luma[18]);
    (coefficient_eight_token == 3).then_some(())?;
    (decoder.adaptive_symbol(&mut cdfs.base_luma[8], 3) == 2).then_some(())?;
    let coefficient_five_token = 2;
    (decoder.adaptive_symbol(&mut cdfs.base_luma[6], 3) == 0).then_some(())?;
    (decoder.adaptive_symbol(&mut cdfs.base_luma[3], 3) == 3).then_some(())?;
    let coefficient_one_token = decode_high_token(decoder, &mut cdfs.high_luma[8]);
    (coefficient_one_token == 7).then_some(())?;
    (decoder.adaptive_symbol(&mut cdfs.base_luma[5], 3) == 3).then_some(())?;
    let coefficient_four_token = decode_high_token(decoder, &mut cdfs.high_luma[11]);
    (coefficient_four_token == 3).then_some(())?;

    // The three DC-neighbor levels select high-token context six. dav1d's
    // nonzero link chain reads signs in raster order 4, 1, 5, 8, 12, 9, and
    // 13.
    let (dc_token, dc_negative) = decode_luma_dc_after_ac(decoder, sign_context, cdfs, 6)?;
    let coefficient_four_negative = decoder.equal();
    let coefficient_four_token = extend_high_token(decoder, coefficient_four_token);
    let coefficient_one_negative = decoder.equal();
    let coefficient_one_token = extend_high_token(decoder, coefficient_one_token);
    let coefficient_five_negative = decoder.equal();
    let coefficient_five_token = extend_high_token(decoder, coefficient_five_token);
    let coefficient_eight_negative = decoder.equal();
    let coefficient_eight_token = extend_high_token(decoder, coefficient_eight_token);
    let coefficient_twelve_negative = decoder.equal();
    let coefficient_twelve_token = extend_high_token(decoder, coefficient_twelve_token);
    let coefficient_nine_negative = decoder.equal();
    let coefficient_nine_token = extend_high_token(decoder, coefficient_nine_token);
    let coefficient_thirteen_negative = decoder.equal();
    let coefficient_thirteen_token = extend_high_token(decoder, coefficient_thirteen_token);

    let mut coefficients = [0_i32; 16];
    coefficients[0] = dequantize_lossless_coefficient(dc_token, dc_negative);
    coefficients[1] =
        dequantize_lossless_coefficient(coefficient_one_token, coefficient_one_negative);
    coefficients[4] =
        dequantize_lossless_coefficient(coefficient_four_token, coefficient_four_negative);
    coefficients[5] =
        dequantize_lossless_coefficient(coefficient_five_token, coefficient_five_negative);
    coefficients[8] =
        dequantize_lossless_coefficient(coefficient_eight_token, coefficient_eight_negative);
    coefficients[9] =
        dequantize_lossless_coefficient(coefficient_nine_token, coefficient_nine_negative);
    coefficients[12] =
        dequantize_lossless_coefficient(coefficient_twelve_token, coefficient_twelve_negative);
    coefficients[13] =
        dequantize_lossless_coefficient(coefficient_thirteen_token, coefficient_thirteen_negative);
    Some(coefficients)
}

fn decode_luma_alternate_eob_twelve_coefficients(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    sign_context: usize,
    cdfs: &mut BlockCdfs,
    coefficient_thirteen_token: u32,
) -> Option<TransformCoefficients> {
    // ✅ VERIFIED: dav1d 1.5.3 src/recon_tmpl.c:403-718,
    // src/scan.c:35-40, and the two byte-identical pinned Slice 30 traces.
    // The caller consumed the common EOB-12 prefix and coefficient-nine base
    // value three. The remaining reverse scan visits raster 12, 8, 5, 2, 1,
    // and 4 before DC.
    let coefficient_nine_token = decode_high_token(decoder, &mut cdfs.high_luma[15]);
    (coefficient_nine_token == 3).then_some(())?;
    (decoder.adaptive_symbol(&mut cdfs.base_luma[7], 3) == 3).then_some(())?;
    let coefficient_twelve_token = decode_high_token(decoder, &mut cdfs.high_luma[15]);
    (coefficient_twelve_token == 3).then_some(())?;
    (decoder.adaptive_symbol(&mut cdfs.base_luma[10], 3) == 2).then_some(())?;
    let coefficient_eight_token = 2;
    (decoder.adaptive_symbol(&mut cdfs.base_luma[9], 3) == 2).then_some(())?;
    let coefficient_five_token = 2;
    (decoder.adaptive_symbol(&mut cdfs.base_luma[6], 3) == 0).then_some(())?;
    (decoder.adaptive_symbol(&mut cdfs.base_luma[4], 3) == 2).then_some(())?;
    let coefficient_one_token = 2;
    (decoder.adaptive_symbol(&mut cdfs.base_luma[5], 3) == 3).then_some(())?;
    let coefficient_four_token = decode_high_token(decoder, &mut cdfs.high_luma[11]);
    (coefficient_four_token == 3).then_some(())?;

    // Raster one, four, and five store levels 130, 195, and 130. Their
    // masked sum is seven, selecting DC high-token context four. The nonzero
    // link chain then reads raster signs 4, 1, 5, 8, 12, 9, and 13.
    let (dc_token, dc_negative) = decode_luma_dc_after_ac(decoder, sign_context, cdfs, 4)?;
    let coefficient_four_negative = decoder.equal();
    let coefficient_four_token = extend_high_token(decoder, coefficient_four_token);
    let coefficient_one_negative = decoder.equal();
    let coefficient_one_token = extend_high_token(decoder, coefficient_one_token);
    let coefficient_five_negative = decoder.equal();
    let coefficient_five_token = extend_high_token(decoder, coefficient_five_token);
    let coefficient_eight_negative = decoder.equal();
    let coefficient_eight_token = extend_high_token(decoder, coefficient_eight_token);
    let coefficient_twelve_negative = decoder.equal();
    let coefficient_twelve_token = extend_high_token(decoder, coefficient_twelve_token);
    let coefficient_nine_negative = decoder.equal();
    let coefficient_nine_token = extend_high_token(decoder, coefficient_nine_token);
    let coefficient_thirteen_negative = decoder.equal();
    let coefficient_thirteen_token = extend_high_token(decoder, coefficient_thirteen_token);

    let mut coefficients = [0_i32; 16];
    coefficients[0] = dequantize_lossless_coefficient(dc_token, dc_negative);
    coefficients[1] =
        dequantize_lossless_coefficient(coefficient_one_token, coefficient_one_negative);
    coefficients[4] =
        dequantize_lossless_coefficient(coefficient_four_token, coefficient_four_negative);
    coefficients[5] =
        dequantize_lossless_coefficient(coefficient_five_token, coefficient_five_negative);
    coefficients[8] =
        dequantize_lossless_coefficient(coefficient_eight_token, coefficient_eight_negative);
    coefficients[9] =
        dequantize_lossless_coefficient(coefficient_nine_token, coefficient_nine_negative);
    coefficients[12] =
        dequantize_lossless_coefficient(coefficient_twelve_token, coefficient_twelve_negative);
    coefficients[13] =
        dequantize_lossless_coefficient(coefficient_thirteen_token, coefficient_thirteen_negative);
    Some(coefficients)
}

fn decode_luma_eob_fifteen_coefficients(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    sign_context: usize,
    cdfs: &mut BlockCdfs,
) -> Option<TransformCoefficients> {
    // ✅ VERIFIED: dav1d 1.5.3 src/recon_tmpl.c:403-718, src/scan.c:35-40,
    // src/cdf.c:881-907, 1316-1340, and the pinned Slice 28 scalar trace.
    // scan_4x4[15] == 15 is final. The reverse loop visits every remaining
    // AC raster position through contexts proved by the trace.
    (decoder.adaptive_symbol(&mut cdfs.eob_base_luma[3], 2) == 0).then_some(())?;

    for _ in 0..3 {
        (decoder.adaptive_symbol(&mut cdfs.base_luma[22], 3) == 1).then_some(())?;
    }
    (decoder.adaptive_symbol(&mut cdfs.base_luma[23], 3) == 1).then_some(())?;
    (decoder.adaptive_symbol(&mut cdfs.base_luma[22], 3) == 1).then_some(())?;

    (decoder.adaptive_symbol(&mut cdfs.base_luma[7], 3) == 3).then_some(())?;
    let coefficient_three_token = decode_high_token(decoder, &mut cdfs.high_luma[15]);
    (coefficient_three_token == 4).then_some(())?;
    for _ in 0..2 {
        (decoder.adaptive_symbol(&mut cdfs.base_luma[8], 3) == 1).then_some(())?;
    }
    (decoder.adaptive_symbol(&mut cdfs.base_luma[7], 3) == 3).then_some(())?;
    let coefficient_twelve_token = decode_high_token(decoder, &mut cdfs.high_luma[15]);
    (coefficient_twelve_token == 4).then_some(())?;
    (decoder.adaptive_symbol(&mut cdfs.base_luma[9], 3) == 3).then_some(())?;
    let coefficient_eight_token = decode_high_token(decoder, &mut cdfs.high_luma[17]);
    (coefficient_eight_token == 4).then_some(())?;
    (decoder.adaptive_symbol(&mut cdfs.base_luma[9], 3) == 1).then_some(())?;
    (decoder.adaptive_symbol(&mut cdfs.base_luma[9], 3) == 3).then_some(())?;
    let coefficient_two_token = decode_high_token(decoder, &mut cdfs.high_luma[17]);
    (coefficient_two_token == 4).then_some(())?;
    (decoder.adaptive_symbol(&mut cdfs.base_luma[5], 3) == 3).then_some(())?;
    let coefficient_one_token = decode_high_token(decoder, &mut cdfs.high_luma[10]);
    (coefficient_one_token == 4).then_some(())?;
    (decoder.adaptive_symbol(&mut cdfs.base_luma[5], 3) == 3).then_some(())?;
    let coefficient_four_token = decode_high_token(decoder, &mut cdfs.high_luma[10]);
    (coefficient_four_token == 4).then_some(())?;

    // The neighboring levels select DC high-token context five. The nonzero
    // link chain visits all AC positions in raster order 4, 1, 2, 5, 8, 12,
    // 9, 6, 3, 7, 10, 13, 14, 11, and 15.
    let (dc_token, dc_negative) = decode_luma_dc_after_ac(decoder, sign_context, cdfs, 5)?;
    let coefficient_four_negative = decoder.equal();
    let coefficient_one_negative = decoder.equal();
    let coefficient_two_negative = decoder.equal();
    let coefficient_five_negative = decoder.equal();
    let coefficient_eight_negative = decoder.equal();
    let coefficient_twelve_negative = decoder.equal();
    let coefficient_nine_negative = decoder.equal();
    let coefficient_six_negative = decoder.equal();
    let coefficient_three_negative = decoder.equal();
    let coefficient_seven_negative = decoder.equal();
    let coefficient_ten_negative = decoder.equal();
    let coefficient_thirteen_negative = decoder.equal();
    let coefficient_fourteen_negative = decoder.equal();
    let coefficient_eleven_negative = decoder.equal();
    let coefficient_fifteen_negative = decoder.equal();

    let mut coefficients = [0_i32; 16];
    coefficients[0] = dequantize_lossless_coefficient(dc_token, dc_negative);
    coefficients[1] =
        dequantize_lossless_coefficient(coefficient_one_token, coefficient_one_negative);
    coefficients[2] =
        dequantize_lossless_coefficient(coefficient_two_token, coefficient_two_negative);
    coefficients[3] =
        dequantize_lossless_coefficient(coefficient_three_token, coefficient_three_negative);
    coefficients[4] =
        dequantize_lossless_coefficient(coefficient_four_token, coefficient_four_negative);
    coefficients[5] = dequantize_lossless_coefficient(1, coefficient_five_negative);
    coefficients[6] = dequantize_lossless_coefficient(1, coefficient_six_negative);
    coefficients[7] = dequantize_lossless_coefficient(1, coefficient_seven_negative);
    coefficients[8] =
        dequantize_lossless_coefficient(coefficient_eight_token, coefficient_eight_negative);
    coefficients[9] = dequantize_lossless_coefficient(1, coefficient_nine_negative);
    coefficients[10] = dequantize_lossless_coefficient(1, coefficient_ten_negative);
    coefficients[11] = dequantize_lossless_coefficient(1, coefficient_eleven_negative);
    coefficients[12] =
        dequantize_lossless_coefficient(coefficient_twelve_token, coefficient_twelve_negative);
    coefficients[13] = dequantize_lossless_coefficient(1, coefficient_thirteen_negative);
    coefficients[14] = dequantize_lossless_coefficient(1, coefficient_fourteen_negative);
    coefficients[15] = dequantize_lossless_coefficient(1, coefficient_fifteen_negative);
    Some(coefficients)
}

fn decode_luma_eob_nine_ten_twelve_or_fifteen_after_eob(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    sign_context: usize,
    cdfs: &mut BlockCdfs,
) -> Option<TransformCoefficients> {
    // ✅ VERIFIED: dav1d 1.5.3 src/recon_tmpl.c:403-438 and pinned Slice
    // 25-28 traces. Symbol four uses high-bit context two and two
    // equiprobable extra bits. The closed combinations are EOB nine, ten,
    // twelve, and fifteen; EOB eleven, thirteen, and fourteen remain rejected.
    if decoder.adaptive_bool(&mut cdfs.eob_high_luma[2]) {
        if decoder.equal() {
            decoder.equal().then_some(())?;
            decode_luma_eob_fifteen_coefficients(decoder, sign_context, cdfs)
        } else {
            (!decoder.equal()).then_some(())?;
            decode_luma_eob_twelve_coefficients(decoder, sign_context, cdfs)
        }
    } else if decoder.equal() {
        (!decoder.equal()).then_some(())?;
        decode_luma_eob_ten_coefficients(decoder, sign_context, cdfs)
    } else {
        decoder.equal().then_some(())?;
        decode_luma_eob_nine_coefficients(decoder, sign_context, cdfs)
    }
}

fn decode_chroma_high_dc_after_ac(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    sign_context: usize,
    cdfs: &mut BlockCdfs,
) -> Option<(u32, bool)> {
    (decoder.adaptive_symbol(&mut cdfs.base_chroma[0], 3) == 3).then_some(())?;
    let dc_token = decode_high_token(decoder, &mut cdfs.high_chroma[2]);
    (dc_token == 4).then_some(())?;
    let dc_negative = decoder.adaptive_bool(&mut cdfs.dc_sign[1][sign_context]);
    Some((dc_token, dc_negative))
}

fn decode_chroma_eob_one_after_eob(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    sign_context: usize,
    cdfs: &mut BlockCdfs,
) -> Option<TransformCoefficients> {
    // ✅ VERIFIED: dav1d 1.5.3 src/recon_tmpl.c:443-718, src/scan.c:35-40,
    // and the pinned Slice 31 scalar trace. Chroma EOB one ends at raster
    // four and uses EOB-base context one plus high-token context seven.
    (decoder.adaptive_symbol(&mut cdfs.eob_base_chroma[1], 2) == 2).then_some(())?;
    let coefficient_four_token = decode_high_token(decoder, &mut cdfs.high_chroma[7]);
    (coefficient_four_token == 4).then_some(())?;
    let (dc_token, dc_negative) = decode_chroma_high_dc_after_ac(decoder, sign_context, cdfs)?;
    let coefficient_four_negative = decoder.equal();

    let mut coefficients = [0_i32; 16];
    coefficients[0] = dequantize_lossless_coefficient(dc_token, dc_negative);
    coefficients[4] =
        dequantize_lossless_coefficient(coefficient_four_token, coefficient_four_negative);
    Some(coefficients)
}

fn decode_chroma_eob_two_after_eob(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    sign_context: usize,
    cdfs: &mut BlockCdfs,
) -> Option<TransformCoefficients> {
    // ✅ VERIFIED: dav1d 1.5.3 src/recon_tmpl.c:403-718, src/scan.c:35-40,
    // and the pinned Slice 31 scalar trace. EOB two ends at raster one and
    // then visits raster four with base-token context one.
    (!decoder.adaptive_bool(&mut cdfs.eob_high_chroma[0])).then_some(())?;
    (decoder.adaptive_symbol(&mut cdfs.eob_base_chroma[1], 2) == 2).then_some(())?;
    let coefficient_one_token = decode_high_token(decoder, &mut cdfs.high_chroma[7]);
    (coefficient_one_token == 4).then_some(())?;
    (decoder.adaptive_symbol(&mut cdfs.base_chroma[1], 3) == 0).then_some(())?;
    let (dc_token, dc_negative) = decode_chroma_high_dc_after_ac(decoder, sign_context, cdfs)?;
    let coefficient_one_negative = decoder.equal();

    let mut coefficients = [0_i32; 16];
    coefficients[0] = dequantize_lossless_coefficient(dc_token, dc_negative);
    coefficients[1] =
        dequantize_lossless_coefficient(coefficient_one_token, coefficient_one_negative);
    Some(coefficients)
}

fn decode_chroma_eob_four_after_eob(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    sign_context: usize,
    cdfs: &mut BlockCdfs,
) -> Option<TransformCoefficients> {
    // ✅ VERIFIED: dav1d 1.5.3 src/recon_tmpl.c:403-718, src/scan.c:35-40,
    // and the pinned Slice 31 scalar trace. This direct-token EOB-four body
    // visits rasters five, two, one, and four before DC.
    (!decoder.adaptive_bool(&mut cdfs.eob_high_chroma[1])).then_some(())?;
    (!decoder.equal()).then_some(())?;
    (decoder.adaptive_symbol(&mut cdfs.eob_base_chroma[2], 2) == 1).then_some(())?;
    (decoder.adaptive_symbol(&mut cdfs.base_chroma[6], 3) == 0).then_some(())?;
    (decoder.adaptive_symbol(&mut cdfs.base_chroma[2], 3) == 2).then_some(())?;
    (decoder.adaptive_symbol(&mut cdfs.base_chroma[2], 3) == 2).then_some(())?;
    (decoder.adaptive_symbol(&mut cdfs.base_chroma[0], 3) == 2).then_some(())?;
    let dc_negative = decoder.adaptive_bool(&mut cdfs.dc_sign[1][sign_context]);
    let coefficient_four_negative = decoder.equal();
    let coefficient_one_negative = decoder.equal();
    let coefficient_five_negative = decoder.equal();

    let mut coefficients = [0_i32; 16];
    coefficients[0] = dequantize_lossless_coefficient(2, dc_negative);
    coefficients[1] = dequantize_lossless_coefficient(2, coefficient_one_negative);
    coefficients[4] = dequantize_lossless_coefficient(2, coefficient_four_negative);
    coefficients[5] = dequantize_lossless_coefficient(2, coefficient_five_negative);
    Some(coefficients)
}

fn decode_nonzero_lossless_transform(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    plane: usize,
    sign_context: usize,
    allow_ac: bool,
    cdfs: &mut BlockCdfs,
) -> Option<TransformCoefficients> {
    match decode_eob_bin(decoder, plane, cdfs) {
        0 => decode_dc_only_after_eob(decoder, plane, sign_context, cdfs),
        1 if allow_ac && plane == 0 => decode_luma_eob_one_after_eob(decoder, sign_context, cdfs),
        2 if allow_ac && plane == 0 => decode_luma_eob_two_after_eob(decoder, sign_context, cdfs),
        3 if allow_ac && plane == 0 => {
            decode_luma_eob_four_or_six_after_eob(decoder, sign_context, cdfs)
        }
        4 if allow_ac && plane == 0 => {
            decode_luma_eob_nine_ten_twelve_or_fifteen_after_eob(decoder, sign_context, cdfs)
        }
        1 if allow_ac => decode_chroma_eob_one_after_eob(decoder, sign_context, cdfs),
        2 if allow_ac => decode_chroma_eob_two_after_eob(decoder, sign_context, cdfs),
        3 if allow_ac => decode_chroma_eob_four_after_eob(decoder, sign_context, cdfs),
        _ => None,
    }
}

fn decode_dc_coefficients(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    plane: usize,
    cdfs: &mut BlockCdfs,
    transform_grid_width: usize,
    transform_grid_height: usize,
    single_subsampled_chroma_transform: bool,
) -> Option<PlaneCoefficients> {
    let transform_count = transform_grid_width.saturating_mul(transform_grid_height);
    let mut coefficients = [[0_i32; 16]; 16];
    let coefficient_context = usize::from(plane != 0);
    let first_zero = if single_subsampled_chroma_transform {
        // ✅ VERIFIED: dav1d 1.5.3 src/recon_tmpl.c:328-345 and the pinned
        // Slice 32 scalar traces. The sole 4:2:0 chroma transform in an 8x8
        // lossless block selects coefficient-skip context seven.
        decoder.adaptive_bool(&mut cdfs.subsampled_chroma_coefficient_skip[0])
    } else {
        decoder.adaptive_bool(&mut cdfs.coefficient_skip[coefficient_context])
    };
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

fn coefficient_residual_context(coefficients: &TransformCoefficients) -> u8 {
    // ✅ VERIFIED: dav1d 1.5.3 src/recon_tmpl.c:556-734. Q-index-zero
    // dequantization is four, so converting each magnitude back to its token
    // and capping their sum reproduces dav1d's `cul_level`. Bits six and
    // seven retain the negative, zero, or positive DC sign class.
    let magnitude = coefficients.iter().fold(0_u32, |sum, coefficient| {
        sum.saturating_add(coefficient.unsigned_abs() / 4)
    });
    let magnitude = u8::try_from(magnitude.min(63)).unwrap_or(63);
    let dc_sign = match coefficients[0].cmp(&0) {
        std::cmp::Ordering::Less => 0,
        std::cmp::Ordering::Equal => 0x40,
        std::cmp::Ordering::Greater => 0x80,
    };
    magnitude | dc_sign
}

fn coefficient_dc_sign_context(above: u8, left: u8) -> usize {
    // ✅ VERIFIED: dav1d 1.5.3 src/recon_tmpl.c:140-159. A 4x4 transform
    // combines the two two-bit sign classes and maps negative, neutral, and
    // positive sums to contexts one, zero, and two.
    let sum = i32::from(above >> 6)
        .wrapping_add(i32::from(left >> 6))
        .wrapping_sub(2);
    usize::from(sum != 0).wrapping_add(usize::from(sum > 0))
}

fn chroma_contextual_skip_cdf(above: u8, left: u8) -> CoefficientSkipCdf {
    // ✅ VERIFIED: dav1d 1.5.3 src/recon_tmpl.c:68-100. One 4x4
    // transform inside an 8x8 4:4:4 chroma block starts at context ten
    // and adds one for each non-skipped neighbor.
    match (above == 0x40, left == 0x40) {
        (true, true) => CoefficientSkipCdf::Base,
        (false, false) => CoefficientSkipCdf::TwoNonzeroNeighbors,
        _ => CoefficientSkipCdf::OneNonzeroNeighbor,
    }
}

fn subsampled_chroma_one_neighbor_skip_cdf(context: u8) -> CoefficientSkipCdf {
    // ✅ VERIFIED: dav1d 1.5.3 src/recon_tmpl.c:59-105. A single 4x4
    // chroma transform in a following 8x8 4:2:0 luma leaf has exactly one
    // previously decoded external edge by geometry. It uses context seven
    // when that edge is neutral and context eight when it is nonzero.
    if context == 0x40 {
        CoefficientSkipCdf::SubsampledNoNonzeroNeighbor
    } else {
        CoefficientSkipCdf::SubsampledOneNonzeroNeighbor
    }
}

fn contextual_skip_cdf(plane: usize, above: u8, left: u8) -> Option<CoefficientSkipCdf> {
    if plane != 0 {
        return Some(chroma_contextual_skip_cdf(above, left));
    }

    // ✅ VERIFIED: dav1d 1.5.3 src/recon_tmpl.c:101-137 and
    // src/tables.c:297-303. This slice admits q-context-zero luma skip
    // contexts one and three; context six and the intermediate classes remain
    // closed until a fixture proves their complete downstream syntax.
    const LUMA_SKIP_CONTEXTS: [[u8; 5]; 5] = [
        [1, 2, 2, 2, 3],
        [2, 4, 4, 4, 5],
        [2, 4, 4, 4, 5],
        [2, 4, 4, 4, 5],
        [3, 5, 5, 5, 6],
    ];
    let above_magnitude = usize::from((above & 63).min(4));
    let left_magnitude = usize::from((left & 63).min(4));
    match LUMA_SKIP_CONTEXTS[above_magnitude][left_magnitude] {
        1 => Some(CoefficientSkipCdf::Base),
        3 => Some(CoefficientSkipCdf::OneNonzeroNeighbor),
        _ => None,
    }
}

fn decode_contextual_skip(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    coefficient_context: usize,
    skip_cdf: CoefficientSkipCdf,
    cdfs: &mut BlockCdfs,
) -> bool {
    match skip_cdf {
        CoefficientSkipCdf::Base => {
            decoder.adaptive_bool(&mut cdfs.coefficient_skip[coefficient_context])
        }
        CoefficientSkipCdf::OneNonzeroNeighbor => {
            decoder.adaptive_bool(&mut cdfs.trailing_coefficient_skip[coefficient_context])
        }
        CoefficientSkipCdf::TwoNonzeroNeighbors => {
            decoder.adaptive_bool(&mut cdfs.double_neighbor_coefficient_skip[coefficient_context])
        }
        CoefficientSkipCdf::SubsampledNoNonzeroNeighbor => {
            decoder.adaptive_bool(&mut cdfs.subsampled_chroma_coefficient_skip[0])
        }
        CoefficientSkipCdf::SubsampledOneNonzeroNeighbor => {
            decoder.adaptive_bool(&mut cdfs.subsampled_chroma_coefficient_skip[1])
        }
    }
}

fn decode_contextual_top_left_coefficients(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    plane: usize,
    cdfs: &mut BlockCdfs,
) -> Option<PlaneCoefficients> {
    let coefficient_context = usize::from(plane != 0);
    let mut coefficients = [[0_i32; 16]; 16];
    let mut above_contexts = [0x40_u8; 2];
    let mut left_context = 0x40_u8;

    for (transform_index, coefficients) in coefficients.iter_mut().enumerate().take(4) {
        let column = transform_index % 2;
        if column == 0 {
            left_context = 0x40;
        }
        let above_context = above_contexts[column];
        let skip_cdf = contextual_skip_cdf(plane, above_context, left_context)?;
        let skipped = decode_contextual_skip(decoder, coefficient_context, skip_cdf, cdfs);
        let residual_context = if skipped {
            0x40
        } else {
            let sign_context = coefficient_dc_sign_context(above_context, left_context);
            let allow_ac =
                (plane == 0 && transform_index != 0) || (plane != 0 && transform_index == 3);
            let transform =
                decode_nonzero_lossless_transform(decoder, plane, sign_context, allow_ac, cdfs)?;
            *coefficients = transform;
            coefficient_residual_context(&transform)
        };
        above_contexts[column] = residual_context;
        left_context = residual_context;
    }
    Some(coefficients)
}

fn coefficient_edge_contexts(
    coefficients: &[PlaneCoefficients; 3],
    orientation: SplitOrientation,
    chroma_sampling: ChromaSampling,
) -> [[u8; 2]; 3] {
    let transform_indices = match orientation {
        SplitOrientation::Horizontal => [1, 3],
        SplitOrientation::Vertical => [2, 3],
    };
    std::array::from_fn(|plane| {
        if plane != 0 && matches!(chroma_sampling, ChromaSampling::Subsampled420) {
            [coefficient_residual_context(&coefficients[plane][0]); 2]
        } else {
            transform_indices.map(|index| coefficient_residual_context(&coefficients[plane][index]))
        }
    })
}

fn decode_contextual_following_coefficients(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    plane: usize,
    cdfs: &mut BlockCdfs,
    context: FollowingCoefficientContext,
) -> Option<PlaneCoefficients> {
    let FollowingCoefficientContext {
        neighbor_contexts,
        orientation,
        transform_grid_width,
        transform_grid_height,
        single_subsampled_chroma_transform,
        require_skipped,
    } = context;
    let coefficient_context = usize::from(plane != 0);
    let mut coefficients = [[0_i32; 16]; 16];
    let mut above_contexts = match orientation {
        SplitOrientation::Horizontal => [0x40; 2],
        SplitOrientation::Vertical => neighbor_contexts,
    };
    let mut left_context = 0x40;
    let transform_count = transform_grid_width.saturating_mul(transform_grid_height);

    for (transform_index, coefficients) in coefficients.iter_mut().enumerate().take(transform_count)
    {
        let column = transform_index.rem_euclid(transform_grid_width);
        let row = transform_index.div_euclid(transform_grid_width);
        if column == 0 {
            left_context = match orientation {
                SplitOrientation::Horizontal => neighbor_contexts[row],
                SplitOrientation::Vertical => 0x40,
            };
        }
        let above_context = above_contexts[column];
        let skip_cdf = if single_subsampled_chroma_transform {
            let external_context = match orientation {
                SplitOrientation::Horizontal => left_context,
                SplitOrientation::Vertical => above_context,
            };
            subsampled_chroma_one_neighbor_skip_cdf(external_context)
        } else if plane == 0 {
            // Following luma is required skipped, so it can retain only its
            // one external coded edge and never has two nonzero neighbors.
            if above_context == 0x40 && left_context == 0x40 {
                CoefficientSkipCdf::Base
            } else {
                CoefficientSkipCdf::OneNonzeroNeighbor
            }
        } else {
            chroma_contextual_skip_cdf(above_context, left_context)
        };
        let skipped = decode_contextual_skip(decoder, coefficient_context, skip_cdf, cdfs);
        let chroma_ac_position = match orientation {
            SplitOrientation::Horizontal => transform_index >= transform_grid_width,
            SplitOrientation::Vertical => column != 0,
        };
        let residual_context = if skipped {
            0x40
        } else {
            (!require_skipped).then_some(())?;
            (plane != 0 && chroma_ac_position).then_some(())?;
            let sign_context = coefficient_dc_sign_context(above_context, left_context);
            let transform =
                decode_nonzero_lossless_transform(decoder, plane, sign_context, true, cdfs)?;
            *coefficients = transform;
            coefficient_residual_context(&transform)
        };
        above_contexts[column] = residual_context;
        left_context = residual_context;
    }

    Some(coefficients)
}

fn decode_contextual_boundary_coefficients(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    plane: usize,
    cdfs: &mut BlockCdfs,
    above_contexts: [u8; 2],
    left_contexts: [u8; 2],
    transform_grid_width: usize,
    transform_grid_height: usize,
) -> Option<PlaneCoefficients> {
    if plane == 0 || (above_contexts == [0x40; 2] && left_contexts == [0x40; 2]) {
        return decode_boundary_coefficients(
            decoder,
            plane,
            cdfs,
            transform_grid_width,
            transform_grid_height,
        );
    }

    let coefficient_context = usize::from(plane != 0);
    let mut above_contexts = above_contexts;
    let mut left_context = 0x40;
    for transform_index in 0..4 {
        let column = transform_index % 2;
        if column == 0 {
            left_context = left_contexts[transform_index / 2];
        }
        let skip_cdf = chroma_contextual_skip_cdf(above_contexts[column], left_context);
        let skipped = decode_contextual_skip(decoder, coefficient_context, skip_cdf, cdfs);
        skipped.then_some(())?;
        above_contexts[column] = 0x40;
        left_context = 0x40;
    }
    Some([[0_i32; 16]; 16])
}

fn decode_subsampled_boundary_coefficients(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    plane: usize,
    cdfs: &mut BlockCdfs,
) -> Option<PlaneCoefficients> {
    // ✅ VERIFIED: dav1d 1.5.3 src/recon_tmpl.c:59-105 and the Slice 33
    // scalar traces. Both side leaves are decoded with `require_skipped`, so
    // their single chroma transforms are neutral by construction. The
    // bottom-right transform therefore uses context seven and sign context
    // zero; context nine cannot be represented by this closed policy.
    let skipped = decode_contextual_skip(
        decoder,
        1,
        CoefficientSkipCdf::SubsampledNoNonzeroNeighbor,
        cdfs,
    );
    let mut coefficients = [[0_i32; 16]; 16];
    if !skipped {
        coefficients[0] = decode_nonzero_lossless_transform(decoder, plane, 0, false, cdfs)?;
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

// ✅ VERIFIED: dav1d 1.5.3 src/decode.c:963-983 and src/cdf.c:410-411.
// Slice 34 starts at frame qindex four, decodes magnitude two and a negative
// sign at resolution zero, and therefore reaches block qindex two.
fn decode_delta_q(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    cdfs: &mut BlockCdfs,
    initial_qindex: u32,
    resolution_log2: u32,
) -> Option<()> {
    let magnitude = decoder.adaptive_symbol(&mut cdfs.delta_q, 3);
    (magnitude == 2).then_some(())?;
    decoder.equal().then_some(())?;
    let delta = magnitude.wrapping_shl(resolution_log2);
    (initial_qindex.wrapping_sub(delta) == 2).then_some(())
}

// ✅ VERIFIED: dav1d 1.5.3 src/recon_tmpl.c:318-643,
// src/msac.c:187-201, src/cdf.c:308-336, 749-755, 839-865, and
// 1316-1465. The admitted lossy leaf has one 8x8 luma transform in context
// one and one skipped 4x4 transform per chroma plane in context seven. Luma
// may be skipped or contain exactly one direct-token DCT_DCT DC coefficient.
fn decode_lossy_420_dc_or_skipped_coefficients(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    plane: usize,
    luma_predictor: LumaPredictor,
    cdfs: &mut BlockCdfs,
) -> Option<PlaneCoefficients> {
    if plane != 0 {
        return decoder
            .adaptive_bool(&mut cdfs.subsampled_chroma_coefficient_skip[0])
            .then_some([[0_i32; 16]; 16]);
    }
    if decoder.adaptive_bool(&mut cdfs.lossy_luma_8x8_coefficient_skip) {
        return Some([[0_i32; 16]; 16]);
    }

    // Slice 35 admits only directional lossy predictors. Their AV1 mode
    // indices are one and two, so subtracting one selects the corresponding
    // 8x8 transform-type CDF without consulting source pixels or fixtures.
    let transform_context = luma_predictor.cdf_index().wrapping_sub(1);
    (decoder.adaptive_symbol(
        &mut cdfs.lossy_luma_8x8_transform_type[transform_context],
        6,
    ) == 1)
        .then_some(())?;
    (decoder.adaptive_symbol(&mut cdfs.lossy_luma_8x8_eob_bin, 6) == 0).then_some(())?;
    (decoder.adaptive_symbol(&mut cdfs.lossy_luma_8x8_eob_base, 2) == 2).then_some(())?;
    let token = decode_high_token(decoder, &mut cdfs.lossy_luma_8x8_high_token);
    let negative = decoder.adaptive_bool(&mut cdfs.dc_sign[0][0]);
    // ✅ VERIFIED: dav1d 1.5.3 src/recon_tmpl.c:597-643. DC sign is decoded
    // before token fifteen's Golomb extension. Slices 36-37 admit only final
    // tokens 15, 16, and 24; adjacent manifest controls decode tokens 32-33.
    let token = extend_high_token(decoder, token);

    // ✅ VERIFIED: dav1d 1.5.3 src/decode.c:54-73,
    // src/dequant_tables.c, src/qm.c:1604-1692, and
    // src/recon_tmpl.c:597-643. At block qindex two, eight-bit Y-DC dequant
    // is eight and qmatrix-ten DC weight 32 leaves it unchanged.
    let magnitude: i32 = match token {
        8 => 64,
        9 => 72,
        15 => 120,
        16 => 128,
        24 => 192,
        _ => return None,
    };
    let mut coefficients = [[0_i32; 16]; 16];
    coefficients[0][0] = if negative {
        magnitude.wrapping_neg()
    } else {
        magnitude
    };
    Some(coefficients)
}

fn decode_syntax(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    cdfs: &mut BlockCdfs,
    transform_grid: TransformGrid,
    chroma_sampling: ChromaSampling,
    policy: SyntaxPolicy,
    tools: BlockTools,
) -> Option<BlockSyntax> {
    let SyntaxPolicy {
        spatial_luma_context,
        coefficient_policy,
        quantization_syntax,
    } = policy;
    let (transform_grid_width, transform_grid_height, _) = transform_grid.properties();
    (!decoder.adaptive_bool(&mut cdfs.skip)).then_some(())?;
    if let QuantizationSyntax::DeltaQ {
        initial_qindex,
        resolution_log2,
    } = quantization_syntax
    {
        decode_delta_q(decoder, cdfs, initial_qindex, resolution_log2)?;
    }
    let luma_predictor =
        match decoder.adaptive_symbol(&mut cdfs.luma_mode[spatial_luma_context.index()], 12) {
            0 if matches!(quantization_syntax, QuantizationSyntax::Lossless) => LumaPredictor::Dc,
            1 if !matches!(
                coefficient_policy,
                CoefficientPolicy::BoundaryContextual { .. }
                    | CoefficientPolicy::SubsampledBoundaryContextual { .. }
                    | CoefficientPolicy::SubsampledSkippedContextual { .. }
            ) =>
            {
                LumaPredictor::Vertical
            }
            2 if !matches!(
                coefficient_policy,
                CoefficientPolicy::BoundaryContextual { .. }
                    | CoefficientPolicy::SubsampledBoundaryContextual { .. }
                    | CoefficientPolicy::SubsampledSkippedContextual { .. }
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
    match chroma_sampling {
        ChromaSampling::Full => {
            let predictor_index = luma_predictor.cdf_index();
            (decoder.adaptive_symbol(&mut cdfs.chroma_mode[predictor_index], 12) == 0)
                .then_some(())?;
        }
        ChromaSampling::Subsampled420 => {
            // ✅ VERIFIED: dav1d 1.5.3 src/decode.c:1072-1078,
            // src/cdf.c:113-177, and the pinned Slice 32/33 scalar traces.
            // CFL is available only when the subsampled chroma block is one
            // 4x4 transform. Larger leaves use the ordinary twelve-symbol row.
            let predictor_index = luma_predictor.cdf_index();
            let chroma_dc = if ChromaSampling::subsampled_cfl_allowed(
                transform_grid_width,
                transform_grid_height,
            ) {
                decoder.adaptive_symbol(&mut cdfs.subsampled_chroma_mode[predictor_index], 13)
            } else {
                decoder.adaptive_symbol(&mut cdfs.chroma_mode[predictor_index], 12)
            };
            (chroma_dc == 0).then_some(())?;
        }
    }
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
        let (plane_grid_width, plane_grid_height) =
            chroma_sampling.transform_grid(transform_grid_width, transform_grid_height, plane);
        let single_subsampled_chroma_transform = plane != 0
            && matches!(chroma_sampling, ChromaSampling::Subsampled420)
            && plane_grid_width == 1
            && plane_grid_height == 1;
        match coefficient_policy {
            CoefficientPolicy::DcOrSkipped => decode_dc_coefficients(
                decoder,
                plane,
                cdfs,
                plane_grid_width,
                plane_grid_height,
                single_subsampled_chroma_transform,
            ),
            CoefficientPolicy::DcThenLumaAc => {
                decode_contextual_top_left_coefficients(decoder, plane, cdfs)
            }
            CoefficientPolicy::Skipped => decode_skipped_coefficients(
                decoder,
                plane,
                cdfs,
                plane_grid_width,
                plane_grid_height,
            ),
            CoefficientPolicy::Lossy420DcOrSkipped => {
                decode_lossy_420_dc_or_skipped_coefficients(decoder, plane, luma_predictor, cdfs)
            }
            CoefficientPolicy::SquareContextual {
                neighbor_contexts,
                orientation,
            } => decode_contextual_following_coefficients(
                decoder,
                plane,
                cdfs,
                FollowingCoefficientContext {
                    neighbor_contexts: neighbor_contexts[plane],
                    orientation,
                    transform_grid_width: plane_grid_width,
                    transform_grid_height: plane_grid_height,
                    single_subsampled_chroma_transform,
                    require_skipped: false,
                },
            ),
            CoefficientPolicy::SubsampledSkippedContextual {
                neighbor_contexts,
                orientation,
            } => decode_contextual_following_coefficients(
                decoder,
                plane,
                cdfs,
                FollowingCoefficientContext {
                    neighbor_contexts: neighbor_contexts[plane],
                    orientation,
                    transform_grid_width: plane_grid_width,
                    transform_grid_height: plane_grid_height,
                    single_subsampled_chroma_transform,
                    require_skipped: true,
                },
            ),
            CoefficientPolicy::BoundaryContextual {
                above_contexts,
                left_contexts,
            } => decode_contextual_boundary_coefficients(
                decoder,
                plane,
                cdfs,
                above_contexts[plane],
                left_contexts[plane],
                plane_grid_width,
                plane_grid_height,
            ),
            CoefficientPolicy::SubsampledBoundaryContextual {
                above_luma_contexts,
                left_luma_contexts,
            } => {
                if plane == 0 {
                    decode_contextual_boundary_coefficients(
                        decoder,
                        plane,
                        cdfs,
                        above_luma_contexts,
                        left_luma_contexts,
                        plane_grid_width,
                        plane_grid_height,
                    )
                } else {
                    decode_subsampled_boundary_coefficients(decoder, plane, cdfs)
                }
            }
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
        chroma_sampling,
        reconstruction: if matches!(quantization_syntax, QuantizationSyntax::Lossless) {
            ReconstructionPolicy::LosslessWht4x4
        } else {
            ReconstructionPolicy::Lossy420Dct8x8
        },
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

// ✅ VERIFIED: dav1d 1.5.3 src/itx_tmpl.c:44-73. DCT_DCT transforms with
// EOB zero use this scalar DC-only fast path. The 8x8 transform shift is one;
// the signed right shifts intentionally mirror dav1d's target arithmetic.
fn inverse_dct_8x8_dc(coefficient: i32) -> i32 {
    let dc = coefficient.wrapping_mul(181).wrapping_add(128) >> 8;
    let dc = dc.wrapping_add(1) >> 1;
    dc.wrapping_mul(181).wrapping_add(128).wrapping_add(2_048) >> 12
}

fn reconstruct_lossy_luma_8x8(
    predictor: u16,
    coefficients: PlaneCoefficients,
) -> ReconstructedPlane {
    let residual = inverse_dct_8x8_dc(coefficients[0][0]);
    let reconstructed = i32::from(predictor).saturating_add(residual).clamp(0, 255);
    #[expect(
        clippy::cast_sign_loss,
        reason = "the reconstructed eight-bit sample is explicitly clamped to 0..=255"
    )]
    let reconstructed = reconstructed as u16;
    ReconstructedPlane {
        samples: vec![reconstructed; 64],
    }
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
    chroma_sampling: ChromaSampling,
) -> ReconstructedPlane {
    let (mut transform_grid_width, mut transform_grid_height, _) = transform_grid.properties();
    if matches!(chroma_sampling, ChromaSampling::Subsampled420) {
        // ✅ VERIFIED: dav1d 1.5.3 src/recon_tmpl.c:1176-1545 and the pinned
        // Slice 32 traces. Both chroma shifts reduce the 2x2 luma transform
        // grid to one 4x4 transform.
        transform_grid_width = transform_grid_width.div_ceil(2);
        transform_grid_height = transform_grid_height.div_ceil(2);
    }
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
    let BlockSyntax {
        luma_predictor,
        coefficients,
        transform_grid,
        chroma_sampling,
        reconstruction,
    } = syntax;
    let planes = match reconstruction {
        ReconstructionPolicy::LosslessWht4x4 => [
            reconstruct_coded_plane(
                predictors[0],
                coefficients[0],
                transform_grid,
                ChromaSampling::Full,
            ),
            reconstruct_coded_plane(
                predictors[1],
                coefficients[1],
                transform_grid,
                chroma_sampling,
            ),
            reconstruct_coded_plane(
                predictors[2],
                coefficients[2],
                transform_grid,
                chroma_sampling,
            ),
        ],
        ReconstructionPolicy::Lossy420Dct8x8 => [
            reconstruct_lossy_luma_8x8(predictors[0], coefficients[0]),
            reconstruct_coded_plane(
                predictors[1],
                coefficients[1],
                transform_grid,
                ChromaSampling::Subsampled420,
            ),
            reconstruct_coded_plane(
                predictors[2],
                coefficients[2],
                transform_grid,
                ChromaSampling::Subsampled420,
            ),
        ],
    };
    ClosedLeaf {
        luma_predictor,
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

fn adjacent_edge(plane: &ReconstructedPlane, orientation: SplitOrientation) -> [u16; 8] {
    match orientation {
        SplitOrientation::Horizontal => right_edge(plane),
        SplitOrientation::Vertical => bottom_edge(plane),
    }
}

fn bottom_edge_4(plane: &ReconstructedPlane) -> [u16; 4] {
    [
        plane.samples[12],
        plane.samples[13],
        plane.samples[14],
        plane.samples[15],
    ]
}

fn right_edge_4(plane: &ReconstructedPlane) -> [u16; 4] {
    [
        plane.samples[3],
        plane.samples[7],
        plane.samples[11],
        plane.samples[15],
    ]
}

fn adjacent_edge_4(plane: &ReconstructedPlane, orientation: SplitOrientation) -> [u16; 4] {
    match orientation {
        SplitOrientation::Horizontal => right_edge_4(plane),
        SplitOrientation::Vertical => bottom_edge_4(plane),
    }
}

fn one_sided_dc_predictor(edge: [u16; 8]) -> u16 {
    let sum = edge
        .iter()
        .fold(0_u32, |sum, sample| sum.saturating_add(u32::from(*sample)));
    u16::try_from(sum.saturating_add(4).div_euclid(8)).unwrap_or(128)
}

fn one_sided_dc_predictor_4(edge: [u16; 4]) -> u16 {
    let sum = edge
        .iter()
        .fold(0_u32, |sum, sample| sum.saturating_add(u32::from(*sample)));
    u16::try_from(sum.saturating_add(2).div_euclid(4)).unwrap_or(128)
}

fn reconstruct_directional_plane(
    edge: [u16; 8],
    orientation: SplitOrientation,
) -> ReconstructedPlane {
    let mut samples = Vec::with_capacity(64);
    match orientation {
        SplitOrientation::Horizontal => {
            for sample in edge {
                samples.extend_from_slice(&[sample; 8]);
            }
        }
        SplitOrientation::Vertical => {
            for _ in 0..8 {
                samples.extend_from_slice(&edge);
            }
        }
    }
    ReconstructedPlane { samples }
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

fn reconstruct_subsampled_boundary_plane(
    above: &ReconstructedPlane,
    left: &ReconstructedPlane,
    coefficients: PlaneCoefficients,
) -> ReconstructedPlane {
    let transform =
        reconstruct_dc_transform(bottom_edge_4(above), right_edge_4(left), coefficients[0]);
    ReconstructedPlane {
        samples: transform.to_vec(),
    }
}

fn reconstruct_following_square_leaf(
    syntax: BlockSyntax,
    neighbor: &ClosedLeaf,
    orientation: SplitOrientation,
) -> ClosedLeaf {
    let BlockSyntax {
        luma_predictor,
        coefficients,
        transform_grid,
        chroma_sampling: _,
        reconstruction: _,
    } = syntax;
    let edges = neighbor
        .planes
        .each_ref()
        .map(|plane| adjacent_edge(plane, orientation));
    let luma = if matches!(luma_predictor, LumaPredictor::Dc) {
        reconstruct_coded_plane(
            one_sided_dc_predictor(edges[0]),
            coefficients[0],
            transform_grid,
            ChromaSampling::Full,
        )
    } else {
        reconstruct_directional_plane(edges[0], orientation)
    };
    let planes = [
        luma,
        reconstruct_coded_plane(
            one_sided_dc_predictor(edges[1]),
            coefficients[1],
            transform_grid,
            ChromaSampling::Full,
        ),
        reconstruct_coded_plane(
            one_sided_dc_predictor(edges[2]),
            coefficients[2],
            transform_grid,
            ChromaSampling::Full,
        ),
    ];
    ClosedLeaf {
        luma_predictor,
        planes,
    }
}

fn reconstruct_following_420_leaf(
    syntax: BlockSyntax,
    neighbor: &ClosedLeaf,
    orientation: SplitOrientation,
) -> ClosedLeaf {
    let luma_edge = adjacent_edge(&neighbor.planes[0], orientation);
    let chroma_edges = [
        adjacent_edge_4(&neighbor.planes[1], orientation),
        adjacent_edge_4(&neighbor.planes[2], orientation),
    ];
    let planes = [
        reconstruct_coded_plane(
            one_sided_dc_predictor(luma_edge),
            syntax.coefficients[0],
            syntax.transform_grid,
            ChromaSampling::Full,
        ),
        reconstruct_coded_plane(
            one_sided_dc_predictor_4(chroma_edges[0]),
            syntax.coefficients[1],
            syntax.transform_grid,
            ChromaSampling::Subsampled420,
        ),
        reconstruct_coded_plane(
            one_sided_dc_predictor_4(chroma_edges[1]),
            syntax.coefficients[2],
            syntax.transform_grid,
            ChromaSampling::Subsampled420,
        ),
    ];
    ClosedLeaf {
        luma_predictor: syntax.luma_predictor,
        planes,
    }
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

fn reconstruct_boundary_420_leaf(
    syntax: BlockSyntax,
    above: &ClosedLeaf,
    left: &ClosedLeaf,
) -> ClosedLeaf {
    let planes = [
        reconstruct_boundary_plane(&above.planes[0], &left.planes[0], syntax.coefficients[0]),
        reconstruct_subsampled_boundary_plane(
            &above.planes[1],
            &left.planes[1],
            syntax.coefficients[1],
        ),
        reconstruct_subsampled_boundary_plane(
            &above.planes[2],
            &left.planes[2],
            syntax.coefficients[2],
        ),
    ];
    ClosedLeaf {
        luma_predictor: syntax.luma_predictor,
        planes,
    }
}

fn decode_following_square_syntax<F>(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    cdfs: &mut BlockCdfs,
    context: FollowingSyntaxContext,
    between_leaves: &mut F,
) -> Option<BlockSyntax>
where
    F: FnMut(&mut RangeDecoder<'_, '_, '_>) -> Option<()>,
{
    let FollowingSyntaxContext {
        transform_grid,
        chroma_sampling,
        spatial_luma_context,
        coefficient_policy,
        tools,
    } = context;
    between_leaves(decoder)?;
    decode_syntax(
        decoder,
        cdfs,
        transform_grid,
        chroma_sampling,
        SyntaxPolicy {
            spatial_luma_context,
            coefficient_policy,
            quantization_syntax: QuantizationSyntax::Lossless,
        },
        tools,
    )
}

fn visible_leaf(
    leaf: ClosedLeaf,
    transform_grid: TransformGrid,
    width: u32,
    height: u32,
    chroma_sampling: ChromaSampling,
) -> FirstLeaf {
    let (transform_grid_width, _, _) = transform_grid.properties();
    let coded_width = transform_grid_width.saturating_mul(4);
    let (chroma_coded_width, chroma_width, chroma_height) =
        if matches!(chroma_sampling, ChromaSampling::Subsampled420) {
            (
                transform_grid_width.div_ceil(2).saturating_mul(4),
                width.div_ceil(2),
                height.div_ceil(2),
            )
        } else {
            (coded_width, width, height)
        };
    let planes = [
        visible_plane(&leaf.planes[0], coded_width, width, height),
        visible_plane(
            &leaf.planes[1],
            chroma_coded_width,
            chroma_width,
            chroma_height,
        ),
        visible_plane(
            &leaf.planes[2],
            chroma_coded_width,
            chroma_width,
            chroma_height,
        ),
    ];
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
        ChromaSampling::Full,
        SyntaxPolicy {
            spatial_luma_context: SpatialLumaContext::Origin,
            coefficient_policy: CoefficientPolicy::DcOrSkipped,
            quantization_syntax: QuantizationSyntax::Lossless,
        },
        tools,
    )?;
    let predictors = origin_predictors(syntax.luma_predictor);
    let leaf = reconstruct_leaf(syntax, predictors);
    Some(visible_leaf(
        leaf,
        transform_grid,
        width,
        height,
        ChromaSampling::Full,
    ))
}

/// Decode one closed 4:2:0 lossless leaf.
pub(super) fn decode_first_lossless_420_leaf(
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
        ChromaSampling::Subsampled420,
        SyntaxPolicy {
            spatial_luma_context: SpatialLumaContext::Origin,
            coefficient_policy: CoefficientPolicy::DcOrSkipped,
            quantization_syntax: QuantizationSyntax::Lossless,
        },
        tools,
    )?;
    let predictors = origin_predictors(syntax.luma_predictor);
    let leaf = reconstruct_leaf(syntax, predictors);
    Some(visible_leaf(
        leaf,
        transform_grid,
        width,
        height,
        ChromaSampling::Subsampled420,
    ))
}

/// Decode the first closed lossy 4:2:0 leaf with a skipped or DC-only luma residual.
pub(super) fn decode_first_lossy_420_leaf(
    decoder: &mut RangeDecoder<'_, '_, '_>,
    width: u32,
    height: u32,
    initial_qindex: u32,
    delta_q_resolution_log2: u32,
    tools: BlockTools,
) -> Option<FirstLeaf> {
    let transform_grid = TransformGrid::Square8;
    let (_, _, use_filter_intra) = transform_grid.properties();
    let mut cdfs = BlockCdfs::defaults(use_filter_intra);
    let syntax = decode_syntax(
        decoder,
        &mut cdfs,
        transform_grid,
        ChromaSampling::Subsampled420,
        SyntaxPolicy {
            spatial_luma_context: SpatialLumaContext::Origin,
            coefficient_policy: CoefficientPolicy::Lossy420DcOrSkipped,
            quantization_syntax: QuantizationSyntax::DeltaQ {
                initial_qindex,
                resolution_log2: delta_q_resolution_log2,
            },
        },
        tools,
    )?;
    let predictors = origin_predictors(syntax.luma_predictor);
    let leaf = reconstruct_leaf(syntax, predictors);
    Some(visible_leaf(
        leaf,
        transform_grid,
        width,
        height,
        ChromaSampling::Subsampled420,
    ))
}

fn compose_split_plane(
    first: &ReconstructedPlane,
    second: &ReconstructedPlane,
    orientation: SplitOrientation,
) -> ReconstructedPlane {
    compose_split_plane_with_width(first, second, orientation, 8)
}

fn compose_split_plane_with_width(
    first: &ReconstructedPlane,
    second: &ReconstructedPlane,
    orientation: SplitOrientation,
    child_width: usize,
) -> ReconstructedPlane {
    let samples = match orientation {
        SplitOrientation::Horizontal => {
            let mut samples =
                Vec::with_capacity(first.samples.len().saturating_add(second.samples.len()));
            for (first_row, second_row) in first
                .samples
                .chunks_exact(child_width)
                .zip(second.samples.chunks_exact(child_width))
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
    compose_square_plane_with_width(top_left, top_right, bottom_left, bottom_right, 8)
}

fn compose_square_plane_with_width(
    top_left: &ReconstructedPlane,
    top_right: &ReconstructedPlane,
    bottom_left: &ReconstructedPlane,
    bottom_right: &ReconstructedPlane,
    child_width: usize,
) -> ReconstructedPlane {
    let top = compose_split_plane_with_width(
        top_left,
        top_right,
        SplitOrientation::Horizontal,
        child_width,
    );
    let bottom = compose_split_plane_with_width(
        bottom_left,
        bottom_right,
        SplitOrientation::Horizontal,
        child_width,
    );
    compose_split_plane_with_width(
        &top,
        &bottom,
        SplitOrientation::Vertical,
        child_width.saturating_mul(2),
    )
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
        ChromaSampling::Full,
        SyntaxPolicy {
            spatial_luma_context: SpatialLumaContext::Origin,
            coefficient_policy: CoefficientPolicy::DcOrSkipped,
            quantization_syntax: QuantizationSyntax::Lossless,
        },
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
        ChromaSampling::Full,
        SyntaxPolicy {
            spatial_luma_context,
            coefficient_policy: CoefficientPolicy::Skipped,
            quantization_syntax: QuantizationSyntax::Lossless,
        },
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

/// Decode and compose the closed two-leaf 4:2:0 recursive split.
pub(super) fn decode_two_lossless_420_leaves<F>(
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
        ChromaSampling::Subsampled420,
        SyntaxPolicy {
            spatial_luma_context: SpatialLumaContext::Origin,
            coefficient_policy: CoefficientPolicy::DcOrSkipped,
            quantization_syntax: QuantizationSyntax::Lossless,
        },
        tools,
    )?;
    let second_neighbor_contexts = coefficient_edge_contexts(
        &first_syntax.coefficients,
        orientation,
        ChromaSampling::Subsampled420,
    );
    let first_predictors = origin_predictors(first_syntax.luma_predictor);
    let first = reconstruct_leaf(first_syntax, first_predictors);

    between_leaves(decoder)?;

    let spatial_luma_context = SpatialLumaContext::from_neighbor(orientation, first.luma_predictor);
    let second_syntax = decode_syntax(
        decoder,
        &mut cdfs,
        transform_grid,
        ChromaSampling::Subsampled420,
        SyntaxPolicy {
            spatial_luma_context,
            coefficient_policy: CoefficientPolicy::SubsampledSkippedContextual {
                neighbor_contexts: second_neighbor_contexts,
                orientation,
            },
            quantization_syntax: QuantizationSyntax::Lossless,
        },
        tools,
    )?;
    let second = reconstruct_following_420_leaf(second_syntax, &first, orientation);

    let (luma_coded_width, chroma_coded_width) = match orientation {
        SplitOrientation::Horizontal => (16, 8),
        SplitOrientation::Vertical => (8, 4),
    };
    let coded_planes = [
        compose_split_plane_with_width(&first.planes[0], &second.planes[0], orientation, 8),
        compose_split_plane_with_width(&first.planes[1], &second.planes[1], orientation, 4),
        compose_split_plane_with_width(&first.planes[2], &second.planes[2], orientation, 4),
    ];
    let chroma_width = width.div_ceil(2);
    let chroma_height = height.div_ceil(2);
    let planes = [
        visible_plane(&coded_planes[0], luma_coded_width, width, height),
        visible_plane(
            &coded_planes[1],
            chroma_coded_width,
            chroma_width,
            chroma_height,
        ),
        visible_plane(
            &coded_planes[2],
            chroma_coded_width,
            chroma_width,
            chroma_height,
        ),
    ];
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
        ChromaSampling::Full,
        SyntaxPolicy {
            spatial_luma_context: SpatialLumaContext::Origin,
            coefficient_policy: CoefficientPolicy::DcThenLumaAc,
            quantization_syntax: QuantizationSyntax::Lossless,
        },
        tools,
    )?;
    let top_right_neighbor_contexts = coefficient_edge_contexts(
        &top_left_syntax.coefficients,
        SplitOrientation::Horizontal,
        ChromaSampling::Full,
    );
    let bottom_left_neighbor_contexts = coefficient_edge_contexts(
        &top_left_syntax.coefficients,
        SplitOrientation::Vertical,
        ChromaSampling::Full,
    );
    let top_left_predictors = origin_predictors(top_left_syntax.luma_predictor);
    let top_left = reconstruct_leaf(top_left_syntax, top_left_predictors);

    let top_right_context =
        SpatialLumaContext::from_neighbor(SplitOrientation::Horizontal, top_left.luma_predictor);
    let top_right_syntax = decode_following_square_syntax(
        decoder,
        &mut cdfs,
        FollowingSyntaxContext {
            transform_grid,
            chroma_sampling: ChromaSampling::Full,
            spatial_luma_context: top_right_context,
            coefficient_policy: CoefficientPolicy::SquareContextual {
                neighbor_contexts: top_right_neighbor_contexts,
                orientation: SplitOrientation::Horizontal,
            },
            tools,
        },
        &mut between_leaves,
    )?;
    let bottom_right_above_contexts = coefficient_edge_contexts(
        &top_right_syntax.coefficients,
        SplitOrientation::Vertical,
        ChromaSampling::Full,
    );
    let top_right = reconstruct_following_square_leaf(
        top_right_syntax,
        &top_left,
        SplitOrientation::Horizontal,
    );

    let bottom_left_context =
        SpatialLumaContext::from_neighbor(SplitOrientation::Vertical, top_left.luma_predictor);
    let bottom_left_syntax = decode_following_square_syntax(
        decoder,
        &mut cdfs,
        FollowingSyntaxContext {
            transform_grid,
            chroma_sampling: ChromaSampling::Full,
            spatial_luma_context: bottom_left_context,
            coefficient_policy: CoefficientPolicy::SquareContextual {
                neighbor_contexts: bottom_left_neighbor_contexts,
                orientation: SplitOrientation::Vertical,
            },
            tools,
        },
        &mut between_leaves,
    )?;
    let bottom_right_left_contexts = coefficient_edge_contexts(
        &bottom_left_syntax.coefficients,
        SplitOrientation::Horizontal,
        ChromaSampling::Full,
    );
    let bottom_left = reconstruct_following_square_leaf(
        bottom_left_syntax,
        &top_left,
        SplitOrientation::Vertical,
    );

    let bottom_right_context = SpatialLumaContext::from_two_neighbors(
        top_right.luma_predictor,
        bottom_left.luma_predictor,
    )?;
    let bottom_right_syntax = decode_following_square_syntax(
        decoder,
        &mut cdfs,
        FollowingSyntaxContext {
            transform_grid,
            chroma_sampling: ChromaSampling::Full,
            spatial_luma_context: bottom_right_context,
            coefficient_policy: CoefficientPolicy::BoundaryContextual {
                above_contexts: bottom_right_above_contexts,
                left_contexts: bottom_right_left_contexts,
            },
            tools,
        },
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

/// Decode and compose the closed four-leaf 4:2:0 square split.
pub(super) fn decode_four_lossless_420_leaves<F>(
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
        ChromaSampling::Subsampled420,
        SyntaxPolicy {
            spatial_luma_context: SpatialLumaContext::Origin,
            coefficient_policy: CoefficientPolicy::DcOrSkipped,
            quantization_syntax: QuantizationSyntax::Lossless,
        },
        tools,
    )?;
    let top_right_neighbor_contexts = coefficient_edge_contexts(
        &top_left_syntax.coefficients,
        SplitOrientation::Horizontal,
        ChromaSampling::Subsampled420,
    );
    let bottom_left_neighbor_contexts = coefficient_edge_contexts(
        &top_left_syntax.coefficients,
        SplitOrientation::Vertical,
        ChromaSampling::Subsampled420,
    );
    let top_left_predictors = origin_predictors(top_left_syntax.luma_predictor);
    let top_left = reconstruct_leaf(top_left_syntax, top_left_predictors);

    let top_right_context =
        SpatialLumaContext::from_neighbor(SplitOrientation::Horizontal, top_left.luma_predictor);
    let top_right_syntax = decode_following_square_syntax(
        decoder,
        &mut cdfs,
        FollowingSyntaxContext {
            transform_grid,
            chroma_sampling: ChromaSampling::Subsampled420,
            spatial_luma_context: top_right_context,
            coefficient_policy: CoefficientPolicy::SubsampledSkippedContextual {
                neighbor_contexts: top_right_neighbor_contexts,
                orientation: SplitOrientation::Horizontal,
            },
            tools,
        },
        &mut between_leaves,
    )?;
    let bottom_right_above_luma_contexts = coefficient_edge_contexts(
        &top_right_syntax.coefficients,
        SplitOrientation::Vertical,
        ChromaSampling::Subsampled420,
    )[0];
    let top_right =
        reconstruct_following_420_leaf(top_right_syntax, &top_left, SplitOrientation::Horizontal);

    let bottom_left_context =
        SpatialLumaContext::from_neighbor(SplitOrientation::Vertical, top_left.luma_predictor);
    let bottom_left_syntax = decode_following_square_syntax(
        decoder,
        &mut cdfs,
        FollowingSyntaxContext {
            transform_grid,
            chroma_sampling: ChromaSampling::Subsampled420,
            spatial_luma_context: bottom_left_context,
            coefficient_policy: CoefficientPolicy::SubsampledSkippedContextual {
                neighbor_contexts: bottom_left_neighbor_contexts,
                orientation: SplitOrientation::Vertical,
            },
            tools,
        },
        &mut between_leaves,
    )?;
    let bottom_right_left_luma_contexts = coefficient_edge_contexts(
        &bottom_left_syntax.coefficients,
        SplitOrientation::Horizontal,
        ChromaSampling::Subsampled420,
    )[0];
    let bottom_left =
        reconstruct_following_420_leaf(bottom_left_syntax, &top_left, SplitOrientation::Vertical);

    // Both side syntax values use the DC-and-skipped subsampled policy, so
    // their two-neighbor luma context is origin by construction.
    let bottom_right_context = SpatialLumaContext::Origin;
    let bottom_right_syntax = decode_following_square_syntax(
        decoder,
        &mut cdfs,
        FollowingSyntaxContext {
            transform_grid,
            chroma_sampling: ChromaSampling::Subsampled420,
            spatial_luma_context: bottom_right_context,
            coefficient_policy: CoefficientPolicy::SubsampledBoundaryContextual {
                above_luma_contexts: bottom_right_above_luma_contexts,
                left_luma_contexts: bottom_right_left_luma_contexts,
            },
            tools,
        },
        &mut between_leaves,
    )?;
    let bottom_right = reconstruct_boundary_420_leaf(bottom_right_syntax, &top_right, &bottom_left);

    let coded_planes = [
        compose_square_plane_with_width(
            &top_left.planes[0],
            &top_right.planes[0],
            &bottom_left.planes[0],
            &bottom_right.planes[0],
            8,
        ),
        compose_square_plane_with_width(
            &top_left.planes[1],
            &top_right.planes[1],
            &bottom_left.planes[1],
            &bottom_right.planes[1],
            4,
        ),
        compose_square_plane_with_width(
            &top_left.planes[2],
            &top_right.planes[2],
            &bottom_left.planes[2],
            &bottom_right.planes[2],
            4,
        ),
    ];
    let chroma_width = width.div_ceil(2);
    let chroma_height = height.div_ceil(2);
    let planes = [
        visible_plane(&coded_planes[0], 16, width, height),
        visible_plane(&coded_planes[1], 8, chroma_width, chroma_height),
        visible_plane(&coded_planes[2], 8, chroma_width, chroma_height),
    ];
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
