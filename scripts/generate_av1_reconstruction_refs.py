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


def instrument(source: Path) -> None:
    recon_path = source / "src" / "recon.h"
    text = recon_path.read_text()
    old = """\
#define DEBUG_BLOCK_INFO 0 && \\
        f->frame_hdr->frame_offset == 2 && t->by >= 0 && t->by < 4 && \\
        t->bx >= 8 && t->bx < 12
#define DEBUG_B_PIXELS 0
"""
    new = """\
#define DEBUG_BLOCK_INFO 1 && t->by >= 0 && t->by < 4 && \
        t->bx >= 0 && t->bx < 4
#define DEBUG_B_PIXELS 1
"""
    if text.count(old) != 1:
        raise RuntimeError("pinned dav1d debug macros no longer match")
    recon_path.write_text(text.replace(old, new))

    recon_template_path = source / "src" / "recon_tmpl.c"
    text = recon_template_path.read_text()
    old = "    const int dbg = DEBUG_BLOCK_INFO && plane && 0;\n"
    new = "    const int dbg = DEBUG_BLOCK_INFO;\n"
    if text.count(old) != 1:
        raise RuntimeError("pinned dav1d coefficient debug guard no longer matches")
    recon_template_path.write_text(text.replace(old, new))

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


def build_dav1d(
    source: Path,
    work: Path,
    meson: Path,
    ninja: Path,
    python_path: Path | None,
) -> tuple[Path, dict[str, str]]:
    clone = work / "dav1d"
    build = work / "build"
    run(["git", "clone", "--quiet", "--no-local", str(source), str(clone)])
    run(["git", "-C", str(clone), "checkout", "--quiet", DAV1D_COMMIT])
    instrument(clone)
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
    plane_size = size[0] * size[1]
    if len(yuv) != plane_size * 3:
        raise RuntimeError(
            f"{path.name} is not the expected 8-bit 4:4:4 dav1d output"
        )
    planes = []
    for index, name in enumerate(("y", "u", "v")):
        plane = yuv[index * plane_size : (index + 1) * plane_size]
        planes.append(
            {
                "name": name,
                "width": size[0],
                "height": size[1],
                "row_bytes": [
                    plane[row * size[0] : (row + 1) * size[0]].hex()
                    for row in range(size[1])
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
        executable, env = build_dav1d(
            dav1d_source, work, meson, ninja, python_path
        )
        cases = [
            decode_fixture(executable, env, work, FIXTURE_DIR / name)
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
