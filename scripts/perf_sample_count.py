#!/usr/bin/env python3
"""Count exact samples in an uncompressed little-endian perf.data file."""

from __future__ import annotations

import argparse
import struct
from pathlib import Path

# Linux perf's PERFILE2 header and perf_event_header / PERF_RECORD_SAMPLE ABI:
# https://github.com/torvalds/linux/blob/v7.0/tools/perf/util/header.h
# https://github.com/torvalds/linux/blob/v7.0/include/uapi/linux/perf_event.h
# PERF_RECORD_COMPRESSED is perf's userspace record type, not a kernel record:
# https://github.com/torvalds/linux/blob/v7.0/tools/lib/perf/include/perf/event.h
HEADER_BYTES = 72
EVENT_HEADER = struct.Struct("<IHH")
SAMPLE = 9
UNUSABLE_RECORDS = {
    2: "lost events", 13: "lost samples", 81: "compressed data", 83: "compressed data",
}


def sample_count(path: Path) -> int:
    """Validate record bounds and count raw samples without rounded report text."""
    with path.open("rb") as stream:
        file_size = stream.seek(0, 2)
        stream.seek(0)
        header = stream.read(HEADER_BYTES)
        if len(header) != HEADER_BYTES or header[:8] != b"PERFILE2":
            raise ValueError("expected a little-endian PERFILE2 file header")
        header_size = struct.unpack_from("<Q", header, 8)[0]
        data_offset, data_size = struct.unpack_from("<QQ", header, 40)
        data_end = data_offset + data_size
        if not HEADER_BYTES <= header_size <= data_offset <= data_end <= file_size:
            raise ValueError("perf data section exceeds the retained file bounds")
        stream.seek(data_offset)
        count = 0
        while stream.tell() < data_end:
            offset = stream.tell()
            if data_end - offset < EVENT_HEADER.size:
                raise ValueError(f"truncated perf event header at offset {offset}")
            record = stream.read(EVENT_HEADER.size)
            if len(record) != EVENT_HEADER.size:
                raise ValueError(f"truncated perf file at offset {offset}")
            kind, _misc, size = EVENT_HEADER.unpack(record)
            if size < EVENT_HEADER.size or offset + size > data_end:
                raise ValueError(f"perf event exceeds the data section at offset {offset}")
            if kind in UNUSABLE_RECORDS:
                raise ValueError(f"perf profile contains {UNUSABLE_RECORDS[kind]}")
            count += kind == SAMPLE
            stream.seek(offset + size)
        return count


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("profile", type=Path)
    args = parser.parse_args()
    try:
        count = sample_count(args.profile)
    except (OSError, ValueError) as error:
        raise SystemExit(f"perf sample counting failed: {error}") from error
    print(count)


if __name__ == "__main__":
    main()
