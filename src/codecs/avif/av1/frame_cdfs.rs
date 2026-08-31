//! Canonical AV1 common/inter/motion CDF families.
//!
//! The compact representation stores inverse thresholds followed immediately
//! by the mutable adaptation count. It deliberately omits dav1d's invariant
//! top entry and SIMD-only tail padding. Default probabilities and logical
//! shapes are pinned to scalar dav1d 1.5.3 `src/cdf.c`/`src/cdf.h`.

use super::geometry::BlockSize;

/// One compact inverse CDF with its adaptation count in the final slot.
#[derive(Clone, Copy)]
pub(super) struct Cdf<const N: usize>(pub(super) [u16; N]);

impl<const N: usize> Cdf<N> {
    const fn from_probabilities<const P: usize>(probabilities: [u16; P]) -> Self {
        assert!(P < N);
        let mut values = [0_u16; N];
        let mut index = 0;
        while index < P {
            values[index] = 32_768_u16.wrapping_sub(probabilities[index]) & !32_768;
            index += 1;
        }
        Self(values)
    }

    const fn zeroed() -> Self {
        Self([0; N])
    }

    fn reset_count(&mut self) {
        self.0[N.saturating_sub(1)] = 0;
    }
}

/// Common transform syntax published by both intra and inter frames.
#[derive(Clone)]
pub(super) struct CommonCdfs {
    pub(super) inter_transform_1: [Cdf<16>; 2],
    pub(super) inter_transform_2: Cdf<12>,
    pub(super) inter_transform_3: [Cdf<2>; 4],
    pub(super) transform_partition: [[Cdf<2>; 3]; 7],
}

impl CommonCdfs {
    pub(super) const fn defaults() -> Self {
        Self {
            inter_transform_1: [
                Cdf::from_probabilities([
                    4_458, 5_560, 7_695, 9_709, 13_330, 14_789, 17_537, 20_266, 21_504, 22_848,
                    23_934, 25_474, 27_727, 28_915, 30_631,
                ]),
                Cdf::from_probabilities([
                    1_645, 2_573, 4_778, 5_711, 7_807, 8_622, 10_522, 15_357, 17_674, 20_408,
                    22_517, 25_010, 27_116, 28_856, 30_749,
                ]),
            ],
            inter_transform_2: Cdf::from_probabilities([
                770, 2_421, 5_225, 12_907, 15_819, 18_927, 21_561, 24_089, 26_595, 28_526, 30_529,
            ]),
            inter_transform_3: [
                Cdf::from_probabilities([16_384]),
                Cdf::from_probabilities([4_167]),
                Cdf::from_probabilities([1_998]),
                Cdf::from_probabilities([748]),
            ],
            transform_partition: [
                [
                    Cdf::from_probabilities([28_581]),
                    Cdf::from_probabilities([23_846]),
                    Cdf::from_probabilities([20_847]),
                ],
                [
                    Cdf::from_probabilities([24_315]),
                    Cdf::from_probabilities([18_196]),
                    Cdf::from_probabilities([12_133]),
                ],
                [
                    Cdf::from_probabilities([18_791]),
                    Cdf::from_probabilities([10_887]),
                    Cdf::from_probabilities([11_005]),
                ],
                [
                    Cdf::from_probabilities([27_179]),
                    Cdf::from_probabilities([20_004]),
                    Cdf::from_probabilities([11_281]),
                ],
                [
                    Cdf::from_probabilities([26_549]),
                    Cdf::from_probabilities([19_308]),
                    Cdf::from_probabilities([14_224]),
                ],
                [
                    Cdf::from_probabilities([28_015]),
                    Cdf::from_probabilities([21_546]),
                    Cdf::from_probabilities([14_400]),
                ],
                [
                    Cdf::from_probabilities([28_165]),
                    Cdf::from_probabilities([22_401]),
                    Cdf::from_probabilities([16_088]),
                ],
            ],
        }
    }

    pub(super) fn publish_from(&mut self, adapted: &Self) {
        *self = adapted.clone();
        for cdf in &mut self.inter_transform_1 {
            cdf.reset_count();
        }
        self.inter_transform_2.reset_count();
        for cdf in &mut self.inter_transform_3 {
            cdf.reset_count();
        }
        for row in &mut self.transform_partition {
            for cdf in row {
                cdf.reset_count();
            }
        }
    }
}

/// Every logical inter/switch-frame adaptive family.
#[allow(
    dead_code,
    reason = "the complete bundle lands before the inter block engine consumes each family"
)]
#[derive(Clone)]
pub(super) struct InterCdfs {
    pub(super) y_mode: [Cdf<13>; 4],
    pub(super) wedge_index: [Cdf<16>; 9],
    pub(super) compound_mode: [Cdf<8>; 8],
    pub(super) interpolation_filter: [[Cdf<3>; 8]; 2],
    pub(super) interintra_mode: [Cdf<4>; 4],
    motion_mode: [Cdf<3>; 22],
    pub(super) skip_mode: [Cdf<2>; 3],
    pub(super) new_mv: [Cdf<2>; 6],
    pub(super) global_mv: [Cdf<2>; 2],
    pub(super) reference_mv: [Cdf<2>; 6],
    pub(super) drl_bit: [Cdf<2>; 3],
    pub(super) intra: [Cdf<2>; 4],
    pub(super) compound: [Cdf<2>; 5],
    pub(super) compound_direction: [Cdf<2>; 5],
    pub(super) joint_compound: [Cdf<2>; 6],
    pub(super) masked_compound: [Cdf<2>; 6],
    pub(super) wedge_compound: [Cdf<2>; 9],
    pub(super) reference: [[Cdf<2>; 3]; 6],
    pub(super) compound_forward_reference: [[Cdf<2>; 3]; 3],
    pub(super) compound_backward_reference: [[Cdf<2>; 3]; 2],
    pub(super) compound_unidirectional_reference: [[Cdf<2>; 3]; 3],
    pub(super) segment_prediction: [Cdf<2>; 3],
    pub(super) interintra: [Cdf<2>; 4],
    pub(super) interintra_wedge: [Cdf<2>; 7],
    obmc: [Cdf<2>; 22],
}

const EMPTY_MOTION_MODE: Cdf<3> = Cdf::zeroed();
const EMPTY_BINARY: Cdf<2> = Cdf::zeroed();

impl InterCdfs {
    pub(super) const fn defaults() -> Self {
        Self {
            y_mode: [
                Cdf::from_probabilities([
                    22_801, 23_489, 24_293, 24_756, 25_601, 26_123, 26_606, 27_418, 27_945, 29_228,
                    29_685, 30_349,
                ]),
                Cdf::from_probabilities([
                    18_673, 19_845, 22_631, 23_318, 23_950, 24_649, 25_527, 27_364, 28_152, 29_701,
                    29_984, 30_852,
                ]),
                Cdf::from_probabilities([
                    19_770, 20_979, 23_396, 23_939, 24_241, 24_654, 25_136, 27_073, 27_830, 29_360,
                    29_730, 30_659,
                ]),
                Cdf::from_probabilities([
                    20_155, 21_301, 22_838, 23_178, 23_261, 23_533, 23_703, 24_804, 25_352, 26_575,
                    27_016, 28_049,
                ]),
            ],
            wedge_index: [
                Cdf::from_probabilities([
                    2_438, 4_440, 6_599, 8_663, 11_005, 12_874, 15_751, 18_094, 20_359, 22_362,
                    24_127, 25_702, 27_752, 29_450, 31_171,
                ]),
                Cdf::from_probabilities([
                    806, 3_266, 6_005, 6_738, 7_218, 7_367, 7_771, 14_588, 16_323, 17_367, 18_452,
                    19_422, 22_839, 26_127, 29_629,
                ]),
                Cdf::from_probabilities([
                    2_779, 3_738, 4_683, 7_213, 7_775, 8_017, 8_655, 14_357, 17_939, 21_332,
                    24_520, 27_470, 29_456, 30_529, 31_656,
                ]),
                Cdf::from_probabilities([
                    1_684, 3_625, 5_675, 7_108, 9_302, 11_274, 14_429, 17_144, 19_163, 20_961,
                    22_884, 24_471, 26_719, 28_714, 30_877,
                ]),
                Cdf::from_probabilities([
                    1_142, 3_491, 6_277, 7_314, 8_089, 8_355, 9_023, 13_624, 15_369, 16_730,
                    18_114, 19_313, 22_521, 26_012, 29_550,
                ]),
                Cdf::from_probabilities([
                    2_742, 4_195, 5_727, 8_035, 8_980, 9_336, 10_146, 14_124, 17_270, 20_533,
                    23_434, 25_972, 27_944, 29_570, 31_416,
                ]),
                Cdf::from_probabilities([
                    1_727, 3_948, 6_101, 7_796, 9_841, 12_344, 15_766, 18_944, 20_638, 22_038,
                    23_963, 25_311, 26_988, 28_766, 31_012,
                ]),
                Cdf::from_probabilities([
                    154, 987, 1_925, 2_051, 2_088, 2_111, 2_151, 23_033, 23_703, 24_284, 24_985,
                    25_684, 27_259, 28_883, 30_911,
                ]),
                Cdf::from_probabilities([
                    1_135, 1_322, 1_493, 2_635, 2_696, 2_737, 2_770, 21_016, 22_935, 25_057,
                    27_251, 29_173, 30_089, 30_960, 31_933,
                ]),
            ],
            compound_mode: [
                Cdf::from_probabilities([7_760, 13_823, 15_808, 17_641, 19_156, 20_666, 26_891]),
                Cdf::from_probabilities([10_730, 19_452, 21_145, 22_749, 24_039, 25_131, 28_724]),
                Cdf::from_probabilities([10_664, 20_221, 21_588, 22_906, 24_295, 25_387, 28_436]),
                Cdf::from_probabilities([13_298, 16_984, 20_471, 24_182, 25_067, 25_736, 26_422]),
                Cdf::from_probabilities([18_904, 23_325, 25_242, 27_432, 27_898, 28_258, 30_758]),
                Cdf::from_probabilities([10_725, 17_454, 20_124, 22_820, 24_195, 25_168, 26_046]),
                Cdf::from_probabilities([17_125, 24_273, 25_814, 27_492, 28_214, 28_704, 30_592]),
                Cdf::from_probabilities([13_046, 23_214, 24_505, 25_942, 27_435, 28_442, 29_330]),
            ],
            interpolation_filter: [
                [
                    Cdf::from_probabilities([31_935, 32_720]),
                    Cdf::from_probabilities([5_568, 32_719]),
                    Cdf::from_probabilities([422, 2_938]),
                    Cdf::from_probabilities([28_244, 32_608]),
                    Cdf::from_probabilities([31_206, 31_953]),
                    Cdf::from_probabilities([4_862, 32_121]),
                    Cdf::from_probabilities([770, 1_152]),
                    Cdf::from_probabilities([20_889, 25_637]),
                ],
                [
                    Cdf::from_probabilities([31_910, 32_724]),
                    Cdf::from_probabilities([4_120, 32_712]),
                    Cdf::from_probabilities([305, 2_247]),
                    Cdf::from_probabilities([27_403, 32_636]),
                    Cdf::from_probabilities([31_022, 32_009]),
                    Cdf::from_probabilities([2_963, 32_093]),
                    Cdf::from_probabilities([601, 943]),
                    Cdf::from_probabilities([14_969, 21_398]),
                ],
            ],
            interintra_mode: [
                Cdf::from_probabilities([8_192, 16_384, 24_576]),
                Cdf::from_probabilities([1_875, 11_082, 27_332]),
                Cdf::from_probabilities([2_473, 9_996, 26_388]),
                Cdf::from_probabilities([4_238, 11_537, 25_926]),
            ],
            motion_mode: [
                EMPTY_MOTION_MODE,
                EMPTY_MOTION_MODE,
                EMPTY_MOTION_MODE,
                Cdf::from_probabilities([7_651, 24_760]),
                Cdf::from_probabilities([4_738, 24_765]),
                Cdf::from_probabilities([5_391, 25_528]),
                Cdf::from_probabilities([19_419, 26_810]),
                Cdf::from_probabilities([5_123, 23_606]),
                Cdf::from_probabilities([11_606, 24_308]),
                Cdf::from_probabilities([26_260, 29_116]),
                Cdf::from_probabilities([20_360, 28_062]),
                Cdf::from_probabilities([21_679, 26_830]),
                Cdf::from_probabilities([29_516, 30_701]),
                Cdf::from_probabilities([28_898, 30_397]),
                Cdf::from_probabilities([30_878, 31_335]),
                Cdf::from_probabilities([32_507, 32_558]),
                EMPTY_MOTION_MODE,
                EMPTY_MOTION_MODE,
                Cdf::from_probabilities([28_799, 31_390]),
                Cdf::from_probabilities([26_431, 30_774]),
                Cdf::from_probabilities([28_973, 31_594]),
                Cdf::from_probabilities([29_742, 31_203]),
            ],
            skip_mode: binary_rows([32_621, 20_708, 8_127]),
            new_mv: binary_rows([24_035, 16_630, 15_339, 8_386, 12_222, 4_676]),
            global_mv: binary_rows([2_175, 1_054]),
            reference_mv: binary_rows([23_974, 24_188, 17_848, 28_622, 24_312, 19_923]),
            drl_bit: binary_rows([13_104, 24_560, 18_945]),
            intra: binary_rows([806, 16_662, 20_186, 26_538]),
            compound: binary_rows([26_828, 24_035, 12_031, 10_640, 2_901]),
            compound_direction: binary_rows([1_198, 2_070, 9_166, 7_499, 22_475]),
            joint_compound: binary_rows([18_244, 12_865, 7_053, 13_259, 9_334, 4_644]),
            masked_compound: binary_rows([26_607, 22_891, 18_840, 24_594, 19_934, 22_674]),
            wedge_compound: binary_rows([
                23_431, 13_171, 11_470, 9_770, 9_100, 8_233, 6_172, 11_820, 7_701,
            ]),
            reference: binary_matrix([
                [4_897, 16_973, 29_744],
                [1_555, 16_751, 30_279],
                [4_236, 19_647, 31_194],
                [8_650, 24_773, 31_895],
                [904, 11_014, 26_875],
                [1_444, 15_087, 30_304],
            ]),
            compound_forward_reference: binary_matrix([
                [4_946, 19_891, 30_731],
                [9_468, 22_441, 31_059],
                [1_503, 15_160, 27_544],
            ]),
            compound_backward_reference: binary_matrix([
                [2_235, 17_182, 30_606],
                [1_423, 15_175, 30_489],
            ]),
            compound_unidirectional_reference: binary_matrix([
                [5_284, 23_152, 31_774],
                [3_865, 14_173, 25_120],
                [3_128, 15_270, 26_710],
            ]),
            segment_prediction: binary_rows([16_384; 3]),
            interintra: binary_rows([16_384, 26_887, 27_597, 30_237]),
            interintra_wedge: binary_rows([20_036, 24_957, 26_704, 27_530, 29_564, 29_444, 26_872]),
            obmc: [
                EMPTY_BINARY,
                EMPTY_BINARY,
                EMPTY_BINARY,
                Cdf::from_probabilities([10_437]),
                Cdf::from_probabilities([9_371]),
                Cdf::from_probabilities([9_301]),
                Cdf::from_probabilities([17_432]),
                Cdf::from_probabilities([14_423]),
                Cdf::from_probabilities([15_142]),
                Cdf::from_probabilities([25_817]),
                Cdf::from_probabilities([22_823]),
                Cdf::from_probabilities([22_083]),
                Cdf::from_probabilities([30_128]),
                Cdf::from_probabilities([31_014]),
                Cdf::from_probabilities([31_560]),
                Cdf::from_probabilities([32_638]),
                EMPTY_BINARY,
                EMPTY_BINARY,
                Cdf::from_probabilities([23_664]),
                Cdf::from_probabilities([20_901]),
                Cdf::from_probabilities([24_008]),
                Cdf::from_probabilities([26_879]),
            ],
        }
    }

    pub(super) fn publish_from(&mut self, adapted: &Self) {
        *self = adapted.clone();
        for cdf in &mut self.y_mode {
            cdf.reset_count();
        }
        for cdf in &mut self.wedge_index {
            cdf.reset_count();
        }
        for cdf in &mut self.compound_mode {
            cdf.reset_count();
        }
        for axis in &mut self.interpolation_filter {
            for cdf in axis {
                cdf.reset_count();
            }
        }
        for cdf in &mut self.interintra_mode {
            cdf.reset_count();
        }
        for cdf in &mut self.motion_mode {
            cdf.reset_count();
        }
        reset_binary_rows(&mut self.skip_mode);
        reset_binary_rows(&mut self.new_mv);
        reset_binary_rows(&mut self.global_mv);
        reset_binary_rows(&mut self.reference_mv);
        reset_binary_rows(&mut self.drl_bit);
        reset_binary_rows(&mut self.intra);
        reset_binary_rows(&mut self.compound);
        reset_binary_rows(&mut self.compound_direction);
        reset_binary_rows(&mut self.joint_compound);
        reset_binary_rows(&mut self.masked_compound);
        reset_binary_rows(&mut self.wedge_compound);
        reset_binary_matrix(&mut self.reference);
        reset_binary_matrix(&mut self.compound_forward_reference);
        reset_binary_matrix(&mut self.compound_backward_reference);
        reset_binary_matrix(&mut self.compound_unidirectional_reference);
        reset_binary_rows(&mut self.segment_prediction);
        reset_binary_rows(&mut self.interintra);
        reset_binary_rows(&mut self.interintra_wedge);
        reset_binary_rows(&mut self.obmc);
    }

    /// Block-size CDFs use this crate's specification-order discriminants,
    /// matching the reverse lookup tables used by the inter syntax state.
    #[allow(
        dead_code,
        reason = "consumed by the inter block engine in the following implementation slice"
    )]
    pub(super) fn motion_mode_for(&mut self, block_size: BlockSize) -> &mut Cdf<3> {
        &mut self.motion_mode[block_size.cdf_index()]
    }

    /// Typed OBMC lookup paired with [`Self::motion_mode_for`].
    #[allow(
        dead_code,
        reason = "consumed by the inter block engine in the following implementation slice"
    )]
    pub(super) fn obmc_for(&mut self, block_size: BlockSize) -> &mut Cdf<2> {
        &mut self.obmc[block_size.cdf_index()]
    }

    pub(super) fn interintra_for(&mut self, size_group: usize) -> Option<&mut Cdf<2>> {
        self.interintra.get_mut(size_group)
    }

    pub(super) fn interintra_mode_for(&mut self, size_group: usize) -> Option<&mut Cdf<4>> {
        self.interintra_mode.get_mut(size_group)
    }

    pub(super) fn interintra_wedge_for(&mut self, context: usize) -> Option<&mut Cdf<2>> {
        self.interintra_wedge.get_mut(context)
    }

    pub(super) fn wedge_index_for(&mut self, context: usize) -> Option<&mut Cdf<16>> {
        self.wedge_index.get_mut(context)
    }
}

const fn binary_rows<const N: usize>(probabilities: [u16; N]) -> [Cdf<2>; N] {
    let mut rows = [EMPTY_BINARY; N];
    let mut index = 0;
    while index < N {
        rows[index] = Cdf::from_probabilities([probabilities[index]]);
        index += 1;
    }
    rows
}

const fn binary_matrix<const R: usize, const C: usize>(
    probabilities: [[u16; C]; R],
) -> [[Cdf<2>; C]; R] {
    let mut rows = [[EMPTY_BINARY; C]; R];
    let mut row = 0;
    while row < R {
        rows[row] = binary_rows(probabilities[row]);
        row += 1;
    }
    rows
}

fn reset_binary_rows<const N: usize>(rows: &mut [Cdf<2>; N]) {
    for cdf in rows {
        cdf.reset_count();
    }
}

fn reset_binary_matrix<const R: usize, const C: usize>(rows: &mut [[Cdf<2>; C]; R]) {
    for row in rows {
        reset_binary_rows(row);
    }
}

/// One signed motion-vector component's adaptive syntax.
#[allow(
    dead_code,
    reason = "the complete bundle lands before the inter block engine consumes each family"
)]
#[derive(Clone)]
pub(super) struct MvComponentCdfs {
    pub(super) classes: Cdf<11>,
    pub(super) sign: Cdf<2>,
    pub(super) class_zero: Cdf<2>,
    pub(super) class_zero_fractional: [Cdf<4>; 2],
    pub(super) class_zero_high_precision: Cdf<2>,
    pub(super) class_n_bits: [Cdf<2>; 10],
    pub(super) class_n_fractional: Cdf<4>,
    pub(super) class_n_high_precision: Cdf<2>,
}

impl MvComponentCdfs {
    const fn defaults() -> Self {
        Self {
            classes: Cdf::from_probabilities([
                28_672, 30_976, 31_858, 32_320, 32_551, 32_656, 32_740, 32_757, 32_762, 32_767,
            ]),
            sign: Cdf::from_probabilities([16_384]),
            class_zero: Cdf::from_probabilities([27_648]),
            class_zero_fractional: [
                Cdf::from_probabilities([16_384, 24_576, 26_624]),
                Cdf::from_probabilities([12_288, 21_248, 24_128]),
            ],
            class_zero_high_precision: Cdf::from_probabilities([20_480]),
            class_n_bits: binary_rows([
                17_408, 17_920, 18_944, 20_480, 22_528, 24_576, 28_672, 29_952, 29_952, 30_720,
            ]),
            class_n_fractional: Cdf::from_probabilities([8_192, 17_408, 21_248]),
            class_n_high_precision: Cdf::from_probabilities([16_384]),
        }
    }

    fn reset_counts(&mut self) {
        self.classes.reset_count();
        self.sign.reset_count();
        self.class_zero.reset_count();
        for cdf in &mut self.class_zero_fractional {
            cdf.reset_count();
        }
        self.class_zero_high_precision.reset_count();
        reset_binary_rows(&mut self.class_n_bits);
        self.class_n_fractional.reset_count();
        self.class_n_high_precision.reset_count();
    }
}

/// Full vertical/horizontal motion-vector CDF state.
#[allow(
    dead_code,
    reason = "the complete bundle lands before the inter block engine consumes each family"
)]
#[derive(Clone)]
pub(super) struct MvCdfs {
    pub(super) component: [MvComponentCdfs; 2],
    pub(super) joint: Cdf<4>,
}

impl MvCdfs {
    pub(super) const fn defaults() -> Self {
        Self {
            component: [MvComponentCdfs::defaults(), MvComponentCdfs::defaults()],
            joint: Cdf::from_probabilities([4_096, 11_264, 19_328]),
        }
    }

    pub(super) fn publish_from(&mut self, adapted: &Self) {
        *self = adapted.clone();
        for component in &mut self.component {
            component.reset_counts();
        }
        self.joint.reset_count();
    }
}

pub(super) const DEFAULT_INTRABC: Cdf<2> = Cdf::from_probabilities([30_531]);
