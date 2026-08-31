//! AV1 film-grain synthesis for the bounded portable display path.
//!
//! The implementation follows the normative fixed-point kernels in dav1d
//! 1.5.3 (`src/filmgrain_tmpl.c`, `src/fg_apply_tmpl.c`, and `src/tables.c`)
//! and libaom 3.13.2 (`av1/decoder/grain_synthesis.c`). It is deliberately
//! display-only: retained reference surfaces remain post-filtered and
//! ungrained. The existing `NOTICE.md`, `PATENTS`, and third-party BSD notices
//! cover this source-derived translation; no native codec is linked.

use super::block::{FirstLeaf, ReconstructedPlane};
use super::frame::FilmGrain;
use super::{Av1Result, malformed};
use crate::CancellationToken;

const AR_PAD: usize = 3;
const GRAIN_WIDTH: usize = 82;
const GRAIN_HEIGHT: usize = 73;
const SUB_GRAIN_WIDTH: usize = 44;
const SUB_GRAIN_HEIGHT: usize = 38;
const BLOCK_SIZE: usize = 32;

#[derive(Clone, Copy)]
struct GrainSampling {
    subx: usize,
    suby: usize,
}

impl GrainSampling {
    const I420: Self = Self { subx: 1, suby: 1 };
    const I422: Self = Self { subx: 1, suby: 0 };
    const I444: Self = Self { subx: 0, suby: 0 };

    fn validate(self) -> Av1Result<()> {
        if self.subx > 1 || self.suby > 1 {
            return Err(malformed("film-grain subsampling exceeds AV1 bounds"));
        }
        Ok(())
    }

    fn plane_extent(
        self,
        value: usize,
        subsampling: usize,
        label: &'static str,
    ) -> Av1Result<usize> {
        if subsampling == 0 {
            return Ok(value);
        }
        value
            .checked_add(1)
            .ok_or_else(|| malformed(label))
            .map(|value| value / 2)
    }

    fn grain_dimensions(self) -> (usize, usize) {
        (
            if self.subx == 0 {
                GRAIN_WIDTH
            } else {
                SUB_GRAIN_WIDTH
            },
            if self.suby == 0 {
                GRAIN_HEIGHT
            } else {
                SUB_GRAIN_HEIGHT
            },
        )
    }

    fn luma_scale(self) -> (usize, usize) {
        (1_usize << self.subx, 1_usize << self.suby)
    }
}

// ✅ VERIFIED: dav1d 1.5.3 src/tables.c:1118-1176.
// This is the normative 11-bit Gaussian sequence, not a generated
// approximation. Keep it literal so every seed has the pinned distribution.
const GAUSSIAN_SEQUENCE: [i16; 2048] = [
    56, 568, -180, 172, 124, -84, 172, -64, -900, 24, 820, 224, 1248, 996, 272, -8, -916, -388,
    -732, -104, -188, 800, 112, -652, -320, -376, 140, -252, 492, -168, 44, -788, 588, -584, 500,
    -228, 12, 680, 272, -476, 972, -100, 652, 368, 432, -196, -720, -192, 1000, -332, 652, -136,
    -552, -604, -4, 192, -220, -136, 1000, -52, 372, -96, -624, 124, -24, 396, 540, -12, -104, 640,
    464, 244, -208, -84, 368, -528, -740, 248, -968, -848, 608, 376, -60, -292, -40, -156, 252,
    -292, 248, 224, -280, 400, -244, 244, -60, 76, -80, 212, 532, 340, 128, -36, 824, -352, -60,
    -264, -96, -612, 416, -704, 220, -204, 640, -160, 1220, -408, 900, 336, 20, -336, -96, -792,
    304, 48, -28, -1232, -1172, -448, 104, -292, -520, 244, 60, -948, 0, -708, 268, 108, 356, -548,
    488, -344, -136, 488, -196, -224, 656, -236, -1128, 60, 4, 140, 276, -676, -376, 168, -108,
    464, 8, 564, 64, 240, 308, -300, -400, -456, -136, 56, 120, -408, -116, 436, 504, -232, 328,
    844, -164, -84, 784, -168, 232, -224, 348, -376, 128, 568, 96, -1244, -288, 276, 848, 832,
    -360, 656, 464, -384, -332, -356, 728, -388, 160, -192, 468, 296, 224, 140, -776, -100, 280, 4,
    196, 44, -36, -648, 932, 16, 1428, 28, 528, 808, 772, 20, 268, 88, -332, -284, 124, -384, -448,
    208, -228, -1044, -328, 660, 380, -148, -300, 588, 240, 540, 28, 136, -88, -436, 256, 296,
    -1000, 1400, 0, -48, 1056, -136, 264, -528, -1108, 632, -484, -592, -344, 796, 124, -668, -768,
    388, 1296, -232, -188, -200, -288, -4, 308, 100, -168, 256, -500, 204, -508, 648, -136, 372,
    -272, -120, -1004, -552, -548, -384, 548, -296, 428, -108, -8, -912, -324, -224, -88, -112,
    -220, -100, 996, -796, 548, 360, -216, 180, 428, -200, -212, 148, 96, 148, 284, 216, -412,
    -320, 120, -300, -384, -604, -572, -332, -8, -180, -176, 696, 116, -88, 628, 76, 44, -516, 240,
    -208, -40, 100, -592, 344, -308, -452, -228, 20, 916, -1752, -136, -340, -804, 140, 40, 512,
    340, 248, 184, -492, 896, -156, 932, -628, 328, -688, -448, -616, -752, -100, 560, -1020, 180,
    -800, -64, 76, 576, 1068, 396, 660, 552, -108, -28, 320, -628, 312, -92, -92, -472, 268, 16,
    560, 516, -672, -52, 492, -100, 260, 384, 284, 292, 304, -148, 88, -152, 1012, 1064, -228, 164,
    -376, -684, 592, -392, 156, 196, -524, -64, -884, 160, -176, 636, 648, 404, -396, -436, 864,
    424, -728, 988, -604, 904, -592, 296, -224, 536, -176, -920, 436, -48, 1176, -884, 416, -776,
    -824, -884, 524, -548, -564, -68, -164, -96, 692, 364, -692, -1012, -68, 260, -480, 876, -1116,
    452, -332, -352, 892, -1088, 1220, -676, 12, -292, 244, 496, 372, -32, 280, 200, 112, -440,
    -96, 24, -644, -184, 56, -432, 224, -980, 272, -260, 144, -436, 420, 356, 364, -528, 76, 172,
    -744, -368, 404, -752, -416, 684, -688, 72, 540, 416, 92, 444, 480, -72, -1416, 164, -1172,
    -68, 24, 424, 264, 1040, 128, -912, -524, -356, 64, 876, -12, 4, -88, 532, 272, -524, 320, 276,
    -508, 940, 24, -400, -120, 756, 60, 236, -412, 100, 376, -484, 400, -100, -740, -108, -260,
    328, -268, 224, -200, -416, 184, -604, -564, -20, 296, 60, 892, -888, 60, 164, 68, -760, 216,
    -296, 904, -336, -28, 404, -356, -568, -208, -1480, -512, 296, 328, -360, -164, -1560, -776,
    1156, -428, 164, -504, -112, 120, -216, -148, -264, 308, 32, 64, -72, 72, 116, 176, -64, -272,
    460, -536, -784, -280, 348, 108, -752, -132, 524, -540, -776, 116, -296, -1196, -288, -560,
    1040, -472, 116, -848, -1116, 116, 636, 696, 284, -176, 1016, 204, -864, -648, -248, 356, 972,
    -584, -204, 264, 880, 528, -24, -184, 116, 448, -144, 828, 524, 212, -212, 52, 12, 200, 268,
    -488, -404, -880, 824, -672, -40, 908, -248, 500, 716, -576, 492, -576, 16, 720, -108, 384,
    124, 344, 280, 576, -500, 252, 104, -308, 196, -188, -8, 1268, 296, 1032, -1196, 436, 316, 372,
    -432, -200, -660, 704, -224, 596, -132, 268, 32, -452, 884, 104, -1008, 424, -1348, -280, 4,
    -1168, 368, 476, 696, 300, -8, 24, 180, -592, -196, 388, 304, 500, 724, -160, 244, -84, 272,
    -256, -420, 320, 208, -144, -156, 156, 364, 452, 28, 540, 316, 220, -644, -248, 464, 72, 360,
    32, -388, 496, -680, -48, 208, -116, -408, 60, -604, -392, 548, -840, 784, -460, 656, -544,
    -388, -264, 908, -800, -628, -612, -568, 572, -220, 164, 288, -16, -308, 308, -112, -636, -760,
    280, -668, 432, 364, 240, -196, 604, 340, 384, 196, 592, -44, -500, 432, -580, -132, 636, -76,
    392, 4, -412, 540, 508, 328, -356, -36, 16, -220, -64, -248, -60, 24, -192, 368, 1040, 92, -24,
    -1044, -32, 40, 104, 148, 192, -136, -520, 56, -816, -224, 732, 392, 356, 212, -80, -424,
    -1008, -324, 588, -1496, 576, 460, -816, -848, 56, -580, -92, -1372, -112, -496, 200, 364, 52,
    -140, 48, -48, -60, 84, 72, 40, 132, -356, -268, -104, -284, -404, 732, -520, 164, -304, -540,
    120, 328, -76, -460, 756, 388, 588, 236, -436, -72, -176, -404, -316, -148, 716, -604, 404,
    -72, -88, -888, -68, 944, 88, -220, -344, 960, 472, 460, -232, 704, 120, 832, -228, 692, -508,
    132, -476, 844, -748, -364, -44, 1116, -1104, -1056, 76, 428, 552, -692, 60, 356, 96, -384,
    -188, -612, -576, 736, 508, 892, 352, -1132, 504, -24, -352, 324, 332, -600, -312, 292, 508,
    -144, -8, 484, 48, 284, -260, -240, 256, -100, -292, -204, -44, 472, -204, 908, -188, -1000,
    -256, 92, 1164, -392, 564, 356, 652, -28, -884, 256, 484, -192, 760, -176, 376, -524, -452,
    -436, 860, -736, 212, 124, 504, -476, 468, 76, -472, 552, -692, -944, -620, 740, -240, 400,
    132, 20, 192, -196, 264, -668, -1012, -60, 296, -316, -828, 76, -156, 284, -768, -448, -832,
    148, 248, 652, 616, 1236, 288, -328, -400, -124, 588, 220, 520, -696, 1032, 768, -740, -92,
    -272, 296, 448, -464, 412, -200, 392, 440, -200, 264, -152, -260, 320, 1032, 216, 320, -8, -64,
    156, -1016, 1084, 1172, 536, 484, -432, 132, 372, -52, -256, 84, 116, -352, 48, 116, 304, -384,
    412, 924, -300, 528, 628, 180, 648, 44, -980, -220, 1320, 48, 332, 748, 524, -268, -720, 540,
    -276, 564, -344, -208, -196, 436, 896, 88, -392, 132, 80, -964, -288, 568, 56, -48, -456, 888,
    8, 552, -156, -292, 948, 288, 128, -716, -292, 1192, -152, 876, 352, -600, -260, -812, -468,
    -28, -120, -32, -44, 1284, 496, 192, 464, 312, -76, -516, -380, -456, -1012, -48, 308, -156,
    36, 492, -156, -808, 188, 1652, 68, -120, -116, 316, 160, -140, 352, 808, -416, 592, 316, -480,
    56, 528, -204, -568, 372, -232, 752, -344, 744, -4, 324, -416, -600, 768, 268, -248, -88, -132,
    -420, -432, 80, -288, 404, -316, -1216, -588, 520, -108, 92, -320, 368, -480, -216, -92, 1688,
    -300, 180, 1020, -176, 820, -68, -228, -260, 436, -904, 20, 40, -508, 440, -736, 312, 332, 204,
    760, -372, 728, 96, -20, -632, -520, -560, 336, 1076, -64, -532, 776, 584, 192, 396, -728,
    -520, 276, -188, 80, -52, -612, -252, -48, 648, 212, -688, 228, -52, -260, 428, -412, -272,
    -404, 180, 816, -796, 48, 152, 484, -88, -216, 988, 696, 188, -528, 648, -116, -180, 316, 476,
    12, -564, 96, 476, -252, -364, -376, -392, 556, -256, -576, 260, -352, 120, -16, -136, -260,
    -492, 72, 556, 660, 580, 616, 772, 436, 424, -32, -324, -1268, 416, -324, -80, 920, 160, 228,
    724, 32, -516, 64, 384, 68, -128, 136, 240, 248, -204, -68, 252, -932, -120, -480, -628, -84,
    192, 852, -404, -288, -132, 204, 100, 168, -68, -196, -868, 460, 1080, 380, -80, 244, 0, 484,
    -888, 64, 184, 352, 600, 460, 164, 604, -196, 320, -64, 588, -184, 228, 12, 372, 48, -848,
    -344, 224, 208, -200, 484, 128, -20, 272, -468, -840, 384, 256, -720, -520, -464, -580, 112,
    -120, 644, -356, -208, -608, -528, 704, 560, -424, 392, 828, 40, 84, 200, -152, 0, -144, 584,
    280, -120, 80, -556, -972, -196, -472, 724, 80, 168, -32, 88, 160, -688, 0, 160, 356, 372,
    -776, 740, -128, 676, -248, -480, 4, -364, 96, 544, 232, -1032, 956, 236, 356, 20, -40, 300,
    24, -676, -596, 132, 1120, -104, 532, -1096, 568, 648, 444, 508, 380, 188, -376, -604, 1488,
    424, 24, 756, -220, -192, 716, 120, 920, 688, 168, 44, -460, 568, 284, 1144, 1160, 600, 424,
    888, 656, -356, -320, 220, 316, -176, -724, -188, -816, -628, -348, -228, -380, 1012, -452,
    -660, 736, 928, 404, -696, -72, -268, -892, 128, 184, -344, -780, 360, 336, 400, 344, 428, 548,
    -112, 136, -228, -216, -820, -516, 340, 92, -136, 116, -300, 376, -244, 100, -316, -520, -284,
    -12, 824, 164, -548, -180, -128, 116, -924, -828, 268, -368, -580, 620, 192, 160, 0, -1676,
    1068, 424, -56, -360, 468, -156, 720, 288, -528, 556, -364, 548, -148, 504, 316, 152, -648,
    -620, -684, -24, -376, -384, -108, -920, -1032, 768, 180, -264, -508, -1268, -260, -60, 300,
    -240, 988, 724, -376, -576, -212, -736, 556, 192, 1092, -620, -880, 376, -56, -4, -216, -32,
    836, 268, 396, 1332, 864, -600, 100, 56, -412, -92, 356, 180, 884, -468, -436, 292, -388, -804,
    -704, -840, 368, -348, 140, -724, 1536, 940, 372, 112, -372, 436, -480, 1136, 296, -32, -228,
    132, -48, -220, 868, -1016, -60, -1044, -464, 328, 916, 244, 12, -736, -296, 360, 468, -376,
    -108, -92, 788, 368, -56, 544, 400, -672, -420, 728, 16, 320, 44, -284, -380, -796, 488, 132,
    204, -596, -372, 88, -152, -908, -636, -572, -624, -116, -692, -200, -56, 276, -88, 484, -324,
    948, 864, 1000, -456, -184, -276, 292, -296, 156, 676, 320, 160, 908, -84, -1236, -288, -116,
    260, -372, -644, 732, -756, -96, 84, 344, -520, 348, -688, 240, -84, 216, -1044, -136, -676,
    -396, -1500, 960, -40, 176, 168, 1516, 420, -504, -344, -364, -360, 1216, -940, -380, -212,
    252, -660, -708, 484, -444, -152, 928, -120, 1112, 476, -260, 560, -148, -344, 108, -196, 228,
    -288, 504, 560, -328, -88, 288, -1008, 460, -228, 468, -836, -196, 76, 388, 232, 412, -1168,
    -716, -644, 756, -172, -356, -504, 116, 432, 528, 48, 476, -168, -608, 448, 160, -532, -272,
    28, -676, -12, 828, 980, 456, 520, 104, -104, 256, -344, -4, -28, -368, -52, -524, -572, -556,
    -200, 768, 1124, -208, -512, 176, 232, 248, -148, -888, 604, -600, -304, 804, -156, -212, 488,
    -192, -804, -256, 368, -360, -916, -328, 228, -240, -448, -472, 856, -556, -364, 572, -12,
    -156, -368, -340, 432, 252, -752, -152, 288, 268, -580, -848, -592, 108, -76, 244, 312, -716,
    592, -80, 436, 360, 4, -248, 160, 516, 584, 732, 44, -468, -280, -292, -156, -588, 28, 308,
    912, 24, 124, 156, 180, -252, 944, -924, -772, -520, -428, -624, 300, -212, -1144, 32, -724,
    800, -1128, -212, -1288, -848, 180, -416, 440, 192, -576, -792, -76, -1080, 80, -532, -352,
    -132, 380, -820, 148, 1112, 128, 164, 456, 700, -924, 144, -668, -384, 648, -832, 508, 552,
    -52, -100, -656, 208, -568, 748, -88, 680, 232, 300, 192, -408, -1012, -152, -252, -268, 272,
    -876, -664, -648, -332, -136, 16, 12, 1152, -28, 332, -536, 320, -672, -460, -316, 532, -260,
    228, -40, 1052, -816, 180, 88, -496, -556, -672, -368, 428, 92, 356, 404, -408, 252, 196, -176,
    -556, 792, 268, 32, 372, 40, 96, -332, 328, 120, 372, -900, -40, 472, -264, -592, 952, 128,
    656, 112, 664, -232, 420, 4, -344, -464, 556, 244, -416, -32, 252, 0, -412, 188, -696, 508,
    -476, 324, -1096, 656, -312, 560, 264, -136, 304, 160, -64, -580, 248, 336, -720, 560, -348,
    -288, -276, -196, -500, 852, -544, -236, -1128, -992, -776, 116, 56, 52, 860, 884, 212, -12,
    168, 1020, 512, -552, 924, -148, 716, 188, 164, -340, -520, -184, 880, -152, -680, -208, -1156,
    -300, -528, -472, 364, 100, -744, -1056, -32, 540, 280, 144, -676, -32, -232, -280, -224, 96,
    568, -76, 172, 148, 148, 104, 32, -296, -32, 788, -80, 32, -16, 280, 288, 944, 428, -484,
];

#[derive(Clone)]
struct GrainLut {
    width: usize,
    height: usize,
    values: Vec<i32>,
}

impl GrainLut {
    fn new(width: usize, height: usize) -> Av1Result<Self> {
        let length = width
            .checked_mul(height)
            .ok_or_else(|| malformed("film-grain LUT size overflows"))?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(length)
            .map_err(|_| malformed("unable to allocate film-grain LUT"))?;
        values.resize(length, 0);
        Ok(Self {
            width,
            height,
            values,
        })
    }

    fn get(&self, x: usize, y: usize) -> Av1Result<i32> {
        if x >= self.width || y >= self.height {
            return Err(malformed("film-grain LUT coordinate is outside its extent"));
        }
        let index = y
            .checked_mul(self.width)
            .and_then(|row| row.checked_add(x))
            .ok_or_else(|| malformed("film-grain LUT index overflows"))?;
        self.values
            .get(index)
            .copied()
            .ok_or_else(|| malformed("film-grain LUT index is outside its extent"))
    }

    fn set(&mut self, x: usize, y: usize, value: i32) -> Av1Result<()> {
        if x >= self.width || y >= self.height {
            return Err(malformed("film-grain LUT coordinate is outside its extent"));
        }
        let index = y
            .checked_mul(self.width)
            .and_then(|row| row.checked_add(x))
            .ok_or_else(|| malformed("film-grain LUT index overflows"))?;
        *self
            .values
            .get_mut(index)
            .ok_or_else(|| malformed("film-grain LUT index is outside its extent"))? = value;
        Ok(())
    }
}

/// Apply a parsed film-grain payload to an owned, post-filter I420 display
/// leaf. The source leaf is consumed so a cancelled or failed synthesis can
/// never publish a partially mutated reference surface.
pub(super) fn apply_i420(
    leaf: FirstLeaf,
    params: &FilmGrain,
    token: Option<&CancellationToken>,
) -> Av1Result<FirstLeaf> {
    apply_with_sampling(leaf, params, GrainSampling::I420, token)
}

/// Apply a parsed film-grain payload to an owned, post-filter I422 display
/// leaf. Horizontal chroma subsampling uses the same checked kernel as I420;
/// the vertical axis remains full resolution.
pub(super) fn apply_i422(
    leaf: FirstLeaf,
    params: &FilmGrain,
    token: Option<&CancellationToken>,
) -> Av1Result<FirstLeaf> {
    apply_with_sampling(leaf, params, GrainSampling::I422, token)
}

/// Apply a parsed film-grain payload to an owned monochrome display plane.
/// Monochrome AV1 carries only the luma grain sentence, so this path reuses
/// the normative Y synthesis without allocating private chroma carriers.
pub(super) fn apply_monochrome(
    mut plane: ReconstructedPlane,
    width: u32,
    height: u32,
    params: &FilmGrain,
    token: Option<&CancellationToken>,
) -> Av1Result<ReconstructedPlane> {
    crate::codecs::error::check_cancelled(token)?;
    if width == 0 || height == 0 || params.seed > u32::from(u16::MAX) {
        return Err(malformed(
            "monochrome film-grain display geometry or seed is invalid",
        ));
    }
    if params.chroma_scaling_from_luma
        || params.uv_points.iter().any(|points| !points.is_empty())
        || params
            .ar_coefficients_uv
            .iter()
            .any(|coefficients| !coefficients.is_empty())
    {
        return Err(malformed(
            "monochrome film-grain payload contains chroma syntax",
        ));
    }
    let width = usize::try_from(width)
        .map_err(|_| malformed("monochrome film-grain width exceeds usize"))?;
    let height = usize::try_from(height)
        .map_err(|_| malformed("monochrome film-grain height exceeds usize"))?;
    let expected = checked_count(width, height, "monochrome film-grain luma extent")?;
    if plane.samples.len() != expected
        || plane
            .samples
            .iter()
            .any(|&sample| sample > u16::from(u8::MAX))
    {
        return Err(malformed(
            "monochrome film-grain display plane has invalid extent",
        ));
    }
    validate_parameters(params)?;
    let y_lut = generate_luma_lut(params, token)?;
    let y_scaling = generate_scaling(&params.y_points)?;
    plane.samples = apply_plane(
        &plane.samples,
        (width, height),
        false,
        GrainSampling::I444,
        &y_lut,
        &y_scaling,
        params,
        0,
        &[],
        (width, height),
        token,
    )?;
    Ok(plane)
}

/// Apply a parsed film-grain payload to an owned, post-filter I444 display
/// leaf. I444 film grain is intentionally limited to eight-bit samples; the
/// entropy admission and display materializer enforce that restriction.
pub(super) fn apply_i444(
    leaf: FirstLeaf,
    params: &FilmGrain,
    token: Option<&CancellationToken>,
) -> Av1Result<FirstLeaf> {
    apply_with_sampling(leaf, params, GrainSampling::I444, token)
}

fn apply_with_sampling(
    mut leaf: FirstLeaf,
    params: &FilmGrain,
    sampling: GrainSampling,
    token: Option<&CancellationToken>,
) -> Av1Result<FirstLeaf> {
    crate::codecs::error::check_cancelled(token)?;
    sampling.validate()?;
    if leaf.width == 0 || leaf.height == 0 || params.seed > u32::from(u16::MAX) {
        return Err(malformed("film-grain display geometry or seed is invalid"));
    }
    let width = usize::try_from(leaf.width)
        .map_err(|_| malformed("film-grain luma width exceeds usize"))?;
    let height = usize::try_from(leaf.height)
        .map_err(|_| malformed("film-grain luma height exceeds usize"))?;
    let chroma_width =
        sampling.plane_extent(width, sampling.subx, "film-grain chroma width overflows")?;
    let chroma_height =
        sampling.plane_extent(height, sampling.suby, "film-grain chroma height overflows")?;
    let expected = [
        checked_count(width, height, "film-grain luma extent")?,
        checked_count(chroma_width, chroma_height, "film-grain U extent")?,
        checked_count(chroma_width, chroma_height, "film-grain V extent")?,
    ];
    if leaf.planes[0].samples.len() != expected[0]
        || leaf.planes[1].samples.len() != expected[1]
        || leaf.planes[2].samples.len() != expected[2]
        || leaf.planes.iter().any(|plane| {
            plane
                .samples
                .iter()
                .any(|&sample| sample > u16::from(u8::MAX))
        })
    {
        return Err(malformed("film-grain display planes have invalid extents"));
    }
    validate_parameters(params)?;

    let y_lut = generate_luma_lut(params, token)?;
    let y_scaling = generate_scaling(&params.y_points)?;
    let y_source = copy_samples(&leaf.planes[0].samples, "film-grain luma source")?;
    let u_lut = if params.uv_points[0].is_empty() && !params.chroma_scaling_from_luma {
        None
    } else {
        Some(generate_chroma_lut(params, 0, &y_lut, sampling, token)?)
    };
    let v_lut = if params.uv_points[1].is_empty() && !params.chroma_scaling_from_luma {
        None
    } else {
        Some(generate_chroma_lut(params, 1, &y_lut, sampling, token)?)
    };
    let u_scaling = if params.chroma_scaling_from_luma {
        None
    } else {
        Some(generate_scaling(&params.uv_points[0])?)
    };
    let v_scaling = if params.chroma_scaling_from_luma {
        None
    } else {
        Some(generate_scaling(&params.uv_points[1])?)
    };
    let u_output = u_lut
        .as_ref()
        .map(|lut| {
            apply_plane(
                &leaf.planes[1].samples,
                (chroma_width, chroma_height),
                true,
                sampling,
                lut,
                u_scaling.as_deref().unwrap_or(&y_scaling),
                params,
                0,
                &y_source,
                (width, height),
                token,
            )
        })
        .transpose()?;
    let v_output = v_lut
        .as_ref()
        .map(|lut| {
            apply_plane(
                &leaf.planes[2].samples,
                (chroma_width, chroma_height),
                true,
                sampling,
                lut,
                v_scaling.as_deref().unwrap_or(&y_scaling),
                params,
                1,
                &y_source,
                (width, height),
                token,
            )
        })
        .transpose()?;
    let y_output = apply_plane(
        &leaf.planes[0].samples,
        (width, height),
        false,
        GrainSampling { subx: 0, suby: 0 },
        &y_lut,
        &y_scaling,
        params,
        0,
        &[],
        (0, 0),
        token,
    )?;
    if let Some(output) = u_output {
        leaf.planes[1].samples = output;
    }
    if let Some(output) = v_output {
        leaf.planes[2].samples = output;
    }
    leaf.planes[0].samples = y_output;
    Ok(leaf)
}

fn validate_parameters(params: &FilmGrain) -> Av1Result<()> {
    if params.y_points.len() > 14
        || params.uv_points.iter().any(|points| points.len() > 10)
        || params.scaling_shift > 11
        || params.ar_coefficient_lag > 3
        || params.ar_coefficient_shift > 9
        || params.grain_scale_shift > 3
    {
        return Err(malformed("film-grain syntax values exceed AV1 bounds"));
    }
    for points in std::iter::once(&params.y_points).chain(params.uv_points.iter()) {
        for pair in points.windows(2) {
            if pair[0][0] >= pair[1][0] {
                return Err(malformed("film-grain scaling points are not increasing"));
            }
        }
        if points
            .iter()
            .any(|point| point[0] > u32::from(u8::MAX) || point[1] > u32::from(u8::MAX))
        {
            return Err(malformed(
                "film-grain scaling point exceeds eight-bit range",
            ));
        }
    }
    let lag = usize::try_from(params.ar_coefficient_lag)
        .map_err(|_| malformed("film-grain AR lag exceeds usize"))?;
    let ar_count = lag
        .checked_mul(lag.saturating_add(1))
        .and_then(|value| value.checked_mul(2))
        .ok_or_else(|| malformed("film-grain AR coefficient count overflows"))?;
    if params.ar_coefficients_y.len()
        != if params.y_points.is_empty() {
            0
        } else {
            ar_count
        }
        || params
            .ar_coefficients_uv
            .iter()
            .enumerate()
            .any(|(plane, coeffs)| {
                let active = !params.uv_points[plane].is_empty() || params.chroma_scaling_from_luma;
                coeffs.len()
                    != if active {
                        ar_count.saturating_add(usize::from(!params.y_points.is_empty()))
                    } else {
                        0
                    }
            })
    {
        return Err(malformed(
            "film-grain AR coefficient vectors have invalid lengths",
        ));
    }
    Ok(())
}

fn checked_count(width: usize, height: usize, label: &'static str) -> Av1Result<usize> {
    width.checked_mul(height).ok_or_else(|| malformed(label))
}

fn copy_samples(source: &[u16], label: &'static str) -> Av1Result<Vec<u16>> {
    let mut copy = Vec::new();
    copy.try_reserve_exact(source.len())
        .map_err(|_| malformed(label))?;
    copy.extend_from_slice(source);
    Ok(copy)
}

fn generate_luma_lut(params: &FilmGrain, token: Option<&CancellationToken>) -> Av1Result<GrainLut> {
    let mut lut = GrainLut::new(GRAIN_WIDTH, GRAIN_HEIGHT)?;
    let mut seed = params.seed;
    let shift = 4_u32
        .checked_add(params.grain_scale_shift)
        .ok_or_else(|| malformed("film-grain luma shift overflows"))?;
    fill_initial_lut(&mut lut, &mut seed, shift, token)?;
    apply_ar_lut(
        &mut lut,
        &params.ar_coefficients_y,
        params.ar_coefficient_lag,
        params.ar_coefficient_shift,
        token,
    )?;
    Ok(lut)
}

fn generate_chroma_lut(
    params: &FilmGrain,
    plane: usize,
    luma_lut: &GrainLut,
    sampling: GrainSampling,
    token: Option<&CancellationToken>,
) -> Av1Result<GrainLut> {
    let (width, height) = sampling.grain_dimensions();
    let mut lut = GrainLut::new(width, height)?;
    let mut seed = params.seed ^ if plane == 0 { 0xb524 } else { 0x49d8 };
    let shift = 4_u32
        .checked_add(params.grain_scale_shift)
        .ok_or_else(|| malformed("film-grain chroma shift overflows"))?;
    fill_initial_lut(&mut lut, &mut seed, shift, token)?;
    apply_chroma_ar_lut(&mut lut, params, plane, luma_lut, sampling, token)?;
    Ok(lut)
}

fn fill_initial_lut(
    lut: &mut GrainLut,
    seed: &mut u32,
    shift: u32,
    token: Option<&CancellationToken>,
) -> Av1Result<()> {
    let shift = u8::try_from(shift).map_err(|_| malformed("film-grain shift exceeds u8"))?;
    for y in 0..lut.height {
        if y.is_multiple_of(8) {
            crate::codecs::error::check_cancelled(token)?;
        }
        for x in 0..lut.width {
            let index = usize::try_from(random_number(11, seed)?)
                .map_err(|_| malformed("film-grain Gaussian index is negative"))?;
            let value = round_signed(i32::from(GAUSSIAN_SEQUENCE[index]), shift)?;
            lut.set(x, y, value.clamp(-128, 127))?;
        }
    }
    Ok(())
}

fn apply_ar_lut(
    lut: &mut GrainLut,
    coefficients: &[i32],
    lag: u32,
    coefficient_shift: u32,
    token: Option<&CancellationToken>,
) -> Av1Result<()> {
    let lag = usize::try_from(lag).map_err(|_| malformed("film-grain AR lag exceeds usize"))?;
    let shift =
        u8::try_from(coefficient_shift).map_err(|_| malformed("film-grain AR shift exceeds u8"))?;
    let mut coefficient_index = 0_usize;
    for y in 0..lut.height.saturating_sub(AR_PAD) {
        if y.is_multiple_of(8) {
            crate::codecs::error::check_cancelled(token)?;
        }
        for x in 0..lut.width.saturating_sub(2 * AR_PAD) {
            let mut sum = 0_i64;
            for dy in 0..=lag {
                for dx in 0..=lag.saturating_mul(2) {
                    if dy == lag && dx == lag {
                        break;
                    }
                    let sample = lut.get(
                        x.checked_add(AR_PAD)
                            .and_then(|value| value.checked_add(dx))
                            .and_then(|value| value.checked_sub(lag))
                            .ok_or_else(|| malformed("film-grain AR x overflows"))?,
                        y.checked_add(AR_PAD)
                            .and_then(|value| value.checked_add(dy))
                            .and_then(|value| value.checked_sub(lag))
                            .ok_or_else(|| malformed("film-grain AR y overflows"))?,
                    )?;
                    let coefficient = *coefficients
                        .get(coefficient_index)
                        .ok_or_else(|| malformed("film-grain AR coefficient is missing"))?;
                    sum = sum
                        .checked_add(i64::from(coefficient) * i64::from(sample))
                        .ok_or_else(|| malformed("film-grain AR sum overflows"))?;
                    coefficient_index = coefficient_index
                        .checked_add(1)
                        .ok_or_else(|| malformed("film-grain AR coefficient index overflows"))?;
                }
            }
            let current = lut.get(x + AR_PAD, y + AR_PAD)?;
            let updated = current
                .checked_add(
                    i32::try_from(round_signed_i64(sum, shift)?)
                        .map_err(|_| malformed("film-grain AR rounded value exceeds i32"))?,
                )
                .ok_or_else(|| malformed("film-grain AR output overflows"))?
                .clamp(-128, 127);
            lut.set(x + AR_PAD, y + AR_PAD, updated)?;
            coefficient_index = 0;
        }
    }
    Ok(())
}

fn apply_chroma_ar_lut(
    lut: &mut GrainLut,
    params: &FilmGrain,
    plane: usize,
    luma_lut: &GrainLut,
    sampling: GrainSampling,
    token: Option<&CancellationToken>,
) -> Av1Result<()> {
    let lag = usize::try_from(params.ar_coefficient_lag)
        .map_err(|_| malformed("film-grain chroma AR lag exceeds usize"))?;
    let shift = u8::try_from(params.ar_coefficient_shift)
        .map_err(|_| malformed("film-grain chroma AR shift exceeds u8"))?;
    let coefficients = params
        .ar_coefficients_uv
        .get(plane)
        .ok_or_else(|| malformed("film-grain chroma plane exceeds two"))?;
    let mut coefficient_index = 0_usize;
    for y in 0..lut.height.saturating_sub(AR_PAD) {
        if y.is_multiple_of(8) {
            crate::codecs::error::check_cancelled(token)?;
        }
        for x in 0..lut.width.saturating_sub(2 * AR_PAD) {
            let mut sum = 0_i64;
            for dy in 0..=lag {
                for dx in 0..=lag.saturating_mul(2) {
                    if dy == lag && dx == lag {
                        if params.y_points.is_empty() {
                            break;
                        }
                        let (scale_x, scale_y) = sampling.luma_scale();
                        let luma_x = x
                            .checked_mul(scale_x)
                            .and_then(|value| value.checked_add(AR_PAD))
                            .ok_or_else(|| malformed("film-grain luma AR x overflows"))?;
                        let luma_y = y
                            .checked_mul(scale_y)
                            .and_then(|value| value.checked_add(AR_PAD))
                            .ok_or_else(|| malformed("film-grain luma AR y overflows"))?;
                        let luma = if params.y_points.is_empty() {
                            0
                        } else {
                            let mut total = 0_i64;
                            for row in 0..scale_y {
                                for column in 0..scale_x {
                                    total = total
                                        .checked_add(i64::from(
                                            luma_lut.get(luma_x + column, luma_y + row)?,
                                        ))
                                        .ok_or_else(|| {
                                            malformed("film-grain luma AR average overflows")
                                        })?;
                                }
                            }
                            let shift = u8::try_from(sampling.subx + sampling.suby)
                                .map_err(|_| malformed("film-grain luma AR shift exceeds u8"))?;
                            round_signed_i64(total, shift)?
                        };
                        let coefficient = *coefficients
                            .get(coefficient_index)
                            .ok_or_else(|| malformed("film-grain chroma AR coefficient missing"))?;
                        sum = sum
                            .checked_add(i64::from(coefficient).checked_mul(luma).ok_or_else(
                                || malformed("film-grain chroma AR luma product overflows"),
                            )?)
                            .ok_or_else(|| malformed("film-grain chroma AR sum overflows"))?;
                        coefficient_index = coefficient_index
                            .checked_add(1)
                            .ok_or_else(|| malformed("film-grain chroma AR index overflows"))?;
                        break;
                    }
                    let sample = lut.get(
                        x.checked_add(AR_PAD)
                            .and_then(|value| value.checked_add(dx))
                            .and_then(|value| value.checked_sub(lag))
                            .ok_or_else(|| malformed("film-grain chroma AR x overflows"))?,
                        y.checked_add(AR_PAD)
                            .and_then(|value| value.checked_add(dy))
                            .and_then(|value| value.checked_sub(lag))
                            .ok_or_else(|| malformed("film-grain chroma AR y overflows"))?,
                    )?;
                    let coefficient = *coefficients
                        .get(coefficient_index)
                        .ok_or_else(|| malformed("film-grain chroma AR coefficient missing"))?;
                    sum = sum
                        .checked_add(i64::from(coefficient) * i64::from(sample))
                        .ok_or_else(|| malformed("film-grain chroma AR sum overflows"))?;
                    coefficient_index = coefficient_index
                        .checked_add(1)
                        .ok_or_else(|| malformed("film-grain chroma AR index overflows"))?;
                }
            }
            let current = lut.get(x + AR_PAD, y + AR_PAD)?;
            let updated = current
                .checked_add(
                    i32::try_from(round_signed_i64(sum, shift)?)
                        .map_err(|_| malformed("film-grain chroma AR rounded value exceeds i32"))?,
                )
                .ok_or_else(|| malformed("film-grain chroma AR output overflows"))?
                .clamp(-128, 127);
            lut.set(x + AR_PAD, y + AR_PAD, updated)?;
            coefficient_index = 0;
        }
    }
    Ok(())
}

fn generate_scaling(points: &[[u32; 2]]) -> Av1Result<Vec<i32>> {
    let mut scaling = Vec::new();
    scaling
        .try_reserve_exact(256)
        .map_err(|_| malformed("unable to allocate film-grain scaling LUT"))?;
    scaling.resize(256, 0);
    if points.is_empty() {
        return Ok(scaling);
    }
    let first_x = usize::try_from(points[0][0])
        .map_err(|_| malformed("film-grain scaling point exceeds usize"))?;
    let first_y = i32::try_from(points[0][1])
        .map_err(|_| malformed("film-grain scaling value exceeds i32"))?;
    scaling[..first_x.min(256)].fill(first_y);
    for pair in points.windows(2) {
        let start_x = usize::try_from(pair[0][0])
            .map_err(|_| malformed("film-grain scaling point exceeds usize"))?;
        let end_x = usize::try_from(pair[1][0])
            .map_err(|_| malformed("film-grain scaling point exceeds usize"))?;
        let start_y = i64::from(
            i32::try_from(pair[0][1]).map_err(|_| malformed("film-grain scaling value invalid"))?,
        );
        let end_y = i64::from(
            i32::try_from(pair[1][1]).map_err(|_| malformed("film-grain scaling value invalid"))?,
        );
        let dx = end_x.saturating_sub(start_x);
        if dx == 0 {
            continue;
        }
        let half = dx / 2;
        let numerator = 65_536_usize
            .checked_add(half)
            .ok_or_else(|| malformed("film-grain scaling numerator overflows"))?;
        let slope = numerator / dx;
        let delta = (end_y - start_y)
            .checked_mul(i64::try_from(slope).map_err(|_| malformed("film-grain slope invalid"))?)
            .ok_or_else(|| malformed("film-grain scaling delta overflows"))?;
        let mut accumulator = 32_768_i64;
        for x in 0..dx {
            let index = start_x
                .checked_add(x)
                .ok_or_else(|| malformed("film-grain scaling index overflows"))?;
            if index < scaling.len() {
                let value = start_y
                    .checked_add(accumulator >> 16)
                    .ok_or_else(|| malformed("film-grain scaling value overflows"))?;
                scaling[index] = i32::try_from(value)
                    .map_err(|_| malformed("film-grain scaling value exceeds i32"))?;
            }
            accumulator = accumulator
                .checked_add(delta)
                .ok_or_else(|| malformed("film-grain scaling accumulator overflows"))?;
        }
    }
    let last_x = usize::try_from(points[points.len() - 1][0])
        .map_err(|_| malformed("film-grain scaling point exceeds usize"))?
        .min(256);
    let last_y = i32::try_from(points[points.len() - 1][1])
        .map_err(|_| malformed("film-grain scaling value exceeds i32"))?;
    scaling[last_x..].fill(last_y);
    Ok(scaling)
}

fn apply_plane(
    source: &[u16],
    (width, height): (usize, usize),
    chroma: bool,
    sampling: GrainSampling,
    lut: &GrainLut,
    scaling: &[i32],
    params: &FilmGrain,
    plane: usize,
    luma: &[u16],
    (luma_width, luma_height): (usize, usize),
    token: Option<&CancellationToken>,
) -> Av1Result<Vec<u16>> {
    let count = checked_count(width, height, "film-grain plane extent")?;
    if source.len() != count || scaling.len() != 256 {
        return Err(malformed("film-grain plane inputs have invalid lengths"));
    }
    if chroma {
        let luma_count = checked_count(luma_width, luma_height, "film-grain luma extent")?;
        if luma_width == 0 || luma_height == 0 || luma.len() != luma_count {
            return Err(malformed("film-grain luma reference has invalid extent"));
        }
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(count)
        .map_err(|_| malformed("unable to allocate film-grain output"))?;
    output.extend_from_slice(source);
    let block_width = if chroma {
        BLOCK_SIZE >> sampling.subx
    } else {
        BLOCK_SIZE
    };
    let block_height = if chroma {
        BLOCK_SIZE >> sampling.suby
    } else {
        BLOCK_SIZE
    };
    let rows = height.div_ceil(block_height);
    let mut offsets = [[0_i32; 2]; 2];
    for row_num in 0..rows {
        crate::codecs::error::check_cancelled(token)?;
        let current_height = block_height.min(height - row_num * block_height);
        let row_count = 1 + usize::from(params.overlap && row_num > 0);
        let mut seeds = row_seeds(row_num, params)?;
        for bx in (0..width).step_by(block_width) {
            let current_width = block_width.min(width - bx);
            if params.overlap && bx != 0 {
                let previous = offsets[0];
                for (current, previous) in
                    offsets[1].iter_mut().zip(previous.iter()).take(row_count)
                {
                    *current = *previous;
                }
            }
            for index in 0..row_count {
                offsets[0][index] = random_number(8, &mut seeds[index])?;
            }
            let x_start = if params.overlap && bx != 0 {
                (2 >> if chroma { sampling.subx } else { 0 }).min(current_width)
            } else {
                0
            };
            let y_start = if params.overlap && row_num != 0 {
                (2 >> if chroma { sampling.suby } else { 0 }).min(current_height)
            } else {
                0
            };
            for y in 0..current_height {
                for x in 0..current_width {
                    let grain = overlapped_grain(lut, offsets, sampling, x, y, x_start, y_start)?;
                    let index = (row_num * block_height + y)
                        .checked_mul(width)
                        .and_then(|row| row.checked_add(bx + x))
                        .ok_or_else(|| malformed("film-grain output index overflows"))?;
                    let source_sample = i32::from(
                        *source
                            .get(index)
                            .ok_or_else(|| malformed("film-grain source sample is missing"))?,
                    );
                    let scale_index = if chroma {
                        let (scale_x, scale_y) = sampling.luma_scale();
                        let luma_x = (bx + x)
                            .checked_mul(scale_x)
                            .ok_or_else(|| malformed("film-grain luma x overflows"))?;
                        let luma_y = (row_num * block_height + y)
                            .checked_mul(scale_y)
                            .ok_or_else(|| malformed("film-grain luma y overflows"))?;
                        let row = luma_y.min(luma_height - 1) * luma_width;
                        let left = luma_x.min(luma_width - 1);
                        let mut avg = i32::from(
                            *luma
                                .get(row + left)
                                .ok_or_else(|| malformed("film-grain luma sample is missing"))?,
                        );
                        if sampling.subx != 0 {
                            let right = luma_x.saturating_add(1).min(luma_width - 1);
                            avg =
                                (avg + i32::from(*luma.get(row + right).ok_or_else(|| {
                                    malformed("film-grain luma sample is missing")
                                })?) + 1)
                                    >> 1;
                        }
                        if params.chroma_scaling_from_luma {
                            avg
                        } else {
                            let multiplier =
                                params.uv_multiplier.get(plane).copied().ok_or_else(|| {
                                    malformed("film-grain chroma plane exceeds two")
                                })?;
                            let luma_multiplier = params
                                .uv_luma_multiplier
                                .get(plane)
                                .copied()
                                .ok_or_else(|| malformed("film-grain chroma plane exceeds two"))?;
                            let offset =
                                params.uv_offset.get(plane).copied().ok_or_else(|| {
                                    malformed("film-grain chroma plane exceeds two")
                                })?;
                            (avg.checked_mul(luma_multiplier)
                                .and_then(|value| {
                                    value.checked_add(source_sample.checked_mul(multiplier)?)
                                })
                                .ok_or_else(|| malformed("film-grain chroma scale overflows"))?
                                >> 6)
                                .checked_add(offset)
                                .ok_or_else(|| malformed("film-grain chroma offset overflows"))?
                                .clamp(0, 255)
                        }
                    } else {
                        source_sample
                    };
                    let scaling_value = *scaling
                        .get(
                            usize::try_from(scale_index.clamp(0, 255))
                                .map_err(|_| malformed("film-grain scaling index exceeds usize"))?,
                        )
                        .ok_or_else(|| malformed("film-grain scaling value is missing"))?;
                    let product = scaling_value
                        .checked_mul(grain)
                        .ok_or_else(|| malformed("film-grain noise product overflows"))?;
                    let noise = round_signed(
                        product,
                        u8::try_from(params.scaling_shift)
                            .map_err(|_| malformed("film-grain scaling shift exceeds u8"))?,
                    )?;
                    let (minimum, maximum) = if params.clip_to_restricted_range {
                        (16_i32, if chroma { 240_i32 } else { 235_i32 })
                    } else {
                        (0_i32, 255_i32)
                    };
                    let value = source_sample
                        .checked_add(noise)
                        .ok_or_else(|| malformed("film-grain output value overflows"))?
                        .clamp(minimum, maximum);
                    *output
                        .get_mut(index)
                        .ok_or_else(|| malformed("film-grain output sample is missing"))? =
                        u16::try_from(value)
                            .map_err(|_| malformed("film-grain output is negative"))?;
                }
            }
        }
    }
    Ok(output)
}

fn row_seeds(row_num: usize, params: &FilmGrain) -> Av1Result<[u32; 2]> {
    let mut seeds = [0_u32; 2];
    let count = 1 + usize::from(params.overlap && row_num > 0);
    for (index, seed) in seeds.iter_mut().take(count).enumerate() {
        let source_row = row_num
            .checked_sub(index)
            .ok_or_else(|| malformed("film-grain row seed underflows"))?;
        let row_mod = u32::try_from(source_row % 256)
            .map_err(|_| malformed("film-grain row seed exceeds u32"))?;
        let first = row_mod
            .checked_mul(37)
            .and_then(|value| value.checked_add(178))
            .ok_or_else(|| malformed("film-grain row seed overflows"))?
            & 0xff;
        let second = row_mod
            .checked_mul(173)
            .and_then(|value| value.checked_add(105))
            .ok_or_else(|| malformed("film-grain row seed overflows"))?
            & 0xff;
        *seed = params.seed ^ (first << 8) ^ second;
    }
    Ok(seeds)
}

fn overlapped_grain(
    lut: &GrainLut,
    offsets: [[i32; 2]; 2],
    sampling: GrainSampling,
    x: usize,
    y: usize,
    x_start: usize,
    y_start: usize,
) -> Av1Result<i32> {
    let mut grain = sample_lut(lut, offsets, sampling, false, false, x, y)?;
    if x < x_start && y < y_start {
        let top_left = sample_lut(lut, offsets, sampling, true, true, x, y)?;
        let top = sample_lut(lut, offsets, sampling, false, true, x, y)?;
        let top = blend(top_left, top, sampling.subx != 0, x)?;
        let left = sample_lut(lut, offsets, sampling, true, false, x, y)?;
        let left = blend(left, grain, sampling.subx != 0, x)?;
        grain = blend(top, left, sampling.suby != 0, y)?;
    } else if x < x_start {
        let old = sample_lut(lut, offsets, sampling, true, false, x, y)?;
        grain = blend(old, grain, sampling.subx != 0, x)?;
    } else if y < y_start {
        let old = sample_lut(lut, offsets, sampling, false, true, x, y)?;
        grain = blend(old, grain, sampling.suby != 0, y)?;
    }
    Ok(grain.clamp(-128, 127))
}

fn sample_lut(
    lut: &GrainLut,
    offsets: [[i32; 2]; 2],
    sampling: GrainSampling,
    old_x: bool,
    old_y: bool,
    x: usize,
    y: usize,
) -> Av1Result<i32> {
    let offset = offsets[usize::from(old_x)][usize::from(old_y)];
    let offset_x = 3_usize
        .checked_add((2 >> sampling.subx) * (3 + usize::try_from(offset >> 4).unwrap_or(0)))
        .ok_or_else(|| malformed("film-grain LUT x offset overflows"))?;
    let offset_y = 3_usize
        .checked_add((2 >> sampling.suby) * (3 + usize::try_from(offset & 0x0f).unwrap_or(0)))
        .ok_or_else(|| malformed("film-grain LUT y offset overflows"))?;
    let block_x = BLOCK_SIZE >> sampling.subx;
    let block_y = BLOCK_SIZE >> sampling.suby;
    let x = offset_x
        .checked_add(x)
        .and_then(|value| value.checked_add(block_x * usize::from(old_x)))
        .ok_or_else(|| malformed("film-grain LUT x index overflows"))?;
    let y = offset_y
        .checked_add(y)
        .and_then(|value| value.checked_add(block_y * usize::from(old_y)))
        .ok_or_else(|| malformed("film-grain LUT y index overflows"))?;
    lut.get(x, y)
}

fn blend(old: i32, current: i32, chroma: bool, index: usize) -> Av1Result<i32> {
    let (old_weight, current_weight) = if chroma {
        if index == 0 { (23_i64, 22_i64) } else { (0, 0) }
    } else if index == 0 {
        (27, 17)
    } else {
        (17, 27)
    };
    let sum = i64::from(old)
        .checked_mul(old_weight)
        .and_then(|value| value.checked_add(i64::from(current).checked_mul(current_weight)?))
        .ok_or_else(|| malformed("film-grain overlap blend overflows"))?;
    i32::try_from(round_signed_i64(sum, 5)?)
        .map(|value| value.clamp(-128, 127))
        .map_err(|_| malformed("film-grain overlap blend exceeds i32"))
}

fn random_number(bits: u8, state: &mut u32) -> Av1Result<i32> {
    if !(1..=16).contains(&bits) || *state > u32::from(u16::MAX) {
        return Err(malformed("film-grain random state is invalid"));
    }
    let bit = (*state ^ (*state >> 1) ^ (*state >> 3) ^ (*state >> 12)) & 1;
    *state = (*state >> 1) | (bit << 15);
    let shift = u32::from(16_u8.saturating_sub(bits));
    let mask = (1_u32 << u32::from(bits)).saturating_sub(1);
    i32::try_from((*state >> shift) & mask)
        .map_err(|_| malformed("film-grain random value exceeds i32"))
}

fn round_signed(value: i32, shift: u8) -> Av1Result<i32> {
    i32::try_from(round_signed_i64(i64::from(value), shift)?)
        .map_err(|_| malformed("film-grain signed rounded value exceeds i32"))
}

fn round_signed_i64(value: i64, shift: u8) -> Av1Result<i64> {
    if shift == 0 {
        return Ok(value);
    }
    let bias = 1_i64
        .checked_shl(u32::from(shift.saturating_sub(1)))
        .ok_or_else(|| malformed("film-grain rounding shift overflows"))?;
    value
        .checked_add(bias)
        .map(|value| value >> shift)
        .ok_or_else(|| malformed("film-grain rounded value overflows"))
}
