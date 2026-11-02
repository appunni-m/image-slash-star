#!/usr/bin/env python3
"""Generate complete JPEGs that distinguish integer and fractional sampling.

Zero DC and AC coefficients isolate sampling acceptance. SOF2 inputs contain
one valid interleaved DC-first scan. All MCU layouts use at most ten blocks.
Pillow independently defines the public decode result.
"""
from __future__ import annotations

import argparse
import hashlib
from io import BytesIO
from pathlib import Path

from PIL import Image, __version__ as pillow_version

ROOT = Path(__file__).resolve().parents[1]
CASES = (
    ("cmyk", "horizontal", False, 2,
     "56de74e9bd247923d69c5220253b534709eee62eba5c0c10dc23bea8182713a2"),
    ("cmyk", "horizontal", False, 1,
     "4f8ef7a7c01bb7d4de52c27a1b8e68efa732983265e4905f47306177bc4c3320"),
    ("cmyk", "horizontal", True, 2,
     "121ed316c5b6e8c23247844932eafd2d20ffd2cc08a2d1acdf9e3c6da4d48c1e"),
    ("cmyk", "horizontal", True, 1,
     "234ce5eb7602dc8fe7aaac2ac6e438d902ed03db12ef1c3ab7cf7a3296876a0c"),
    ("cmyk", "vertical", False, 2,
     "e027fc387feb01937551396a0dfd91ef91a993925017cb69f10fe0e77099435d"),
    ("cmyk", "vertical", False, 1,
     "398281345acc7046189fa8b772493d6a1a26f46533869958f87b47c528730086"),
    ("cmyk", "vertical", True, 2,
     "bfa61eea0db3879bb77bbd480ec6ab93043c743fdfb9672e3de0db82ffbc5b64"),
    ("cmyk", "vertical", True, 1,
     "48156ad9d00774e266d9c1b44e83d80f3f0275ede49663a22f37699ff29da2dc"),
    ("ycbcr", "horizontal", False, 2,
     "599c6f859e2a4ca991100cdb78dfba638df1bee3738bd98211772507ab0c05c8"),
    ("ycbcr", "horizontal", False, 1,
     "e8971cff1248d375f7f4f61852e121533465d5027e9381e6c93f43b3636627a1"),
    ("ycbcr", "horizontal", True, 2,
     "bbd42ad74796fa442018a08962b503ac50c010f1faa6179c69fd58940bb3c405"),
    ("ycbcr", "horizontal", True, 1,
     "9e24abe8be1826a187b087f5f9a3017255d3a7cb5d9cef666b4b63d3afbd7da2"),
    ("ycbcr", "vertical", False, 2,
     "e596e73cad9e3e8a79fa418eadb61c1f4eae8f3093ceefc8216e8ac9d4f25007"),
    ("ycbcr", "vertical", False, 1,
     "760420a5cda06dc1c308d7c3ace377b54c36eb7b67053992b9b67902396cb34a"),
    ("ycbcr", "vertical", True, 2,
     "c6d3c95acdee295c67d809971b6db1baf38392700e4060e028df12d1aa9d24a0"),
    ("ycbcr", "vertical", True, 1,
     "2f4e7b406b90d68db8590768a12022fd9ad783006d16e1ec9808ed42c213f403"),
)
PIXEL_SHA256 = {
    "cmyk": "22527f13f62eb6f2d8cdc5800222cebe9fec473fbab698764198b94d0ab81a5f",
    "ycbcr": "fa7b78cc215df21d7ce54d8c3c6637c326dab95c10fbc12263101365973f4268",
}


def marker(code: int, payload: bytes) -> bytes:
    return bytes((255, code)) + (len(payload) + 2).to_bytes(2, "big") + payload


def sampling_fixture(mode: str, axis: str, progressive: bool, reduced: int) -> bytes:
    count = 4 if mode == "cmyk" else 3
    factors = (3, reduced, reduced, 3)[:count]
    sampling = [(1, factor) if axis == "vertical" else (factor, 1) for factor in factors]
    adobe = marker(0xEE, b"Adobe\x00\x64\x00\x00\x00\x00\x00") if mode == "cmyk" else b""
    quantization = marker(0xDB, b"\x00" + b"\x01" * 64)
    components = b"".join(
        bytes((index + 1, (horizontal << 4) | vertical, 0))
        for index, (horizontal, vertical) in enumerate(sampling)
    )
    frame = marker(
        0xC2 if progressive else 0xC0,
        bytes((8, 0, 8, 0, 8, count)) + components,
    )
    huffman = marker(
        0xC4,
        bytes((0, 1)) + bytes(15) + bytes((0, 0x10, 1)) + bytes(15) + bytes((0,)),
    )
    selectors = b"".join(bytes((index + 1, 0)) for index in range(count))
    scan = marker(0xDA, bytes((count,)) + selectors + bytes((0, 0 if progressive else 63, 0)))
    # A zero coefficient block uses one DC-category bit, plus AC EOB in SOF0.
    bit_count = sum(horizontal * vertical for horizontal, vertical in sampling)
    bit_count *= 1 if progressive else 2
    entropy = bytearray((bit_count + 7) // 8)
    if bit_count % 8:
        entropy[-1] = (1 << ((-bit_count) % 8)) - 1
    return b"\xff\xd8" + adobe + quantization + frame + huffman + scan + entropy + b"\xff\xd9"


def verify_pillow(data: bytes, mode: str, reduced: int) -> None:
    with Image.open(BytesIO(data)) as image:
        image.verify()
    with Image.open(BytesIO(data)) as image:
        if image.size != (8, 8) or image.mode != ("CMYK" if mode == "cmyk" else "RGB"):
            raise RuntimeError("unexpected sampling fixture metadata")
        if reduced == 2:
            try:
                image.load()
            except OSError as error:
                if str(error) != "broken data stream when reading image file":
                    raise RuntimeError("fractional sampling error changed") from error
            else:
                raise RuntimeError("Pillow unexpectedly accepted fractional sampling")
        else:
            image.load()
            if hashlib.sha256(image.tobytes()).hexdigest() != PIXEL_SHA256[mode]:
                raise RuntimeError("integer sampling pixels changed")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify without writing")
    args = parser.parse_args()
    if pillow_version != "12.2.0":
        raise RuntimeError(f"expected Pillow 12.2.0, found {pillow_version}")
    for mode, axis, progressive, reduced, expected_sha256 in CASES:
        data = sampling_fixture(mode, axis, progressive, reduced)
        if hashlib.sha256(data).hexdigest() != expected_sha256:
            raise RuntimeError("sampling fixture bytes changed")
        verify_pillow(data, mode, reduced)
        ratio = "fractional" if reduced == 2 else "integer"
        scan = "progressive" if progressive else "baseline"
        name = f"{mode}_{ratio}_{axis}_3x{reduced}_{scan}_8x8.jpg"
        destination = ROOT / "tests/fixtures/input/images/jpeg" / name
        if args.check:
            if not destination.is_file() or destination.read_bytes() != data:
                raise RuntimeError(f"fixture is stale or missing: {name}")
        else:
            if destination.exists() and destination.read_bytes() != data:
                raise RuntimeError(f"refusing to replace a different fixture: {name}")
            destination.write_bytes(data)
        print(f"{'Checked' if args.check else 'Generated'} {name}")


if __name__ == "__main__":
    main()
