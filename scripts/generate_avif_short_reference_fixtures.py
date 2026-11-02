#!/usr/bin/env python3
"""Generate complete animated AVIF short-reference and order-hint inputs.

Pinned Pillow supplies deterministic lossless/lossy sources. Checked AV1 header
rewrites retain every tile byte and sample extent. Expected public outcomes
are generated independently by the normal live-Pillow reference workflow.
"""

from __future__ import annotations

import argparse
import hashlib
from io import BytesIO
from pathlib import Path
import tempfile

from PIL import Image
import yaml

from avif_bmff_boxes import normalize_creation_timestamps
from generate_avif_filmgrain_edge_fixtures import replace_field, require_pinned_oracle
from generate_avif_filmgrain_reference_reuse import (
    append_bits, bits_from_payload, leb128, pack_bits, padding_obu, source_frames,
)
from generate_decode_refs import verify_primary_oracle
from inspect_av1_obus import inspect

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "tests/fixtures/input/images/avif"
ADVANCED = {
    "min-partition-size": "64", "max-partition-size": "64", "use-intra-dct-only": "1",
    "enable-cdef": "0", "enable-restoration": "0", "aq-mode": "0", "deltaq-mode": "0",
    "enable-order-hint": "1", "enable-ref-frame-mvs": "0", "enable-dist-wtd-comp": "0",
}
# Input recipes: frame count, key hint, final hint, LAST slot, GOLDEN slot.
VARIANTS = {
    "past_aliased": (2, 0, 1, 0, 0),
    "past_distinct": (2, 0, 1, 7, 6),
    "tied_aliased": (2, 0, 0, 0, 0),
    "tied_distinct": (2, 0, 0, 7, 6),
    "half_range": (2, 0, 64, 7, 6),
    "wrapped_future": (2, 0, 65, 0, 1),
    "wrapped_near_future": (2, 0, 127, 7, 6),
    "near_past": (2, 0, 63, 0, 1),
    "past_distinct_images": (3, 0, 2, 1, 0),
    "tied_previous_image": (3, 0, 1, 1, 0),
    "reordered_past_images": (3, 2, 3, 1, 0),
    "reordered_future_images": (3, 2, 1, 1, 0),
}
SOURCE_SHA256 = {
    (2, 100): "6f9ed25fb57594c9f44c42309f3d32291bd004bc872905f9845ab58bed2d32cb",
    (2, 99): "3f813888bef53741d0e78f0f6cad807b05bd9923f94133790ec0a5a6626beb56",
    (3, 100): "561d15558a2593bbb85dc5fa18457cf27cd78078ced67c673ee375e57cd07eeb",
    (3, 99): "133586d6d64d00ddcedae5bb4b5e881dd602a7435976829ee57a43405c520875",
}
# Complete-input digests only; construction never consults expected pixels.
FIXTURE_SHA256 = {'short_refs_q100_half_range': 'eeaba276228d2e211db60b58c0c99207a8a27ac48df49e62419c41333adb09fc',
 'short_refs_q100_near_past': 'c1f16b78ef72c5c3bbb6df1e35b52f1550777e189613d239415fdb32b40329bb',
 'short_refs_q100_past_aliased': '83bd0b42082ba4d9f436a731175a4a2c44a4f6f7cab58e86b98a7f5b7a1fe66e',
 'short_refs_q100_past_distinct': '84ebe172290b57825023cea43054b00e853554eb5e0a2e1eda6c84391ffb9ee4',
 'short_refs_q100_past_distinct_images': '6d83e0858fa9dcf5b6177629daa9b8ed68799978c738dd340c2d9ba7d1e57da0',
 'short_refs_q100_reordered_future_images': 'fb4ca6823a5b981d75b93f8c751ce0ef874b99499fef4b01e9fa8f985ff2594f',
 'short_refs_q100_reordered_past_images': '32dd31a98e26951f62bb57e216ba972c27d56f1240b2192c7228935426e9fd55',
 'short_refs_q100_tied_aliased': '39637c228398cb9db43c0d926ea1d212addac2d343e065bae7c1b12ea25c3397',
 'short_refs_q100_tied_distinct': 'af9b26092a8d83d15fc247905d2a15043b9033f79c5d2ed63ff870e347af2121',
 'short_refs_q100_tied_previous_image': '45c095edeaecfd52fe80e1bc2f68c371f56a87861bb88433430669960b3eb785',
 'short_refs_q100_wrapped_future': 'ca119400574aa4585350f356a5c1b1a7ea2733e9de36a9e52dedd1682400e4fd',
 'short_refs_q100_wrapped_near_future': 'a8376fac449e9259ab61b8ffeff28a3c3e6283672195a3b4c78af3b6f2361edd',
 'short_refs_q99_half_range': '347a10523ea08ac8732941e47dc7d6137f0708727bc16c5d000c0e1dd00949f6',
 'short_refs_q99_near_past': '252a58c2318bf863eabf33bc99bdd2dea2e10d5ffc471cdbf89d388d379c245b',
 'short_refs_q99_past_aliased': 'e4a49eb1b4d394383aaf48a09bf8c2c5b7cd1e0c3d2f2a54d8ebe2cf0a005254',
 'short_refs_q99_past_distinct': 'c8641325a2d6222ce53e8e286e2cbfbe7dc70ade0a3a3b25415db12737d28b4e',
 'short_refs_q99_past_distinct_images': '5569badd39681c8f415f1ee3e1a9088001c70277736b55da6ef39540bb5ee771',
 'short_refs_q99_reordered_future_images': '2b972c54344d9e96c5a348f50d75d2499c77c639d69535070d9af734341c38ab',
 'short_refs_q99_reordered_past_images': '8e73a4a1d02c3c576ee00d24e06e2ee2041b6fa189b74481e95e2cd6eb684c6f',
 'short_refs_q99_tied_aliased': 'd37d2d898bc7e24ab97d0ad4743121fcb2beb3e4f04c3648a904982699424ddb',
 'short_refs_q99_tied_distinct': '2189bdfd0c18ad2c7c7eeb6635d85926b45684eac224c95d7983842cb769813d',
 'short_refs_q99_tied_previous_image': 'b8ea81096f216f31f03c3e2a75078bd54f956b148ca1d113a527a9340458b40f',
 'short_refs_q99_wrapped_future': '3d7ed8b94ad3663aa97567716834caf9bcf5b9131260da76ad2897e4e614d09c',
 'short_refs_q99_wrapped_near_future': '05917a4b2de698d9a117ce7c2a5e32116066c09f045d96313956aceb697c701f'}


def picture_frames(report: dict) -> list[tuple[dict, dict]]:
    """Return coded picture-track frame OBUs in sample order."""
    samples = sorted((sample for sample in report["samples"] if sample["role"] == "track_pict"),
                     key=lambda sample: sample["identity"]["sample"])
    return [(sample, obu) for sample in samples for obu in sample["obus"] if "frame_header" in obu]


def inspect_bytes(data: bytes) -> dict:
    """Inspect a complete source without modifying retained fixtures."""
    with tempfile.TemporaryDirectory(prefix="avif-short-reference-") as directory:
        path = Path(directory) / "input.avif"
        path.write_bytes(data)
        return inspect(path)


def encode_source(frame_count: int, quality: int) -> bytes:
    """Build varied frames with separate localized second/third-frame changes."""
    first, second = source_frames()
    third = bytearray(second.tobytes())
    for y in range(38, 54):
        for x in range(38, 54):
            start = (y * 64 + x) * 3
            third[start:start + 3] = bytes(255 - value for value in third[start:start + 3])
    frames = [first, second, Image.frombytes("RGB", (64, 64), bytes(third))][:frame_count]
    output = BytesIO()
    first.save(output, format="AVIF", save_all=True, append_images=frames[1:],
               duration=[100] * frame_count, loop=0, quality=quality, speed=0,
               max_threads=1, subsampling="4:4:4", autotiling=False, codec="aom", advanced=ADVANCED)
    return normalize_creation_timestamps(output.getvalue())


def rewrite_final_references(data: bytes, report: dict, order: int, last: int, golden: int) -> bytes:
    """Replace seven explicit indices with the six-bit short-reference form."""
    if not (0 <= order < 128 and 0 <= last < 8 and 0 <= golden < 8):
        raise ValueError("short-reference fields exceed the pinned syntax widths")
    sample, obu = picture_frames(report)[-1]
    header = obu["frame_header"]
    short_bit = header["frame_refs_short_signaling_bit"]
    if (header["frame_type"] != "inter" or header["frame_refs_short_signaling"]
            or short_bit is None
            or header["reference_indices_bits"] != list(range(short_bit + 1, short_bit + 22, 3))
            or len(sample["spans"]) != 1 or len(obu["payload_spans"]) != 1
            or len(obu["header_spans"]) != 1
            or obu["tile_group"]["start"] != 0 or obu["tile_group"]["end"] != 0):
        raise ValueError("source is not the pinned single-tile explicit-reference frame")
    span = obu["payload_spans"][0]
    payload = data[span["offset"]:span["offset"] + span["length"]]
    old_bits = bits_from_payload(payload, header["header_bits"])
    if old_bits[short_bit] != 0:
        raise ValueError("source short-reference flag differs from its inspected value")
    for index, expected in enumerate(header["reference_indices"]):
        start = short_bit + 1 + index * 3
        if int("".join(map(str, old_bits[start:start + 3])), 2) != expected:
            raise ValueError("source reference index differs from its inspected value")
    order_bits: list[int] = []
    append_bits(order_bits, order, 7)
    old_bits[header["order_hint_bit"]:header["order_hint_bit"] + 7] = order_bits
    bits = old_bits[:short_bit]
    append_bits(bits, 1, 1)
    append_bits(bits, last, 3)
    append_bits(bits, golden, 3)
    bits.extend(old_bits[short_bit + 22:])
    while len(bits) % 8:
        bits.append(0)
    tile_bit = obu["tile_group"]["data_bit"]
    if tile_bit % 8:
        raise ValueError("source tile payload is not byte aligned")
    new_payload = bytes(pack_bits(bits)) + payload[tile_bit // 8:]
    hs = obu["header_spans"][0]
    obu_header = data[hs["offset"]:hs["offset"] + 1 + int(obu["has_extension"])]
    ss = sample["spans"][0]
    start, end = ss["offset"], ss["offset"] + ss["length"]
    if hs["offset"] + hs["length"] + span["length"] != end:
        raise ValueError("source frame OBU is not last in its sample")
    new_sample = data[start:hs["offset"]] + obu_header + leb128(len(new_payload)) + new_payload
    difference = ss["length"] - len(new_sample)
    if difference == 1:
        # A padding OBU needs two bytes. A legal extended size LEB128 consumes
        # the remaining byte instead, retaining all following physical offsets.
        size = bytearray(leb128(len(new_payload)))
        size[-1] |= 0x80
        size.append(0)
        new_sample = data[start:hs["offset"]] + obu_header + size + new_payload
    elif difference >= 2:
        new_sample += padding_obu(difference)
    else:
        raise ValueError("rewritten header does not fit the retained sample")
    if len(new_sample) != ss["length"]:
        raise ValueError("short-reference rewrite changes the sample extent")
    return data[:start] + new_sample + data[end:]


def verify_tiles(before: bytes, after: bytes) -> None:
    """Reparse both complete inputs and prove every sample/tile extent stable."""
    old, new = picture_frames(inspect_bytes(before)), picture_frames(inspect_bytes(after))
    if len(before) != len(after) or len(old) != len(new):
        raise ValueError("rewrite changes the file extent or frame count")
    for (old_sample, a), (new_sample, b) in zip(old, new):
        if old_sample["spans"] != new_sample["spans"]:
            raise ValueError("rewrite changes a sample extent")
        sa, sb = a["payload_spans"][0], b["payload_spans"][0]
        old_tile = before[sa["offset"] + a["tile_group"]["data_bit"] // 8:sa["offset"] + sa["length"]]
        new_tile = after[sb["offset"] + b["tile_group"]["data_bit"] // 8:sb["offset"] + sb["length"]]
        if old_tile != new_tile:
            raise ValueError("rewrite changes encoded tile bytes")


def validate(path: Path, variant: str) -> None:
    """Check complete-input identity, independent syntax and live metadata."""
    data = path.read_bytes()
    if hashlib.sha256(data).hexdigest() != FIXTURE_SHA256[path.stem]:
        raise ValueError(f"fixture input digest differs: {path}")
    frame_count, key_hint, order, last, golden = VARIANTS[variant]
    frames = picture_frames(inspect(path))
    if len(frames) != frame_count:
        raise ValueError("fixture has an unexpected coded frame count")
    first, final = frames[0][1]["frame_header"], frames[-1][1]["frame_header"]
    if (first["order_hint"] != key_hint or not final["frame_refs_short_signaling"]
            or final["order_hint"] != order or final["last_frame_idx"] != last
            or final["gold_frame_idx"] != golden):
        raise ValueError("fixture short-reference syntax differs from its recipe")
    with Image.open(path) as image:
        if image.format != "AVIF" or image.size != (64, 64) or image.mode != "RGB" or image.n_frames != frame_count:
            raise ValueError("live Pillow fixture metadata differs from the source recipe")
        for index in range(frame_count):
            image.seek(index)
            image.load()
            if image.info.get("duration") != 100:
                raise ValueError("live Pillow frame duration differs from the source recipe")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, default=OUTPUT)
    parser.add_argument("--check", action="store_true", help="check retained inputs without encoding or rewriting")
    args = parser.parse_args()
    require_pinned_oracle()
    verify_primary_oracle(yaml.safe_load((ROOT / "manifest.yaml").read_text()))
    output = args.output_dir.resolve()
    sources = {}
    if not args.check:
        for frame_count, quality in SOURCE_SHA256:
            data, repeated = encode_source(frame_count, quality), encode_source(frame_count, quality)
            if data != repeated or hashlib.sha256(data).hexdigest() != SOURCE_SHA256[frame_count, quality]:
                raise ValueError("pinned short-reference source encode is not byte stable")
            sources[frame_count, quality] = data
        output.mkdir(parents=True, exist_ok=True)
    for quality in (100, 99):
        for variant, (frame_count, key_hint, order, last, golden) in VARIANTS.items():
            path = output / f"short_refs_q{quality}_{variant}.avif"
            if not args.check:
                source = sources[frame_count, quality]
                data = source
                if key_hint:
                    _, key = picture_frames(inspect_bytes(source))[0]
                    data = replace_field(data, key, key["frame_header"]["order_hint_bit"], 7, 0, key_hint)
                data = rewrite_final_references(data, inspect_bytes(data), order, last, golden)
                verify_tiles(source, data)
                if hashlib.sha256(data).hexdigest() != FIXTURE_SHA256[path.stem]:
                    raise ValueError("generated short-reference input digest differs")
                if path.exists() and path.read_bytes() != data:
                    raise ValueError(f"refusing to replace differing fixture: {path}")
                path.write_bytes(data)
            validate(path, variant)
    print(f"{'Checked' if args.check else 'Generated'} 24 complete short-reference AVIF inputs")


if __name__ == "__main__":
    main()
