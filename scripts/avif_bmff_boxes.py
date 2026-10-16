"""Small, dependency-free helpers for rewriting standard BMFF boxes."""

from __future__ import annotations

from collections.abc import Callable
import struct


Box = tuple[bytes, bytes, bytes]
CONTAINER_KINDS = (b"moov", b"trak", b"mdia", b"minf", b"stbl", b"iprp")


def read_boxes(data: bytes, start: int, end: int) -> list[Box]:
    """Return each bounded box's kind, complete bytes, and payload."""
    boxes = []
    position = start
    while position < end:
        if end - position < 8:
            raise RuntimeError("AVIF box header is truncated")
        size = struct.unpack_from(">I", data, position)[0]
        kind = data[position + 4 : position + 8]
        header_size = 8
        if size == 1:
            if end - position < 16:
                raise RuntimeError("AVIF large-box header is truncated")
            size = struct.unpack_from(">Q", data, position + 8)[0]
            header_size = 16
        elif size == 0:
            size = end - position
        if size < header_size or size > end - position:
            raise RuntimeError("AVIF box size is outside its parent")
        box_end = position + size
        boxes.append((kind, data[position:box_end], data[position + header_size : box_end]))
        position = box_end
    if position != end:
        raise RuntimeError("AVIF boxes do not fill their parent")
    return boxes


def pack_box(kind: bytes, payload: bytes) -> bytes:
    """Encode one box using a checked 32-bit size field."""
    if len(kind) != 4:
        raise RuntimeError("AVIF box kind must contain exactly four bytes")
    size = len(payload) + 8
    if size > 0xFFFF_FFFF:
        raise RuntimeError("AVIF fixture box exceeds its 32-bit size field")
    return struct.pack(">I4s", size, kind) + payload


def rewrite_boxes(
    data: bytes,
    transform: Callable[[tuple[bytes, ...], bytes], bytes],
    path: tuple[bytes, ...] = (),
) -> bytes:
    """Rebuild standard-size boxes while preserving untouched payloads."""
    boxes = read_boxes(data, 0, len(data))
    if b"".join(pack_box(kind, payload) for kind, _, payload in boxes) != data:
        raise RuntimeError("container-edge source must use standard-size box headers")

    rebuilt = []
    for kind, _, payload in boxes:
        child_path = path + (kind,)
        if kind == b"meta":
            if payload[:4] != bytes(4):
                raise RuntimeError("container-edge meta must use a version-zero FullBox")
            payload = payload[:4] + rewrite_boxes(payload[4:], transform, child_path)
        elif kind in CONTAINER_KINDS:
            payload = rewrite_boxes(payload, transform, child_path)
        rebuilt.append(pack_box(kind, transform(child_path, payload)))
    return b"".join(rebuilt)
