#!/usr/bin/env python3
"""Add the existing essential primary-item irot property to an AVIF sequence.

The complete coded samples remain unchanged. Growing the metadata by ten
bytes rebases both the primary-item extent and the sequence chunk offset.
This generator writes input bytes only; the maintained oracle generates refs.
"""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import struct

from generate_avif_container_edge_fixtures import rewrite_boxes
from generate_avif_zero_width_clap_fixture import pack_box, read_boxes


ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "tests/fixtures/input/images/avif"
SOURCE_SHA256 = "2f8683d21725261f37f86e115f0c212cc52d0fefd3a2ddfcc4fa648c1859906d"
IROT_SOURCE_SHA256 = "df1fadfd3b7780e3c41825352ee5768731a6753dac613412dfa94d6715f93f7f"
OUTPUT = ASSETS / "animated_primary_item_irot.avif"


def candidate_bytes() -> bytes:
    source = (ASSETS / "animated.avif").read_bytes()
    rotation_source = (ASSETS / "primary_item_irot.avif").read_bytes()
    if hashlib.sha256(source).hexdigest() != SOURCE_SHA256:
        raise RuntimeError("animated source differs from its pin")
    if hashlib.sha256(rotation_source).hexdigest() != IROT_SOURCE_SHA256:
        raise RuntimeError("existing primary-item rotation source differs from its pin")
    rotation = pack_box(b"irot", b"\x03")
    if rotation_source.count(rotation) != 1:
        raise RuntimeError("existing rotation fixture must contain the exact irot property")
    delta = len(rotation) + 1
    changed = {b"ipco": 0, b"ipma": 0, b"iloc": 0, b"stco": 0}

    def transform(path: tuple[bytes, ...], payload: bytes) -> bytes:
        kind = path[-1]
        if path == (b"meta", b"iprp", b"ipco"):
            properties = read_boxes(payload, 0, len(payload))
            if [item[0] for item in properties] != [b"ispe", b"pixi", b"av1C", b"colr"]:
                raise RuntimeError("animated primary properties differ from the pinned layout")
            changed[kind] += 1
            return payload + rotation
        if path == (b"meta", b"iprp", b"ipma"):
            if payload != bytes.fromhex("000000000000000100010401028304"):
                raise RuntimeError("animated primary property associations differ")
            changed[kind] += 1
            return payload[:10] + b"\x05" + payload[11:] + b"\x85"
        if path == (b"meta", b"iloc"):
            if payload != bytes.fromhex("0000000044000001000100000001000003ff00000027"):
                raise RuntimeError("animated primary extent differs")
            changed[kind] += 1
            return payload[:14] + struct.pack(">I", 0x3FF + delta) + payload[18:]
        if path == (b"moov", b"trak", b"mdia", b"minf", b"stbl", b"stco"):
            if payload != bytes.fromhex("0000000000000001000003ff"):
                raise RuntimeError("animated sequence chunk offset differs")
            changed[kind] += 1
            return payload[:8] + struct.pack(">I", 0x3FF + delta)
        return payload

    result = rewrite_boxes(source, transform)
    if changed != dict.fromkeys(changed, 1) or len(result) != len(source) + delta:
        raise RuntimeError("rotation mutation changed unexpected boxes")
    media = lambda data: [complete for kind, complete, _ in read_boxes(data, 0, len(data)) if kind == b"mdat"]
    if media(result) != media(source):
        raise RuntimeError("rotation mutation changed encoded AV1 samples")
    return result


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    result = candidate_bytes()
    if args.check:
        if OUTPUT.read_bytes() != result:
            raise RuntimeError("animated irot input differs from the maintained recipe")
    else:
        OUTPUT.write_bytes(result)
    print(f"{OUTPUT.name}: {len(result)} bytes, sha256={hashlib.sha256(result).hexdigest()}")


if __name__ == "__main__":
    main()
