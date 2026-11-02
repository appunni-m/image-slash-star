#!/usr/bin/env python3
"""Generate pinned RGB-color and narrow-chroma JPEG parity inputs."""
from __future__ import annotations

import argparse
import hashlib
from io import BytesIO
from pathlib import Path

from PIL import Image, __version__ as pillow_version

ROOT = Path(__file__).resolve().parents[1]
CASES = (
    ("jpeg_chroma_1x9_420_baseline", 1, 9, 2, False, False, "none",
     "9d39024f3c495ff4936e25c5e37b7c113003e5280150dac1651209fa0750d472",
     "17cbebbaf28bafcaf358fa86bdd51e43bfa91c19d3a1e7ffe2bc99204e9b5fb3"),
    ("jpeg_chroma_1x9_420_progressive", 1, 9, 2, False, True, "none",
     "ef4a41ffa3f41ba795f1f966a51f06ad14eb9bbc87ec02746c695e055e290249",
     "17cbebbaf28bafcaf358fa86bdd51e43bfa91c19d3a1e7ffe2bc99204e9b5fb3"),
    ("jpeg_chroma_3x9_422_baseline", 3, 9, 1, False, False, "none",
     "2c438b145cb4c1beb9c76a2cffabdb97978c89f3df9f29eaab0a6cceef47a56e",
     "1053442ed9ca659d7a2bb54def0acff524156ab1fdb052f632ead1612cc3ee4f"),
    ("jpeg_chroma_3x9_422_progressive", 3, 9, 1, False, True, "none",
     "c5bad847b349639c44b543214f683edd80b080003192670d216e4a6c98510381",
     "1053442ed9ca659d7a2bb54def0acff524156ab1fdb052f632ead1612cc3ee4f"),
    ("jpeg_chroma_3x9_420_baseline", 3, 9, 2, False, False, "none",
     "c599ce79344e0f2b00b9ef564749abd5a06d9afd553693acf8076e8e941df770",
     "dd21495d18aba0e70091d153ca7516d24b5103554bb9e14b9d439b6815ae233b"),
    ("jpeg_chroma_3x9_420_progressive", 3, 9, 2, False, True, "none",
     "57d0acf733aad5859b5f872450ed16d665ec3eceeef869d73ebfc471cbffbd0d",
     "dd21495d18aba0e70091d153ca7516d24b5103554bb9e14b9d439b6815ae233b"),
    ("jpeg_chroma_5x9_422_progressive", 5, 9, 1, False, True, "none",
     "7b7f7adbe4bd62a832ae0bad7fd540536a5ac7919183d880550f8b73e9d69c8e",
     "33e6096b0fdc15d1ed7caf2c7688aa961a48a19360ebd95b22b51b3a7b02ce06"),
    ("jpeg_chroma_5x9_420_progressive", 5, 9, 2, False, True, "none",
     "807758341df9e974d12127df3a822fa9481154e7152906758ce327aac3b5f01d",
     "51655bd959226e8fcb45e387e04f072341a7597e041ae04009553844fb32288e"),
    ("jpeg_rgb_1x1_baseline", 1, 1, 0, True, False, "none",
     "d8d81ca154697351d2291c388dd0cab2228fc485468d6b919a401b9bdc87f755",
     "c70406763cd0f149b4caf62c4e1ffdb8c0c3402ac386569e09e701eb2779b6a7"),
    ("jpeg_rgb_1x1_progressive", 1, 1, 0, True, True, "none",
     "fea97e1c9c1f3f896b6d0392b1ae2b1854259dea32806201933ec18ff7a81657",
     "c70406763cd0f149b4caf62c4e1ffdb8c0c3402ac386569e09e701eb2779b6a7"),
    ("jpeg_rgb_17x17_baseline", 17, 17, 0, True, False, "none",
     "f1aa65d8d851900cfb0ab25075b0abf5926331bed853161f854b579c06ae1063",
     "067d6b1858fc9f54620a2732f9c82fccb9be42448afc7c4b69876e2404c34052"),
    ("jpeg_rgb_17x17_progressive", 17, 17, 0, True, True, "none",
     "5ec2d633dc0d5675072a145d60604396ebcedbf277a5b8682a778b32b07b4ef1",
     "067d6b1858fc9f54620a2732f9c82fccb9be42448afc7c4b69876e2404c34052"),
    ("jpeg_rgb_128x128_baseline", 128, 128, 0, True, False, "none",
     "0ef7a033aadbaab956459529a98959d262248e2c14dbc77104ad43f5831110b4",
     "8d906840b8792c8137a7fdc1d685a097cb06d783148500c8ed569007c8e36c89"),
    ("jpeg_rgb_128x128_progressive", 128, 128, 0, True, True, "none",
     "32a460da269696389d389b49731ff4c10976a1703d7693a2a891ff5a55c02f8b",
     "8d906840b8792c8137a7fdc1d685a097cb06d783148500c8ed569007c8e36c89"),
    ("jpeg_rgb_no_adobe_baseline", 17, 17, 0, True, False, "no_adobe",
     "efd66c7a1f0e00a1be1326178bc998af28acceaecb8bb6b60ce2262fca6c4344",
     "067d6b1858fc9f54620a2732f9c82fccb9be42448afc7c4b69876e2404c34052"),
    ("jpeg_rgb_no_adobe_progressive", 17, 17, 0, True, True, "no_adobe",
     "07544cba494b0dc0cffbe89659559f2de0ea152b1b940ef54bdddf95cf62a44d",
     "067d6b1858fc9f54620a2732f9c82fccb9be42448afc7c4b69876e2404c34052"),
    ("jpeg_rgb_jfif_override_baseline", 17, 17, 0, True, False, "jfif_override",
     "b443949ad2070cc6f3e4db932946e5cdb49580b91949c00f9defe5adb0065bf5",
     "2c7ae1c3f689d1349aa0d61d99591de0fd3edd18749b594282bd684237049538"),
    ("jpeg_rgb_jfif_override_progressive", 17, 17, 0, True, True, "jfif_override",
     "17e6c8ba34808b14b0b819deb3ae97dfc70d267c8ebd716d1a6b90b2675c4b02",
     "2c7ae1c3f689d1349aa0d61d99591de0fd3edd18749b594282bd684237049538"),
    ("jpeg_rgb_adobe_ycbcr_baseline", 17, 17, 0, True, False, "adobe_ycbcr",
     "e4a60a16347632ce84c49570d137ce140384b6c25da6fd8381faf80c23c80627",
     "2c7ae1c3f689d1349aa0d61d99591de0fd3edd18749b594282bd684237049538"),
    ("jpeg_rgb_adobe_ycbcr_progressive", 17, 17, 0, True, True, "adobe_ycbcr",
     "d7ad32cd547b3a385c6a9aab7112293475094d0a170bceb8faf4cb249b904aa2",
     "2c7ae1c3f689d1349aa0d61d99591de0fd3edd18749b594282bd684237049538"),
    ("jpeg_rgb_late_adobe_ycbcr_baseline", 17, 17, 0, True, False, "late_adobe_ycbcr",
     "0e1e706330c76793d8d46da58f2114994ca2cb148ddc70abf58132cbf6eaf788",
     "067d6b1858fc9f54620a2732f9c82fccb9be42448afc7c4b69876e2404c34052"),
    ("jpeg_rgb_late_adobe_ycbcr_progressive", 17, 17, 0, True, True, "late_adobe_ycbcr",
     "caac79e4823ae9bde14fd99293b4023c9ba41b61a0acc6be05a798c297ee1135",
     "067d6b1858fc9f54620a2732f9c82fccb9be42448afc7c4b69876e2404c34052"),
)


def pixels(width: int, height: int) -> bytes:
    state = 0x12345678
    result = bytearray(width * height * 3)
    for index in range(len(result)):
        state ^= (state << 13) & 0xFFFFFFFF
        state ^= state >> 17
        state ^= (state << 5) & 0xFFFFFFFF
        result[index] = state & 255
    return bytes(result)


def marker_variant(data: bytes, variant: str) -> bytes:
    if variant == "none":
        return data
    if data[2:4] != b"\xff\xee" or data[6:11] != b"Adobe" or data[17] != 0:
        raise RuntimeError("RGB encoder no longer emits the expected first Adobe marker")
    end = 4 + int.from_bytes(data[4:6], "big")
    adobe_ycbcr = bytearray(data[2:end])
    adobe_ycbcr[15] = 1
    if variant == "no_adobe":
        return data[:2] + data[end:]
    if variant == "jfif_override":
        payload = b"JFIF\x00\x01\x01\x00\x00\x01\x00\x01\x00\x00"
        jfif = b"\xff\xe0" + (len(payload) + 2).to_bytes(2, "big") + payload
        return data[:2] + jfif + data[2:]
    if variant == "adobe_ycbcr":
        return data[:2] + adobe_ycbcr + data[end:]
    if variant == "late_adobe_ycbcr":
        return data[:2] + data[end:-2] + adobe_ycbcr + data[-2:]
    raise RuntimeError(f"unknown JPEG marker variant: {variant}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify without writing")
    args = parser.parse_args()
    if pillow_version != "12.2.0":
        raise RuntimeError(f"expected Pillow 12.2.0, found {pillow_version}")
    for name, width, height, sampling, rgb, progressive, variant, source_hash, pixel_hash in CASES:
        output = BytesIO()
        with Image.frombytes("RGB", (width, height), pixels(width, height)) as image:
            image.save(output, format="JPEG", quality=85, subsampling=sampling,
                       keep_rgb=rgb, progressive=progressive, optimize=False)
        data = marker_variant(output.getvalue(), variant)
        if hashlib.sha256(data).hexdigest() != source_hash:
            raise RuntimeError(f"JPEG input bytes changed: {name}")
        with Image.open(BytesIO(data)) as image:
            image.verify()
        with Image.open(BytesIO(data)) as image:
            image.load()
            if image.size != (width, height) or image.mode != "RGB":
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
        print(f"{'Checked' if args.check else 'Generated'} {name}")


if __name__ == "__main__":
    main()
