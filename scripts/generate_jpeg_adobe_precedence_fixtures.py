#!/usr/bin/env python3
"""Generate pinned JPEG Adobe-marker precedence parity inputs."""
from __future__ import annotations

import argparse
import hashlib
from io import BytesIO
from pathlib import Path

from PIL import Image, __version__ as pillow_version

ROOT = Path(__file__).resolve().parents[1]
EXPECTED = {
    'jpeg_rgb_adobe_duplicate_0_to_nonzero_baseline': (
        '06880899f04cff82caeb08a2fc737821009dc4cd32575d3d8827b8d9c4cbc438',
        '2c7ae1c3f689d1349aa0d61d99591de0fd3edd18749b594282bd684237049538',
    ),
    'jpeg_rgb_adobe_duplicate_nonzero_to_0_baseline': (
        'e1297737aa53f116b2096b2d92875337b8e2a6cd3e098192fbce4cf521380684',
        '067d6b1858fc9f54620a2732f9c82fccb9be42448afc7c4b69876e2404c34052',
    ),
    'jpeg_rgb_adobe_late_first_nonzero_baseline': (
        '0e1e706330c76793d8d46da58f2114994ca2cb148ddc70abf58132cbf6eaf788',
        '067d6b1858fc9f54620a2732f9c82fccb9be42448afc7c4b69876e2404c34052',
    ),
    'jpeg_rgb_adobe_late_conflict_nonzero_baseline': (
        '7c271bfe01a2921ab6376bb26e448735cc5c245bb878aa772db98d682f27fa4e',
        '067d6b1858fc9f54620a2732f9c82fccb9be42448afc7c4b69876e2404c34052',
    ),
    'jpeg_rgb_adobe_late_conflict_zero_baseline': (
        '5a63d523c1bfeee97caeafd3e3ff1c6326c4c0c3138cd604e070f244542a438a',
        '2c7ae1c3f689d1349aa0d61d99591de0fd3edd18749b594282bd684237049538',
    ),
    'jpeg_rgb_adobe_duplicate_same_zero_baseline': (
        '58324b52baf26012b4be1fcd5d3864195b7b0a40c128a16797b90601bfee7fa6',
        '067d6b1858fc9f54620a2732f9c82fccb9be42448afc7c4b69876e2404c34052',
    ),
    'jpeg_rgb_adobe_short_adobe_after_zero_baseline': (
        '5320b19f53ee2ef6eacf03a637673c3b871bee26b3107f457c9cd357511cc284',
        '067d6b1858fc9f54620a2732f9c82fccb9be42448afc7c4b69876e2404c34052',
    ),
    'jpeg_rgb_adobe_non_adobe_after_zero_baseline': (
        '65d11e48107a4c05a86e6f668744d023627ee8d24b156a48338e8a2843e0138d',
        '067d6b1858fc9f54620a2732f9c82fccb9be42448afc7c4b69876e2404c34052',
    ),
    'jpeg_rgb_adobe_duplicate_0_to_nonzero_progressive': (
        'ee745a77bdad311a51cc36237bc1818b1a810655a8bb3b08708760b473acc2e4',
        '2c7ae1c3f689d1349aa0d61d99591de0fd3edd18749b594282bd684237049538',
    ),
    'jpeg_rgb_adobe_duplicate_nonzero_to_0_progressive': (
        '03529f045a33d86da31a44b18ef29daa78bbc07bd2792a615db0a4c97f1fd5cb',
        '067d6b1858fc9f54620a2732f9c82fccb9be42448afc7c4b69876e2404c34052',
    ),
    'jpeg_rgb_adobe_late_first_nonzero_progressive': (
        'caac79e4823ae9bde14fd99293b4023c9ba41b61a0acc6be05a798c297ee1135',
        '067d6b1858fc9f54620a2732f9c82fccb9be42448afc7c4b69876e2404c34052',
    ),
    'jpeg_rgb_adobe_late_conflict_nonzero_progressive': (
        'a371393240228444f0c7303a3b48bea9b77f7dbdf103dfa637dddbaac54f5569',
        '067d6b1858fc9f54620a2732f9c82fccb9be42448afc7c4b69876e2404c34052',
    ),
    'jpeg_rgb_adobe_late_conflict_zero_progressive': (
        '8df0e0f15c51e9702b8d065a181a69a6676ac674c9f26b6ada684bad03776433',
        '2c7ae1c3f689d1349aa0d61d99591de0fd3edd18749b594282bd684237049538',
    ),
    'jpeg_rgb_adobe_duplicate_same_zero_progressive': (
        '0557f5d2dd5c2a2c35cd37127e280c24435ff81cbef32182e7f1e2d215b01c59',
        '067d6b1858fc9f54620a2732f9c82fccb9be42448afc7c4b69876e2404c34052',
    ),
    'jpeg_rgb_adobe_short_adobe_after_zero_progressive': (
        '49028847cfa6f2b988fe11e13ea2e7d2d89676eed45551970b76104b9523f86c',
        '067d6b1858fc9f54620a2732f9c82fccb9be42448afc7c4b69876e2404c34052',
    ),
    'jpeg_rgb_adobe_non_adobe_after_zero_progressive': (
        'aa3b3066c804b3bf8a946d433adfa62fd0dff38e2ea49e6952b97585545979d8',
        '067d6b1858fc9f54620a2732f9c82fccb9be42448afc7c4b69876e2404c34052',
    ),
    'jpeg_cmyk_adobe_duplicate_0_to_nonzero_baseline': (
        '33afd226bf1368df825ad77dce325070010a61d47b208a9ca2ec066e6c6a2fda',
        'dd79c54aff9a1386d55ed7e686fba1e0827ac73c1be1567063cfa6fcc286c04c',
    ),
    'jpeg_cmyk_adobe_duplicate_nonzero_to_0_baseline': (
        '11b3f8d696f50d4836307259c64e28c002b88cbed07888fb644de1124f8854b0',
        '0555d0ebbc6c525e3cb94a48523e6b2b56c5384b8657decd04e71431faabe36a',
    ),
    'jpeg_cmyk_adobe_late_first_nonzero_baseline': (
        '0ee0bcc4d9427cb35cf18641367b19e4a199d0d5f5f0f67ed2d70eb074bdf534',
        '0555d0ebbc6c525e3cb94a48523e6b2b56c5384b8657decd04e71431faabe36a',
    ),
    'jpeg_cmyk_adobe_late_conflict_nonzero_baseline': (
        '0df5d9660e2d168ef8243ffddf35861efbdc63036c0a7e504c49a94c75d133ec',
        '0555d0ebbc6c525e3cb94a48523e6b2b56c5384b8657decd04e71431faabe36a',
    ),
    'jpeg_cmyk_adobe_late_conflict_zero_baseline': (
        '06e1e6df0d040b431b942e96f27362f8fde55e0d08385d412913e78d1f3268f9',
        'dd79c54aff9a1386d55ed7e686fba1e0827ac73c1be1567063cfa6fcc286c04c',
    ),
    'jpeg_cmyk_adobe_duplicate_same_zero_baseline': (
        '8ff8809f8fb875c15486e398c515df3e8210b36742e29bf4f461de16c518586f',
        '0555d0ebbc6c525e3cb94a48523e6b2b56c5384b8657decd04e71431faabe36a',
    ),
    'jpeg_cmyk_adobe_short_adobe_after_zero_baseline': (
        '77a48f1ffe7061f3fceb90b6acdcb477f1c00624c5a343a2cca6b4e8645de60e',
        '0555d0ebbc6c525e3cb94a48523e6b2b56c5384b8657decd04e71431faabe36a',
    ),
    'jpeg_cmyk_adobe_non_adobe_after_zero_baseline': (
        '27f17ed8ce14694d44d186ba90a9212d490936c6419b028b68651f34ebbdacfd',
        '0555d0ebbc6c525e3cb94a48523e6b2b56c5384b8657decd04e71431faabe36a',
    ),
    'jpeg_cmyk_adobe_duplicate_0_to_nonzero_progressive': (
        '2ea8cd24e4a4a4736833ee718884e499375da7fac04e7446edd67c41a4d98864',
        'dd79c54aff9a1386d55ed7e686fba1e0827ac73c1be1567063cfa6fcc286c04c',
    ),
    'jpeg_cmyk_adobe_duplicate_nonzero_to_0_progressive': (
        '6cd8eb6e5b6aa0bf995e33df9d2c81c0b23a62e778a926edb04bbb42353879cd',
        '0555d0ebbc6c525e3cb94a48523e6b2b56c5384b8657decd04e71431faabe36a',
    ),
    'jpeg_cmyk_adobe_late_first_nonzero_progressive': (
        '9d548313e548d7d0190b824e1dfebee9c55c1eb9069d5745f0b433ffb4d86d28',
        '0555d0ebbc6c525e3cb94a48523e6b2b56c5384b8657decd04e71431faabe36a',
    ),
    'jpeg_cmyk_adobe_late_conflict_nonzero_progressive': (
        'f3b8b36a0ea63cdc7e20cc55c0d083cdf9b3fc09211da3b456769497666d7048',
        '0555d0ebbc6c525e3cb94a48523e6b2b56c5384b8657decd04e71431faabe36a',
    ),
    'jpeg_cmyk_adobe_late_conflict_zero_progressive': (
        '26fd5293699dd799866f279d44da13b7fc9f6d208819549ff5db6f6b9d6b8e32',
        'dd79c54aff9a1386d55ed7e686fba1e0827ac73c1be1567063cfa6fcc286c04c',
    ),
    'jpeg_cmyk_adobe_duplicate_same_zero_progressive': (
        '38ef6ba7285356ba801c050f2e28b1bdd6e4bbcc76a5216c7139c7f0c49af646',
        '0555d0ebbc6c525e3cb94a48523e6b2b56c5384b8657decd04e71431faabe36a',
    ),
    'jpeg_cmyk_adobe_short_adobe_after_zero_progressive': (
        '8f709523a04a1aaaf93dbaf20273630a9b91ba63acb45ec3cab9aa0b53b21e57',
        '0555d0ebbc6c525e3cb94a48523e6b2b56c5384b8657decd04e71431faabe36a',
    ),
    'jpeg_cmyk_adobe_non_adobe_after_zero_progressive': (
        '83127d134f2abcd1058503ecbe4922eb9ce0cb1f7db930fb0967fd470cd33bf5',
        '0555d0ebbc6c525e3cb94a48523e6b2b56c5384b8657decd04e71431faabe36a',
    ),
    'jpeg_rgb_adobe_unknown_transform_low_baseline': (
        'e24ff8546091e09f2987c82dc16695f2f9d999c85c8aa52060ad8ff7bf179270',
        '2c7ae1c3f689d1349aa0d61d99591de0fd3edd18749b594282bd684237049538',
    ),
    'jpeg_rgb_adobe_unknown_transform_max_baseline': (
        '4efb97aaa457f1e60ff058f2b5041976a54ef4a385c39b2eed3aa08126de3580',
        '2c7ae1c3f689d1349aa0d61d99591de0fd3edd18749b594282bd684237049538',
    ),
    'jpeg_rgb_adobe_unknown_transform_low_progressive': (
        '779a003c3e031a09985a06a11c56c0d516de557b9e76b3256347594ba9c5fba4',
        '2c7ae1c3f689d1349aa0d61d99591de0fd3edd18749b594282bd684237049538',
    ),
    'jpeg_rgb_adobe_unknown_transform_max_progressive': (
        '266cde51b5056b751b685fc97d35f05fe26fa95007a5b2aac1b62806f9463202',
        '2c7ae1c3f689d1349aa0d61d99591de0fd3edd18749b594282bd684237049538',
    ),
    'jpeg_cmyk_adobe_unknown_transform_low_baseline': (
        '78d17dc5d5027dd55fafa1fa475408c4c9e38f90a7a923c315f19f5a1e7d05ca',
        'dd79c54aff9a1386d55ed7e686fba1e0827ac73c1be1567063cfa6fcc286c04c',
    ),
    'jpeg_cmyk_adobe_unknown_transform_max_baseline': (
        'd37ae1c9c9da7736a2d09f6e241624ae7c1c9573f2df139e55f6eb489c1f69e8',
        'dd79c54aff9a1386d55ed7e686fba1e0827ac73c1be1567063cfa6fcc286c04c',
    ),
    'jpeg_cmyk_adobe_unknown_transform_low_progressive': (
        '511cdc949007d4216224fbd4a32154a56ee33e72321c75ead734f54a7b90fa5d',
        'dd79c54aff9a1386d55ed7e686fba1e0827ac73c1be1567063cfa6fcc286c04c',
    ),
    'jpeg_cmyk_adobe_unknown_transform_max_progressive': (
        'f2d1ed30695cad47377d307d733cd755f69124af5cf2cc00559b76cb6ccece9c',
        'dd79c54aff9a1386d55ed7e686fba1e0827ac73c1be1567063cfa6fcc286c04c',
    ),
}


def pixels(channels: int) -> bytes:
    state = 0x12345678
    result = bytearray(17 * 17 * channels)
    for index in range(len(result)):
        state ^= (state << 13) & 0xFFFFFFFF
        state ^= state >> 17
        state ^= (state << 5) & 0xFFFFFFFF
        result[index] = state & 255
    return bytes(result)


def marker_variants(data: bytes, transform: int) -> dict[str, bytes]:
    if (data[2:4] != b"\xff\xee" or data[6:11] != b"Adobe"
            or data[17] != 0 or data[-2:] != b"\xff\xd9"):
        raise RuntimeError("JPEG encoder no longer emits the expected Adobe/EOI layout")
    end = 4 + int.from_bytes(data[4:6], "big")
    adobe_zero = data[2:end]
    adobe_other = bytearray(adobe_zero)
    adobe_other[15] = transform
    adobe_other = bytes(adobe_other)
    short = bytearray(adobe_zero[:-1])
    short[2:4] = (13).to_bytes(2, "big")
    unrecognized = bytearray(adobe_other)
    unrecognized[4] = ord("X")
    variants = {
        "duplicate_0_to_nonzero": data[:2] + adobe_zero + adobe_other + data[end:],
        "duplicate_nonzero_to_0": data[:2] + adobe_other + adobe_zero + data[end:],
        "late_first_nonzero": data[:2] + data[end:-2] + adobe_other + data[-2:],
        "late_conflict_nonzero": data[:-2] + adobe_other + data[-2:],
        "late_conflict_zero": data[:2] + adobe_other + data[end:-2] + adobe_zero + data[-2:],
        "duplicate_same_zero": data[:2] + adobe_zero + adobe_zero + data[end:],
        "short_adobe_after_zero": data[:2] + adobe_zero + short + data[end:],
        "non_adobe_after_zero": data[:2] + adobe_zero + unrecognized + data[end:],
    }
    for name, value in (("unknown_transform_low", 2 if transform == 1 else 1),
                        ("unknown_transform_max", 255)):
        unknown = bytearray(adobe_zero)
        unknown[15] = value
        variants[name] = data[:2] + unknown + data[end:]
    return variants


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify without writing")
    args = parser.parse_args()
    if pillow_version != "12.2.0":
        raise RuntimeError(f"expected Pillow 12.2.0, found {pillow_version}")
    checked = set()
    for mode in ("RGB", "CMYK"):
        for progressive in (False, True):
            output = BytesIO()
            with Image.frombytes(mode, (17, 17), pixels(len(mode))) as image:
                image.save(output, format="JPEG", quality=85, subsampling=0,
                           keep_rgb=mode == "RGB", progressive=progressive, optimize=False)
            scan_mode = "progressive" if progressive else "baseline"
            for variant, data in marker_variants(output.getvalue(), 1 if mode == "RGB" else 2).items():
                name = f"jpeg_{mode.lower()}_adobe_{variant}_{scan_mode}"
                source_hash, pixel_hash = EXPECTED[name]
                if hashlib.sha256(data).hexdigest() != source_hash:
                    raise RuntimeError(f"JPEG input bytes changed: {name}")
                with Image.open(BytesIO(data)) as image:
                    image.verify()
                with Image.open(BytesIO(data)) as image:
                    image.load()
                    if image.size != (17, 17) or image.mode != mode:
                        raise RuntimeError(f"JPEG input metadata changed: {name}")
                    if hashlib.sha256(image.tobytes()).hexdigest() != pixel_hash:
                        raise RuntimeError(f"Pillow decoded pixels changed: {name}")
                destination = ROOT / "tests/fixtures/input/images/jpeg" / (name + ".jpg")
                if args.check:
                    if not destination.is_file() or destination.read_bytes() != data:
                        raise RuntimeError(f"fixture is stale or missing: {name}")
                else:
                    if destination.exists() and destination.read_bytes() != data:
                        raise RuntimeError(f"refusing to replace a different fixture: {name}")
                    destination.write_bytes(data)
                checked.add(name)
                print(f"{'Checked' if args.check else 'Generated'} {name}")
    if checked != set(EXPECTED):
        raise RuntimeError("Adobe fixture hash inventory differs from generated inputs")


if __name__ == "__main__":
    main()
