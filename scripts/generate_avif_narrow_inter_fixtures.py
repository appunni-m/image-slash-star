#!/usr/bin/env python3
"""Generate pinned narrow AVIF inter blocks from an independent native encoder.

The producer reads no target output or expected pixels. Exact public frames
are collected separately by generate_decode_refs.py with pinned Pillow.
"""
from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile

from PIL import Image

from avif_bmff_boxes import normalize_creation_timestamps
from generate_avif_filmgrain_edge_fixtures import require_pinned_oracle
from inspect_av1_obus import inspect

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "tests/fixtures/input/images/avif"
VARIANTS = {
    "animated_lossy_inter_420_10bit_b16x4_q99_64x16": {
        "bit_depth": 10,
        "height": 16,
        "input_name": "horizontal-420-q99",
        "layout": "420",
        "quality": 99,
        "sha256": "f5f4844b35b01041579c7df6a8da5920e6250ee642cdb7ac1984f9f3a902654a",
        "width": 64
    },
    "animated_lossy_inter_420_10bit_b4x16_fine_q99_16x64": {
        "bit_depth": 10,
        "height": 64,
        "input_name": "vertical-420-q99",
        "layout": "420",
        "pattern": "fine",
        "quality": 99,
        "sha256": "809ba8beeafd050f67a34ea38cd7a0f169fb7b1493b2722e6ba88c36bdf3c49b",
        "width": 16
    },
    "animated_lossy_inter_420_10bit_b4x16_q99_16x64": {
        "bit_depth": 10,
        "height": 64,
        "input_name": "vertical-420-q99",
        "layout": "420",
        "quality": 99,
        "sha256": "c499ff3669a714514680ef5a5fa966f65d7236c859a861d976afbc9e5ff995c7",
        "width": 16
    },
    "animated_lossy_inter_420_12bit_b16x4_q99_64x16": {
        "bit_depth": 12,
        "height": 16,
        "input_name": "horizontal-420-q99",
        "layout": "420",
        "quality": 99,
        "sha256": "2ad79b720118af8c51d9ce90af2ff66d924a91e37ba248177deb67a480aee653",
        "width": 64
    },
    "animated_lossy_inter_420_12bit_b4x16_fine_q99_16x64": {
        "bit_depth": 12,
        "height": 64,
        "input_name": "vertical-420-q99",
        "layout": "420",
        "pattern": "fine",
        "quality": 99,
        "sha256": "cb9b98d09e00807bca2759fa7bb5c44e9e59e21651fbc6be3a01c5850bb060fd",
        "width": 16
    },
    "animated_lossy_inter_420_12bit_b4x16_q99_16x64": {
        "bit_depth": 12,
        "height": 64,
        "input_name": "vertical-420-q99",
        "layout": "420",
        "quality": 99,
        "sha256": "fab0e71887d8e8130e729d9449bd42fe951b045170a66f72db5dfa31eb54dca6",
        "width": 16
    },
    "animated_lossy_inter_420_b16x4_q45_64x16": {
        "height": 16,
        "input_name": "horizontal-420-q45",
        "layout": "420",
        "quality": 45,
        "sha256": "dbaaf03acd8b5b1819710870caaa9108043a1734530e28a656aec34b9f11481a",
        "width": 64
    },
    "animated_lossy_inter_420_b16x4_q70_64x16": {
        "height": 16,
        "input_name": "horizontal-420-q70",
        "layout": "420",
        "quality": 70,
        "sha256": "fa412144d15f941caf49416afe6af1c6b33ad8f9326748a712ea9b780a548286",
        "width": 64
    },
    "animated_lossy_inter_420_b16x4_q99_64x16": {
        "height": 16,
        "input_name": "horizontal-420-q99",
        "layout": "420",
        "quality": 99,
        "sha256": "d1aee7bb97195edb3ec8ea0cbc4d3f0a998b4440d8d3e12880c5aae4fee2e3f0",
        "width": 64
    },
    "animated_lossy_inter_420_b4x16_q45_16x64": {
        "height": 64,
        "input_name": "vertical-420-q45",
        "layout": "420",
        "quality": 45,
        "sha256": "208092164efb3eee1899376810507adf24b57334db1a260de0480fcc0c784933",
        "width": 16
    },
    "animated_lossy_inter_420_b4x16_q70_16x64": {
        "height": 64,
        "input_name": "vertical-420-q70",
        "layout": "420",
        "quality": 70,
        "sha256": "ab60cec38de20a78b945fcdd3267c47adbd0b6a08f9820b96ba46b6c462740d0",
        "width": 16
    },
    "animated_lossy_inter_420_b4x16_q99_16x64": {
        "height": 64,
        "input_name": "vertical-420-q99",
        "layout": "420",
        "quality": 99,
        "sha256": "fb3d30b4828faa053082aac576e08c88de83554c38fa0fccdc2bfa06cad63f87",
        "width": 16
    },
    "animated_lossy_inter_422_10bit_b16x4_q99_64x16": {
        "bit_depth": 10,
        "height": 16,
        "input_name": "horizontal-422-q99",
        "layout": "422",
        "quality": 99,
        "sha256": "1bedf2607ded647945fad966e627907adeb82ab8b139e6718b17720ebe4ad861",
        "width": 64
    },
    "animated_lossy_inter_422_12bit_b16x4_q99_64x16": {
        "bit_depth": 12,
        "height": 16,
        "input_name": "horizontal-422-q99",
        "layout": "422",
        "quality": 99,
        "sha256": "fc881ef2ad779a05222bc5d89e23b7bde3252507803c08413dbc1a9187d71736",
        "width": 64
    },
    "animated_lossy_inter_422_b16x4_q45_64x16": {
        "height": 16,
        "input_name": "horizontal-422-q45",
        "layout": "422",
        "quality": 45,
        "sha256": "a2bc965afb5a01848c38eba790da0af86d143747c87a4aa0b0a8200f16e03198",
        "width": 64
    },
    "animated_lossy_inter_422_b16x4_q70_64x16": {
        "height": 16,
        "input_name": "horizontal-422-q70",
        "layout": "422",
        "quality": 70,
        "sha256": "f8c4975e11d54f4e013a2700974fd5c1b68c52ea0229eba0e821e99f3d27ba67",
        "width": 64
    },
    "animated_lossy_inter_422_b16x4_q99_64x16": {
        "height": 16,
        "input_name": "horizontal-422-q99",
        "layout": "422",
        "quality": 99,
        "sha256": "bb12465478e20e84aecb5dd8247a27d5b221de0e47eaf622d56140d89bdde9ba",
        "width": 64
    },
    "animated_lossy_inter_444_10bit_b16x4_q99_64x16": {
        "bit_depth": 10,
        "height": 16,
        "input_name": "horizontal-444-q99",
        "layout": "444",
        "quality": 99,
        "sha256": "c63fa843621614dbadc1c3fb14dd68bd6baf5df7cb4f146cc5bf865b956002b9",
        "width": 64
    },
    "animated_lossy_inter_444_10bit_b4x16_fine_q99_16x64": {
        "bit_depth": 10,
        "height": 64,
        "input_name": "vertical-444-q99",
        "layout": "444",
        "pattern": "fine",
        "quality": 99,
        "sha256": "44d119079318c736357643fce2558ac6d2d04b443cf793edd6dc6f6e406c36db",
        "width": 16
    },
    "animated_lossy_inter_444_10bit_b4x16_q99_16x64": {
        "bit_depth": 10,
        "height": 64,
        "input_name": "vertical-444-q99",
        "layout": "444",
        "quality": 99,
        "sha256": "4931f1c53fcadc57f4d025a6e2f1dabfcad35c5c7620cd55d46144b63f358729",
        "width": 16
    },
    "animated_lossy_inter_444_12bit_b16x4_q99_64x16": {
        "bit_depth": 12,
        "height": 16,
        "input_name": "horizontal-444-q99",
        "layout": "444",
        "quality": 99,
        "sha256": "5ae3dd60624e55f3c322842417ce725b1d5defa613d2582b9af1f14d31fffdb3",
        "width": 64
    },
    "animated_lossy_inter_444_12bit_b4x16_fine_q99_16x64": {
        "bit_depth": 12,
        "height": 64,
        "input_name": "vertical-444-q99",
        "layout": "444",
        "pattern": "fine",
        "quality": 99,
        "sha256": "2509c8349c1af9840afb113c614d052f6b3763bed8fe1a911f86a95976730dc1",
        "width": 16
    },
    "animated_lossy_inter_444_12bit_b4x16_q99_16x64": {
        "bit_depth": 12,
        "height": 64,
        "input_name": "vertical-444-q99",
        "layout": "444",
        "quality": 99,
        "sha256": "70ba8dac040e6e7d1ac879a17934d02579df5d3ce7d814488ff1ee3de7d51172",
        "width": 16
    },
    "animated_lossy_inter_444_b16x4_q45_64x16": {
        "height": 16,
        "input_name": "horizontal-444-q45",
        "layout": "444",
        "quality": 45,
        "sha256": "73f17e705c08f744fa9d1c14f108a57d8d0cf3ba857b85034dde641fc1b5b681",
        "width": 64
    },
    "animated_lossy_inter_444_b16x4_q70_64x16": {
        "height": 16,
        "input_name": "horizontal-444-q70",
        "layout": "444",
        "quality": 70,
        "sha256": "7c1c750472a71e34e4a1f8dffa57a679e2bb6255ea57f0b231726f8e4a9f9f13",
        "width": 64
    },
    "animated_lossy_inter_444_b16x4_q99_64x16": {
        "height": 16,
        "input_name": "horizontal-444-q99",
        "layout": "444",
        "quality": 99,
        "sha256": "33d9c7e75e158d7c2f86ff533af393c8bfb072151d6fe067b8a8226122938a1e",
        "width": 64
    },
    "animated_lossy_inter_444_b4x16_q45_16x64": {
        "height": 64,
        "input_name": "vertical-444-q45",
        "layout": "444",
        "quality": 45,
        "sha256": "2a10f8d1813a971192b03f2e60e057558829973196ca880ba72c8f14d0a022cb",
        "width": 16
    },
    "animated_lossy_inter_444_b4x16_q70_16x64": {
        "height": 64,
        "input_name": "vertical-444-q70",
        "layout": "444",
        "quality": 70,
        "sha256": "1b9c0fda442408847681d20df597c621693e8f80ec278d93e70bdc9b36b96cfc",
        "width": 16
    },
    "animated_lossy_inter_444_b4x16_q99_16x64": {
        "height": 64,
        "input_name": "vertical-444-q99",
        "layout": "444",
        "quality": 99,
        "sha256": "cf3de7722a93d3f5214907e9f48ab8366ab729cf1b20ecf1beeac7d5fe1f4676",
        "width": 16
    },
    "animated_lossy_inter_420_10bit_fine_q99_cdf_frozen_16x64": {
        "input_name": "vertical-420-q99",
        "width": 16,
        "height": 64,
        "layout": "420",
        "quality": 99,
        "bit_depth": 10,
        "pattern": "fine",
        "cdf_mode": 0,
        "sha256": "e1c17a95082ab9fb703983f024f7ee2fd1c79050973e4c7448a7547b690bd2e6"
    },
    "animated_lossy_inter_420_12bit_fine_q99_cdf_frozen_16x64": {
        "input_name": "vertical-420-q99",
        "width": 16,
        "height": 64,
        "layout": "420",
        "quality": 99,
        "bit_depth": 12,
        "pattern": "fine",
        "cdf_mode": 0,
        "sha256": "bbbba21945dc3814f2d4ca4b73c3b57831b6c3231fc2988f8c0a9698089ccdbf"
    },
    "animated_lossy_inter_420_8bit_fine_q99_cdf_frozen_16x64": {
        "input_name": "vertical-420-q99",
        "width": 16,
        "height": 64,
        "layout": "420",
        "quality": 99,
        "bit_depth": 8,
        "pattern": "fine",
        "cdf_mode": 0,
        "sha256": "8f91043054786db0e74e777fdc2bd6b54bc0051168104d8e70cc54e452d407ba"
    },
    "animated_lossy_inter_420_8bit_fine_q99_cdf_selective_16x64": {
        "input_name": "vertical-420-q99",
        "width": 16,
        "height": 64,
        "layout": "420",
        "quality": 99,
        "bit_depth": 8,
        "pattern": "fine",
        "cdf_mode": 2,
        "sha256": "0b195ed71bbd49d8691b1b1e56fd665cd6732bd1772163417f0cb901fcde23b7"
    },
    "animated_lossy_inter_422_10bit_fine_q99_cdf_frozen_64x16": {
        "input_name": "horizontal-422-q99",
        "width": 64,
        "height": 16,
        "layout": "422",
        "quality": 99,
        "bit_depth": 10,
        "pattern": "fine",
        "cdf_mode": 0,
        "sha256": "922add08d9c3a59c2a114ff32daa76b1cc64f8857c1f97b40a60f33edb19cd21"
    },
    "animated_lossy_inter_422_10bit_fine_q99_cdf_selective_64x16": {
        "input_name": "horizontal-422-q99",
        "width": 64,
        "height": 16,
        "layout": "422",
        "quality": 99,
        "bit_depth": 10,
        "pattern": "fine",
        "cdf_mode": 2,
        "sha256": "8f4d106538db9a923a82735f56996141c0d79dc0f7e724ea6a7594273944c2a5"
    },
    "animated_lossy_inter_422_12bit_fine_q99_cdf_frozen_64x16": {
        "input_name": "horizontal-422-q99",
        "width": 64,
        "height": 16,
        "layout": "422",
        "quality": 99,
        "bit_depth": 12,
        "pattern": "fine",
        "cdf_mode": 0,
        "sha256": "2816cd96523a4ed9f4a106d733972266751f18eb1d89856f48ca932d6f133aad"
    },
    "animated_lossy_inter_422_8bit_fine_q99_cdf_frozen_64x16": {
        "input_name": "horizontal-422-q99",
        "width": 64,
        "height": 16,
        "layout": "422",
        "quality": 99,
        "bit_depth": 8,
        "pattern": "fine",
        "cdf_mode": 0,
        "sha256": "5f983e0773d97812427ce340b9d06a14cff7037a045feb9cb37ec29e63e03ee2"
    },
    "animated_lossy_inter_444_10bit_fine_q99_cdf_frozen_16x64": {
        "input_name": "vertical-444-q99",
        "width": 16,
        "height": 64,
        "layout": "444",
        "quality": 99,
        "bit_depth": 10,
        "pattern": "fine",
        "cdf_mode": 0,
        "sha256": "7d7f03bd17e30b7f1bfc1bc2229b9c38b18ec8921977596eca5040b45fb4fe6c"
    },
    "animated_lossy_inter_444_12bit_fine_q99_cdf_frozen_16x64": {
        "input_name": "vertical-444-q99",
        "width": 16,
        "height": 64,
        "layout": "444",
        "quality": 99,
        "bit_depth": 12,
        "pattern": "fine",
        "cdf_mode": 0,
        "sha256": "eb3a6a9f4165924af98cfdef3a1611a86038ac2f5808049d65f2bfba8168ef03"
    },
    "animated_lossy_inter_444_12bit_fine_q99_cdf_selective_16x64": {
        "input_name": "vertical-444-q99",
        "width": 16,
        "height": 64,
        "layout": "444",
        "quality": 99,
        "bit_depth": 12,
        "pattern": "fine",
        "cdf_mode": 2,
        "sha256": "a8a01649a805d6be1d249b3c5523f5d8cc3288830570e9cf2d8e03b6c997e08b"
    },
    "animated_lossy_inter_444_8bit_fine_q99_cdf_frozen_16x64": {
        "input_name": "vertical-444-q99",
        "width": 16,
        "height": 64,
        "layout": "444",
        "quality": 99,
        "bit_depth": 8,
        "pattern": "fine",
        "cdf_mode": 0,
        "sha256": "2aa825afb1860041b9ee6f3b2879fa072588c4167e52c8beac98a0620317b9f0"
    },
    "animated_lossy_inter_400_10bit_b32x8_fine_q70_64x32": {
        "input_name": "wide8-horizontal-400-q70",
        "width": 64,
        "height": 32,
        "layout": "400",
        "quality": 70,
        "bit_depth": 10,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "936f5702d16d93e8b091aa7f64ab89dd349f4e250f4bd3bc7d69f179eaf97042"
    },
    "animated_lossy_inter_400_10bit_b32x8_fine_q99_64x32": {
        "input_name": "wide8-horizontal-400-q99",
        "width": 64,
        "height": 32,
        "layout": "400",
        "quality": 99,
        "bit_depth": 10,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "9cfcd227a4e014bdbd681620d21da95bfbb79c8f00148e93176f2cc9b6e388e3"
    },
    "animated_lossy_inter_400_10bit_b8x32_fine_q70_32x64": {
        "input_name": "wide8-vertical-400-q70",
        "width": 32,
        "height": 64,
        "layout": "400",
        "quality": 70,
        "bit_depth": 10,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "d49067d191e2148f4b0d94dc4b50052aa3683e4d6f07ae56a3e9aa7a90b3a236"
    },
    "animated_lossy_inter_400_10bit_b8x32_fine_q99_32x64": {
        "input_name": "wide8-vertical-400-q99",
        "width": 32,
        "height": 64,
        "layout": "400",
        "quality": 99,
        "bit_depth": 10,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "54b75f5ff4f9446e28d5871cfdff047d8cb5dbf5163254237da785cf47163bc7"
    },
    "animated_lossy_inter_400_12bit_b32x8_fine_q70_64x32": {
        "input_name": "wide8-horizontal-400-q70",
        "width": 64,
        "height": 32,
        "layout": "400",
        "quality": 70,
        "bit_depth": 12,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "e8ad9ca967b958299ef4198ae2b430b6ff350f1deee51160ad70ec7889662145"
    },
    "animated_lossy_inter_400_12bit_b32x8_fine_q99_64x32": {
        "input_name": "wide8-horizontal-400-q99",
        "width": 64,
        "height": 32,
        "layout": "400",
        "quality": 99,
        "bit_depth": 12,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "7b6bd9fe5630455fff39da47ca1869bd8c9a00f31d3aeda1135f850f4e48ee93"
    },
    "animated_lossy_inter_400_12bit_b8x32_fine_q70_32x64": {
        "input_name": "wide8-vertical-400-q70",
        "width": 32,
        "height": 64,
        "layout": "400",
        "quality": 70,
        "bit_depth": 12,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "3fdca072f9ea166dd1032fbbf35dad59f3e04663077583f194b7079ef9cf2b27"
    },
    "animated_lossy_inter_400_12bit_b8x32_fine_q99_32x64": {
        "input_name": "wide8-vertical-400-q99",
        "width": 32,
        "height": 64,
        "layout": "400",
        "quality": 99,
        "bit_depth": 12,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "07ff6adfac88fea007223ec7493c5707adabc4597537fd7ffae240c5204d621f"
    },
    "animated_lossy_inter_400_8bit_b32x8_fine_q70_64x32": {
        "input_name": "wide8-horizontal-400-q70",
        "width": 64,
        "height": 32,
        "layout": "400",
        "quality": 70,
        "bit_depth": 8,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "6499e9c83b5dbe6d6e7e1d029b700c4cee72e6c7d27b45950c39e733981fc06b"
    },
    "animated_lossy_inter_400_8bit_b32x8_fine_q99_64x32": {
        "input_name": "wide8-horizontal-400-q99",
        "width": 64,
        "height": 32,
        "layout": "400",
        "quality": 99,
        "bit_depth": 8,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "beb888e6e4a654b2698b4490fa773c964553aca420dc3c45184ea54f0a62d444"
    },
    "animated_lossy_inter_400_8bit_b8x32_fine_q70_32x64": {
        "input_name": "wide8-vertical-400-q70",
        "width": 32,
        "height": 64,
        "layout": "400",
        "quality": 70,
        "bit_depth": 8,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "4f5c62c9046b1de7cd743437af31b4faea91fb684b4c95ec9a5d6f37e0a3d8b7"
    },
    "animated_lossy_inter_400_8bit_b8x32_fine_q99_32x64": {
        "input_name": "wide8-vertical-400-q99",
        "width": 32,
        "height": 64,
        "layout": "400",
        "quality": 99,
        "bit_depth": 8,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "de726377979f411e50489c1c80597ff98b568dcc16e63bc39c21a8c5e92b16a2"
    },
    "animated_lossy_inter_420_10bit_b32x8_fine_q70_64x32": {
        "input_name": "wide8-horizontal-420-q70",
        "width": 64,
        "height": 32,
        "layout": "420",
        "quality": 70,
        "bit_depth": 10,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "22af699de04142c20333811aecf1441c20788c06dd86f8a9184e5d480546a66d"
    },
    "animated_lossy_inter_420_10bit_b32x8_fine_q99_64x32": {
        "input_name": "wide8-horizontal-420-q99",
        "width": 64,
        "height": 32,
        "layout": "420",
        "quality": 99,
        "bit_depth": 10,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "c5f75a5ca0ae4bc44a9bc49012a1d6727c8bac886b4ebd40c5eba22aa04c6723"
    },
    "animated_lossy_inter_420_10bit_b8x32_fine_q70_32x64": {
        "input_name": "wide8-vertical-420-q70",
        "width": 32,
        "height": 64,
        "layout": "420",
        "quality": 70,
        "bit_depth": 10,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "c2725c659ae60b271b7ab35c87fd564c92b5eb76e17394c8774f8fa74880c5ec"
    },
    "animated_lossy_inter_420_10bit_b8x32_fine_q99_32x64": {
        "input_name": "wide8-vertical-420-q99",
        "width": 32,
        "height": 64,
        "layout": "420",
        "quality": 99,
        "bit_depth": 10,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "dec3100d93b7af1216048bb87e6fb70b813362eb6cbc223fe2bf716458c31436"
    },
    "animated_lossy_inter_420_12bit_b32x8_fine_q70_64x32": {
        "input_name": "wide8-horizontal-420-q70",
        "width": 64,
        "height": 32,
        "layout": "420",
        "quality": 70,
        "bit_depth": 12,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "081528b3dc6aefe10351e51c81356634634261858994babca18e4814ef4f0d62"
    },
    "animated_lossy_inter_420_12bit_b32x8_fine_q99_64x32": {
        "input_name": "wide8-horizontal-420-q99",
        "width": 64,
        "height": 32,
        "layout": "420",
        "quality": 99,
        "bit_depth": 12,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "46c8f5b31a3f4e9f03c153c9a533204b7ba811ec45f8b9b2d22ca8e2b59a2cef"
    },
    "animated_lossy_inter_420_12bit_b8x32_fine_q70_32x64": {
        "input_name": "wide8-vertical-420-q70",
        "width": 32,
        "height": 64,
        "layout": "420",
        "quality": 70,
        "bit_depth": 12,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "e6107b46f0b7ad9c7fae023c3fc2028b15d5a94f059482f4744c0a6ae3959b8a"
    },
    "animated_lossy_inter_420_12bit_b8x32_fine_q99_32x64": {
        "input_name": "wide8-vertical-420-q99",
        "width": 32,
        "height": 64,
        "layout": "420",
        "quality": 99,
        "bit_depth": 12,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "b1d63965da624bf5c03ae6a638c82bff3aaa9f2c73ead3996a025797f4d1d0b6"
    },
    "animated_lossy_inter_420_8bit_b32x8_fine_q70_64x32": {
        "input_name": "wide8-horizontal-420-q70",
        "width": 64,
        "height": 32,
        "layout": "420",
        "quality": 70,
        "bit_depth": 8,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "d329bfe0ab5f9a494079e82e777880a55842bb706e8e6fb620453eb1562e2647"
    },
    "animated_lossy_inter_420_8bit_b32x8_fine_q99_64x32": {
        "input_name": "wide8-horizontal-420-q99",
        "width": 64,
        "height": 32,
        "layout": "420",
        "quality": 99,
        "bit_depth": 8,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "ace7208719b374448050df467c0290b95a6ffbaae3ac54b06ed05811c02a1c65"
    },
    "animated_lossy_inter_420_8bit_b8x32_fine_q70_32x64": {
        "input_name": "wide8-vertical-420-q70",
        "width": 32,
        "height": 64,
        "layout": "420",
        "quality": 70,
        "bit_depth": 8,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "313407ca95b721e8cd6aafa257aa3c86d70b65495e700a8b8291cb24aa4347da"
    },
    "animated_lossy_inter_420_8bit_b8x32_fine_q99_32x64": {
        "input_name": "wide8-vertical-420-q99",
        "width": 32,
        "height": 64,
        "layout": "420",
        "quality": 99,
        "bit_depth": 8,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "fffe6bcfbcd058ff60da3e5ddcf66126a898a937bab2532eb73d3e82c7919862"
    },
    "animated_lossy_inter_422_10bit_b32x8_fine_q70_64x32": {
        "input_name": "wide8-horizontal-422-q70",
        "width": 64,
        "height": 32,
        "layout": "422",
        "quality": 70,
        "bit_depth": 10,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "ed20922dc874f9a76c3876a44cd1ddd781c7e3a9d1b7fd963ca5f3a5efceac81"
    },
    "animated_lossy_inter_422_10bit_b32x8_fine_q99_64x32": {
        "input_name": "wide8-horizontal-422-q99",
        "width": 64,
        "height": 32,
        "layout": "422",
        "quality": 99,
        "bit_depth": 10,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "9c6cd0216ea53b6b02d16576e355ca5477a4ff05562f8711244423624a3d69a0"
    },
    "animated_lossy_inter_422_12bit_b32x8_fine_q70_64x32": {
        "input_name": "wide8-horizontal-422-q70",
        "width": 64,
        "height": 32,
        "layout": "422",
        "quality": 70,
        "bit_depth": 12,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "620945fb47fea0c038a4feaae766140b620ac5749d87073759154ba6a05a7b32"
    },
    "animated_lossy_inter_422_12bit_b32x8_fine_q99_64x32": {
        "input_name": "wide8-horizontal-422-q99",
        "width": 64,
        "height": 32,
        "layout": "422",
        "quality": 99,
        "bit_depth": 12,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "e1db7df8902375e481d48ef795b0e4bbe4c47a79c99b2f2e47de93f71798f392"
    },
    "animated_lossy_inter_422_8bit_b32x8_fine_q70_64x32": {
        "input_name": "wide8-horizontal-422-q70",
        "width": 64,
        "height": 32,
        "layout": "422",
        "quality": 70,
        "bit_depth": 8,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "bf47c9d891722e2370db4dfa013aaadeb5e6e00951f5a3468132914ad264b464"
    },
    "animated_lossy_inter_422_8bit_b32x8_fine_q99_64x32": {
        "input_name": "wide8-horizontal-422-q99",
        "width": 64,
        "height": 32,
        "layout": "422",
        "quality": 99,
        "bit_depth": 8,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "624272ee6fc567be1e21ab17f04907d9f140bf243aba7d7ea793d7464a8391b0"
    },
    "animated_lossy_inter_444_10bit_b32x8_fine_q70_64x32": {
        "input_name": "wide8-horizontal-444-q70",
        "width": 64,
        "height": 32,
        "layout": "444",
        "quality": 70,
        "bit_depth": 10,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "a06e87aa12fd4426b2c4fb25a6cbf64b78afc3093812c19c973617a32f22ed2d"
    },
    "animated_lossy_inter_444_10bit_b32x8_fine_q99_64x32": {
        "input_name": "wide8-horizontal-444-q99",
        "width": 64,
        "height": 32,
        "layout": "444",
        "quality": 99,
        "bit_depth": 10,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "f795474b4ce43d9e220d3b1ab7c2cf601a4222bf3b2928ec17b1ad896770db06"
    },
    "animated_lossy_inter_444_10bit_b8x32_fine_q70_32x64": {
        "input_name": "wide8-vertical-444-q70",
        "width": 32,
        "height": 64,
        "layout": "444",
        "quality": 70,
        "bit_depth": 10,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "5de06160e7a732f1a665aab3833a18efb24e396af70a11fac22e58f40b3e8846"
    },
    "animated_lossy_inter_444_10bit_b8x32_fine_q99_32x64": {
        "input_name": "wide8-vertical-444-q99",
        "width": 32,
        "height": 64,
        "layout": "444",
        "quality": 99,
        "bit_depth": 10,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "8b64087b62639992a4a4d95e016bb20f5ae5702e9d0cd63d3e83f9b17eeaa073"
    },
    "animated_lossy_inter_444_12bit_b32x8_fine_q70_64x32": {
        "input_name": "wide8-horizontal-444-q70",
        "width": 64,
        "height": 32,
        "layout": "444",
        "quality": 70,
        "bit_depth": 12,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "d1ddc1629ae10d202960d6710077c8fd977829cb16d8a99520c42521aa365213"
    },
    "animated_lossy_inter_444_12bit_b32x8_fine_q99_64x32": {
        "input_name": "wide8-horizontal-444-q99",
        "width": 64,
        "height": 32,
        "layout": "444",
        "quality": 99,
        "bit_depth": 12,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "d3100bb64ec47796a838b2082cd6d1f76cce8f081ee5a00364d40136c177cd1f"
    },
    "animated_lossy_inter_444_12bit_b8x32_fine_q70_32x64": {
        "input_name": "wide8-vertical-444-q70",
        "width": 32,
        "height": 64,
        "layout": "444",
        "quality": 70,
        "bit_depth": 12,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "69a5e083e0b88b3a92e1d465251867833904f9e26208a2860ae620d9fc603da8"
    },
    "animated_lossy_inter_444_12bit_b8x32_fine_q99_32x64": {
        "input_name": "wide8-vertical-444-q99",
        "width": 32,
        "height": 64,
        "layout": "444",
        "quality": 99,
        "bit_depth": 12,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "a65e5f31a658000a530198fdb1fec48600265993e02409f0dbfd3859d695615c"
    },
    "animated_lossy_inter_444_8bit_b32x8_fine_q70_64x32": {
        "input_name": "wide8-horizontal-444-q70",
        "width": 64,
        "height": 32,
        "layout": "444",
        "quality": 70,
        "bit_depth": 8,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "f311b4b5f8315ed9d43496348f4b74707d0f58fab779345c4153ce328a6df1bc"
    },
    "animated_lossy_inter_444_8bit_b32x8_fine_q99_64x32": {
        "input_name": "wide8-horizontal-444-q99",
        "width": 64,
        "height": 32,
        "layout": "444",
        "quality": 99,
        "bit_depth": 8,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "4aa8e523e1f83a95b39fd65f468910055892af1eda7386a1ca4575fbe3efbb55"
    },
    "animated_lossy_inter_444_8bit_b8x32_fine_q70_32x64": {
        "input_name": "wide8-vertical-444-q70",
        "width": 32,
        "height": 64,
        "layout": "444",
        "quality": 70,
        "bit_depth": 8,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "f62b283664c0998a04c8f8e15ed4ae6c9155d7b668cf40ad678b89de0a6379df"
    },
    "animated_lossy_inter_444_8bit_b8x32_fine_q99_32x64": {
        "input_name": "wide8-vertical-444-q99",
        "width": 32,
        "height": 64,
        "layout": "444",
        "quality": 99,
        "bit_depth": 8,
        "pattern": "fine",
        "root_size": 32,
        "sha256": "d32bb7e65e591b1d1cd8a1ef222d08032e8eb2a1d08fc8c562eaa799be093429"
    },
    "animated_lossy_inter_400_10bit_b16x64_fine_q70_64x128": {
        "input_name": "wide16-vertical-400-q70",
        "width": 64,
        "height": 128,
        "layout": "400",
        "quality": 70,
        "bit_depth": 10,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "49bad6f01d2beeb984b0c3870f8007c86f0e74b1caabcf2f040f354b69b9ede7"
    },
    "animated_lossy_inter_400_10bit_b16x64_fine_q99_64x128": {
        "input_name": "wide16-vertical-400-q99",
        "width": 64,
        "height": 128,
        "layout": "400",
        "quality": 99,
        "bit_depth": 10,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "ca9560fe84b4aa3d5a7ed8c003fe683c7ae0d9e04cd4d2f604dee4c19d1db90f"
    },
    "animated_lossy_inter_400_10bit_b64x16_fine_q70_128x64": {
        "input_name": "wide16-horizontal-400-q70",
        "width": 128,
        "height": 64,
        "layout": "400",
        "quality": 70,
        "bit_depth": 10,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "3ad12ae941fb951539b1175274ef94a87ab8ce421e290c02229208e78fff5339"
    },
    "animated_lossy_inter_400_10bit_b64x16_fine_q99_128x64": {
        "input_name": "wide16-horizontal-400-q99",
        "width": 128,
        "height": 64,
        "layout": "400",
        "quality": 99,
        "bit_depth": 10,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "cb9a102a9cf51b4775351a376bcaad9ae394563871d4cbc68c34d3834aa50b3d"
    },
    "animated_lossy_inter_400_12bit_b16x64_fine_q70_64x128": {
        "input_name": "wide16-vertical-400-q70",
        "width": 64,
        "height": 128,
        "layout": "400",
        "quality": 70,
        "bit_depth": 12,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "4671c3d0e852df04bf1d622e4f65761106c071902e710c6ccb825344817f7252"
    },
    "animated_lossy_inter_400_12bit_b16x64_fine_q99_64x128": {
        "input_name": "wide16-vertical-400-q99",
        "width": 64,
        "height": 128,
        "layout": "400",
        "quality": 99,
        "bit_depth": 12,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "6689524391098bc11d49d3ef94c01e2322adddcb01a26f51dbf106b4b7906ad4"
    },
    "animated_lossy_inter_400_12bit_b64x16_fine_q70_128x64": {
        "input_name": "wide16-horizontal-400-q70",
        "width": 128,
        "height": 64,
        "layout": "400",
        "quality": 70,
        "bit_depth": 12,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "5e0706f835ed08e3b748867e78dd5997bf68db7d7d88a7967578c22b76973ef0"
    },
    "animated_lossy_inter_400_12bit_b64x16_fine_q99_128x64": {
        "input_name": "wide16-horizontal-400-q99",
        "width": 128,
        "height": 64,
        "layout": "400",
        "quality": 99,
        "bit_depth": 12,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "7ed4f24c091ebe3f4fe2c9baeb5f29e13f4f1b44bec7b09d93336886531a0d6b"
    },
    "animated_lossy_inter_400_8bit_b16x64_fine_q70_64x128": {
        "input_name": "wide16-vertical-400-q70",
        "width": 64,
        "height": 128,
        "layout": "400",
        "quality": 70,
        "bit_depth": 8,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "90b13958df771aec1a129a9c0bf420a66a2208b28ba948479c6ae9458e03ed6a"
    },
    "animated_lossy_inter_400_8bit_b16x64_fine_q99_64x128": {
        "input_name": "wide16-vertical-400-q99",
        "width": 64,
        "height": 128,
        "layout": "400",
        "quality": 99,
        "bit_depth": 8,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "c00403f756dcb108d406ce8f45f7764edb2fd86ae58bf779fe7e4dd934df79b1"
    },
    "animated_lossy_inter_400_8bit_b64x16_fine_q70_128x64": {
        "input_name": "wide16-horizontal-400-q70",
        "width": 128,
        "height": 64,
        "layout": "400",
        "quality": 70,
        "bit_depth": 8,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "a194dac95b7c59f896b07ae2a3e69c12830b3b70be7d349a42d2dbdd76bee499"
    },
    "animated_lossy_inter_400_8bit_b64x16_fine_q99_128x64": {
        "input_name": "wide16-horizontal-400-q99",
        "width": 128,
        "height": 64,
        "layout": "400",
        "quality": 99,
        "bit_depth": 8,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "3b2eaabf9fa7b123d1fc765a43969a7fd7b44bd6ffdda5de190bafe46ea2c3bc"
    },
    "animated_lossy_inter_420_10bit_b16x64_fine_q70_64x128": {
        "input_name": "wide16-vertical-420-q70",
        "width": 64,
        "height": 128,
        "layout": "420",
        "quality": 70,
        "bit_depth": 10,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "5c257fda08669a989e7f04e819d4e59e31425c8c55f5cf308fc33f0c290b1f16"
    },
    "animated_lossy_inter_420_10bit_b16x64_fine_q99_64x128": {
        "input_name": "wide16-vertical-420-q99",
        "width": 64,
        "height": 128,
        "layout": "420",
        "quality": 99,
        "bit_depth": 10,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "3d8f720022101a8a6b447600328d346f8c670ce085a0bc4f48b51024ce74ee22"
    },
    "animated_lossy_inter_420_10bit_b64x16_fine_q70_128x64": {
        "input_name": "wide16-horizontal-420-q70",
        "width": 128,
        "height": 64,
        "layout": "420",
        "quality": 70,
        "bit_depth": 10,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "ee7758bf8934caa079e1de77d038f3f4686ad001b49cc7b383b91612bd372f6b"
    },
    "animated_lossy_inter_420_10bit_b64x16_fine_q99_128x64": {
        "input_name": "wide16-horizontal-420-q99",
        "width": 128,
        "height": 64,
        "layout": "420",
        "quality": 99,
        "bit_depth": 10,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "019fa5687bebfdfc2d3913b48bb5436f0b8d0e638566d19a5a36b84a092669f5"
    },
    "animated_lossy_inter_420_12bit_b16x64_fine_q70_64x128": {
        "input_name": "wide16-vertical-420-q70",
        "width": 64,
        "height": 128,
        "layout": "420",
        "quality": 70,
        "bit_depth": 12,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "972476dbddb95d4d9c19492c5258f5bc0742ce3b50926c45166230e074263ba9"
    },
    "animated_lossy_inter_420_12bit_b16x64_fine_q99_64x128": {
        "input_name": "wide16-vertical-420-q99",
        "width": 64,
        "height": 128,
        "layout": "420",
        "quality": 99,
        "bit_depth": 12,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "c7d228764354de4529614bc5789b88b16a6badbff3ab6ac14c7434e9ef778959"
    },
    "animated_lossy_inter_420_12bit_b64x16_fine_q70_128x64": {
        "input_name": "wide16-horizontal-420-q70",
        "width": 128,
        "height": 64,
        "layout": "420",
        "quality": 70,
        "bit_depth": 12,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "1adc5f8be84cfa7c217d3e9fb357c7977717deeeff198b6597704efeaeca8e68"
    },
    "animated_lossy_inter_420_12bit_b64x16_fine_q99_128x64": {
        "input_name": "wide16-horizontal-420-q99",
        "width": 128,
        "height": 64,
        "layout": "420",
        "quality": 99,
        "bit_depth": 12,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "f21e305a093004847824757adbfffbb8f9b3d1c631954e4ffddb6dccb9244d93"
    },
    "animated_lossy_inter_420_8bit_b16x64_fine_q70_64x128": {
        "input_name": "wide16-vertical-420-q70",
        "width": 64,
        "height": 128,
        "layout": "420",
        "quality": 70,
        "bit_depth": 8,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "503cc5d32348d98b6a4f35326ed68ac8be16a16b67749ce109dfa7f05a057a18"
    },
    "animated_lossy_inter_420_8bit_b16x64_fine_q99_64x128": {
        "input_name": "wide16-vertical-420-q99",
        "width": 64,
        "height": 128,
        "layout": "420",
        "quality": 99,
        "bit_depth": 8,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "8983ec15d4c75bebef5e9c80b7d7428d64cbbc1f454924420f33924e9083311d"
    },
    "animated_lossy_inter_420_8bit_b64x16_fine_q70_128x64": {
        "input_name": "wide16-horizontal-420-q70",
        "width": 128,
        "height": 64,
        "layout": "420",
        "quality": 70,
        "bit_depth": 8,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "4f256bdc7b27f92467c8ef0828d9a7f14df5e47a717ba6dd212d2f036a52b081"
    },
    "animated_lossy_inter_420_8bit_b64x16_fine_q99_128x64": {
        "input_name": "wide16-horizontal-420-q99",
        "width": 128,
        "height": 64,
        "layout": "420",
        "quality": 99,
        "bit_depth": 8,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "5d3e7d46b57ad063b47e2a356dc0b4f5b303b4e8d5d6e1d91f7f08f225b95c39"
    },
    "animated_lossy_inter_422_10bit_b64x16_fine_q70_128x64": {
        "input_name": "wide16-horizontal-422-q70",
        "width": 128,
        "height": 64,
        "layout": "422",
        "quality": 70,
        "bit_depth": 10,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "d3fbac1b2774b170fc74d906e38a91dd836743f5a78450e71b22be0f736a91a2"
    },
    "animated_lossy_inter_422_10bit_b64x16_fine_q99_128x64": {
        "input_name": "wide16-horizontal-422-q99",
        "width": 128,
        "height": 64,
        "layout": "422",
        "quality": 99,
        "bit_depth": 10,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "cb9d15caeae2d0752b352d017ea9ea5dffe3c2749d28b46dbfd7dac901cd10d8"
    },
    "animated_lossy_inter_422_12bit_b64x16_fine_q70_128x64": {
        "input_name": "wide16-horizontal-422-q70",
        "width": 128,
        "height": 64,
        "layout": "422",
        "quality": 70,
        "bit_depth": 12,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "361ae1722b3f09b6272e7c0ddee39b508ae090912fb966745cba7a6f415e2b63"
    },
    "animated_lossy_inter_422_12bit_b64x16_fine_q99_128x64": {
        "input_name": "wide16-horizontal-422-q99",
        "width": 128,
        "height": 64,
        "layout": "422",
        "quality": 99,
        "bit_depth": 12,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "3d5c3acea1c71099fcd940d37035e2c475f92b739416a67736bf4a40191764ac"
    },
    "animated_lossy_inter_422_8bit_b64x16_fine_q70_128x64": {
        "input_name": "wide16-horizontal-422-q70",
        "width": 128,
        "height": 64,
        "layout": "422",
        "quality": 70,
        "bit_depth": 8,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "a969a0b3ae5b95f2a207413b416dc46eedd4f9d6ed0a655f237ff26e24849c01"
    },
    "animated_lossy_inter_422_8bit_b64x16_fine_q99_128x64": {
        "input_name": "wide16-horizontal-422-q99",
        "width": 128,
        "height": 64,
        "layout": "422",
        "quality": 99,
        "bit_depth": 8,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "9d409b9a083f5a96f3c5b3716b323c1fcb3c19b34e50646c248cd752d647bcdc"
    },
    "animated_lossy_inter_444_10bit_b16x64_fine_q70_64x128": {
        "input_name": "wide16-vertical-444-q70",
        "width": 64,
        "height": 128,
        "layout": "444",
        "quality": 70,
        "bit_depth": 10,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "a0c9488b3b586b87a900933627c0440c216e9291c56e37133625a2b6f2c6f425"
    },
    "animated_lossy_inter_444_10bit_b16x64_fine_q99_64x128": {
        "input_name": "wide16-vertical-444-q99",
        "width": 64,
        "height": 128,
        "layout": "444",
        "quality": 99,
        "bit_depth": 10,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "ed5619bacc370fcd6c96109769898fdb5150c96015ba3b113219bd1e97994781"
    },
    "animated_lossy_inter_444_10bit_b64x16_fine_q70_128x64": {
        "input_name": "wide16-horizontal-444-q70",
        "width": 128,
        "height": 64,
        "layout": "444",
        "quality": 70,
        "bit_depth": 10,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "aa0938c000cb88daf73f1e7c7f7d6e6436f3fabdc529820f9699137eb4d63c5b"
    },
    "animated_lossy_inter_444_10bit_b64x16_fine_q99_128x64": {
        "input_name": "wide16-horizontal-444-q99",
        "width": 128,
        "height": 64,
        "layout": "444",
        "quality": 99,
        "bit_depth": 10,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "135be2b0a4e031bfd563e2ee15d6a84721682fd60e0a299094edee7317ece272"
    },
    "animated_lossy_inter_444_12bit_b16x64_fine_q70_64x128": {
        "input_name": "wide16-vertical-444-q70",
        "width": 64,
        "height": 128,
        "layout": "444",
        "quality": 70,
        "bit_depth": 12,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "ebb47c4009befc31a837fd357d629aae6906272c5e33d54b248dbc14c7138238"
    },
    "animated_lossy_inter_444_12bit_b16x64_fine_q99_64x128": {
        "input_name": "wide16-vertical-444-q99",
        "width": 64,
        "height": 128,
        "layout": "444",
        "quality": 99,
        "bit_depth": 12,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "40642ad5137705d5223adab0e84e497165b98f1394872e0dc79c18d31fe35767"
    },
    "animated_lossy_inter_444_12bit_b64x16_fine_q70_128x64": {
        "input_name": "wide16-horizontal-444-q70",
        "width": 128,
        "height": 64,
        "layout": "444",
        "quality": 70,
        "bit_depth": 12,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "8347cdaa8c2beff489e17bb65800efe1cb893a6827a2f05984784ab20ee96e8c"
    },
    "animated_lossy_inter_444_12bit_b64x16_fine_q99_128x64": {
        "input_name": "wide16-horizontal-444-q99",
        "width": 128,
        "height": 64,
        "layout": "444",
        "quality": 99,
        "bit_depth": 12,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "7739ead4747e3c877d223b9eeb458852b6baf6539ab75c0f32d05851c54bd121"
    },
    "animated_lossy_inter_444_8bit_b16x64_fine_q70_64x128": {
        "input_name": "wide16-vertical-444-q70",
        "width": 64,
        "height": 128,
        "layout": "444",
        "quality": 70,
        "bit_depth": 8,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "41e1b3bea261d0114cefad676d74efd5ece895ac963e4fe9588479497c176d8c"
    },
    "animated_lossy_inter_444_8bit_b16x64_fine_q99_64x128": {
        "input_name": "wide16-vertical-444-q99",
        "width": 64,
        "height": 128,
        "layout": "444",
        "quality": 99,
        "bit_depth": 8,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "a2cfdd6925197628aee4de3217618955f47a438e19d59e49a496ce4dfaf1ad35"
    },
    "animated_lossy_inter_444_8bit_b64x16_fine_q70_128x64": {
        "input_name": "wide16-horizontal-444-q70",
        "width": 128,
        "height": 64,
        "layout": "444",
        "quality": 70,
        "bit_depth": 8,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "4a91227df1be5279658e9aabeee020e635d880297419ed5fcecd3801829ddef1"
    },
    "animated_lossy_inter_444_8bit_b64x16_fine_q99_128x64": {
        "input_name": "wide16-horizontal-444-q99",
        "width": 128,
        "height": 64,
        "layout": "444",
        "quality": 99,
        "bit_depth": 8,
        "pattern": "fine",
        "root_size": 64,
        "sha256": "a10d281a2423a1e7bf7420c8cb8e6233a8cb5f9aeb194186a1e067daee3767c8"
    },
}

FIXTURE_SHA256 = {name: spec["sha256"] for name, spec in VARIANTS.items()}


def validate(path: Path, spec: dict) -> None:
    """Check the complete input identity and live oracle/header contract."""
    encoded = path.read_bytes()
    if hashlib.sha256(encoded).hexdigest() != spec["sha256"]:
        raise RuntimeError(f"fixture source changed: {path}")
    if normalize_creation_timestamps(encoded) != encoded:
        raise RuntimeError(f"fixture timestamps are not normalized: {path}")
    with Image.open(path) as image:
        if (image.format, image.mode, image.size, image.n_frames) != (
            "AVIF", "RGB", (spec["width"], spec["height"]), 2
        ):
            raise RuntimeError(f"unexpected Pillow sequence metadata: {path}")
        for frame in range(image.n_frames):
            image.seek(frame)
            image.load()
            if image.info.get("duration") != 100:
                raise RuntimeError(f"unexpected Pillow frame duration: {path}")
    report = inspect(path)
    samples = [sample for sample in report["samples"] if sample.get("role") == "track_pict"]
    headers = [obu["frame_header"] for sample in samples
               for obu in sample["obus"] if "frame_header" in obu]
    sequences = [obu["sequence_header"] for sample in samples
                 for obu in sample["obus"] if "sequence_header" in obu]
    layout = spec["layout"]
    if (len(samples) != 2 or len(headers) != 2 or not sequences
        or [header.get("frame_type") for header in headers] != ["key", "inter"]
        or any(sequence.get("bit_depth") != spec.get("bit_depth", 8)
               or sequence.get("subsampling_x") != int(layout != "444")
               or sequence.get("subsampling_y") != int(layout in ("400", "420"))
               or sequence.get("monochrome") != (layout == "400")
               for sequence in sequences)
        or headers[1].get("all_lossless") is not False):
        raise RuntimeError(f"unexpected AV1 header contract: {path}")
    if "cdf_mode" in spec:
        expected_frozen = {
            0: [True, True],
            1: [False, False],
            2: [False, True],
        }[spec["cdf_mode"]]
        if [header.get("disable_cdf_update") for header in headers] != expected_frozen:
            raise RuntimeError(f"unexpected AV1 CDF update contract: {path}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--encoder-dir", type=Path,
                        help="directory containing the native encode-{orientation}-{layout}-{quality} tools")
    parser.add_argument("--output-dir", type=Path, default=OUTPUT)
    parser.add_argument("--variant", choices=sorted(VARIANTS))
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    require_pinned_oracle()
    if not args.check and args.encoder_dir is None:
        parser.error("--encoder-dir is required when generating inputs")
    variants = {args.variant: VARIANTS[args.variant]} if args.variant else VARIANTS
    for name, spec in variants.items():
        output = args.output_dir.resolve() / f"{name}.avif"
        if not args.check:
            encoder = args.encoder_dir.resolve() / f"encode-{spec['input_name'].replace('-q', '-')}"
            with tempfile.TemporaryDirectory(prefix="avif-narrow-inter-") as directory:
                paths = [Path(directory) / f"encoded-{repeat}.avif" for repeat in range(2)]
                for path in paths:
                    command = [str(encoder), str(path), "select", str(spec.get("bit_depth", 8))]
                    if "pattern" in spec or "cdf_mode" in spec:
                        command.append(spec.get("pattern", "coarse"))
                    if "cdf_mode" in spec:
                        command.append(str(spec["cdf_mode"]))
                    subprocess.run(command, check=True, cwd=ROOT)
                encoded = [normalize_creation_timestamps(path.read_bytes()) for path in paths]
                if encoded[0] != encoded[1]:
                    raise RuntimeError(f"native fixture producer is not deterministic: {name}")
                paths[0].write_bytes(encoded[0])
                validate(paths[0], spec)
                if hashlib.sha256(encoded[0]).hexdigest() != spec["sha256"]:
                    raise RuntimeError(f"native fixture source changed: {name}")
                if output.exists() and output.read_bytes() != encoded[0]:
                    raise RuntimeError(f"refusing to replace differing fixture: {output}")
                output.parent.mkdir(parents=True, exist_ok=True)
                if not output.exists():
                    output.write_bytes(encoded[0])
        validate(output, spec)
        print(f"Verified {output.name}")


if __name__ == "__main__":
    main()
