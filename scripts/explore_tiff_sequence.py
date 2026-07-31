#!/usr/bin/env python3
"""Map Pillow's deterministic classic-TIFF multipage byte layout.

This is an exploration tool, not a test oracle. It writes only in-memory TIFF
streams and reports the IFD chain, strip placement, decoded page identity, and
repeat-save determinism needed before implementing sequence encode/decode.
"""

import hashlib
import io
import json

from PIL import Image, features
from PIL import __version__ as pillow_version


def scalar(values):
    if isinstance(values, tuple):
        return values[0] if len(values) == 1 else list(values)
    return values


def ifd_chain(data):
    if data[:2] == b"II":
        byte_order = "little"
    elif data[:2] == b"MM":
        byte_order = "big"
    else:
        raise ValueError("not a classic TIFF byte-order marker")
    if int.from_bytes(data[2:4], byte_order) != 42:
        raise ValueError("not a classic TIFF stream")

    type_sizes = {1: 1, 2: 1, 3: 2, 4: 4, 5: 8}
    offset = int.from_bytes(data[4:8], byte_order)
    seen = set()
    pages = []
    while offset:
        if offset in seen:
            pages.append({"repeated_ifd_offset": offset})
            break
        seen.add(offset)
        count = int.from_bytes(data[offset : offset + 2], byte_order)
        entries = {}
        cursor = offset + 2
        for _ in range(count):
            entry = data[cursor : cursor + 12]
            tag = int.from_bytes(entry[0:2], byte_order)
            field_type = int.from_bytes(entry[2:4], byte_order)
            value_count = int.from_bytes(entry[4:8], byte_order)
            byte_len = type_sizes.get(field_type, 1) * value_count
            value_offset = (
                cursor + 8
                if byte_len <= 4
                else int.from_bytes(entry[8:12], byte_order)
            )
            raw = data[value_offset : value_offset + byte_len]
            unit = type_sizes.get(field_type, 1)
            values = tuple(
                int.from_bytes(raw[index : index + unit], byte_order)
                for index in range(0, len(raw), unit)
            )
            entries[tag] = scalar(values)
            cursor += 12
        next_offset = int.from_bytes(data[cursor : cursor + 4], byte_order)
        pages.append(
            {
                "ifd_offset": offset,
                "entry_count": count,
                "width": entries.get(256),
                "height": entries.get(257),
                "bits_per_sample": entries.get(258, 1),
                "compression": entries.get(259, 1),
                "strip_offsets": entries.get(273),
                "samples_per_pixel": entries.get(277, 1),
                "strip_byte_counts": entries.get(279),
                "next_ifd_offset": next_offset,
            }
        )
        offset = next_offset
    return pages


def decoded_pages(data):
    result = []
    with Image.open(io.BytesIO(data)) as image:
        for index in range(image.n_frames):
            image.seek(index)
            image.load()
            pixels = image.tobytes()
            result.append(
                {
                    "index": index,
                    "size": list(image.size),
                    "mode": image.mode,
                    "pixels_sha256": hashlib.sha256(pixels).hexdigest(),
                }
            )
    return result


def standalone_tiff(image, compression):
    stream = io.BytesIO()
    kwargs = {}
    if compression != "raw":
        kwargs["compression"] = compression
    image.save(stream, format="TIFF", **kwargs)
    return stream.getvalue()


def relocate_standalone(output, page, base, previous_next_position):
    """Append one Pillow still TIFF and relocate its absolute IFD references."""
    if page[:4] != b"II\x2a\x00":
        raise ValueError("exploration assembly expects little-endian classic TIFF")
    output.extend(page)
    local_ifd = int.from_bytes(page[4:8], "little")
    ifd = base + local_ifd
    count = int.from_bytes(output[ifd : ifd + 2], "little")
    cursor = ifd + 2
    type_sizes = {1: 1, 2: 1, 3: 2, 4: 4, 5: 8}
    for _ in range(count):
        tag = int.from_bytes(output[cursor : cursor + 2], "little")
        field_type = int.from_bytes(output[cursor + 2 : cursor + 4], "little")
        value_count = int.from_bytes(output[cursor + 4 : cursor + 8], "little")
        byte_len = type_sizes.get(field_type, 1) * value_count
        if byte_len > 4:
            local_value = int.from_bytes(output[cursor + 8 : cursor + 12], "little")
            output[cursor + 8 : cursor + 12] = (local_value + base).to_bytes(
                4, "little"
            )
        if tag == 273:
            if value_count != 1 or field_type != 4:
                raise ValueError("exploration assembly expects one LONG strip offset")
            local_strip = int.from_bytes(output[cursor + 8 : cursor + 12], "little")
            output[cursor + 8 : cursor + 12] = (local_strip + base).to_bytes(
                4, "little"
            )
        cursor += 12
    next_position = cursor
    if previous_next_position is not None:
        output[previous_next_position : previous_next_position + 4] = ifd.to_bytes(
            4, "little"
        )
    return next_position


def assemble_relocated_stills(frames, compression):
    output = bytearray()
    previous_next_position = None
    for frame in frames:
        while len(output) % 16:
            output.append(0)
        base = len(output)
        page = standalone_tiff(frame, compression)
        previous_next_position = relocate_standalone(
            output, page, base, previous_next_position
        )
    while len(output) % 16:
        output.append(0)
    return bytes(output)


def save_case(name, frames, compression):
    kwargs = {"save_all": True, "append_images": frames[1:]}
    if compression != "raw":
        kwargs["compression"] = compression

    outputs = []
    for _ in range(2):
        stream = io.BytesIO()
        frames[0].save(stream, format="TIFF", **kwargs)
        outputs.append(stream.getvalue())
    data = outputs[0]
    assembled = assemble_relocated_stills(frames, compression)
    return {
        "name": name,
        "compression": compression,
        "deterministic": outputs[0] == outputs[1],
        "matches_relocated_still_assembly": data == assembled,
        "bytes": len(data),
        "sha256": hashlib.sha256(data).hexdigest(),
        "ifds": ifd_chain(data),
        "decoded": decoded_pages(data),
    }


def main():
    if pillow_version != "12.2.0":
        raise RuntimeError(f"Pillow 12.2.0 is required, found {pillow_version}")
    libtiff = features.version_codec("libtiff")
    if libtiff != "4.7.1":
        raise RuntimeError(f"libtiff 4.7.1 is required, found {libtiff}")

    rgb_a = Image.new("RGB", (9, 7), (17, 43, 91))
    rgb_b = Image.new("RGB", (9, 7), (201, 79, 23))
    small_l = Image.new("L", (5, 3), 137)
    cases = []
    for compression in ("raw", "tiff_lzw", "tiff_adobe_deflate", "packbits"):
        cases.append(save_case(f"rgb_{compression}", [rgb_a, rgb_b], compression))
    cases.append(save_case("mixed_size_mode_raw", [rgb_a, small_l], "raw"))
    print(
        json.dumps(
            {
                "pillow": pillow_version,
                "libtiff": libtiff,
                "cases": cases,
            },
            indent=2,
            sort_keys=True,
        )
    )


if __name__ == "__main__":
    main()
