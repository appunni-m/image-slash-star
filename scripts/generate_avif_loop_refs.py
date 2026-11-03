#!/usr/bin/env python3
"""Collect pinned native AVIF repetition observations from complete files.

Only a fresh ignored staging directory is written. Mutations preserve file
length, every encoded sample, and primary-image data. Rust is never executed.
"""

from __future__ import annotations

import argparse
import ctypes
import io
import shutil
import struct
import sys
import tempfile
from pathlib import Path

from PIL import Image, _avif, _imaging, features

from generate_av1_sequence_refs import (
    LIBAVIF_COMMIT, artifact_records, digest_file, git_tree_digest,
    resolve_tool, run, sha256, text_output, write_json,
)
from avif_sample_table_fixtures import (
    alpha_track_duration_mismatch,
    alpha_track_timescale_mismatch,
    stts_timing_variants,
    zero_stts_sample_delta,
)
from inspect_avif_bitstreams import children, parse_boxes, unique_box

ROOT = Path(__file__).resolve().parent.parent
FIXTURES = {
    'animated_buffer_removal_time_zero': ('animated_buffer_removal_time_zero.avif', '3079d08c334b737716c5571317e711130da028fc0d8e6e4a3e9ab85b1d9f7de6'),
    'animated_buffer_removal_time_one': ('animated_buffer_removal_time_one.avif', '950f342512092d6cbc051e649acd23bbb9d31d1bbccb43fc5ae87ecb096a5347'),
    'animated_buffer_removal_time_without_point_parameters': ('animated_buffer_removal_time_without_point_parameters.avif', '0906822ac392ecb8be54e5306d68690490c1f10db3266551ec45efa74504dbd4'),
    'animated_buffer_removal_time_mixed_point_parameters': ('animated_buffer_removal_time_mixed_point_parameters.avif', 'd5348e5c71841e293fbd7aaeea49a222f1d6b26eaa957457596461ec86f293f9'),
    'animated_buffer_removal_time_ignored_temporal_point': ('animated_buffer_removal_time_ignored_temporal_point.avif', 'adaf92da2b6d3a7336b66020f1ff844f1868e774479170d950fb1bb17cfa8fcb'),
    'animated_buffer_removal_time_ignored_spatial_point': ('animated_buffer_removal_time_ignored_spatial_point.avif', '53aed64c97e99732dc538a951234dc42b73136974bdad09cabfebafad107d8a0'),
    'animated_buffer_removal_time_ignored_temporal_spatial_point': ('animated_buffer_removal_time_ignored_temporal_spatial_point.avif', 'a46d624562637894c530d4320012aa233a7082da9d04ec1196e478bbdb4dc044'),
    'animated_buffer_removal_time_two_admitted_points': ('animated_buffer_removal_time_two_admitted_points.avif', '42a160e63388a938c4534a9463a26b28b7381ceff076675207e21282e342b6c4'),
    'animated_buffer_removal_time_selected_explicit_layer': ('animated_buffer_removal_time_selected_explicit_layer.avif', 'ab87e6dfdd7d5cc36eccc6a9d579fdbe3cdfc6c32bc69c87617921ca00070b34'),
    'animated_buffer_removal_time_two_bit': ('animated_buffer_removal_time_two_bit.avif', 'c55d5dd6433ae1fbdb4117f519db7ed0a9d17a8fe394d857c91fb0b061d236a7'),
    'animated_buffer_removal_time_selected_implicit_layer': ('animated_buffer_removal_time_selected_implicit_layer.avif', 'b5e3afc51c87eea23087acea01163c0e257d06b94cc3ad0bda64ca87bd1e9fc8'),
    'animated_buffer_removal_time_selected_temporal_one': ('animated_buffer_removal_time_selected_temporal_one.avif', '0c5041bcbf6f64652e1d500acaca510ff5f1138f04aca648e13bb3e5426273cd'),
    'animated_buffer_removal_time_width32_zero': ('animated_buffer_removal_time_width32_zero.avif', '2433f9fc10478701343ec1b0963200049f1811ece986472a845076914d2a6c10'),
    'animated_buffer_removal_time_width32_maximum': ('animated_buffer_removal_time_width32_maximum.avif', 'a51a3a5e1c35a87fcfdd6d063fdee046e5e4c77ba74f1679ea81320ba5bdee9d'),
    'animated_buffer_removal_time_points32_width1_zero': ('animated_buffer_removal_time_points32_width1_zero.avif', 'fa4c61d6c8e9df517564fd18d09898d911718b1d2c6ca28f75a8e06375c97398'),
    'animated_buffer_removal_time_points32_width1_one': ('animated_buffer_removal_time_points32_width1_one.avif', 'be30f0784f255a61430edf2cf1e3934054aefe1a030c94a0c065db2a5d254e7b'),
    'animated_buffer_removal_time_points32_width32_maximum': ('animated_buffer_removal_time_points32_width32_maximum.avif', 'aaf8d8bf72c07f2c67430f100f68f014dfe65967ad018fce57168319a01ce38b'),
    'animated_buffer_removal_time_points32_mixed_width32_maximum': ('animated_buffer_removal_time_points32_mixed_width32_maximum.avif', '0394f364d1b98f6f36cd9c25a997634c2efbad7d1363eebdf81508e8880f9965'),
    'animated_buffer_removal_time_points32_ignored_width32_maximum': ('animated_buffer_removal_time_points32_ignored_width32_maximum.avif', '3f300b24fc723f708cffa38a992c896dc631628387e1100e6928f195c2a3ce03'),
    'animated_buffer_removal_time_all_layers_width32_maximum': ('animated_buffer_removal_time_all_layers_width32_maximum.avif', '0cf2df00e877b1185927730f8a0228a0d1f033dded79fdc14f70aa2214f6a605'),
    "animated_sequence_without_extra_zero_padding": (
        "animated_sequence_without_extra_zero_padding.avif",
        "94eff1c2a3171c570e8eb9000436ba4f1d2d62cf271b00a602054a689ed25819",
    ),
    "animated_ignored_temporal_layer": ("animated_ignored_temporal_layer.avif", "1ba3cb1481a80a6e776b5c42199defca0c51f82a6c92ccbebe044a7ff2ad3276"),
    "animated_ignored_spatial_layer": ("animated_ignored_spatial_layer.avif", "93410e0c7dd6cc3048c3a741ad95ffcf87617bdd9fe9725df181bd1df34cb39e"),
    "animated_mixed_lossless_key_superres_420_8bit_integer_mv_s3_128x48_d10": ("animated_mixed_lossless_key_superres_420_8bit_integer_mv_s3_128x48_d10.avif", "ad52321fb01335ed4ea68990a12d0680b0c9a124fc2647302685b820b686595f"),
    "animated_mixed_lossless_key_superres_420_10bit_integer_mv_s3_128x48_d10": ("animated_mixed_lossless_key_superres_420_10bit_integer_mv_s3_128x48_d10.avif", "5695f864c484707f0cf3fa13fdb52641e20f168745007906b7174cf151a13c92"),
    "animated_mixed_lossless_key_superres_420_12bit_integer_mv_s3_128x48_d10": ("animated_mixed_lossless_key_superres_420_12bit_integer_mv_s3_128x48_d10.avif", "549243ce45a00247538054f47626f2c4c4e08fa99e0070c27f7da2480d57a106"),
    "animated_mixed_lossless_key_superres_422_8bit_integer_mv_s3_128x48_d10": ("animated_mixed_lossless_key_superres_422_8bit_integer_mv_s3_128x48_d10.avif", "0e17d24734181180c5c45f3de7c480d13b28f3635d14448ee8ea6452eb2646a0"),
    "animated_mixed_lossless_key_superres_422_10bit_integer_mv_s3_128x48_d10": ("animated_mixed_lossless_key_superres_422_10bit_integer_mv_s3_128x48_d10.avif", "de12c3192b5353b2aa5d6697795cbec3b0c26fec5baacb485acc51ecfc9d39a1"),
    "animated_mixed_lossless_key_superres_422_12bit_integer_mv_s3_128x48_d10": ("animated_mixed_lossless_key_superres_422_12bit_integer_mv_s3_128x48_d10.avif", "14b220fc4a288328e7965ca2b9d365f5ebbe58e53da1aee33b31b546493552fb"),
    "animated_mixed_lossless_key_superres_444_8bit_integer_mv_s3_128x48_d10": ("animated_mixed_lossless_key_superres_444_8bit_integer_mv_s3_128x48_d10.avif", "21d46ba6643955f8d8a9e9afd709cf831d7f019c2438a829f35453c9b9523618"),
    "animated_mixed_lossless_key_superres_444_10bit_integer_mv_s3_128x48_d10": ("animated_mixed_lossless_key_superres_444_10bit_integer_mv_s3_128x48_d10.avif", "cd2250eb22fea4c8cf6e02314b4aa105dbcedb7f1666ff190f0b29d6f6df9862"),
    "animated_mixed_lossless_key_superres_444_12bit_integer_mv_s3_128x48_d10": ("animated_mixed_lossless_key_superres_444_12bit_integer_mv_s3_128x48_d10.avif", "0a2688ed113caff925559dd2304217757e69931fae0f239715f3a68e231abac7"),
    "animated_mixed_lossless_key_superres_420_8bit_translated_s3_128x48_d10": ("animated_mixed_lossless_key_superres_420_8bit_translated_s3_128x48_d10.avif", "512c6abd6039fe3039ddc6547b1da8ccfd96e628b6b47a32695e46140a5fdfc8"),
    "animated_mixed_lossless_key_superres_420_10bit_translated_s3_128x48_d10": ("animated_mixed_lossless_key_superres_420_10bit_translated_s3_128x48_d10.avif", "a002af881b6df0e20b0ad3747f75e70d620ed05f2a2d8671fd625f04fe2d22f2"),
    "animated_mixed_lossless_key_superres_420_12bit_translated_s3_128x48_d10": ("animated_mixed_lossless_key_superres_420_12bit_translated_s3_128x48_d10.avif", "06e12a6dd89b61fe6aed9540ce7cd36d459db89c76ddee33e3d7401b16b9175e"),
    "animated_mixed_lossless_key_superres_422_8bit_translated_s3_128x48_d10": ("animated_mixed_lossless_key_superres_422_8bit_translated_s3_128x48_d10.avif", "18734ef99935eb482aec5b0de34a89a6eae29d7b83f27167416d5945d1c9588b"),
    "animated_mixed_lossless_key_superres_422_10bit_translated_s3_128x48_d10": ("animated_mixed_lossless_key_superres_422_10bit_translated_s3_128x48_d10.avif", "045a9f481b1aa5f4655f2a2e8380fa706e0a8d05654487e13381c96e36e96f9f"),
    "animated_mixed_lossless_key_superres_422_12bit_translated_s3_128x48_d10": ("animated_mixed_lossless_key_superres_422_12bit_translated_s3_128x48_d10.avif", "c3ee332a015f35d588a2ac60bbe9b814030d0fd6e5d0dbad3db3f0a961194185"),
    "animated_mixed_lossless_key_superres_444_8bit_translated_s3_128x48_d10": ("animated_mixed_lossless_key_superres_444_8bit_translated_s3_128x48_d10.avif", "5d92832b1dff264387701705a09c61829de6ea6eb71b7aa47510658cc90c412e"),
    "animated_mixed_lossless_key_superres_444_10bit_translated_s3_128x48_d10": ("animated_mixed_lossless_key_superres_444_10bit_translated_s3_128x48_d10.avif", "218738a70fcea72cc1aa2fe7fa6ca108184e275e542cbcbaac02790ca5da8da5"),
    "animated_mixed_lossless_key_superres_444_12bit_translated_s3_128x48_d10": ("animated_mixed_lossless_key_superres_444_12bit_translated_s3_128x48_d10.avif", "eacb7b4e3259106c3fdb924dee4e713746617ec2c1ca8575d53b6d622d798fc5"),
    "animated_mixed_lossless_key_superres_420_8bit_flat_s3_128x48_d10": ("animated_mixed_lossless_key_superres_420_8bit_flat_s3_128x48_d10.avif", "776e63b097e3c5d4b4ea7ab9603e63881258e7cc50a3c0531ee4ee287c3f7480"),
    "animated_mixed_lossless_key_superres_420_10bit_flat_s3_128x48_d10": ("animated_mixed_lossless_key_superres_420_10bit_flat_s3_128x48_d10.avif", "d7fb4428e1f4b4b245a6b2642a46192315b6936cc3bdbcabb6a122808efc675d"),
    "animated_mixed_lossless_key_superres_420_12bit_flat_s3_128x48_d10": ("animated_mixed_lossless_key_superres_420_12bit_flat_s3_128x48_d10.avif", "9bf72dd104ac5dbe494558b715256ef319db96f23cc284c32a2f7d60ce94cbb2"),
    "animated_mixed_lossless_key_superres_422_8bit_flat_s3_128x48_d10": ("animated_mixed_lossless_key_superres_422_8bit_flat_s3_128x48_d10.avif", "973a1ce88b4828fd00490b1376ccb24729b6ed49c936ba8683da3436526ece0d"),
    "animated_mixed_lossless_key_superres_422_10bit_flat_s3_128x48_d10": ("animated_mixed_lossless_key_superres_422_10bit_flat_s3_128x48_d10.avif", "1ce67e109587258abfec2d472c0f94bbb867def145e9e5b88683ddc335147e81"),
    "animated_mixed_lossless_key_superres_422_12bit_flat_s3_128x48_d10": ("animated_mixed_lossless_key_superres_422_12bit_flat_s3_128x48_d10.avif", "3053a2d11cb6a238bb1b30b47d5d322d2b6facff071c4e36953f5e55f4f9d9ea"),
    "animated_mixed_lossless_key_superres_444_8bit_flat_s3_128x48_d10": ("animated_mixed_lossless_key_superres_444_8bit_flat_s3_128x48_d10.avif", "6a7e197f61128b03322793936997832aca3d7de6e7845a02f3d8e45108c34a14"),
    "animated_mixed_lossless_key_superres_444_10bit_flat_s3_128x48_d10": ("animated_mixed_lossless_key_superres_444_10bit_flat_s3_128x48_d10.avif", "e323ce61904e0a4e3e068f025dd9c6aac934360512500ff227193d8f9db0d757"),
    "animated_mixed_lossless_key_superres_444_12bit_flat_s3_128x48_d10": ("animated_mixed_lossless_key_superres_444_12bit_flat_s3_128x48_d10.avif", "7c9ffeb97a7c2ded144ebdb36c0ac7e4284dbaf994b20a6f2287548a0cb53d61"),
    "animated_mixed_lossless_key_superres_420_8bit_flat_s3_80x20_d9": ("animated_mixed_lossless_key_superres_420_8bit_flat_s3_80x20_d9.avif", "d82211d4ba396acba17038f27a1e9fb982059371eea2a99bd581ad782dddc0da"),
    "animated_mixed_lossless_key_superres_420_10bit_flat_s3_80x20_d9": ("animated_mixed_lossless_key_superres_420_10bit_flat_s3_80x20_d9.avif", "c9ca6b87aab7e7b90e5a31b96ceed04154e13cd3096e2965e0295fefc93b4174"),
    "animated_mixed_lossless_key_superres_420_12bit_flat_s3_80x20_d9": ("animated_mixed_lossless_key_superres_420_12bit_flat_s3_80x20_d9.avif", "70c980197aeb9d58e79c18d6b96d4074b49a647c814eff70f4ed6223ee8605d3"),
    "animated_mixed_lossless_key_superres_422_8bit_flat_s3_80x20_d9": ("animated_mixed_lossless_key_superres_422_8bit_flat_s3_80x20_d9.avif", "ac62bd8b39fd109d504e1e4e5ba04afa921f810b844c18b41244567bdd34199e"),
    "animated_mixed_lossless_key_superres_422_10bit_flat_s3_80x20_d9": ("animated_mixed_lossless_key_superres_422_10bit_flat_s3_80x20_d9.avif", "f8a0e522528ec32842ad034e091f073690e379c8adf9556ed14f6f872dd60d55"),
    "animated_mixed_lossless_key_superres_422_12bit_flat_s3_80x20_d9": ("animated_mixed_lossless_key_superres_422_12bit_flat_s3_80x20_d9.avif", "c15e4bf7561cdf2d4a2c31edb0d7bdef4ec15787c6c765ec9474b5d7dc3b7fc0"),
    "animated_mixed_lossless_key_superres_444_8bit_flat_s3_80x20_d9": ("animated_mixed_lossless_key_superres_444_8bit_flat_s3_80x20_d9.avif", "14727f84ca1fc70fb9207daf59b8828417761059e0989632ba4456af6e82aaca"),
    "animated_mixed_lossless_key_superres_444_10bit_flat_s3_80x20_d9": ("animated_mixed_lossless_key_superres_444_10bit_flat_s3_80x20_d9.avif", "d504234c0b8cbbeb862d588b233ce326a98cdd0561024f626ab2c0560e428b2f"),
    "animated_mixed_lossless_key_superres_444_12bit_flat_s3_80x20_d9": ("animated_mixed_lossless_key_superres_444_12bit_flat_s3_80x20_d9.avif", "a9b1953e092dfd12392e2f8fd41d1828b17c74c5ceee1e75da0eb993e3d3fc6d"),
    "animated_mixed_lossless_key_superres_420_8bit_flat_s3_112x20_d9": ("animated_mixed_lossless_key_superres_420_8bit_flat_s3_112x20_d9.avif", "98bf91fa58409a26e598576a31836894a73f6d9b8be69fa626d7159ebbe3f420"),
    "animated_mixed_lossless_key_superres_420_10bit_flat_s3_112x20_d9": ("animated_mixed_lossless_key_superres_420_10bit_flat_s3_112x20_d9.avif", "90a7741bea0e628c6468353f3e5bc8d631093f9d1d58415d15042e4bb7ea0288"),
    "animated_mixed_lossless_key_superres_420_12bit_flat_s3_112x20_d9": ("animated_mixed_lossless_key_superres_420_12bit_flat_s3_112x20_d9.avif", "d5429fb9530cf2442870c7bae029bb9fe1e769ede185de462fc8e4da2d927e0e"),
    "animated_mixed_lossless_key_superres_422_8bit_flat_s3_112x20_d9": ("animated_mixed_lossless_key_superres_422_8bit_flat_s3_112x20_d9.avif", "3a70bf4dde2f0eb25fa2a111e23b0ea8441e00f096f3a7105cd15f693bccef59"),
    "animated_mixed_lossless_key_superres_422_10bit_flat_s3_112x20_d9": ("animated_mixed_lossless_key_superres_422_10bit_flat_s3_112x20_d9.avif", "0972a3b9feee7ed33d3f8d6874ae37a5c1757488482fa35343c1e4169f298583"),
    "animated_mixed_lossless_key_superres_422_12bit_flat_s3_112x20_d9": ("animated_mixed_lossless_key_superres_422_12bit_flat_s3_112x20_d9.avif", "4349a36a2736493ea7c5b3ebba1ff9df0802a0b404e57aab5adba6d938158b13"),
    "animated_mixed_lossless_key_superres_444_8bit_flat_s3_112x20_d9": ("animated_mixed_lossless_key_superres_444_8bit_flat_s3_112x20_d9.avif", "43e38143ca61107bd4ccaea1f76031db5aeff9851fdfc2244784ff7fddc67a2e"),
    "animated_mixed_lossless_key_superres_444_10bit_flat_s3_112x20_d9": ("animated_mixed_lossless_key_superres_444_10bit_flat_s3_112x20_d9.avif", "3127c7af4b18cd09fd12f4bbd3cc37e4c074583d7ecebd772160449e410ef8a1"),
    "animated_mixed_lossless_key_superres_444_12bit_flat_s3_112x20_d9": ("animated_mixed_lossless_key_superres_444_12bit_flat_s3_112x20_d9.avif", "1748f119d4e120ecafdcc7472c2d96ebc717128f1e734e6d0364e75e33bdeb32"),
    "animated_mixed_lossless_key_superres_420_8bit_varied_s3_128x48_d10": ("animated_mixed_lossless_key_superres_420_8bit_varied_s3_128x48_d10.avif", "ad361a025e522e8e602c9c443478ca4fe1b2d5f6e5467187f6d3ac7f9fe765de"),
    "animated_mixed_lossless_key_superres_420_10bit_varied_s3_128x48_d10": ("animated_mixed_lossless_key_superres_420_10bit_varied_s3_128x48_d10.avif", "6095347a8400721baf7520a0d01054f7a58392b32491bc016e721cce5b8638ff"),
    "animated_mixed_lossless_key_superres_420_12bit_varied_s3_128x48_d10": ("animated_mixed_lossless_key_superres_420_12bit_varied_s3_128x48_d10.avif", "2211ceb1a073ba40ddc60a24907d0fb0e7df6a3e5658daf6855655e155410653"),
    "animated_mixed_lossless_key_superres_422_8bit_varied_s3_128x48_d10": ("animated_mixed_lossless_key_superres_422_8bit_varied_s3_128x48_d10.avif", "deee56a79c4bf12ce894e8fe17e51f84473921d71906b8bde9bbcbc8e7bf3e86"),
    "animated_mixed_lossless_key_superres_422_10bit_varied_s3_128x48_d10": ("animated_mixed_lossless_key_superres_422_10bit_varied_s3_128x48_d10.avif", "00b06e8df4619721717b42c17ca24c93ec2603f31a94bc865dcebbde8836fce3"),
    "animated_mixed_lossless_key_superres_422_12bit_varied_s3_128x48_d10": ("animated_mixed_lossless_key_superres_422_12bit_varied_s3_128x48_d10.avif", "fefd4d34f98314c42f6a66496237fd57fc881770645e21555c86d455ec9cc599"),
    "animated_mixed_lossless_key_superres_444_8bit_varied_s3_128x48_d10": ("animated_mixed_lossless_key_superres_444_8bit_varied_s3_128x48_d10.avif", "456ab9d2b4267a0d7bcd610b274d165d7d2cbac79fd498925b69a9299048deff"),
    "animated_mixed_lossless_key_superres_444_10bit_varied_s3_128x48_d10": ("animated_mixed_lossless_key_superres_444_10bit_varied_s3_128x48_d10.avif", "850f709597c066d74b3f16b33a0b8e472b3eba87f3e8bcb7f5d3dbf341c08d20"),
    "animated_mixed_lossless_key_superres_444_12bit_varied_s3_128x48_d10": ("animated_mixed_lossless_key_superres_444_12bit_varied_s3_128x48_d10.avif", "c1420456268a9789a2e28b72d5be72a191bc8f7c7b76d45fc033813c6af0e7b1"),
    "animated_mixed_lossless_key_superres_420_8bit_varied_s3_80x20_d9": ("animated_mixed_lossless_key_superres_420_8bit_varied_s3_80x20_d9.avif", "bbf89bad8078c1a876e72d39bc259142b4dd2dbb146294fa08a0a87020e6ed47"),
    "animated_mixed_lossless_key_superres_420_10bit_varied_s3_80x20_d9": ("animated_mixed_lossless_key_superres_420_10bit_varied_s3_80x20_d9.avif", "0b310f31e01e2357d99398da2aaf83e0134c95ef905b71be5bf5c064081d3e3f"),
    "animated_mixed_lossless_key_superres_420_12bit_varied_s3_80x20_d9": ("animated_mixed_lossless_key_superres_420_12bit_varied_s3_80x20_d9.avif", "a840e6988c16908aaabdce5aae899888059f5f0d566b64dc3553d7e114de176b"),
    "animated_mixed_lossless_key_superres_422_8bit_varied_s3_80x20_d9": ("animated_mixed_lossless_key_superres_422_8bit_varied_s3_80x20_d9.avif", "2c7c7b8d7bbf869af284863684fbdfa4e53a5b87a01537c1bc92c7ffc8be022b"),
    "animated_mixed_lossless_key_superres_422_10bit_varied_s3_80x20_d9": ("animated_mixed_lossless_key_superres_422_10bit_varied_s3_80x20_d9.avif", "a7367c64f5b5e156cc096651b45cce9e7b58f84f92f47131030ea5b0425b39cc"),
    "animated_mixed_lossless_key_superres_422_12bit_varied_s3_80x20_d9": ("animated_mixed_lossless_key_superres_422_12bit_varied_s3_80x20_d9.avif", "20f838bc3fcd8c892db04a3a2cf36a8b706dc9ff32039768847342ec388f0848"),
    "animated_mixed_lossless_key_superres_444_8bit_varied_s3_80x20_d9": ("animated_mixed_lossless_key_superres_444_8bit_varied_s3_80x20_d9.avif", "85a572ef7210bb5ca9027ced056f404faa9cbf0c5ff1f54f72825da7fff77e8d"),
    "animated_mixed_lossless_key_superres_444_10bit_varied_s3_80x20_d9": ("animated_mixed_lossless_key_superres_444_10bit_varied_s3_80x20_d9.avif", "f6555df6058babfcbdb6c1a4033d124e3c4da68a337c13676fa7f35852326f90"),
    "animated_mixed_lossless_key_superres_444_12bit_varied_s3_80x20_d9": ("animated_mixed_lossless_key_superres_444_12bit_varied_s3_80x20_d9.avif", "01b697a48e0319971ad41e08d553f34d78a8cc6d7a497dd1c9d6f4184f235046"),
    "animated_mixed_lossless_key_superres_420_8bit_varied_s3_112x20_d9": ("animated_mixed_lossless_key_superres_420_8bit_varied_s3_112x20_d9.avif", "c8be9012fa16909e09a4035a5f73dd89bb2d599a39c4556349847eada2480682"),
    "animated_mixed_lossless_key_superres_420_10bit_varied_s3_112x20_d9": ("animated_mixed_lossless_key_superres_420_10bit_varied_s3_112x20_d9.avif", "129483fa447ab409ce7b77bc4ea05891865f9174af155ec0cb5d1272dfcd4f2f"),
    "animated_mixed_lossless_key_superres_420_12bit_varied_s3_112x20_d9": ("animated_mixed_lossless_key_superres_420_12bit_varied_s3_112x20_d9.avif", "ace67b030953cddbecb74fc6523b83f127b4809caeb65b3931fc5f81961f4262"),
    "animated_mixed_lossless_key_superres_422_8bit_varied_s3_112x20_d9": ("animated_mixed_lossless_key_superres_422_8bit_varied_s3_112x20_d9.avif", "4c6ce71de3fcfef41223f85852f029fb4b32e66dc531d4e5d575f34b3f2cbe38"),
    "animated_mixed_lossless_key_superres_422_10bit_varied_s3_112x20_d9": ("animated_mixed_lossless_key_superres_422_10bit_varied_s3_112x20_d9.avif", "1dc1b288512fd820d166e210eee91a3ba94ed094ac7257f9851ea17ec62f010a"),
    "animated_mixed_lossless_key_superres_422_12bit_varied_s3_112x20_d9": ("animated_mixed_lossless_key_superres_422_12bit_varied_s3_112x20_d9.avif", "f454b8a3207cdf8c6d5b563cfdb50c9194116a49e5103e89f72c0725159d1e36"),
    "animated_mixed_lossless_key_superres_444_8bit_varied_s3_112x20_d9": ("animated_mixed_lossless_key_superres_444_8bit_varied_s3_112x20_d9.avif", "b27f7bd930c6ef7a05996e242894a37ab3617f8503bef211bd50fab7061e3e1c"),
    "animated_mixed_lossless_key_superres_444_10bit_varied_s3_112x20_d9": ("animated_mixed_lossless_key_superres_444_10bit_varied_s3_112x20_d9.avif", "37400c10ae079236ccb23c144ebd456835ebcaade65f28fbfccb7c6e708541c3"),
    "animated_mixed_lossless_key_superres_444_12bit_varied_s3_112x20_d9": ("animated_mixed_lossless_key_superres_444_12bit_varied_s3_112x20_d9.avif", "173b397fc7cf11f9dc6ae65fffa940c7a879d5deb8f2df87335bf0e2ec9bf49c"),
    "animated_mixed_lossless_key_superres_400_8bit_varied_s3_128x128_d10": ("animated_mixed_lossless_key_superres_400_8bit_varied_s3_128x128_d10.avif", "ec9f489ed9eec383c1a333988a5e3b61e6efbc9373bb6808612e6ce8024cda55"),
    "animated_mixed_lossless_key_superres_400_10bit_varied_s3_128x128_d10": ("animated_mixed_lossless_key_superres_400_10bit_varied_s3_128x128_d10.avif", "1551d1351886eb0cafd4c3ab1aeb72da3049488f3f014ee4407b5da2402c4127"),
    "animated_mixed_lossless_key_superres_400_12bit_varied_s3_128x128_d10": ("animated_mixed_lossless_key_superres_400_12bit_varied_s3_128x128_d10.avif", "e509be8ff85da0bb8b9553c9db58819349e8751cd5bab789a2e683685e4b79ff"),
    "animated_mixed_lossless_key_superres_400_8bit_varied_s3_128x20_d10": ("animated_mixed_lossless_key_superres_400_8bit_varied_s3_128x20_d10.avif", "835ed4d5832c6798a5c2eb022121e2a501bcbfede889295a6195eab01366e99d"),
    "animated_mixed_lossless_key_superres_400_10bit_varied_s3_128x20_d10": ("animated_mixed_lossless_key_superres_400_10bit_varied_s3_128x20_d10.avif", "69dc712bb6f7fd326ee2f453a218413e10a1c0a58465ba147bf4271289d9eb66"),
    "animated_mixed_lossless_key_superres_400_12bit_varied_s3_128x20_d10": ("animated_mixed_lossless_key_superres_400_12bit_varied_s3_128x20_d10.avif", "548b05e77918217ca3dd983c10706039e86f1e4b007d76e66fc9b0ef2e12b567"),
    "animated_mixed_lossless_key_superres_400_8bit_varied_s3_80x20_d9": ("animated_mixed_lossless_key_superres_400_8bit_varied_s3_80x20_d9.avif", "49cefe1da0c4e2c56c45484336c4099cd78923675071d0343c2fab5f9a5cfb62"),
    "animated_mixed_lossless_key_superres_400_10bit_varied_s3_80x20_d9": ("animated_mixed_lossless_key_superres_400_10bit_varied_s3_80x20_d9.avif", "5470195c2ac498c0d317757835cd58e211ac130878a6a4c9e548cc6049fc89b9"),
    "animated_mixed_lossless_key_superres_400_12bit_varied_s3_80x20_d9": ("animated_mixed_lossless_key_superres_400_12bit_varied_s3_80x20_d9.avif", "734822466824cf283cc5b6cf6c348fafeb519d4be861bc7a61b103ceb9bd701b"),
    "animated_mixed_lossless_key_superres_400_8bit_varied_s3_112x20_d9": ("animated_mixed_lossless_key_superres_400_8bit_varied_s3_112x20_d9.avif", "bdfe1a8ab1bf9dad7c91f4dbd857ab1cc2378fc272ded17960bf5e2ccd1a2503"),
    "animated_mixed_lossless_key_superres_400_10bit_varied_s3_112x20_d9": ("animated_mixed_lossless_key_superres_400_10bit_varied_s3_112x20_d9.avif", "662344af2caed8f4110951d59819677496cf7c96a26a3a8a61866acb8ae5b306"),
    "animated_mixed_lossless_key_superres_400_12bit_varied_s3_112x20_d9": ("animated_mixed_lossless_key_superres_400_12bit_varied_s3_112x20_d9.avif", "8074f00bc56bcd5b67a9c1cbb2fcaa7493db8e8a63ffe21cbd5eb265a5e2e421"),
    "animated_mixed_lossless_key_superres_400_8bit_varied_s3_40x16": ("animated_mixed_lossless_key_superres_400_8bit_varied_s3_40x16.avif", "37426534a13298b0ccd46fcd79f6ed9def527b20ef0789c2cbeee16bedb1207b"),
    "animated_mixed_lossless_key_superres_400_10bit_varied_s3_40x16": ("animated_mixed_lossless_key_superres_400_10bit_varied_s3_40x16.avif", "65da92bb19404a088ded5f6912318c54e5d7063862a683fd9a8df0a8e236b306"),
    "animated_mixed_lossless_key_superres_400_12bit_varied_s3_40x16": ("animated_mixed_lossless_key_superres_400_12bit_varied_s3_40x16.avif", "c86a971a0cd76a5081ef586af951c2e5461d4549430925ab74b065b3a3f03bd9"),
    "animated_mixed_lossless_key_superres_400_8bit_varied_s3_60x20": ("animated_mixed_lossless_key_superres_400_8bit_varied_s3_60x20.avif", "bd99fe703376390ce1097eedb4492cfd9018bba5a8e8187f20ec35fc8dfb81f3"),
    "animated_mixed_lossless_key_superres_400_10bit_varied_s3_60x20": ("animated_mixed_lossless_key_superres_400_10bit_varied_s3_60x20.avif", "41f918afc0b1a4c33e40e2b302a8dc52fdaa50c0d017a55f72cb2a4a4efff40b"),
    "animated_mixed_lossless_key_superres_400_12bit_varied_s3_60x20": ("animated_mixed_lossless_key_superres_400_12bit_varied_s3_60x20.avif", "7609b307a9adc128386fa87380cea5dca007166a17ca3e5bd93859fe6e19e2e0"),
    "animated_mixed_lossless_key_superres_400_8bit_varied_s3_128x128_d16": ("animated_mixed_lossless_key_superres_400_8bit_varied_s3_128x128_d16.avif", "8538f6a8513c13f0dfe4f4efdce8dca0dc8f0e214daf2c0b711bda472421f953"),
    "animated_mixed_lossless_key_superres_400_10bit_varied_s3_128x128_d16": ("animated_mixed_lossless_key_superres_400_10bit_varied_s3_128x128_d16.avif", "4ed6ed92bf50fba6647fa2a9f58dce84f140ddba09347c66d4fd1d3020a76730"),
    "animated_mixed_lossless_key_superres_400_12bit_varied_s3_128x128_d16": ("animated_mixed_lossless_key_superres_400_12bit_varied_s3_128x128_d16.avif", "21bd2732b82cf7e9a213d2b1e5143432f16026fbe78d136182e69051d1099700"),
    "animated_mixed_lossless_key_superres_400_8bit_flat_80x48": ("animated_mixed_lossless_key_superres_400_8bit_flat_80x48.avif", "75ceb95aac1814941074dd5972f1ae95b69a4c4fb52b6a3fbecae002d662b206"),
    "animated_mixed_lossless_key_superres_400_8bit_varied_s3_80x48": ("animated_mixed_lossless_key_superres_400_8bit_varied_s3_80x48.avif", "74934829b5040745d8f266b56b07f62ea25f702a7d0b5f16ec29752490a6ccc4"),
    "animated_mixed_lossless_key_superres_400_10bit_varied_s3_80x48": ("animated_mixed_lossless_key_superres_400_10bit_varied_s3_80x48.avif", "7f064d7e8bb6ea2d0fb852db8fca02da7021f5d617a322c00a71c060b2dd16ec"),
    "animated_mixed_lossless_key_superres_400_12bit_varied_s3_80x48": ("animated_mixed_lossless_key_superres_400_12bit_varied_s3_80x48.avif", "da7835a6a45ca4eeb400e250e5f47f24908d5a59caeb69213404344a324b18ae"),
    "animated_mixed_lossless_key_superres_400_8bit_varied_s7_80x48": ("animated_mixed_lossless_key_superres_400_8bit_varied_s7_80x48.avif", "c21b123af4891f15367691d3c167de90da03b19045b5a94c07da7fac147f8d05"),
    "animated_lossy_inter_422_neutral_superres_8bit_80x48_d10": ("animated_lossy_inter_422_neutral_superres_8bit_80x48_d10.avif", "147ddc1287cbf44bdfc591190ef2b2212b6a668961d8ad1f9dfe7c2fe76e111f"),
    "animated_lossy_inter_422_neutral_superres_8bit_128x64_d16": ("animated_lossy_inter_422_neutral_superres_8bit_128x64_d16.avif", "3a8c24812f32856b16d670ef2c3b013f7971673b44c2ca077594ea217c2334d5"),
    "animated_lossy_inter_422_neutral_superres_8bit_40x16_d10": ("animated_lossy_inter_422_neutral_superres_8bit_40x16_d10.avif", "a2e188c2232952bccff3d1d9d19ee1614b381193a9a4638be3af7b3dff4d3e94"),
    "animated_lossy_inter_422_neutral_superres_8bit_80x48_d9": ("animated_lossy_inter_422_neutral_superres_8bit_80x48_d9.avif", "6c8a1ea5d23ef79d4261d585b520918deb4cf0d4fce939dd1563fafd5400838f"),
    "animated_monochrome_sgr_superres_8bit_80x48": ("animated_monochrome_sgr_superres_8bit_80x48.avif", "9753ecbfbe6c924e1199968ebd44064656562f0265252511df730b22030b3924"),
    "animated_monochrome_sgr_superres_10bit_80x48": ("animated_monochrome_sgr_superres_10bit_80x48.avif", "e3703a35efc5085564f5c2e570808bb72a5e6e4131442017ab5dca5a0b198afb"),
    "animated_monochrome_neutral_superres_8bit_80x48": ("animated_monochrome_neutral_superres_8bit_80x48.avif", "b7daf431bafe9eb89a9c2b4f34eda3ac50d8477f9591ec85a8fd4cbf8c398f30"),
    "animated_monochrome_neutral_superres_10bit_80x48": ("animated_monochrome_neutral_superres_10bit_80x48.avif", "b73ca9f56bb2f60dbe29e85c5cdb687982e6d2cbbc0e93de362312f580881177"),
    "animated_monochrome_neutral_superres_12bit_80x48": ("animated_monochrome_neutral_superres_12bit_80x48.avif", "116e22846e49619fec712071799ff602b2ee7f4178bb3cd4920ebd7bbd56feb1"),
    "lossless_inter_420_derived_skip_refs_b32x32_64x64": (
        "animated_lossless_inter_420_derived_skip_refs_b32x32_64x64.avif",
        "50d8bdd33c5a5e958d5991d25767748088ba3354d35d4b809d52e84a45713c15",
    ),
    "animated_lossless_inter_420_derived_skip_refs_clipped_b32x32_56x64": (
        "animated_lossless_inter_420_derived_skip_refs_clipped_b32x32_56x64.avif",
        "b801ab1e84328dfe552543858fad84a59c7edeb7ac7497972c2499b0736d6eb5",
    ),
    "lossless_inter_i422_derived_skip_refs_b32x32_64x64": (
        "animated_lossless_inter_i422_derived_skip_refs_b32x32_64x64.avif",
        "0eae260be09a74b98b1f525261a237ef3783105870de44224de91dd65ee5d168",
    ),
    "lossless_inter_i444_derived_skip_refs_b32x32_64x64": (
        "animated_lossless_inter_i444_derived_skip_refs_b32x32_64x64.avif",
        "3769c145bb4d97f357a8b3c4660fb8a2f976c682643304e64833d06515901009",
    ),
    "lossy_inter_420_skip_mode_enabled_64x64": (
        "animated_lossy_inter_420_skip_mode_enabled_64x64.avif",
        "98ead811784a1a8434582d6993fdc9891672543b59ed6135fbb0feabc205c8e3",
    ),
    "animated": ("animated.avif", "2f8683d21725261f37f86e115f0c212cc52d0fefd3a2ddfcc4fa648c1859906d"),
    "animated_monochrome_superres_sequence_17x16": (
        "animated_monochrome_superres_sequence_17x16.avif",
        "d7018788a6c11276df9721e556e726542baa7c6195acb3d42f8404ed4d2b0310",
    ),
    "animated_duplicate_alpha_track": (
        "animated_duplicate_alpha_track.avif",
        "a1635ad5da1194bed67af764f88f49801cc4afbe4711c71eb9b59404eca4c89a",
    ),
    "animated_duplicate_track_tref_hint_before_edts": (
        "animated_duplicate_track_tref_hint_before_edts.avif",
        "f20b077c2b38c9e8e944f4a39549fe215a9d864374c34fa8b44180b86c60254e",
    ),
    "animated_duplicate_track_av1c_conflicting_depth": (
        "animated_duplicate_track_av1c_conflicting_depth.avif",
        "5325225cbcd2171dbc009a935e6318a995be90ea1641bcf6f4eecc277aa445c8",
    ),
    "animated_duplicate_track_auxi_conflicting_alpha": (
        "animated_duplicate_track_auxi_conflicting_alpha.avif",
        "d549d98a957011df232993ab5605012110e173bb200a1af1f7a0773c74464cc5",
    ),
    "animated_track_duration_repeating_5_of_5": (
        "animated_track_duration_repeating_5_of_5.avif",
        "076c6e24ac7dc4c91f82b94ed214e57da2e2d0392269782e3097427e128bcca1",
    ),
    "animated_track_duration_repeating_6_of_5": (
        "animated_track_duration_repeating_6_of_5.avif",
        "fc517b7bd5850eff7f50332298ba8ac6772958c07965d2d6a5bb311c14e7f1b2",
    ),
    "animated_track_duration_repeating_large": (
        "animated_track_duration_repeating_large.avif",
        "65f39e3119b871fa15333c98f52c00c69d1064bcb0b9da89ae46d3871ff8ece3",
    ),
    "animated_sequence_only_no_meta": (
        "animated_sequence_only_no_meta.avif",
        "c861761e45518018150dc52302da7ff5015d1f259ff2c1309eea94eaf8176fdb",
    ),
    "animated_sequence_only_trailing_meta_after_mdat": (
        "animated_sequence_only_trailing_meta_after_mdat.avif",
        "a58d67aaece0bd6502b9f3bb63467e22de4452d4d99d9fb21d088d371f0e0c1f",
    ),
    "animated_sequence_only_interstitial_meta_before_moov": (
        "animated_sequence_only_interstitial_meta_before_moov.avif",
        "2c60d420f540730712e700640cd01aa8d71657029ed4873321137eecf159fc59",
    ),
    "animated_timing_without_decoder_model": (
        "animated_timing_without_decoder_model.avif",
        "7978ad21887be7ec5e74b13ab9555e2c2caef9911b900947b67c231b74f1f5cf",
    ),
    "animated_equal_picture_interval_two_ticks": (
        "animated_equal_picture_interval_two_ticks.avif",
        "e49851e6d94c721525f3b4130d7279c2ed2329a68032ea81f3714ae68119db7d",
    ),
    "animated_order_hint_disabled": (
        "animated_order_hint_disabled.avif",
        "565363788fd1a30cc51b2f3f3d4852a447292646de0b37ab81589590a93e706d",
    ),
    "animated_screen_content_tools_disabled": (
        "animated_screen_content_tools_disabled.avif",
        "d0c88a39ca31c30f56afa5540505d7e20892ed0d9d5339ff843cdf6d9040a357",
    ),
    "animated_integer_mv_disabled": (
        "animated_integer_mv_disabled.avif",
        "47b3767081913f190e0473842a466a9a81ae4acce5e5099c323c9793187cdcce",
    ),
    "animated_transfer_characteristics_bt709": (
        "animated_transfer_bt709.avif",
        "8b7c981b052241b55112e6ae3ce64b00681c101a25c7d9bd9c9091d727417af2",
    ),
    "animated_identity_matrix_profile1_444": (
        "animated_identity_matrix_profile1_444.avif",
        "4535db4548ddf82d23c72ef970b01f1632f4db4c9f7ddfd392bf0d5d011bdd4a",
    ),
    "animated_identity_matrix_profile2_12bit_444": (
        "animated_identity_matrix_profile2_12bit_444.avif",
        "d0079f47a1f7709a373191d2248d42daec16d01132b6aaf1cdb1d7dc613fb1da",
    ),
    "animated_initial_display_delay_10": (
        "animated_initial_display_delay_10.avif",
        "aa95d8f4b6f2576f91a010fd0cc1cd5ed10ea826b89f8aba661c3dd6a9fa1b7d",
    ),
    "animated_operating_point_high_tier": (
        "animated_operating_point_high_tier.avif",
        "bb06afad6c56b58389eede0bdda980ca42ee36ca045eb92e39de7f5a85e19fb0",
    ),
    "animated_decoder_model_without_operating_point_parameters": (
        "animated_decoder_model_without_operating_point_parameters.avif",
        "e220130a0614df8c5fce0acf525c2c69a9fccaa8ec140373eac1f91d424d757d",
    ),
    "animated_stsd_trailing_payload": (
        "animated_stsd_trailing_payload.avif",
        "153ead664b729f1854c738ddd2e771e18c7488457f7716d8516df626eb2543ef",
    ),
    "animated_unknown_sample_entry_ignored_hint_track": (
        "animated_unknown_sample_entry_ignored_hint_track.avif",
        "dcc55d191d0f5e9cbabcd1dfbc9119b2f8e712ec1cea33880873739ed9987a49",
    ),
    "animated_decoder_model_parameters_change": (
        "animated_decoder_model_parameters_change.avif",
        "9191549ae1373fc31e093a36d9ff2d88525f4219378c7b7ba1ecb6b74cec8396",
    ),
    "animated_sequence_header_cicp_mismatch_nclx": (
        "animated_sequence_header_cicp_mismatch_nclx.avif",
        "74a0a8f8725cc7b44232c4e970859be1ace37ad1898f80da38dd5cc98b7ed628",
    ),
    "animated_co64_chunk_offsets": (
        "animated_co64_chunk_offsets.avif",
        "589f8836e3e1fbe9f8524353b5bc56cc615927dc15848c1e4fb671c467c04021",
    ),
    "animated_stsc_two_chunk_mapping": (
        "animated_stsc_two_chunk_mapping.avif",
        "117a8b2d61e01d9e9a5d911313524509c4b99d527b108e552a0b6e27d2a8500c",
    ),
    "animated_stsc_trailing_payload": (
        "animated_stsc_trailing_payload.avif",
        "afd53fee7738e9b4fe280bb0ff1a19f938cdaad326977abf2cb4266c8e376e4b",
    ),
    "animated_stco_trailing_payload": (
        "animated_stco_trailing_payload.avif",
        "e773e124d5c3954d8fd0ca5b7fde7f6ad4d50b198dddc20b86b45a1cc70e2d15",
    ),
    "animated_stss_trailing_payload": (
        "animated_stss_trailing_payload.avif",
        "607f1a675785f65fe6579920e803969030a3b67e13152f7303b12b97c3c60768",
    ),
    "animated_zero_track_tref_auxl_before_edts": (
        "animated_zero_track_tref_auxl_before_edts.avif",
        "00bd7d70c5c9285d612bfec8fcca43de555ee0d5f7deedd0cb0f52bc03971563",
    ),
    "animated_stsz_trailing_payload": (
        "animated_stsz_trailing_payload.avif",
        "ec5869776d517f36dcbd9d19eb05e8a87dd709b3651a2b2ca00a826cf546f8c3",
    ),
    "animated_stsc_zero_description_index_first": (
        "animated_stsc_zero_description_index_first.avif",
        "c6f3ff4272a7ade6f51cd77a3c643fb7301e416e4e5ebbd75e4fca4397c5db64",
    ),
    "animated_duplicate_stsc": (
        "animated_duplicate_stsc.avif",
        "56b417e69e63d2d56be80d3e58a71b8043a4134c7e8f283b2ed5e088357e43d0",
    ),
    "animated_duplicate_stsc_override_four": (
        "animated_duplicate_stsc_override_four.avif",
        "3343bc98df777765f8c79b1c180832af10262503a187af3a4206894547949589",
    ),
    "animated_duplicate_media_boxes": (
        "animated_duplicate_media_boxes.avif",
        "340adc83f1288ad23d5efe2511a8b100381d862108210c7b3d4b5ef57a5def87",
    ),
    "animated_duplicate_top_level_meta_after_mdat": (
        "animated_duplicate_top_level_meta_after_mdat.avif",
        "86ae1b02cbcec44de7ca7bf5c08815a1ac9ca4325f5d74fb89835fb37516156d",
    ),
    "animated_duplicate_top_level_moov_after_mdat": (
        "animated_duplicate_top_level_moov_after_mdat.avif",
        "f88aa18f6c19d1f29431dfec69eff4ac0f4cc5b91e56c4c84206d0ab629e59b5",
    ),
    "animated_missing_hdlr": (
        "animated_missing_hdlr.avif",
        "eb3369448430e7ad13614ee1e7c9caea16cb885667297114ca2a21747326f891",
    ),
    "animated_primary_item_irot": (
        "animated_primary_item_irot.avif",
        "39e6cb6e7227b4b748335c2e8093316195c7eceeda9433377aaa8ebff2debafe",
    ),
    "tkhd_version_zero": (
        "animated_tkhd_version_zero.avif",
        "eb7b131fb8288fbe8e67daf9e41e7f80bdf3f923d474177fac84a0b58550b674",
    ),
    "tkhd_version_zero_unknown_duration": (
        "animated_tkhd_version_zero_unknown_duration.avif",
        "ca1834317cd4b5b0db00130370cf3db3c94426ff7479c6414c4da2cabfef2a09",
    ),
    "motion_multitile": (
        "animated_motion_multitile.avif",
        "c248f019a008e9dea4425e1d5f44d1418b44929604312d95a69a0a3ac5d4709f",
    ),
    "motion_multitile_split_groups": (
        "animated_motion_multitile_split_groups.avif",
        "eb19bcebc0698dfa3149c76fb6828705b22e37f45e213b70541a61b3abd20b89",
    ),
    "lossless_motion_multitile": (
        "animated_lossless_motion_multitile.avif",
        "24612186624c6b470a50a549d876469c8265976633afee87fa030d910cd0532b",
    ),
    "lossless_motion_multitile_split_groups": (
        "animated_lossless_motion_multitile_split_groups.avif",
        "04b61cf8b3e9887ee651cec5dc055b5a102198de9f69e2ad64b4e183639cf596",
    ),
    "opidc_0x101": ("animated_opidc_0x101.avif", "25b79a856ea2767e02e5a509f72e7af3f9563202301779be65b0745724ab23be"),
    "error_resilient": ("animated_error_resilient.avif", "06ea9771f8b46c3432c6c6cdf324f1c05e86a5fdccd774c8e3c9a8fce0b831f0"),
    "frame_id_high_contiguous": (
        "animated_frame_id_high_contiguous.avif",
        "25c5e52ca0759816ce94f4aa5d2be42ed99353a004da85e1cf80e8006925eecf",
    ),
    "frame_id_future_reference_invalidation": (
        "animated_frame_id_future_reference_invalidation.avif",
        "ebe40d4c20e39723979ac8fc5968c846b54f30045739af26d8d91974d7492cbb",
    ),
    "frame_id_stale_reference_invalidation": (
        "animated_frame_id_stale_reference_invalidation.avif",
        "3b8ce1b8ddccf7b114f2ceac3505c254d73bc823d2138cae01106ce04029dd6d",
    ),
    "frame_id_wrap_contiguous": (
        "animated_frame_id_wrap_contiguous.avif",
        "8a53bf96c97b66d07b849ec6e4a6dcf18f37ab9db5b3edb045f2b357bd59e35e",
    ),
    "filmgrain_reference_reuse_i444_64x64": (
        "animated_filmgrain_reference_reuse_i444_64x64.avif",
        "bd947085ed6437edfd50a97b43506cc56e96af8b5450a8ef5cb8289b8ec62b34",
    ),
    "filmgrain_inactive_reference_reuse_i444_64x64": (
        "animated_filmgrain_inactive_reference_reuse_i444_64x64.avif",
        "d1c4c2b6c9c24571452ccb95387a4c18580d9e7b871e704217b17b58ae916536",
    ),
    "lossless_inter_420_b16x16": (
        "animated_lossless_inter_420_b16x16.avif",
        "6973cfff29185ffc7283ec539d9cf6736dfe4cba52850c46e4b196ba9db51ead",
    ),
    "lossless_inter_420_fixed_b4x4_64x64": (
        "animated_lossless_inter_420_b4x4_leaves_64x64.avif",
        "8c588829bd49a125fee501c9cdcaa1e6f9c0e6f058084270eaa039688acc9748",
    ),
    "lossless_inter_420_b8x16": (
        "animated_lossless_inter_420_b8x16.avif",
        "9e3fbac5e42c61413ec8c1f7f4f72bc1338fb7cfd21587e30826855732e369c0",
    ),
    "lossless_inter_intra_i444_b8x8_20x20": (
        "animated_lossless_inter_intra_i444_b8x8_20x20.avif",
        "7636f2de4078d5271941e6cf7de85f9c77205acce155c75e529e43fbc1d0acbf",
    ),
    "odd_dimensions_inter_intra_boundary_420": (
        "animated_odd_dimensions_inter_intra_boundary_420.avif",
        "736914091e00473af577a12de9910ce6abe2b038c84d2f580c5f528b0618c422",
    ),
    "lossy_interintra_420_nowedge_b16x16_64x64": (
        "animated_lossy_interintra_420_nowedge_b16x16_64x64.avif",
        "adc58d72626865406be3d0a048ecf0b9a1670d16a68f48babe4b9167a2dc2dea",
    ),
    "lossy_interintra_420_nowedge_mode1_b16x16_64x64": (
        "animated_lossy_interintra_420_nowedge_mode1_b16x16_64x64.avif",
        "dd4e06e559d83cc0863d65878799d2f7496c8f5d378eaeb639704fe3ac539f2f",
    ),
    "lossy_interintra_420_nowedge_mode2_b16x16_64x64": (
        "animated_lossy_interintra_420_nowedge_mode2_b16x16_64x64.avif",
        "08f616c2e14f9abadb6157c425653b5a7fd703f2797f3db04e23b58bc519eda8",
    ),
    "lossy_interintra_420_nowedge_mode3_b16x16_64x64": (
        "animated_lossy_interintra_420_nowedge_mode3_b16x16_64x64.avif",
        "7e035cef7293dfff72e50ff8c729bc0fad360ee14d7bf80d663b82b96ca7c4d7",
    ),
    "lossy_interintra_420_wedge_b16x16_64x64": (
        "animated_lossy_interintra_420_wedge_b16x16_64x64.avif",
        "1510b764d7e642f2caf02fff53e6dc966cfb92df8eca2e8f2d91c0b3bead1791",
    ),
    "lossy_interintra_420_wedge_idx2_no_flip_b16x16_64x64": (
        "animated_lossy_interintra_420_wedge_idx2_no_flip_b16x16_64x64.avif",
        "62f1f2704bdd6efcd3b936909dd3d9351116fb8d072add8a0194d9ca5459e275",
    ),
    "lossless_inter_420_b32x32": (
        "animated_lossless_inter_420_b32x32.avif",
        "65e8617044f7f12db081f65276235296f8dc208a006451210ebb9603f27f20a7",
    ),
    "lossless_inter_420_qmatrix_level15_qindex0_16x64": (
        "animated_lossless_inter_420_qmatrix_level15_qindex0_16x64.avif",
        "b76924ec8cf9fa01ad27e48dec5990332b089d75460fa7bf37d9fa00108100de",
    ),
    "lossless_inter_420_b16x16_10bit": (
        "animated_lossless_inter_420_b16x16_10bit.avif",
        "8eb14ad3c57916b9e7fdded28b08a9254302cfc76f0cb5a934b41f11dc4184b6",
    ),
    "lossless_inter_420_b16x16_10bit_small_patch": (
        "animated_lossless_inter_420_b16x16_10bit_small_patch.avif",
        "99dd72e3dc39af9f7d9bcb89d5e3bb0553f7cc04db021c0107fe2f4dcd06ae7b",
    ),
    "lossless_inter_420_b32x32_10bit": (
        "animated_lossless_inter_420_b32x32_10bit.avif",
        "c90bf9e75b0c091e19ab6b8aa70c17243ae1dbbf818493e815015f998f14e73a",
    ),
    "lossless_inter_420_b32x32_10bit_partition32_full_texture": (
        "animated_lossless_inter_420_b32x32_10bit_partition32_full_texture.avif",
        "992e7bfe661bcdc1390127ae716059acb0f5ecdd79f12b5869eaa7bb89831257",
    ),
    "lossless_inter_420_b32x32_10bit_64x64": (
        "animated_lossless_inter_420_b32x32_10bit_64x64.avif",
        "62a154bb3e8a7c92816045d58f492aaa30c3aa215526f6c3d7433de942fb2448",
    ),
    "lossless_inter_420_b128x128_10bit": (
        "animated_lossless_inter_420_b128x128_10bit.avif",
        "a1056a8b40154eab74c1571cce2c39832e18a37de4b8bd1a3f808c6df231e851",
    ),
    "animated_lossless_inter_420_wide_grid_b128x128": (
        "animated_lossless_inter_420_wide_grid_b128x128.avif",
        "e44c5a53a1cf185a29b4a6570836c82078d1cd2e1d0d0963e69ebf1028f7d47f",
    ),
    "lossless_inter_monochrome_clipped_wide_b128x128_124x128": (
        "animated_lossless_inter_monochrome_clipped_b128x128_124x128.avif",
        "7553404e35f1583e5bb2e785acb4908de5a4bbfa8d888054b163cc8da32391e8",
    ),
    "animated_lossless_compound_wide_i420_b128x128": (
        "animated_lossless_compound_wide_i420_b128x128.avif",
        "7e736d84c4a9477930c013517e4f3a8e4ad759ebbcf524ecbb67240b8bbfaefc",
    ),
    "animated_lossless_compound_vertical_i420_b64x128": (
        "animated_lossless_compound_vertical_i420_b64x128.avif",
        "eb40e16aa47a612c272e29c2ad2cd62a7dc6115cce8cd40da1f1a01f95ec166a",
    ),
    "animated_lossless_inter_420_vertical_b64x128": (
        "animated_lossless_inter_420_vertical_b64x128.avif",
        "9ed8dd60feedcb8480bb8d6de7f8ea2ec23583da923e916e28a128c3746ca650",
    ),
    "mixed_lossless_inter_b32_i420_64x64": (
        "animated_mixed_lossless_inter_b32_i420_64x64.avif",
        "04c15f5bdc0e640fcfa09d22f63378d0655109db67a8f9236942c92dc0d8dbf6",
    ),
    "mixed_lossless_inter_b32_i420_60x60": (
        "animated_mixed_lossless_inter_b32_i420_60x60.avif",
        "acdf18ae0fbbaf52cf6b888ab9c1f33b78b06e529bc545736d995c61d6d36ab2",
    ),
    "mixed_lossless_inter_b32_i420_residual_64x64": (
        "animated_mixed_lossless_inter_b32_i420_residual_64x64.avif",
        "58552db6049a5220936acccbb483fd0366224744e7a2f81058a099b4016d5d36",
    ),
    "mixed_lossless_inter_b32_i420_impulse_residual_64x64": (
        "animated_mixed_lossless_inter_b32_i420_impulse_residual_64x64.avif",
        "321a0c17722c9e3739cf61e1bbc57bf52385b58a5588e185a760f81f00380547",
    ),
    "mixed_lossless_inter_b32_i420_10bit_impulse_residual_64x64": (
        "animated_mixed_lossless_inter_b32_i420_10bit_impulse_residual_64x64.avif",
        "9a136b442ee7f742259cabc9bb1b2ce68004f9831eb4b169f08fa507f6dabbce",
    ),
    "lossless_inter_420_clipped_b32x32_10bit_28x28": (
        "animated_lossless_inter_420_clipped_b32x32_10bit_28x28.avif",
        "f9d829f20968483dbd078a47f136b9c413205ce78d423fb0b5f1bdb2d2c0cce4",
    ),
    "lossless_inter_420_clipped_b32x32_10bit_31x32": (
        "animated_lossless_inter_420_clipped_b32x32_10bit_31x32.avif",
        "843526363c1a61d5957ad85aa6e43d32c4fc6ff95f84484ffca4c87a5413defd",
    ),
    "lossless_inter_420_clipped_b32x32_10bit_partition32_31x32": (
        "animated_lossless_inter_420_clipped_b32x32_10bit_partition32_31x32.avif",
        "3965e171dbbc38077d36b7dbf93ac7cb17aeec94efc99cb3deb8c95b480ea325",
    ),
    "lossless_inter_420_clipped_b32x32_10bit_partition32_28x64": (
        "animated_lossless_inter_420_clipped_b32x32_10bit_partition32_28x64.avif",
        "8f2b787faea151faf9c7f69c2b92e098dedd3bb70f28fd33546194c002b3456f",
    ),
    "lossless_inter_420_clipped_b32x32_10bit_partition32_56x64": (
        "animated_lossless_inter_420_clipped_b32x32_10bit_partition32_56x64.avif",
        "a819711996c804d7c5e4dcc26cee2e6a35bb82c48a88a6db8e34bd61d3b5f9fe",
    ),
    "lossless_inter_420_clipped_b32x32_10bit_partition32_63x64": (
        "animated_lossless_inter_420_clipped_b32x32_10bit_partition32_63x64.avif",
        "248ea28edff232b792c3f9e71b05ae6762f0f807b9981cd91ba9ea7ff19da42f",
    ),
    "lossless_inter_420_clipped_b32x32_12bit_partition32_56x64": (
        "animated_lossless_inter_420_clipped_b32x32_12bit_partition32_56x64.avif",
        "cec5a564546c2ef8893c3f4e44c7f46c49ea9daa89d1f782a06717f11d183c87",
    ),
    "lossless_inter_420_clipped_b32x32_17x17": (
        "animated_lossless_inter_420_clipped_b32x32_17x17.avif",
        "b29bedb6d7486dae5bed2abb74986a65e47bc737bfd26f85b1f69fa8bd666819",
    ),
    "lossless_inter_i444_clipped_b32x32_49x64": (
        "animated_lossless_inter_i444_clipped_b32x32_49x64.avif",
        "d45defceb497272879b1093f76e6a6c645f0bde8a6d10a1903e2322af422caae",
    ),
    "lossless_inter_i422_clipped_b32x32_49x64": (
        "animated_lossless_inter_i422_clipped_b32x32_49x64.avif",
        "fd18cdcef915327fd4aab6992c901e27f2efc1411d1d2d01eee93dd3f277d7b1",
    ),
    "lossless_inter_i422_clipped_b32x32_52x64": (
        "animated_lossless_inter_i422_clipped_b32x32_52x64.avif",
        "43bbcd3d92593e026d88ce4dea727074f74134175ee42a962886c6de697a4e05",
    ),
    "lossless_inter_i444_clipped_b32x32_64x56": (
        "animated_lossless_inter_i444_clipped_b32x32_64x56.avif",
        "9004ec840b2a413c0e15294f52c723ebe5c3689cbe3decbd4afd5575cad78e23",
    ),
    "lossless_inter_i422_clipped_b32x32_52x60": (
        "animated_lossless_inter_i422_clipped_b32x32_52x60.avif",
        "53e06d79c586151d920b94cc6173eebcc1f9869ebb0f1fa5bec673df11c45e50",
    ),
    "lossless_inter_i444_clipped_b32x32_52x60": (
        "animated_lossless_inter_i444_clipped_b32x32_52x60.avif",
        "9fdd099eb28fed9eaff2760b154e42d91933758fd6b01e532e441646f564993a",
    ),
    "lossless_inter_420_clipped_b32x32_60x64": (
        "animated_lossless_inter_420_clipped_b32x32_60x64.avif",
        "a60dde64c6b7bf9f6fbb3afe7e75a74374d446f8106613b00837300df4963bbc",
    ),
    "lossless_inter_420_clipped_b32x32_64x60": (
        "animated_lossless_inter_420_clipped_b32x32_64x60.avif",
        "7602fd062d0aefdaa0221b35a3a6c9403fecc27bf8a66cb00c0b4f9b8763c0d6",
    ),
    "lossless_inter_420_clipped_b32x32_184x64": (
        "animated_lossless_inter_420_clipped_b32x32_184x64.avif",
        "0a8c935fe67694fe576e7f21064eec4c428cffc2a05a3ff2d0be33e401a20c1d",
    ),
    "lossless_inter_420_clipped_b32x32_185x64": (
        "animated_lossless_inter_420_clipped_b32x32_185x64.avif",
        "701d247ebbe2666b1f28f447fa1a086573e7dc6d88b0b29405f9f7a6f9460b59",
    ),
    "lossless_inter_420_clipped_b16x16_60x64": (
        "animated_lossless_inter_420_clipped_b16x16_60x64.avif",
        "7c8bd0f88cd6654fb1150d2220ecb33c6ec5874b26a2250909b51cab4ed2e17f",
    ),
    "lossless_inter_420_clipped_b16x16_10bit_partition16_60x64": (
        "animated_lossless_inter_420_clipped_b16x16_10bit_partition16_60x64.avif",
        "ce481c1839200b05932d69730174d5967973aa7f4b3af874a108f2660f01142f",
    ),
    "lossless_inter_monochrome_b16x16": (
        "animated_lossless_inter_monochrome_b16x16.avif",
        "d29aaab346465bf7ac413954e3c862b2222d3ac510299f865fe579899d063fb5",
    ),
    "lossless_inter_monochrome_b32x32_10bit": (
        "animated_lossless_inter_monochrome_b32x32_10bit.avif",
        "28427120f6a0e9de71d0abcfe36bbeaae4d701dacefb62f852dfd71eeb82d624",
    ),
    "lossy_split_inter_420_b16x16": (
        "animated_lossy_split_inter_420_b16x16.avif",
        "9eb50f5a45dc2eb549c62ac9bcb67abd931a977076685bcf3c0f69e6827ce323",
    ),
    "lossy_b16_mixed_topology_inter_420_b16x16": (
        "animated_lossy_b16_mixed_topology_inter_420_b16x16.avif",
        "3264482e2a50d80bd39be178b843fdfe9997e947d54b4466c745751837821d74",
    ),
    "lossy_wide_monochrome_b128x128": (
        "animated_lossy_wide_monochrome_b128x128.avif",
        "eb8dda5000882ffd03c182a817e08fc944ef110afa1e78aaf1678bef5efc65bf",
    ),
    "lossy_wide_i420_b128x128": (
        "animated_lossy_wide_i420_b128x128.avif",
        "a8cdebb85ccc240bbab9adde924e13b13d82e76d22600beb8511e62b58ae8bdd",
    ),
    "lossy_wide_i420_10bit_b128x128": (
        "animated_lossy_wide_i420_10bit_b128x128.avif",
        "4167f64f7b7336c0f16cbac49379cae49671a988f80b2ba066fdec9fe54c32d4",
    ),
    "lossy_compound_wide_i420_b128x128": (
        "animated_lossy_compound_wide_i420_b128x128.avif",
        "d0491ed004f7e6d722ce55b7cbeaa6db1898073a9868b306ef412f89693b7f71",
    ),
    "lossy_compound_wide_i420_unsplit_b128x128": (
        "animated_lossy_compound_wide_i420_unsplit_b128x128.avif",
        "8fed3a09dd26a5baf9aa52918f479ee8f83ebfe49642f8b1c021a86fca9abe35",
    ),
    "lossy_interinter_wedge_i420_b16x16_64x64": (
        "animated_lossy_interinter_wedge_i420_b16x16_64x64.avif",
        "48defb0aff134ef32ed8c2e0ade85619f7c0dc1d48bc2473b7b8a67b0e54ffed",
    ),
    "lossy_interinter_wedge_i422_b16x16_64x64": (
        "animated_lossy_interinter_wedge_i422_b16x16_64x64.avif",
        "c8b77442e52d68206e6f4100a940a7e85a8deae6677371712ba6b0c760b565b6",
    ),
    "lossy_interinter_wedge_i444_b16x16_64x64": (
        "animated_lossy_interinter_wedge_i444_b16x16_64x64.avif",
        "9525cb01d99c78fe0b479a37da45228ac168c468f4759e90b4cc3d37c3e51e66",
    ),
    "lossy_interinter_diffwtd_i420_b16x16_64x64": (
        "animated_lossy_interinter_diffwtd_i420_b16x16_64x64.avif",
        "cfd5adb4e0c97a9c3d206f2de0b0a8ce195fd7aa4f69e3b937129cf581986ae0",
    ),
    "lossy_interinter_distance_i420_b16x16_64x64": (
        "animated_lossy_interinter_distance_i420_b16x16_64x64.avif",
        "7abfec6d36ae819aaea9a9dc814c33c538488ba87362cd174b5cd62dfcaaab13",
    ),
    "lossy_interinter_superres_distance_i420_b16x16_64x64": (
        "animated_lossy_interinter_superres_distance_i420_b16x16_64x64.avif",
        "bf6049e6ed687af3367ec31f43517f5cbcd5633171d7dd381b6eb7a6ae9f36ad",
    ),
    "lossy_interinter_distance_altref_i420_b16x16_64x64": (
        "animated_lossy_interinter_distance_altref_i420_b16x16_64x64.avif",
        "89ffa2673b63ff0c8afc9c59ce2cc76d932b5bdbaeedeef63b7bb19dec8d927c",
    ),
    "lossy_interinter_distance_ratio_edge_i420_b16x16_64x64": (
        "animated_lossy_interinter_distance_ratio_edge_i420_b16x16_64x64.avif",
        "abc38ca334493341926ff5f728ece919a1b71569cd8bfd4dec08f18d180811ba",
    ),
    "lossy_interinter_diffwtd_i420_inverted_b16x16_64x64": (
        "animated_lossy_interinter_diffwtd_i420_inverted_b16x16_64x64.avif",
        "23c64470703c11c0ecfedfd906cc7b5098db1cc4b707676464c8c8064f91dd45",
    ),
    "lossy_interinter_diffwtd_i422_b16x16_64x64": (
        "animated_lossy_interinter_diffwtd_i422_b16x16_64x64.avif",
        "fad2e3b5264d0ec4d58dd53beea98c15462fbf36fd02e0a82a3f21447e291962",
    ),
    "lossy_interinter_diffwtd_i444_b16x16_64x64": (
        "animated_lossy_interinter_diffwtd_i444_b16x16_64x64.avif",
        "abafc8fd5f727e83dbef669a3e43e57b283e780e0274a55f540da9070eae837b",
    ),
    "lossy_wide_i422_b128x64": (
        "animated_lossy_wide_i422_b128x64.avif",
        "dbfb8c364abaddaeab4894511ff28f42fa9512226a27af097da35975a4ee5dbc",
    ),
    "lossy_wide_i444_b128x128": (
        "animated_lossy_wide_i444_b128x128.avif",
        "b4693fd2fb43d1c9406fe4ac8d782d74709bf8eed1b2d36771c0bd80763d84e8",
    ),
    "lossy_wide_i444_mode2_unsplit_b128x128": (
        "animated_lossy_wide_i444_mode2_unsplit_b128x128.avif",
        "ea87cfc19c427135396b44ea6b287b24e5301437abb5bd6769666d9a24144565",
    ),
    "lossy_wide_i444_mode2_split32_b128x128": (
        "animated_lossy_wide_i444_mode2_split32_b128x128.avif",
        "f48a6d235c7ab245e8d57aedcc6135b897cea1bcbc579c3d630aa7c20e1a61aa",
    ),
    "lossy_wide_i444_mode2_deep16_b128x128": (
        "animated_lossy_wide_i444_mode2_deep16_b128x128.avif",
        "a806f4585aa11699bc831fff1b02b7f1eacbd2089a88a2578bb6933afbcbdaf0",
    ),
    "lossy_wide_i444_mode2_mixed_b128x128": (
        "animated_lossy_wide_i444_mode2_mixed_b128x128.avif",
        "0e72f7d47e8e83e0a6e13a76a816cb98974cdb2976fff72857901712e2633d61",
    ),
    "lossy_i420_mode2_variable_tx_loopfilter_b128x128": (
        "animated_lossy_i420_mode2_variable_tx_loopfilter_b128x128.avif",
        "7be16326561aa3e67340b787cecfe9c5917ea282b312a28f665a03549fd62e31",
    ),
    "lossy_global_rotzoom_compound_i444_b128x128": (
        "animated_lossy_global_rotzoom_compound_i444_b128x128.avif",
        "14ffb1529c54f07a397a1fe94d02135f9def3a7730416cb80d5acbf3c54c144e",
    ),
    "lossy_global_subexp_endpoint_i444_b128x128": (
        "animated_lossy_global_subexp_endpoint_i444_b128x128.avif",
        "ff156ac8e1bfeb017b38846ef0dc4abca0312083523d317a7a892bde16ef3e97",
    ),
    "lossy_global_halfblend_spatial_i444_b256x256": (
        "animated_lossy_global_halfblend_spatial_i444_b256x256.avif",
        "ec2f145f64fa87c5b7c255b13b6009fb58760be06070724ed367cd4ccd9a6d7b",
    ),
    "lossy_inter_i444_tx_largest_64x64": (
        "animated_lossy_inter_i444_tx_largest_64x64.avif",
        "270ab1fb4f9e066d6898c52bdd8d94911502684d13eb7cdc62d1c034a57148ea",
    ),
    "motion_chroma": ("animated_motion_chroma.avif", "473469335e58cff3c442df51b4bbdca00b9467e8f352319812f50cf789748535"),
    "has_chroma_4x4": ("animated_has_chroma_4x4.avif", "f9434d10ee465784ccb6c09f03c98813282f7033e3f06b90739e7815ec1eb75b"),
    "motion_chroma_422": ("animated_motion_chroma_422.avif", "5a83530dcc60f75f0bec7d36cb4db67ae7258b2acc481cbb08cc0987247ea66a"),
    "lossy_inter_422_checker_random_b32x32": (
        "animated_lossy_inter_422_checker_random_b32x32.avif",
        "e46aef4aed9076da48627a449e53c6f318a0494ba5d94a32a4f13edc548ab134",
    ),
    "animated_lossy_inter_422_cdef_8bit_16x16": (
        "animated_lossy_inter_422_cdef_8bit_16x16.avif",
        "84446e8a38a5ad9a742f6e4274a0b8982f07fd4ceb778787b10abe7dcc420c28",
    ),
    "lossy_inter_422_vertical_split_b32x64": (
        "animated_lossy_inter_422_vertical_split_b32x64.avif",
        "0eb45159064b3f1c30919d0062afa19056415f76be203278cd3ddd7174e8a798",
    ),
    "lossy_inter_420_deep_b32x64_64x64": (
        "animated_lossy_inter_420_deep_b32x64_64x64.avif",
        "a64648f0a78b83b679a0a09448688be51e4c933b84a0215b6af8a766b52e2e8b",
    ),
    "lossy_inter_420_topology_b32x64_64x64": (
        "animated_lossy_inter_420_topology_b32x64_64x64.avif",
        "71e64835c2d78a25599a5b195c9a819dba8b0f61ac63136a64af2932f7bdddd6",
    ),
    "lossy_inter_422_small_partitions_32x64": (
        "animated_lossy_inter_422_small_partitions_32x64.avif",
        "f150bd51312eee0a9e7dccc57b3d26fc08899c400d82565b839d704e98c6c029",
    ),
    "lossy_inter_422_qindex0_mode_select_64x64": (
        "animated_lossy_inter_422_qindex0_mode_select_64x64.avif",
        "062cec9bf86cfa7e5d2130baec72884dec70d96d9621578f91388612d21a1fab",
    ),
    "lossy_inter_422_tx_context_checker_b64x32": (
        "animated_lossy_inter_422_tx_context_checker_b64x32.avif",
        "38c94eed681628437dd11423d62573fee3b1325a749ffc9f74dda1301c6acd56",
    ),
    "motion_chroma_wide": ("animated_motion_chroma_wide.avif", "54ca9823b7b3f6f64af19d3b7ab7dbc0be0b9db6583d85fedcc310dd62d28311"),
    "tx64_root_split_inter_420_64x64": (
        "animated_tx64_root_split_inter_420_64x64.avif",
        "9f4450d4d9c7c2738d4f9f34eafb02100c5fb15b85ac121a057c3eef367f6d7c",
    ),
    "motion_temporal_window_left_512x128": (
        "animated_motion_temporal_window_left_512x128.avif",
        "29e856c6c8a117c764bb0a176ccc1d82bac982fdf7287eb461f827ebdc31b399",
    ),
    "motion_temporal_window_right_512x128": (
        "animated_motion_temporal_window_right_512x128.avif",
        "10d8e5514c1d8d9cad355f9a912b3e3075bf8bd661ee6af5ce44a9ce62d7cad2",
    ),
    "motion_temporal_mv_range_2048x128": (
        "animated_motion_temporal_mv_range_2048x128.avif",
        "294468c2498c8fa494bfcc2e701a944e90f15ec4b5f08eb64fa34f907e5a6a1a",
    ),
    "motion_temporal_mv_range_positive_x_2048x128": (
        "animated_motion_temporal_mv_range_positive_x_2048x128.avif",
        "f802a724db9dea26e0d71abb7d123c9338449e77008282874f58580c0b2163d1",
    ),
    "motion_temporal_mv_range_negative_y_128x2048": (
        "animated_motion_temporal_mv_range_negative_y_128x2048.avif",
        "e32c9ff32e87922be280d26fef6d8c92635e84d2918df7c23f3f8ec3087b051c",
    ),
    "motion_temporal_mv_range_positive_y_128x2048": (
        "animated_motion_temporal_mv_range_positive_y_128x2048.avif",
        "307a001ecd6e10c4b840a3d0e0b30b24a6a90ac1c8f5f5ea1b8d9e209b1c3333",
    ),
    "lossy_inter_i444_mode2_b32x32": (
        "animated_lossy_inter_i444_mode2_b32x32.avif",
        "43aac6364eebb113b128861f0e1c2cfb976295d7b5e9e226fa8f0eaa93704af4",
    ),
    "lossy_inter_420_mode_largest_16x64": (
        "animated_lossy_inter_420_mode_largest_16x64.avif",
        "4e2eba0851921d8629b8c3d1d1660652bf9b5d01e279737cbe7594e2e0a9ec40",
    ),
    "lossy_inter_420_mode_select_16x64": (
        "animated_lossy_inter_420_mode_select_16x64.avif",
        "6de7ecf1d5ed5eab61c60fab3899cdc328cf3416ddcc1338bc7823412e64ddd2",
    ),
    "lossy_inter_420_b16x64_mode_select_64x64": (
        "animated_lossy_inter_420_b16x64_mode_select_64x64.avif",
        "6241cd581ea930154ddf66554966a069d03ebf06c8d2a4880a4a0008398e1a4a",
    ),
    "lossy_inter_420_b16x64_reduced_transform_set_64x64": (
        "animated_lossy_inter_420_b16x64_reduced_transform_set_64x64.avif",
        "3a52bdb0573571d5b9d6c5f79cbf4f41816fa84e4ec052034a9e418b9797511e",
    ),
    "lossy_inter_420_b16x64_qindex0_mode_select_64x64": (
        "animated_lossy_inter_420_b16x64_qindex0_mode_select_64x64.avif",
        "f9a69337b2eab0d0e6623aa874fdfcf58c35ada2154e8377a595b401c487317c",
    ),
    "lossy_inter_monochrome_b16x64_qindex0_mode_select_64x64": (
        "animated_lossy_inter_monochrome_b16x64_qindex0_mode_select_64x64.avif",
        "972330af1b5c8bd691dae557ccb6e70532959c52009f5e840aca73989975bf3c",
    ),
    "lossy_inter_420_mode_largest_64x16": (
        "animated_lossy_inter_420_mode_largest_64x16.avif",
        "74d3b5b9b06e208175d4ac533c05de73eb37f46fce8cfc2de581116689d59468",
    ),
    "lossy_inter_420_b64x16_mode_largest_64x64": (
        "animated_lossy_inter_420_b64x16_mode_largest_64x64.avif",
        "35f16973d0bda15b2a2038102b3f646f2b04d6e07157933bd03b8b32a6452daf",
    ),
    "lossy_inter_420_b64x16_mode_select_64x64": (
        "animated_lossy_inter_420_b64x16_mode_select_64x64.avif",
        "84eab6d96f97deb02fca49e66602f9cb507d2281cde5d4b52b2a50c54cb0433a",
    ),
    "lossy_inter_420_b64x16_10bit_mode_select_64x64": (
        "animated_lossy_inter_420_b64x16_10bit_mode_select_64x64.avif",
        "0d85dc1e3e8958e7375c3dcddd048f93cc5bd7ad12f50f82f1299f1c482c1880",
    ),
    "lossy_inter_420_b64x16_12bit_mode_select_64x64": (
        "animated_lossy_inter_420_b64x16_12bit_mode_select_64x64.avif",
        "8583340a327c7feffa803fa3b0a80267a49b6fa21c062f9b19799e2a82bf1dcc",
    ),
    "lossy_inter_420_b8x8_mode_select_64x64": (
        "animated_lossy_inter_420_b8x8_mode_select_64x64.avif",
        "a4963f38d42c857cbacd6306ab5585c0e42455fd6c3725dfdffda27b7553e6f1",
    ),
    "lossy_inter_i444_b8x16_mode_select_64x64": (
        "animated_lossy_inter_i444_b8x16_mode_select_64x64.avif",
        "0187f7634e7ffb21019961fbb47091c3c2deaaccd88a04c043a7ffe1b05b8f93",
    ),
    "lossy_inter_i444_b16x64_mode_select_10bit_64x64": (
        "animated_lossy_inter_i444_b16x64_mode_select_10bit_64x64.avif",
        "86fdcab5cc8355ec8706ea40ea3e9f75e8ee05a5b54c3043bf7416417bfe1de6",
    ),
    "lossy_inter_420_b8x16_mixed_topology_64x64": (
        "animated_lossy_inter_420_b8x16_mixed_topology_64x64.avif",
        "07e1d146cf7a3c5f565fab5eddecd19ae53cca665a70faf6078e1b05baeebe12",
    ),
    "lossy_inter_420_single_extended_ref_128x128": (
        "animated_lossy_inter_420_single_extended_ref_128x128.avif",
        "6fe9058db08a06e520922eb2d63d2dc1111d842e1f8d26e023acd560fbdde271",
    ),
    "lossy_inter_i444_split_b16x32_mode2": (
        "animated_lossy_inter_i444_split_b16x32_mode2.avif",
        "ad7ce564a11440b91237e0dfffedbde1053bc663ae0e9bf0894f87e05e669bd0",
    ),
    "lossy_inter_i444_compound_split_b16x32_mode2": (
        "animated_lossy_inter_i444_compound_split_b16x32_mode2.avif",
        "bb43388411119ac10f7990ecb8523e8b62f85227b501e3402b962b5c2b624714",
    ),
    "lossy_inter_422_compound_refs_b64x32": (
        "animated_lossy_inter_422_compound_refs_b64x32.avif",
        "58179bf4eec6cd6a78dc575646dc3db0fa30b0d8c844f74a1dbf02694c785350",
    ),
    "lossy_inter_422_compound_direct_grid_b64x64": (
        "animated_lossy_inter_422_compound_direct_grid_b64x64.avif",
        "5eef65a8499170972eb9edf607844b39292721f4e53ebbcdc4d61e78f2d7c51e",
    ),
    "lossy_inter_422_mode_select_b64x64": (
        "animated_lossy_inter_422_mode_select_b64x64.avif",
        "eba71559d1bd1c34ebce8d3bd82ea3f9aea8935ac8f2749ed42784423a1126d6",
    ),
    "lossy_inter_422_direct_grid_mode_select_mixed_b128x64": (
        "animated_lossy_inter_422_direct_grid_mode_select_mixed_b128x64.avif",
        "7aee9a35ab56e6209dbc11b2feccd8d636edc4c6b7b448d128c66832d6da198e",
    ),
    "lossy_inter_444_direct_grid_mode_select_mixed_b128x64": (
        "animated_lossy_inter_444_direct_grid_mode_select_mixed_b128x64.avif",
        "8eb2916792a241372d03d5ccf1802549363ad61325c3ec3489b36752aaa7cfc2",
    ),
    "lossy_inter_i444_obmc_mixed_b16x32": (
        "animated_lossy_inter_i444_obmc_mixed_b16x32.avif",
        "71e737f7b196173d98ae6447925cd8f6f77d95ced3eaa80d924ad81febcb3743",
    ),
    "motion_large_420": ("animated_motion_large_420.avif", "5b9ea2b9d552e8ff44f2818a7ae2b73a9dbe1eda84ead9c24c9f8a957c6e546f"),
    "highdepth": ("10bit.avif", "3bf9f91da471749e7df639ba7945d4d94c1c3e3968c26f3619fbbcfc92790576"),
    "lo8_superres_sgr_inter_160x56": (
        "animated_lossy_inter_420_superres_sgr_8bit_160x56.avif",
        "c294163610d4a45852fe374e0345c878979bb81e5ea94596960ef64411180fd7",
    ),
    "lo8_i422_superres_sgr_inter_160x56": (
        "animated_lossy_inter_422_superres_sgr_8bit_160x56.avif",
        "a3f55297651b8659e9a76fda1276a2a6330151d802af1e72cdb659858cfcb1a1",
    ),
    "lo8_i444_superres_sgr_inter_160x56": (
        "animated_lossy_inter_444_superres_sgr_8bit_160x56.avif",
        "8c3323e3971c653b41a16e6c12c6a12cf512c1247d8a399d97e2f1be2b544716",
    ),
    "hi10_superres_sgr_inter_160x56": (
        "animated_lossy_inter_420_superres_sgr_10bit_160x56.avif",
        "016e4a8433002b60899744fba6f26a7c10af82c192f65b4c5c19233b15c3cb11",
    ),
    "hi10_i444_superres_sgr_inter_160x56": (
        "animated_lossy_inter_444_superres_sgr_10bit_160x56.avif",
        "d0f13c712995e0464efb51575ca353aabd26c8ff6e3001c415fd98f239c8221c",
    ),
    "hi10_i422_superres_sgr_inter_160x56": (
        "animated_lossy_inter_422_superres_sgr_10bit_160x56.avif",
        "60748f33355d8f8b54f48942e319c1791e75c75c94da55a1568fd6679edf578a",
    ),
    "hi12_i422_superres_inter_160x56": (
        "animated_lossy_inter_422_superres_12bit_160x56.avif",
        "565ad0b866eb0fd25b37c5b23e0c3dba8ab791312e782cf4d58fd80a92684db9",
    ),
    "hi10_superres_select_inter_160x56": (
        "animated_lossy_inter_420_superres_select_10bit_160x56.avif",
        "04e31a3ac36c25ef77061a2ed09b79fd8fb885a5ff14d6ab3a82e2d32ad3b02e",
    ),
    "short_refs_q100_half_range": ("short_refs_q100_half_range.avif", "eeaba276228d2e211db60b58c0c99207a8a27ac48df49e62419c41333adb09fc"),
    "short_refs_q100_near_past": ("short_refs_q100_near_past.avif", "c1f16b78ef72c5c3bbb6df1e35b52f1550777e189613d239415fdb32b40329bb"),
    "short_refs_q100_past_aliased": ("short_refs_q100_past_aliased.avif", "83bd0b42082ba4d9f436a731175a4a2c44a4f6f7cab58e86b98a7f5b7a1fe66e"),
    "short_refs_q100_past_distinct": ("short_refs_q100_past_distinct.avif", "84ebe172290b57825023cea43054b00e853554eb5e0a2e1eda6c84391ffb9ee4"),
    "short_refs_q100_past_distinct_images": ("short_refs_q100_past_distinct_images.avif", "6d83e0858fa9dcf5b6177629daa9b8ed68799978c738dd340c2d9ba7d1e57da0"),
    "short_refs_q100_reordered_future_images": ("short_refs_q100_reordered_future_images.avif", "fb4ca6823a5b981d75b93f8c751ce0ef874b99499fef4b01e9fa8f985ff2594f"),
    "short_refs_q100_reordered_past_images": ("short_refs_q100_reordered_past_images.avif", "32dd31a98e26951f62bb57e216ba972c27d56f1240b2192c7228935426e9fd55"),
    "short_refs_q100_tied_aliased": ("short_refs_q100_tied_aliased.avif", "39637c228398cb9db43c0d926ea1d212addac2d343e065bae7c1b12ea25c3397"),
    "short_refs_q100_tied_distinct": ("short_refs_q100_tied_distinct.avif", "af9b26092a8d83d15fc247905d2a15043b9033f79c5d2ed63ff870e347af2121"),
    "short_refs_q100_tied_previous_image": ("short_refs_q100_tied_previous_image.avif", "45c095edeaecfd52fe80e1bc2f68c371f56a87861bb88433430669960b3eb785"),
    "short_refs_q100_wrapped_future": ("short_refs_q100_wrapped_future.avif", "ca119400574aa4585350f356a5c1b1a7ea2733e9de36a9e52dedd1682400e4fd"),
    "short_refs_q100_wrapped_near_future": ("short_refs_q100_wrapped_near_future.avif", "a8376fac449e9259ab61b8ffeff28a3c3e6283672195a3b4c78af3b6f2361edd"),
    "short_refs_q99_half_range": ("short_refs_q99_half_range.avif", "347a10523ea08ac8732941e47dc7d6137f0708727bc16c5d000c0e1dd00949f6"),
    "short_refs_q99_near_past": ("short_refs_q99_near_past.avif", "252a58c2318bf863eabf33bc99bdd2dea2e10d5ffc471cdbf89d388d379c245b"),
    "short_refs_q99_past_aliased": ("short_refs_q99_past_aliased.avif", "e4a49eb1b4d394383aaf48a09bf8c2c5b7cd1e0c3d2f2a54d8ebe2cf0a005254"),
    "short_refs_q99_past_distinct": ("short_refs_q99_past_distinct.avif", "c8641325a2d6222ce53e8e286e2cbfbe7dc70ade0a3a3b25415db12737d28b4e"),
    "short_refs_q99_past_distinct_images": ("short_refs_q99_past_distinct_images.avif", "5569badd39681c8f415f1ee3e1a9088001c70277736b55da6ef39540bb5ee771"),
    "short_refs_q99_reordered_future_images": ("short_refs_q99_reordered_future_images.avif", "2b972c54344d9e96c5a348f50d75d2499c77c639d69535070d9af734341c38ab"),
    "short_refs_q99_reordered_past_images": ("short_refs_q99_reordered_past_images.avif", "8e73a4a1d02c3c576ee00d24e06e2ee2041b6fa189b74481e95e2cd6eb684c6f"),
    "short_refs_q99_tied_aliased": ("short_refs_q99_tied_aliased.avif", "d37d2d898bc7e24ab97d0ad4743121fcb2beb3e4f04c3648a904982699424ddb"),
    "short_refs_q99_tied_distinct": ("short_refs_q99_tied_distinct.avif", "2189bdfd0c18ad2c7c7eeb6635d85926b45684eac224c95d7983842cb769813d"),
    "short_refs_q99_tied_previous_image": ("short_refs_q99_tied_previous_image.avif", "b8ea81096f216f31f03c3e2a75078bd54f956b148ca1d113a527a9340458b40f"),
    "short_refs_q99_wrapped_future": ("short_refs_q99_wrapped_future.avif", "3d7ed8b94ad3663aa97567716834caf9bcf5b9131260da76ad2897e4e614d09c"),
    "short_refs_q99_wrapped_near_future": ("short_refs_q99_wrapped_near_future.avif", "05917a4b2de698d9a117ce7c2a5e32116066c09f045d96313956aceb697c701f"),
    "animated_lossy_wide_i420_10bit_mode2_deep16_b128x128": ("animated_lossy_wide_i420_10bit_mode2_deep16_b128x128.avif", "ce093eb35970f1070ce151241c49a302d1143e6d6db75af1e4cd93745ab6800a"),
    "animated_lossy_wide_i420_10bit_mode2_mixed_b128x128": ("animated_lossy_wide_i420_10bit_mode2_mixed_b128x128.avif", "61b9891a58b657ba3090c7a2a2f516a0a16e7eb0c9277163f59eaa54bcae6b67"),
    "animated_lossy_wide_i420_10bit_mode2_split32_b128x128": ("animated_lossy_wide_i420_10bit_mode2_split32_b128x128.avif", "089c1bfc9d00dff16f6d2b9f5355680f74d5b97c04f71e563db2c9f41ef57c81"),
    "animated_lossy_wide_i420_12bit_mode2_deep16_b128x128": ("animated_lossy_wide_i420_12bit_mode2_deep16_b128x128.avif", "282b01da5f5d88b613d9887a21884562024940f5ea63d1c7badec6266d1270f3"),
    "animated_lossy_wide_i420_12bit_mode2_mixed_b128x128": ("animated_lossy_wide_i420_12bit_mode2_mixed_b128x128.avif", "4095136d5ed94192930a33ce81c4e2cab40c8cf8386aa183e859666338683488"),
    "animated_lossy_wide_i420_12bit_mode2_split32_b128x128": ("animated_lossy_wide_i420_12bit_mode2_split32_b128x128.avif", "7ad161fe3ad29199f2d910959ac58f03f0a052980fbcdfcdbd1860f9c6e629f0"),
    "animated_lossy_wide_i420_mode2_deep16_b128x128": ("animated_lossy_wide_i420_mode2_deep16_b128x128.avif", "6dc4388ea8eeaa7b0dbbed2ad2eff580d916f891f8cb4e3b6c444b4bd10ec9e4"),
    "animated_lossy_wide_i420_mode2_deep16_color_b128x128": ("animated_lossy_wide_i420_mode2_deep16_color_b128x128.avif", "09ecf856d078b089112b3ed1e28e837403b4dc7365124a7a2a7913fc68ba82cc"),
    "animated_lossy_wide_i420_mode2_mixed_b128x128": ("animated_lossy_wide_i420_mode2_mixed_b128x128.avif", "7a7b335faee120f33536f653f31992a604056f35f2d90c14fc587f63ae0ab488"),
    "animated_lossy_wide_i420_mode2_mixed_color_b128x128": ("animated_lossy_wide_i420_mode2_mixed_color_b128x128.avif", "81b6f280034503dde23a4f9a113ffedb3a8ac9fb07f5825ffe9ec58556a43761"),
    "animated_lossy_wide_i420_mode2_split32_b128x128": ("animated_lossy_wide_i420_mode2_split32_b128x128.avif", "c207f562d7652dc445e86e53bdf8ee4507d122d78edf04fbaa555d07606f47ed"),
    "animated_lossy_wide_i420_mode2_split32_color_b128x128": ("animated_lossy_wide_i420_mode2_split32_color_b128x128.avif", "cf6d3c6efc2a844e68f58dcef57e22e1215c340dc8f82b35cbcee056cbb53e72"),
    "animated_lossy_wide_i422_10bit_mode2_deep16_b128x128": ("animated_lossy_wide_i422_10bit_mode2_deep16_b128x128.avif", "240c9f9f7e053ff2d45ba2829547502167230f2ecd31739b37c3b62fb74d07ff"),
    "animated_lossy_wide_i422_10bit_mode2_mixed_b128x128": ("animated_lossy_wide_i422_10bit_mode2_mixed_b128x128.avif", "1de536b8a610005830a9f036cd3ee35529ba1ecb1224953f4417c1ba8ee3fe95"),
    "animated_lossy_wide_i422_10bit_mode2_split32_b128x128": ("animated_lossy_wide_i422_10bit_mode2_split32_b128x128.avif", "ea85fb65343171ab15c81a7bf8a98852679c1097d7f81d0f85fd947720de88a6"),
    "animated_lossy_wide_i422_12bit_mode2_deep16_b128x128": ("animated_lossy_wide_i422_12bit_mode2_deep16_b128x128.avif", "fcb9a3fe363293799510fb34c9bd4999d8cc2ff03dcbeb179796b04cf39e6200"),
    "animated_lossy_wide_i422_12bit_mode2_mixed_b128x128": ("animated_lossy_wide_i422_12bit_mode2_mixed_b128x128.avif", "02b6878702cd871581b6ddc28f9b3cb960bf0c9671daf79e20b208df2bde044a"),
    "animated_lossy_wide_i422_12bit_mode2_split32_b128x128": ("animated_lossy_wide_i422_12bit_mode2_split32_b128x128.avif", "2fa5567f2fdd9f467ebabba6d9b37e373da54ba67d082d0b7afa8ce2e7e37223"),
    "animated_lossy_wide_i422_mode2_deep16_b128x128": ("animated_lossy_wide_i422_mode2_deep16_b128x128.avif", "a251e0e94f1005c3d566a7dcb89a8b5223d86d54ed035d17fd372d48e981fd33"),
    "animated_lossy_wide_i422_mode2_deep16_color_b128x128": ("animated_lossy_wide_i422_mode2_deep16_color_b128x128.avif", "65769fa6ce4920d390e415e3ae40aba7cbe24ec5299ade14517f6662ad7f3b2e"),
    "animated_lossy_wide_i422_mode2_mixed_b128x128": ("animated_lossy_wide_i422_mode2_mixed_b128x128.avif", "3384cb410730928ddc57d09f89728e93f901feee013ad17c8f6c8729542cc86b"),
    "animated_lossy_wide_i422_mode2_mixed_color_b128x128": ("animated_lossy_wide_i422_mode2_mixed_color_b128x128.avif", "8cedea55c079f444f48876de9d23b7918c59a262cc64d2d41aaa2a073d65c168"),
    "animated_lossy_wide_i422_mode2_split32_b128x128": ("animated_lossy_wide_i422_mode2_split32_b128x128.avif", "66aaf55006ce3c37b92329ec1b3290d2dc676b6d3c75f1a8cf1879bbe7a7fe82"),
    "animated_lossy_wide_i422_mode2_split32_color_b128x128": ("animated_lossy_wide_i422_mode2_split32_color_b128x128.avif", "3955d8b4618ce2de6993e7a1fe4dd4cd45a24c8adbbb75f8512f94c84b6bc9bc"),
    "animated_lossy_wide_i444_10bit_mode2_deep16_b128x128": ("animated_lossy_wide_i444_10bit_mode2_deep16_b128x128.avif", "dfa469bff5617b7cc27384c281e5a78082979e40a2208e80ca90f43fe96ef789"),
    "animated_lossy_wide_i444_10bit_mode2_mixed_b128x128": ("animated_lossy_wide_i444_10bit_mode2_mixed_b128x128.avif", "143452baa7be593e89e7f31b6266ddfb54166bee4762be2b1a7100bd235448f2"),
    "animated_lossy_wide_i444_10bit_mode2_split32_b128x128": ("animated_lossy_wide_i444_10bit_mode2_split32_b128x128.avif", "ac36ee94eb7cbe79e8677791fef5b12e3656232c3cf9e3fa4eda23494c30dde5"),
    "animated_lossy_wide_i444_12bit_mode2_deep16_b128x128": ("animated_lossy_wide_i444_12bit_mode2_deep16_b128x128.avif", "5d0d6645530501e99fd57398de0c51fbc1d8e226fb660b153302e53cc481d4ac"),
    "animated_lossy_wide_i444_12bit_mode2_mixed_b128x128": ("animated_lossy_wide_i444_12bit_mode2_mixed_b128x128.avif", "11407ec5bb0765818b543bc70f69f04b8b802ad7279a5b74541aa732304b9717"),
    "animated_lossy_wide_i444_12bit_mode2_split32_b128x128": ("animated_lossy_wide_i444_12bit_mode2_split32_b128x128.avif", "38b9aaa05374897ad076d38b53fd9765437a52058365d4571703836e128591c6"),
    "animated_lossy_wide_i400_10bit_mode2_deep16_b128x128": ("animated_lossy_wide_i400_10bit_mode2_deep16_b128x128.avif", "cb17bf443b3d4a623a7165bf797cf02fdebb5056ce54f3ecf4824cee98d08b75"),
    "animated_lossy_wide_i400_10bit_mode2_mixed_128x132": ("animated_lossy_wide_i400_10bit_mode2_mixed_128x132.avif", "348d2ae12759f9a9fe2778ee2efd978a82e99173aa3a6c4763ed3de2a6b50c62"),
    "animated_lossy_wide_i400_10bit_mode2_mixed_132x128": ("animated_lossy_wide_i400_10bit_mode2_mixed_132x128.avif", "b4cafd28775d59196645a837bb978a3690d7f5a660b38282b904226b58b4238d"),
    "animated_lossy_wide_i400_10bit_mode2_mixed_252x256": ("animated_lossy_wide_i400_10bit_mode2_mixed_252x256.avif", "9e9fc446aa79ea3767000e1e798b290f6e163c47e62c3e46fe1ab74b4a7619bb"),
    "animated_lossy_wide_i400_10bit_mode2_mixed_256x252": ("animated_lossy_wide_i400_10bit_mode2_mixed_256x252.avif", "6dac20ec493d4efc699473aeb5bd438e17af5261dda8be52eab71ea053a6fae7"),
    "animated_lossy_wide_i400_10bit_mode2_mixed_b128x128": ("animated_lossy_wide_i400_10bit_mode2_mixed_b128x128.avif", "bc13f8c23e3cd4c6bf4a1ec23f53f407b4ebb45aba9fd277f3557c3173cb3150"),
    "animated_lossy_wide_i400_10bit_mode2_split32_b128x128": ("animated_lossy_wide_i400_10bit_mode2_split32_b128x128.avif", "d15cd04681be4e6c7605a1488252fb276558086cfaa835a287bf2959731b0288"),
    "animated_lossy_wide_i400_12bit_mode2_deep16_b128x128": ("animated_lossy_wide_i400_12bit_mode2_deep16_b128x128.avif", "7c2868e05c761c4f5aa356eb61021bb309035d5a0adc81317c9c46f387837a5f"),
    "animated_lossy_wide_i400_12bit_mode2_mixed_128x132": ("animated_lossy_wide_i400_12bit_mode2_mixed_128x132.avif", "d28a062e3ce117594f296c051289f052b072883c6ca5160d96d561b6d432e159"),
    "animated_lossy_wide_i400_12bit_mode2_mixed_132x128": ("animated_lossy_wide_i400_12bit_mode2_mixed_132x128.avif", "94f35e81abcf02ab24e1e5156e5c6844ab3f7af6df0bd0e9eac1e84745461402"),
    "animated_lossy_wide_i400_12bit_mode2_mixed_252x256": ("animated_lossy_wide_i400_12bit_mode2_mixed_252x256.avif", "87f954a0ea31e07eb6f396c3f21cb1573a1c6395534cf636bf79a5b6f4144887"),
    "animated_lossy_wide_i400_12bit_mode2_mixed_256x252": ("animated_lossy_wide_i400_12bit_mode2_mixed_256x252.avif", "e784414f0f68cbcc113703de367828f7579a380bb59c6826b54fffd1f2f9138f"),
    "animated_lossy_wide_i400_12bit_mode2_mixed_b128x128": ("animated_lossy_wide_i400_12bit_mode2_mixed_b128x128.avif", "637f3f3d70346c5ea629af8a0aab5e48e4e3d5d8d66b251f9abb7d1efb80f690"),
    "animated_lossy_wide_i400_12bit_mode2_split32_b128x128": ("animated_lossy_wide_i400_12bit_mode2_split32_b128x128.avif", "e3d377a55d86099f6f081dd4f956729dd0e831a810c375cefff2b7a19c916cf8"),
    "animated_lossy_wide_i400_8bit_mode2_deep16_b128x128": ("animated_lossy_wide_i400_8bit_mode2_deep16_b128x128.avif", "902c108ea984964ec6a3ca701c3636fda239b255529d26074ff6cde88814ce67"),
    "animated_lossy_wide_i400_8bit_mode2_mixed_128x132": ("animated_lossy_wide_i400_8bit_mode2_mixed_128x132.avif", "a97d0523087de28964dc8c45502ecabeca833605936332db897c695feb9bf0f4"),
    "animated_lossy_wide_i400_8bit_mode2_mixed_132x128": ("animated_lossy_wide_i400_8bit_mode2_mixed_132x128.avif", "fccebfffe74243614b8cb99522a31d97d6110513ca9fc36d13ed666bb5eaa6a4"),
    "animated_lossy_wide_i400_8bit_mode2_mixed_252x256": ("animated_lossy_wide_i400_8bit_mode2_mixed_252x256.avif", "8a130511632143469866dedbd41fea5abbd998784cb127ff5b429f8979cfa9a7"),
    "animated_lossy_wide_i400_8bit_mode2_mixed_256x252": ("animated_lossy_wide_i400_8bit_mode2_mixed_256x252.avif", "11d43ae00445eea9b4779cf7439ec4ab7ba64756a77b6b50276f9f59208fb9fa"),
    "animated_lossy_wide_i400_8bit_mode2_mixed_b128x128": ("animated_lossy_wide_i400_8bit_mode2_mixed_b128x128.avif", "f9d4cbc2f1f176f6e373ce840f0c0e3d3d3ceb61357345e8c105abec3c24ccc0"),
    "animated_lossy_wide_i400_8bit_mode2_split32_b128x128": ("animated_lossy_wide_i400_8bit_mode2_split32_b128x128.avif", "81021b030555153e49c564baeee9f1f41e876af43fab2c05a8a3d329e5d5f153"),
    "animated_lossy_inter_420_b16x4_q45_64x16": ("animated_lossy_inter_420_b16x4_q45_64x16.avif", "dbaaf03acd8b5b1819710870caaa9108043a1734530e28a656aec34b9f11481a"),
    "animated_lossy_inter_420_b16x4_q70_64x16": ("animated_lossy_inter_420_b16x4_q70_64x16.avif", "fa412144d15f941caf49416afe6af1c6b33ad8f9326748a712ea9b780a548286"),
    "animated_lossy_inter_420_b16x4_q99_64x16": ("animated_lossy_inter_420_b16x4_q99_64x16.avif", "d1aee7bb97195edb3ec8ea0cbc4d3f0a998b4440d8d3e12880c5aae4fee2e3f0"),
    "animated_lossy_inter_420_b4x16_q45_16x64": ("animated_lossy_inter_420_b4x16_q45_16x64.avif", "208092164efb3eee1899376810507adf24b57334db1a260de0480fcc0c784933"),
    "animated_lossy_inter_420_b4x16_q70_16x64": ("animated_lossy_inter_420_b4x16_q70_16x64.avif", "ab60cec38de20a78b945fcdd3267c47adbd0b6a08f9820b96ba46b6c462740d0"),
    "animated_lossy_inter_420_b4x16_q99_16x64": ("animated_lossy_inter_420_b4x16_q99_16x64.avif", "fb3d30b4828faa053082aac576e08c88de83554c38fa0fccdc2bfa06cad63f87"),
    "animated_lossy_inter_422_b16x4_q45_64x16": ("animated_lossy_inter_422_b16x4_q45_64x16.avif", "a2bc965afb5a01848c38eba790da0af86d143747c87a4aa0b0a8200f16e03198"),
    "animated_lossy_inter_422_b16x4_q70_64x16": ("animated_lossy_inter_422_b16x4_q70_64x16.avif", "f8c4975e11d54f4e013a2700974fd5c1b68c52ea0229eba0e821e99f3d27ba67"),
    "animated_lossy_inter_422_b16x4_q99_64x16": ("animated_lossy_inter_422_b16x4_q99_64x16.avif", "bb12465478e20e84aecb5dd8247a27d5b221de0e47eaf622d56140d89bdde9ba"),
    "animated_lossy_inter_444_b16x4_q45_64x16": ("animated_lossy_inter_444_b16x4_q45_64x16.avif", "73f17e705c08f744fa9d1c14f108a57d8d0cf3ba857b85034dde641fc1b5b681"),
    "animated_lossy_inter_444_b16x4_q70_64x16": ("animated_lossy_inter_444_b16x4_q70_64x16.avif", "7c1c750472a71e34e4a1f8dffa57a679e2bb6255ea57f0b231726f8e4a9f9f13"),
    "animated_lossy_inter_444_b16x4_q99_64x16": ("animated_lossy_inter_444_b16x4_q99_64x16.avif", "33d9c7e75e158d7c2f86ff533af393c8bfb072151d6fe067b8a8226122938a1e"),
    "animated_lossy_inter_444_b4x16_q45_16x64": ("animated_lossy_inter_444_b4x16_q45_16x64.avif", "2a10f8d1813a971192b03f2e60e057558829973196ca880ba72c8f14d0a022cb"),
    "animated_lossy_inter_444_b4x16_q70_16x64": ("animated_lossy_inter_444_b4x16_q70_16x64.avif", "1b9c0fda442408847681d20df597c621693e8f80ec278d93e70bdc9b36b96cfc"),
    "animated_lossy_inter_444_b4x16_q99_16x64": ("animated_lossy_inter_444_b4x16_q99_16x64.avif", "cf3de7722a93d3f5214907e9f48ab8366ab729cf1b20ecf1beeac7d5fe1f4676"),
    "animated_lossy_inter_420_10bit_b16x4_q99_64x16": ("animated_lossy_inter_420_10bit_b16x4_q99_64x16.avif", "f5f4844b35b01041579c7df6a8da5920e6250ee642cdb7ac1984f9f3a902654a"),
    "animated_lossy_inter_420_10bit_b4x16_q99_16x64": ("animated_lossy_inter_420_10bit_b4x16_q99_16x64.avif", "c499ff3669a714514680ef5a5fa966f65d7236c859a861d976afbc9e5ff995c7"),
    "animated_lossy_inter_420_12bit_b16x4_q99_64x16": ("animated_lossy_inter_420_12bit_b16x4_q99_64x16.avif", "2ad79b720118af8c51d9ce90af2ff66d924a91e37ba248177deb67a480aee653"),
    "animated_lossy_inter_420_12bit_b4x16_q99_16x64": ("animated_lossy_inter_420_12bit_b4x16_q99_16x64.avif", "fab0e71887d8e8130e729d9449bd42fe951b045170a66f72db5dfa31eb54dca6"),
    "animated_lossy_inter_422_10bit_b16x4_q99_64x16": ("animated_lossy_inter_422_10bit_b16x4_q99_64x16.avif", "1bedf2607ded647945fad966e627907adeb82ab8b139e6718b17720ebe4ad861"),
    "animated_lossy_inter_422_12bit_b16x4_q99_64x16": ("animated_lossy_inter_422_12bit_b16x4_q99_64x16.avif", "fc881ef2ad779a05222bc5d89e23b7bde3252507803c08413dbc1a9187d71736"),
    "animated_lossy_inter_444_10bit_b16x4_q99_64x16": ("animated_lossy_inter_444_10bit_b16x4_q99_64x16.avif", "c63fa843621614dbadc1c3fb14dd68bd6baf5df7cb4f146cc5bf865b956002b9"),
    "animated_lossy_inter_444_10bit_b4x16_q99_16x64": ("animated_lossy_inter_444_10bit_b4x16_q99_16x64.avif", "4931f1c53fcadc57f4d025a6e2f1dabfcad35c5c7620cd55d46144b63f358729"),
    "animated_lossy_inter_444_12bit_b16x4_q99_64x16": ("animated_lossy_inter_444_12bit_b16x4_q99_64x16.avif", "5ae3dd60624e55f3c322842417ce725b1d5defa613d2582b9af1f14d31fffdb3"),
    "animated_lossy_inter_444_12bit_b4x16_q99_16x64": ("animated_lossy_inter_444_12bit_b4x16_q99_16x64.avif", "70ba8dac040e6e7d1ac879a17934d02579df5d3ce7d814488ff1ee3de7d51172"),
    "animated_lossy_inter_420_10bit_b4x16_fine_q99_16x64": ("animated_lossy_inter_420_10bit_b4x16_fine_q99_16x64.avif", "809ba8beeafd050f67a34ea38cd7a0f169fb7b1493b2722e6ba88c36bdf3c49b"),
    "animated_lossy_inter_420_12bit_b4x16_fine_q99_16x64": ("animated_lossy_inter_420_12bit_b4x16_fine_q99_16x64.avif", "cb9b98d09e00807bca2759fa7bb5c44e9e59e21651fbc6be3a01c5850bb060fd"),
    "animated_lossy_inter_444_10bit_b4x16_fine_q99_16x64": ("animated_lossy_inter_444_10bit_b4x16_fine_q99_16x64.avif", "44d119079318c736357643fce2558ac6d2d04b443cf793edd6dc6f6e406c36db"),
    "animated_lossy_inter_444_12bit_b4x16_fine_q99_16x64": ("animated_lossy_inter_444_12bit_b4x16_fine_q99_16x64.avif", "2509c8349c1af9840afb113c614d052f6b3763bed8fe1a911f86a95976730dc1"),
    "animated_lossy_inter_420_10bit_fine_q99_cdf_frozen_16x64": ("animated_lossy_inter_420_10bit_fine_q99_cdf_frozen_16x64.avif", "e1c17a95082ab9fb703983f024f7ee2fd1c79050973e4c7448a7547b690bd2e6"),
    "animated_lossy_inter_420_12bit_fine_q99_cdf_frozen_16x64": ("animated_lossy_inter_420_12bit_fine_q99_cdf_frozen_16x64.avif", "bbbba21945dc3814f2d4ca4b73c3b57831b6c3231fc2988f8c0a9698089ccdbf"),
    "animated_lossy_inter_420_8bit_fine_q99_cdf_frozen_16x64": ("animated_lossy_inter_420_8bit_fine_q99_cdf_frozen_16x64.avif", "8f91043054786db0e74e777fdc2bd6b54bc0051168104d8e70cc54e452d407ba"),
    "animated_lossy_inter_420_8bit_fine_q99_cdf_selective_16x64": ("animated_lossy_inter_420_8bit_fine_q99_cdf_selective_16x64.avif", "0b195ed71bbd49d8691b1b1e56fd665cd6732bd1772163417f0cb901fcde23b7"),
    "animated_lossy_inter_422_10bit_fine_q99_cdf_frozen_64x16": ("animated_lossy_inter_422_10bit_fine_q99_cdf_frozen_64x16.avif", "922add08d9c3a59c2a114ff32daa76b1cc64f8857c1f97b40a60f33edb19cd21"),
    "animated_lossy_inter_422_10bit_fine_q99_cdf_selective_64x16": ("animated_lossy_inter_422_10bit_fine_q99_cdf_selective_64x16.avif", "8f4d106538db9a923a82735f56996141c0d79dc0f7e724ea6a7594273944c2a5"),
    "animated_lossy_inter_422_12bit_fine_q99_cdf_frozen_64x16": ("animated_lossy_inter_422_12bit_fine_q99_cdf_frozen_64x16.avif", "2816cd96523a4ed9f4a106d733972266751f18eb1d89856f48ca932d6f133aad"),
    "animated_lossy_inter_422_8bit_fine_q99_cdf_frozen_64x16": ("animated_lossy_inter_422_8bit_fine_q99_cdf_frozen_64x16.avif", "5f983e0773d97812427ce340b9d06a14cff7037a045feb9cb37ec29e63e03ee2"),
    "animated_lossy_inter_444_10bit_fine_q99_cdf_frozen_16x64": ("animated_lossy_inter_444_10bit_fine_q99_cdf_frozen_16x64.avif", "7d7f03bd17e30b7f1bfc1bc2229b9c38b18ec8921977596eca5040b45fb4fe6c"),
    "animated_lossy_inter_444_12bit_fine_q99_cdf_frozen_16x64": ("animated_lossy_inter_444_12bit_fine_q99_cdf_frozen_16x64.avif", "eb3a6a9f4165924af98cfdef3a1611a86038ac2f5808049d65f2bfba8168ef03"),
    "animated_lossy_inter_444_12bit_fine_q99_cdf_selective_16x64": ("animated_lossy_inter_444_12bit_fine_q99_cdf_selective_16x64.avif", "a8a01649a805d6be1d249b3c5523f5d8cc3288830570e9cf2d8e03b6c997e08b"),
    "animated_lossy_inter_444_8bit_fine_q99_cdf_frozen_16x64": ("animated_lossy_inter_444_8bit_fine_q99_cdf_frozen_16x64.avif", "2aa825afb1860041b9ee6f3b2879fa072588c4167e52c8beac98a0620317b9f0"),
    "animated_lossy_inter_400_10bit_b32x8_fine_q70_64x32": ("animated_lossy_inter_400_10bit_b32x8_fine_q70_64x32.avif", "936f5702d16d93e8b091aa7f64ab89dd349f4e250f4bd3bc7d69f179eaf97042"),
    "animated_lossy_inter_400_10bit_b32x8_fine_q99_64x32": ("animated_lossy_inter_400_10bit_b32x8_fine_q99_64x32.avif", "9cfcd227a4e014bdbd681620d21da95bfbb79c8f00148e93176f2cc9b6e388e3"),
    "animated_lossy_inter_400_10bit_b8x32_fine_q70_32x64": ("animated_lossy_inter_400_10bit_b8x32_fine_q70_32x64.avif", "d49067d191e2148f4b0d94dc4b50052aa3683e4d6f07ae56a3e9aa7a90b3a236"),
    "animated_lossy_inter_400_10bit_b8x32_fine_q99_32x64": ("animated_lossy_inter_400_10bit_b8x32_fine_q99_32x64.avif", "54b75f5ff4f9446e28d5871cfdff047d8cb5dbf5163254237da785cf47163bc7"),
    "animated_lossy_inter_400_12bit_b32x8_fine_q70_64x32": ("animated_lossy_inter_400_12bit_b32x8_fine_q70_64x32.avif", "e8ad9ca967b958299ef4198ae2b430b6ff350f1deee51160ad70ec7889662145"),
    "animated_lossy_inter_400_12bit_b32x8_fine_q99_64x32": ("animated_lossy_inter_400_12bit_b32x8_fine_q99_64x32.avif", "7b6bd9fe5630455fff39da47ca1869bd8c9a00f31d3aeda1135f850f4e48ee93"),
    "animated_lossy_inter_400_12bit_b8x32_fine_q70_32x64": ("animated_lossy_inter_400_12bit_b8x32_fine_q70_32x64.avif", "3fdca072f9ea166dd1032fbbf35dad59f3e04663077583f194b7079ef9cf2b27"),
    "animated_lossy_inter_400_12bit_b8x32_fine_q99_32x64": ("animated_lossy_inter_400_12bit_b8x32_fine_q99_32x64.avif", "07ff6adfac88fea007223ec7493c5707adabc4597537fd7ffae240c5204d621f"),
    "animated_lossy_inter_400_8bit_b32x8_fine_q70_64x32": ("animated_lossy_inter_400_8bit_b32x8_fine_q70_64x32.avif", "6499e9c83b5dbe6d6e7e1d029b700c4cee72e6c7d27b45950c39e733981fc06b"),
    "animated_lossy_inter_400_8bit_b32x8_fine_q99_64x32": ("animated_lossy_inter_400_8bit_b32x8_fine_q99_64x32.avif", "beb888e6e4a654b2698b4490fa773c964553aca420dc3c45184ea54f0a62d444"),
    "animated_lossy_inter_400_8bit_b8x32_fine_q70_32x64": ("animated_lossy_inter_400_8bit_b8x32_fine_q70_32x64.avif", "4f5c62c9046b1de7cd743437af31b4faea91fb684b4c95ec9a5d6f37e0a3d8b7"),
    "animated_lossy_inter_400_8bit_b8x32_fine_q99_32x64": ("animated_lossy_inter_400_8bit_b8x32_fine_q99_32x64.avif", "de726377979f411e50489c1c80597ff98b568dcc16e63bc39c21a8c5e92b16a2"),
    "animated_lossy_inter_420_10bit_b32x8_fine_q70_64x32": ("animated_lossy_inter_420_10bit_b32x8_fine_q70_64x32.avif", "22af699de04142c20333811aecf1441c20788c06dd86f8a9184e5d480546a66d"),
    "animated_lossy_inter_420_10bit_b32x8_fine_q99_64x32": ("animated_lossy_inter_420_10bit_b32x8_fine_q99_64x32.avif", "c5f75a5ca0ae4bc44a9bc49012a1d6727c8bac886b4ebd40c5eba22aa04c6723"),
    "animated_lossy_inter_420_10bit_b8x32_fine_q70_32x64": ("animated_lossy_inter_420_10bit_b8x32_fine_q70_32x64.avif", "c2725c659ae60b271b7ab35c87fd564c92b5eb76e17394c8774f8fa74880c5ec"),
    "animated_lossy_inter_420_10bit_b8x32_fine_q99_32x64": ("animated_lossy_inter_420_10bit_b8x32_fine_q99_32x64.avif", "dec3100d93b7af1216048bb87e6fb70b813362eb6cbc223fe2bf716458c31436"),
    "animated_lossy_inter_420_12bit_b32x8_fine_q70_64x32": ("animated_lossy_inter_420_12bit_b32x8_fine_q70_64x32.avif", "081528b3dc6aefe10351e51c81356634634261858994babca18e4814ef4f0d62"),
    "animated_lossy_inter_420_12bit_b32x8_fine_q99_64x32": ("animated_lossy_inter_420_12bit_b32x8_fine_q99_64x32.avif", "46c8f5b31a3f4e9f03c153c9a533204b7ba811ec45f8b9b2d22ca8e2b59a2cef"),
    "animated_lossy_inter_420_12bit_b8x32_fine_q70_32x64": ("animated_lossy_inter_420_12bit_b8x32_fine_q70_32x64.avif", "e6107b46f0b7ad9c7fae023c3fc2028b15d5a94f059482f4744c0a6ae3959b8a"),
    "animated_lossy_inter_420_12bit_b8x32_fine_q99_32x64": ("animated_lossy_inter_420_12bit_b8x32_fine_q99_32x64.avif", "b1d63965da624bf5c03ae6a638c82bff3aaa9f2c73ead3996a025797f4d1d0b6"),
    "animated_lossy_inter_420_8bit_b32x8_fine_q70_64x32": ("animated_lossy_inter_420_8bit_b32x8_fine_q70_64x32.avif", "d329bfe0ab5f9a494079e82e777880a55842bb706e8e6fb620453eb1562e2647"),
    "animated_lossy_inter_420_8bit_b32x8_fine_q99_64x32": ("animated_lossy_inter_420_8bit_b32x8_fine_q99_64x32.avif", "ace7208719b374448050df467c0290b95a6ffbaae3ac54b06ed05811c02a1c65"),
    "animated_lossy_inter_420_8bit_b8x32_fine_q70_32x64": ("animated_lossy_inter_420_8bit_b8x32_fine_q70_32x64.avif", "313407ca95b721e8cd6aafa257aa3c86d70b65495e700a8b8291cb24aa4347da"),
    "animated_lossy_inter_420_8bit_b8x32_fine_q99_32x64": ("animated_lossy_inter_420_8bit_b8x32_fine_q99_32x64.avif", "fffe6bcfbcd058ff60da3e5ddcf66126a898a937bab2532eb73d3e82c7919862"),
    "animated_lossy_inter_422_10bit_b32x8_fine_q70_64x32": ("animated_lossy_inter_422_10bit_b32x8_fine_q70_64x32.avif", "ed20922dc874f9a76c3876a44cd1ddd781c7e3a9d1b7fd963ca5f3a5efceac81"),
    "animated_lossy_inter_422_10bit_b32x8_fine_q99_64x32": ("animated_lossy_inter_422_10bit_b32x8_fine_q99_64x32.avif", "9c6cd0216ea53b6b02d16576e355ca5477a4ff05562f8711244423624a3d69a0"),
    "animated_lossy_inter_422_12bit_b32x8_fine_q70_64x32": ("animated_lossy_inter_422_12bit_b32x8_fine_q70_64x32.avif", "620945fb47fea0c038a4feaae766140b620ac5749d87073759154ba6a05a7b32"),
    "animated_lossy_inter_422_12bit_b32x8_fine_q99_64x32": ("animated_lossy_inter_422_12bit_b32x8_fine_q99_64x32.avif", "e1db7df8902375e481d48ef795b0e4bbe4c47a79c99b2f2e47de93f71798f392"),
    "animated_lossy_inter_422_8bit_b32x8_fine_q70_64x32": ("animated_lossy_inter_422_8bit_b32x8_fine_q70_64x32.avif", "bf47c9d891722e2370db4dfa013aaadeb5e6e00951f5a3468132914ad264b464"),
    "animated_lossy_inter_422_8bit_b32x8_fine_q99_64x32": ("animated_lossy_inter_422_8bit_b32x8_fine_q99_64x32.avif", "624272ee6fc567be1e21ab17f04907d9f140bf243aba7d7ea793d7464a8391b0"),
    "animated_lossy_inter_444_10bit_b32x8_fine_q70_64x32": ("animated_lossy_inter_444_10bit_b32x8_fine_q70_64x32.avif", "a06e87aa12fd4426b2c4fb25a6cbf64b78afc3093812c19c973617a32f22ed2d"),
    "animated_lossy_inter_444_10bit_b32x8_fine_q99_64x32": ("animated_lossy_inter_444_10bit_b32x8_fine_q99_64x32.avif", "f795474b4ce43d9e220d3b1ab7c2cf601a4222bf3b2928ec17b1ad896770db06"),
    "animated_lossy_inter_444_10bit_b8x32_fine_q70_32x64": ("animated_lossy_inter_444_10bit_b8x32_fine_q70_32x64.avif", "5de06160e7a732f1a665aab3833a18efb24e396af70a11fac22e58f40b3e8846"),
    "animated_lossy_inter_444_10bit_b8x32_fine_q99_32x64": ("animated_lossy_inter_444_10bit_b8x32_fine_q99_32x64.avif", "8b64087b62639992a4a4d95e016bb20f5ae5702e9d0cd63d3e83f9b17eeaa073"),
    "animated_lossy_inter_444_12bit_b32x8_fine_q70_64x32": ("animated_lossy_inter_444_12bit_b32x8_fine_q70_64x32.avif", "d1ddc1629ae10d202960d6710077c8fd977829cb16d8a99520c42521aa365213"),
    "animated_lossy_inter_444_12bit_b32x8_fine_q99_64x32": ("animated_lossy_inter_444_12bit_b32x8_fine_q99_64x32.avif", "d3100bb64ec47796a838b2082cd6d1f76cce8f081ee5a00364d40136c177cd1f"),
    "animated_lossy_inter_444_12bit_b8x32_fine_q70_32x64": ("animated_lossy_inter_444_12bit_b8x32_fine_q70_32x64.avif", "69a5e083e0b88b3a92e1d465251867833904f9e26208a2860ae620d9fc603da8"),
    "animated_lossy_inter_444_12bit_b8x32_fine_q99_32x64": ("animated_lossy_inter_444_12bit_b8x32_fine_q99_32x64.avif", "a65e5f31a658000a530198fdb1fec48600265993e02409f0dbfd3859d695615c"),
    "animated_lossy_inter_444_8bit_b32x8_fine_q70_64x32": ("animated_lossy_inter_444_8bit_b32x8_fine_q70_64x32.avif", "f311b4b5f8315ed9d43496348f4b74707d0f58fab779345c4153ce328a6df1bc"),
    "animated_lossy_inter_444_8bit_b32x8_fine_q99_64x32": ("animated_lossy_inter_444_8bit_b32x8_fine_q99_64x32.avif", "4aa8e523e1f83a95b39fd65f468910055892af1eda7386a1ca4575fbe3efbb55"),
    "animated_lossy_inter_444_8bit_b8x32_fine_q70_32x64": ("animated_lossy_inter_444_8bit_b8x32_fine_q70_32x64.avif", "f62b283664c0998a04c8f8e15ed4ae6c9155d7b668cf40ad678b89de0a6379df"),
    "animated_lossy_inter_444_8bit_b8x32_fine_q99_32x64": ("animated_lossy_inter_444_8bit_b8x32_fine_q99_32x64.avif", "d32bb7e65e591b1d1cd8a1ef222d08032e8eb2a1d08fc8c562eaa799be093429"),
    "animated_lossy_inter_400_10bit_b16x64_fine_q70_64x128": ("animated_lossy_inter_400_10bit_b16x64_fine_q70_64x128.avif", "49bad6f01d2beeb984b0c3870f8007c86f0e74b1caabcf2f040f354b69b9ede7"),
    "animated_lossy_inter_400_10bit_b16x64_fine_q99_64x128": ("animated_lossy_inter_400_10bit_b16x64_fine_q99_64x128.avif", "ca9560fe84b4aa3d5a7ed8c003fe683c7ae0d9e04cd4d2f604dee4c19d1db90f"),
    "animated_lossy_inter_400_10bit_b64x16_fine_q70_128x64": ("animated_lossy_inter_400_10bit_b64x16_fine_q70_128x64.avif", "3ad12ae941fb951539b1175274ef94a87ab8ce421e290c02229208e78fff5339"),
    "animated_lossy_inter_400_10bit_b64x16_fine_q99_128x64": ("animated_lossy_inter_400_10bit_b64x16_fine_q99_128x64.avif", "cb9a102a9cf51b4775351a376bcaad9ae394563871d4cbc68c34d3834aa50b3d"),
    "animated_lossy_inter_400_12bit_b16x64_fine_q70_64x128": ("animated_lossy_inter_400_12bit_b16x64_fine_q70_64x128.avif", "4671c3d0e852df04bf1d622e4f65761106c071902e710c6ccb825344817f7252"),
    "animated_lossy_inter_400_12bit_b16x64_fine_q99_64x128": ("animated_lossy_inter_400_12bit_b16x64_fine_q99_64x128.avif", "6689524391098bc11d49d3ef94c01e2322adddcb01a26f51dbf106b4b7906ad4"),
    "animated_lossy_inter_400_12bit_b64x16_fine_q70_128x64": ("animated_lossy_inter_400_12bit_b64x16_fine_q70_128x64.avif", "5e0706f835ed08e3b748867e78dd5997bf68db7d7d88a7967578c22b76973ef0"),
    "animated_lossy_inter_400_12bit_b64x16_fine_q99_128x64": ("animated_lossy_inter_400_12bit_b64x16_fine_q99_128x64.avif", "7ed4f24c091ebe3f4fe2c9baeb5f29e13f4f1b44bec7b09d93336886531a0d6b"),
    "animated_lossy_inter_400_8bit_b16x64_fine_q70_64x128": ("animated_lossy_inter_400_8bit_b16x64_fine_q70_64x128.avif", "90b13958df771aec1a129a9c0bf420a66a2208b28ba948479c6ae9458e03ed6a"),
    "animated_lossy_inter_400_8bit_b16x64_fine_q99_64x128": ("animated_lossy_inter_400_8bit_b16x64_fine_q99_64x128.avif", "c00403f756dcb108d406ce8f45f7764edb2fd86ae58bf779fe7e4dd934df79b1"),
    "animated_lossy_inter_400_8bit_b64x16_fine_q70_128x64": ("animated_lossy_inter_400_8bit_b64x16_fine_q70_128x64.avif", "a194dac95b7c59f896b07ae2a3e69c12830b3b70be7d349a42d2dbdd76bee499"),
    "animated_lossy_inter_400_8bit_b64x16_fine_q99_128x64": ("animated_lossy_inter_400_8bit_b64x16_fine_q99_128x64.avif", "3b2eaabf9fa7b123d1fc765a43969a7fd7b44bd6ffdda5de190bafe46ea2c3bc"),
    "animated_lossy_inter_420_10bit_b16x64_fine_q70_64x128": ("animated_lossy_inter_420_10bit_b16x64_fine_q70_64x128.avif", "5c257fda08669a989e7f04e819d4e59e31425c8c55f5cf308fc33f0c290b1f16"),
    "animated_lossy_inter_420_10bit_b16x64_fine_q99_64x128": ("animated_lossy_inter_420_10bit_b16x64_fine_q99_64x128.avif", "3d8f720022101a8a6b447600328d346f8c670ce085a0bc4f48b51024ce74ee22"),
    "animated_lossy_inter_420_10bit_b64x16_fine_q70_128x64": ("animated_lossy_inter_420_10bit_b64x16_fine_q70_128x64.avif", "ee7758bf8934caa079e1de77d038f3f4686ad001b49cc7b383b91612bd372f6b"),
    "animated_lossy_inter_420_10bit_b64x16_fine_q99_128x64": ("animated_lossy_inter_420_10bit_b64x16_fine_q99_128x64.avif", "019fa5687bebfdfc2d3913b48bb5436f0b8d0e638566d19a5a36b84a092669f5"),
    "animated_lossy_inter_420_12bit_b16x64_fine_q70_64x128": ("animated_lossy_inter_420_12bit_b16x64_fine_q70_64x128.avif", "972476dbddb95d4d9c19492c5258f5bc0742ce3b50926c45166230e074263ba9"),
    "animated_lossy_inter_420_12bit_b16x64_fine_q99_64x128": ("animated_lossy_inter_420_12bit_b16x64_fine_q99_64x128.avif", "c7d228764354de4529614bc5789b88b16a6badbff3ab6ac14c7434e9ef778959"),
    "animated_lossy_inter_420_12bit_b64x16_fine_q70_128x64": ("animated_lossy_inter_420_12bit_b64x16_fine_q70_128x64.avif", "1adc5f8be84cfa7c217d3e9fb357c7977717deeeff198b6597704efeaeca8e68"),
    "animated_lossy_inter_420_12bit_b64x16_fine_q99_128x64": ("animated_lossy_inter_420_12bit_b64x16_fine_q99_128x64.avif", "f21e305a093004847824757adbfffbb8f9b3d1c631954e4ffddb6dccb9244d93"),
    "animated_lossy_inter_420_8bit_b16x64_fine_q70_64x128": ("animated_lossy_inter_420_8bit_b16x64_fine_q70_64x128.avif", "503cc5d32348d98b6a4f35326ed68ac8be16a16b67749ce109dfa7f05a057a18"),
    "animated_lossy_inter_420_8bit_b16x64_fine_q99_64x128": ("animated_lossy_inter_420_8bit_b16x64_fine_q99_64x128.avif", "8983ec15d4c75bebef5e9c80b7d7428d64cbbc1f454924420f33924e9083311d"),
    "animated_lossy_inter_420_8bit_b64x16_fine_q70_128x64": ("animated_lossy_inter_420_8bit_b64x16_fine_q70_128x64.avif", "4f256bdc7b27f92467c8ef0828d9a7f14df5e47a717ba6dd212d2f036a52b081"),
    "animated_lossy_inter_420_8bit_b64x16_fine_q99_128x64": ("animated_lossy_inter_420_8bit_b64x16_fine_q99_128x64.avif", "5d3e7d46b57ad063b47e2a356dc0b4f5b303b4e8d5d6e1d91f7f08f225b95c39"),
    "animated_lossy_inter_422_10bit_b64x16_fine_q70_128x64": ("animated_lossy_inter_422_10bit_b64x16_fine_q70_128x64.avif", "d3fbac1b2774b170fc74d906e38a91dd836743f5a78450e71b22be0f736a91a2"),
    "animated_lossy_inter_422_10bit_b64x16_fine_q99_128x64": ("animated_lossy_inter_422_10bit_b64x16_fine_q99_128x64.avif", "cb9d15caeae2d0752b352d017ea9ea5dffe3c2749d28b46dbfd7dac901cd10d8"),
    "animated_lossy_inter_422_12bit_b64x16_fine_q70_128x64": ("animated_lossy_inter_422_12bit_b64x16_fine_q70_128x64.avif", "361ae1722b3f09b6272e7c0ddee39b508ae090912fb966745cba7a6f415e2b63"),
    "animated_lossy_inter_422_12bit_b64x16_fine_q99_128x64": ("animated_lossy_inter_422_12bit_b64x16_fine_q99_128x64.avif", "3d5c3acea1c71099fcd940d37035e2c475f92b739416a67736bf4a40191764ac"),
    "animated_lossy_inter_422_8bit_b64x16_fine_q70_128x64": ("animated_lossy_inter_422_8bit_b64x16_fine_q70_128x64.avif", "a969a0b3ae5b95f2a207413b416dc46eedd4f9d6ed0a655f237ff26e24849c01"),
    "animated_lossy_inter_422_8bit_b64x16_fine_q99_128x64": ("animated_lossy_inter_422_8bit_b64x16_fine_q99_128x64.avif", "9d409b9a083f5a96f3c5b3716b323c1fcb3c19b34e50646c248cd752d647bcdc"),
    "animated_lossy_inter_444_10bit_b16x64_fine_q70_64x128": ("animated_lossy_inter_444_10bit_b16x64_fine_q70_64x128.avif", "a0c9488b3b586b87a900933627c0440c216e9291c56e37133625a2b6f2c6f425"),
    "animated_lossy_inter_444_10bit_b16x64_fine_q99_64x128": ("animated_lossy_inter_444_10bit_b16x64_fine_q99_64x128.avif", "ed5619bacc370fcd6c96109769898fdb5150c96015ba3b113219bd1e97994781"),
    "animated_lossy_inter_444_10bit_b64x16_fine_q70_128x64": ("animated_lossy_inter_444_10bit_b64x16_fine_q70_128x64.avif", "aa0938c000cb88daf73f1e7c7f7d6e6436f3fabdc529820f9699137eb4d63c5b"),
    "animated_lossy_inter_444_10bit_b64x16_fine_q99_128x64": ("animated_lossy_inter_444_10bit_b64x16_fine_q99_128x64.avif", "135be2b0a4e031bfd563e2ee15d6a84721682fd60e0a299094edee7317ece272"),
    "animated_lossy_inter_444_12bit_b16x64_fine_q70_64x128": ("animated_lossy_inter_444_12bit_b16x64_fine_q70_64x128.avif", "ebb47c4009befc31a837fd357d629aae6906272c5e33d54b248dbc14c7138238"),
    "animated_lossy_inter_444_12bit_b16x64_fine_q99_64x128": ("animated_lossy_inter_444_12bit_b16x64_fine_q99_64x128.avif", "40642ad5137705d5223adab0e84e497165b98f1394872e0dc79c18d31fe35767"),
    "animated_lossy_inter_444_12bit_b64x16_fine_q70_128x64": ("animated_lossy_inter_444_12bit_b64x16_fine_q70_128x64.avif", "8347cdaa8c2beff489e17bb65800efe1cb893a6827a2f05984784ab20ee96e8c"),
    "animated_lossy_inter_444_12bit_b64x16_fine_q99_128x64": ("animated_lossy_inter_444_12bit_b64x16_fine_q99_128x64.avif", "7739ead4747e3c877d223b9eeb458852b6baf6539ab75c0f32d05851c54bd121"),
    "animated_lossy_inter_444_8bit_b16x64_fine_q70_64x128": ("animated_lossy_inter_444_8bit_b16x64_fine_q70_64x128.avif", "41e1b3bea261d0114cefad676d74efd5ece895ac963e4fe9588479497c176d8c"),
    "animated_lossy_inter_444_8bit_b16x64_fine_q99_64x128": ("animated_lossy_inter_444_8bit_b16x64_fine_q99_64x128.avif", "a2cfdd6925197628aee4de3217618955f47a438e19d59e49a496ce4dfaf1ad35"),
    "animated_lossy_inter_444_8bit_b64x16_fine_q70_128x64": ("animated_lossy_inter_444_8bit_b64x16_fine_q70_128x64.avif", "4a91227df1be5279658e9aabeee020e635d880297419ed5fcecd3801829ddef1"),
    "animated_lossy_inter_444_8bit_b64x16_fine_q99_128x64": ("animated_lossy_inter_444_8bit_b64x16_fine_q99_128x64.avif", "a10d281a2423a1e7bf7420c8cb8e6233a8cb5f9aeb194186a1e067daee3767c8"),
    "animated_lossy_inter_422_10bit_b64x64_chroma_neighbors_q70_128x128": ("animated_lossy_inter_422_10bit_b64x64_chroma_neighbors_q70_128x128.avif", "edf322e0eadf3410d5235bb485ae2c527d50a7b48ee52f3905f7474208acf9c1"),
    "animated_lossy_inter_422_10bit_b64x64_chroma_neighbors_q95_128x128": ("animated_lossy_inter_422_10bit_b64x64_chroma_neighbors_q95_128x128.avif", "76cc24ef48bd0f97153a515e3fcc2305273d3703826bf9283abe06fe2d03a481"),
    "animated_lossy_inter_422_10bit_b64x64_chroma_neighbors_q99_128x128": ("animated_lossy_inter_422_10bit_b64x64_chroma_neighbors_q99_128x128.avif", "6b9ecccad6f3c17011a770383d67174ae377f2d2e5b04cafb49abba059e83a2b"),
    "animated_lossy_inter_422_12bit_b64x64_chroma_neighbors_q70_128x128": ("animated_lossy_inter_422_12bit_b64x64_chroma_neighbors_q70_128x128.avif", "8eb986ee0975669302bce752153dbb74110f2fc6658ad6041da9e94e43ea88b2"),
    "animated_lossy_inter_422_12bit_b64x64_chroma_neighbors_q95_128x128": ("animated_lossy_inter_422_12bit_b64x64_chroma_neighbors_q95_128x128.avif", "6e5ac1c5a5e90c5a7a1dc0c029a535bcadd74bc2ce5e53176c9ff60e765c0f54"),
    "animated_lossy_inter_422_12bit_b64x64_chroma_neighbors_q99_128x128": ("animated_lossy_inter_422_12bit_b64x64_chroma_neighbors_q99_128x128.avif", "5e20c66fdfcf724dd912fa42bc1bd01184c6ffebd9ced56579e3ab203cc97b0e"),
    "animated_lossy_inter_422_8bit_b64x64_chroma_neighbors_q70_128x128": ("animated_lossy_inter_422_8bit_b64x64_chroma_neighbors_q70_128x128.avif", "4ebbf5720ed2c7f37624c276cef435a298b07676af339fc6e18413c82aadf08e"),
    "animated_lossy_inter_422_8bit_b64x64_chroma_neighbors_q95_128x128": ("animated_lossy_inter_422_8bit_b64x64_chroma_neighbors_q95_128x128.avif", "9d8d7c7b7d54558a0f13d5796d085f6ffbd5a66a2f4854c8614378e15a6e9d6c"),
    "animated_lossy_inter_422_8bit_b64x64_chroma_neighbors_q99_128x128": ("animated_lossy_inter_422_8bit_b64x64_chroma_neighbors_q99_128x128.avif", "fa9eccb266209951f7029d31a9411cc8bd7a64f3b5120b394433a28aba639d0e"),
    "animated_lossy_inter_444_10bit_b64x64_chroma_neighbors_q70_128x128": ("animated_lossy_inter_444_10bit_b64x64_chroma_neighbors_q70_128x128.avif", "6702f30cc913b6593855f9794df3a8b7ba7d5f0b2a5250c2cccd7d54c86867f7"),
    "animated_lossy_inter_444_10bit_b64x64_chroma_neighbors_q95_128x128": ("animated_lossy_inter_444_10bit_b64x64_chroma_neighbors_q95_128x128.avif", "0dcaa8301a001f1e12dcbb320643f790bac0cee8497b22597d52fa0164645baa"),
    "animated_lossy_inter_444_10bit_b64x64_chroma_neighbors_q99_128x128": ("animated_lossy_inter_444_10bit_b64x64_chroma_neighbors_q99_128x128.avif", "b66ae69d670292ac9037c284223771a8ea67619b339a2732e677e317e72970d6"),
    "animated_lossy_inter_444_12bit_b64x64_chroma_neighbors_q70_128x128": ("animated_lossy_inter_444_12bit_b64x64_chroma_neighbors_q70_128x128.avif", "9783e41462f61f123fface7381182f3806c8b0ac01e00c326c5fd270b488752c"),
    "animated_lossy_inter_444_12bit_b64x64_chroma_neighbors_q95_128x128": ("animated_lossy_inter_444_12bit_b64x64_chroma_neighbors_q95_128x128.avif", "051fe0551d5131335d62ebf088c9a05be5d33315734baf2b3a4c31d222f59fe5"),
    "animated_lossy_inter_444_12bit_b64x64_chroma_neighbors_q99_128x128": ("animated_lossy_inter_444_12bit_b64x64_chroma_neighbors_q99_128x128.avif", "000526c180b9b11154623bd297ee4550b583694cc2636715b2e7cc8b69bfc0f5"),
    "animated_lossy_inter_444_8bit_b64x64_chroma_neighbors_q70_128x128": ("animated_lossy_inter_444_8bit_b64x64_chroma_neighbors_q70_128x128.avif", "245d91f8a17fac4aee30b0d0317e9fb33931e6747fe481383b799e75d69f2046"),
    "animated_lossy_inter_444_8bit_b64x64_chroma_neighbors_q95_128x128": ("animated_lossy_inter_444_8bit_b64x64_chroma_neighbors_q95_128x128.avif", "1d72a389510eb9de4107b14242405177a1ece6a8f7cf95336a56aeb1e8c46c6d"),
    "animated_lossy_inter_444_8bit_b64x64_chroma_neighbors_q99_128x128": ("animated_lossy_inter_444_8bit_b64x64_chroma_neighbors_q99_128x128.avif", "d152f921af6d2a2a6c48dc5e851646d400dc8994b2acb57797f41ba2a0133d55"),
    'animated_mixed_lossless_superres_400_10bit_neutral_80x48': ('animated_mixed_lossless_superres_400_10bit_neutral_80x48.avif', '6748e3ac11be0ee1f4d06191e4981419de26a71f992ac3074cb4fcdcdbf46125'),
    'animated_mixed_lossless_superres_400_10bit_sgr_80x48': ('animated_mixed_lossless_superres_400_10bit_sgr_80x48.avif', 'c6ec5a4851183d0d06c3af9b43f1aca419656b1c1e3eb1ea28624fd902d37eb2'),
    'animated_mixed_lossless_superres_400_10bit_filmgrain_80x48': ('animated_mixed_lossless_superres_400_10bit_filmgrain_80x48.avif', 'eb12b32b20cf07fcf25191c18f710c04a62d4451cbd54094e2acb98ffc01d531'),
    'animated_mixed_lossless_superres_400_12bit_neutral_80x48': ('animated_mixed_lossless_superres_400_12bit_neutral_80x48.avif', 'a538876e087283a7d2596e3f5ef87446869308895c34736f5887c8fab5006529'),
    'animated_mixed_lossless_superres_400_12bit_filmgrain_80x48': ('animated_mixed_lossless_superres_400_12bit_filmgrain_80x48.avif', '8b05f1891e4720bf3906e9905641e4f23137c5284a3cdb165cf5241e527000d2'),
    'animated_mixed_lossless_superres_400_8bit_neutral_80x48': ('animated_mixed_lossless_superres_400_8bit_neutral_80x48.avif', 'aa2299060bc5f9b18ae88894bd29dfe06801606160dca1498a74fe6547faa47c'),
    'animated_mixed_lossless_superres_400_8bit_filmgrain_80x48': ('animated_mixed_lossless_superres_400_8bit_filmgrain_80x48.avif', '903594a10cea1383ddde6930d31119a46b9ba7f756793e47e351c2d158256196'),
    'animated_mixed_lossless_superres_400_8bit_sgr_80x48': ('animated_mixed_lossless_superres_400_8bit_sgr_80x48.avif', '8886acec3f9b44eab0bb9dffe033b862f17f39117188eb8ec2425922b401e301'),
    'animated_mixed_lossless_superres_420_10bit_neutral_80x48': ('animated_mixed_lossless_superres_420_10bit_neutral_80x48.avif', '9cef3d1c8cc2c8a7484ce6a04ec4a706de060cc0212d68409cd3e3b6e34f0e2d'),
    'animated_mixed_lossless_superres_420_10bit_filmgrain_80x48': ('animated_mixed_lossless_superres_420_10bit_filmgrain_80x48.avif', 'aec952b0067e4b4a022ddb0427f7e87d9fed11ee235a454c60da6be92cb88f2f'),
    'animated_mixed_lossless_superres_420_10bit_sgr_80x48': ('animated_mixed_lossless_superres_420_10bit_sgr_80x48.avif', 'dbe51bd886ae6b5077c33f836f269098d8904099235e8a46baa76774d0ec7b44'),
    'animated_mixed_lossless_superres_420_12bit_neutral_80x48': ('animated_mixed_lossless_superres_420_12bit_neutral_80x48.avif', '8dd67ec59bd9fb4f40cf6a518ffca77905f7541044f10c5875e5b9c4c82bd40b'),
    'animated_mixed_lossless_superres_420_12bit_filmgrain_80x48': ('animated_mixed_lossless_superres_420_12bit_filmgrain_80x48.avif', 'aaa8f784f24357bc84ce212b8e995f68b251d4a84152c3953e6e76af865a1589'),
    'animated_mixed_lossless_superres_420_8bit_neutral_80x48': ('animated_mixed_lossless_superres_420_8bit_neutral_80x48.avif', 'eb779d61beadb9e5e05ccb34834860a8ce39b451be13c207b3c1f0ea30395d21'),
    'animated_mixed_lossless_superres_420_8bit_filmgrain_80x48': ('animated_mixed_lossless_superres_420_8bit_filmgrain_80x48.avif', '4486713a624c8282f82f165aadacbf6a6dfda4244375dcae2878cb550cb4fea1'),
    'animated_mixed_lossless_superres_420_8bit_cdef_filmgrain_80x48': ('animated_mixed_lossless_superres_420_8bit_cdef_filmgrain_80x48.avif', 'dcc57c6d407a6c842ffc83e9e94855f548c58f7bc8a68d198f12847fa84628a2'),
    'animated_mixed_lossless_superres_420_8bit_loopfilter_80x48': ('animated_mixed_lossless_superres_420_8bit_loopfilter_80x48.avif', 'd34b4d36352a5401c5320b9576b26ab3a6a6b7587b1e38e078549b367a4f0b3b'),
    'animated_mixed_lossless_superres_420_8bit_sgr_80x48': ('animated_mixed_lossless_superres_420_8bit_sgr_80x48.avif', '706483e6149e8b8aeab31f85344aaad8f38b3fda125cad40db65134d0db43048'),
    'animated_mixed_lossless_superres_420_8bit_sgr_filmgrain_80x48': ('animated_mixed_lossless_superres_420_8bit_sgr_filmgrain_80x48.avif', 'c9bbded2b96c08a2a2e5792bee2f313858e7889727e7eb6c99eba059c3905563'),
    'animated_mixed_lossless_superres_422_10bit_neutral_80x48': ('animated_mixed_lossless_superres_422_10bit_neutral_80x48.avif', 'af446607101962de4733065bc5770cd9a7ff66714cf6dc3861ee41e5dd20d3c2'),
    'animated_mixed_lossless_superres_422_10bit_filmgrain_80x48': ('animated_mixed_lossless_superres_422_10bit_filmgrain_80x48.avif', 'f9bc47d21bfa3e40d7724547603a25ae171170ca8b12264f437b20df7f7108ba'),
    'animated_mixed_lossless_superres_422_10bit_sgr_80x48': ('animated_mixed_lossless_superres_422_10bit_sgr_80x48.avif', '707f9011d2bfc877ac0f51800ef8bc5661971294e7defacc5508eaa676470575'),
    'animated_mixed_lossless_superres_422_12bit_neutral_80x48': ('animated_mixed_lossless_superres_422_12bit_neutral_80x48.avif', 'da6d31fc9ac3f7687c8273eda7f66f91995c6f1dc1e41bca3c98fd7d3d7450e9'),
    'animated_mixed_lossless_superres_422_12bit_filmgrain_80x48': ('animated_mixed_lossless_superres_422_12bit_filmgrain_80x48.avif', '7cd21564db3da0e82a20270034e9c85f7e7551e14f520cbe9b3bb141efa27e99'),
    'animated_mixed_lossless_superres_422_8bit_neutral_80x48': ('animated_mixed_lossless_superres_422_8bit_neutral_80x48.avif', '9c0b476ffff1a1665073179d6c8bddf5f8e87e22d817b871c6f6a1f538fadca1'),
    'animated_mixed_lossless_superres_422_8bit_filmgrain_80x48': ('animated_mixed_lossless_superres_422_8bit_filmgrain_80x48.avif', '41a52794082870ba454795331fbe40dcb1a9fa12fed53acd821150f7afb0998b'),
    'animated_mixed_lossless_superres_422_8bit_sgr_80x48': ('animated_mixed_lossless_superres_422_8bit_sgr_80x48.avif', 'a6f709522cf9a32d8430f4fb1833f38a8e3905379369406da4dfdda21558d082'),
    'animated_mixed_lossless_superres_444_10bit_neutral_80x48': ('animated_mixed_lossless_superres_444_10bit_neutral_80x48.avif', 'bb40405f9a5a057e09c9f17b02fba7be8a289e5f7b83dd8e24d5b275d1ed8f1f'),
    'animated_mixed_lossless_superres_444_10bit_loopfilter_80x48': ('animated_mixed_lossless_superres_444_10bit_loopfilter_80x48.avif', 'e7c75ed78f3ba1ef3bcd4c56c32e7ea832901cfcc41b0282a05c1bce4cfda646'),
    'animated_mixed_lossless_superres_444_10bit_filmgrain_128x64': ('animated_mixed_lossless_superres_444_10bit_filmgrain_128x64.avif', '14d1effed3e3c34e17ca86936ba332d28cbcd25376f2a642ff7fd1d4d1e673c6'),
    'animated_mixed_lossless_superres_444_10bit_sgr_80x48': ('animated_mixed_lossless_superres_444_10bit_sgr_80x48.avif', '4abbb1d3a14c6fa89f42501572349e277c12478d36678102163c4359da89a8d6'),
    'animated_mixed_lossless_superres_444_12bit_neutral_80x48': ('animated_mixed_lossless_superres_444_12bit_neutral_80x48.avif', '97f8d605402baade83f5889470ab9ba1dc4dd149efbfe4178d155a9b9c77c46d'),
    'animated_mixed_lossless_superres_444_12bit_filmgrain_128x64': ('animated_mixed_lossless_superres_444_12bit_filmgrain_128x64.avif', '00c469a1475ddeffeefa69a9a314c3bf4f172596ebcf2b826bd457dbc70222da'),
    'animated_mixed_lossless_superres_444_8bit_neutral_80x48': ('animated_mixed_lossless_superres_444_8bit_neutral_80x48.avif', '3694d481225590893e003ea85295323eb223a1f1e304ad16572a7b39cf19deec'),
    'animated_mixed_lossless_superres_444_8bit_sgr_80x48': ('animated_mixed_lossless_superres_444_8bit_sgr_80x48.avif', 'a49b14c52ccab5014531b7f7281c531bf093cba2cd338b2babb033436a2c9dab'),
}
STTS_CASES = {
    "stts_zero_delta": {"stts_sample_delta": 0},
    "stts_empty": {"stts_entry_count": 0, "stts_box_size": 16},
    "stts_zero_sample_count": {"stts_sample_count": 0},
    "stts_underfilled": {"stts_sample_count": 4},
    "stts_overfilled": {"stts_sample_count": 6},
    "stts_wrapped_sample_count": {
        "stts_sample_counts": [1, 0xFFFF_FFFF],
        "stts_sample_deltas": [1, 2],
    },
    "stts_trailing_payload": {"stts_trailing_payload_bytes": 4},
}
ALPHA_TIMING_CASES = {
    "sequence_alpha_duration_mismatch",
    "sequence_alpha_timescale_mismatch",
}
OBSERVER = r'''
#include <avif/avif.h>
#include <dlfcn.h>
#include <cstdio>
extern "C" int observe(void * library, const uint8_t * input, size_t size,
                        int * fields, char * diagnostic, size_t capacity) {
#define LOAD(name) \
    auto p_##name = reinterpret_cast<decltype(&name)>(dlsym(library, #name)); \
    if (!p_##name) return 1
    LOAD(avifDecoderCreate); LOAD(avifDecoderDestroy);
    LOAD(avifDecoderSetIOMemory); LOAD(avifDecoderParse); LOAD(avifDecoderNextImage);
#undef LOAD
    avifDecoder * decoder = p_avifDecoderCreate();
    if (!decoder) return 2;
    decoder->codecChoice = AVIF_CODEC_CHOICE_DAV1D;
    decoder->maxThreads = 1;
    auto finish = [&](int result) { p_avifDecoderDestroy(decoder); return result; };
    avifResult result = p_avifDecoderSetIOMemory(decoder, input, size);
    if (result != AVIF_RESULT_OK) return finish(3);
    result = p_avifDecoderParse(decoder);
    fields[0] = result;
    if (result != AVIF_RESULT_OK) {
        std::snprintf(diagnostic, capacity, "%s", decoder->diag.error);
        return finish(0);
    }
    fields[1] = decoder->repetitionCount;
    fields[2] = decoder->imageCount;
    fields[3] = 0;
    while ((result = p_avifDecoderNextImage(decoder)) == AVIF_RESULT_OK) ++fields[3];
    fields[4] = result;
    fields[5] = decoder->imageSequenceTrackPresent;
    std::snprintf(diagnostic, capacity, "%s", decoder->diag.error);
    if (result != AVIF_RESULT_NO_IMAGES_REMAINING) return finish(4);
    return finish(0);
}
'''


def track_boxes(data, ordinal=0):
    moov = unique_box(parse_boxes(data, 0, len(data)), b"moov")
    track = [b for b in children(data, moov) if b.kind == b"trak"][ordinal]
    boxes = children(data, track)
    tkhd = unique_box(boxes, b"tkhd")
    edts = unique_box(boxes, b"edts")
    elst = unique_box(children(data, edts), b"elst")
    if data[tkhd.payload_start] != 1 or data[elst.payload_start] != 1 or elst.size != 36:
        raise RuntimeError("pinned version-one mutation shape changed")
    return tkhd, edts, elst


def mutate(data, *, duration=None, flags=1, count=1, segment=5, version=1,
           no_edts=False, tail=None, size=None, track=0):
    tkhd, edts, elst = track_boxes(data, track)
    output = bytearray(data)
    if duration is not None:
        struct.pack_into(">Q", output, tkhd.payload_start + 28, duration)
    struct.pack_into(">II", output, elst.payload_start, (version << 24) | flags, count)
    struct.pack_into(">Q", output, elst.payload_start + 8, segment)
    if tail is not None:
        output[elst.payload_start + 16:elst.end] = tail
    if no_edts:
        output[edts.start + 4:edts.start + 8] = b"free"
    if size is not None:
        if not 8 <= size <= elst.size - 8:
            raise RuntimeError("cannot preserve a valid trailing free box")
        struct.pack_into(">I", output, elst.start, size)
        struct.pack_into(">I4s", output, elst.start + size, elst.size - size, b"free")
    if len(output) != len(data):
        raise RuntimeError("mutation changed complete file length")
    return bytes(output)


def pillow_observation(data, destination=None):
    try:
        frames = []
        with Image.open(io.BytesIO(data)) as image:
            for index in range(image.n_frames):
                image.seek(index)
                image.load()
                raw = image.tobytes()
                if destination is not None:
                    destination.mkdir(parents=True, exist_ok=True)
                    (destination / f"frame_{index}.bin").write_bytes(raw)
                frames.append({"index": index, "mode": image.mode, "size": list(image.size),
                               "bytes": len(raw), "sha256": sha256(raw),
                               "duration_ms": image.info.get("duration")})
            return {"status": "ok", "loop_key": image.info.get("loop"), "frames": frames}
    except Exception as error:
        message = str(error)
        if message.startswith("cannot identify image file <_io.BytesIO object at 0x"):
            message = "cannot identify image file <bytes>"
        return {"status": "error", "type": f"{type(error).__module__}.{type(error).__name__}",
                "message": message}


def collect(bundle, observe, only=None):
    originals, records = {}, []
    if only in STTS_CASES:
        fixtures = [("animated", FIXTURES["animated"])]
    elif only in ALPHA_TIMING_CASES:
        fixtures = [("highdepth", FIXTURES["highdepth"])]
    else:
        fixtures = FIXTURES.items() if only is None else [(only, FIXTURES[only])]
    for name, (filename, expected_hash) in fixtures:
        source = ROOT / "tests/fixtures/input/images/avif" / filename
        data = source.read_bytes()
        if sha256(data) != expected_hash:
            raise RuntimeError(f"pinned {name} source changed")
        originals[name] = data
        if only in STTS_CASES or only in ALPHA_TIMING_CASES:
            continue
        record = pillow_observation(data, bundle / "pixels" / name)
        if record["status"] != "ok" or record != pillow_observation(data):
            raise RuntimeError("original Pillow frames failed or changed")
        records.append((name, name, data, None))
    cases = [
        ("no_edit_list", dict(no_edts=True)),
        ("no_edit_list_zero_duration", dict(no_edts=True, duration=0)),
        ("nonrepeating_zero_duration", dict(flags=0, duration=0)),
        ("nonrepeating_ignored_fields", dict(flags=0xFFFFFE, version=255, count=0, segment=0, tail=b"\xff" * 12)),
        ("nonrepeating_header_only", dict(flags=0, size=12)),
        ("repeating_exact", dict(duration=15)),
        ("repeating_rounded", dict(duration=16)),
        ("repeating_partial", dict(duration=1)),
        ("repeating_reserved_flags", dict(duration=15, flags=0xFFFFFF)),
        ("repeating_ignored_media", dict(duration=15, tail=b"\xff" * 12)),
        ("repeating_segment_only", dict(duration=15, size=24)),
        ("repeating_v0", dict(duration=15, version=0, segment=5 << 32)),
        ("largest_finite", dict(duration=(1 << 31) * 5)),
        ("first_infinite", dict(duration=(1 << 31) * 5 + 1)),
        ("huge_finite_duration", dict(duration=(1 << 64) - 2, segment=1)),
        ("indefinite", dict(duration=(1 << 64) - 1)),
        ("error_zero_duration", dict(duration=0)),
        ("error_zero_segment", dict(segment=0)),
        ("error_indefinite_zero_segment", dict(duration=(1 << 64) - 1, segment=0)),
        ("error_entry_count", dict(count=2)),
        ("error_version", dict(version=2)),
        ("error_missing_segment", dict(size=16)),
        ("error_missing_flags", dict(size=8)),
    ] if only is None else []
    if only in STTS_CASES:
        cases = [(only, {})]
    elif only is None:
        cases.extend((name, {}) for name in STTS_CASES)
    timing_variants = (
        stts_timing_variants(originals["animated"])
        if any(name in STTS_CASES for name, _ in cases)
        else {}
    )
    for name, parameters in cases:
        if name == "stts_zero_delta":
            data = zero_stts_sample_delta(originals["animated"])
        elif name in timing_variants:
            data = timing_variants[name]
        else:
            data = mutate(originals["animated"], **parameters)
        mutation = STTS_CASES[name] if name in STTS_CASES else parameters
        records.append((name, "animated", data, mutation))
    if only in ALPHA_TIMING_CASES or only is None:
        source = originals["highdepth"]
        variants = (
            (
                "sequence_alpha_duration_mismatch",
                alpha_track_duration_mismatch,
                {"track": "auxiliary alpha", "stts_sample_delta": 2},
            ),
            (
                "sequence_alpha_timescale_mismatch",
                alpha_track_timescale_mismatch,
                {"track": "auxiliary alpha", "mdhd_timescale": 2},
            ),
        )
        for name, mutate_alpha, mutation in variants:
            if only is not None and only != name:
                continue
            derived = mutate_alpha(source)
            fixture_path = ROOT / "tests/fixtures/input/images/avif" / f"{name}.avif"
            data = fixture_path.read_bytes()
            if data != derived:
                raise RuntimeError(f"AVIF {name} fixture differs from its pinned mutation")
            records.append((name, "highdepth", data, mutation))
    if only is None:
        records.append(("alpha_loop_disagreement", "highdepth",
                        mutate(originals["highdepth"], track=1, flags=0), {"track": 1, "flags": 0}))
        records.append(("error_alpha_segment", "highdepth",
                        mutate(originals["highdepth"], track=1, segment=0), {"track": 1, "segment": 0}))
    output = []
    for name, source, data, mutation in records:
        native_repeats = []
        for _ in range(2):
            fields = (ctypes.c_int * 6)(*([-999] * 6))
            diagnostic = ctypes.create_string_buffer(512)
            if observe(data, len(data), fields, diagnostic, len(diagnostic)):
                raise RuntimeError(f"native observer failed for {name}")
            native_repeats.append({"parse_result": fields[0], "repetition_count": fields[1],
                                   "frame_count": fields[2], "decoded_frames": fields[3],
                                   "terminal_result": fields[4], "sequence_track_present": fields[5],
                                   "diagnostic": diagnostic.value.decode("utf-8")})
        pillow = pillow_observation(data)
        if native_repeats[0] != native_repeats[1] or pillow != pillow_observation(data):
            raise RuntimeError(f"native or Pillow observation changed: {name}")
        native = native_repeats[0]
        if name.startswith("error_") and name != "error_resilient":
            if native["parse_result"] == 0 or pillow["status"] != "error":
                raise RuntimeError(f"expected independent malformed-file rejection: {name}")
        elif name == "stts_zero_delta":
            original = pillow_observation(originals[source])
            if (
                native["parse_result"] != 0
                or native["decoded_frames"] != len(original["frames"])
                or pillow["status"] != "ok"
                or len(pillow["frames"]) != len(original["frames"])
            ):
                raise RuntimeError("zero stts delta did not preserve native/Pillow frame decoding")
            for before, after in zip(original["frames"], pillow["frames"]):
                if any(before[key] != after[key] for key in ("mode", "size", "bytes", "sha256")):
                    raise RuntimeError("zero stts delta changed Pillow frame pixels")
                if after["duration_ms"] != 0:
                    raise RuntimeError("Pillow did not report zero duration for zero stts delta")
        elif name == "stts_wrapped_sample_count":
            expected = pillow_observation(originals[source])
            if (
                native["parse_result"] != 0
                or native["decoded_frames"] != len(expected["frames"])
                or pillow["status"] != "ok"
                or len(pillow["frames"]) != len(expected["frames"])
            ):
                raise RuntimeError(
                    "wrapped stts count did not preserve native/Pillow frame decoding"
                )
            for index, (before, after) in enumerate(zip(expected["frames"], pillow["frames"])):
                if any(before[key] != after[key] for key in ("mode", "size", "bytes", "sha256")):
                    raise RuntimeError("wrapped stts count changed Pillow frame pixels")
                expected_duration = 33 if index == 0 else 67
                if after["duration_ms"] != expected_duration:
                    raise RuntimeError("Pillow timing differs from wrapped stts count mutation")
        else:
            expected = pillow_observation(originals[source])
            if (
                native["parse_result"] != 0
                or native["decoded_frames"] != len(expected["frames"])
                or pillow != expected
            ):
                raise RuntimeError(f"full-frame mutation changed native/Pillow decoding: {name}")
        relative = f"inputs/{name}.avif"
        (bundle / "inputs").mkdir(exist_ok=True)
        (bundle / relative).write_bytes(data)
        source_data = originals[source]
        source_mdats = [
            box for box in parse_boxes(source_data, 0, len(source_data))
            if box.kind == b"mdat"
        ]
        mutated_mdats = [
            box for box in parse_boxes(data, 0, len(data)) if box.kind == b"mdat"
        ]
        sample_payloads_equal = len(source_mdats) == len(mutated_mdats) and all(
            data[mutated.payload_start : mutated.end]
            == source_data[original.payload_start : original.end]
            for original, mutated in zip(source_mdats, mutated_mdats)
        )
        if not sample_payloads_equal:
            raise RuntimeError("mutation changed media bytes")
        output.append({"name": name, "source": source, "input_path": relative,
                       "input_bytes": len(data), "input_sha256": sha256(data),
                       "source_sha256": sha256(source_data), "mutation": {k: v.hex() if isinstance(v, bytes) else v for k, v in (mutation or {}).items()},
                       "native": native, "pillow": pillow, "repeat_equal": True,
                       "media_payloads_equal": True})
    return output


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--libavif-source", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument(
        "--only",
        choices=sorted((*FIXTURES, *STTS_CASES, *ALPHA_TIMING_CASES)),
        help="collect one unchanged fixture or one timing-table mutation",
    )
    args = parser.parse_args()
    output, source = args.output.resolve(), args.libavif_source.resolve()
    if (ROOT / "target/oracle-staging").resolve() not in output.parents or output.exists():
        raise RuntimeError("output must be fresh and below target/oracle-staging")
    if text_output(run(["git", "-C", str(source), "rev-parse", "HEAD"])) != LIBAVIF_COMMIT or run(["git", "-C", str(source), "status", "--porcelain"]).stdout:
        raise RuntimeError("libavif source must be clean and pinned")
    if digest_file(source / "LICENSE") != digest_file(ROOT / "third_party/libavif/LICENSE"):
        raise RuntimeError("libavif retained license differs")
    if (Image.__version__, features.version("avif"), _avif.codec_versions()) != (
        "12.2.0", "1.4.1", "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2"
    ):
        raise RuntimeError("Pillow/native version differs from pins")
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".avif-loops-", dir=output.parent) as temporary:
        work = Path(temporary)
        bundle = work / "bundle"
        bundle.mkdir()
        retained_readme = ROOT / "tests" / "fixtures" / "outputs" / "avif_loops" / "README.md"
        if retained_readme.is_file():
            shutil.copy2(retained_readme, bundle / "README.md")
        observer = bundle / "observer.cc"
        observer.write_text(OBSERVER)
        compiler = resolve_tool("c++", "C++ compiler")
        shared = work / "observer.so"
        command = [str(compiler), "-std=c++17", "-O2", "-Wall", "-Wextra", "-Werror", "-shared", "-fPIC",
                   f"-I{source / 'include'}", str(observer), "-o", str(shared)]
        if sys.platform.startswith("linux"):
            command.append("-ldl")
        run(command)
        library, harness = ctypes.CDLL(_avif.__file__), ctypes.CDLL(str(shared))
        native = harness.observe
        native.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_size_t, ctypes.POINTER(ctypes.c_int), ctypes.c_void_p, ctypes.c_size_t]
        native.restype = ctypes.c_int
        cases = collect(
            bundle,
            lambda *values: native(library._handle, *values),
            only=args.only,
        )
        build = {
            "compiler": text_output(run([str(compiler), "--version"])),
            "binary_sha256": digest_file(shared),
            "argv": [
                value.replace(str(source), "<source>").replace(str(work), "<work>")
                for value in command
            ],
        }
        for case in cases:
            case["observer_binary_sha256"] = build["binary_sha256"]
        index = {"schema": "image-slash-star/avif-loop-oracle@1", "cases": cases,
                 "origin": "libavif.avifDecoder.repetitionCount",
                 "source": {"commit": LIBAVIF_COMMIT, "tree_sha256": git_tree_digest(source),
                            "files": [{"path": name, "sha256": digest_file(source / name)} for name in ("include/avif/avif.h", "src/read.c", "LICENSE")]},
                 "oracle": {"pillow": Image.__version__, "libavif": features.version("avif"), "codecs": _avif.codec_versions(),
                            "avif_binary_sha256": digest_file(Path(_avif.__file__)), "imaging_binary_sha256": digest_file(Path(_imaging.__file__))},
                 "build": build,
                 "artifacts": artifact_records(bundle), "rust_execution": "deferred"}
        write_json(bundle / "index.json", index)
        shutil.move(bundle, output)
    print(f"Collected {len(cases)} complete AVIF loop cases: {output}")


if __name__ == "__main__":
    main()
