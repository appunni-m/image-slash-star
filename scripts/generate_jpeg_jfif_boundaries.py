#!/usr/bin/env python3
"""Generate pinned JPEG JFIF length and marker-position parity inputs."""
from __future__ import annotations

import argparse
import hashlib
from io import BytesIO
from pathlib import Path

from PIL import Image, UnidentifiedImageError, __version__ as pillow_version

ROOT = Path(__file__).resolve().parents[1]
SOURCES = {
    "baseline": "f1aa65d8d851900cfb0ab25075b0abf5926331bed853161f854b579c06ae1063",
    "progressive": "5ec2d633dc0d5675072a145d60604396ebcedbf277a5b8682a778b32b07b4ef1",
}
EXPECTED = {
    'jpeg_rgb_jfif_length_0_baseline': (
        '393b66aa050424e55086d250764dbd0d570889021ccaa1ef35877578ac8c5364',
        '067d6b1858fc9f54620a2732f9c82fccb9be42448afc7c4b69876e2404c34052',
    ),
    'jpeg_rgb_jfif_length_4_baseline': (
        'd5f94ca8503574115a90d663d9ae9719f0b9249392027e9c41a738cc615407ac',
        None,
    ),
    'jpeg_rgb_jfif_length_5_baseline': (
        'b39aa1049b29dbb13da0752602440a139910bc588cd01ee462171957651703e9',
        None,
    ),
    'jpeg_rgb_jfif_length_6_baseline': (
        'a586bffcbabda8cc7cc46be809668c6e5eddb02e91978470353ac7e9e5a1f722',
        None,
    ),
    'jpeg_rgb_jfif_length_7_baseline': (
        '6733f551d282afd21be8f67c90b7095e91ee6c05a5b4b1df1014590b6fd5774e',
        '067d6b1858fc9f54620a2732f9c82fccb9be42448afc7c4b69876e2404c34052',
    ),
    'jpeg_rgb_jfif_length_8_baseline': (
        '7006abbbec6d7a90f1ffb72aca5ca63357b447dff212ac0a44835d3c6944b887',
        '067d6b1858fc9f54620a2732f9c82fccb9be42448afc7c4b69876e2404c34052',
    ),
    'jpeg_rgb_jfif_length_11_baseline': (
        '8a7e6373d303817f24a16347df7c04d159fab639cfb8271eec73750c2decaf6e',
        '067d6b1858fc9f54620a2732f9c82fccb9be42448afc7c4b69876e2404c34052',
    ),
    'jpeg_rgb_jfif_length_12_baseline': (
        '29a0a4e0e12f69ab18c5af6dd645c9f68f2c58387bacd67ecb7c5a4a0bdf8e92',
        '067d6b1858fc9f54620a2732f9c82fccb9be42448afc7c4b69876e2404c34052',
    ),
    'jpeg_rgb_jfif_length_13_baseline': (
        'ec020a4dcba6eec132caefcc33342ea741497aa8f5b7c358fa5fe5a3f7231793',
        '067d6b1858fc9f54620a2732f9c82fccb9be42448afc7c4b69876e2404c34052',
    ),
    'jpeg_rgb_jfif_length_14_baseline': (
        'b443949ad2070cc6f3e4db932946e5cdb49580b91949c00f9defe5adb0065bf5',
        '2c7ae1c3f689d1349aa0d61d99591de0fd3edd18749b594282bd684237049538',
    ),
    'jpeg_rgb_jfif_missing_nul_baseline': (
        'cd834078c9c6251468bacad5f1c3d89d9efc37681c02c481142e8bbb68a2cc9b',
        '067d6b1858fc9f54620a2732f9c82fccb9be42448afc7c4b69876e2404c34052',
    ),
    'jpeg_rgb_jfif_unrecognized_magic_baseline': (
        '446f36cc50ab0bcc408bea8580b7382975e71d50af3f191001ebaf20d8f14265',
        '067d6b1858fc9f54620a2732f9c82fccb9be42448afc7c4b69876e2404c34052',
    ),
    'jpeg_rgb_jfif_length_0_progressive': (
        '129fa42952f2019cb8e60b67e9052cda5359576773b9ed601a90df88f28d3635',
        '067d6b1858fc9f54620a2732f9c82fccb9be42448afc7c4b69876e2404c34052',
    ),
    'jpeg_rgb_jfif_length_4_progressive': (
        'fbf957b98d3b92ccddffebd9e05cf0469ab51d57e56cf0b2ae8b6d5f608a513d',
        None,
    ),
    'jpeg_rgb_jfif_length_5_progressive': (
        'f56bf6447f2eb6916d13479ec69ed0ea1749300eb058d60d2727904e91310063',
        None,
    ),
    'jpeg_rgb_jfif_length_6_progressive': (
        '8b7f3aa13f15f8fe811dcb812fe0af79548aa58f52b2d13559dcebcdea18c61c',
        None,
    ),
    'jpeg_rgb_jfif_length_7_progressive': (
        'ed1176b43d24611794bc5daa8fb730658ed82f6af56401fe77684056962e9edf',
        '067d6b1858fc9f54620a2732f9c82fccb9be42448afc7c4b69876e2404c34052',
    ),
    'jpeg_rgb_jfif_length_8_progressive': (
        '10f2de17dafcf89abf05e3a886b4d15e1fb0777c204e004ced898c9a58403e05',
        '067d6b1858fc9f54620a2732f9c82fccb9be42448afc7c4b69876e2404c34052',
    ),
    'jpeg_rgb_jfif_length_11_progressive': (
        '2def42c4a2ca578224a1247f3254702a22f5af31016eed27955eeeaf1a697c31',
        '067d6b1858fc9f54620a2732f9c82fccb9be42448afc7c4b69876e2404c34052',
    ),
    'jpeg_rgb_jfif_length_12_progressive': (
        '4f732efe213fa306af7ab19eb2177367ae7c2adb79e85069d9d664da9d4cd67e',
        '067d6b1858fc9f54620a2732f9c82fccb9be42448afc7c4b69876e2404c34052',
    ),
    'jpeg_rgb_jfif_length_13_progressive': (
        '53417343075980424facd507018898713660cf34e1f9f5a7a521390d6e94946c',
        '067d6b1858fc9f54620a2732f9c82fccb9be42448afc7c4b69876e2404c34052',
    ),
    'jpeg_rgb_jfif_length_14_progressive': (
        '17e6c8ba34808b14b0b819deb3ae97dfc70d267c8ebd716d1a6b90b2675c4b02',
        '2c7ae1c3f689d1349aa0d61d99591de0fd3edd18749b594282bd684237049538',
    ),
    'jpeg_rgb_jfif_missing_nul_progressive': (
        '1192714252ba2c19569ca57abc54720790e0c93a64406de1f602e4b2db7c1781',
        '067d6b1858fc9f54620a2732f9c82fccb9be42448afc7c4b69876e2404c34052',
    ),
    'jpeg_rgb_jfif_unrecognized_magic_progressive': (
        '3f7bfbe675062e5973ba375b2d3a70d792780b0cd64b35aa5a667b1951b441fa',
        '067d6b1858fc9f54620a2732f9c82fccb9be42448afc7c4b69876e2404c34052',
    ),
    'jpeg_rgb_jfif_late_length_4_baseline': (
        '1fea4c6bb6d4c7c841ee480a5e3e5cca9d8a56a98f4a97dc76d093ca4ed704bc',
        '067d6b1858fc9f54620a2732f9c82fccb9be42448afc7c4b69876e2404c34052',
    ),
    'jpeg_rgb_jfif_late_length_5_baseline': (
        '84a1bc3a723287a99dbfac1ca672f9f8903fae67f7423055b1c4ba3d92e0c9cc',
        '067d6b1858fc9f54620a2732f9c82fccb9be42448afc7c4b69876e2404c34052',
    ),
    'jpeg_rgb_jfif_late_length_6_baseline': (
        'a55bf6dbc6763d19e5eb7f621eaa85497dce675d843639c8c9d9658881743bb0',
        '067d6b1858fc9f54620a2732f9c82fccb9be42448afc7c4b69876e2404c34052',
    ),
    'jpeg_rgb_jfif_late_length_4_progressive': (
        '7899cf31e66bcfd47749f771382c27bc0066175da019cdfc5911b83ea99c099d',
        '067d6b1858fc9f54620a2732f9c82fccb9be42448afc7c4b69876e2404c34052',
    ),
    'jpeg_rgb_jfif_late_length_5_progressive': (
        'ca11cd727ddd8b126116c69aba354853c5333c33a9777e38a350c28a5773a46e',
        '067d6b1858fc9f54620a2732f9c82fccb9be42448afc7c4b69876e2404c34052',
    ),
    'jpeg_rgb_jfif_late_length_6_progressive': (
        '88ac9556966cf7712cf7c271c66fbcd56922daeb76cf9019f4c26b4131009a1e',
        '067d6b1858fc9f54620a2732f9c82fccb9be42448afc7c4b69876e2404c34052',
    ),
}


def variants(data: bytes) -> dict[str, bytes]:
    payload = b"JFIF\0\x01\x01\0\0\x01\0\x01\0\0"
    payloads = {f"length_{n}": payload[:n] for n in (0, 4, 5, 6, 7, 8, 11, 12, 13, 14)}
    payloads["missing_nul"] = payload[:4] + b"X" + payload[5:]
    payloads["unrecognized_magic"] = b"X" + payload[1:]
    result = {}
    for name, app in payloads.items():
        marker = b"\xff\xe0" + (len(app) + 2).to_bytes(2, "big") + app
        result[name] = data[:2] + marker + data[2:]
    for length in (4, 5, 6):
        app = payload[:length]
        marker = b"\xff\xe0" + (len(app) + 2).to_bytes(2, "big") + app
        result[f"late_length_{length}"] = data[:-2] + marker + data[-2:]
    return result


def verify_pillow(name: str, data: bytes, pixel_hash: str | None) -> None:
    try:
        with Image.open(BytesIO(data)) as image:
            image.verify()
        with Image.open(BytesIO(data)) as image:
            image.load()
            if image.size != (17, 17) or image.mode != "RGB":
                raise RuntimeError(f"Pillow metadata changed: {name}")
            actual = hashlib.sha256(image.tobytes()).hexdigest()
    except UnidentifiedImageError:
        if pixel_hash is not None:
            raise RuntimeError(f"Pillow unexpectedly rejected {name}") from None
    else:
        if pixel_hash is None or actual != pixel_hash:
            raise RuntimeError(f"Pillow result changed: {name}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify without writing")
    args = parser.parse_args()
    if pillow_version != "12.2.0":
        raise RuntimeError(f"expected Pillow 12.2.0, found {pillow_version}")
    checked = set()
    for scan_mode, source_hash in SOURCES.items():
        source = ROOT / "tests/fixtures/input/images/jpeg" / f"jpeg_rgb_17x17_{scan_mode}.jpg"
        data = source.read_bytes()
        if hashlib.sha256(data).hexdigest() != source_hash or not data.endswith(b"\xff\xd9"):
            raise RuntimeError(f"JFIF fixture source changed: {source}")
        for variant, encoded in variants(data).items():
            name = f"jpeg_rgb_jfif_{variant}_{scan_mode}"
            input_hash, pixel_hash = EXPECTED[name]
            if hashlib.sha256(encoded).hexdigest() != input_hash:
                raise RuntimeError(f"JPEG input bytes changed: {name}")
            verify_pillow(name, encoded, pixel_hash)
            destination = ROOT / "tests/fixtures/input/images/jpeg" / (name + ".jpg")
            if args.check:
                if not destination.is_file() or destination.read_bytes() != encoded:
                    raise RuntimeError(f"fixture is stale or missing: {name}")
            else:
                if destination.exists() and destination.read_bytes() != encoded:
                    raise RuntimeError(f"refusing to replace a different fixture: {name}")
                destination.write_bytes(encoded)
            checked.add(name)
            print(f"{'Checked' if args.check else 'Generated'} {name}")
    if checked != set(EXPECTED):
        raise RuntimeError("JFIF fixture hash inventory differs from generated inputs")


if __name__ == "__main__":
    main()
