#!/usr/bin/env python3
"""Observe short-reference selection while pinned dav1d decodes complete AVIFs.

Only ignored staging output is written. The six mutations rebuild one pinned
frame header, preserving its complete tile bytes and repairing container sizes.
Rust is never executed. Native selected indices, not identical reference pixels,
are the authority for the private reference-selection regression.
"""

from __future__ import annotations

import argparse
import json
import shutil
import struct
import tempfile
from pathlib import Path

from PIL import Image, _avif, _imaging, features

from generate_av1_sequence_refs import (
    COPYING_SHA256, DAV1D_COMMIT, artifact_records, build_dav1d, copy_source,
    digest_file, git_tree_digest, make_ivf, pillow_report, replace_once,
    resolve_tool, run, run_decoder, sha256, tool_environment,
    verify_source, write_json,
)
from inspect_av1_obus import inspect as inspect_av1
from inspect_avif_bitstreams import children, inspect as inspect_avif, parse_boxes, unique_box

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "tests/fixtures/input/images/avif/animated_error_resilient.avif"
SOURCE_SHA = "06ea9771f8b46c3432c6c6cdf324f1c05e86a5fdccd774c8e3c9a8fce0b831f0"
CASES = (
    ("past_distinct", 1, 0, 6),
    ("equal_distinct", 0, 0, 6),
    ("wrapped_future_distinct", 127, 0, 6),
    ("past_duplicate", 1, 7, 7),
    ("equal_duplicate", 0, 7, 7),
    ("wrapped_future_duplicate", 127, 7, 7),
)


def instrument(source: Path) -> None:
    anchor = "        for (int i = 0; i < 7; i++) {\n            if (!hdr->frame_ref_short_signaling)"
    trace = r'''        if (hdr->frame_ref_short_signaling) {
            printf("@LIFECYCLE {\"event\":\"short_references\",\"order_hint_bits\":%u,"
                   "\"order_hint\":%u,\"last\":%d,\"golden\":%d,\"reference_hints\":[",
                   seqhdr->order_hint_n_bits, hdr->frame_offset, hdr->refidx[0], hdr->refidx[3]);
            for (int j = 0; j < 8; j++) {
                if (j) putchar(',');
                printf("%u", c->refs[j].p.p.frame_hdr->frame_offset);
            }
            fputs("],\"distances\":[", stdout);
            for (int j = 0; j < 8; j++) {
                if (j) putchar(',');
                printf("%d", get_poc_diff(seqhdr->order_hint_n_bits,
                                         c->refs[j].p.p.frame_hdr->frame_offset, hdr->frame_offset));
            }
            fputs("],\"selected\":[", stdout);
            for (int j = 0; j < 7; j++) {
                if (j) putchar(',');
                printf("%d", hdr->refidx[j]);
            }
            puts("]}");
        }
'''
    replace_once(source / "src/obu.c", anchor, trace + anchor)


def bits(value: int, width: int) -> str:
    if value < 0 or value >= 1 << width:
        raise RuntimeError("mutation value does not fit its field")
    return f"{value:0{width}b}"


def mutate(data: bytes, order_hint: int, last: int, golden: int) -> bytes:
    if len(data) != 1084 or sha256(data) != SOURCE_SHA:
        raise RuntimeError("pinned complete source changed")
    top = parse_boxes(data, 0, len(data))
    mdat = unique_box(top, b"mdat")
    if mdat is None or (mdat.start, mdat.size, mdat.end) != (989, 95, 1084):
        raise RuntimeError("mutation requires the pinned final mdat")
    parent = unique_box(top, b"moov")
    for kind in (b"trak", b"mdia", b"minf", b"stbl", b"stsz"):
        if parent is None:
            raise RuntimeError("pinned track box is absent")
        parent = unique_box(children(data, parent), kind)
    if parent is None or parent.start != 921 or parent.size != 28:
        raise RuntimeError("pinned stsz shape changed")
    if struct.unpack_from(">6I", data, parent.start + 4) != (0x7374737A, 0, 0, 2, 41, 46):
        raise RuntimeError("pinned stsz fields changed")
    if data[1040:1042] != bytes((0x32, 42)):
        raise RuntimeError("pinned frame OBU moved")
    payload = "".join(bits(byte, 8) for byte in data[1042:1084])
    if payload[94] != "0" or len(payload) != 336:
        raise RuntimeError("pinned explicit-reference header changed")
    pairs = [payload[95 + i * 17:112 + i * 17] for i in range(7)]
    if [int(pair[:3], 2) for pair in pairs] != [0, 1, 7, 6, 7, 7, 0]:
        raise RuntimeError("pinned explicit reference indices changed")
    if any(int(pair[3:], 2) for pair in pairs):
        raise RuntimeError("pinned reference deltas changed")
    header = payload[:23] + bits(order_hint, 7) + payload[30:94]
    header += "1" + bits(last, 3) + bits(golden, 3) + "".join(pair[3:] for pair in pairs)
    header += payload[214:291]
    if len(header) != 276 or any(bit != "0" for bit in payload[291:296]):
        raise RuntimeError("pinned header alignment changed")
    header += "0" * (-len(header) % 8)
    new_payload = bytes(int(header[i:i + 8], 2) for i in range(0, len(header), 8)) + data[1079:1084]
    if len(new_payload) != 40:
        raise RuntimeError("short-reference payload length differs")
    result = bytearray(data[:1040] + bytes((0x32, 40)) + new_payload)
    struct.pack_into(">I", result, mdat.start, 93)
    struct.pack_into(">I", result, parent.start + 24, 44)
    return bytes(result)


def collect_case(case, data, bundle, work, binaries, env, verify_syntax):
    name, order_hint, last, golden = case
    root = bundle / name
    root.mkdir()
    encoded = mutate(data, order_hint, last, golden)
    (root / "input.avif").write_bytes(encoded)
    container = inspect_avif(root / "input.avif")
    track, = container["tracks"]
    if [(s["offset"], s["length"]) for s in track["samples"]] != [(997, 41), (1038, 44)]:
        raise RuntimeError("mutation changed an unrelated sample boundary")
    if container["items"]["color"][0]["sha256"] != sha256(data[997:1038]):
        raise RuntimeError("mutation changed the independent primary image")
    write_json(root / "container.json", container)
    ivf = root / "color.ivf"
    make_ivf(encoded, track["samples"], 16, 16, ivf)
    observations = {}
    for kind, (binary, build) in binaries.items():
        repeats = []
        yuv = work / f"{name}-{kind}.yuv"
        for _ in range(2):
            events, stdout, stderr = run_decoder(binary, build, ivf, yuv, env)
            if stderr or (kind == "plain" and stdout) or len(yuv.read_bytes()) != 768:
                raise RuntimeError(f"native {name}/{kind} did not decode two complete frames")
            repeats.append((events, yuv.read_bytes()))
        if repeats[0] != repeats[1]:
            raise RuntimeError("native observation changed on repeat")
        observations[kind] = repeats[0]
    if observations["plain"][1] != observations["trace"][1]:
        raise RuntimeError("instrumentation changed native pixels")
    events = observations["trace"][0]
    if len(events) != 1 or events[0]["event"] != "short_references":
        raise RuntimeError("expected exactly one native short-reference event")
    event = events[0]
    if (event["order_hint_bits"], event["order_hint"], event["last"], event["golden"]) != (7, order_hint, last, golden):
        raise RuntimeError("native header observation differs from the input mutation")
    (root / "display.yuv").write_bytes(observations["plain"][1])
    write_json(root / "native.json", event)
    pillow = pillow_report(encoded, root)
    if pillow != pillow_report(encoded, root) or pillow["frame_count"] != 2 or pillow["size"] != [16, 16]:
        raise RuntimeError("Pillow did not repeat two complete frames")
    if verify_syntax:
        syntax = inspect_av1(root / "input.avif")
        sample = next(s for s in syntax["samples"] if s["role"] == "track_pict" and s["identity"]["sample"] == 1)
        header = next(obu["frame_header"] for obu in sample["obus"] if "frame_header" in obu)
        if header["reference_indices"] != event["selected"] or not header["frame_refs_short_signaling"]:
            raise RuntimeError("corrected syntax inspector differs from native selection")
        write_json(root / "syntax.json", syntax)
    return {"name": name, "input_path": f"{name}/input.avif", "input_bytes": len(encoded),
            "input_sha256": sha256(encoded), "native": event, "pillow": pillow,
            "native_yuv_bytes": 768, "native_yuv_sha256": sha256(observations["plain"][1]),
            "native_noninterference": True, "native_repeat_equal": True, "pillow_repeat_equal": True,
            "syntax_matches_native": verify_syntax}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dav1d-source", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--meson", default="meson")
    parser.add_argument("--ninja", default="ninja")
    parser.add_argument("--verify-syntax", action="store_true")
    args = parser.parse_args()
    output = args.output.resolve()
    if (ROOT / "target/oracle-staging").resolve() not in output.parents or output.exists():
        raise RuntimeError("output must be a fresh directory below target/oracle-staging")
    if (Image.__version__, features.version("avif"), _avif.codec_versions()) != (
        "12.2.0", "1.4.1", "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2"
    ):
        raise RuntimeError("native Pillow versions differ from pins")
    source = args.dav1d_source.resolve()
    verify_source(source)
    if digest_file(source / "COPYING") != COPYING_SHA256:
        raise RuntimeError("pinned native license changed")
    meson, ninja = resolve_tool(args.meson, "Meson"), resolve_tool(args.ninja, "Ninja")
    env = tool_environment(meson, ninja, None)
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".short-reference-", dir=output.parent) as temporary:
        work = Path(temporary)
        bundle = work / "bundle"
        bundle.mkdir()
        binaries, builds = {}, {}
        for kind in ("plain", "trace"):
            checkout, build = work / f"{kind}-source", work / f"{kind}-build"
            copy_source(source, checkout)
            if kind == "trace":
                instrument(checkout)
                patch = run(["git", "-C", str(checkout), "diff", "--binary"]).stdout
                (bundle / "native-trace.patch").write_bytes(patch)
            binary, info = build_dav1d(checkout, build, meson, ninja, env)
            binaries[kind], builds[kind] = (binary, build), {**info, "binary_sha256": digest_file(binary)}
        data = SOURCE.read_bytes()
        cases = [collect_case(case, data, bundle, work, binaries, env, args.verify_syntax) for case in CASES]
        index = {"schema": "image-slash-star/av1-short-references-oracle@1",
                 "source_fixture": {"path": str(SOURCE.relative_to(ROOT)), "bytes": len(data), "sha256": sha256(data)},
                 "source": {"commit": DAV1D_COMMIT, "tree_sha256": git_tree_digest(source), "copying_sha256": COPYING_SHA256,
                            "inspectors": [{"path": f"scripts/{name}", "sha256": digest_file(ROOT / "scripts" / name)}
                                           for name in ("inspect_av1_obus.py", "inspect_avif_bitstreams.py")]},
                 "oracle": {"pillow": Image.__version__, "libavif": features.version("avif"), "codecs": _avif.codec_versions(),
                            "pillow_avif_sha256": digest_file(Path(_avif.__file__)), "pillow_imaging_sha256": digest_file(Path(_imaging.__file__))},
                 "builds": builds, "cases": cases, "artifacts": artifact_records(bundle),
                 "target_execution": "deferred", "source_boundary": "Complete pinned AVIF header mutations; no Rust output used"}
        verify_source(source)
        write_json(bundle / "index.json", index)
        bundle.replace(output)
    print(json.dumps({"output": str(output.relative_to(ROOT)), "cases": len(cases)}, sort_keys=True))


if __name__ == "__main__":
    main()
