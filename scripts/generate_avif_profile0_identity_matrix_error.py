#!/usr/bin/env python3
"""Create a complete AVIF that declares an identity matrix for profile 0."""

from __future__ import annotations

import argparse
import hashlib
import io
from pathlib import Path
import tempfile

import PIL
from PIL import Image, _avif, features

from avif_bmff_boxes import normalize_creation_timestamps
from inspect_av1_obus import Bits, read_uleb128
from inspect_avif_bitstreams import inspect as inspect_avif


ROOT = Path(__file__).resolve().parent.parent
SOURCE_PATH = (
    ROOT / "tests/fixtures/input/images/avif/animated_transfer_bt709.avif"
)
SOURCE_SHA256 = "8b7c981b052241b55112e6ae3ce64b00681c101a25c7d9bd9c9091d727417af2"
OUTPUT_PATH = (
    ROOT
    / "tests/fixtures/input/images/avif/error_animated_identity_matrix_profile0_8bit_420.avif"
)
EXPECTED_SHA256 = "a88b8b552040ea93afb486c17efca9c7cdb0e7151854e1a0e350375489cd5526"
EXPECTED_CODECS = "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2"
SOURCE_SEQUENCE_CICP = (1, 1, 6)
SOURCE_NCLX_CICP = (1, 13, 6)
IDENTITY_CICP = (1, 13, 0)
EXPECTED_NCLX_COUNT = 2
EXPECTED_PILLOW_ERROR = "Failed to decode frame 0: Decoding of color planes failed"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def sequence_headers(
    data: bytes,
    path: Path,
) -> list[tuple[int, int, bytes]]:
    """Return sequence-header payloads and offsets in complete AVIF samples."""
    tracks = [
        track for track in inspect_avif(path)["tracks"] if track["handler"] == "pict"
    ]
    require(len(tracks) == 1, "pinned AVIF must contain one picture track")

    headers = []
    for sample_index, sample in enumerate(tracks[0]["samples"]):
        cursor = sample["offset"]
        sample_end = cursor + sample["length"]
        while cursor < sample_end:
            header = data[cursor]
            if header & 0x85 or not header & 0x02:
                raise ValueError("pinned AVIF has an unexpected AV1 OBU header")
            payload_size, payload_start = read_uleb128(data, cursor + 1)
            payload_end = payload_start + payload_size
            if payload_end > sample_end:
                raise ValueError("AV1 OBU exceeds its pinned sample")
            if (header >> 3) & 0x0F == 1:
                headers.append(
                    (sample_index, payload_start, data[payload_start:payload_end])
                )
            cursor = payload_end
        if cursor != sample_end:
            raise ValueError("AV1 OBUs do not fill their pinned sample")
    return headers


def color_field_positions(payload: bytes) -> dict[str, int | bool]:
    """Read profile-0 sequence fields and locate its byte-sized CICP values."""
    bits = Bits(payload)
    profile = bits.read(3)
    still_picture = bool(bits.bit())
    reduced_header = bool(bits.bit())
    if profile != 0 or still_picture or reduced_header:
        raise ValueError("source must use an ordinary profile-0 sequence header")

    decoder_model_present = False
    decoder_delay_length = 0
    if bits.bit():
        bits.read(32)
        bits.read(32)
        if bits.bit():
            bits.uvlc()
        decoder_model_present = bool(bits.bit())
        if decoder_model_present:
            decoder_delay_length = bits.read(5) + 1
            bits.read(32)
            bits.read(5)
            bits.read(5)

    display_model_present = bool(bits.bit())
    operating_point_count = bits.read(5) + 1
    for _ in range(operating_point_count):
        bits.read(12)
        level = bits.read(5)
        if level > 7:
            bits.bit()
        if decoder_model_present and bits.bit():
            bits.read(decoder_delay_length)
            bits.read(decoder_delay_length)
            bits.bit()
        if display_model_present and bits.bit():
            bits.read(4)

    width_bits = bits.read(4) + 1
    height_bits = bits.read(4) + 1
    bits.read(width_bits)
    bits.read(height_bits)
    if bits.bit():
        bits.read(4)
        bits.read(3)

    bits.read(3)  # 128x128 superblocks and two intra-filter flags
    bits.read(4)  # compound, warped motion, dual filter, and order hint
    order_hint = bool(bits.bit())
    if order_hint:
        bits.read(2)
    screen_content_tools = 2 if bits.bit() else bits.bit()
    if screen_content_tools and not bits.bit():
        bits.bit()
    if order_hint:
        bits.read(3)
    bits.read(3)  # super-resolution, CDEF, and restoration

    high_bitdepth = bool(bits.bit())
    monochrome = bool(bits.bit())
    if not bits.bit():
        raise ValueError("source sequence must include CICP color description")

    primaries_position = bits.position
    color_primaries = bits.read(8)
    transfer_characteristics = bits.read(8)
    matrix_position = bits.position
    matrix_coefficients = bits.read(8)
    return {
        "profile": profile,
        "bit_depth": 10 if high_bitdepth else 8,
        "monochrome": monochrome,
        "color_primaries": color_primaries,
        "transfer_characteristics": transfer_characteristics,
        "matrix_coefficients": matrix_coefficients,
        "primaries_position": primaries_position,
        "matrix_position": matrix_position,
    }


def write_bits(data: bytearray, start: int, width: int, value: int) -> None:
    """Replace one bounded big-endian AV1 syntax field."""
    if start < 0 or width <= 0 or start + width > len(data) * 8:
        raise ValueError("AV1 sequence-header field is outside the pinned sample")
    if value < 0 or value >= 1 << width:
        raise ValueError("replacement AV1 syntax value does not fit its field")
    for index in range(width):
        position = start + index
        mask = 1 << (7 - position % 8)
        bit = (value >> (width - index - 1)) & 1
        data[position // 8] = (data[position // 8] & ~mask) | (bit * mask)


def nclx_offsets(data: bytes) -> list[int]:
    """Return the offsets of every nclx color property in the complete file."""
    offsets = []
    cursor = 0
    while True:
        offset = data.find(b"nclx", cursor)
        if offset < 0:
            break
        offsets.append(offset)
        cursor = offset + len(b"nclx")
    require(
        len(offsets) == EXPECTED_NCLX_COUNT,
        "source AVIF must contain its item and track nclx properties",
    )
    return offsets


def nclx_cicp(data: bytes, offset: int) -> tuple[int, int, int]:
    return tuple(
        int.from_bytes(data[offset + start : offset + start + 2], "big")
        for start in (4, 6, 8)
    )


def replace_nclx(data: bytearray, offset: int) -> None:
    data[offset + 4 : offset + 10] = b"".join(
        value.to_bytes(2, "big") for value in IDENTITY_CICP
    )


def fixture_bytes(source: bytes) -> bytes:
    if sha256(source) != SOURCE_SHA256:
        raise ValueError("pinned profile-0 AVIF differs from its source hash")
    headers = sequence_headers(source, SOURCE_PATH)
    if [sample for sample, _, _ in headers] != [0]:
        raise ValueError("pinned AVIF must signal one sequence header in sample zero")
    _, payload_start, payload = headers[0]
    fields = color_field_positions(payload)
    if (
        fields["bit_depth"] != 8
        or fields["monochrome"]
        or (
            fields["color_primaries"],
            fields["transfer_characteristics"],
            fields["matrix_coefficients"],
        )
        != SOURCE_SEQUENCE_CICP
    ):
        raise ValueError("pinned source is not the expected 8-bit profile-0 BT.709 AVIF")

    output = bytearray(source)
    base_bit = payload_start * 8
    primaries_position = int(fields["primaries_position"])
    matrix_position = int(fields["matrix_position"])
    write_bits(output, base_bit + primaries_position, 8, IDENTITY_CICP[0])
    write_bits(output, base_bit + primaries_position + 8, 8, IDENTITY_CICP[1])
    write_bits(output, base_bit + matrix_position, 8, IDENTITY_CICP[2])

    for offset in nclx_offsets(source):
        if nclx_cicp(source, offset) != SOURCE_NCLX_CICP:
            raise ValueError("pinned item or track nclx CICP differs from source")
        replace_nclx(output, offset)

    result = normalize_creation_timestamps(bytes(output))
    digest = sha256(result)
    require(digest == EXPECTED_SHA256, f"generated AVIF hash changed: {digest}")
    validate_output(result)
    return result


def validate_output(data: bytes) -> None:
    require(PIL.__version__ == "12.2.0", "Pillow oracle version changed")
    require(features.version("avif") == "1.4.1", "libavif version changed")
    require(
        _avif.codec_versions() == EXPECTED_CODECS,
        "AVIF decoder or encoder codec version changed",
    )
    require(sha256(data) == EXPECTED_SHA256, "identity-matrix error fixture hash differs")

    with tempfile.NamedTemporaryFile(suffix=".avif") as handle:
        handle.write(data)
        handle.flush()
        headers = sequence_headers(data, Path(handle.name))
    require(len(headers) == 1, "error fixture must retain one AV1 sequence header")
    sequence = color_field_positions(headers[0][2])
    require(
        sequence["profile"] == 0
        and sequence["bit_depth"] == 8
        and (
            sequence["color_primaries"],
            sequence["transfer_characteristics"],
            sequence["matrix_coefficients"],
        )
        == IDENTITY_CICP,
        "error fixture does not declare profile-0 identity CICP",
    )
    require(
        all(nclx_cicp(data, offset) == IDENTITY_CICP for offset in nclx_offsets(data)),
        "error fixture item and track CICP declarations differ",
    )

    try:
        with Image.open(io.BytesIO(data)) as image:
            require(
                image.format == "AVIF"
                and image.size == (64, 64)
                and image.n_frames == 5,
                "Pillow opened the error fixture with unexpected image metadata",
            )
            image.seek(0)
            image.load()
    except RuntimeError as error:
        require(
            str(error) == EXPECTED_PILLOW_ERROR,
            f"Pillow failure changed: {type(error).__name__}: {error}",
        )
    else:
        raise ValueError("Pillow unexpectedly decoded the profile-0 identity matrix")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    data = fixture_bytes(SOURCE_PATH.read_bytes())
    if args.check:
        if not OUTPUT_PATH.is_file() or OUTPUT_PATH.read_bytes() != data:
            raise SystemExit(
                f"{OUTPUT_PATH.relative_to(ROOT)} differs from its deterministic mutation"
            )
        action = "verified"
    else:
        if OUTPUT_PATH.exists() and OUTPUT_PATH.read_bytes() != data:
            raise SystemExit(f"refusing to replace different fixture at {OUTPUT_PATH}")
        OUTPUT_PATH.parent.mkdir(parents=True, exist_ok=True)
        OUTPUT_PATH.write_bytes(data)
        action = "wrote"
    print(f"{action} {OUTPUT_PATH.relative_to(ROOT)} sha256={EXPECTED_SHA256}")


if __name__ == "__main__":
    main()
