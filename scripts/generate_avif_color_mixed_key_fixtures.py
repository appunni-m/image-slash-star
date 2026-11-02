#!/usr/bin/env python3
"""Generate input-only color AVIF sequences with mixed-segment key frames."""
from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile
from typing import NamedTuple

from generate_avif_10bit_lossless_inter_fixture import normalize_sequence_timestamps
from generate_avif_filmgrain_edge_fixtures import require_pinned_oracle
from inspect_av1_obus import inspect

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "tests/fixtures/input/images/avif"


class Fixture(NamedTuple):
    """Input-only native recipe and complete normalized file identity."""

    width: int
    height: int
    depth: int
    layout: str
    speed: int
    restoration: int
    varied_key: bool
    denominator: int
    input_sha256: str
    motion_key: bool = False
    integer_key: bool = False


FIXTURES = {
    "animated_mixed_lossless_key_superres_420_8bit_integer_mv_s3_128x48_d10":
        Fixture(128, 48, 8, "420", 3, 0, True, 10,
                "ad52321fb01335ed4ea68990a12d0680b0c9a124fc2647302685b820b686595f", integer_key=True),
    "animated_mixed_lossless_key_superres_420_10bit_integer_mv_s3_128x48_d10":
        Fixture(128, 48, 10, "420", 3, 0, True, 10,
                "5695f864c484707f0cf3fa13fdb52641e20f168745007906b7174cf151a13c92", integer_key=True),
    "animated_mixed_lossless_key_superres_420_12bit_integer_mv_s3_128x48_d10":
        Fixture(128, 48, 12, "420", 3, 0, True, 10,
                "549243ce45a00247538054f47626f2c4c4e08fa99e0070c27f7da2480d57a106", integer_key=True),
    "animated_mixed_lossless_key_superres_422_8bit_integer_mv_s3_128x48_d10":
        Fixture(128, 48, 8, "422", 3, 0, True, 10,
                "0e17d24734181180c5c45f3de7c480d13b28f3635d14448ee8ea6452eb2646a0", integer_key=True),
    "animated_mixed_lossless_key_superres_422_10bit_integer_mv_s3_128x48_d10":
        Fixture(128, 48, 10, "422", 3, 0, True, 10,
                "de12c3192b5353b2aa5d6697795cbec3b0c26fec5baacb485acc51ecfc9d39a1", integer_key=True),
    "animated_mixed_lossless_key_superres_422_12bit_integer_mv_s3_128x48_d10":
        Fixture(128, 48, 12, "422", 3, 0, True, 10,
                "14b220fc4a288328e7965ca2b9d365f5ebbe58e53da1aee33b31b546493552fb", integer_key=True),
    "animated_mixed_lossless_key_superres_444_8bit_integer_mv_s3_128x48_d10":
        Fixture(128, 48, 8, "444", 3, 0, True, 10,
                "21d46ba6643955f8d8a9e9afd709cf831d7f019c2438a829f35453c9b9523618", integer_key=True),
    "animated_mixed_lossless_key_superres_444_10bit_integer_mv_s3_128x48_d10":
        Fixture(128, 48, 10, "444", 3, 0, True, 10,
                "cd2250eb22fea4c8cf6e02314b4aa105dbcedb7f1666ff190f0b29d6f6df9862", integer_key=True),
    "animated_mixed_lossless_key_superres_444_12bit_integer_mv_s3_128x48_d10":
        Fixture(128, 48, 12, "444", 3, 0, True, 10,
                "0a2688ed113caff925559dd2304217757e69931fae0f239715f3a68e231abac7", integer_key=True),
    "animated_mixed_lossless_key_superres_420_8bit_translated_s3_128x48_d10":
        Fixture(128, 48, 8, "420", 3, 0, True, 10,
                "512c6abd6039fe3039ddc6547b1da8ccfd96e628b6b47a32695e46140a5fdfc8", True),
    "animated_mixed_lossless_key_superres_420_10bit_translated_s3_128x48_d10":
        Fixture(128, 48, 10, "420", 3, 0, True, 10,
                "a002af881b6df0e20b0ad3747f75e70d620ed05f2a2d8671fd625f04fe2d22f2", True),
    "animated_mixed_lossless_key_superres_420_12bit_translated_s3_128x48_d10":
        Fixture(128, 48, 12, "420", 3, 0, True, 10,
                "06e12a6dd89b61fe6aed9540ce7cd36d459db89c76ddee33e3d7401b16b9175e", True),
    "animated_mixed_lossless_key_superres_422_8bit_translated_s3_128x48_d10":
        Fixture(128, 48, 8, "422", 3, 0, True, 10,
                "18734ef99935eb482aec5b0de34a89a6eae29d7b83f27167416d5945d1c9588b", True),
    "animated_mixed_lossless_key_superres_422_10bit_translated_s3_128x48_d10":
        Fixture(128, 48, 10, "422", 3, 0, True, 10,
                "045a9f481b1aa5f4655f2a2e8380fa706e0a8d05654487e13381c96e36e96f9f", True),
    "animated_mixed_lossless_key_superres_422_12bit_translated_s3_128x48_d10":
        Fixture(128, 48, 12, "422", 3, 0, True, 10,
                "c3ee332a015f35d588a2ac60bbe9b814030d0fd6e5d0dbad3db3f0a961194185", True),
    "animated_mixed_lossless_key_superres_444_8bit_translated_s3_128x48_d10":
        Fixture(128, 48, 8, "444", 3, 0, True, 10,
                "5d92832b1dff264387701705a09c61829de6ea6eb71b7aa47510658cc90c412e", True),
    "animated_mixed_lossless_key_superres_444_10bit_translated_s3_128x48_d10":
        Fixture(128, 48, 10, "444", 3, 0, True, 10,
                "218738a70fcea72cc1aa2fe7fa6ca108184e275e542cbcbaac02790ca5da8da5", True),
    "animated_mixed_lossless_key_superres_444_12bit_translated_s3_128x48_d10":
        Fixture(128, 48, 12, "444", 3, 0, True, 10,
                "eacb7b4e3259106c3fdb924dee4e713746617ec2c1ca8575d53b6d622d798fc5", True),
    'animated_mixed_lossless_key_superres_420_8bit_flat_s3_128x48_d10':
        Fixture(128, 48, 8, "420", 3, 0, False, 10,
                "776e63b097e3c5d4b4ea7ab9603e63881258e7cc50a3c0531ee4ee287c3f7480"),
    'animated_mixed_lossless_key_superres_420_10bit_flat_s3_128x48_d10':
        Fixture(128, 48, 10, "420", 3, 0, False, 10,
                "d7fb4428e1f4b4b245a6b2642a46192315b6936cc3bdbcabb6a122808efc675d"),
    'animated_mixed_lossless_key_superres_420_12bit_flat_s3_128x48_d10':
        Fixture(128, 48, 12, "420", 3, 0, False, 10,
                "9bf72dd104ac5dbe494558b715256ef319db96f23cc284c32a2f7d60ce94cbb2"),
    'animated_mixed_lossless_key_superres_422_8bit_flat_s3_128x48_d10':
        Fixture(128, 48, 8, "422", 3, 0, False, 10,
                "973a1ce88b4828fd00490b1376ccb24729b6ed49c936ba8683da3436526ece0d"),
    'animated_mixed_lossless_key_superres_422_10bit_flat_s3_128x48_d10':
        Fixture(128, 48, 10, "422", 3, 0, False, 10,
                "1ce67e109587258abfec2d472c0f94bbb867def145e9e5b88683ddc335147e81"),
    'animated_mixed_lossless_key_superres_422_12bit_flat_s3_128x48_d10':
        Fixture(128, 48, 12, "422", 3, 0, False, 10,
                "3053a2d11cb6a238bb1b30b47d5d322d2b6facff071c4e36953f5e55f4f9d9ea"),
    'animated_mixed_lossless_key_superres_444_8bit_flat_s3_128x48_d10':
        Fixture(128, 48, 8, "444", 3, 0, False, 10,
                "6a7e197f61128b03322793936997832aca3d7de6e7845a02f3d8e45108c34a14"),
    'animated_mixed_lossless_key_superres_444_10bit_flat_s3_128x48_d10':
        Fixture(128, 48, 10, "444", 3, 0, False, 10,
                "e323ce61904e0a4e3e068f025dd9c6aac934360512500ff227193d8f9db0d757"),
    'animated_mixed_lossless_key_superres_444_12bit_flat_s3_128x48_d10':
        Fixture(128, 48, 12, "444", 3, 0, False, 10,
                "7c9ffeb97a7c2ded144ebdb36c0ac7e4284dbaf994b20a6f2287548a0cb53d61"),
    'animated_mixed_lossless_key_superres_420_8bit_flat_s3_80x20_d9':
        Fixture(80, 20, 8, "420", 3, 0, False, 9,
                "d82211d4ba396acba17038f27a1e9fb982059371eea2a99bd581ad782dddc0da"),
    'animated_mixed_lossless_key_superres_420_10bit_flat_s3_80x20_d9':
        Fixture(80, 20, 10, "420", 3, 0, False, 9,
                "c9ca6b87aab7e7b90e5a31b96ceed04154e13cd3096e2965e0295fefc93b4174"),
    'animated_mixed_lossless_key_superres_420_12bit_flat_s3_80x20_d9':
        Fixture(80, 20, 12, "420", 3, 0, False, 9,
                "70c980197aeb9d58e79c18d6b96d4074b49a647c814eff70f4ed6223ee8605d3"),
    'animated_mixed_lossless_key_superres_422_8bit_flat_s3_80x20_d9':
        Fixture(80, 20, 8, "422", 3, 0, False, 9,
                "ac62bd8b39fd109d504e1e4e5ba04afa921f810b844c18b41244567bdd34199e"),
    'animated_mixed_lossless_key_superres_422_10bit_flat_s3_80x20_d9':
        Fixture(80, 20, 10, "422", 3, 0, False, 9,
                "f8a0e522528ec32842ad034e091f073690e379c8adf9556ed14f6f872dd60d55"),
    'animated_mixed_lossless_key_superres_422_12bit_flat_s3_80x20_d9':
        Fixture(80, 20, 12, "422", 3, 0, False, 9,
                "c15e4bf7561cdf2d4a2c31edb0d7bdef4ec15787c6c765ec9474b5d7dc3b7fc0"),
    'animated_mixed_lossless_key_superres_444_8bit_flat_s3_80x20_d9':
        Fixture(80, 20, 8, "444", 3, 0, False, 9,
                "14727f84ca1fc70fb9207daf59b8828417761059e0989632ba4456af6e82aaca"),
    'animated_mixed_lossless_key_superres_444_10bit_flat_s3_80x20_d9':
        Fixture(80, 20, 10, "444", 3, 0, False, 9,
                "d504234c0b8cbbeb862d588b233ce326a98cdd0561024f626ab2c0560e428b2f"),
    'animated_mixed_lossless_key_superres_444_12bit_flat_s3_80x20_d9':
        Fixture(80, 20, 12, "444", 3, 0, False, 9,
                "a9b1953e092dfd12392e2f8fd41d1828b17c74c5ceee1e75da0eb993e3d3fc6d"),
    'animated_mixed_lossless_key_superres_420_8bit_flat_s3_112x20_d9':
        Fixture(112, 20, 8, "420", 3, 0, False, 9,
                "98bf91fa58409a26e598576a31836894a73f6d9b8be69fa626d7159ebbe3f420"),
    'animated_mixed_lossless_key_superres_420_10bit_flat_s3_112x20_d9':
        Fixture(112, 20, 10, "420", 3, 0, False, 9,
                "90a7741bea0e628c6468353f3e5bc8d631093f9d1d58415d15042e4bb7ea0288"),
    'animated_mixed_lossless_key_superres_420_12bit_flat_s3_112x20_d9':
        Fixture(112, 20, 12, "420", 3, 0, False, 9,
                "d5429fb9530cf2442870c7bae029bb9fe1e769ede185de462fc8e4da2d927e0e"),
    'animated_mixed_lossless_key_superres_422_8bit_flat_s3_112x20_d9':
        Fixture(112, 20, 8, "422", 3, 0, False, 9,
                "3a70bf4dde2f0eb25fa2a111e23b0ea8441e00f096f3a7105cd15f693bccef59"),
    'animated_mixed_lossless_key_superres_422_10bit_flat_s3_112x20_d9':
        Fixture(112, 20, 10, "422", 3, 0, False, 9,
                "0972a3b9feee7ed33d3f8d6874ae37a5c1757488482fa35343c1e4169f298583"),
    'animated_mixed_lossless_key_superres_422_12bit_flat_s3_112x20_d9':
        Fixture(112, 20, 12, "422", 3, 0, False, 9,
                "4349a36a2736493ea7c5b3ebba1ff9df0802a0b404e57aab5adba6d938158b13"),
    'animated_mixed_lossless_key_superres_444_8bit_flat_s3_112x20_d9':
        Fixture(112, 20, 8, "444", 3, 0, False, 9,
                "43e38143ca61107bd4ccaea1f76031db5aeff9851fdfc2244784ff7fddc67a2e"),
    'animated_mixed_lossless_key_superres_444_10bit_flat_s3_112x20_d9':
        Fixture(112, 20, 10, "444", 3, 0, False, 9,
                "3127c7af4b18cd09fd12f4bbd3cc37e4c074583d7ecebd772160449e410ef8a1"),
    'animated_mixed_lossless_key_superres_444_12bit_flat_s3_112x20_d9':
        Fixture(112, 20, 12, "444", 3, 0, False, 9,
                "1748f119d4e120ecafdcc7472c2d96ebc717128f1e734e6d0364e75e33bdeb32"),
    'animated_mixed_lossless_key_superres_420_8bit_varied_s3_128x48_d10':
        Fixture(128, 48, 8, "420", 3, 0, True, 10,
                "ad361a025e522e8e602c9c443478ca4fe1b2d5f6e5467187f6d3ac7f9fe765de"),
    'animated_mixed_lossless_key_superres_420_10bit_varied_s3_128x48_d10':
        Fixture(128, 48, 10, "420", 3, 0, True, 10,
                "6095347a8400721baf7520a0d01054f7a58392b32491bc016e721cce5b8638ff"),
    'animated_mixed_lossless_key_superres_420_12bit_varied_s3_128x48_d10':
        Fixture(128, 48, 12, "420", 3, 0, True, 10,
                "2211ceb1a073ba40ddc60a24907d0fb0e7df6a3e5658daf6855655e155410653"),
    'animated_mixed_lossless_key_superres_422_8bit_varied_s3_128x48_d10':
        Fixture(128, 48, 8, "422", 3, 0, True, 10,
                "deee56a79c4bf12ce894e8fe17e51f84473921d71906b8bde9bbcbc8e7bf3e86"),
    'animated_mixed_lossless_key_superres_422_10bit_varied_s3_128x48_d10':
        Fixture(128, 48, 10, "422", 3, 0, True, 10,
                "00b06e8df4619721717b42c17ca24c93ec2603f31a94bc865dcebbde8836fce3"),
    'animated_mixed_lossless_key_superres_422_12bit_varied_s3_128x48_d10':
        Fixture(128, 48, 12, "422", 3, 0, True, 10,
                "fefd4d34f98314c42f6a66496237fd57fc881770645e21555c86d455ec9cc599"),
    'animated_mixed_lossless_key_superres_444_8bit_varied_s3_128x48_d10':
        Fixture(128, 48, 8, "444", 3, 0, True, 10,
                "456ab9d2b4267a0d7bcd610b274d165d7d2cbac79fd498925b69a9299048deff"),
    'animated_mixed_lossless_key_superres_444_10bit_varied_s3_128x48_d10':
        Fixture(128, 48, 10, "444", 3, 0, True, 10,
                "850f709597c066d74b3f16b33a0b8e472b3eba87f3e8bcb7f5d3dbf341c08d20"),
    'animated_mixed_lossless_key_superres_444_12bit_varied_s3_128x48_d10':
        Fixture(128, 48, 12, "444", 3, 0, True, 10,
                "c1420456268a9789a2e28b72d5be72a191bc8f7c7b76d45fc033813c6af0e7b1"),
    'animated_mixed_lossless_key_superres_420_8bit_varied_s3_80x20_d9':
        Fixture(80, 20, 8, "420", 3, 0, True, 9,
                "bbf89bad8078c1a876e72d39bc259142b4dd2dbb146294fa08a0a87020e6ed47"),
    'animated_mixed_lossless_key_superres_420_10bit_varied_s3_80x20_d9':
        Fixture(80, 20, 10, "420", 3, 0, True, 9,
                "0b310f31e01e2357d99398da2aaf83e0134c95ef905b71be5bf5c064081d3e3f"),
    'animated_mixed_lossless_key_superres_420_12bit_varied_s3_80x20_d9':
        Fixture(80, 20, 12, "420", 3, 0, True, 9,
                "a840e6988c16908aaabdce5aae899888059f5f0d566b64dc3553d7e114de176b"),
    'animated_mixed_lossless_key_superres_422_8bit_varied_s3_80x20_d9':
        Fixture(80, 20, 8, "422", 3, 0, True, 9,
                "2c7c7b8d7bbf869af284863684fbdfa4e53a5b87a01537c1bc92c7ffc8be022b"),
    'animated_mixed_lossless_key_superres_422_10bit_varied_s3_80x20_d9':
        Fixture(80, 20, 10, "422", 3, 0, True, 9,
                "a7367c64f5b5e156cc096651b45cce9e7b58f84f92f47131030ea5b0425b39cc"),
    'animated_mixed_lossless_key_superres_422_12bit_varied_s3_80x20_d9':
        Fixture(80, 20, 12, "422", 3, 0, True, 9,
                "20f838bc3fcd8c892db04a3a2cf36a8b706dc9ff32039768847342ec388f0848"),
    'animated_mixed_lossless_key_superres_444_8bit_varied_s3_80x20_d9':
        Fixture(80, 20, 8, "444", 3, 0, True, 9,
                "85a572ef7210bb5ca9027ced056f404faa9cbf0c5ff1f54f72825da7fff77e8d"),
    'animated_mixed_lossless_key_superres_444_10bit_varied_s3_80x20_d9':
        Fixture(80, 20, 10, "444", 3, 0, True, 9,
                "f6555df6058babfcbdb6c1a4033d124e3c4da68a337c13676fa7f35852326f90"),
    'animated_mixed_lossless_key_superres_444_12bit_varied_s3_80x20_d9':
        Fixture(80, 20, 12, "444", 3, 0, True, 9,
                "01b697a48e0319971ad41e08d553f34d78a8cc6d7a497dd1c9d6f4184f235046"),
    'animated_mixed_lossless_key_superres_420_8bit_varied_s3_112x20_d9':
        Fixture(112, 20, 8, "420", 3, 0, True, 9,
                "c8be9012fa16909e09a4035a5f73dd89bb2d599a39c4556349847eada2480682"),
    'animated_mixed_lossless_key_superres_420_10bit_varied_s3_112x20_d9':
        Fixture(112, 20, 10, "420", 3, 0, True, 9,
                "129483fa447ab409ce7b77bc4ea05891865f9174af155ec0cb5d1272dfcd4f2f"),
    'animated_mixed_lossless_key_superres_420_12bit_varied_s3_112x20_d9':
        Fixture(112, 20, 12, "420", 3, 0, True, 9,
                "ace67b030953cddbecb74fc6523b83f127b4809caeb65b3931fc5f81961f4262"),
    'animated_mixed_lossless_key_superres_422_8bit_varied_s3_112x20_d9':
        Fixture(112, 20, 8, "422", 3, 0, True, 9,
                "4c6ce71de3fcfef41223f85852f029fb4b32e66dc531d4e5d575f34b3f2cbe38"),
    'animated_mixed_lossless_key_superres_422_10bit_varied_s3_112x20_d9':
        Fixture(112, 20, 10, "422", 3, 0, True, 9,
                "1dc1b288512fd820d166e210eee91a3ba94ed094ac7257f9851ea17ec62f010a"),
    'animated_mixed_lossless_key_superres_422_12bit_varied_s3_112x20_d9':
        Fixture(112, 20, 12, "422", 3, 0, True, 9,
                "f454b8a3207cdf8c6d5b563cfdb50c9194116a49e5103e89f72c0725159d1e36"),
    'animated_mixed_lossless_key_superres_444_8bit_varied_s3_112x20_d9':
        Fixture(112, 20, 8, "444", 3, 0, True, 9,
                "b27f7bd930c6ef7a05996e242894a37ab3617f8503bef211bd50fab7061e3e1c"),
    'animated_mixed_lossless_key_superres_444_10bit_varied_s3_112x20_d9':
        Fixture(112, 20, 10, "444", 3, 0, True, 9,
                "37400c10ae079236ccb23c144ebd456835ebcaade65f28fbfccb7c6e708541c3"),
    'animated_mixed_lossless_key_superres_444_12bit_varied_s3_112x20_d9':
        Fixture(112, 20, 12, "444", 3, 0, True, 9,
                "173b397fc7cf11f9dc6ae65fffa940c7a879d5deb8f2df87335bf0e2ec9bf49c"),
}


def validate(path: Path, name: str) -> None:
    """Check file identity and independently parsed mixed-segment frame syntax."""
    spec = FIXTURES[name]
    if hashlib.sha256(path.read_bytes()).hexdigest() != spec.input_sha256:
        raise RuntimeError(f"{name}: complete input differs from pinned recipe")
    report = inspect(path)
    headers = [obu["sequence_header"] for sample in report["samples"]
               for obu in sample["obus"] if "sequence_header" in obu]
    expected = {"bit_depth": spec.depth, "max_width": spec.width, "max_height": spec.height,
                "monochrome": False,
                "subsampling_x": int(spec.layout != "444"),
                "subsampling_y": int(spec.layout == "420"), "enable_superres": True,
                "enable_restoration": False}
    if not headers or any(any(header.get(key) != value for key, value in expected.items())
                          for header in headers):
        raise RuntimeError(f"{name}: sequence features differ from the planar recipe")
    frames = [obu["frame_header"] for sample in report["samples"]
              if sample.get("role") == "track_pict" for obu in sample["obus"]
              if "frame_header" in obu]
    if len(frames) != 2:
        raise RuntimeError(f"{name}: expected one key and one inter frame")
    coded_width = max(min(spec.width, 16),
                      (spec.width * 8 + spec.denominator // 2) // spec.denominator)
    for index, frame in enumerate(frames):
        segmentation = frame.get("segmentation", {})
        base = frame.get("quantization", {}).get("base")
        qindices = {max(0, min(255, base + segment["delta_q"]))
                    for segment in segmentation.get("segments", [])} if base is not None else set()
        if (frame.get("frame_type") != ("key" if index == 0 else "inter")
                or frame.get("frame_width") != (spec.width if index == 0 else coded_width)
                or frame.get("upscaled_width") != spec.width
                or frame.get("frame_height") != spec.height
                or frame.get("superres_enabled") != bool(index)
                or frame.get("all_lossless") is not False
                or segmentation.get("enabled") is not True
                or segmentation.get("update_map") is not True
                or segmentation.get("temporal") is not False
                or 0 not in qindices or not any(qindex > 0 for qindex in qindices)
                or frame.get("restoration", {}).get("types") != [0, 0, 0]
                or frame.get("loop_filter", {}).get("level_y") != [0, 0]):
            raise RuntimeError(f"{name}: frame lacks the neutral mixed-segment features")
        if spec.integer_key and (frame.get("force_integer_mv") is not True
                                 or frame.get("allow_screen_content_tools") is not True):
            raise RuntimeError(f"{name}: frame lacks screen-content integer motion precision")


def generate(encoder: Path, destination: Path, name: str) -> None:
    spec = FIXTURES[name]
    with tempfile.TemporaryDirectory(prefix="avif-color-mixed-key-") as temporary:
        outputs = []
        for attempt in range(2):
            path = Path(temporary) / f"{attempt}.avif"
            command = [str(encoder), str(path), str(spec.width), str(spec.height), str(spec.depth), spec.layout,
                       "45", str(spec.speed), str(spec.denominator), str(spec.restoration)]
            if spec.integer_key:
                command.append("integer-key")
            elif spec.motion_key:
                command.append("motion-key")
            elif spec.varied_key:
                command.append("mixed-key")
            result = subprocess.run(command, capture_output=True, text=True, check=False)
            if result.returncode != 0:
                raise RuntimeError(f"native producer failed ({result.returncode}): {result.stderr}")
            path.write_bytes(normalize_sequence_timestamps(path.read_bytes()))
            validate(path, name)
            outputs.append(path.read_bytes())
        if outputs[0] != outputs[1]:
            raise RuntimeError(f"{name}: independently normalized encodes differ")
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(outputs[0])


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--encoder", type=Path)
    parser.add_argument("--output-dir", type=Path, default=OUTPUT)
    parser.add_argument("--variant", choices=tuple(FIXTURES))
    args = parser.parse_args()
    require_pinned_oracle()
    encoder = None
    if not args.check:
        if args.encoder is None:
            parser.error("--encoder is required to generate native inputs")
        encoder = args.encoder.expanduser().resolve(strict=True)
        version = subprocess.run([str(encoder), "--version"], capture_output=True, text=True, check=True)
        if version.stdout.strip() != "libavif 1.4.1 (aom [enc/dec]:v3.13.2)":
            raise RuntimeError(f"pinned producer required: {version.stdout!r}")
    names = [args.variant] if args.variant else FIXTURES
    for name in names:
        path = args.output_dir / (name + ".avif")
        if args.check:
            validate(path, name)
        else:
            assert encoder is not None
            generate(encoder, path, name)
    print(f"color mixed-key inputs: {len(names)} "
          f"{'checked' if args.check else 'reproduced twice'}")


if __name__ == "__main__":
    main()
