#!/usr/bin/env python3
"""Probe whether Pillow full-keyframe WebP frames reuse still bitstreams."""

import argparse
import io
import json
import struct

from PIL import Image


def chunk_stream(data, offset=0):
    output = []
    while offset + 8 <= len(data):
        kind = data[offset : offset + 4]
        size = struct.unpack_from("<I", data, offset + 4)[0]
        payload_start = offset + 8
        payload_end = payload_start + size
        if payload_end > len(data):
            raise ValueError("truncated WebP chunk")
        output.append((kind, data[payload_start:payload_end]))
        offset = payload_end + (size & 1)
    return output


def chunks(data):
    if data[:4] != b"RIFF" or data[8:12] != b"WEBP":
        raise ValueError("not a WebP RIFF container")
    return chunk_stream(data, 12)


def image_chunks(data):
    return [
        (kind, payload)
        for kind, payload in chunks(data)
        if kind in (b"ALPH", b"VP8 ", b"VP8L")
    ]


def encoded(image, **kwargs):
    buffer = io.BytesIO()
    image.save(buffer, format="WEBP", **kwargs)
    return buffer.getvalue()


def frame_report(mode, lossless, controls):
    first = Image.new(mode, (9, 7), (17, 34, 51, 128) if mode == "RGBA" else (17, 34, 51))
    second = Image.new(mode, (9, 7), (201, 7, 99, 192) if mode == "RGBA" else (201, 7, 99))
    common = {"lossless": lossless, "quality": 80, "method": 4}
    animation_options = {
        "default": {},
        "retained": {"loop": 2, "background": (0, 0, 0, 0)},
        "opaque_background": {"loop": 2, "background": (1, 2, 3, 4)},
    }[controls]
    animation = encoded(
        first,
        save_all=True,
        append_images=[second],
        duration=[17, 33],
        kmax=1,
        **animation_options,
        **common,
    )
    frames = [
        (payload[:16], chunk_stream(payload[16:]))
        for kind, payload in chunks(animation)
        if kind == b"ANMF"
    ]
    still_chunks = [image_chunks(encoded(image, **common)) for image in (first, second)]
    return {
        "mode": mode,
        "lossless": lossless,
        "controls": controls,
        "animation_bytes": len(animation),
        "frame_count": len(frames),
        "frame_headers_hex": [header.hex() for header, _ in frames],
        "nested_chunk_kinds": [
            [kind.decode("ascii") for kind, _ in nested] for _, nested in frames
        ],
        "still_chunk_kinds": [
            [kind.decode("ascii") for kind, _ in nested] for nested in still_chunks
        ],
        "nested_equals_still": [
            nested == still for (_, nested), still in zip(frames, still_chunks, strict=True)
        ],
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--pretty", action="store_true")
    args = parser.parse_args()
    report = [
        frame_report(mode, lossless, controls)
        for mode in ("RGB", "RGBA")
        for lossless in (False, True)
        for controls in ("default", "retained", "opaque_background")
    ]
    print(json.dumps(report, indent=2 if args.pretty else None, sort_keys=True))


if __name__ == "__main__":
    main()
