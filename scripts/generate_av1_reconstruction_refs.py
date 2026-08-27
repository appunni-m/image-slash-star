#!/usr/bin/env python3
"""Generate first-block AV1 reconstruction references from pinned dav1d.

This diagnostic oracle builds an instrumented temporary clone of the exact
dav1d source revision used by Pillow's AVIF decoder. It extracts the encoded
AV1 item directly from each committed AVIF fixture, decodes it with dav1d's
scalar C path, and records the first block trace plus stride-independent YUV
planes. It never invokes repository Rust code.

Meson and Ninja are development tools used only while regenerating this
reference. They are not crate dependencies.
"""

from __future__ import annotations

import argparse
import ctypes
import hashlib
import json
import os
import re
import shutil
import subprocess
import tempfile
from pathlib import Path

from PIL import Image, _avif, features

from inspect_av1_obus import inspect as inspect_av1
from inspect_avif_bitstreams import inspect as inspect_avif


ROOT = Path(__file__).resolve().parent.parent
FIXTURE_DIR = ROOT / "tests" / "fixtures" / "input" / "images" / "avif"
DEFAULT_OUTPUT = ROOT / "tests" / "fixtures" / "outputs" / "av1_reconstruction.json"
DAV1D_COMMIT = "b546257f770768b2c88258c533da38b91a06f737"
VERTICAL_FOLLOWING_TARGET_FIXTURES = frozenset(
    {
        "coverage_r32x8_filter_intra_cdf9_false_01.avif",
        "coverage_r16x32_following_filter_intra_split_mode0_01.avif",
        "coverage_r16x32_following_filter_intra_split_mode3_01.avif",
    }
)
SQUARE32_SPLIT_TARGET_FIXTURES = frozenset(
    {"coverage_square32_origin_tx16x16_split_01.avif"}
)
SQUARE64_SPLIT_TARGET_FIXTURES = frozenset(
    {"coverage_square64_origin_tx32x32_split_01.avif"}
)
SQUARE16_FILTER_INTRA_TARGET_FIXTURES = frozenset(
    {"coverage_square16_filter_intra_mode0_01.avif"}
)
SQUARE16_CHROMA_SMOOTH_HORIZONTAL_TARGET_FIXTURES = frozenset(
    {"coverage_square16_chroma_smooth_horizontal_01.avif"}
)
SQUARE16_CHROMA_SMOOTH_VERTICAL_TARGET_FIXTURES = frozenset(
    {"coverage_square16_chroma_smooth_vertical_01.avif"}
)
SQUARE16_CHROMA_SMOOTH_TARGET_FIXTURES = frozenset(
    {"coverage_square16_chroma_smooth_01.avif"}
)
VERTICAL8X16_FILTER_INTRA_TARGET_FIXTURES = frozenset(
    {
        "coverage_vertical8x16_filter_intra_mode0_01.avif",
        "coverage_vertical8x16_filter_intra_mode1_01.avif",
        "coverage_vertical8x16_filter_intra_mode2_01.avif",
    }
)
CHROMA_DIAGONAL113_TARGET_FIXTURES = frozenset(
    {"coverage_square8_chroma_diagonal113_01.avif"}
)
CHROMA_DIAGONAL45_TARGET_FIXTURES = frozenset(
    {"coverage_square8_chroma_diagonal45_angle51_01.avif"}
)
LUMA_DIAGONAL_DOWN_RIGHT_TARGET_FIXTURES = frozenset(
    {"coverage_square8_luma_diagonal_down_right_01.avif"}
)
LUMA_DIAGONAL45_TARGET_FIXTURES = frozenset(
    {"coverage_square8_luma_diagonal45_01.avif"}
)
CHROMA_DIAGONAL67_VERTICAL_TARGET_FIXTURES = frozenset(
    {"coverage_square8_chroma_diagonal67_vertical_01.avif"}
)
LUMA_SMOOTH_TARGET_FIXTURES = frozenset(
    {
        "coverage_square8_luma_smooth_01.avif",
        "coverage_square8_luma_smooth_vertical_01.avif",
        "coverage_square8_luma_smooth_horizontal_01.avif",
    }
)
CHROMA_DIAGONAL157_TARGET_FIXTURES = frozenset(
    {"coverage_vertical8x16_chroma_diagonal157_01.avif"}
)
CHROMA_HORIZONTAL_TARGET_FIXTURES = frozenset(
    {"coverage_vertical8x16_chroma_horizontal_01.avif"}
)
CHROMA_VERTICAL_TARGET_FIXTURES = frozenset(
    {"coverage_vertical8x16_chroma_vertical_01.avif"}
)
CHROMA_PAETH_TARGET_FIXTURES = frozenset(
    {
        "coverage_vertical8x16_chroma_paeth_01.avif",
        "coverage_vertical8x16_chroma_paeth_02.avif",
        "coverage_vertical8x16_chroma_paeth_03.avif",
    }
)
SQUARE16_CFL_TARGET_FIXTURES = frozenset(
    {
        "coverage_i444_square16_cfl_01.avif",
        "coverage_i444_square16_cfl_02.avif",
        "coverage_i444_square16_cfl_03.avif",
    }
)
EXPECTED_FIXTURES = {
    "portable_lossless_a.avif": {
        "file_sha256": "ccc84752237af0549d7310af7a5b948435b07c78f9b20c322240a18f1667c411",
        "rgb_sha256": "0fdfb2ec7d6741b65177c1343d0e510798f3177b75018fdbc8da541ea2d32a0b",
        "size": [4, 4],
    },
    "portable_lossless_b.avif": {
        "file_sha256": "4d319cc51aee3d79d5fb8a7c1fba1b42b42303ffa8baef1f7a1511fa4ec031ee",
        "rgb_sha256": "34a99c606d95db58868b24c3ce3ade1c502adcf213130c403486cbd50bc4fad5",
        "size": [4, 4],
    },
    "portable_lossless_420_a.avif": {
        "file_sha256": "640d19800ff27dbd1cd28e881736e923a48eb46e8223bed9d52bfb624b85e6a7",
        "rgb_sha256": "0fdfb2ec7d6741b65177c1343d0e510798f3177b75018fdbc8da541ea2d32a0b",
        "size": [4, 4],
    },
    "portable_lossless_420_b.avif": {
        "file_sha256": "bd6427ce4848cb4d65f83b1621ffda46a4614e6a8b316998b69234298077ffba",
        "rgb_sha256": "34a99c606d95db58868b24c3ce3ade1c502adcf213130c403486cbd50bc4fad5",
        "size": [4, 4],
    },
    "portable_lossless_420_8x8_a.avif": {
        "file_sha256": "21d453da436be1bbb47238e35d919499c7814a2a8073550b9ae958cafe78d15e",
        "rgb_sha256": "1f403e7f414473b888fcba438d60d269e54fc1d04c802dd32f96fa657932b2ac",
        "size": [8, 8],
    },
    "portable_lossless_420_8x8_b.avif": {
        "file_sha256": "311de615cc4f0f7cbd9f6c136170c383f5263659c07dcaa8fabb1877f87f415e",
        "rgb_sha256": "1217b329eae17189460716ba186b4d01617aa8648cd5c03aee2e8905cc20e008",
        "size": [8, 8],
    },
    "portable_lossy_420_q99_gray_0.avif": {
        "file_sha256": "7f1485129fd93e4318cf21bcf59934963c1a84b3bcb0d74f3e7555b3bad20b38",
        "rgb_sha256": "17b0761f87b081d5cf10757ccc89f12be355c70e2e29df288b65b30710dcbcd1",
        "size": [4, 4],
    },
    "portable_lossy_420_q99_token_1048_control.avif": {
        "file_sha256": "1097067dca85e499768a40e15232dce3602afbb1cabcbf485e8a14bf83e9bb73",
        "rgb_sha256": "17b0761f87b081d5cf10757ccc89f12be355c70e2e29df288b65b30710dcbcd1",
        "size": [4, 4],
    },
    "portable_lossy_420_q99_token_2061.avif": {
        "file_sha256": "bc97b1f2ca96f6072239101e096e1b18fe87cb6ecf13b48188b37b52a50d761e",
        "rgb_sha256": "80a76a18acf8cb64fec3a659ffc4bab4a87cd9a6fde4dab2161a8751d136c9d2",
        "size": [4, 4],
    },
    "portable_lossy_420_q99_token_2988.avif": {
        "file_sha256": "0153d56609f86e637159836af94d103523853c9002c92dc7411925d97a919250",
        "rgb_sha256": "80a76a18acf8cb64fec3a659ffc4bab4a87cd9a6fde4dab2161a8751d136c9d2",
        "size": [4, 4],
    },
    "portable_lossy_420_q99_token_7940.avif": {
        "file_sha256": "503ca52689395ec769b5453f7a30b4340f4234132338b1dd16e6a945ab34c37a",
        "rgb_sha256": "80a76a18acf8cb64fec3a659ffc4bab4a87cd9a6fde4dab2161a8751d136c9d2",
        "size": [4, 4],
    },
    "portable_lossy_420_q99_token_7764.avif": {
        "file_sha256": "15822dfb32fea6432adf1c7ddb9ea648dd6d2e028b12c9f117c6031420760367",
        "rgb_sha256": "17b0761f87b081d5cf10757ccc89f12be355c70e2e29df288b65b30710dcbcd1",
        "size": [4, 4],
    },
    "portable_lossy_420_q99_8x8_gray_0.avif": {
        "file_sha256": "75df02eb1a44eb478b17910a79179dcc563a4b1b72db2b6b25d229ba377320eb",
        "rgb_sha256": "5d89f056865052bcb89c910d2d62872e029fb273c3db03f8968a52a41593c1b5",
        "size": [8, 8],
    },
    "portable_lossy_420_q99_gray_64.avif": {
        "file_sha256": "6f4d9be7282279fdaaf38c1a464c49e44fb1373be0cfb83bb632f85167d1022e",
        "rgb_sha256": "30c8d471cc44e88da2fec08638a4215ed2ce34c899f330115a604b80d19f2831",
        "size": [4, 4],
    },
    "portable_lossy_420_q99_8x8_gray_64.avif": {
        "file_sha256": "350a8eca70ae23d2e4981c3a4f0e31c5edf060e6da940c56750fa5b4dbed3ff8",
        "rgb_sha256": "557f22c418e6f4fcd4d4c1df7eb2b46180b67956794483587205e2e82163b395",
        "size": [8, 8],
    },
    "portable_lossy_420_q99_gray_122_control.avif": {
        "file_sha256": "17c312d10c6cd7ecd6a1bf1fb6b1bfff07aa970ff2ff3e722f2dd984c714a80a",
        "rgb_sha256": "ad287d41398b2bc6aae343d24767bded9795b882f382b5abf480a6fc0bbddfdf",
        "size": [4, 4],
    },
    "portable_lossy_420_q99_8x8_gray_122_control.avif": {
        "file_sha256": "7163cc6aee6597f1792a6b963fb2777758fdbe7096bcfe5712df0e150f5c4d49",
        "rgb_sha256": "9e96fe6320d50c09026df65c9676a19e57fe86b26652cf513c2cc03015711df0",
        "size": [8, 8],
    },
    "portable_lossy_420_q99_gray_123_control.avif": {
        "file_sha256": "1e0f1f2ae4da78ca2cee5af734916106bb822d2d780f44111f257beed7c05890",
        "rgb_sha256": "819d474948483b42b8e875e2bb3446526e0a5f1f090d012b993d6a12fcf0e4da",
        "size": [4, 4],
    },
    "portable_lossy_420_q99_8x8_gray_123_control.avif": {
        "file_sha256": "842883fdf557bb56f02454da1f5e5fe91a87f4afa21b87ba4155abd51396687f",
        "rgb_sha256": "d9bced69730dcb4567fcd0eac9073a83993278a18aebf3c03544b49d5660576d",
        "size": [8, 8],
    },
    "portable_lossy_420_q99_gray_124_control.avif": {
        "file_sha256": "f2c1d46376a93d91baa784dfd69615bb1d334471ac997515612366085e2cb781",
        "rgb_sha256": "5acbd8048d53d1aa8fcbaacb57506e7eb6a1f570d93c899bd97f899f535f7ce9",
        "size": [4, 4],
    },
    "portable_lossy_420_q99_8x8_gray_124_control.avif": {
        "file_sha256": "4d1fc957ddb0e368fe179d7f93c8d64afb01bb24f3b876bd6c8cc7d2b337c033",
        "rgb_sha256": "25c65b22ccf260aff6e521fbce082a40fd940968592a3c2e5272768c362481da",
        "size": [8, 8],
    },
    "portable_lossy_420_q99_gray_125_control.avif": {
        "file_sha256": "43e09f9447cb94aaa979956887dad091ec1f630f6dab5e33eb68dfbc989537fa",
        "rgb_sha256": "e82feb502523b0e30e96c557012bbc79208f186e3fcb858916b2972db760aac1",
        "size": [4, 4],
    },
    "portable_lossy_420_q99_8x8_gray_125_control.avif": {
        "file_sha256": "70b97a8ecfdca48dadf67624fe03db3fa4672dfd921f11f7da1396c393c0b7be",
        "rgb_sha256": "4d11382e9da0a7e9facadaf22c7d9036b341797376ecc5a77c2779e1884e1ec5",
        "size": [8, 8],
    },
    "portable_lossy_420_q99_gray_126.avif": {
        "file_sha256": "f82b264295ffb7ea9e357a352e674200ed89138a182b0de7c4002fbc55fade4d",
        "rgb_sha256": "0bc6b6903ab77a6d1706777bb507e076f01290f57cb975508aec1cd5cf589810",
        "size": [4, 4],
    },
    "portable_lossy_420_q99_8x8_gray_126.avif": {
        "file_sha256": "90d415cfd1292d211e6b3874837853f8a7690f27de93c28a133e18f4af986ad1",
        "rgb_sha256": "9a5f0b79fce197304a6aa5a89af73862b128be0db6e93117a67d3ddd07e28edd",
        "size": [8, 8],
    },
    "portable_lossy_420_q99_gray_127.avif": {
        "file_sha256": "c232a943aef1ec71422567e9c00a137a70576c63a383621a4417a9637ee08732",
        "rgb_sha256": "a1fa26e9a041c510e9f8412accef2e5e0cda5eddd97fa6db80b30400b7964d42",
        "size": [4, 4],
    },
    "portable_lossy_420_q99_8x8_gray_127.avif": {
        "file_sha256": "947d6326cc09f88e50e0aba60d9cb468970d793ac323003a5d2452934998dcf1",
        "rgb_sha256": "c24e73f000a4255a612416ecc4df81c9313e4c099877384712e4d8530dd7acbd",
        "size": [8, 8],
    },
    "portable_lossy_420_q99_gray_129.avif": {
        "file_sha256": "79e3d72995eb382d5462e4309fec24e37111cd039a10bc9b28bd370b9b26fa64",
        "rgb_sha256": "b34e1e1e7cd63c9fb7069154ccd855d827a3dd3eca076232b4217745a2b6db57",
        "size": [4, 4],
    },
    "portable_lossy_420_q99_8x8_gray_129.avif": {
        "file_sha256": "ca48aaddde1310eecde25c24c24314089a5e62164c8dbd36b0c64b2ef9812507",
        "rgb_sha256": "fca06fef259b9ebb452449c7feda724ccec06a4a76b2b4fb1e6420a0beac435e",
        "size": [8, 8],
    },
    "portable_lossy_420_q99_gray_130.avif": {
        "file_sha256": "cf98497c2b678d67bbb9327f7816b9ef9d3d186ffee51b24ee10ec50e8e8d776",
        "rgb_sha256": "2c28ec0de076c8c2e7d6d8222ada07a0da8ec45ea53160a39b5dd64b79d7bcc8",
        "size": [4, 4],
    },
    "portable_lossy_420_q99_8x8_gray_130.avif": {
        "file_sha256": "a579a6a3f85a4b5574d237c3c06f1cff79404bb565ece13e099c3611bac7b39f",
        "rgb_sha256": "4371170b5239419060ed559afe13157740d69ef2aee0592cf4fc71c47dff58a5",
        "size": [8, 8],
    },
    "portable_lossy_420_q99_gray_131_control.avif": {
        "file_sha256": "ff49a749f44a139b697671a2c21032ff0a3298a0fb749ed7d9a7c193fbbeacfb",
        "rgb_sha256": "d8044c92ef2a961ebee78d49908caae12338872a8cb36675ef6dbfb0f244e2e9",
        "size": [4, 4],
    },
    "portable_lossy_420_q99_8x8_gray_131_control.avif": {
        "file_sha256": "f238b91f4c6b225691933fc5a46a1c2b42dd2460bdc3567a92dcd907fb8ac7bb",
        "rgb_sha256": "831ca0567d6d09bf16b7c76da27026347d9000d12ca92f486dd9c56b4226055e",
        "size": [8, 8],
    },
    "portable_lossy_420_q99_gray_132_control.avif": {
        "file_sha256": "98ee27816a74ee14b345e4a3c39856a328f18d77c7bbba95e40630b335bf44dd",
        "rgb_sha256": "03a471cd2fdf8af4107b02673eec688e6c7bc946d184af0c514af6a206e51681",
        "size": [4, 4],
    },
    "portable_lossy_420_q99_8x8_gray_132_control.avif": {
        "file_sha256": "adeca8ec9e6cbe47fc2a7a046d631be33772e385f868d7d943d99175e6535c32",
        "rgb_sha256": "603bfc293271617bfea86708fcd2820aa7246d3d73a47fd2c0184de328c68ab3",
        "size": [8, 8],
    },
    "portable_lossy_420_q99_gray_133_control.avif": {
        "file_sha256": "536cd711fe24a5c63489ecefc3f53d3a732aa606ebb3cb94a00789a5b4d9798d",
        "rgb_sha256": "7f0d7099d13d4903645f8fc327e2f0fe46fd9655a948fbc375024f82acc50fc2",
        "size": [4, 4],
    },
    "portable_lossy_420_q99_8x8_gray_133_control.avif": {
        "file_sha256": "6abbf10ccf33392f217a6db1e1b9a66cd6b0cea9e95d06845252a0389beaa029",
        "rgb_sha256": "108f70bd32bd6aa8f4d1a6ee9450a6505f16158b350b293f7e37ca87724ae29a",
        "size": [8, 8],
    },
    "portable_lossy_420_q99_gray_134_control.avif": {
        "file_sha256": "88a3a51f1107ca20a77bd70db89891e9431dd932914a2e4494d017e11018ca68",
        "rgb_sha256": "96a0187338028cdac12765e42d24b4cf369039db628878c674d273bdb0af4324",
        "size": [4, 4],
    },
    "portable_lossy_420_q99_8x8_gray_134_control.avif": {
        "file_sha256": "65fe71943e62a346b20249a420f323dab9601ba99cbb5bf9782074d0d16a6331",
        "rgb_sha256": "d4ea4542b1b805cc3f636afb8bf16a483cc0fe47a40b4fba4c876ebb26432b2a",
        "size": [8, 8],
    },
    "portable_lossy_420_q99_gray_192.avif": {
        "file_sha256": "8b517a977c091cbe56ec1997907c27706ba9bdd6c660e646d49df8a6dd16677f",
        "rgb_sha256": "af14d74c13f430d78f29de7246b5cbdf0937adbeb872ffe6dcf68282860d7cba",
        "size": [4, 4],
    },
    "portable_lossy_420_q99_8x8_gray_192.avif": {
        "file_sha256": "5edccf35d44da2f17d41b106681b7535f264863d44e26c0c1e16d1a67bd6e8f9",
        "rgb_sha256": "6845b27f00c23448c01b082d69fdf01aae50f11e3f0b29b073dfe5e6b864c36b",
        "size": [8, 8],
    },
    "portable_lossy_420_q99_gray_255.avif": {
        "file_sha256": "e1c3b423417b18795071054196ce1f95e6cf19a841a632c616ab3a96969d6e3f",
        "rgb_sha256": "80a76a18acf8cb64fec3a659ffc4bab4a87cd9a6fde4dab2161a8751d136c9d2",
        "size": [4, 4],
    },
    "portable_lossy_420_q99_8x8_gray_255.avif": {
        "file_sha256": "cf7660907939a12972c8ba2def48cb0b8b6014cc24bd75ab82cd0ffe1162f6c5",
        "rgb_sha256": "8f62c344eff1568474fb693b8c18526629db443b9653a84264189c97693605de",
        "size": [8, 8],
    },
    "portable_lossless_420_leaf_4x8_a.avif": {
        "file_sha256": "31aae6e6395da7d749786b00c339ace12d29af7acbfa7d9710bca10d9d92346e",
        "rgb_sha256": "116d1d3509d9d2a7558a2fad832f923fc1193f04b8e0e57946f49e57fa045475",
        "size": [4, 8],
    },
    "portable_lossless_420_leaf_8x4_a.avif": {
        "file_sha256": "7108ddc6197b99e99d89f1327108cf070ff051d32cac02b82eae1531feb0daf7",
        "rgb_sha256": "116d1d3509d9d2a7558a2fad832f923fc1193f04b8e0e57946f49e57fa045475",
        "size": [8, 4],
    },
    "portable_lossless_420_rect_12x4_gray_127.avif": {
        "file_sha256": "d8bf37e044015315531fa44a412619bba0eede149b2caae9baeae3e0175d9b3f",
        "rgb_sha256": "35fc07c937c1c3d13641f32cdc94ce1315ec420dd26e12b81a4651cfc1786ee3",
        "size": [12, 4],
    },
    "portable_lossless_420_rect_16x4_gray_127.avif": {
        "file_sha256": "bf3bc36ebd94d157ea028e41d12077ffec574d5b9ca6e115b3947a55f81f7580",
        "rgb_sha256": "c24e73f000a4255a612416ecc4df81c9313e4c099877384712e4d8530dd7acbd",
        "size": [16, 4],
    },
    "portable_lossless_420_rect_12x8_gray_127.avif": {
        "file_sha256": "b58a1b66e5dcd33c1686f072634c0e5f0662eb67dd0a8e3833303d4d7ad57808",
        "rgb_sha256": "cf8691a9b8c6c8e329b94f40345d822ef7d4f6e8e5c2343d74b12aa16e84838a",
        "size": [12, 8],
    },
    "portable_lossless_420_rect_16x8_gray_127.avif": {
        "file_sha256": "ae83d9122ffad59a687f03e74e4dd2d78a08b6a5693cae0e67e299545584fe2b",
        "rgb_sha256": "7e18f1b2ca4e075b955848b4deafd56e47eeda83cc15b3ecdeb71d7ff58a5f57",
        "size": [16, 8],
    },
    "portable_lossless_420_rect_4x12_gray_127.avif": {
        "file_sha256": "1020c7340e5d9079777e7522229f30b1058817138acefc83330ad3e22c6a9010",
        "rgb_sha256": "35fc07c937c1c3d13641f32cdc94ce1315ec420dd26e12b81a4651cfc1786ee3",
        "size": [4, 12],
    },
    "portable_lossless_420_rect_4x16_gray_127.avif": {
        "file_sha256": "8f5adda734549c4e0f7b88055f3819c553cb62f4ce902c5bb4e4a952cdf1f2d4",
        "rgb_sha256": "c24e73f000a4255a612416ecc4df81c9313e4c099877384712e4d8530dd7acbd",
        "size": [4, 16],
    },
    "portable_lossless_420_rect_8x12_gray_127.avif": {
        "file_sha256": "ad7c2d567edfa34b7988a64a18b19efe129c4f19ae67ab0265d58fecb654a10c",
        "rgb_sha256": "cf8691a9b8c6c8e329b94f40345d822ef7d4f6e8e5c2343d74b12aa16e84838a",
        "size": [8, 12],
    },
    "portable_lossless_420_rect_8x16_gray_127.avif": {
        "file_sha256": "076d56c74ee714f01d26d21177c95c88be85157ad0a5d612ab94c3365f3a8520",
        "rgb_sha256": "7e18f1b2ca4e075b955848b4deafd56e47eeda83cc15b3ecdeb71d7ff58a5f57",
        "size": [8, 16],
    },
    "portable_lossless_420_split_12x4_a.avif": {
        "file_sha256": "5fadea5fcf4a48c7b77ea0a89761263516a6fde5472f1ab1b42d85e4a8bc1782",
        "rgb_sha256": "09fddd84398ad9a9d3ce8b981fea278a82e6b1fa62483fa0ef3c45cd484ae29e",
        "size": [12, 4],
    },
    "portable_lossless_420_split_16x4_a.avif": {
        "file_sha256": "c262f88b8bae4ee384b69c705dfbe42d2dae6601c61b23b1d64b1e59db25be73",
        "rgb_sha256": "1f403e7f414473b888fcba438d60d269e54fc1d04c802dd32f96fa657932b2ac",
        "size": [16, 4],
    },
    "portable_lossless_420_split_12x8_a.avif": {
        "file_sha256": "25ab515d0bdde387c97d6bf9b44b33e8327bf8642c4738c3ba424297b5a41ccb",
        "rgb_sha256": "47c4a5d65d8ac82aa68f04754b38e5bf00438aeb64b2e48c2bb54a9268e6e4e7",
        "size": [12, 8],
    },
    "portable_lossless_420_split_16x8_a.avif": {
        "file_sha256": "325a6e737bd018076105cd3a22cc48d6b9c1d7b9dc0d9b29d6d749e6295de0b8",
        "rgb_sha256": "983aef668db1ea0d5801725fdf2b49d32232fc7f1d9ae578a03ffad6aebc4fc2",
        "size": [16, 8],
    },
    "portable_lossless_420_split_4x12_a.avif": {
        "file_sha256": "633040bba8ebb2c38a0201783869474b2867a79c19759eb70e8930ffb517c2cd",
        "rgb_sha256": "09fddd84398ad9a9d3ce8b981fea278a82e6b1fa62483fa0ef3c45cd484ae29e",
        "size": [4, 12],
    },
    "portable_lossless_420_split_4x16_a.avif": {
        "file_sha256": "fc8ad1c44445df13afee7f176501fa754b1ea094cef9734ea8258281897b795b",
        "rgb_sha256": "1f403e7f414473b888fcba438d60d269e54fc1d04c802dd32f96fa657932b2ac",
        "size": [4, 16],
    },
    "portable_lossless_420_split_8x12_a.avif": {
        "file_sha256": "539783b6e6c6ad4b54ef8e0f1f445c5f6e38b82b37529ab88a891c86958d17fd",
        "rgb_sha256": "47c4a5d65d8ac82aa68f04754b38e5bf00438aeb64b2e48c2bb54a9268e6e4e7",
        "size": [8, 12],
    },
    "portable_lossless_420_split_8x16_a.avif": {
        "file_sha256": "995ac2a192f5e08af7535ee8151cb98386e82bb696abd99afd0204665b0b1da0",
        "rgb_sha256": "983aef668db1ea0d5801725fdf2b49d32232fc7f1d9ae578a03ffad6aebc4fc2",
        "size": [8, 16],
    },
    "portable_lossless_420_square_12x12_a.avif": {
        "file_sha256": "d1f328bb548b6d0911ed6c2125fa8d26ed2a72738c081040a9e64990c916adf3",
        "rgb_sha256": "cbc97cf0c2652e60e6e36611be9869444f603abf5f48b292a03d340f501320f8",
        "size": [12, 12],
    },
    "portable_lossless_420_square_12x16_a.avif": {
        "file_sha256": "9b93917ebc8120ce0d3f7ed5c8e9b41f1d5dc4afb248647c694ff1b634d4623b",
        "rgb_sha256": "f6b42085d682a064da2a9956545f33ae7595b288f7589e8e498c62e6bc26e874",
        "size": [12, 16],
    },
    "portable_lossless_420_square_16x12_a.avif": {
        "file_sha256": "1124115b0edb90a5b751e11b502f07788d03edf0bc3305ca6fb3f1a018ce4f9e",
        "rgb_sha256": "f6b42085d682a064da2a9956545f33ae7595b288f7589e8e498c62e6bc26e874",
        "size": [16, 12],
    },
    "portable_lossless_420_square_16x16_a.avif": {
        "file_sha256": "bde1f73324f6b1bd1ec41ed68ecf9a15d0ada9d7e3508ef70e54fe9216ebd73a",
        "rgb_sha256": "8bdcc97ae19b09ec3d6b76a7d59f13d4aa3dd7a06d21db706f2a1d15caaa0431",
        "size": [16, 16],
    },
    "partitioned_square_420_16x16_rgb_delta.avif": {
        "file_sha256": "9cb30c2c2391c414c5dfef0a0ed27d9409089f88cdd05aad45103e720b6b12f7",
        "rgb_sha256": "33170bbddccc8cf1c2ce5dada1ab0dc1c510fc9b059ede87dff076f9df47e18d",
        "size": [16, 16],
    },
    "partitioned_square_420_16x16_g96.avif": {
        "file_sha256": "7e66769bff63133cbab59a6d93aa143f4d2f0982fa142567dfc4727783c3330a",
        "rgb_sha256": "1773a465660162ba2a563e2b05acb59d0ccd578de177210f9252a9abd2013bcf",
        "size": [16, 16],
    },
    "portable_lossless_gray_32.avif": {
        "file_sha256": "f57c5df28dc28add5b9913c9d3cc0c0aae2e69e0087e7a8614674c8658987875",
        "rgb_sha256": "b4a53f2b248b5701814756a08eb3435e49117eda791610ff85dd22e8a6a86df3",
        "size": [4, 4],
    },
    "portable_lossless_gray_127.avif": {
        "file_sha256": "40de5aecb3fb4c8b6ad9e242beda63204aae00f6be34694c14554aab91c330f8",
        "rgb_sha256": "a1fa26e9a041c510e9f8412accef2e5e0cda5eddd97fa6db80b30400b7964d42",
        "size": [4, 4],
    },
    "portable_probe_gray_128.avif": {
        "file_sha256": "26713256cc2769ab320d6017dca8b0f822dbdb03e66351e8ebc37bda64e440dc",
        "rgb_sha256": "2ac4dd6f486e2f061ebe8ce8b651dbdf25d71b88184d0bf308608cdcaae05309",
        "size": [4, 4],
    },
    "portable_probe_gray_129.avif": {
        "file_sha256": "649c4e452a350a51a9230070d768e432ff03bf7b3fb5700a50653f5f2887fc7a",
        "rgb_sha256": "b34e1e1e7cd63c9fb7069154ccd855d827a3dd3eca076232b4217745a2b6db57",
        "size": [4, 4],
    },
    "portable_lossless_8x8_a.avif": {
        "file_sha256": "b7f758da88a2a9835bcda1a709b1de1ce47e232113d7d67f027ec430bb089714",
        "rgb_sha256": "1f403e7f414473b888fcba438d60d269e54fc1d04c802dd32f96fa657932b2ac",
        "size": [8, 8],
    },
    "portable_lossless_8x8_gray_127.avif": {
        "file_sha256": "1b6a333257e226a63b6b33b34da54917a02518cf91864618222f60c160a883b7",
        "rgb_sha256": "c24e73f000a4255a612416ecc4df81c9313e4c099877384712e4d8530dd7acbd",
        "size": [8, 8],
    },
    "portable_probe_8x8_gray_128.avif": {
        "file_sha256": "6a4f26af5a873630c21a85fa1b7a1337026991acac0a305ecd6dd059a84fba63",
        "rgb_sha256": "fa7b78cc215df21d7ce54d8c3c6637c326dab95c10fbc12263101365973f4268",
        "size": [8, 8],
    },
    "portable_probe_8x8_gray_129.avif": {
        "file_sha256": "43ab3fc61b01b3b173323da584abe8a07c06eb4fcf4254cf8e04f3333f654237",
        "rgb_sha256": "fca06fef259b9ebb452449c7feda724ccec06a4a76b2b4fb1e6420a0beac435e",
        "size": [8, 8],
    },
    "portable_lossless_4x8_a.avif": {
        "file_sha256": "cb0b3da2e8a31551ba34dc01d126a69aa9db266f5403a035291df0dac6c6e618",
        "rgb_sha256": "116d1d3509d9d2a7558a2fad832f923fc1193f04b8e0e57946f49e57fa045475",
        "size": [4, 8],
    },
    "portable_lossless_4x8_gray_127.avif": {
        "file_sha256": "ce82141d38ba572468c4ca15dec6e34f21a95597e58c37ffc48eefd941f9b24d",
        "rgb_sha256": "faa8c27b41b2603cd12911cd93ee3953ff1f98c9fba83fdeef738cc8406c4b3f",
        "size": [4, 8],
    },
    "portable_probe_4x8_gray_128.avif": {
        "file_sha256": "4f4617e81863740af3f2342f115263e403318540e6766832956fd465a95bc832",
        "rgb_sha256": "1b34669db94decae583e183ee2ffeb07cf504b9f52fae0056c5cf343325157e4",
        "size": [4, 8],
    },
    "portable_probe_4x8_gray_129.avif": {
        "file_sha256": "0445afe75c7fdf364f93977071cca852eac150689c6cee27caa3756d77349736",
        "rgb_sha256": "780832a7ab39814257a857d37a67ab541a1152afbcf6a1883a16ad32c264ff4e",
        "size": [4, 8],
    },
    "portable_lossless_8x4_a.avif": {
        "file_sha256": "1a63667e7dc169346398a887b7e058e35908001823c5e2f88e591b292f9b15e8",
        "rgb_sha256": "116d1d3509d9d2a7558a2fad832f923fc1193f04b8e0e57946f49e57fa045475",
        "size": [8, 4],
    },
    "portable_lossless_8x4_gray_127.avif": {
        "file_sha256": "5bab699fbac28f885286c88d9dfc89c7211851fa508466ad1983400281c3bffe",
        "rgb_sha256": "faa8c27b41b2603cd12911cd93ee3953ff1f98c9fba83fdeef738cc8406c4b3f",
        "size": [8, 4],
    },
    "portable_probe_8x4_gray_128.avif": {
        "file_sha256": "fbe50a5bede60325dc8d41f980167c44085d6d4a4bcfbd7c8773eb36693aaef8",
        "rgb_sha256": "1b34669db94decae583e183ee2ffeb07cf504b9f52fae0056c5cf343325157e4",
        "size": [8, 4],
    },
    "portable_probe_8x4_gray_129.avif": {
        "file_sha256": "1a42be7e964815aea58848948cf8b79272f48161449319f57d2ad49b928a68e9",
        "rgb_sha256": "780832a7ab39814257a857d37a67ab541a1152afbcf6a1883a16ad32c264ff4e",
        "size": [8, 4],
    },
    "portable_lossless_12x12_a.avif": {
        "file_sha256": "daccd674b98dc26baad851ef95d75e6099c0397db5d8e28fb7f5f1f6eef9ac6c",
        "rgb_sha256": "cbc97cf0c2652e60e6e36611be9869444f603abf5f48b292a03d340f501320f8",
        "size": [12, 12],
    },
    "portable_lossless_12x12_gray_127.avif": {
        "file_sha256": "6120db571a64651ce6a3579863b808a9f68d60474235fb80d32c53448990ab0b",
        "rgb_sha256": "cb4987527501d0915664b8e624e5f51ebbf5f48b52917058615c1f3b96764076",
        "size": [12, 12],
    },
    "portable_probe_12x12_gray_128.avif": {
        "file_sha256": "fd155de27375f8569d97e0e0934d3f287b7ad41a18933dd98b4c9c95647e483a",
        "rgb_sha256": "cc0fcf371bdd305ff6099895e60aac93968bf0358724de1678979a37a9bd7a17",
        "size": [12, 12],
    },
    "portable_probe_12x12_gray_129.avif": {
        "file_sha256": "1e9c6b6a39e1fe7b73686fe6d1ac75770abaef9e28b173ec938a1937de02bb78",
        "rgb_sha256": "143efd9552ea35a74333bbfc58d10ae5a0eccfe76d2283c05b2b4a9391c346cd",
        "size": [12, 12],
    },
    "portable_lossless_16x16_a.avif": {
        "file_sha256": "04de5e1b6e056c08fdb33131d04dec6708b1ee4912c515fda6d973a29e592381",
        "rgb_sha256": "8bdcc97ae19b09ec3d6b76a7d59f13d4aa3dd7a06d21db706f2a1d15caaa0431",
        "size": [16, 16],
    },
    "portable_lossless_16x16_gray_127.avif": {
        "file_sha256": "db4305845c2773ff873835b6d401f77635e6f5aafab08204486caf602945fd49",
        "rgb_sha256": "cbab715ff6cfaa81c9b09e014dc1406ceff24034caa265de65f9f948c5434807",
        "size": [16, 16],
    },
    "portable_probe_16x16_gray_128.avif": {
        "file_sha256": "eca269fc4be5813d9865a0b8c0db9fd652c58474cc583a208e15bcbaab0bd7cf",
        "rgb_sha256": "7f3e5e4e65eca4390e9242558012bc9bdad133d7ac9f6aed53fa156a2288f73b",
        "size": [16, 16],
    },
    "portable_probe_16x16_gray_129.avif": {
        "file_sha256": "543643938a746a7e68daf23c118fab0673dcbc36de80eddf8d1603940814015c",
        "rgb_sha256": "15dc2c3b0ea25a84b4994b9a73dbcf65eef174bad152c689cc1945843b543657",
        "size": [16, 16],
    },
    "partitioned_square_12x12_g96_direct_tokens.avif": {
        "file_sha256": "b61f62f12306af9744ea06ac8c68bfd86f8b10f27caca820405b295756a3f194",
        "rgb_sha256": "8fd169458756409edfaf3380195c6ab881e3d7043d5c3b158a82feaaa82b993f",
        "size": [12, 12],
    },
    "partitioned_square_12x12_midpoint_g96_ac.avif": {
        "file_sha256": "d10972f944777129121ef100ee66903959138ae946295bb5fe271cef8035b258",
        "rgb_sha256": "1d316f3236ecba0ebb2e4483622a7dbaa736686fc6ce609a44c3e7c7380a0ff4",
        "size": [12, 12],
    },
    "partitioned_square_12x12_top_left_luma_eob4.avif": {
        "file_sha256": "fbc5e3cec5da21a1c1095ecf82525dac5d6ae60ff4a71b101502392de754cc45",
        "rgb_sha256": "fcfe3605207a28cd1596ae0cb2b9b4ad1b8b356f7457cd2e60276b8d6530a691",
        "size": [12, 12],
    },
    "partitioned_square_12x12_top_left_luma_eob12_control.avif": {
        "file_sha256": "b8b703ee9e1f2d8200fea338ee85f7ada1b905539bb163712209f60d83af0713",
        "rgb_sha256": "16195f9646d15f2857da1864cbffdd3f12a965bbd287ca888b7dde113c2d7ec7",
        "size": [12, 12],
    },
    "partitioned_square_12x12_luma_eob1.avif": {
        "file_sha256": "db9102a9b302387df2214814ac2cd02c8414beaf4751f3f374370237a210e9bc",
        "rgb_sha256": "d8ddfb34c1d4da25851a33b0515d025bd092a6bfd942eeda21683b9e564d6691",
        "size": [12, 12],
    },
    "partitioned_square_12x12_luma_eob2_control.avif": {
        "file_sha256": "89842483e159b7d9d98f58282679f9b6d09f4e164576270e9546b13df176c986",
        "rgb_sha256": "13878ffdf1168508a15759ff58c897370e8428fe522422d52149126a9cc42ef4",
        "size": [12, 12],
    },
    "partitioned_square_12x12_luma_eob4_control.avif": {
        "file_sha256": "307512d55df127d8546273a57dedd182fbdb5282aa830191f7fe201b8eff419f",
        "rgb_sha256": "299dc7d8cf7b620bb3cc3a56ab17da5414d8377e0b79196fce64cae0e05ca7f3",
        "size": [12, 12],
    },
    "partitioned_square_12x12_luma_eob6_control.avif": {
        "file_sha256": "90583dd6d88fce42d0cfdb8f9e7217d02d5a711f873955a04526dd99f5886efa",
        "rgb_sha256": "84c006c2c0f8e322453101374baeb3c0f1e30653b7960fb1068cfc8f33c96e68",
        "size": [12, 12],
    },
    "partitioned_square_12x12_luma_eob9_control.avif": {
        "file_sha256": "d57ddb0c7dbcdfc63aa77f3bdbd64a793246451528dfec553c0bf800f8137d4b",
        "rgb_sha256": "7b69d30ebe2894d11aa6d4f7c3385c8675a4cf8daf702d5b6cd709a6001ce506",
        "size": [12, 12],
    },
    "partitioned_square_12x12_luma_eob10_control.avif": {
        "file_sha256": "2cb6cfd94fb6cfaf62375d0c7c9dd51b9193d4b3740b31a62750b14ddc39e072",
        "rgb_sha256": "edb3552022d80b01938371e9e0d78ea4544d2b1bab41cfe67253a89458774264",
        "size": [12, 12],
    },
    "partitioned_square_12x12_luma_eob12_control.avif": {
        "file_sha256": "52293589be5deed92756c5e11447b571686381ec3c873dbb9e5a221b91eb820c",
        "rgb_sha256": "a98fa8dc8ff3ed903815016c02089c888bee48bfb8774903c8bf70d57aed2735",
        "size": [12, 12],
    },
    "partitioned_square_12x12_luma_eob15_control.avif": {
        "file_sha256": "3265cf40613523eab69cba5ae73af453f781a29ab3b36f13c21b6720a4d42d7a",
        "rgb_sha256": "2d41c17b74e78417fd7ab3fdb5da3225f52c4035e39133275ee01496cc21a77a",
        "size": [12, 12],
    },
    "partitioned_square_16x16_g64.avif": {
        "file_sha256": "4a8703a56c56a2d6cbcdbec90e12d266fc28603db1f84e725f7f1a75f504fed7",
        "rgb_sha256": "d7efc58f710522b0c6e2609ab53339cf9aa4c3c419b4023593bffd94fcb883fe",
        "size": [16, 16],
    },
    "partitioned_square_16x16_g96_direct_tokens.avif": {
        "file_sha256": "1fcdc276a8521a7d248fa9382aca518c880921615a392d6116e3fff28320032d",
        "rgb_sha256": "87cf9f38f5bc4a0a75c3284ff3b5826e0c0734066e863bcf416f2296623b890f",
        "size": [16, 16],
    },
    "partitioned_square_16x16_r64.avif": {
        "file_sha256": "fe7610630b212d87a5b9b9650fa156be9729e1bd49d8c01df5df416e5e524898",
        "rgb_sha256": "6492bb904bafc0a5c8acedff1fd7cd70965e3be844e8fd19d0e04a6bd63e2017",
        "size": [16, 16],
    },
    "partitioned_square_16x16_g127.avif": {
        "file_sha256": "4085fdb230e1bcc93a3a3be408d5fbbf0a5c740590df3983c07b191d3b59ba08",
        "rgb_sha256": "d1ce3617b6228d74d2b208847c20486f1a6301cf8b0708242c0019894eeb055e",
        "size": [16, 16],
    },
    "portable_lossless_12x16_a.avif": {
        "file_sha256": "67e0005a989d761d36df0ddb12e53f1535a6a04e3606b97dfd33829949bc30ca",
        "rgb_sha256": "f6b42085d682a064da2a9956545f33ae7595b288f7589e8e498c62e6bc26e874",
        "size": [12, 16],
    },
    "portable_lossless_12x16_gray_127.avif": {
        "file_sha256": "4597c81363e602580e94625c7a01b029201a533489f404465698a194b9531e31",
        "rgb_sha256": "1b9924ee11c55d5fd4d944003b8b272c1f4ce12ea8e800c33563bed483fa406d",
        "size": [12, 16],
    },
    "portable_probe_12x16_gray_128.avif": {
        "file_sha256": "3406413b66b7f10d4d760a26b62f5f74c9f13d7a245557b8ba79fd340614b7a4",
        "rgb_sha256": "af1857bf5516aa3e2e39b6842559746fa7b45daa8dc4cc6675ad86e0cfe425b9",
        "size": [12, 16],
    },
    "portable_probe_12x16_gray_129.avif": {
        "file_sha256": "eac42af171713fab4c6535de7f3c1874b5c9f14669bcb36b8c5df53b927bfa9f",
        "rgb_sha256": "5269c00892aff8abcc6a4da60b82b890936aef6b1aa24c6b713c5a80a831c0b9",
        "size": [12, 16],
    },
    "portable_lossless_16x12_a.avif": {
        "file_sha256": "423d91243ff4e7d42bb9b77cf255dab869e31e3e12a306f139c2c73a3df9a807",
        "rgb_sha256": "f6b42085d682a064da2a9956545f33ae7595b288f7589e8e498c62e6bc26e874",
        "size": [16, 12],
    },
    "portable_lossless_16x12_gray_127.avif": {
        "file_sha256": "5df228fa447ee7f788c5bf486520f1d51ecaff4db147dbc86c7d9a804b40b60a",
        "rgb_sha256": "1b9924ee11c55d5fd4d944003b8b272c1f4ce12ea8e800c33563bed483fa406d",
        "size": [16, 12],
    },
    "portable_probe_16x12_gray_128.avif": {
        "file_sha256": "f5873bf081f14f72dad784bf4c2835d42ee4a68482da4f964a0d4ea7902d8a12",
        "rgb_sha256": "af1857bf5516aa3e2e39b6842559746fa7b45daa8dc4cc6675ad86e0cfe425b9",
        "size": [16, 12],
    },
    "portable_probe_16x12_gray_129.avif": {
        "file_sha256": "ca46b221a159dc66999b184e1655f696ad4ef05512d200591f9769ef0851424b",
        "rgb_sha256": "5269c00892aff8abcc6a4da60b82b890936aef6b1aa24c6b713c5a80a831c0b9",
        "size": [16, 12],
    },
    "partitioned_12x4_a.avif": {
        "file_sha256": "631fe6bf1c2f72cc60acdbb5e682d8195c512f67a5c8f38bd6ae10fdb7a8c59d",
        "rgb_sha256": "09fddd84398ad9a9d3ce8b981fea278a82e6b1fa62483fa0ef3c45cd484ae29e",
        "size": [12, 4],
    },
    "partitioned_12x4_gray_32.avif": {
        "file_sha256": "b2d4d2b81939374c9e11cc4a57b3ec0cc9212e2e1eb8d199c6c411cf41d5360b",
        "rgb_sha256": "31178565d9d883446d9e273ee881220f43cb4c5de74e237f590f845e25659f38",
        "size": [12, 4],
    },
    "partitioned_12x4_green.avif": {
        "file_sha256": "f0e1f0effed11bfde68129920f87590ee7af0962aca855272cbf1a1c586b9f62",
        "rgb_sha256": "7f5e545c140df34ec243d4449ab8c4c0e476f532d3f6472ce956e7060b271e1c",
        "size": [12, 4],
    },
    "partitioned_16x4_a.avif": {
        "file_sha256": "1d6bd1273fb2890b27dd1999e0766220059d99cd3973da859a0d13070b068dc4",
        "rgb_sha256": "1f403e7f414473b888fcba438d60d269e54fc1d04c802dd32f96fa657932b2ac",
        "size": [16, 4],
    },
    "partitioned_16x4_gray_32.avif": {
        "file_sha256": "5603b8d4ff018f442ca8c2008a5f4d75b1877094f65adc121d8a634059ba359e",
        "rgb_sha256": "1d3659ada1bf4b80ae974a7b544090591793cb954ac3f9ad13d3af3f09c21967",
        "size": [16, 4],
    },
    "partitioned_16x4_green.avif": {
        "file_sha256": "065ca6683c44348263bb922862a2dfd459650b2929464acc3e71a56b09cafa85",
        "rgb_sha256": "32e7c45e59200de4c1012eac0ef31f3fa35d02b40d563f4602644bca9266f7fc",
        "size": [16, 4],
    },
    "partitioned_12x8_a.avif": {
        "file_sha256": "3c84461626938d85b0b51798f18666e9476c881d5a0f1289b583cd6ed691a7bf",
        "rgb_sha256": "47c4a5d65d8ac82aa68f04754b38e5bf00438aeb64b2e48c2bb54a9268e6e4e7",
        "size": [12, 8],
    },
    "partitioned_12x8_gray_32.avif": {
        "file_sha256": "1ec10cc4930d5934d77ffda0fceef577daae0c3e003f08ad315b8ada4a9d6f26",
        "rgb_sha256": "a80ec409692fd6c32b82fa895a118a06751d63671cd6da6ed14ef5bb59f41541",
        "size": [12, 8],
    },
    "partitioned_12x8_green.avif": {
        "file_sha256": "a1ade0b1491bfbb45a220dabe3dd7199df0d7c43d0bac44b538fa88e78e74b3b",
        "rgb_sha256": "c1046797ae8db85c1b32d232085bdc2251d6e94567771f20ce9f86b6a2cc5cbc",
        "size": [12, 8],
    },
    "partitioned_16x8_a.avif": {
        "file_sha256": "ed6197a68ca5ce65a447217beddfd5c4ef89e3e370579727f75b364263c5cd56",
        "rgb_sha256": "983aef668db1ea0d5801725fdf2b49d32232fc7f1d9ae578a03ffad6aebc4fc2",
        "size": [16, 8],
    },
    "partitioned_16x8_gray_32.avif": {
        "file_sha256": "b46eac4615030f89bf2e9027d3763e7059cf9c80788c4a3e0fd32cbdef85cb8a",
        "rgb_sha256": "f89d41f00d89e8b0bf8cb8cff89f9f23e9fa1e5113473dda8d16098575db7388",
        "size": [16, 8],
    },
    "partitioned_16x8_green.avif": {
        "file_sha256": "d39d76dad2e9a3c939db591ade9c6e655a06d8e6ada0ddc42c49a13089244840",
        "rgb_sha256": "ff87dfd10bc6c01f8e9dac23bb518192e6579a383b2ff1bbd8b8c80a58e677b4",
        "size": [16, 8],
    },
    "partitioned_12x4_gray_127.avif": {
        "file_sha256": "8050ba2808d418cb39c1462a71d061e261582b2f4dffc255a5cfc65cf26e1fd2",
        "rgb_sha256": "35fc07c937c1c3d13641f32cdc94ce1315ec420dd26e12b81a4651cfc1786ee3",
        "size": [12, 4],
    },
    "portable_rect_12x4_gray_128.avif": {
        "file_sha256": "4c8481a5800bcd2a54314d6fef24544240f601e818396b420495d5d84d623101",
        "rgb_sha256": "7053108d4e37b600ae17d35890c69102ee6484d79a3a5cd622afca6f5606c543",
        "size": [12, 4],
    },
    "portable_rect_12x4_gray_129.avif": {
        "file_sha256": "be3773c9d0cc723661c3275d786b4c3e5928bdc5db7cba32c1bf9c063f7885d6",
        "rgb_sha256": "c60b05f1911c0ccc80c5af2cd922c7cf1836279d44a17682c918cdaa5c7747e6",
        "size": [12, 4],
    },
    "portable_rect_12x8_gray_127.avif": {
        "file_sha256": "bb70428bb33c88bb106d407da50bd9358b2ea2db05f00164a3300abe80be9873",
        "rgb_sha256": "cf8691a9b8c6c8e329b94f40345d822ef7d4f6e8e5c2343d74b12aa16e84838a",
        "size": [12, 8],
    },
    "portable_rect_12x8_gray_128.avif": {
        "file_sha256": "a3949ecc8e13ffdea49ae02e58ef42876db36062b5ee194dac7ec6b0eaba737f",
        "rgb_sha256": "88f2f6050a4ef8c9fd8bd69d3e51689155f6aa570f0ac0da6d3c0ee794bf3867",
        "size": [12, 8],
    },
    "portable_rect_12x8_gray_129.avif": {
        "file_sha256": "e329d7d9e35a6674dc8badffd19e2ef6812af3914c9c6701569b16a94a6efba0",
        "rgb_sha256": "fe124f63ee1300955e9b2ffbed15cf383e9f4ae7c5cf60a09b074e4b0d73947f",
        "size": [12, 8],
    },
    "portable_rect_16x4_gray_127.avif": {
        "file_sha256": "55c0aaab5f00ea3d2c39703d4914c7a5bc1a59e73232c9771be551396fef2ebf",
        "rgb_sha256": "c24e73f000a4255a612416ecc4df81c9313e4c099877384712e4d8530dd7acbd",
        "size": [16, 4],
    },
    "portable_rect_16x4_gray_128.avif": {
        "file_sha256": "6918691db9f48f4c94cf42b75c61897f17b504043ccab34e25698bb9d971993e",
        "rgb_sha256": "fa7b78cc215df21d7ce54d8c3c6637c326dab95c10fbc12263101365973f4268",
        "size": [16, 4],
    },
    "portable_rect_16x4_gray_129.avif": {
        "file_sha256": "c7af47b14d2b54c9a4eae925f77723627f26f79c6e1fdb5012c9e1599ed91f5c",
        "rgb_sha256": "fca06fef259b9ebb452449c7feda724ccec06a4a76b2b4fb1e6420a0beac435e",
        "size": [16, 4],
    },
    "portable_rect_16x8_gray_127.avif": {
        "file_sha256": "91125e2b485b356da5284cb4112d57dd5509628137c203406d48c47153a63e13",
        "rgb_sha256": "7e18f1b2ca4e075b955848b4deafd56e47eeda83cc15b3ecdeb71d7ff58a5f57",
        "size": [16, 8],
    },
    "portable_rect_16x8_gray_128.avif": {
        "file_sha256": "691c0a4b301177f935791b4b5cc404f749096536d38128c7ab983e31dae0f65b",
        "rgb_sha256": "f83545d43c6939ec393b6b8310959b6174fd764b08a12fc22d908408a7e6a43e",
        "size": [16, 8],
    },
    "portable_rect_16x8_gray_129.avif": {
        "file_sha256": "97594ae3755cb1579095ab520fc986fb94d8de1782e67cd73db99ef48ffae9d7",
        "rgb_sha256": "7d965db8cbcf57e71b10b16973c9c2439222485594191da31460986a000f497c",
        "size": [16, 8],
    },
    "partitioned_4x12_a.avif": {
        "file_sha256": "0aa4e381b6412dd1ffa92a51e5bd4519ce038261485f59d1decd7ef5777690f8",
        "rgb_sha256": "09fddd84398ad9a9d3ce8b981fea278a82e6b1fa62483fa0ef3c45cd484ae29e",
        "size": [4, 12],
    },
    "partitioned_4x12_gray_32.avif": {
        "file_sha256": "0a20cbc9fca468a631f0ccee83080296e8ce51813eb97430fc686ccc6700d218",
        "rgb_sha256": "31178565d9d883446d9e273ee881220f43cb4c5de74e237f590f845e25659f38",
        "size": [4, 12],
    },
    "partitioned_4x12_green.avif": {
        "file_sha256": "ad3e35211dccfcefeff87fccb96319573a32d92f2d42c7564c39fbc88a9aee9c",
        "rgb_sha256": "7f5e545c140df34ec243d4449ab8c4c0e476f532d3f6472ce956e7060b271e1c",
        "size": [4, 12],
    },
    "partitioned_4x16_a.avif": {
        "file_sha256": "d2c62b8197dd5080a461202ff18b4c218e47f436eef05cc9856e6e7c1fec0245",
        "rgb_sha256": "1f403e7f414473b888fcba438d60d269e54fc1d04c802dd32f96fa657932b2ac",
        "size": [4, 16],
    },
    "partitioned_4x16_gray_32.avif": {
        "file_sha256": "a5e8c50931fdb5cf94f7d2d1a4edf114f23ea9fc83f782fb24a30f793035b7a0",
        "rgb_sha256": "1d3659ada1bf4b80ae974a7b544090591793cb954ac3f9ad13d3af3f09c21967",
        "size": [4, 16],
    },
    "partitioned_4x16_green.avif": {
        "file_sha256": "0e6cbfb32ea49625e8e43b221cc57f47cacbd9594a79786185b78ea4336253ed",
        "rgb_sha256": "32e7c45e59200de4c1012eac0ef31f3fa35d02b40d563f4602644bca9266f7fc",
        "size": [4, 16],
    },
    "partitioned_8x12_a.avif": {
        "file_sha256": "498c54f6c31f6b2ff0620da37b1e3fdd56760c7951fd1281c7c2baf21ade0bb8",
        "rgb_sha256": "47c4a5d65d8ac82aa68f04754b38e5bf00438aeb64b2e48c2bb54a9268e6e4e7",
        "size": [8, 12],
    },
    "partitioned_8x12_gray_32.avif": {
        "file_sha256": "d368148298780e652a20fbbf3a77d671bc72cf67d38f88b91b4a20ff4bcbdc9a",
        "rgb_sha256": "a80ec409692fd6c32b82fa895a118a06751d63671cd6da6ed14ef5bb59f41541",
        "size": [8, 12],
    },
    "partitioned_8x12_green.avif": {
        "file_sha256": "b11a0b802bddcb8654978d53395afef62afce9adc571a94f87b820c4f128da7d",
        "rgb_sha256": "c1046797ae8db85c1b32d232085bdc2251d6e94567771f20ce9f86b6a2cc5cbc",
        "size": [8, 12],
    },
    "partitioned_8x16_a.avif": {
        "file_sha256": "003d1203ec343319da24875b1c66d3be4626804300d36cff089f1efeb9145dae",
        "rgb_sha256": "983aef668db1ea0d5801725fdf2b49d32232fc7f1d9ae578a03ffad6aebc4fc2",
        "size": [8, 16],
    },
    "partitioned_8x16_gray_32.avif": {
        "file_sha256": "967f4e6c4d216cea64514c7de72ccb5b81b800ca62f928cb4e043fe06736982f",
        "rgb_sha256": "f89d41f00d89e8b0bf8cb8cff89f9f23e9fa1e5113473dda8d16098575db7388",
        "size": [8, 16],
    },
    "partitioned_8x16_green.avif": {
        "file_sha256": "fe5069db1c23f0335fcc9855de689932e3b1d195e16250f97ea6c07004394954",
        "rgb_sha256": "ff87dfd10bc6c01f8e9dac23bb518192e6579a383b2ff1bbd8b8c80a58e677b4",
        "size": [8, 16],
    },
    "partitioned_4x12_gray_127.avif": {
        "file_sha256": "a64e56b075bd724e0eeb25f29913010ff9fa3bd89fbfc3ba27eaa121fac0746f",
        "rgb_sha256": "35fc07c937c1c3d13641f32cdc94ce1315ec420dd26e12b81a4651cfc1786ee3",
        "size": [4, 12],
    },
    "portable_rect_4x12_gray_128.avif": {
        "file_sha256": "eb1fcc1d6176aae147183e8d292209c24431f6d45b2a63b5cb8710f9979ec79a",
        "rgb_sha256": "7053108d4e37b600ae17d35890c69102ee6484d79a3a5cd622afca6f5606c543",
        "size": [4, 12],
    },
    "portable_rect_4x12_gray_129.avif": {
        "file_sha256": "6d3be68bfdc174208f172e20bd309b352d9789b704249c34aba11eb96e3f5c31",
        "rgb_sha256": "c60b05f1911c0ccc80c5af2cd922c7cf1836279d44a17682c918cdaa5c7747e6",
        "size": [4, 12],
    },
    "portable_rect_8x12_gray_127.avif": {
        "file_sha256": "eeebe03575aa80c8be38b65671619d1a81f1be96dfcde917c47a7d602fe5bf51",
        "rgb_sha256": "cf8691a9b8c6c8e329b94f40345d822ef7d4f6e8e5c2343d74b12aa16e84838a",
        "size": [8, 12],
    },
    "portable_rect_8x12_gray_128.avif": {
        "file_sha256": "ff4d86a80c112eb02fd81848ee5107b82d55f95817511189a15558517a1a047e",
        "rgb_sha256": "88f2f6050a4ef8c9fd8bd69d3e51689155f6aa570f0ac0da6d3c0ee794bf3867",
        "size": [8, 12],
    },
    "portable_rect_8x12_gray_129.avif": {
        "file_sha256": "4e181377befea58e23cd989fdeffaeffd8d45ca08fe9ebb5d7cfa262eddf5678",
        "rgb_sha256": "fe124f63ee1300955e9b2ffbed15cf383e9f4ae7c5cf60a09b074e4b0d73947f",
        "size": [8, 12],
    },
    "portable_rect_4x16_gray_127.avif": {
        "file_sha256": "dcb0bd66d21c10ebc035dd4d598c24b6d92bf7cd19fc2b779678478820acf616",
        "rgb_sha256": "c24e73f000a4255a612416ecc4df81c9313e4c099877384712e4d8530dd7acbd",
        "size": [4, 16],
    },
    "portable_rect_4x16_gray_128.avif": {
        "file_sha256": "dd06b06c8cc9fa8416ce8debc24f7374c4f682f9aaf11b57cf9ce70af3e5a03a",
        "rgb_sha256": "fa7b78cc215df21d7ce54d8c3c6637c326dab95c10fbc12263101365973f4268",
        "size": [4, 16],
    },
    "portable_rect_4x16_gray_129.avif": {
        "file_sha256": "6b0e87f423614d87d5c30200327cb1960d9f34f11d499a6f61fe0dfd15bbe5df",
        "rgb_sha256": "fca06fef259b9ebb452449c7feda724ccec06a4a76b2b4fb1e6420a0beac435e",
        "size": [4, 16],
    },
    "portable_rect_8x16_gray_127.avif": {
        "file_sha256": "5596cf6f0e74c0dee066142ce6c1906a044c307d0cacdbba676e47d22d9d4487",
        "rgb_sha256": "7e18f1b2ca4e075b955848b4deafd56e47eeda83cc15b3ecdeb71d7ff58a5f57",
        "size": [8, 16],
    },
    "portable_rect_8x16_gray_128.avif": {
        "file_sha256": "c2606f2088bdffafe58340d972bca033935e418144eb6b697ba06e27e968ed1a",
        "rgb_sha256": "f83545d43c6939ec393b6b8310959b6174fd764b08a12fc22d908408a7e6a43e",
        "size": [8, 16],
    },
    "portable_rect_8x16_gray_129.avif": {
        "file_sha256": "907501fb1471801f5ef669630fcc9ec55932ecdd7df350624dcfd52058e31d66",
        "rgb_sha256": "7d965db8cbcf57e71b10b16973c9c2439222485594191da31460986a000f497c",
        "size": [8, 16],
    },
    "portable_rect_12x4_a_speed0.avif": {
        "file_sha256": "c1d9d3dd4532845f4fe5f337afaa768fcc54d57e1a69448950cce21a00175a06",
        "rgb_sha256": "09fddd84398ad9a9d3ce8b981fea278a82e6b1fa62483fa0ef3c45cd484ae29e",
        "size": [12, 4],
    },
    "portable_rect_12x4_gray_32_speed0.avif": {
        "file_sha256": "49f4f000e4670bbe3069871e5114e6f26531a7157d50a6c98907ee19f7bb4f88",
        "rgb_sha256": "31178565d9d883446d9e273ee881220f43cb4c5de74e237f590f845e25659f38",
        "size": [12, 4],
    },
    "portable_rect_4x12_a_speed0.avif": {
        "file_sha256": "89a2e559311b51e2e334f60a8026470991651af2466d051eff77d1b992708e0d",
        "rgb_sha256": "09fddd84398ad9a9d3ce8b981fea278a82e6b1fa62483fa0ef3c45cd484ae29e",
        "size": [4, 12],
    },
    "portable_rect_4x12_gray_32_speed0.avif": {
        "file_sha256": "c19f188da8ac6c2cdd160090038471363c949798fddb9586fb11a680ef8fb66a",
        "rgb_sha256": "31178565d9d883446d9e273ee881220f43cb4c5de74e237f590f845e25659f38",
        "size": [4, 12],
    },
    "coverage_adst_public_02.avif": {
        "file_sha256": "409a7c2deabad47673090ee3de9f361418f3128c87aa0b12d2ddc0d987c67d55",
        "rgb_sha256": "d872557591a66de992c9ecb7af416ac0c5d8dd364c0c26f1acc2ec530b75375f",
        "size": [8, 4],
    },
    "coverage_r8x16_band_05.avif": {
        "file_sha256": "e4b15a28de5a282b10bb99adb06562226bc8683246d81429dce8b31427ea420f",
        "rgb_sha256": "c11a94094afc690f85b60f373368af7995dca863a978e1835386df16567d5840",
        "size": [8, 32],
    },
    "coverage_r8x16_band_06.avif": {
        "file_sha256": "9e7410ee03a63a9598810155f20a3a372113c337321969513ad6c57b49fcb36c",
        "rgb_sha256": "70a7a0107bec2a81f759155aaf760088704eff6de4c628616a5173a3fb0df610",
        "size": [8, 32],
    },
    "coverage_r16x32_grid_01.avif": {
        "file_sha256": "fb8ee178f58141a3152894c5af033681c871e8562970b2fbab4b89dd487e824a",
        "rgb_sha256": "8a72d87e179a92b6fb293008f6fbfabc4df0ead6cd96311b1345f6f706c8eeac",
        "size": [32, 32],
    },
    "coverage_r32x16_origin_01.avif": {
        "file_sha256": "efa45a748283d03b65005365b0c6c281e5378a22369e0119add49536fc3bf8ba",
        "rgb_sha256": "0269cf259d6753f2ed578b701877c2fe4de42b3f2d812c168a079fc43b9d3328",
        "size": [32, 16],
    },
    "coverage_r32x32_following_01.avif": {
        "file_sha256": "28b9df05ad61bf01fef2ac11a6a9fa775ced07aed181e30c5c905caaedf4b6d3",
        "rgb_sha256": "da5131edb6e36e25f3604f7ff5eda45b4c796dcf4a06f2a4807cc9948e0827e7",
        "size": [32, 32],
    },
    "coverage_square32_origin_tx16x16_split_01.avif": {
        "file_sha256": "f4bf64e6de7a7265a1c5564324c812103135c043a05b7119ef4c97bf9892c987",
        "rgb_sha256": "6f55403182b74ed6bb0f581ebb3e53b6857d0a1934c0650923feac0a0e52b88b",
        "size": [32, 32],
    },
    "coverage_square64_origin_tx32x32_split_01.avif": {
        "file_sha256": "657fa025c72a516cd0e2a6a8c1a6670e270bebbb301f39bb6a6858db65ced555",
        "rgb_sha256": "be7eab35fabf3bd1032e7f1da118d4d4010584789051da47d0ae9500e8aeaa2c",
        "size": [64, 64],
    },
    "coverage_r32x32_filter_intra_probe_01.avif": {
        "file_sha256": "2c4eb6014ec79e58d5fbc79b8e89024fbf624b918c4decee0cef790d98914c56",
        "rgb_sha256": "979a9de4159e978b1fdbf2fb33f240da857c8a69107d635ca0a00550e459299b",
        "size": [32, 32],
    },
    "coverage_r32x32_filter_intra_mode3_01.avif": {
        "file_sha256": "79efb409fcfecd8cc3cd1fccb4ab22dd33190b6d91148524c03affaf9b809b29",
        "rgb_sha256": "8593fcb0b09a3d12243a6600505f3c77262e8103d453604099a29c500c1f9495",
        "size": [32, 32],
    },
    "coverage_r32x32_following_filter_intra_split_mode0_01.avif": {
        "file_sha256": "925c90b4341178968e1ed74c2abef6148b826c77be869730b6ca9b6f0cf8f1db",
        "rgb_sha256": "ea277bdded250f326c4dd7da3cd87e6ab514db4e14870857f5e79b5276a43e16",
        "size": [32, 32],
    },
    "coverage_r32x16_filter_intra_tx8x8_01.avif": {
        "file_sha256": "9e3f8459144572f29db4932816c957e237fb4911f4c45be7496dab681295b066",
        "rgb_sha256": "fe39183daabbf77ecbc191b4cb9b3fea01486b1fa28ccfef651372763ac975b8",
        "size": [32, 16],
    },
    "coverage_r16x32_following_filter_intra_split_mode3_01.avif": {
        "file_sha256": "cd15edc5af5d16f553595f9a81a35b472e6e37b4c933a471d613b380037a76f4",
        "rgb_sha256": "d135a06efafa72998c7c55dfa25f7ec0603cf9fa2231fd874ea10074234ea186",
        "size": [32, 32],
    },
    "coverage_r16x32_following_filter_intra_split_mode0_01.avif": {
        "file_sha256": "f903b64aa74c2d7d4132a43061af1e10ace4cbf1d9cc883043e223cc5de7ba54",
        "rgb_sha256": "cac42b39973f40158ad8fec42946726538adddb9a0d113ed0a16b054a9189272",
        "size": [32, 32],
    },
    "coverage_square16_filter_intra_mode0_01.avif": {
        "file_sha256": "2fb3de2676b560d379d05782b3e57c7af028b2fdac0350364389b3f9ceb77bcc",
        "rgb_sha256": "4090aed7681e287536328b3ec8ee9235c8e32979b8a249824d258fd57145b008",
        "size": [16, 16],
    },
    "coverage_square16_chroma_smooth_horizontal_01.avif": {
        "file_sha256": "f81f98994be3ecce57b31a721d80af4c75ad31acf2ea45ace4a5d05d677cf1f3",
        "rgb_sha256": "cbca1ceee34545f791090f42e152e5bfd495f4ab0cefcce6d943c57ec8edc144",
        "size": [32, 16],
    },
    "coverage_square16_chroma_smooth_vertical_01.avif": {
        "file_sha256": "66ed5a57015730ce80eb529483102fbe781d1d073e3443fa041177e38be8e380",
        "rgb_sha256": "76390242834678d6b4ecd14ec7b291b7fbec921a8c96f4c269ca5a67228ac258",
        "size": [32, 16],
    },
    "coverage_square16_chroma_smooth_01.avif": {
        "file_sha256": "1d663c7f7e3d65f12062124880ab1ae4d3eee5eaada570d9a38504aa58093080",
        "rgb_sha256": "04aa5e9f6facb7895149696ada7e559de9e44a50c13ac7be2db57d9fd1f273b6",
        "size": [32, 16],
    },
    "coverage_vertical8x16_filter_intra_mode0_01.avif": {
        "file_sha256": "da511e016e1e8720cb21af34b4cf41001a97af0f0380576dc47355dcd630f39a",
        "rgb_sha256": "82b2100ac5f6f02e88ea931a90b2abab261b7486209ee4f63c538464c52b5c30",
        "size": [8, 16],
    },
    "coverage_vertical8x16_filter_intra_mode1_01.avif": {
        "file_sha256": "7c04bf5be19e0e1acf757dbdda04b3fd48419a2df1dcf7a12871cdefbce99917",
        "rgb_sha256": "6051c012bac9735f10fb18bfe680fc9e3582ef6acfaa295a028f02ead7a642fe",
        "size": [8, 16],
    },
    "coverage_vertical8x16_filter_intra_mode2_01.avif": {
        "file_sha256": "a9a4a6ccb31aaed0164ce68ca9988fab9d8e8b0407e3e4e93de5dd0d53b48c41",
        "rgb_sha256": "5bf4eb2849056ecbba6885bbab1852d39449dec94909f05f6b26657b74104b8d",
        "size": [8, 16],
    },
    "coverage_square8_chroma_diagonal113_01.avif": {
        "file_sha256": "c014c0d3a2108ab2e97b3dd7575985dec029390b049d08335faa8b3d2aad31f7",
        "rgb_sha256": "05f6f725de2e882646a7bf059b444ffc26e2a7b048ad09f573890222bd029462",
        "size": [16, 8],
    },
    "coverage_square8_chroma_diagonal45_angle51_01.avif": {
        "file_sha256": "49a5be35748530ce5747f0f73f24d2e1e84f94a443c72274e92cfc605351655e",
        "rgb_sha256": "2b09c1b7c72c153a4ad6456a06bf63a6cd31b2b8952dcb8a78a714d0d6b0d08a",
        "size": [16, 8],
    },
    "coverage_square8_luma_diagonal_down_right_01.avif": {
        "file_sha256": "fddb447f61b8aa89d5d2bc4dee0baf8dd2c3711ade6d4384edb052841cf4940f",
        "rgb_sha256": "44a7d5e7b2c778b65ee4dbd1379b87a2fc33cca36b2a180519d68cfc34eea01b",
        "size": [16, 8],
    },
    "coverage_square8_luma_diagonal45_01.avif": {
        "file_sha256": "0c91a151906a907282309db84231ec2a502a7186fe7403803535871dacd5f0fc",
        "rgb_sha256": "86bf348ca94bf0609609d58aaff66a92b66834caf9c4261e697a6cb57863c01a",
        "size": [16, 8],
    },
    "coverage_square8_chroma_diagonal67_vertical_01.avif": {
        "file_sha256": "7251e37d120b6cd170d0f2de705b2e56cccda3dfbd3ea4384369132bd0ea0f3f",
        "rgb_sha256": "2c5534101754f03cecccf894872055062fba481fd0886fb68eb853a55b2cf2ae",
        "size": [8, 16],
    },
    "coverage_square8_luma_smooth_01.avif": {
        "file_sha256": "6efe46905dc699d37beef0620e5b935b2114367c7254b03d0c9c1c2b57c8d4d0",
        "rgb_sha256": "26372cd592790e77ea2738edb81af446a8ba366533779673d2031f4c3b7aa530",
        "size": [16, 8],
    },
    "coverage_square8_luma_smooth_horizontal_01.avif": {
        "file_sha256": "621dbd34cae09e8246e47cdfe2a3d993ec8572f3f64ef6ff8e7f2ef49beca1a4",
        "rgb_sha256": "db4447d10c5a73b65b8d7a5fba0331e9a457722c42171658c45c123101759e25",
        "size": [16, 8],
    },
    "coverage_square8_luma_smooth_vertical_01.avif": {
        "file_sha256": "2875bcb27db302966ee2bf841db15fabcdec2250021597fe6bb00b205a6bd109",
        "rgb_sha256": "9ff23d9ce13531af06b602347ba92e3e5797415b48d25ecdcf94f7301b8dfd91",
        "size": [16, 8],
    },
    "coverage_vertical8x16_chroma_diagonal157_01.avif": {
        "file_sha256": "13a6903043df42aec082de0e3afeb82e30932749e1542160a7e039e5fd53b744",
        "rgb_sha256": "fbd17283709360e2d26a968e2a0781d6dd3e59401a574b3adbb4cd06a8820fa8",
        "size": [16, 16],
    },
    "coverage_vertical8x16_chroma_horizontal_01.avif": {
        "file_sha256": "a4f4638ba60bc5ac4a5e15e161135a7cc51d521801dccbe83a1cdfbfb3cec00b",
        "rgb_sha256": "fe06a9e4a35a7a479f62725e4c0716a0f5133849e8d1e351c866506fdbae680f",
        "size": [16, 16],
    },
    "coverage_vertical8x16_chroma_vertical_01.avif": {
        "file_sha256": "2e397a17d61aad197148e86f64f2d93b6afa1c3ac3f7acb9a72370d43b3da108",
        "rgb_sha256": "56c7822ea3a4ea606bd563b91d17a96a25fb54afa85aea7ce57d3b75f60fa794",
        "size": [16, 16],
    },
    "coverage_vertical8x16_chroma_paeth_01.avif": {
        "file_sha256": "880fa280f92839b65e46a15f81a72fcf8ff5ffb7bd16820d42b303fe1ea1a587",
        "rgb_sha256": "0a05b452b8f1d623db4a663260696241fb183938c8718f7bc4eb1bc5d019914b",
        "size": [16, 16],
    },
    "coverage_vertical8x16_chroma_paeth_02.avif": {
        "file_sha256": "5c4ce0eb3a7679b32619ca39277433ca7d85b8dfea04f6ab08946bd61c519297",
        "rgb_sha256": "9edeaf44a0e8ef22777109c1228a491ea1d879d9bb75051d2c5200675e20c9ca",
        "size": [16, 16],
    },
    "coverage_vertical8x16_chroma_paeth_03.avif": {
        "file_sha256": "13fd1d5aff12ff7157f6cb114653c5fedb4085f247af008c9ae8557e7f0f088c",
        "rgb_sha256": "bdb2eefd28dbe8a00d21d18a45cfed874e635ea82fa138dcef67247bc84400fb",
        "size": [16, 16],
    },
    "coverage_r32x8_h4_ripple_01.avif": {
        "file_sha256": "95bba5fd36e7e09566ceaa3b30a616e7145609085a10f3a2adcff419218be4dd",
        "rgb_sha256": "ffb5ecf24ee59d59852e8c11713e54488b151afdf4c4c66ac027b1332d0eab53",
        "size": [32, 32],
    },
    "coverage_r32x8_filter_intra_cdf9_false_01.avif": {
        "file_sha256": "edce5f86fefc08615431608a60154c96efbada2416cf70d4fdc338bf5b78e1eb",
        "rgb_sha256": "8d7376ab37f3483ecafd2a47bcb0473ff4ff3ce25fdfac4bf1047fa61911ecfc",
        "size": [32, 32],
    },
    "coverage_h64x16_horizontal_ramp_01.avif": {
        "file_sha256": "931ba2c08b1d59b62a16496b171459d27d8d565b51cca43fd6246cc565c0ad6e",
        "rgb_sha256": "cb9f9717f9c796f868918297787c1ee8d1db3b43df3556bafde101d4d8b388c3",
        "size": [64, 64],
    },
    "coverage_r16x64_grid_01.avif": {
        "file_sha256": "af54f720d208bea68fb852bad6bbdca22355b5a646ce40b95dbd6fc01b8af3ed",
        "rgb_sha256": "f17df57e0946031d2b81ad5316e801aea9c27fe94422f360b1e328013b71ea15",
        "size": [16, 64],
    },
    "coverage_entropy_mosaic_02.avif": {
        "file_sha256": "ae7bc782e59e5647afc8787716e66ec5659bce6e40c573cfc930d1edc12c011b",
        "rgb_sha256": "89ca340e1520088f629bb46bdb0c07e08b630e2b13163ae869aca49ae0c72028",
        "size": [32, 32],
    },
    "coverage_entropy_mosaic_01.avif": {
        "file_sha256": "af928d83a7a4bf90ef4dcf74afcfb0b9057db26784f2226f444076aac28abff0",
        "rgb_sha256": "52660ed52ff5e28a3bc05d35023875e225f70acd76a1191ecd4f72cc765b8cd7",
        "size": [32, 32],
    },
    "coverage_v4_vertical_checker.avif": {
        "file_sha256": "b0b4dfb60657f4b87254773d6cca9fbc1ab07c742239fa2e0f3b862785163774",
        "rgb_sha256": "cfd11c3f8287b7e78ebf5da228ed44e04ccaac6cc6cb14a89e49f1bc446ab9ff",
        "size": [16, 16],
    },
    "coverage_h4_horizontal_bands.avif": {
        "file_sha256": "35f037da62ec75ca71341970f7751ba49d64d0f987b90c4f479c278c536c4088",
        "rgb_sha256": "c83e86163bf5e8b7121c05a41d8cdb8ae73a27d544565bc717464875b3f459c7",
        "size": [16, 16],
    },
    "coverage_h16x4_filter_intra_cdf14_false_01.avif": {
        "file_sha256": "5bc0e17ad980bfa5d5a55c26cae21718a67be584c294530d1255e4f8e37647f9",
        "rgb_sha256": "d59a569d0d1c93fb9b2537196cc6a5453691d959e7e67bc6417c9a9a1f7b4fc4",
        "size": [16, 16],
    },
    "coverage_v4x16_filter_intra_cdf19_false_01.avif": {
        "file_sha256": "2e898e072d787fd34045ec1d7892a58375912e090213fa47b942d4545a544ebe",
        "rgb_sha256": "a93370d52a860f2b22bc1730ffe1a8bc38d376f678fc32b3c6328555e0bebb11",
        "size": [16, 16],
    },
    "coverage_adst_public_04.avif": {
        "file_sha256": "2e182dc0b3e66744f22e9e6683898bda8b4031f24e66b04d8099d6699141919c",
        "rgb_sha256": "8bf5648d07e20627c47a5909233a14efdeba2d9bb30ac51c2f1d0e9c3dc568f8",
        "size": [16, 4],
    },
    "coverage_i444_palette2_square8_four_leaves.avif": {
        "file_sha256": "7d13f753585fd646426ed1d8900c38ea95c7b06ada9c9204e4b8e6d47e1e4a56",
        "rgb_sha256": "ae90d60419a44e909e312e762e05d6f73d70d32c43366eb8885aabe4d2c7725b",
        "size": [16, 16],
    },
    "coverage_adst_public_03.avif": {
        "file_sha256": "b6d15fa1ceb3eedcd3636ed660c0ed6755ce3a2af3ff6a3b2dcf6fa0b1adcc25",
        "rgb_sha256": "c4cbd418d7f72de0fd778268c0a4c40ac6c30b982987a3a4bfa84372c3c102e9",
        "size": [4, 16],
    },
    "coverage_adst_public_05.avif": {
        "file_sha256": "b398d1af52e414bee7e6d2a5ff071b8dd8d9af16d84dc301765f7fd05968537e",
        "rgb_sha256": "ccf631ee65a05977a2020995f5dc442905ad0c21450f3e3e0df3bd0f0d2b8e11",
        "size": [8, 16],
    },
    "coverage_adst_public_06.avif": {
        "file_sha256": "d4dc8bcf2e10acc54d24712def595d16a1550b7eacb44eefb58089a50a6b8ce8",
        "rgb_sha256": "988aef43dcf1c4eeaa0cffee66f3ba32e9c127c0b07996830900b4a79ed07cd6",
        "size": [16, 8],
    },
    "coverage_adst_public_07.avif": {
        "file_sha256": "0afade55d9a04a29af287c04e2f16a0cfc05758e3531658fd3be569948abe8d0",
        "rgb_sha256": "a40858233036b25f36900bd39be40e6eda843493ac27b767448b891ac8437492",
        "size": [8, 32],
    },
    "coverage_adst_public_08.avif": {
        "file_sha256": "0f59e5943381edf9361311a39d6e73a726cb028f9003fed675835104392abe5a",
        "rgb_sha256": "8b308e80e0a1a904072657a1f8b3472b5b89e37dc01238c8dc6066689a9ebf6a",
        "size": [32, 8],
    },
    "coverage_adst_public_09.avif": {
        "file_sha256": "866fc5bee5d19bf06df56b90c7b92d72c7725a1ed8aa7384a0cadb931c945a5e",
        "rgb_sha256": "e0e5a1ae7b7aef892258e7f7f2332f13f959b419ba0f9b14c8edcc9a298e487d",
        "size": [16, 16],
    },
    "coverage_adst_public_10.avif": {
        "file_sha256": "018eecce2e1f068cfe6ae022bd5e0f48f4a547c736bd946edb6ba45bc6663bcf",
        "rgb_sha256": "93047df7e452ceca5c0cf243100db0b2e1508e7db35d86dc00ad34b70069db4e",
        "size": [32, 16],
    },
    "coverage_i444_rect_01.avif": {
        "file_sha256": "fd6e70ee3664c47f0f622be98f7e654d473caf0076cf5ea9ad6d29375445a4c9",
        "rgb_sha256": "df91c9d9099a10d439672ff73982db4ed13e6aeb0b3ee9db48f791a9964fcb54",
        "size": [16, 16],
    },
    "coverage_i444_rect_02.avif": {
        "file_sha256": "fad07546f32d265ddcf03122c8b148705ebff833785b655a1b5e44bbd1d98897",
        "rgb_sha256": "81b867c7a1081b13395b3a37a7dd79d41f43542f095f048ab71693fb471c8bbb",
        "size": [16, 16],
    },
    "coverage_i444_square16_cfl_01.avif": {
        "file_sha256": "7b6d33f6ca51ca5ce5f69fcd4e1960d1d1b20d52aa4d0b954f555d6e8d47dc6d",
        "rgb_sha256": "937289169b35c042aa7000bcac5896cc781979f96867c872176a19cd08763d20",
        "size": [16, 16],
    },
    "coverage_i444_square16_cfl_02.avif": {
        "file_sha256": "496d2b4edf3ed6f4d9882087b047ac5d5e3e979f1762486762c352ef4d3da8e8",
        "rgb_sha256": "c5672465e10df70e92f05c07e8ad290410ff778f748c70abd564c59766ec5b44",
        "size": [16, 16],
    },
    "coverage_i444_square16_cfl_03.avif": {
        "file_sha256": "475f6ce83fd295a52e59d97cb9504cf6309371b9fe74cd3306d64964875b3663",
        "rgb_sha256": "3b0bdcbaa2f2b1495939a79b77c4ec273ecc5cb9cc5770ca2fe6947b86763128",
        "size": [16, 16],
    },
    "coverage_i444_v16x32_following_filter_intra_mode3_01.avif": {
        "file_sha256": "fd4465d0f0c47266f7999731081eb8f5dc1f0cb4ad74b33e38b6f013b940484e",
        "rgb_sha256": "968e7f9616cf2236f5f94d18c48ef532319d3b338d5fab45d2dfef76a74eb2f4",
        "size": [32, 32],
    },
}
DEBUG_BLOCK_PATTERN = re.compile(
    r"^poc=(?P<poc>-?\d+),y=(?P<y>-?\d+),x=(?P<x>-?\d+),"
    r"bl=(?P<level>\d+),ctx=(?P<context>\d+),bp=(?P<partition>\d+): "
    r"r=(?P<range>\d+)$"
)
DEBUG_STATE_PATTERN = re.compile(
    r"^Post-(?P<label>[^\[]+)\[(?P<value>[^\]]*)\]: r=(?P<range>\d+)"
    r"(?: \[[^\]]*\])?$"
)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def run(
    command: list[str],
    *,
    env: dict[str, str] | None = None,
    cwd: Path | None = None,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        command,
        check=True,
        cwd=cwd,
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )


def verify_source(source: Path) -> None:
    required = (
        source / "meson.build",
        source / "src" / "decode.c",
        source / "src" / "msac.c",
        source / "src" / "recon.h",
        source / "src" / "recon_tmpl.c",
    )
    if not all(path.is_file() for path in required):
        raise RuntimeError(f"{source} is not a dav1d source checkout")
    commit = run(["git", "-C", str(source), "rev-parse", "HEAD"]).stdout.strip()
    if commit != DAV1D_COMMIT:
        raise RuntimeError(f"dav1d must be {DAV1D_COMMIT}, found {commit}")
    status = run(
        ["git", "-C", str(source), "status", "--porcelain", "--untracked-files=no"]
    ).stdout
    if status:
        raise RuntimeError("dav1d source must have no tracked modifications")


def tool_environment(
    meson: Path, ninja: Path, python_path: Path | None
) -> dict[str, str]:
    env = dict(os.environ)
    tool_dirs = [str(meson.parent), str(ninja.parent)]
    env["PATH"] = os.pathsep.join(tool_dirs + [env.get("PATH", "")])
    if python_path is not None:
        existing = env.get("PYTHONPATH")
        env["PYTHONPATH"] = (
            str(python_path)
            if not existing
            else os.pathsep.join((str(python_path), existing))
        )
    return env


def instrument(
    source: Path,
    broaden_vertical_following: bool = True,
    include_block_angles: bool = False,
    include_luma_angles: bool = False,
    broaden_horizontal_square16: bool = False,
    square64_origin: bool = False,
) -> None:
    recon_path = source / "src" / "recon.h"
    text = recon_path.read_text()
    old = """\
#define DEBUG_BLOCK_INFO 0 && \\
        f->frame_hdr->frame_offset == 2 && t->by >= 0 && t->by < 4 && \\
        t->bx >= 8 && t->bx < 12
#define DEBUG_B_PIXELS 0
"""
    legacy = """\
#define DEBUG_BLOCK_INFO 1 && \
        (t->by >= 0 && t->by < 4 || \
         (t->by == 4 && f->frame_hdr->width[0] == 32 && \
          f->frame_hdr->height == 32 && !f->frame_hdr->delta.q.present && \
          !f->frame_hdr->allow_screen_content_tools)) && \
        t->bx >= 0 && t->bx < 4
#define DEBUG_B_PIXELS 1
"""
    broadened = """\
#define DEBUG_BLOCK_INFO 1 && \
        (((t->by >= 0 && t->by < 4 || \
           (t->by == 4 && f->frame_hdr->width[0] == 32 && \
            f->frame_hdr->height == 32 && !f->frame_hdr->delta.q.present && \
            !f->frame_hdr->allow_screen_content_tools)) && \
          t->bx >= 0 && t->bx < 4) || \
         (f->frame_hdr->width[0] == 32 && f->frame_hdr->height == 32 && \
          !f->frame_hdr->delta.q.present && \
          !f->frame_hdr->allow_screen_content_tools && \
          t->by >= 0 && t->by < 8 && t->bx >= 0 && t->bx < 8))
#define DEBUG_B_PIXELS 1
"""
    horizontal_square16 = """\
#define DEBUG_BLOCK_INFO 1 && \
        (((t->by >= 0 && t->by < 4 || \
           (t->by == 4 && f->frame_hdr->width[0] == 32 && \
            f->frame_hdr->height == 32 && !f->frame_hdr->delta.q.present && \
            !f->frame_hdr->allow_screen_content_tools)) && \
          t->bx >= 0 && t->bx < 4) || \
         (f->frame_hdr->width[0] == 32 && f->frame_hdr->height == 16 && \
          f->cur.p.layout == DAV1D_PIXEL_LAYOUT_I420 && \
          t->by >= 0 && t->by < 4 && t->bx >= 4 && t->bx < 8))
#define DEBUG_B_PIXELS 1
"""
    square64_origin_debug = """\
#define DEBUG_BLOCK_INFO 1 && \
        f->frame_hdr->width[0] == 64 && f->frame_hdr->height == 64 && \
        f->cur.p.layout == DAV1D_PIXEL_LAYOUT_I420 && \
        t->by >= 0 && t->by < 16 && t->bx >= 0 && t->bx < 16
#define DEBUG_SQUARE64_ORIGIN 1
#define DEBUG_B_PIXELS 0
"""
    new = (
        square64_origin_debug
        if square64_origin
        else horizontal_square16
        if broaden_horizontal_square16
        else broadened
        if broaden_vertical_following
        else legacy
    )
    if text.count(old) != 1:
        raise RuntimeError("pinned dav1d debug macros no longer match")
    recon_path.write_text(text.replace(old, new))

    recon_template_path = source / "src" / "recon_tmpl.c"
    text = recon_template_path.read_text()
    old = "    const int dbg = DEBUG_BLOCK_INFO && plane && 0;\n"
    legacy = "    const int dbg = DEBUG_BLOCK_INFO;\n"
    broadened = """\
    const int dbg = DEBUG_BLOCK_INFO &&
                    (((t->by >= 0 && t->by < 4 ||
                       (t->by == 4 && f->frame_hdr->width[0] == 32 &&
                        f->frame_hdr->height == 32 && !f->frame_hdr->delta.q.present &&
                        !f->frame_hdr->allow_screen_content_tools)) &&
                      t->bx >= 0 && t->bx < 4) ||
                     (f->cur.p.layout == DAV1D_PIXEL_LAYOUT_I420 &&
                      bs == BS_16x32 && t->bx >= 4 && t->bx < 8 &&
                     b->y_mode == FILTER_PRED &&
                     (b->y_angle == 0 || b->y_angle == 3)));
"""
    horizontal_square16 = """\
    const int dbg = DEBUG_BLOCK_INFO;
"""
    new = (
        "    const int dbg = DEBUG_BLOCK_INFO;\n"
        if square64_origin
        else horizontal_square16
        if broaden_horizontal_square16
        else broadened
        if broaden_vertical_following
        else legacy
    )
    if text.count(old) != 1:
        raise RuntimeError("pinned dav1d coefficient debug guard no longer matches")
    text = text.replace(old, new)
    if square64_origin:
        luma_coordinate_pattern = re.compile(
            r'(?m)^(?P<indent>[ ]*)if \(DEBUG_BLOCK_INFO\)\n'
            r'(?P<print_indent>[ ]*)printf\("Post-y-cf-blk'
        )

        def add_luma_coordinate(match: re.Match[str]) -> str:
            indent = match.group("indent")
            print_indent = match.group("print_indent")
            return (
                f'{indent}if (DEBUG_SQUARE64_ORIGIN)\n'
                f'{print_indent}printf("Post-y-cf-coord[x=%d,y=%d]: r=%d\\n",\n'
                f'{print_indent}       4 * t->bx, 4 * t->by, ts->msac.rng);\n'
                f'{match.group(0)}'
            )

        text, luma_count = luma_coordinate_pattern.subn(
            add_luma_coordinate, text
        )
        if luma_count != 3:
            raise RuntimeError(
                "pinned dav1d luma coordinate instrumentation points changed"
            )

        chroma_coordinate_pattern = re.compile(
            r'(?m)^(?P<indent>[ ]*)if \(DEBUG_BLOCK_INFO\)\n'
            r'(?P<print_indent>[ ]*)printf\("Post-uv-cf-blk'
        )

        def add_chroma_coordinate(match: re.Match[str]) -> str:
            indent = match.group("indent")
            print_indent = match.group("print_indent")
            return (
                f'{indent}if (DEBUG_SQUARE64_ORIGIN)\n'
                f'{print_indent}printf("Post-uv-cf-coord[pl=%d,x=%d,y=%d]: r=%d\\n",\n'
                f'{print_indent}       pl, 4 * t->bx, 4 * t->by, ts->msac.rng);\n'
                f'{match.group(0)}'
            )

        text, chroma_count = chroma_coordinate_pattern.subn(
            add_chroma_coordinate, text
        )
        if chroma_count != 3:
            raise RuntimeError(
                "pinned dav1d chroma coordinate instrumentation points changed"
            )
    recon_template_path.write_text(text)

    msac_path = source / "src" / "msac.c"
    text = msac_path.read_text()
    replacements = (
        (
            '#include <limits.h>\n',
            """\
#include <limits.h>
#include <stdio.h>
""",
        ),
        (
            '#define EC_WIN_SIZE (sizeof(ec_win) << 3)\n',
            """\
#define EC_WIN_SIZE (sizeof(ec_win) << 3)

static const uint8_t *trace_base;
static unsigned trace_step;

static void trace_msac(const char *const operation, const long long value,
                       const long long parameter, const MsacContext *const s,
                       const uint16_t *const cdf, const size_t cdf_len)
{
    printf("@MSAC {\\\"step\\\":%u,\\\"operation\\\":\\\"%s\\\","
           "\\\"value\\\":%lld,\\\"parameter\\\":%lld,"
           "\\\"byte_position\\\":%td,\\\"difference\\\":%llu,"
           "\\\"range\\\":%u,\\\"count\\\":%d,\\\"cdf\\\":[",
           trace_step++, operation, value, parameter, s->buf_pos - trace_base,
           (unsigned long long)s->dif, s->rng, s->cnt);
    for (size_t i = 0; i < cdf_len; i++) {
        if (i) putchar(',');
        printf("%u", cdf[i]);
    }
    puts("]}");
}
""",
        ),
        (
            """\
    ctx_norm(s, dif, v);
    return !ret;
}

/* Decode a single binary value.
""",
            """\
    ctx_norm(s, dif, v);
    const unsigned value = !ret;
    trace_msac("equal", value, -1, s, NULL, 0);
    return value;
}

/* Decode a single binary value.
""",
        ),
        (
            """\
    ctx_norm(s, dif, v);
    return !ret;
}

/* Decodes a symbol given an inverse cumulative distribution function (CDF)
""",
            """\
    ctx_norm(s, dif, v);
    const unsigned value = !ret;
    trace_msac("fixed", value, f, s, NULL, 0);
    return value;
}

/* Decodes a symbol given an inverse cumulative distribution function (CDF)
""",
        ),
        (
            """\
    return val;
}

unsigned dav1d_msac_decode_bool_adapt_c""",
            """\
    trace_msac("adaptive_symbol", val, n_symbols, s, cdf, n_symbols + 1);
    return val;
}

unsigned dav1d_msac_decode_bool_adapt_c""",
        ),
        (
            """\
    return bit;
}

unsigned dav1d_msac_decode_hi_tok_c""",
            """\
    trace_msac("adaptive_bool", bit, 1, s, cdf, 2);
    return bit;
}

unsigned dav1d_msac_decode_hi_tok_c""",
        ),
        (
            """\
    return tok;
}
#endif
""",
            """\
    trace_msac("high_token", tok, 3, s, cdf, 4);
    return tok;
}
#endif
""",
        ),
        (
            """\
    ctx_refill(s);

#if ARCH_X86_64 && HAVE_ASM
""",
            """\
    trace_base = data;
    trace_step = 0;
    ctx_refill(s);
    trace_msac("init", -1, disable_cdf_update_flag, s, NULL, 0);

#if ARCH_X86_64 && HAVE_ASM
""",
        ),
    )
    for old_text, new_text in replacements:
        if text.count(old_text) != 1:
            raise RuntimeError("pinned dav1d MSAC instrumentation point changed")
        text = text.replace(old_text, new_text)
    msac_path.write_text(text)

    if include_luma_angles or include_block_angles:
        decode_path = source / "src" / "decode.c"
        text = decode_path.read_text()
        if include_luma_angles:
            old = """
            const int angle = dav1d_msac_decode_symbol_adapt8(&ts->msac, acdf, 6);
            b->y_angle = angle - 3;
        } else {
            b->y_angle = 0;
        }
"""
            new = """
            const int angle = dav1d_msac_decode_symbol_adapt8(&ts->msac, acdf, 6);
            b->y_angle = angle - 3;
            if (DEBUG_BLOCK_INFO)
                printf("Post-yangle-symbol[%d]: r=%d\\n", angle, ts->msac.rng);
        } else {
            b->y_angle = 0;
        }
"""
            if text.count(old) != 1:
                raise RuntimeError("pinned dav1d luma angle instrumentation point changed")
            text = text.replace(old, new)
        old = """
                const int angle = dav1d_msac_decode_symbol_adapt8(&ts->msac, acdf, 6);
                b->uv_angle = angle - 3;
            }
        }

        b->pal_sz[0] = b->pal_sz[1] = 0;
"""
        new = """
                const int angle = dav1d_msac_decode_symbol_adapt8(&ts->msac, acdf, 6);
                b->uv_angle = angle - 3;
                if (DEBUG_BLOCK_INFO)
                    printf("Post-uvangle-symbol[%d]: r=%d\\n", angle, ts->msac.rng);
            }
        }

        b->pal_sz[0] = b->pal_sz[1] = 0;
"""
        if text.count(old) != 1:
            raise RuntimeError("pinned dav1d UV angle instrumentation point changed")
        decode_path.write_text(text.replace(old, new))

    if square64_origin:
        decode_path = source / "src" / "decode.c"
        text = decode_path.read_text()
        old = """\
                uint16_t *const tx_cdf = ts->cdf.m.txsz[t_dim->max - 1][tctx];
                int depth = dav1d_msac_decode_symbol_adapt4(&ts->msac, tx_cdf,
                                imin(t_dim->max, 2));

                while (depth--) {
                    b->tx = t_dim->sub;
                    t_dim = &dav1d_txfm_dimensions[b->tx];
                }
"""
        new = """\
                const int max_tx = t_dim->max;
                uint16_t *const tx_cdf = ts->cdf.m.txsz[max_tx - 1][tctx];
                int depth = dav1d_msac_decode_symbol_adapt4(&ts->msac, tx_cdf,
                                imin(max_tx, 2));
                const int selected_depth = depth;

                while (depth--) {
                    b->tx = t_dim->sub;
                    t_dim = &dav1d_txfm_dimensions[b->tx];
                }
                if (DEBUG_SQUARE64_ORIGIN) {
                    printf("Post-square64-trace[scope=origin64-v1]: r=%d\\n",
                           ts->msac.rng);
                    printf("Post-tx-detail[max=%d,selected=%d,depth=%d,"
                           "x=%d,y=%d]: r=%d\\n",
                           max_tx, b->tx, selected_depth, 4 * t->bx,
                           4 * t->by, ts->msac.rng);
                }
"""
        if text.count(old) != 1:
            raise RuntimeError("pinned dav1d Square64 transform instrumentation changed")
        decode_path.write_text(text.replace(old, new))


def build_dav1d(
    source: Path,
    work: Path,
    meson: Path,
    ninja: Path,
    python_path: Path | None,
    broaden_vertical_following: bool = True,
    include_block_angles: bool = False,
    include_luma_angles: bool = False,
    broaden_horizontal_square16: bool = False,
    square64_origin: bool = False,
) -> tuple[Path, dict[str, str]]:
    clone = work / "dav1d"
    build = work / "build"
    run(["git", "clone", "--quiet", "--no-hardlinks", str(source), str(clone)])
    run(["git", "-C", str(clone), "checkout", "--quiet", DAV1D_COMMIT])
    instrument(
        clone,
        broaden_vertical_following,
        include_block_angles,
        include_luma_angles,
        broaden_horizontal_square16,
        square64_origin,
    )
    env = tool_environment(meson, ninja, python_path)
    run(
        [
            str(meson),
            "setup",
            str(build),
            str(clone),
            "--buildtype=debug",
            "-Denable_tools=true",
            "-Denable_tests=false",
            "-Denable_examples=false",
            "-Dtestdata_tests=false",
            "-Denable_asm=false",
        ],
        env=env,
    )
    run([str(meson), "compile", "-C", str(build), "tools/dav1d"], env=env)
    executable = build / "tools" / "dav1d"
    version_result = run([str(executable), "--version"], env=env)
    version = (version_result.stdout + version_result.stderr).strip()
    if not version.startswith("1.5.3-0-gb546257"):
        raise RuntimeError(f"unexpected dav1d executable version: {version}")
    return executable, env


def extract_color_item(path: Path) -> tuple[bytes, dict[str, object]]:
    report = inspect_avif(path)
    color_items = report.get("items", {}).get("color", [])
    if len(color_items) != 1:
        raise RuntimeError(f"{path.name} must contain exactly one color item")
    item = color_items[0]
    data = path.read_bytes()
    sample = b"".join(
        data[span["offset"] : span["offset"] + span["length"]]
        for span in item["spans"]
    )
    if len(sample) != item["length"] or sha256(sample) != item["sha256"]:
        raise RuntimeError(f"{path.name} color item boundary mismatch")
    return sample, report


def parse_debug_log(
    output: str,
) -> tuple[
    list[str],
    list[dict[str, object]],
    list[dict[str, object]],
    list[dict[str, int]],
    list[dict[str, object]],
]:
    lines = [line.rstrip() for line in output.splitlines() if line.strip()]
    event_stream = []
    for line in lines:
        if line.startswith("@MSAC "):
            event_stream.append(
                {"kind": "entropy", "operation": json.loads(line.removeprefix("@MSAC "))}
            )
        else:
            event_stream.append({"kind": "debug", "line": line})
    entropy_operations = [
        json.loads(line.removeprefix("@MSAC "))
        for line in lines
        if line.startswith("@MSAC ")
    ]
    lines = [line for line in lines if not line.startswith("@MSAC ")]
    if not entropy_operations or entropy_operations[0]["operation"] != "init":
        raise RuntimeError("missing scalar MSAC trace")
    expected_steps = list(range(len(entropy_operations)))
    if [operation["step"] for operation in entropy_operations] != expected_steps:
        raise RuntimeError("non-contiguous scalar MSAC trace")
    block_matches = [
        match for line in lines if (match := DEBUG_BLOCK_PATTERN.fullmatch(line))
    ]
    if not block_matches:
        raise RuntimeError("expected at least one partition-block header")
    blocks = [
        {name: int(value) for name, value in match.groupdict().items()}
        for match in block_matches
    ]
    states = []
    for line in lines:
        match = DEBUG_STATE_PATTERN.fullmatch(line)
        if match is not None:
            states.append(
                {
                    "label": match.group("label"),
                    "value": match.group("value"),
                    "range": int(match.group("range")),
                }
            )
    required = {"skip", "ymode", "uvmode", "y-cf-blk", "uv-cf-blk"}
    labels = {state["label"] for state in states}
    if not required.issubset(labels):
        raise RuntimeError(
            f"incomplete first-block trace: missing {sorted(required - labels)}; "
            f"log={lines!r}"
        )
    return lines, event_stream, entropy_operations, blocks, states


def pillow_reference(path: Path) -> tuple[list[int], bytes]:
    with Image.open(path) as image:
        image.load()
        if image.mode != "RGB":
            raise RuntimeError(f"{path.name} must decode to Pillow RGB, found {image.mode}")
        return list(image.size), image.tobytes()


def pillow_libyuv_version() -> int:
    library = ctypes.CDLL(_avif.__file__)
    version = library.avifLibYUVVersion
    version.argtypes = []
    version.restype = ctypes.c_uint
    return int(version())


def portable_color_reference(path: Path) -> dict[str, object]:
    report = inspect_av1(path)
    samples = report.get("samples", [])
    if len(samples) != 1:
        raise RuntimeError(f"{path.name} must contain one AV1 color sample")
    obus = samples[0]["obus"]
    sequence_headers = [
        obu["sequence_header"] for obu in obus if "sequence_header" in obu
    ]
    frame_headers = [obu["frame_header"] for obu in obus if "frame_header" in obu]
    if len(sequence_headers) != 1 or len(frame_headers) != 1:
        raise RuntimeError(f"{path.name} must contain one sequence and frame header")
    sequence = sequence_headers[0]
    frame = frame_headers[0]
    return {
        "width": frame["frame_width"],
        "height": frame["frame_height"],
        "bit_depth": sequence["bit_depth"],
        "monochrome": sequence["monochrome"],
        "color_primaries": sequence["color_primaries"],
        "transfer_characteristics": sequence["transfer_characteristics"],
        "matrix_coefficients": sequence["matrix_coefficients"],
        "color_range": bool(sequence["color_range"]),
        "subsampling_x": bool(sequence["subsampling_x"]),
        "subsampling_y": bool(sequence["subsampling_y"]),
    }


def decode_fixture(
    executable: Path,
    env: dict[str, str],
    work: Path,
    path: Path,
) -> dict[str, object]:
    expected = EXPECTED_FIXTURES[path.name]
    file_bytes = path.read_bytes()
    if sha256(file_bytes) != expected["file_sha256"]:
        raise RuntimeError(f"{path.name} does not match its pinned fixture hash")
    sample, container = extract_color_item(path)
    size, rgb = pillow_reference(path)
    if size != expected["size"] or sha256(rgb) != expected["rgb_sha256"]:
        raise RuntimeError(f"{path.name} Pillow reference changed")
    portable_color = portable_color_reference(path)

    stem = path.stem
    sample_path = work / f"{stem}.obu"
    yuv_path = work / f"{stem}.yuv"
    sample_path.write_bytes(sample)
    result = run(
        [
            str(executable),
            "--input",
            str(sample_path),
            "--demuxer",
            "section5",
            "--output",
            str(yuv_path),
            "--muxer",
            "yuv",
            "--threads",
            "1",
            "--framedelay",
            "1",
            "--cpumask",
            "0",
            "--quiet",
        ],
        env=env,
    )
    log, event_stream, entropy_operations, blocks, states = parse_debug_log(result.stdout)
    yuv = yuv_path.read_bytes()
    chroma_width = (size[0] + int(portable_color["subsampling_x"])) >> int(
        portable_color["subsampling_x"]
    )
    chroma_height = (size[1] + int(portable_color["subsampling_y"])) >> int(
        portable_color["subsampling_y"]
    )
    plane_dimensions = [
        (size[0], size[1]),
        (chroma_width, chroma_height),
        (chroma_width, chroma_height),
    ]
    plane_lengths = [width * height for width, height in plane_dimensions]
    if len(yuv) != sum(plane_lengths):
        raise RuntimeError(f"{path.name} has an unexpected decoded YUV length")
    planes = []
    offset = 0
    for name, (plane_width, plane_height), plane_length in zip(
        ("y", "u", "v"), plane_dimensions, plane_lengths
    ):
        plane = yuv[offset : offset + plane_length]
        offset += plane_length
        planes.append(
            {
                "name": name,
                "width": plane_width,
                "height": plane_height,
                "row_bytes": [
                    plane[row * plane_width : (row + 1) * plane_width].hex()
                    for row in range(plane_height)
                ],
                "sha256": sha256(plane),
            }
        )
    color_item = container["items"]["color"][0]
    return {
        "fixture": path.name,
        "fixture_sha256": sha256(file_bytes),
        "encoded_item": {
            "length": len(sample),
            "sha256": sha256(sample),
            "spans": color_item["spans"],
        },
        "pillow": {
            "mode": "RGB",
            "size": size,
            "bytes": len(rgb),
            "sha256": sha256(rgb),
            "row_bytes": [
                rgb[row * size[0] * 3 : (row + 1) * size[0] * 3].hex()
                for row in range(size[1])
            ],
        },
        "portable_color": portable_color,
        "first_block": blocks[0],
        "partition_blocks": blocks,
        "decoder_events": event_stream,
        "entropy_operations": entropy_operations,
        "entropy_states": states,
        "dav1d_debug_log": log,
        "decoded_planes": planes,
    }


def generate(
    dav1d_source: Path,
    meson: Path,
    ninja: Path,
    python_path: Path | None,
) -> dict[str, object]:
    verify_source(dav1d_source)
    if features.version("avif") != "1.4.1":
        raise RuntimeError("Pillow must report libavif 1.4.1")
    codecs = _avif.codec_versions()
    for expected in ("dav1d [dec]:1.5.3", "aom [enc]:3.13.2"):
        if expected not in codecs:
            raise RuntimeError(f"Pillow AVIF oracle lacks {expected}: {codecs}")
    libyuv_version = pillow_libyuv_version()
    if libyuv_version != 1922:
        raise RuntimeError(
            f"Pillow AVIF oracle must use libyuv 1922, found {libyuv_version}"
        )

    with tempfile.TemporaryDirectory(prefix="image-star-av1-reconstruction-") as name:
        work = Path(name)
        legacy_executable, legacy_env = build_dav1d(
            dav1d_source,
            work / "legacy",
            meson,
            ninja,
            python_path,
            broaden_vertical_following=False,
        )
        target_executable, target_env = build_dav1d(
            dav1d_source, work / "target", meson, ninja, python_path
        )
        square64_executable, square64_env = build_dav1d(
            dav1d_source,
            work / "square64",
            meson,
            ninja,
            python_path,
            square64_origin=True,
        )
        square16_chroma_executable, square16_chroma_env = build_dav1d(
            dav1d_source,
            work / "square16-chroma",
            meson,
            ninja,
            python_path,
            broaden_vertical_following=False,
            broaden_horizontal_square16=True,
        )
        angle_executable, angle_env = build_dav1d(
            dav1d_source,
            work / "angle",
            meson,
            ninja,
            python_path,
            include_block_angles=True,
        )
        luma_angle_executable, luma_angle_env = build_dav1d(
            dav1d_source,
            work / "luma-angle",
            meson,
            ninja,
            python_path,
            include_luma_angles=True,
        )
        cases = [
                decode_fixture(
                    luma_angle_executable
                if name in LUMA_DIAGONAL_DOWN_RIGHT_TARGET_FIXTURES
                or name in LUMA_DIAGONAL45_TARGET_FIXTURES
                or name in CHROMA_DIAGONAL67_VERTICAL_TARGET_FIXTURES
                else angle_executable
                if name in CHROMA_DIAGONAL45_TARGET_FIXTURES
                or name in CHROMA_HORIZONTAL_TARGET_FIXTURES
                else square16_chroma_executable
                if name in SQUARE16_CHROMA_SMOOTH_HORIZONTAL_TARGET_FIXTURES
                or name in SQUARE16_CHROMA_SMOOTH_VERTICAL_TARGET_FIXTURES
                or name in SQUARE16_CHROMA_SMOOTH_TARGET_FIXTURES
                else square64_executable
                if name in SQUARE64_SPLIT_TARGET_FIXTURES
                else target_executable
                if name in VERTICAL_FOLLOWING_TARGET_FIXTURES
                or name in SQUARE32_SPLIT_TARGET_FIXTURES
                or name in SQUARE16_FILTER_INTRA_TARGET_FIXTURES
                or name in LUMA_SMOOTH_TARGET_FIXTURES
                or name in VERTICAL8X16_FILTER_INTRA_TARGET_FIXTURES
                or name in CHROMA_DIAGONAL113_TARGET_FIXTURES
                or name in CHROMA_DIAGONAL157_TARGET_FIXTURES
                or name in CHROMA_HORIZONTAL_TARGET_FIXTURES
                or name in CHROMA_VERTICAL_TARGET_FIXTURES
                or name in CHROMA_PAETH_TARGET_FIXTURES
                or name in SQUARE16_CFL_TARGET_FIXTURES
            else legacy_executable,
                luma_angle_env
                if name in LUMA_DIAGONAL_DOWN_RIGHT_TARGET_FIXTURES
                or name in LUMA_DIAGONAL45_TARGET_FIXTURES
                or name in CHROMA_DIAGONAL67_VERTICAL_TARGET_FIXTURES
                else angle_env
                if name in CHROMA_DIAGONAL45_TARGET_FIXTURES
                or name in CHROMA_HORIZONTAL_TARGET_FIXTURES
                else square16_chroma_env
                if name in SQUARE16_CHROMA_SMOOTH_HORIZONTAL_TARGET_FIXTURES
                or name in SQUARE16_CHROMA_SMOOTH_VERTICAL_TARGET_FIXTURES
                or name in SQUARE16_CHROMA_SMOOTH_TARGET_FIXTURES
                else square64_env
                if name in SQUARE64_SPLIT_TARGET_FIXTURES
                else target_env
                if name in VERTICAL_FOLLOWING_TARGET_FIXTURES
                or name in SQUARE32_SPLIT_TARGET_FIXTURES
                or name in SQUARE16_FILTER_INTRA_TARGET_FIXTURES
                or name in LUMA_SMOOTH_TARGET_FIXTURES
                or name in VERTICAL8X16_FILTER_INTRA_TARGET_FIXTURES
                or name in CHROMA_DIAGONAL113_TARGET_FIXTURES
                or name in CHROMA_DIAGONAL157_TARGET_FIXTURES
                or name in CHROMA_HORIZONTAL_TARGET_FIXTURES
                or name in CHROMA_VERTICAL_TARGET_FIXTURES
                or name in CHROMA_PAETH_TARGET_FIXTURES
                or name in SQUARE16_CFL_TARGET_FIXTURES
            else legacy_env,
                work,
                FIXTURE_DIR / name,
            )
            for name in EXPECTED_FIXTURES
        ]
    return {
        "format_version": 3,
        "oracle": {
            "implementation": "dav1d",
            "version": "1.5.3",
            "commit": DAV1D_COMMIT,
            "pillow_avif": "1.4.1",
            "pillow_codecs": codecs,
            "pillow_libyuv": libyuv_version,
        },
        "scope": (
            "private AV1 first-block reconstruction and closed-class AVIF "
            "materialization; not a public image-processing API"
        ),
        "cases": cases,
    }


def resolve_tool(value: str, name: str) -> Path:
    resolved = shutil.which(value)
    if resolved is None:
        raise RuntimeError(f"{name} executable not found: {value}")
    return Path(resolved).resolve()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dav1d-source", type=Path, required=True)
    parser.add_argument("--meson", default="meson")
    parser.add_argument("--ninja", default="ninja")
    parser.add_argument(
        "--python-path",
        type=Path,
        help="Optional site-packages path for an isolated Meson installation",
    )
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    args = parser.parse_args()

    meson = resolve_tool(args.meson, "Meson")
    ninja = resolve_tool(args.ninja, "Ninja")
    first = generate(
        args.dav1d_source.resolve(),
        meson,
        ninja,
        args.python_path.resolve() if args.python_path else None,
    )
    second = generate(
        args.dav1d_source.resolve(),
        meson,
        ninja,
        args.python_path.resolve() if args.python_path else None,
    )
    if first != second:
        raise RuntimeError("instrumented dav1d trace is not deterministic")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(first, indent=2, sort_keys=True) + "\n")
    print(f"Written deterministic trace: {args.output}")


if __name__ == "__main__":
    main()
