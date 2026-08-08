//! Exact libwebp-compatible VP8 4×4 intra predictions.

use super::{
    cost::{rd_score, residual_cost, spectral_distortion_4x4, squared_error_4x4},
    dct::{
        vp8_fdct_4x4, vp8_fdct_4x4_with_control, vp8_idct_add_4x4, vp8_idct_add_4x4_with_control,
    },
    quant::{
        QuantMatrix, SegmentMatrices, quantize_block, quantize_block_with_control,
        trellis_quantize_block, trellis_quantize_block_with_control,
    },
};
use crate::codecs::CodecResult;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(super) enum Intra4Mode {
    Dc = 0,
    TrueMotion = 1,
    Vertical = 2,
    Horizontal = 3,
    DownRight = 4,
    VerticalRight = 5,
    DownLeft = 6,
    VerticalLeft = 7,
    HorizontalDown = 8,
    HorizontalUp = 9,
}

impl Intra4Mode {
    pub(super) const ALL: [Self; 10] = [
        Self::Dc,
        Self::TrueMotion,
        Self::Vertical,
        Self::Horizontal,
        Self::DownRight,
        Self::VerticalRight,
        Self::DownLeft,
        Self::VerticalLeft,
        Self::HorizontalDown,
        Self::HorizontalUp,
    ];
}

// Fixed keyframe intra-4 mode costs from libwebp 1.6.0 `cost_enc.c`.
const FIXED_MODE_COSTS: [[[u16; 10]; 10]; 10] = [
    [
        [40, 1151, 1723, 1874, 2103, 2019, 1628, 1777, 2226, 2137],
        [192, 469, 1296, 1308, 1849, 1794, 1781, 1703, 1713, 1522],
        [142, 910, 762, 1684, 1849, 1576, 1460, 1305, 1801, 1657],
        [559, 641, 1370, 421, 1182, 1569, 1612, 1725, 863, 1007],
        [299, 1059, 1256, 1108, 636, 1068, 1581, 1883, 869, 1142],
        [277, 1111, 707, 1362, 1089, 672, 1603, 1541, 1545, 1291],
        [214, 781, 1609, 1303, 1632, 2229, 726, 1560, 1713, 918],
        [152, 1037, 1046, 1759, 1983, 2174, 1358, 742, 1740, 1390],
        [512, 1046, 1420, 753, 752, 1297, 1486, 1613, 460, 1207],
        [424, 827, 1362, 719, 1462, 1202, 1199, 1476, 1199, 538],
    ],
    [
        [240, 402, 1134, 1491, 1659, 1505, 1517, 1555, 1979, 2099],
        [467, 242, 960, 1232, 1714, 1620, 1834, 1570, 1676, 1391],
        [500, 455, 463, 1507, 1699, 1282, 1564, 982, 2114, 2114],
        [672, 643, 1372, 331, 1589, 1667, 1453, 1938, 996, 876],
        [458, 783, 1037, 911, 738, 968, 1165, 1518, 859, 1033],
        [504, 815, 504, 1139, 1219, 719, 1506, 1085, 1268, 1268],
        [333, 630, 1445, 1239, 1883, 3672, 799, 1548, 1865, 598],
        [399, 644, 746, 1342, 1856, 1350, 1493, 613, 1855, 1015],
        [622, 749, 1205, 608, 1066, 1408, 1290, 1406, 546, 971],
        [500, 753, 1041, 668, 1230, 1617, 1297, 1425, 1383, 523],
    ],
    [
        [394, 553, 523, 1502, 1536, 981, 1608, 1142, 1666, 2181],
        [655, 430, 375, 1411, 1861, 1220, 1677, 1135, 1978, 1553],
        [690, 640, 245, 1954, 2070, 1194, 1528, 982, 1972, 2232],
        [559, 834, 741, 867, 1131, 980, 1225, 852, 1092, 784],
        [690, 875, 516, 959, 673, 894, 1056, 1190, 1528, 1126],
        [740, 951, 384, 1277, 1177, 492, 1579, 1155, 1846, 1513],
        [323, 775, 1062, 1776, 3062, 1274, 813, 1188, 1372, 655],
        [488, 971, 484, 1767, 1515, 1775, 1115, 503, 1539, 1461],
        [740, 1006, 998, 709, 851, 1230, 1337, 788, 741, 721],
        [522, 1073, 573, 1045, 1346, 887, 1046, 1146, 1203, 697],
    ],
    [
        [105, 864, 1442, 1009, 1934, 1840, 1519, 1920, 1673, 1579],
        [534, 305, 1193, 683, 1388, 2164, 1802, 1894, 1264, 1170],
        [305, 518, 877, 1108, 1426, 3215, 1425, 1064, 1320, 1242],
        [683, 732, 1927, 257, 1493, 2048, 1858, 1552, 1055, 947],
        [394, 814, 1024, 660, 959, 1556, 1282, 1289, 893, 1047],
        [528, 615, 996, 940, 1201, 635, 1094, 2515, 803, 1358],
        [347, 614, 1609, 1187, 3133, 1345, 1007, 1339, 1017, 667],
        [218, 740, 878, 1605, 3650, 3650, 1345, 758, 1357, 1617],
        [672, 750, 1541, 558, 1257, 1599, 1870, 2135, 402, 1087],
        [592, 684, 1161, 430, 1092, 1497, 1475, 1489, 1095, 822],
    ],
    [
        [228, 1056, 1059, 1368, 752, 982, 1512, 1518, 987, 1782],
        [494, 514, 818, 942, 965, 892, 1610, 1356, 1048, 1363],
        [512, 648, 591, 1042, 761, 991, 1196, 1454, 1309, 1463],
        [683, 749, 1043, 676, 841, 1396, 1133, 1138, 654, 939],
        [622, 1101, 1126, 994, 361, 1077, 1203, 1318, 877, 1219],
        [631, 1068, 857, 1650, 651, 477, 1650, 1419, 828, 1170],
        [555, 727, 1068, 1335, 3127, 1339, 820, 1331, 1077, 429],
        [504, 879, 624, 1398, 889, 889, 1392, 808, 891, 1406],
        [683, 1602, 1289, 977, 578, 983, 1280, 1708, 406, 1122],
        [399, 865, 1433, 1070, 1072, 764, 968, 1477, 1223, 678],
    ],
    [
        [333, 760, 935, 1638, 1010, 529, 1646, 1410, 1472, 2219],
        [512, 494, 750, 1160, 1215, 610, 1870, 1868, 1628, 1169],
        [572, 646, 492, 1934, 1208, 603, 1580, 1099, 1398, 1995],
        [786, 789, 942, 581, 1018, 951, 1599, 1207, 731, 768],
        [690, 1015, 672, 1078, 582, 504, 1693, 1438, 1108, 2897],
        [768, 1267, 571, 2005, 1243, 244, 2881, 1380, 1786, 1453],
        [452, 899, 1293, 903, 1311, 3100, 465, 1311, 1319, 813],
        [394, 927, 942, 1103, 1358, 1104, 946, 593, 1363, 1109],
        [559, 1005, 1007, 1016, 658, 1173, 1021, 1164, 623, 1028],
        [564, 796, 632, 1005, 1014, 863, 2316, 1268, 938, 764],
    ],
    [
        [266, 606, 1098, 1228, 1497, 1243, 948, 1030, 1734, 1461],
        [366, 585, 901, 1060, 1407, 1247, 876, 1134, 1620, 1054],
        [452, 565, 542, 1729, 1479, 1479, 1016, 886, 2938, 1150],
        [555, 1088, 1533, 950, 1354, 895, 834, 1019, 1021, 496],
        [704, 815, 1193, 971, 973, 640, 1217, 2214, 832, 578],
        [672, 1245, 579, 871, 875, 774, 872, 1273, 1027, 949],
        [296, 1134, 2050, 1784, 1636, 3425, 442, 1550, 2076, 722],
        [342, 982, 1259, 1846, 1848, 1848, 622, 568, 1847, 1052],
        [555, 1064, 1304, 828, 746, 1343, 1075, 1329, 1078, 494],
        [288, 1167, 1285, 1174, 1639, 1639, 833, 2254, 1304, 509],
    ],
    [
        [342, 719, 767, 1866, 1757, 1270, 1246, 550, 1746, 2151],
        [483, 653, 694, 1509, 1459, 1410, 1218, 507, 1914, 1266],
        [488, 757, 447, 2979, 1813, 1268, 1654, 539, 1849, 2109],
        [522, 1097, 1085, 851, 1365, 1111, 851, 901, 961, 605],
        [709, 716, 841, 728, 736, 945, 941, 862, 2845, 1057],
        [512, 1323, 500, 1336, 1083, 681, 1342, 717, 1604, 1350],
        [452, 1155, 1372, 1900, 1501, 3290, 311, 944, 1919, 922],
        [403, 1520, 977, 2132, 1733, 3522, 1076, 276, 3335, 1547],
        [559, 1374, 1101, 615, 673, 2462, 974, 795, 984, 984],
        [547, 1122, 1062, 812, 1410, 951, 1140, 622, 1268, 651],
    ],
    [
        [165, 982, 1235, 938, 1334, 1366, 1659, 1578, 964, 1612],
        [592, 422, 925, 847, 1139, 1112, 1387, 2036, 861, 1041],
        [403, 837, 732, 770, 941, 1658, 1250, 809, 1407, 1407],
        [896, 874, 1071, 381, 1568, 1722, 1437, 2192, 480, 1035],
        [640, 1098, 1012, 1032, 684, 1382, 1581, 2106, 416, 865],
        [559, 1005, 819, 914, 710, 770, 1418, 920, 838, 1435],
        [415, 1258, 1245, 870, 1278, 3067, 770, 1021, 1287, 522],
        [406, 990, 601, 1009, 1265, 1265, 1267, 759, 1017, 1277],
        [968, 1182, 1329, 788, 1032, 1292, 1705, 1714, 203, 1403],
        [732, 877, 1279, 471, 901, 1161, 1545, 1294, 755, 755],
    ],
    [
        [111, 931, 1378, 1185, 1933, 1648, 1148, 1714, 1873, 1307],
        [406, 414, 1030, 1023, 1910, 1404, 1313, 1647, 1509, 793],
        [342, 640, 575, 1088, 1241, 1349, 1161, 1350, 1756, 1502],
        [559, 766, 1185, 357, 1682, 1428, 1329, 1897, 1219, 802],
        [473, 909, 1164, 771, 719, 2508, 1427, 1432, 722, 782],
        [342, 892, 785, 1145, 1150, 794, 1296, 1550, 973, 1057],
        [208, 1036, 1326, 1343, 1606, 3395, 815, 1455, 1618, 712],
        [228, 928, 890, 1046, 3499, 1711, 994, 829, 1720, 1318],
        [768, 724, 1058, 636, 991, 1075, 1319, 1324, 616, 825],
        [305, 1167, 1358, 899, 1587, 1587, 987, 1988, 1332, 501],
    ],
];

pub(super) fn fixed_mode_cost(top: Intra4Mode, left: Intra4Mode, mode: Intra4Mode) -> u16 {
    FIXED_MODE_COSTS[top as usize][left as usize][mode as usize]
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Intra4Result {
    pub(super) modes: [Intra4Mode; 16],
    pub(super) levels: [[i16; 16]; 16],
    pub(super) reconstructed: [u8; 256],
    pub(super) distortion: u32,
    pub(super) spectral_distortion: u32,
    pub(super) header_cost: u32,
    pub(super) rate_cost: u32,
    pub(super) score: u64,
    pub(super) nonzero: u32,
}

trait SelectionCheckpointControl {
    fn after_candidate_stage(&mut self) -> CodecResult<()>;
    fn forward_transform(&mut self, block: &[i16; 16]) -> CodecResult<[i16; 16]>;
    fn inverse_transform(
        &mut self,
        prediction: &[u8; 16],
        coefficients: &[i16; 16],
    ) -> CodecResult<[u8; 16]>;
    fn quantize_block(
        &mut self,
        coefficients: &mut [i16; 16],
        levels: &mut [i16; 16],
        matrix: &QuantMatrix,
    ) -> CodecResult<bool>;
    fn trellis_quantize_block(
        &mut self,
        coefficients: &mut [i16; 16],
        levels: &mut [i16; 16],
        initial_context: usize,
        coefficient_type: usize,
        matrix: &QuantMatrix,
        lambda: i32,
        probabilities: &[[[[u8; 11]; 3]; 8]; 4],
    ) -> CodecResult<bool>;
    fn after_candidate(&mut self) -> CodecResult<()>;
    fn after_block(&mut self) -> CodecResult<()>;
}

struct NoopSelectionCheckpoint;

impl SelectionCheckpointControl for NoopSelectionCheckpoint {
    #[inline(always)]
    fn after_candidate_stage(&mut self) -> CodecResult<()> {
        Ok(())
    }

    #[inline(always)]
    fn forward_transform(&mut self, block: &[i16; 16]) -> CodecResult<[i16; 16]> {
        Ok(vp8_fdct_4x4(block))
    }

    #[inline(always)]
    fn inverse_transform(
        &mut self,
        prediction: &[u8; 16],
        coefficients: &[i16; 16],
    ) -> CodecResult<[u8; 16]> {
        Ok(vp8_idct_add_4x4(prediction, coefficients))
    }

    #[inline(always)]
    fn quantize_block(
        &mut self,
        coefficients: &mut [i16; 16],
        levels: &mut [i16; 16],
        matrix: &QuantMatrix,
    ) -> CodecResult<bool> {
        Ok(quantize_block(coefficients, levels, matrix))
    }

    #[inline(always)]
    fn trellis_quantize_block(
        &mut self,
        coefficients: &mut [i16; 16],
        levels: &mut [i16; 16],
        initial_context: usize,
        coefficient_type: usize,
        matrix: &QuantMatrix,
        lambda: i32,
        probabilities: &[[[[u8; 11]; 3]; 8]; 4],
    ) -> CodecResult<bool> {
        Ok(trellis_quantize_block(
            coefficients,
            levels,
            initial_context,
            coefficient_type,
            matrix,
            lambda,
            probabilities,
        ))
    }

    #[inline(always)]
    fn after_candidate(&mut self) -> CodecResult<()> {
        Ok(())
    }

    #[inline(always)]
    fn after_block(&mut self) -> CodecResult<()> {
        Ok(())
    }
}

struct TokenSelectionCheckpoint<'a> {
    token: &'a crate::CancellationToken,
}

impl SelectionCheckpointControl for TokenSelectionCheckpoint<'_> {
    #[inline]
    fn after_candidate_stage(&mut self) -> CodecResult<()> {
        crate::codecs::error::check_cancelled(Some(self.token))
    }

    #[inline]
    fn forward_transform(&mut self, block: &[i16; 16]) -> CodecResult<[i16; 16]> {
        vp8_fdct_4x4_with_control(block, || self.after_candidate_stage())
    }

    #[inline]
    fn inverse_transform(
        &mut self,
        prediction: &[u8; 16],
        coefficients: &[i16; 16],
    ) -> CodecResult<[u8; 16]> {
        vp8_idct_add_4x4_with_control(prediction, coefficients, || self.after_candidate_stage())
    }

    #[inline]
    fn quantize_block(
        &mut self,
        coefficients: &mut [i16; 16],
        levels: &mut [i16; 16],
        matrix: &QuantMatrix,
    ) -> CodecResult<bool> {
        quantize_block_with_control(coefficients, levels, matrix, || {
            self.after_candidate_stage()
        })
    }

    #[inline]
    fn trellis_quantize_block(
        &mut self,
        coefficients: &mut [i16; 16],
        levels: &mut [i16; 16],
        initial_context: usize,
        coefficient_type: usize,
        matrix: &QuantMatrix,
        lambda: i32,
        probabilities: &[[[[u8; 11]; 3]; 8]; 4],
    ) -> CodecResult<bool> {
        trellis_quantize_block_with_control(
            coefficients,
            levels,
            initial_context,
            coefficient_type,
            matrix,
            lambda,
            probabilities,
            || self.after_candidate_stage(),
        )
    }

    #[inline]
    fn after_candidate(&mut self) -> CodecResult<()> {
        crate::codecs::error::check_cancelled(Some(self.token))
    }

    #[inline]
    fn after_block(&mut self) -> CodecResult<()> {
        crate::codecs::error::check_cancelled(Some(self.token))
    }
}

#[derive(Clone, Copy)]
struct Intra4SelectionInput<'a> {
    source: &'a [u8; 256],
    top_boundary: &'a [u8; 20],
    left_boundary: &'a [u8; 16],
    top_left: u8,
    top_modes: &'a [Intra4Mode; 4],
    left_modes: &'a [Intra4Mode; 4],
    top_nonzero: [u8; 4],
    left_nonzero: [u8; 4],
    matrices: &'a SegmentMatrices,
    lambda_i4: u32,
    lambda_mode: u32,
    texture_lambda: u32,
    distortion_only: bool,
    coefficient_probabilities: &'a [[[[u8; 11]; 3]; 8]; 4],
    trellis: bool,
}

#[allow(clippy::expect_used, clippy::too_many_arguments)]
pub(super) fn select_macroblock(
    source: &[u8; 256],
    top_boundary: &[u8; 20],
    left_boundary: &[u8; 16],
    top_left: u8,
    top_modes: &[Intra4Mode; 4],
    left_modes: &[Intra4Mode; 4],
    top_nonzero: [u8; 4],
    left_nonzero: [u8; 4],
    matrices: &SegmentMatrices,
    lambda_i4: u32,
    lambda_mode: u32,
    texture_lambda: u32,
    distortion_only: bool,
    coefficient_probabilities: &[[[[u8; 11]; 3]; 8]; 4],
    trellis: bool,
) -> Intra4Result {
    select_macroblock_with_control(
        Intra4SelectionInput {
            source,
            top_boundary,
            left_boundary,
            top_left,
            top_modes,
            left_modes,
            top_nonzero,
            left_nonzero,
            matrices,
            lambda_i4,
            lambda_mode,
            texture_lambda,
            distortion_only,
            coefficient_probabilities,
            trellis,
        },
        &mut NoopSelectionCheckpoint,
    )
    .expect("the no-token selection controller cannot fail")
}

#[allow(clippy::too_many_arguments)]
pub(super) fn select_macroblock_with_token(
    source: &[u8; 256],
    top_boundary: &[u8; 20],
    left_boundary: &[u8; 16],
    top_left: u8,
    top_modes: &[Intra4Mode; 4],
    left_modes: &[Intra4Mode; 4],
    top_nonzero: [u8; 4],
    left_nonzero: [u8; 4],
    matrices: &SegmentMatrices,
    lambda_i4: u32,
    lambda_mode: u32,
    texture_lambda: u32,
    distortion_only: bool,
    coefficient_probabilities: &[[[[u8; 11]; 3]; 8]; 4],
    trellis: bool,
    token: &crate::CancellationToken,
) -> CodecResult<Intra4Result> {
    select_macroblock_with_control(
        Intra4SelectionInput {
            source,
            top_boundary,
            left_boundary,
            top_left,
            top_modes,
            left_modes,
            top_nonzero,
            left_nonzero,
            matrices,
            lambda_i4,
            lambda_mode,
            texture_lambda,
            distortion_only,
            coefficient_probabilities,
            trellis,
        },
        &mut TokenSelectionCheckpoint { token },
    )
}

fn select_macroblock_with_control<C: SelectionCheckpointControl>(
    input: Intra4SelectionInput<'_>,
    checkpoint: &mut C,
) -> CodecResult<Intra4Result> {
    let Intra4SelectionInput {
        source,
        top_boundary,
        left_boundary,
        top_left,
        top_modes,
        left_modes,
        mut top_nonzero,
        mut left_nonzero,
        matrices,
        lambda_i4,
        lambda_mode,
        texture_lambda,
        distortion_only,
        coefficient_probabilities,
        trellis,
    } = input;
    let mut result = Intra4Result {
        modes: [Intra4Mode::Dc; 16],
        levels: [[0; 16]; 16],
        reconstructed: [0; 256],
        distortion: 0,
        spectral_distortion: 0,
        header_cost: 211,
        rate_cost: 0,
        score: u64::from(211_u32.wrapping_mul(lambda_mode)),
        nonzero: 0,
    };

    for block_y in 0_usize..4 {
        for block_x in 0_usize..4 {
            let block_index = block_y.wrapping_mul(4).wrapping_add(block_x);
            let mut block_source = [0; 16];
            for row in 0_usize..4 {
                let source_offset = block_y
                    .wrapping_mul(4)
                    .wrapping_add(row)
                    .wrapping_mul(16)
                    .wrapping_add(block_x.wrapping_mul(4));
                let block_offset = row.wrapping_mul(4);
                block_source[block_offset..block_offset.wrapping_add(4)]
                    .copy_from_slice(&source[source_offset..source_offset.wrapping_add(4)]);
            }

            let mut top = [0; 8];
            for (offset, sample) in top.iter_mut().enumerate() {
                let column = block_x.wrapping_mul(4).wrapping_add(offset);
                *sample = if block_y == 0 {
                    top_boundary[column]
                } else if column < 16 {
                    result.reconstructed[block_y
                        .wrapping_mul(4)
                        .wrapping_sub(1)
                        .wrapping_mul(16)
                        .wrapping_add(column)]
                } else {
                    top_boundary[column]
                };
            }
            let left = std::array::from_fn(|offset| {
                let row = block_y.wrapping_mul(4).wrapping_add(offset);
                if block_x == 0 {
                    left_boundary[row]
                } else {
                    result.reconstructed[row
                        .wrapping_mul(16)
                        .wrapping_add(block_x.wrapping_mul(4))
                        .wrapping_sub(1)]
                }
            });
            let block_top_left = match (block_x, block_y) {
                (0, 0) => top_left,
                (_, 0) => top_boundary[block_x.wrapping_mul(4).wrapping_sub(1)],
                (0, _) => left_boundary[block_y.wrapping_mul(4).wrapping_sub(1)],
                _ => {
                    result.reconstructed[block_y
                        .wrapping_mul(4)
                        .wrapping_sub(1)
                        .wrapping_mul(16)
                        .wrapping_add(block_x.wrapping_mul(4))
                        .wrapping_sub(1)]
                }
            };
            let top_mode = if block_y == 0 {
                top_modes[block_x]
            } else {
                result.modes[block_index.wrapping_sub(4)]
            };
            let left_mode = if block_x == 0 {
                left_modes[block_y]
            } else {
                result.modes[block_index.wrapping_sub(1)]
            };
            let context = usize::from(top_nonzero[block_x].wrapping_add(left_nonzero[block_y]));
            let selected_mode = if distortion_only {
                let mut selected: Option<(u32, Intra4Mode)> = None;
                for mode in Intra4Mode::ALL {
                    let prediction = predict(mode, &top, &left, block_top_left);
                    checkpoint.after_candidate_stage()?;
                    let distortion = squared_error_4x4(&block_source, &prediction);
                    checkpoint.after_candidate_stage()?;
                    let mode_cost = fixed_mode_cost(top_mode, left_mode, mode);
                    checkpoint.after_candidate_stage()?;
                    let score = 256_u32
                        .wrapping_mul(distortion)
                        .wrapping_add(11_u32.wrapping_mul(u32::from(mode_cost)));
                    if selected.is_none_or(|(best, _)| score < best) {
                        selected = Some((score, mode));
                    }
                    checkpoint.after_candidate()?;
                }
                // The complete intra4 mode set is statically non-empty.
                #[allow(clippy::expect_used)]
                Some(selected.expect("VP8 always has intra4 candidates").1)
            } else {
                None
            };

            type Candidate = (u64, Intra4Mode, [i16; 16], [u8; 16], u32, u32, u32, u32);
            let mut best: Option<Candidate> = None;
            for mode in Intra4Mode::ALL
                .into_iter()
                .filter(|&mode| selected_mode.is_none_or(|selected| selected == mode))
            {
                let prediction = predict(mode, &top, &left, block_top_left);
                checkpoint.after_candidate_stage()?;
                let (nonzero, levels, reconstructed) = if trellis {
                    let residual = std::array::from_fn(|index| {
                        i16::from(block_source[index]).wrapping_sub(i16::from(prediction[index]))
                    });
                    let mut coefficients = checkpoint.forward_transform(&residual)?;
                    let mut levels = [0; 16];
                    let nonzero = checkpoint.trellis_quantize_block(
                        &mut coefficients,
                        &mut levels,
                        context,
                        3,
                        &matrices.y1,
                        matrices.lambda_trellis_i4,
                        coefficient_probabilities,
                    )?;
                    checkpoint.after_candidate_stage()?;
                    let reconstructed = checkpoint.inverse_transform(&prediction, &coefficients)?;
                    (nonzero, levels, reconstructed)
                } else {
                    let residual = std::array::from_fn(|index| {
                        i16::from(block_source[index]).wrapping_sub(i16::from(prediction[index]))
                    });
                    let mut coefficients = checkpoint.forward_transform(&residual)?;
                    let mut levels = [0; 16];
                    let nonzero =
                        checkpoint.quantize_block(&mut coefficients, &mut levels, &matrices.y1)?;
                    let reconstructed = checkpoint.inverse_transform(&prediction, &coefficients)?;
                    (nonzero, levels, reconstructed)
                };
                let distortion = squared_error_4x4(&block_source, &reconstructed);
                checkpoint.after_candidate_stage()?;
                let texture = spectral_distortion_4x4(&block_source, &reconstructed);
                checkpoint.after_candidate_stage()?;
                let spectral = texture_lambda
                    .wrapping_mul(texture)
                    .wrapping_add(128)
                    .wrapping_shr(8);
                let header = u32::from(fixed_mode_cost(top_mode, left_mode, mode));
                let flat_penalty = if mode != Intra4Mode::Dc
                    && levels[1..].iter().filter(|&&level| level != 0).count() <= 3
                {
                    140
                } else {
                    0
                };
                let preliminary_score = rd_score(
                    flat_penalty,
                    header,
                    distortion.wrapping_add(spectral),
                    lambda_i4,
                );
                if !distortion_only
                    && best
                        .as_ref()
                        .is_some_and(|best| preliminary_score >= best.0)
                {
                    checkpoint.after_candidate()?;
                    continue;
                }
                let rate = flat_penalty.wrapping_add(residual_cost(
                    &levels,
                    0,
                    3,
                    context,
                    coefficient_probabilities,
                ));
                checkpoint.after_candidate_stage()?;
                let score = rd_score(rate, header, distortion.wrapping_add(spectral), lambda_i4);
                if distortion_only || best.as_ref().is_none_or(|best| score < best.0) {
                    best = Some((
                        score,
                        mode,
                        levels,
                        reconstructed,
                        distortion,
                        spectral,
                        header,
                        rate,
                    ));
                }
                let _ = nonzero;
                checkpoint.after_candidate()?;
            }
            // The selected mode is always evaluated by the loop above.
            #[allow(clippy::expect_used)]
            let (_, mode, levels, reconstructed, distortion, spectral, header, rate) =
                best.expect("VP8 always has intra4 candidates");
            let nonzero = levels.iter().any(|&level| level != 0);
            result.modes[block_index] = mode;
            result.levels[block_index] = levels;
            result.distortion = result.distortion.wrapping_add(distortion);
            result.spectral_distortion = result.spectral_distortion.wrapping_add(spectral);
            result.header_cost = result.header_cost.wrapping_add(header);
            result.rate_cost = result.rate_cost.wrapping_add(rate);
            result.score = result.score.wrapping_add(rd_score(
                rate,
                header,
                distortion.wrapping_add(spectral),
                lambda_mode,
            ));
            if nonzero {
                result.nonzero |= 1_u32.wrapping_shl(block_index.to_le_bytes()[0].into());
            }
            top_nonzero[block_x] = u8::from(nonzero);
            left_nonzero[block_y] = u8::from(nonzero);
            for row in 0_usize..4 {
                let destination_offset = block_y
                    .wrapping_mul(4)
                    .wrapping_add(row)
                    .wrapping_mul(16)
                    .wrapping_add(block_x.wrapping_mul(4));
                let source_offset = row.wrapping_mul(4);
                result.reconstructed[destination_offset..destination_offset.wrapping_add(4)]
                    .copy_from_slice(&reconstructed[source_offset..source_offset.wrapping_add(4)]);
            }
            checkpoint.after_block()?;
        }
    }
    Ok(result)
}

fn average_two(a: u8, b: u8) -> u8 {
    u16::from(a)
        .wrapping_add(u16::from(b))
        .wrapping_add(1)
        .wrapping_shr(1)
        .to_le_bytes()[0]
}

fn average_three(a: u8, b: u8, c: u8) -> u8 {
    u16::from(a)
        .wrapping_add(2_u16.wrapping_mul(u16::from(b)))
        .wrapping_add(u16::from(c))
        .wrapping_add(2)
        .wrapping_shr(2)
        .to_le_bytes()[0]
}

fn set(output: &mut [u8; 16], column: usize, row: usize, value: u8) {
    output[row.wrapping_mul(4).wrapping_add(column)] = value;
}

pub(super) fn predict(mode: Intra4Mode, top: &[u8; 8], left: &[u8; 4], top_left: u8) -> [u8; 16] {
    let [a, b, c, d, e, f, g, h] = *top;
    let [i, j, k, l] = *left;
    let x = top_left;
    let mut output = [0; 16];

    match mode {
        Intra4Mode::Dc => {
            let dc = top[..4]
                .iter()
                .chain(left)
                .fold(4_u32, |sum, &value| sum.wrapping_add(u32::from(value)))
                .wrapping_shr(3);
            output.fill(dc.to_le_bytes()[0]);
        }
        Intra4Mode::TrueMotion => {
            for (row, &left_value) in left.iter().enumerate() {
                for (column, &top_value) in top.iter().take(4).enumerate() {
                    set(
                        &mut output,
                        column,
                        row,
                        i16::from(top_value)
                            .wrapping_add(i16::from(left_value))
                            .wrapping_sub(i16::from(x))
                            .clamp(0, 255)
                            .to_le_bytes()[0],
                    );
                }
            }
        }
        Intra4Mode::Vertical => {
            let row = [
                average_three(x, a, b),
                average_three(a, b, c),
                average_three(b, c, d),
                average_three(c, d, e),
            ];
            for row_index in 0_usize..4 {
                let row_start = row_index.wrapping_mul(4);
                output[row_start..row_start.wrapping_add(4)].copy_from_slice(&row);
            }
        }
        Intra4Mode::Horizontal => {
            let rows = [
                average_three(x, i, j),
                average_three(i, j, k),
                average_three(j, k, l),
                average_three(k, l, l),
            ];
            for (row, value) in rows.into_iter().enumerate() {
                let row_start = row.wrapping_mul(4);
                output[row_start..row_start.wrapping_add(4)].fill(value);
            }
        }
        Intra4Mode::DownRight => {
            let references = [l, k, j, i, x, a, b, c, d];
            for row in 0_usize..4 {
                for column in 0_usize..4 {
                    let center = 4_usize.wrapping_add(column).wrapping_sub(row);
                    set(
                        &mut output,
                        column,
                        row,
                        average_three(
                            references[center.wrapping_sub(1)],
                            references[center],
                            references[center.wrapping_add(1)],
                        ),
                    );
                }
            }
        }
        Intra4Mode::VerticalRight => {
            output = [
                average_two(x, a),
                average_two(a, b),
                average_two(b, c),
                average_two(c, d),
                average_three(i, x, a),
                average_three(x, a, b),
                average_three(a, b, c),
                average_three(b, c, d),
                average_three(j, i, x),
                average_two(x, a),
                average_two(a, b),
                average_two(b, c),
                average_three(k, j, i),
                average_three(i, x, a),
                average_three(x, a, b),
                average_three(a, b, c),
            ];
        }
        Intra4Mode::DownLeft => {
            let references = [a, b, c, d, e, f, g, h, h];
            for row in 0_usize..4 {
                for column in 0_usize..4 {
                    let index = row.wrapping_add(column);
                    set(
                        &mut output,
                        column,
                        row,
                        average_three(
                            references[index],
                            references[index.wrapping_add(1)],
                            references[index.wrapping_add(2)],
                        ),
                    );
                }
            }
        }
        Intra4Mode::VerticalLeft => {
            output = [
                average_two(a, b),
                average_two(b, c),
                average_two(c, d),
                average_two(d, e),
                average_three(a, b, c),
                average_three(b, c, d),
                average_three(c, d, e),
                average_three(d, e, f),
                average_two(b, c),
                average_two(c, d),
                average_two(d, e),
                average_three(e, f, g),
                average_three(b, c, d),
                average_three(c, d, e),
                average_three(d, e, f),
                average_three(f, g, h),
            ];
        }
        Intra4Mode::HorizontalDown => {
            output = [
                average_two(i, x),
                average_three(i, x, a),
                average_three(x, a, b),
                average_three(a, b, c),
                average_two(j, i),
                average_three(j, i, x),
                average_two(i, x),
                average_three(i, x, a),
                average_two(k, j),
                average_three(k, j, i),
                average_two(j, i),
                average_three(j, i, x),
                average_two(l, k),
                average_three(l, k, j),
                average_two(k, j),
                average_three(k, j, i),
            ];
        }
        Intra4Mode::HorizontalUp => {
            output = [
                average_two(i, j),
                average_three(i, j, k),
                average_two(j, k),
                average_three(j, k, l),
                average_two(j, k),
                average_three(j, k, l),
                average_two(k, l),
                average_three(k, l, l),
                average_two(k, l),
                average_three(k, l, l),
                l,
                l,
                l,
                l,
                l,
                l,
            ];
        }
    }
    output
}
