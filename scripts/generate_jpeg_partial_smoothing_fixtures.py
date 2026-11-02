#!/usr/bin/env python3
"""Generate complete Pillow parity inputs for progressive coefficient smoothing."""
from __future__ import annotations

import argparse
import hashlib
from collections.abc import Iterator
from io import BytesIO
from pathlib import Path

from PIL import Image, __version__ as pillow_version
from generate_jpeg_jcoef_boundary_fixtures import entropy, fixture, segment

ROOT = Path(__file__).resolve().parents[1]

# Independent fixed complete-input and pinned Pillow pixel SHA-256 values.
EXPECTED = {
    "progressive_smoothing_jcoef_17x9_cat1_al12_positive_first": (
        "1e77fb3349e3803a35c04970ebbc7cd83b7473224ea94bf7f5ab95cada691b25",
        "493ca2274c843df8389f1d670cb19e7f5f7df87f31cf80be4869d0287966d337",
    ),
    "progressive_smoothing_jcoef_17x9_cat1_al12_negative_first": (
        "48677f3fe5dc37f7006ef5c2bc52a52c1de75a66ccb5cf70b4ef91856d5ea814",
        "2b65e8733e1895999a5823e8929cc4efe2427b8c18be9ac9fa5168fa30244f73",
    ),
    "progressive_smoothing_jcoef_17x9_cat1_al13_positive_first": (
        "e50f77a4e3a6a97ce0cccf72f393c422c1ce811151476d1a33c3a50b4be1eed5",
        "43931be3b096c93e83e7bab227ead0f09be7d7adb5b8aa667e26e828cbec3877",
    ),
    "progressive_smoothing_jcoef_17x9_cat1_al13_negative_first": (
        "5f56dbc4f284883cd7eb90cc321dc1e191333fb0205b6aac7855624078c5633b",
        "3bdf990297dc73e902579f3640962a86ae7fe0d4fbb2ef3ce2747a8dd83ecbbd",
    ),
    "progressive_smoothing_jcoef_17x9_cat3_al12_positive_first": (
        "902214559671b6a64e336b7af2ffcaadcc393bb1ced0ce8e3d8bb0ce70eae8ed",
        "d14c8e074629066bc162d134625af820333ffb74f6d6c732111b9e4f66347a21",
    ),
    "progressive_smoothing_jcoef_17x9_cat3_al12_negative_first": (
        "19e68bd25546f9b5693a60b6fb45604cc9783a7d609c007cb8cdb5f40d3dae35",
        "21d941d8f054e15931890f681fd7dc2ee733a353fff3435d4a43b8670119aa51",
    ),
    "progressive_smoothing_jcoef_17x9_cat3_al13_positive_first": (
        "6defc5af367eb0df244fad096274b4526d9d1005a2775995bf9a2e9a0318e84e",
        "3bdf990297dc73e902579f3640962a86ae7fe0d4fbb2ef3ce2747a8dd83ecbbd",
    ),
    "progressive_smoothing_jcoef_17x9_cat3_al13_negative_first": (
        "bb940151f7a9c4f229e2ea746a44f338a100d47987bcdcc294ac44e1e358bb74",
        "43931be3b096c93e83e7bab227ead0f09be7d7adb5b8aa667e26e828cbec3877",
    ),
    "progressive_smoothing_jcoef_17x9_cat15_al12_positive_first": (
        "367f3a80aa4c45b49d77d0faa61af6352735b72b72127c6b4c036c9fd499a431",
        "2b65e8733e1895999a5823e8929cc4efe2427b8c18be9ac9fa5168fa30244f73",
    ),
    "progressive_smoothing_jcoef_17x9_cat15_al12_negative_first": (
        "cdc966c2b62d87f182b440732337b3f1aec29aadceae7a1d3ff8fa66f189b114",
        "493ca2274c843df8389f1d670cb19e7f5f7df87f31cf80be4869d0287966d337",
    ),
    "progressive_smoothing_jcoef_17x9_cat15_al13_positive_first": (
        "2247f0c0c2c134a92cc505ff5541f0cee0fa2df1ab248f9ad9b77defae59f3c8",
        "3bdf990297dc73e902579f3640962a86ae7fe0d4fbb2ef3ce2747a8dd83ecbbd",
    ),
    "progressive_smoothing_jcoef_17x9_cat15_al13_negative_first": (
        "e5e1d0ef26232961a5aa0b84b8afdae5ca072a0fc4901d8db77c0413a1e0cd0d",
        "43931be3b096c93e83e7bab227ead0f09be7d7adb5b8aa667e26e828cbec3877",
    ),
    "progressive_smoothing_clamped_zero_band_positive": (
        "da6a53ef19944cf97c6400920bdf0a304ae7baaf86f3ac3026ded37a98f01167",
        "fdd967eb557dfa603f6c0b6740ebecd7a2a24aaf42b52605ddfd25b932b21a84",
    ),
    "progressive_smoothing_preserve_received_positive": (
        "12958b0dfc1ebddb492cd57acc5685dd40669d10561c0c63fdfd12533469a40c",
        "d621cd1bcea4c7150ab610e31f47f8768a603170a436a3ac42ffa1ebb34e0403",
    ),
    "progressive_smoothing_accurate_zero_band_positive": (
        "905881fc1b23f1cceb0c68eeaa28502585a1916e6f567871331d7546f2e39283",
        "534fa49840ce524bf401089c90e554e05148a8001dca509952ee401a1c42c82f",
    ),
    "progressive_smoothing_dc_preserve_high_band_positive": (
        "f0396867cece9ee47ef2c73e9fbc5f49357ea55dceb8dfe4167584cd00f7e29f",
        "0c6805647f02a21f99d8ea0f8222cf2d42cec7465178571b4774493be2b6e849",
    ),
    "progressive_smoothing_zero_quantizer_positive": (
        "7a477407b1ce7ba121648f3847671d796ad6b09c295c6fdbe6439769474aa234",
        "df054f12f079b9494227efbfaa90de97a0e8a40c06459af35a2b5e1eea922ca2",
    ),
    "progressive_smoothing_clamped_zero_band_negative": (
        "0d4254bbaad651da61a4073d13009918af5b3d13672b98735769b566981405c8",
        "72522d6e1da4b12b3ff285f665717e8848d2b0cd3bc3c081c8af3e8e9a9a1210",
    ),
    "progressive_smoothing_preserve_received_negative": (
        "873cc9ccbb594905d6b81824cf4528d187d37937effec61976ad7cdcfb410f57",
        "f7c839f570fd58468a5beb14d20f94f13ebbcab390c0fbdb54b16fdfda9a42b8",
    ),
    "progressive_smoothing_accurate_zero_band_negative": (
        "f25a84159e40a795bb0771042a04dd8e5807edb6ffbd8c339743b642d7b97cfd",
        "b61bb9ab05ad7c461c1176a773a3e5f5906b51586c6cd4d1d5f00404ee1f82ef",
    ),
    "progressive_smoothing_dc_preserve_high_band_negative": (
        "c6685466475615b3bf8bf403280e5cf942a38c0ca81c0bab08a3d58bff24076a",
        "6023a9507c1f4ded9295d5e338dcef988e188a6d92f6bbd5b777bb8269d03f0c",
    ),
    "progressive_smoothing_zero_quantizer_negative": (
        "b2d7f6d72ae4ee45c996b99c3ef51aaf78efbc29e347963e63362c5bbcf0eb55",
        "fe1c5f8a20d3854f164721dd0f8df66c6cee06c5b1f3c536b94f17f5a0a6c557",
    ),
    "progressive_smoothing_rgb_422_scans_1": (
        "651adb31036338ab06fac3acb6179d0442696a08fa8a73a97e50120521e8f0bd",
        "55e0ba6a148a0053308e571604b36f54af27b684f24a62d9f5e2d3622a18fb23",
    ),
    "progressive_smoothing_rgb_422_scans_2": (
        "580c962ac18028acec513607abe7143b2b70e34c249d29e045047044fc711122",
        "506004f0a8735604a74da04b2206599f85d1506ce4c18726aa3f9511caf66fc0",
    ),
    "progressive_smoothing_rgb_422_scans_3": (
        "2b10f65e3ed532945c77f3afc483a7c381e16d70374bdff73c05d8b017495bb3",
        "54bac106d2abd2c77d7641190c2f81ffa1b499a4e10740630bc484c4af0710bd",
    ),
    "progressive_smoothing_rgb_422_scans_5": (
        "4a6808973fc5fc4f7706fbb2f6af557c240fe8c4517852d3e5c357b15869360c",
        "60dd40eea183ceaa1f8ab0d3adcdae3458a6c0de7ec34dcb7aa44277dc66caef",
    ),
    "progressive_smoothing_rgb_420_scans_1": (
        "e419c0ad389ead46934c961137e32c5cd01aca77be366563f28e65dd07bcde71",
        "cc64e95d1c19e6ee1b181758776b9a012720a4603ca38f880407a8a8ade34bc8",
    ),
    "progressive_smoothing_rgb_420_scans_2": (
        "4b7a81d8e39258d4dfb238f2548d69d4ae0892dd2766e5a9f9b8e57e1ae588e0",
        "68088417271a404fca853cbaa60713fd3475be5ea2a83ed957105954054ac5ab",
    ),
    "progressive_smoothing_rgb_420_scans_3": (
        "23102e2716cac8fba3eea8257e174571f82eb14eb78a7223aac6f57a0c3c0b06",
        "b6e5d93c701d915b1c76231d997900e8123164c7afc238672255e2283db52a38",
    ),
    "progressive_smoothing_rgb_420_scans_5": (
        "72138daac69c968f29251f870ffefbcf403afcbf0218f9ae7c3e9f3ef6fc3da2",
        "731e141e3e4f3ee4e7c098b7a74d2d1ed0a86614a4456814aa0923dfb12a5f21",
    ),
    "progressive_smoothing_missing_dc_scan": (
        "9c0809395468ffb6638ea696a51ff4b40186f0faa06a973e2b15803243865bc7",
        "6aa4f148c4538afa4e831403884b0fe13ee3bab9532483b73c3e5e57d1e459fd",
    ),
}


def signed_amplitude(value: int) -> tuple[int, str]:
    """Encode the signed JPEG amplitude without simulating reconstruction."""
    category = abs(value).bit_length()
    if not category:
        return 0, ""
    amplitude = value if value >= 0 else value + (1 << category) - 1
    return category, format(amplitude, f"0{category}b")


def band_fixture(
    negative: bool, band: tuple[int, int], al: int, present: bool, zero_quant: bool,
) -> bytes:
    """Write explicit scan bands over a six-block, nonuniform DC grid."""
    width, height = 17, 9
    quant = bytearray([1] * 64)
    if zero_quant:
        quant[3] = 0
    frame = (
        bytes([8]) + height.to_bytes(2, "big") + width.to_bytes(2, "big")
        + bytes([1, 1, 0x11, 0])
    )
    # Complete, non-oversubscribed fixed-length Huffman tables. AC includes
    # EOB and ZRL; no assigned code is the all-ones padding code.
    dc = bytes([0]) + bytes([0] * 4 + [16] + [0] * 11) + bytes(range(16))
    symbols = [0, 0xf0] + [
        (run << 4) | size for run in range(16) for size in range(1, 16)
    ]
    ac = bytes([0x10]) + bytes([0] * 7 + [len(symbols)] + [0] * 8) + bytes(symbols)
    indices = {symbol: index for index, symbol in enumerate(symbols)}
    dcs = [-400, -50, 250, 100, -200, 300]
    if negative:
        dcs = [-value for value in dcs]
    bits = ""
    previous = 0
    for value in dcs:
        category, amplitude = signed_amplitude(value - previous)
        previous = value
        bits += format(category, "05b") + amplitude
    dc_scan = segment(0xda, bytes([1, 1, 0, 0, 0, 0])) + entropy(bits)
    ss, se = band
    bits = ""
    for block in range(6):
        if present:
            value = (-3 if negative else 3) if block % 2 == 0 else (2 if negative else -2)
            category, amplitude = signed_amplitude(value)
            bits += format(indices[category], "08b") + amplitude
        if not present or ss < se:
            bits += format(indices[0], "08b")
    ac_scan = segment(0xda, bytes([1, 1, 0, ss, se, al])) + entropy(bits)
    return (
        b"\xff\xd8" + segment(0xdb, b"\x00" + quant) + segment(0xc2, frame)
        + segment(0xc4, dc + ac) + dc_scan + ac_scan + b"\xff\xd9"
    )


def scan_ranges(data: bytes) -> Iterator[tuple[int, int]]:
    """Yield complete scan ranges, recognizing stuffed bytes and restart markers."""
    offset = 2
    while offset < len(data):
        start = offset
        if data[offset] != 255:
            raise RuntimeError("expected JPEG marker")
        while data[offset] == 255:
            offset += 1
        marker = data[offset]
        offset += 1
        if marker == 0xd9:
            return
        length = int.from_bytes(data[offset:offset + 2], "big")
        offset += length
        if marker != 0xda:
            continue
        while offset < len(data):
            if data[offset] != 255:
                offset += 1
                continue
            end = offset + 1
            while data[end] == 255:
                end += 1
            if data[end] == 0 or 0xd0 <= data[end] <= 0xd7:
                offset = end + 1
                continue
            break
        yield start, offset


def fixture_inputs() -> dict[str, bytes]:
    cases = {}
    for category in (1, 3, 15):
        for al in (12, 13):
            for negative in (False, True):
                sign = "negative" if negative else "positive"
                name = f"progressive_smoothing_jcoef_17x9_cat{category}_al{al}_{sign}_first"
                cases[name] = fixture(17, 9, category, negative, al, False)
    for negative in (False, True):
        sign = "negative" if negative else "positive"
        for label, band, al, present, zero in (
            ("clamped_zero_band", (1, 5), 1, False, False),
            ("preserve_received", (1, 5), 2, True, False),
            ("accurate_zero_band", (1, 2), 0, False, False),
            ("dc_preserve_high_band", (10, 63), 0, True, False),
            ("zero_quantizer", (1, 5), 1, False, True),
        ):
            cases[f"progressive_smoothing_{label}_{sign}"] = band_fixture(
                negative, band, al, present, zero
            )
    width, height = 33, 17
    pixels = bytes((i * 37 + i // 7 * 23) % 256 for i in range(width * height * 3))
    for sampling, label in ((1, "422"), (2, "420")):
        stream = BytesIO()
        with Image.frombytes("RGB", (width, height), pixels) as image:
            image.save(stream, format="JPEG", quality=77, subsampling=sampling,
                       progressive=True, optimize=False)
        data = stream.getvalue()
        ends = [end for _, end in scan_ranges(data)]
        for scans in (1, 2, 3, 5):
            # End at a complete entropy segment and retain an ordinary EOI marker.
            cases[f"progressive_smoothing_rgb_{label}_scans_{scans}"] = (
                data[:ends[scans - 1]] + b"\xff\xd9"
            )
    encoded = band_fixture(False, (1, 5), 2, True, False)
    start, end = next(scan_ranges(encoded))
    cases["progressive_smoothing_missing_dc_scan"] = encoded[:start] + encoded[end:]
    return cases


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify without writing")
    args = parser.parse_args()
    if pillow_version != "12.2.0":
        raise RuntimeError(f"expected Pillow 12.2.0, found {pillow_version}")
    cases = fixture_inputs()
    if cases.keys() != EXPECTED.keys():
        raise RuntimeError("smoothing fixture inventory changed")
    for name, encoded in cases.items():
        input_hash, pixel_hash = EXPECTED[name]
        if hashlib.sha256(encoded).hexdigest() != input_hash:
            raise RuntimeError(f"JPEG input bytes changed: {name}")
        with Image.open(BytesIO(encoded)) as image:
            image.verify()
        with Image.open(BytesIO(encoded)) as image:
            image.load()
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
