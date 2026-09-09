#!/usr/bin/env python3
"""Collect an independent persistent dav1d trace for ``animated.avif``.

This is a bounded diagnostic oracle for the first AVIF sequence case.  It
copies the pinned dav1d checkout into a disposable ignored workspace, builds a
scalar command-line decoder, and records the decoder's frame lifecycle,
high-level block traversal, and displayed YUV bytes.  The repository's Rust
decoder is never invoked and no canonical fixture, matrix, roadmap, or output
file is written.

The AV1 OBU report and BMFF report are kept as separate syntax evidence.  The
instrumented dav1d trace is the only evidence labelled as decoder state.  In
particular, a show-existing display has no corresponding tile decode event.
"""

from __future__ import annotations

import argparse
import collections
import hashlib
import io
import json
import os
import shutil
import struct
import subprocess
import tempfile
from pathlib import Path
from typing import Any

from PIL import Image

from generate_av1_reconstruction_refs import (
    DAV1D_COMMIT,
    resolve_tool,
    sha256,
    tool_environment,
    verify_source,
)
from inspect_av1_obus import inspect as inspect_av1
from inspect_avif_bitstreams import inspect as inspect_avif


ROOT = Path(__file__).resolve().parent.parent
FIXTURE = ROOT / "tests" / "fixtures" / "input" / "images" / "avif" / "animated.avif"
OUTPUT = ROOT / "target" / "oracle-staging" / "av1-sequence" / "animated"
COPYING_SHA256 = "dd92c3c2247c5651606fc23a5e2d6a1ebc5ace9a3e49cbde0e12f05ad1cb1ee5"
SCHEMA = "image-slash-star/av1-sequence-oracle@1"
TRACE_FILES = ("src/obu.c", "src/decode.c", "tools/output/output.c")


def run(
    command: list[str],
    *,
    cwd: Path | None = None,
    env: dict[str, str] | None = None,
    check: bool = True,
) -> subprocess.CompletedProcess[bytes]:
    """Run a command without exposing an invocation-specific path in output."""

    result = subprocess.run(
        command,
        cwd=cwd,
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if check and result.returncode:
        stderr = result.stderr.decode("utf-8", "replace").strip()
        stdout = result.stdout.decode("utf-8", "replace").strip()
        raise RuntimeError(
            f"command failed ({result.returncode}): {command[0]}"
            + (f": {(stderr or stdout)[-2000:]}" if (stderr or stdout) else "")
        )
    return result


def text_output(result: subprocess.CompletedProcess[bytes]) -> str:
    return result.stdout.decode("utf-8", "replace").strip()


def combined_output(result: subprocess.CompletedProcess[bytes]) -> str:
    return (result.stdout + result.stderr).decode("utf-8", "replace").strip()


def digest_file(path: Path) -> str:
    return sha256(path.read_bytes())


def git_tree_digest(source: Path) -> str:
    return sha256(run(["git", "-C", str(source), "ls-tree", "-r", "HEAD"]).stdout)


def json_bytes(value: object) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True, ensure_ascii=False) + "\n").encode(
        "utf-8"
    )


def write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(json_bytes(value))


def relative_artifacts(root: Path) -> dict[str, bytes]:
    return {
        item.relative_to(root).as_posix(): item.read_bytes()
        for item in sorted(root.rglob("*"))
        if item.is_file() and item.name != "index.json"
    }


def artifact_records(root: Path) -> list[dict[str, object]]:
    return [
        {"path": name, "bytes": len(data), "sha256": sha256(data)}
        for name, data in relative_artifacts(root).items()
    ]


def output_guard() -> None:
    root = ROOT.resolve()
    target = OUTPUT.resolve()
    if root not in target.parents:
        raise RuntimeError("sequence output escaped the repository")
    canonical = (
        ROOT / "manifest.yaml",
        ROOT / "pillow-oracle.lock.yaml",
        ROOT / "roadmap.json",
        ROOT / "tests" / "fixtures" / "coverage_matrix.json",
        ROOT / "tests" / "fixtures" / "input",
        ROOT / "tests" / "fixtures" / "outputs",
        ROOT / "docs",
        ROOT / "src",
    )
    for path in canonical:
        resolved = path.resolve()
        if target == resolved or target in resolved.parents or resolved in target.parents:
            raise RuntimeError(f"sequence output collides with canonical path: {path}")


def copy_source(source: Path, destination: Path) -> None:
    run(["git", "clone", "--quiet", "--no-hardlinks", str(source), str(destination)])
    run(["git", "-C", str(destination), "checkout", "--quiet", DAV1D_COMMIT])
    verify_source(destination)


def replace_once(path: Path, old: str, new: str) -> None:
    text = path.read_text()
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"trace anchor count for {path.name}: expected 1, found {count}")
    path.write_text(text.replace(old, new, 1))


def instrument(source: Path) -> None:
    """Install only high-level, field-wise trace hooks in the copied source."""

    obu = source / "src" / "obu.c"
    replace_once(obu, "#include <errno.h>\n", "#include <errno.h>\n#include <inttypes.h>\n")
    replace_once(
        obu,
        '#include "src/thread_task.h"\n\n',
        '''#include "src/thread_task.h"\n\n'''
        "static unsigned sequence_trace_header_index;\n"
        "static unsigned sequence_trace_last_header_index;\n\n"
        "static void sequence_trace_reference(const Dav1dContext *const c, const int slot)\n"
        "{\n"
        "    const Dav1dFrameHeader *const hdr = c->refs[slot].p.p.frame_hdr;\n"
        "    if (!hdr) {\n"
        '        fputs("null", stdout);\n'
        "        return;\n"
        "    }\n"
        '    printf("{\\"slot\\":%d,\\"frame_type\\":%d,\\"frame_offset\\":%u,"\n'
        '           "\\"frame_id\\":%u,\\"show_frame\\":%u,\\"showable_frame\\":%u,"\n'
        '           "\\"width\\":%d,\\"height\\":%d,\\"render_width\\":%d,"\n'
        '           "\\"render_height\\":%d,\\"refresh_frame_flags\\":%u}",\n'
        "           slot, hdr->frame_type, hdr->frame_offset, hdr->frame_id,\n"
        "           hdr->show_frame, hdr->showable_frame, hdr->width[1], hdr->height,\n"
        "           hdr->render_width, hdr->render_height, hdr->refresh_frame_flags);\n"
        "}\n\n"
        "static void sequence_trace_frame_header(const Dav1dContext *const c,\n"
        "                                        const Dav1dData *const in,\n"
        "                                        const int obu_type)\n"
        "{\n"
        "    const Dav1dFrameHeader *const hdr = c->frame_hdr;\n"
        "    const unsigned index = sequence_trace_header_index++;\n"
        "    sequence_trace_last_header_index = index;\n"
        '    printf("@LIFECYCLE {\\"event\\":\\"frame_header\\","\n'
        '           "\\"header_index\\":%u,\\"obu_type\\":%d,\\"pts\\":%" PRId64 ","\n'
        '           "\\"frame_type\\":%d,\\"frame_offset\\":%u,\\"frame_id\\":%u,"\n'
        '           "\\"show_existing_frame\\":%u,\\"existing_frame_idx\\":%u,"\n'
        '           "\\"show_frame\\":%u,\\"showable_frame\\":%u,"\n'
        '           "\\"error_resilient_mode\\":%u,\\"refresh_frame_flags\\":%u,"\n'
        '           "\\"width\\":%d,\\"height\\":%d,\\"render_width\\":%d,"\n'
        '           "\\"render_height\\":%d,\\"references_before\\":[",\n'
        "           index, obu_type, in->m.timestamp, hdr->frame_type,\n"
        "           hdr->frame_offset, hdr->frame_id, hdr->show_existing_frame,\n"
        "           hdr->show_existing_frame ? hdr->existing_frame_idx : 0,\n"
        "           hdr->show_frame, hdr->showable_frame, hdr->error_resilient_mode,\n"
        "           hdr->refresh_frame_flags, hdr->width[1], hdr->height,\n"
        "           hdr->render_width, hdr->render_height);\n"
        "    for (int slot = 0; slot < 8; slot++) {\n"
        "        if (slot) putchar(',');\n"
        "        sequence_trace_reference(c, slot);\n"
        "    }\n"
        '    puts("]}");\n'
        "}\n\n",
    )
    replace_once(
        obu,
        "        if ((res = parse_frame_hdr(c, &gb)) < 0) {\n"
        "            c->frame_hdr = NULL;\n"
        "            goto error;\n"
        "        }\n",
        "        if ((res = parse_frame_hdr(c, &gb)) < 0) {\n"
        "            c->frame_hdr = NULL;\n"
        "            goto error;\n"
        "        }\n"
        "        sequence_trace_frame_header(c, in, type);\n",
    )
    replace_once(
        obu,
        "            c->frame_hdr = NULL;\n"
        "        } else if (c->n_tiles == c->frame_hdr->tiling.cols * c->frame_hdr->tiling.rows) {\n",
        "            printf(\"@LIFECYCLE {\\\"event\\\":\\\"show_existing_queued\\\",\"\n"
        "                   \"\\\"header_index\\\":%u,\\\"pts\\\":%\" PRId64 \",\"\n"
        "                   \"\\\"existing_frame_idx\\\":%u,\\\"frame_type\\\":%d,\"\n"
        "                   \"\\\"show_frame\\\":%u,\\\"showable_frame\\\":%u}\\n\",\n"
        "                   sequence_trace_last_header_index, in->m.timestamp,\n"
        "                   c->frame_hdr->existing_frame_idx, c->frame_hdr->frame_type,\n"
        "                   c->frame_hdr->show_frame, c->frame_hdr->showable_frame);\n"
        "            c->frame_hdr = NULL;\n"
        "        } else if (c->n_tiles == c->frame_hdr->tiling.cols * c->frame_hdr->tiling.rows) {\n",
    )
    replace_once(
        obu,
        "            if (!c->n_tile_data)\n"
        "                goto error;\n"
        "            if ((res = dav1d_submit_frame(c)) < 0)\n"
        "                return res;\n"
        "            assert(!c->n_tile_data);\n",
        "            if (!c->n_tile_data)\n"
        "                goto error;\n"
        "            const int trace_frame_type = c->frame_hdr->frame_type;\n"
        "            const unsigned trace_frame_offset = c->frame_hdr->frame_offset;\n"
        "            const unsigned trace_show_frame = c->frame_hdr->show_frame;\n"
        "            const unsigned trace_showable_frame = c->frame_hdr->showable_frame;\n"
        "            const unsigned trace_refresh_frame_flags = c->frame_hdr->refresh_frame_flags;\n"
        "            const int trace_tile_groups = c->n_tile_data;\n"
        "            if ((res = dav1d_submit_frame(c)) < 0)\n"
        "                return res;\n"
        "            printf(\"@LIFECYCLE {\\\"event\\\":\\\"frame_submitted\\\",\"\n"
        "                   \"\\\"header_index\\\":%u,\\\"pts\\\":%\" PRId64 \",\"\n"
        "                   \"\\\"frame_type\\\":%d,\\\"frame_offset\\\":%u,\"\n"
        "                   \"\\\"show_frame\\\":%u,\\\"showable_frame\\\":%u,\"\n"
        "                   \"\\\"refresh_frame_flags\\\":%u,\\\"tile_groups\\\":%d}\\n\",\n"
        "                   sequence_trace_last_header_index, in->m.timestamp,\n"
        "                   trace_frame_type, trace_frame_offset, trace_show_frame,\n"
        "                   trace_showable_frame, trace_refresh_frame_flags,\n"
        "                   trace_tile_groups);\n"
        "            assert(!c->n_tile_data);\n",
    )

    decode = source / "src" / "decode.c"
    replace_once(
        decode,
        "    b->bl = bl;\n    b->bp = bp;\n    b->bs = bs;\n\n",
        "    b->bl = bl;\n    b->bp = bp;\n    b->bs = bs;\n\n"
        "    if (t->frame_thread.pass == 0)\n"
        '        printf("@BLOCK {\\"kind\\":\\"leaf\\",\\"pts\\":%" PRId64 ","\n'
        '               "\\"frame_offset\\":%u,\\"tile_col\\":%d,\\"tile_row\\":%d,"\n'
        '               "\\"x4\\":%d,\\"y4\\":%d,\\"width4\\":%d,\\"height4\\":%d,"\n'
        '               "\\"block_level\\":%d,\\"partition\\":%d,\\"block_size\\":%d,"\n'
        '               "\\"layout\\":%d,\\"bpc\\":%d,\\"pass\\":%d}\\n",\n'
        "               f->tile[0].data.m.timestamp, f->frame_hdr->frame_offset,\n"
        "               ts->tiling.col, ts->tiling.row, t->bx, t->by, bw4, bh4, bl, bp,\n"
        "               bs, f->cur.p.layout, f->cur.p.bpc, t->frame_thread.pass);\n\n",
    )
    replace_once(
        decode,
        "            if (DEBUG_BLOCK_INFO)\n"
        "                printf(\"poc=%d,y=%d,x=%d,bl=%d,ctx=%d,bp=%d: r=%d\\n\",\n"
        "                       f->frame_hdr->frame_offset, t->by, t->bx, bl, ctx, bp,\n"
        "                       ts->msac.rng);\n",
        "            if (t->frame_thread.pass == 0)\n"
        '                printf("@BLOCK {\\"kind\\":\\"partition\\",\\"pts\\":%" PRId64 ","\n'
        '                       "\\"frame_offset\\":%u,\\"tile_col\\":%d,\\"tile_row\\":%d,"\n'
        '                       "\\"x4\\":%d,\\"y4\\":%d,\\"block_level\\":%d,"\n'
        '                       "\\"context\\":%d,\\"partition\\":%d,\\"pass\\":%d}\\n",\n'
        "                       f->tile[0].data.m.timestamp, f->frame_hdr->frame_offset,\n"
        "                       ts->tiling.col, ts->tiling.row, t->bx, t->by, bl, ctx,\n"
        "                       bp, t->frame_thread.pass);\n"
        "            if (DEBUG_BLOCK_INFO)\n"
        "                printf(\"poc=%d,y=%d,x=%d,bl=%d,ctx=%d,bp=%d: r=%d\\n\",\n"
        "                       f->frame_hdr->frame_offset, t->by, t->bx, bl, ctx, bp,\n"
        "                       ts->msac.rng);\n",
    )
    replace_once(
        decode,
        "void dav1d_decode_frame_exit(Dav1dFrameContext *const f, int retval) {\n"
        "    const Dav1dContext *const c = f->c;\n\n",
        "void dav1d_decode_frame_exit(Dav1dFrameContext *const f, int retval) {\n"
        "    const Dav1dContext *const c = f->c;\n\n"
        '    printf("@LIFECYCLE {\\"event\\":\\"frame_decode_exit\\",\\"pts\\":%" PRId64 ","\n'
        '           "\\"frame_offset\\":%u,\\"status\\":%d}\\n",\n'
        "           f->n_tile_data ? f->tile[0].data.m.timestamp : INT64_MIN,\n"
        "           f->frame_hdr ? f->frame_hdr->frame_offset : 0, retval);\n\n",
    )

    output = source / "tools" / "output" / "output.c"
    replace_once(
        output,
        "#include <errno.h>\n",
        "#include <errno.h>\n#include <inttypes.h>\n",
    )
    replace_once(
        output,
        "int output_write(MuxerContext *const ctx, Dav1dPicture *const p) {\n"
        "    int res;\n",
        "int output_write(MuxerContext *const ctx, Dav1dPicture *const p) {\n"
        "    static unsigned sequence_trace_output_index;\n"
        "    const Dav1dFrameHeader *const hdr = p->frame_hdr;\n"
        '    printf("@OUTPUT {\\"event\\":\\"picture_output\\",\\"output_index\\":%u,"\n'
        '           "\\"pts\\":%" PRId64 ",\\"duration\\":%" PRId64 ","\n'
        '           "\\"width\\":%d,\\"height\\":%d,\\"layout\\":%d,\\"bpc\\":%d,"\n'
        '           "\\"frame_header_available\\":%d,\\"frame_type\\":%d,"\n'
        '           "\\"show_frame\\":%d,\\"showable_frame\\":%d}\\n",\n'
        "           sequence_trace_output_index++, p->m.timestamp, p->m.duration,\n"
        "           p->p.w, p->p.h, p->p.layout, p->p.bpc, hdr != NULL,\n"
        "           hdr ? hdr->frame_type : -1, hdr ? hdr->show_frame : -1,\n"
        "           hdr ? hdr->showable_frame : -1);\n"
        "    int res;\n",
    )


def source_anchor_report(source: Path) -> dict[str, object]:
    anchors = {
        "ivf_timestamp": (
            "tools/input/ivf.c",
            ("timebase[0]", "*ts = rl64(data)", "buf->m.timestamp = ts"),
        ),
        "cli_send_get_drain": (
            "tools/dav1d.c",
            ("dav1d_send_data(c, &data)", "dav1d_get_picture(c, &p)", "// flush"),
        ),
        "frame_header_and_submit": (
            "src/obu.c",
            ("parse_frame_hdr(c, &gb)", "show_existing_frame", "dav1d_submit_frame(c)"),
        ),
        "decode_exit": (
            "src/decode.c",
            ("dav1d_decode_frame_exit", "f->n_tile_data", "f->frame_hdr"),
        ),
    }
    result: dict[str, object] = {}
    for name, (relative, needles) in anchors.items():
        path = source / relative
        text = path.read_text()
        missing = [needle for needle in needles if needle not in text]
        if missing:
            raise RuntimeError(f"source anchor missing in {relative}: {missing}")
        result[name] = {
            "path": relative,
            "sha256": digest_file(path),
            "contains": list(needles),
        }
    return result


def sequence_geometry(syntax_samples: list[dict[str, object]]) -> dict[str, object]:
    sequence: dict[str, object] | None = None
    frame: dict[str, object] | None = None
    for sample in syntax_samples:
        for obu in sample.get("obus", []):
            if sequence is None and obu.get("sequence_header") is not None:
                sequence = obu["sequence_header"]
            if frame is None and obu.get("frame_header") is not None:
                candidate = obu["frame_header"]
                if not candidate.get("show_existing_frame"):
                    frame = candidate
    if sequence is None or frame is None:
        raise RuntimeError("AV1 syntax report lacks sequence and display frame headers")
    width = int(frame["frame_width"])
    height = int(frame["frame_height"])
    if width <= 0 or height <= 0:
        raise RuntimeError("AV1 display frame has invalid dimensions")
    return {
        "width": width,
        "height": height,
        "bit_depth": int(sequence["bit_depth"]),
        "profile": int(sequence["profile"]),
        "subsampling_x": int(sequence["subsampling_x"]),
        "subsampling_y": int(sequence["subsampling_y"]),
        "monochrome": bool(sequence["monochrome"]),
    }


def make_ivf(
    data: bytes, samples: list[dict[str, object]], width: int, height: int, output: Path
) -> dict[str, object]:
    blob = bytearray(
        struct.pack(
            "<4sHH4sHHIIII",
            b"DKIF",
            0,
            32,
            b"AV01",
            width,
            height,
            30,
            1,
            len(samples),
            0,
        )
    )
    records = []
    for index, sample in enumerate(samples):
        start = int(sample["offset"])
        length = int(sample["length"])
        end = start + length
        if start < 0 or end > len(data):
            raise RuntimeError(f"sample {index} exceeds the AVIF input")
        payload = data[start:end]
        actual_hash = sha256(payload)
        if actual_hash != sample["sha256"]:
            raise RuntimeError(f"sample {index} hash disagrees with inspector")
        blob.extend(struct.pack("<IQ", len(payload), index))
        blob.extend(payload)
        records.append(
            {
                "sample_index": index,
                "pts": index,
                "offset": start,
                "length": len(payload),
                "sha256": actual_hash,
            }
        )
    output.write_bytes(bytes(blob))
    return {
        "path": output.name,
        "bytes": len(blob),
        "sha256": sha256(bytes(blob)),
        "codec": "AV01",
        "width": width,
        "height": height,
        "rate_num": 30,
        "rate_den": 1,
        "timestamps": [record["pts"] for record in records],
        "samples": records,
    }


def pillow_report(data: bytes) -> dict[str, object]:
    stream = io.BytesIO(data)
    stream.name = FIXTURE.as_posix()
    with Image.open(stream) as image:
        count = int(getattr(image, "n_frames", 1))
        frames = []
        for index in range(count):
            image.seek(index)
            raw = image.tobytes()
            frames.append(
                {
                    "index": index,
                    "mode": image.mode,
                    "size": list(image.size),
                    "raw_bytes": len(raw),
                    "raw_sha256": sha256(raw),
                    "duration": image.info.get("duration"),
                    "duration_observation": (
                        "Pillow.Image.info" if "duration" in image.info else "not_present"
                    ),
                }
            )
        return {
            "frame_count": count,
            "mode": image.mode,
            "size": list(image.size),
            "loop_count": image.info.get("loop"),
            "loop_observation": (
                "Pillow.Image.info" if "loop" in image.info else "not_present"
            ),
            "frames": frames,
        }


def parse_trace(stdout: bytes) -> list[dict[str, object]]:
    records = []
    for line_number, raw_line in enumerate(stdout.splitlines(), 1):
        line = raw_line.decode("utf-8", "strict")
        if not line:
            continue
        for prefix, kind in (
            ("@LIFECYCLE ", "lifecycle"),
            ("@BLOCK ", "block"),
            ("@OUTPUT ", "output"),
        ):
            if line.startswith(prefix):
                payload = json.loads(line[len(prefix) :])
                if not isinstance(payload, dict):
                    raise RuntimeError(f"trace line {line_number} is not an object")
                if kind == "block":
                    payload["block_kind"] = payload.pop("kind")
                records.append({"kind": kind, **payload})
                break
        else:
            raise RuntimeError(f"unexpected dav1d stdout at line {line_number}: {line}")
    return records


def layout_name(layout: int) -> str:
    return {0: "I400", 1: "I420", 2: "I422", 3: "I444"}.get(
        layout, f"unknown:{layout}"
    )


def plane_shapes(width: int, height: int, layout: int) -> list[tuple[str, int, int]]:
    if layout == 0:
        return [("y", width, height)]
    if layout == 1:
        return [("y", width, height), ("u", (width + 1) // 2, (height + 1) // 2), ("v", (width + 1) // 2, (height + 1) // 2)]
    if layout == 2:
        return [("y", width, height), ("u", (width + 1) // 2, height), ("v", (width + 1) // 2, height)]
    if layout == 3:
        return [("y", width, height), ("u", width, height), ("v", width, height)]
    raise RuntimeError(f"unsupported dav1d output layout {layout}")


def output_frames(
    root: Path, role: str, output_path: Path, output_events: list[dict[str, object]]
) -> list[dict[str, object]]:
    data = output_path.read_bytes()
    cursor = 0
    frames = []
    for display_index, event in enumerate(output_events):
        width = int(event["width"])
        height = int(event["height"])
        layout = int(event["layout"])
        bpc = int(event["bpc"])
        if bpc <= 0 or bpc > 16:
            raise RuntimeError(f"invalid output bit depth {bpc}")
        bytes_per_sample = 1 if bpc <= 8 else 2
        planes = []
        total = 0
        for name, plane_width, plane_height in plane_shapes(width, height, layout):
            length = plane_width * plane_height * bytes_per_sample
            planes.append((name, plane_width, plane_height, length))
            total += length
        end = cursor + total
        if end > len(data):
            raise RuntimeError("dav1d YUV output ended before its output trace")
        raw = data[cursor:end]
        cursor = end
        offset = 0
        plane_reports = []
        for name, plane_width, plane_height, length in planes:
            plane = raw[offset : offset + length]
            offset += length
            plane_reports.append(
                {
                    "name": name,
                    "width": plane_width,
                    "height": plane_height,
                    "bytes": length,
                    "sha256": sha256(plane),
                }
            )
        relative = Path("decoded") / role / f"display_{display_index}.yuv"
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(raw)
        frames.append(
            {
                "display_index": display_index,
                "pts": int(event["pts"]),
                "duration": int(event["duration"]),
                "width": width,
                "height": height,
                "layout": layout_name(layout),
                "layout_code": layout,
                "bit_depth": bpc,
                "raw_path": relative.as_posix(),
                "raw_bytes": len(raw),
                "raw_sha256": sha256(raw),
                "planes": plane_reports,
            }
        )
    if cursor != len(data):
        raise RuntimeError(
            f"dav1d YUV output has {len(data) - cursor} trailing bytes after trace outputs"
        )
    return frames


def run_decoder(
    binary: Path,
    build: Path,
    ivf: Path,
    yuv: Path,
    env: dict[str, str],
) -> tuple[list[dict[str, object]], bytes, bytes]:
    command = [
        str(binary),
        "--input",
        str(ivf),
        "--demuxer",
        "ivf",
        "--output",
        str(yuv),
        "--muxer",
        "yuv",
        "--threads",
        "1",
        "--framedelay",
        "1",
        "--cpumask",
        "0",
        "--quiet",
    ]
    result = run(command, cwd=build, env=env, check=False)
    if result.returncode:
        stderr = result.stderr.decode("utf-8", "replace")
        raise RuntimeError(f"dav1d failed with {result.returncode}: {stderr[-1600:]}")
    return parse_trace(result.stdout), result.stdout, result.stderr


def build_dav1d(
    source: Path,
    build: Path,
    meson: Path,
    ninja: Path,
    env: dict[str, str],
) -> tuple[Path, dict[str, object]]:
    configure = [
        str(meson),
        "setup",
        str(build),
        str(source),
        "--buildtype=debug",
        "-Denable_tools=true",
        "-Denable_tests=false",
        "-Denable_examples=false",
        "-Dtestdata_tests=false",
        "-Denable_asm=false",
    ]
    run(configure, cwd=ROOT, env=env)
    compile_command = [str(meson), "compile", "-C", str(build), "tools/dav1d"]
    run(compile_command, cwd=ROOT, env=env)
    binary = build / "tools" / "dav1d"
    if not binary.is_file():
        raise RuntimeError("Meson did not produce tools/dav1d")
    version_result = run([str(binary), "--version"], cwd=build, env=env)
    meson_version = run([str(meson), "--version"], cwd=ROOT, env=env)
    ninja_version = run([str(ninja), "--version"], cwd=ROOT, env=env)
    tool_version = combined_output(version_result)
    return binary, {
        "configure_argv": [
            "meson",
            "setup",
            "<build>",
            "<source-copy>",
            "--buildtype=debug",
            "-Denable_tools=true",
            "-Denable_tests=false",
            "-Denable_examples=false",
            "-Dtestdata_tests=false",
            "-Denable_asm=false",
        ],
        "compile_argv": ["meson", "compile", "-C", "<build>", "tools/dav1d"],
        "tool_version": tool_version,
        "tool_version_sha256": sha256(tool_version.encode("utf-8")),
        "meson_version": text_output(meson_version),
        "meson_version_sha256": sha256(text_output(meson_version).encode("utf-8")),
        "ninja_version": text_output(ninja_version),
        "ninja_version_sha256": sha256(text_output(ninja_version).encode("utf-8")),
    }


def build_and_trace(
    source: Path,
    root: Path,
    ivf_path: Path,
    width: int,
    height: int,
    meson: Path,
    ninja: Path,
    env: dict[str, str],
    role: str,
) -> tuple[list[dict[str, object]], list[dict[str, object]], dict[str, object]]:
    plain_source = root / f"{role}-plain-source"
    trace_source = root / f"{role}-trace-source"
    plain_build = root / f"{role}-plain-build"
    trace_build = root / f"{role}-trace-build"
    copy_source(source, plain_source)
    copy_source(source, trace_source)
    instrument(trace_source)
    patch = run(["git", "-C", str(trace_source), "diff", "--no-ext-diff", "--binary"]).stdout
    if not patch:
        raise RuntimeError("trace source has no patch")

    plain_binary, plain_build_info = build_dav1d(
        plain_source, plain_build, meson, ninja, env
    )
    trace_binary, trace_build_info = build_dav1d(
        trace_source, trace_build, meson, ninja, env
    )
    plain_yuv = root / f"{role}-plain.yuv"
    trace_yuv = root / f"{role}-trace.yuv"
    plain_records, plain_stdout, plain_stderr = run_decoder(
        plain_binary, plain_build, ivf_path, plain_yuv, env
    )
    trace_records, trace_stdout, trace_stderr = run_decoder(
        trace_binary, trace_build, ivf_path, trace_yuv, env
    )
    if plain_stdout.strip() or plain_stderr.strip():
        raise RuntimeError("plain dav1d emitted unexpected diagnostic output")
    if trace_stderr.strip():
        raise RuntimeError("instrumented dav1d emitted stderr unexpectedly")
    plain_bytes = plain_yuv.read_bytes()
    trace_bytes = trace_yuv.read_bytes()
    if plain_bytes != trace_bytes:
        raise RuntimeError("instrumentation changed dav1d YUV output bytes")
    patch_path = root / "provenance" / f"{role}-sequence-trace.patch"
    patch_path.parent.mkdir(parents=True, exist_ok=True)
    patch_path.write_bytes(patch)
    return (
        plain_records,
        trace_records,
        {
            "role": role,
            "patch_path": patch_path,
            "patch_sha256": sha256(patch),
            "patch_bytes": len(patch),
            "plain_yuv_sha256": sha256(plain_bytes),
            "trace_yuv_sha256": sha256(trace_bytes),
            "yuv_bytes": len(trace_bytes),
            "plain_build": plain_build_info,
            "trace_build": trace_build_info,
        },
    )


def syntax_samples_for_track(
    syntax: dict[str, object], handler: str, track_id: int
) -> list[dict[str, object]]:
    role = f"track_{handler}"
    selected = [
        sample
        for sample in syntax.get("samples", [])
        if sample.get("role") == role
        and int(sample.get("identity", {}).get("track_id", -1)) == track_id
    ]
    selected.sort(key=lambda sample: int(sample["identity"]["sample"]))
    return selected


def track_report(
    container: dict[str, object], handler: str, track_id: int | None
) -> dict[str, object] | None:
    if track_id is None:
        return None
    tracks = [
        track
        for track in container.get("tracks", [])
        if int(track["track_id"]) == track_id and track["handler"] == handler
    ]
    if len(tracks) != 1:
        raise RuntimeError(f"expected one {handler} track with ID {track_id}")
    return tracks[0]


def frame_headers(sample: dict[str, object]) -> list[dict[str, object]]:
    return [
        obu["frame_header"]
        for obu in sample.get("obus", [])
        if obu.get("frame_header") is not None
    ]


def summarize_identity(
    track: dict[str, object],
    syntax_samples: list[dict[str, object]],
    records: list[dict[str, object]],
    displays: list[dict[str, object]],
) -> tuple[list[dict[str, object]], dict[str, object]]:
    headers = [record for record in records if record.get("kind") == "lifecycle" and record.get("event") == "frame_header"]
    submits = [record for record in records if record.get("kind") == "lifecycle" and record.get("event") == "frame_submitted"]
    exits = [record for record in records if record.get("kind") == "lifecycle" and record.get("event") == "frame_decode_exit"]
    show_existing = [record for record in records if record.get("kind") == "lifecycle" and record.get("event") == "show_existing_queued"]
    blocks = [record for record in records if record.get("kind") == "block"]
    if len(displays) != len(track["samples"]):
        raise RuntimeError(
            f"{track['handler']} track displayed {len(displays)} frames for {len(track['samples'])} samples"
        )
    if len(headers) != 7 or len(submits) != 6 or len(exits) != 6 or len(show_existing) != 1:
        raise RuntimeError(
            "animated lifecycle cardinality differs from the expected 7 headers, "
            "6 decoded frames, and 1 show-existing event"
        )
    if any(int(exit_record["status"]) != 0 for exit_record in exits):
        raise RuntimeError("dav1d reported a nonzero frame decode exit")
    output_by_pts = {int(display["pts"]): display for display in displays}
    if len(output_by_pts) != len(displays):
        raise RuntimeError("display PTS values are not unique")
    submit_by_header = {int(record["header_index"]): record for record in submits}
    headers_by_pts: dict[int, list[dict[str, object]]] = collections.defaultdict(list)
    for record in headers:
        headers_by_pts[int(record["pts"])].append(record)
    exit_by_key: dict[tuple[int, int], list[dict[str, object]]] = collections.defaultdict(list)
    for record in exits:
        exit_by_key[(int(record["pts"]), int(record["frame_offset"]))].append(record)
    block_by_key: dict[tuple[int, int], list[dict[str, object]]] = collections.defaultdict(list)
    for record in blocks:
        block_by_key[(int(record["pts"]), int(record["frame_offset"]))].append(record)

    identities = []
    decoded_index = 0
    for sample_index, (sample, syntax_sample) in enumerate(
        zip(track["samples"], syntax_samples)
    ):
        pts = sample_index
        syntax_headers = frame_headers(syntax_sample)
        actual_headers = headers_by_pts[pts]
        if len(syntax_headers) != len(actual_headers):
            raise RuntimeError(
                f"sample {sample_index} syntax/header count mismatch: "
                f"{len(syntax_headers)} vs {len(actual_headers)}"
            )
        events = []
        for header, syntax_header in zip(actual_headers, syntax_headers):
            existing = bool(header["show_existing_frame"])
            frame_offset = int(header["frame_offset"])
            event: dict[str, object] = {
                "header_index": int(header["header_index"]),
                "frame_offset": frame_offset,
                "frame_type": int(header["frame_type"]),
                "show_existing_frame": existing,
                "show_frame": bool(syntax_header.get("show_frame", False)),
                "showable_frame": bool(syntax_header.get("showable_frame", False)),
                "dav1d_show_frame": bool(header.get("show_frame", False)),
                "dav1d_showable_frame": bool(header.get("showable_frame", False)),
                "refresh_frame_flags": int(syntax_header.get("refresh_frame_flags", 0)),
                "syntax_frame_header": syntax_header,
                "dav1d_frame_header": header,
                "references_before": header.get("references_before"),
                "block_count": len(block_by_key[(pts, frame_offset)]),
                "decoded_frame_index": None,
                "display_index": None,
            }
            if existing:
                event["display_reason"] = "show_existing"
                if not any(int(item["pts"]) == pts for item in show_existing):
                    raise RuntimeError("show-existing header has no queued display event")
            else:
                if int(header["header_index"]) not in submit_by_header:
                    raise RuntimeError("decoded frame header has no submit event")
                if not exit_by_key[(pts, frame_offset)]:
                    raise RuntimeError("decoded frame header has no decode-exit event")
                event["decoded_frame_index"] = decoded_index
                event["display_reason"] = "show_frame" if event["show_frame"] else "hidden"
                decoded_index += 1
            events.append(event)
        display = output_by_pts.get(pts)
        if display is None:
            raise RuntimeError(f"sample {sample_index} has no displayed output")
        visible = [event for event in events if event["show_frame"]]
        if len(visible) != 1:
            raise RuntimeError(f"sample {sample_index} has {len(visible)} visible syntax frames")
        visible[0]["display_index"] = int(display["display_index"])
        identities.append(
            {
                "sample_index": sample_index,
                "pts": pts,
                "duration_num": int(sample["duration"]),
                "duration_den": int(track["timescale"]),
                "pts_num": pts,
                "pts_den": int(track["timescale"]),
                "sample_offset": int(sample["offset"]),
                "sample_length": int(sample["length"]),
                "sample_sha256": sample["sha256"],
                "sync": bool(sample["sync"]),
                "frames": events,
            }
        )
    if decoded_index != len(submits):
        raise RuntimeError("decoded-frame index cardinality mismatch")
    histogram = collections.Counter()
    per_frame = collections.defaultdict(collections.Counter)
    for block in blocks:
        block_kind = block["block_kind"]
        key = [str(block_kind)]
        if block_kind == "leaf":
            key.extend([str(block["block_size"]), str(block["partition"])])
        else:
            key.extend([str(block["block_level"]), str(block["partition"])])
        label = ":".join(key)
        histogram[label] += 1
        per_frame[str(block["frame_offset"])][label] += 1
    return identities, {
        "decoded_frame_count": len(submits),
        "display_count": len(displays),
        "show_existing_count": len(show_existing),
        "hidden_decoded_count": sum(
            1 for header in headers if not header["show_existing_frame"] and not header["show_frame"]
        ),
        "block_count": len(blocks),
        "block_class_histogram": dict(sorted(histogram.items())),
        "block_class_histogram_by_frame_offset": {
            key: dict(sorted(value.items())) for key, value in sorted(per_frame.items())
        },
    }


def rust_source_comparison(first_sample: dict[str, object]) -> dict[str, object]:
    files = {
        "entropy": ROOT / "src" / "codecs" / "avif" / "av1" / "entropy.rs",
        "frame": ROOT / "src" / "codecs" / "avif" / "av1" / "frame.rs",
        "av1_module": ROOT / "src" / "codecs" / "avif" / "av1" / "mod.rs",
        "decode": ROOT / "src" / "codecs" / "avif" / "decode.rs",
    }
    anchors = {
        "entropy": ("validate_complete_lossy_420_partition", "decode_complete"),
        "frame": ("decode_complete", "complete_show_existing", "selected_display"),
        "av1_module": ("selected_display_for_temporal_unit_with_token",),
        "decode": ("decode_sequence", "decode_portable"),
    }
    result = {}
    for name, path in files.items():
        text = path.read_text()
        result[name] = {
            "path": path.relative_to(ROOT).as_posix(),
            "sha256": digest_file(path),
            "contains": list(anchors[name]),
            "status": "source_only_no_target_execution",
        }
        missing = [needle for needle in anchors[name] if needle not in text]
        if missing:
            raise RuntimeError(f"Rust source comparison anchor missing: {name}: {missing}")
    result["interpretation"] = (
        "The pinned decoder trace records persistent references and block/lifecycle "
        "events for an I420 8-bit sequence. It does not prove that the Rust "
        "validate_complete_lossy_420_partition admission predicate is the causal "
        "gap; target execution is intentionally deferred."
    )
    first_frame = first_sample["frames"][0]
    result["trace_predicate_context"] = {
        "sample_index": first_sample["sample_index"],
        "dimensions": [
            first_frame["syntax_frame_header"].get("frame_width"),
            first_frame["syntax_frame_header"].get("frame_height"),
        ],
        "frame_type": first_frame["syntax_frame_header"].get("frame_type"),
        "layout": "I420",
        "bit_depth": 8,
        "block_count": first_frame["block_count"],
        "status": "observation_only_source_comparison",
    }
    return result


def collect_role(
    role: str,
    track: dict[str, object],
    syntax_samples: list[dict[str, object]],
    avif_data: bytes,
    dav1d_source: Path,
    work: Path,
    bundle: Path,
    meson: Path,
    ninja: Path,
    env: dict[str, str],
) -> dict[str, object]:
    geometry = sequence_geometry(syntax_samples)
    ivf_relative = Path("inputs") / f"{role}.ivf"
    ivf_path = bundle / ivf_relative
    ivf_path.parent.mkdir(parents=True, exist_ok=True)
    ivf = make_ivf(
        avif_data,
        list(track["samples"]),
        int(geometry["width"]),
        int(geometry["height"]),
        ivf_path,
    )
    plain_records, trace_records, build_info = build_and_trace(
        dav1d_source,
        work,
        ivf_path,
        int(geometry["width"]),
        int(geometry["height"]),
        meson,
        ninja,
        env,
        role,
    )
    del plain_records
    trace_events = [record for record in trace_records if record["kind"] == "output"]
    trace_relative = Path("traces") / f"{role}.jsonl"
    trace_path = bundle / trace_relative
    trace_path.parent.mkdir(parents=True, exist_ok=True)
    trace_path.write_bytes(
        b"".join(json_bytes(record) for record in trace_records)
    )
    trace_yuv = work / f"{role}-trace.yuv"
    display_reports = output_frames(bundle, role, trace_yuv, trace_events)
    identity, counts = summarize_identity(
        track, syntax_samples, trace_records, display_reports
    )
    return {
        "handler": track["handler"],
        "track_id": int(track["track_id"]),
        "timescale": int(track["timescale"]),
        "av1c": track["av1c"],
        "geometry": geometry,
        "ivf": {**ivf, "path": ivf_relative.as_posix()},
        "trace_path": trace_relative.as_posix(),
        "trace_sha256": digest_file(trace_path),
        "trace_event_count": len(trace_records),
        "display_frames": display_reports,
        "identity": identity,
        "counts": counts,
        "noninterference": {
            "plain_yuv_sha256": build_info["plain_yuv_sha256"],
            "trace_yuv_sha256": build_info["trace_yuv_sha256"],
            "byte_equal": build_info["plain_yuv_sha256"] == build_info["trace_yuv_sha256"],
            "bytes": build_info["yuv_bytes"],
        },
        "build": {
            "plain": build_info["plain_build"],
            "instrumented": build_info["trace_build"],
        },
        "patch": {
            "path": (Path("provenance") / Path(build_info["patch_path"]).name).as_posix(),
            "bytes": build_info["patch_bytes"],
            "sha256": build_info["patch_sha256"],
        },
        "msac_detail": {
            "status": "not_collected",
            "reason": "High-level lifecycle/block evidence is complete; detailed MSAC "
            "instrumentation is deferred until a relevant unsupported block class is isolated.",
        },
    }


def build_bundle(
    bundle: Path,
    work: Path,
    source: Path,
    meson: Path,
    ninja: Path,
    env: dict[str, str],
) -> None:
    if not FIXTURE.is_file():
        raise RuntimeError(f"missing animated fixture: {FIXTURE}")
    avif_data = FIXTURE.read_bytes()
    container = inspect_avif(FIXTURE)
    syntax = inspect_av1(FIXTURE)
    write_json(bundle / "syntax" / "avif-container.json", container)
    write_json(bundle / "syntax" / "av1-obus.json", syntax)
    primary = track_report(container, "pict", container.get("color_track_id"))
    alpha = track_report(container, "auxv", container.get("alpha_track_id"))
    if primary is None:
        raise RuntimeError("animated fixture has no primary pict track")
    if len(primary["samples"]) != 5:
        raise RuntimeError("animated fixture must contain exactly five primary samples")
    roles = [("color", primary)]
    if alpha is not None:
        roles.append(("alpha", alpha))
    role_reports = {}
    for role, track in roles:
        track_id = int(track["track_id"])
        syntax_samples = syntax_samples_for_track(syntax, track["handler"], track_id)
        if len(syntax_samples) != len(track["samples"]):
            raise RuntimeError(f"{role} syntax/sample count mismatch")
        role_reports[role] = collect_role(
            role,
            track,
            syntax_samples,
            avif_data,
            source,
            work,
            bundle,
            meson,
            ninja,
            env,
        )
    if alpha is None:
        role_reports["alpha"] = {
            "status": "not_applicable",
            "reason": "animated.avif has no auxiliary alpha track",
        }
    pillow = pillow_report(avif_data)
    if pillow["frame_count"] != 5 or pillow["mode"] != "RGB" or pillow["size"] != [150, 150]:
        raise RuntimeError("Pillow animated fixture observation changed unexpectedly")
    source_verify_before = {
        "commit": text_output(run(["git", "-C", str(source), "rev-parse", "HEAD"])),
        "tree_sha256": git_tree_digest(source),
        "copying_sha256": digest_file(source / "COPYING"),
        "clean": True,
    }
    if source_verify_before["commit"] != DAV1D_COMMIT:
        raise RuntimeError("source commit changed during sequence collection")
    if source_verify_before["copying_sha256"] != COPYING_SHA256:
        raise RuntimeError("pinned dav1d COPYING hash changed")
    verify_source(source)
    source_verify_after = {
        "commit": text_output(run(["git", "-C", str(source), "rev-parse", "HEAD"])),
        "tree_sha256": git_tree_digest(source),
        "copying_sha256": digest_file(source / "COPYING"),
        "clean": True,
    }
    source_anchors = source_anchor_report(source)
    color_first_sample = role_reports["color"]["identity"][0]
    index = {
        "schema": SCHEMA,
        "fixture": {
            "path": FIXTURE.relative_to(ROOT).as_posix(),
            "bytes": len(avif_data),
            "sha256": sha256(avif_data),
        },
        "pillow": pillow,
        "container": {
            "color_track_id": container.get("color_track_id"),
            "alpha_track_id": container.get("alpha_track_id"),
            "sha256": container["sha256"],
            "tracks": [
                {
                    "track_id": track["track_id"],
                    "handler": track["handler"],
                    "timescale": track["timescale"],
                    "av1c": track["av1c"],
                    "samples": track["samples"],
                }
                for track in container.get("tracks", [])
            ],
        },
        "identity": {
            "sample_index": "BMFF track sample order and IVF PTS index",
            "decoded_frame_index": "actual dav1d frame_submitted order; excludes show-existing",
            "display_index": "actual dav1d picture_output order, associated by unique PTS",
            "show_existing": "actual dav1d show_existing_queued event; no tile decode expected",
            "hidden_frames": "actual dav1d frame_header/frame_submitted with show_frame false",
        },
        "roles": role_reports,
        "source_provenance": {
            "dav1d_commit": DAV1D_COMMIT,
            "dav1d_source_tree_sha256": source_verify_before["tree_sha256"],
            "copying_path": "COPYING",
            "copying_sha256": source_verify_before["copying_sha256"],
            "source_checkout": "external clean checkout; invocation path intentionally omitted",
            "verify_before": source_verify_before,
            "verify_after": source_verify_after,
            "anchors": source_anchors,
        },
        "rust_source_comparison": rust_source_comparison(color_first_sample),
        "trace_scope": {
            "state_provenance": "instrumented pinned dav1d scalar decoder",
            "syntax_provenance": "independent inspect_avif_bitstreams and inspect_av1_obus",
            "msac_detail": "not_collected",
            "target_rust_execution": "deferred_by_slice_contract",
        },
        "artifacts": [],
    }
    write_json(bundle / "index.json", index)
    index["artifacts"] = artifact_records(bundle)
    write_json(bundle / "index.json", index)


def publish(bundle: Path) -> str:
    candidate = relative_artifacts(bundle)
    index = (bundle / "index.json").read_bytes()
    if OUTPUT.exists():
        if not OUTPUT.is_dir():
            raise RuntimeError("existing sequence output is not a directory")
        existing = relative_artifacts(OUTPUT)
        existing_index = (OUTPUT / "index.json").read_bytes()
        if existing == candidate and existing_index == index:
            shutil.rmtree(bundle)
            return "identical-no-op"
        shutil.rmtree(bundle)
        differing = sorted(
            name
            for name in set(existing) | set(candidate)
            if existing.get(name) != candidate.get(name)
        )
        if existing_index != index:
            differing.append("index.json")
        raise RuntimeError(
            "existing sequence output differs; refusing replacement ("
            + ", ".join(differing)
            + ")"
        )
    bundle.replace(OUTPUT)
    return "published"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dav1d-source", type=Path, required=True)
    parser.add_argument("--meson", default="meson")
    parser.add_argument("--ninja", default="ninja")
    parser.add_argument(
        "--python-path",
        type=Path,
        help="Optional site-packages path for an isolated Meson installation",
    )
    args = parser.parse_args()
    output_guard()
    source = args.dav1d_source.resolve()
    verify_source(source)
    meson = resolve_tool(args.meson, "Meson")
    ninja = resolve_tool(args.ninja, "Ninja")
    env = tool_environment(meson, ninja, args.python_path)
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    if OUTPUT.exists() and not OUTPUT.is_dir():
        raise RuntimeError("existing sequence output is not a directory")
    temp_root = Path(
        tempfile.mkdtemp(prefix=".animated-sequence-", dir=str(OUTPUT.parent))
    )
    bundle = temp_root / "animated"
    bundle.mkdir()
    try:
        build_bundle(bundle, temp_root, source, meson, ninja, env)
        outcome = publish(bundle)
    except Exception:
        if temp_root.exists():
            shutil.rmtree(temp_root)
        raise
    if temp_root.exists():
        shutil.rmtree(temp_root)
    print(json.dumps({"output": "target/oracle-staging/av1-sequence/animated", "result": outcome}, sort_keys=True))


if __name__ == "__main__":
    main()
