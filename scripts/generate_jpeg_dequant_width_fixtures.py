#!/usr/bin/env python3
"""Generate complete JPEG inputs for signed-word quantization and IDCT parity."""
from __future__ import annotations

import argparse
import hashlib
from io import BytesIO
from pathlib import Path

from PIL import Image, __version__ as pillow_version
from generate_jpeg_jcoef_boundary_fixtures import amplitude, entropy, segment

ROOT = Path(__file__).resolve().parents[1]
# Complete input parameters, input SHA-256, and independently observed Pillow
# pixel SHA-256. Stimulus construction never consumes expected pixel data.
SPARSE = {
    "progressive_dequant_vertical_8x8_cat1_q1_positive": (
        (8, 8, 1, 1, False, True, 2),
        "fce6fa4937755a99f7e3c2c26a3907acb4f22f9a1511c0464e414847c4964389",
        "1df1b7ce1fd8fcbe20cde61646875e54fe38d8945ea7911afd59e025cc520a68",
    ),
    "progressive_dequant_vertical_8x8_cat1_q1_negative": (
        (8, 8, 1, 1, True, True, 2),
        "601c12b004a94092bf963daab5c187dea3888d41a2dbcacce6818ad2171a82b7",
        "1df1b7ce1fd8fcbe20cde61646875e54fe38d8945ea7911afd59e025cc520a68",
    ),
    "progressive_dequant_vertical_8x8_cat1_q255_positive": (
        (8, 8, 1, 255, False, True, 2),
        "cf2fdd26ca64f534338e6ffaae357d164cbf8fc7d3f867e427960b64488c0e99",
        "df21e7d19e5ec65f2dfaadb042131e4d7a7cbaae6e6057a91b0d09383ffafb54",
    ),
    "progressive_dequant_vertical_8x8_cat1_q255_negative": (
        (8, 8, 1, 255, True, True, 2),
        "afddb05125aef5b7ef15436367283eab94ded6ae7181d7908847ca398b5a32d0",
        "65ed00bb47b1822d9d6a958459c2d5c9f4ad831980fbbb827d5fd30ee7f0bf35",
    ),
    "progressive_dequant_vertical_8x8_cat1_q32768_positive": (
        (8, 8, 1, 32768, False, True, 2),
        "b63f23a3bae5331877832c58d7fd2da3aeae8abcb8218d3b20edb09b9754a0af",
        "bdce5d57d7bbd650f9ea272d9d13823421b7ad624a9013218209461c60940010",
    ),
    "progressive_dequant_vertical_8x8_cat1_q32768_negative": (
        (8, 8, 1, 32768, True, True, 2),
        "922e7b288eb20e01357a6405c376a96c5820b9e4962afe67d892bd710d12ff90",
        "bdce5d57d7bbd650f9ea272d9d13823421b7ad624a9013218209461c60940010",
    ),
    "progressive_dequant_vertical_8x8_cat1_q65535_positive": (
        (8, 8, 1, 65535, False, True, 2),
        "f04f97c7dea16b0384b3c4e57218a4c0c9029d62530134686b3909cc739a3cb4",
        "1df1b7ce1fd8fcbe20cde61646875e54fe38d8945ea7911afd59e025cc520a68",
    ),
    "progressive_dequant_vertical_8x8_cat1_q65535_negative": (
        (8, 8, 1, 65535, True, True, 2),
        "7c1206afc0fbab8bc6fdf7c90e58bca3c3f3e833991e18251efbba0d6b9a2126",
        "1df1b7ce1fd8fcbe20cde61646875e54fe38d8945ea7911afd59e025cc520a68",
    ),
    "progressive_dequant_vertical_8x8_cat10_q1_positive": (
        (8, 8, 10, 1, False, True, 2),
        "4120a5d00c5aa52211864672cd4c5d08b97546702bb92f41424dce7a6ea0af18",
        "3a8d5f69d992d1bb0931a348629f8e88581bf8b8ab9895fe9e016859d5ca2b4e",
    ),
    "progressive_dequant_vertical_8x8_cat10_q1_negative": (
        (8, 8, 10, 1, True, True, 2),
        "79ef27eea8369a9f74078214c260709361bb01846d4e918ae8c2e8eb684e01e2",
        "1448acd1bf019d2b3a0e8118e534424fd86037b96c9c575f560f8673864fcf83",
    ),
    "progressive_dequant_vertical_8x8_cat10_q255_positive": (
        (8, 8, 10, 255, False, True, 2),
        "f2721bea7e25e65421fefb3c77cb0dc9e60a3bf44ec3b2ce83e2ddcd767877d3",
        "fe7348be9a54a6f51cb190e6f845ae3a1c33dc12bc1e060a6e623658e312869f",
    ),
    "progressive_dequant_vertical_8x8_cat10_q255_negative": (
        (8, 8, 10, 255, True, True, 2),
        "3765775e8e5e5b58c08ea7f022bee2b2136e8c6c07b5a608607c517f139ce3db",
        "2520cd3a52207b2a48898d126d6f71270ed63c27b102961350dd3d77ab323c9a",
    ),
    "progressive_dequant_vertical_8x8_cat10_q32768_positive": (
        (8, 8, 10, 32768, False, True, 2),
        "5522c215908898edfc2d760ecf2e8a1aa80cae09075bdc66a8b5e52168773228",
        "bdce5d57d7bbd650f9ea272d9d13823421b7ad624a9013218209461c60940010",
    ),
    "progressive_dequant_vertical_8x8_cat10_q32768_negative": (
        (8, 8, 10, 32768, True, True, 2),
        "8352bf49b143a3ce451fc1880bbf25660b26998683f49dd9bff2f7950aa8c1c1",
        "bdce5d57d7bbd650f9ea272d9d13823421b7ad624a9013218209461c60940010",
    ),
    "progressive_dequant_vertical_8x8_cat10_q65535_positive": (
        (8, 8, 10, 65535, False, True, 2),
        "d2084bf168fffbe0b04f4760c8169dc21a275581712eb8126bc64f075c8a462a",
        "1448acd1bf019d2b3a0e8118e534424fd86037b96c9c575f560f8673864fcf83",
    ),
    "progressive_dequant_vertical_8x8_cat10_q65535_negative": (
        (8, 8, 10, 65535, True, True, 2),
        "f09051baad081410cafbc6ac544d7c7550171f5294d5af0e18147a67a701af25",
        "3a8d5f69d992d1bb0931a348629f8e88581bf8b8ab9895fe9e016859d5ca2b4e",
    ),
    "progressive_dequant_vertical_17x9_cat1_q1_positive": (
        (17, 9, 1, 1, False, True, 2),
        "3c087347e01cb9a0fac5adfc50dd46e52c8652f3676cd583b37d10cb41714c29",
        "9a094f344e8b2b8835c13b540629613db1eb0d428fc6f86f0fdd7fc146075f01",
    ),
    "progressive_dequant_vertical_17x9_cat1_q1_negative": (
        (17, 9, 1, 1, True, True, 2),
        "f0228275efb1a72dd35199ba4c0e1a7b47f4d2cbc84adb20ca9eb140f166dab5",
        "70bb2c1ba78305924191cfe451c0ae7f1095a42f68466f1192aa43099ea08576",
    ),
    "progressive_dequant_vertical_17x9_cat1_q255_positive": (
        (17, 9, 1, 255, False, True, 2),
        "3773d241d4050075a8ab5e7377b19278159b41cec2bf8455ae4ed36969344141",
        "c797b0a0a4bb0bda3a1ac6ec898baa06fffb769480d0f976162fd6755e455e7c",
    ),
    "progressive_dequant_vertical_17x9_cat1_q255_negative": (
        (17, 9, 1, 255, True, True, 2),
        "eeefc908078e41ea9f4ead214d6aa0fdd6bc4cbd88daa3853022053c0e663742",
        "ad112cbd0b67b63c803202f1f900a205459d2e39493a1ae28dc7fffb59862d78",
    ),
    "progressive_dequant_vertical_17x9_cat1_q32768_positive": (
        (17, 9, 1, 32768, False, True, 2),
        "8f9366172a4cdf8347dc22d6019919a86bdea98c5c12b26c14058d6083bf2590",
        "05ad1c1b77b1f460d5f47d534072486f7cefad381a40c13e70b01a0b6e488886",
    ),
    "progressive_dequant_vertical_17x9_cat1_q32768_negative": (
        (17, 9, 1, 32768, True, True, 2),
        "36309df0e20d83f91606845e514c740a4dd042f4180e0bfa6c3120ef8b3f5399",
        "05ad1c1b77b1f460d5f47d534072486f7cefad381a40c13e70b01a0b6e488886",
    ),
    "progressive_dequant_vertical_17x9_cat1_q65535_positive": (
        (17, 9, 1, 65535, False, True, 2),
        "8ed8e9c03f2da20d052cc14ad0f0f9f5f3506f5c38161a65a03d832a580fa75e",
        "70bb2c1ba78305924191cfe451c0ae7f1095a42f68466f1192aa43099ea08576",
    ),
    "progressive_dequant_vertical_17x9_cat1_q65535_negative": (
        (17, 9, 1, 65535, True, True, 2),
        "a9a4c43ae8b2dfef878b1a87dd6402d1f65cc371b7bc057792d01efb53853c2c",
        "9a094f344e8b2b8835c13b540629613db1eb0d428fc6f86f0fdd7fc146075f01",
    ),
    "progressive_dequant_vertical_17x9_cat3_q32767_positive": (
        (17, 9, 3, 32767, False, True, 2),
        "e9969d13f472c608370524b8ede44e57eb677ad5eddb637e4783f498674b1dd8",
        "fe0f453bc212775e72aa090ecea50edf6d7e9da63ad063de6e5ce9c11a7313c3",
    ),
    "progressive_dequant_vertical_17x9_cat3_q32767_negative": (
        (17, 9, 3, 32767, True, True, 2),
        "75f7f207774da70cfaa32f1e8089a06c5264da21aa81ea728df9f509972ac541",
        "05ad1c1b77b1f460d5f47d534072486f7cefad381a40c13e70b01a0b6e488886",
    ),
    "progressive_dequant_vertical_17x9_cat10_q1_positive": (
        (17, 9, 10, 1, False, True, 2),
        "3069130ed324cce790d33c176ef556ab25d6501ca8916d5a2fadc44375443e07",
        "94ae1926a0d2cd0a09342f8bf76e425485b3a3ad46faee533566f2f43ab4da58",
    ),
    "progressive_dequant_vertical_17x9_cat10_q1_negative": (
        (17, 9, 10, 1, True, True, 2),
        "68f11bc9a2001f2357e89ad411a16ca2442b85ff7feda3da84b30801d4ffbd54",
        "167fa8c6c03a2dd97d576b82152de5eeb0fe29adc3502074189f3315924b972a",
    ),
    "progressive_dequant_vertical_17x9_cat10_q255_positive": (
        (17, 9, 10, 255, False, True, 2),
        "0f5dc6cf45b1ca1a84f3d7f3a42b86360bd9a6ec886033aa149804e0bb82c7f1",
        "54bb1cfe96c6621f5b305d7058444bca211b78476fe81365cceb5c0ba806758e",
    ),
    "progressive_dequant_vertical_17x9_cat10_q255_negative": (
        (17, 9, 10, 255, True, True, 2),
        "5810f07d62145c946470ce8056f3c423c6b8f03bd035aee8a39e48b9ac9d4ab9",
        "f876b4088160434880285011fa1880ba3d510ca634bdd2696637bf4de0786754",
    ),
    "progressive_dequant_vertical_17x9_cat10_q32768_positive": (
        (17, 9, 10, 32768, False, True, 2),
        "8cc70a95bc785b5ac5370abadfd6a5d7aba5e4ef29df6289ddd43fbce8675604",
        "05ad1c1b77b1f460d5f47d534072486f7cefad381a40c13e70b01a0b6e488886",
    ),
    "progressive_dequant_vertical_17x9_cat10_q32768_negative": (
        (17, 9, 10, 32768, True, True, 2),
        "a3e4bac1225cfa2e336b988807633f45bf58ce403f07927e2df922ed55d44de7",
        "05ad1c1b77b1f460d5f47d534072486f7cefad381a40c13e70b01a0b6e488886",
    ),
    "progressive_dequant_vertical_17x9_cat10_q65535_positive": (
        (17, 9, 10, 65535, False, True, 2),
        "cc7d4e086d413731d100599f0dd48669d189e11e36c2dbeeb96490919c35bc5f",
        "167fa8c6c03a2dd97d576b82152de5eeb0fe29adc3502074189f3315924b972a",
    ),
    "progressive_dequant_vertical_17x9_cat10_q65535_negative": (
        (17, 9, 10, 65535, True, True, 2),
        "f62fefa8c306d6fd486010064c746c076b1667a05becb916d28f3cb329dc5589",
        "94ae1926a0d2cd0a09342f8bf76e425485b3a3ad46faee533566f2f43ab4da58",
    ),
    "sequential_dequant_vertical_17x9_cat15_q1_positive": (
        (17, 9, 15, 1, False, False, 2),
        "4bc83d581bd8b168bc648cc39eeef954e84794437eb9ec10fc624825e7db1aba",
        "fe0f453bc212775e72aa090ecea50edf6d7e9da63ad063de6e5ce9c11a7313c3",
    ),
    "sequential_dequant_vertical_17x9_cat15_q1_negative": (
        (17, 9, 15, 1, True, False, 2),
        "9ec7592e87862fbd8f556d13c5357112f363c8cc7b8fb4ffc39e6c45bfb17dee",
        "05ad1c1b77b1f460d5f47d534072486f7cefad381a40c13e70b01a0b6e488886",
    ),
    "sequential_dequant_vertical_17x9_cat15_q65535_positive": (
        (17, 9, 15, 65535, False, False, 2),
        "9de7efbda184cb79d5165bd9db29b09b2c97a162b05627c82991973bfd4172e2",
        "05ad1c1b77b1f460d5f47d534072486f7cefad381a40c13e70b01a0b6e488886",
    ),
    "sequential_dequant_vertical_17x9_cat15_q65535_negative": (
        (17, 9, 15, 65535, True, False, 2),
        "318c34f01ce1103e34f33e658e91921cda53734b39f3e83442555129d54a5b09",
        "fe0f453bc212775e72aa090ecea50edf6d7e9da63ad063de6e5ce9c11a7313c3",
    ),
    "sequential_dequant_17x9_cat15_q1_positive": (
        (17, 9, 15, 1, False, False, 1),
        "ab73f7df87212f3ea4140cd316fa931aa167d5e75af77817af6749aad0792eb6",
        "56de78f344a294a6f6e38ca132e29b9924a077ff0930de2c1c9da964136ced27",
    ),
    "sequential_dequant_17x9_cat15_q1_negative": (
        (17, 9, 15, 1, True, False, 1),
        "077bcb261d96697b939a45c61d7e7ab06f22ecd9bf3d93aa4eed8bcd4a15e9f9",
        "58c65f92ad73f9fe4e2f0d588c0ab9fe33f1a3ed4d971c80f5c22303f1f7dc1f",
    ),
    "sequential_dequant_17x9_cat15_q65535_positive": (
        (17, 9, 15, 65535, False, False, 1),
        "05209d0b36e9ee36be23c86afcaf7036487dcfd0639655ec9e85cd8856840880",
        "58c65f92ad73f9fe4e2f0d588c0ab9fe33f1a3ed4d971c80f5c22303f1f7dc1f",
    ),
    "sequential_dequant_17x9_cat15_q65535_negative": (
        (17, 9, 15, 65535, True, False, 1),
        "a6db3d1e3cb0f61040d77a3acea76c87983bb53d9bf891c5bb871880f47cf767",
        "56de78f344a294a6f6e38ca132e29b9924a077ff0930de2c1c9da964136ced27",
    ),
    "progressive_dequant_vertical_right_8x8_cat1_q1_positive": (
        (8, 8, 1, 1, False, True, 16),
        "865f1c48816b57baa99b72637fbb697652e8f241c4c7113c894422cddcb45f83",
        "1df1b7ce1fd8fcbe20cde61646875e54fe38d8945ea7911afd59e025cc520a68",
    ),
    "progressive_dequant_vertical_right_8x8_cat1_q1_negative": (
        (8, 8, 1, 1, True, True, 16),
        "dc8c93f3313176a0a990fe4f6eede0ea8503f37742a6ab985360fe60ea970f31",
        "1df1b7ce1fd8fcbe20cde61646875e54fe38d8945ea7911afd59e025cc520a68",
    ),
    "progressive_dequant_vertical_right_8x8_cat1_q255_positive": (
        (8, 8, 1, 255, False, True, 16),
        "3c6596cb809c8bcdfaff97b5b898218792fbae4bdcd8057fd741590b05548fb2",
        "46e31e4e558d3a7c5d665806cda5baad277d78ed70d0519ee7fffed3b639fb44",
    ),
    "progressive_dequant_vertical_right_8x8_cat1_q255_negative": (
        (8, 8, 1, 255, True, True, 16),
        "3caab26b09a1c502f16cd708f7ab6077e28ef2ddc3a1f6fe171300da8ade9c0b",
        "2ffea9109bbd68f73b15014927fc13f720fa81c1a4fbef0571c3246e8dc5f2fb",
    ),
    "progressive_dequant_vertical_right_8x8_cat1_q32768_positive": (
        (8, 8, 1, 32768, False, True, 16),
        "ac3c0583d8ccb00c21c61a15c6a41908e9375e8670df4e9299b80f5423345018",
        "2ce7641bc07acc55780ba9e2852b69412c8af53aeae08f179f1567022f47dc9b",
    ),
    "progressive_dequant_vertical_right_8x8_cat1_q32768_negative": (
        (8, 8, 1, 32768, True, True, 16),
        "cf330e35fae89588b425de2a92f0921b768bd89b72d3b6789e6d3ce622823e9d",
        "2ce7641bc07acc55780ba9e2852b69412c8af53aeae08f179f1567022f47dc9b",
    ),
    "progressive_dequant_vertical_right_8x8_cat1_q65535_positive": (
        (8, 8, 1, 65535, False, True, 16),
        "dd6e08aa2dc5b0210057d2a153d7e914ea8d35185b8dedbdbc225f0d87e2f190",
        "1df1b7ce1fd8fcbe20cde61646875e54fe38d8945ea7911afd59e025cc520a68",
    ),
    "progressive_dequant_vertical_right_8x8_cat1_q65535_negative": (
        (8, 8, 1, 65535, True, True, 16),
        "ce17e2bf99342ab480a804ed5d60c8d2aec5b6c234e8f6bc701a1a71ee20f4bc",
        "1df1b7ce1fd8fcbe20cde61646875e54fe38d8945ea7911afd59e025cc520a68",
    ),
}

DENSE = {
    "sequential_dequant_dense_positive_q1": (
        ('positive', 1, False),
        "6ee4da0e19d5fc997a37205deda095ec04edc0b18dbf22a60d639f2164b397fb",
        "c28f840424f6ba738754fdbeb7149455cf15176ff843e9107628e8be71a8fb98",
    ),
    "progressive_dequant_dense_positive_q1": (
        ('positive', 1, True),
        "050bb0168943dddc632d195bb2d976be2c9c98b4f7c45c5443bb0d0ada51edac",
        "c28f840424f6ba738754fdbeb7149455cf15176ff843e9107628e8be71a8fb98",
    ),
    "sequential_dequant_dense_positive_q65535": (
        ('positive', 65535, False),
        "57e44980151fcb151161b390607b121149ed86096eac541ec9a2e5a3476a6dda",
        "97fcbfa1409584b1e586510f675d9a0377ee67efc92cd87ee5a3d0db4d5ffd8d",
    ),
    "progressive_dequant_dense_positive_q65535": (
        ('positive', 65535, True),
        "bad325cf9908d12f4cb3738e57c055e7650b883079076314d78313522ff417d0",
        "97fcbfa1409584b1e586510f675d9a0377ee67efc92cd87ee5a3d0db4d5ffd8d",
    ),
    "sequential_dequant_dense_alternating_q1": (
        ('alternating', 1, False),
        "f2aae95a47a2eac745fba1c6fd3cead5efdd67c58416db47e4a87e18f96fc53d",
        "e2aabde2f3cf629d00d187849faab0357cbb920afdbd2874d1378c08d30bd862",
    ),
    "progressive_dequant_dense_alternating_q1": (
        ('alternating', 1, True),
        "34cdbee1ee96c1a27ea542552e61bc8ce06b4e5d7324e638fb53c3dec7a5ff04",
        "e2aabde2f3cf629d00d187849faab0357cbb920afdbd2874d1378c08d30bd862",
    ),
    "sequential_dequant_dense_alternating_q65535": (
        ('alternating', 65535, False),
        "b60c419a7ffbe45c751f689e6ec31e993208c8d514e239f7e44b5225a64f251d",
        "7978e1d2fe22d6c3c04f39c2e29921e50cfcc1f49a9db014cf39926b16d32766",
    ),
    "progressive_dequant_dense_alternating_q65535": (
        ('alternating', 65535, True),
        "c64ea36e0a100c7fc6d7ab3131f4759dfeae65bed6de631353612d8730236f35",
        "7978e1d2fe22d6c3c04f39c2e29921e50cfcc1f49a9db014cf39926b16d32766",
    ),
}


def sparse_fixture(
    width: int, height: int, category: int, quantizer: int,
    negative: bool, progressive: bool, position: int,
) -> bytes:
    """Build DC increments and one AC coefficient at the declared zigzag index."""
    quantization = segment(0xdb, b"\x10" + quantizer.to_bytes(2, "big") * 64)
    frame = segment(
        0xc2 if progressive else 0xc0,
        bytes([8]) + height.to_bytes(2, "big") + width.to_bytes(2, "big")
        + bytes([1, 1, 0x11, 0]),
    )
    dc_table = bytes([0, 1]) + bytes(15) + bytes([category])
    ac_symbol = ((position - 1) << 4) | category
    ac_table = bytes([0x10, 0, 2]) + bytes(14) + bytes([ac_symbol, 0])
    tables = segment(0xc4, dc_table + ac_table)
    blocks = ((width + 7) // 8) * ((height + 7) // 8)
    value = amplitude(category, negative)
    dc = "0" + value
    ac = "00" + value + "01"
    if progressive:
        scans = segment(0xda, bytes([1, 1, 0, 0, 0, 0])) + entropy(dc * blocks)
        scans += segment(0xda, bytes([1, 1, 0, 1, 63, 0])) + entropy(ac * blocks)
    else:
        scans = segment(0xda, bytes([1, 1, 0, 0, 63, 0])) + entropy((dc + ac) * blocks)
    return b"\xff\xd8" + quantization + frame + tables + scans + b"\xff\xd9"


def dense_fixture(pattern: str, quantizer: int, progressive: bool) -> bytes:
    """Build one complete block with every coefficient nonzero."""
    if pattern not in {"positive", "alternating"}:
        raise ValueError(f"unknown dense coefficient pattern: {pattern}")
    quantization = segment(0xdb, b"\x10" + quantizer.to_bytes(2, "big") * 64)
    frame = segment(0xc2 if progressive else 0xc0, bytes([8, 0, 8, 0, 8, 1, 1, 0x11, 0]))
    counts = bytes([0, 0, 0, 0, 16]) + bytes(11)
    tables = segment(
        0xc4, b"\x00" + counts + bytes(range(16))
        + b"\x10" + counts + bytes(range(16)),
    )
    # Category 15 uses canonical code 01111 in these five-bit tables.
    values = ["01111" + amplitude(15, pattern == "alternating" and index % 2 == 1)
              for index in range(64)]
    dc, ac = values[0], "".join(values[1:])
    if progressive:
        scans = segment(0xda, bytes([1, 1, 0, 0, 0, 0])) + entropy(dc)
        scans += segment(0xda, bytes([1, 1, 0, 1, 63, 0])) + entropy(ac)
    else:
        scans = segment(0xda, bytes([1, 1, 0, 0, 63, 0])) + entropy(dc + ac)
    return b"\xff\xd8" + quantization + frame + tables + scans + b"\xff\xd9"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify without writing")
    args = parser.parse_args()
    if pillow_version != "12.2.0":
        raise RuntimeError(f"expected Pillow 12.2.0, found {pillow_version}")
    for cases, factory in [(SPARSE, sparse_fixture), (DENSE, dense_fixture)]:
        for name, (parameters, input_hash, pixel_hash) in cases.items():
            encoded = factory(*parameters)
            if hashlib.sha256(encoded).hexdigest() != input_hash:
                raise RuntimeError(f"JPEG input bytes changed: {name}")
            with Image.open(BytesIO(encoded)) as image:
                image.verify()
            with Image.open(BytesIO(encoded)) as image:
                image.load()
                size = parameters[:2] if factory is sparse_fixture else (8, 8)
                if image.mode != "L" or image.size != size:
                    raise RuntimeError(f"Pillow metadata changed: {name}")
                if hashlib.sha256(image.tobytes()).hexdigest() != pixel_hash:
                    raise RuntimeError(f"Pillow pixels changed: {name}")
            path = ROOT / "tests/fixtures/input/images/jpeg" / (name + ".jpg")
            if path.exists() and path.read_bytes() != encoded:
                raise RuntimeError(f"refusing to replace different fixture bytes: {name}")
            if args.check:
                if not path.is_file():
                    raise RuntimeError(f"fixture is missing: {name}")
            else:
                path.write_bytes(encoded)
            print(f"{'Checked' if args.check else 'Generated'} {name}")


if __name__ == "__main__":
    main()
