#!/usr/bin/env python3
"""Collect full-file libaom encoder traces and independent native replays.

This development-only collector builds clean pinned native sources. It never
executes Rust, installs native runtime dependencies, or changes matrix status.
Output must be a fresh directory beneath target/oracle-staging.
"""

from __future__ import annotations

import argparse
import collections
import difflib
import gzip
import hashlib
import json
import os
import shutil
import struct
import tempfile
from pathlib import Path

import yaml
from PIL import Image, _avif

import generate_decode_refs as oracle
from generate_av1_sequence_refs import (
    LIBAVIF_COMMIT, artifact_records, digest_file, git_tree_digest, replace_once,
    run, text_output, write_json,
)
from inspect_av1_obus import inspect

ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "scripts/av1_encoder_oracle"
AOM_COMMIT = "ad44980d7f3c7a2605c25d51ea96946949000841"
AOM_OPTIONS = ["-DCMAKE_BUILD_TYPE=Release", "-DAOM_TARGET_CPU=generic",
               "-DENABLE_TESTS=OFF", "-DENABLE_DOCS=OFF", "-DENABLE_TOOLS=OFF",
               "-DENABLE_EXAMPLES=OFF", "-DCONFIG_AV1_DECODER=0", "-DCONFIG_MULTITHREAD=0"]
AVIF_OPTIONS = ["-DCMAKE_BUILD_TYPE=Release", "-DBUILD_SHARED_LIBS=OFF",
                "-DAVIF_CODEC_AOM=SYSTEM", "-DAVIF_CODEC_AOM_DECODE=OFF",
                "-DAVIF_CODEC_DAV1D=OFF", "-DAVIF_LIBYUV=OFF",
                "-DAVIF_BUILD_APPS=OFF", "-DAVIF_BUILD_TESTS=OFF"]
# name, width, height, depth, AVIF pixel format, alpha, quality, speed, column log2
CASES = [
    ("mono_tiny", 2, 3, 8, 4, 0, 75, 6, 0),
    ("mono_lossless", 8, 8, 8, 4, 0, 100, 6, 0),
    ("yuv420", 17, 13, 8, 3, 0, 75, 6, 0),
    ("yuv422", 17, 13, 8, 2, 0, 75, 6, 0),
    ("yuv444", 8, 8, 8, 1, 0, 75, 6, 0),
    ("high10_420", 9, 7, 10, 3, 0, 75, 6, 0),
    ("high12_422", 9, 7, 12, 2, 0, 75, 6, 0),
    ("high12_444", 9, 7, 12, 1, 0, 75, 6, 0),
    ("alpha", 8, 8, 8, 1, 1, 75, 6, 0),
    ("two_tiles", 128, 64, 8, 4, 0, 75, 6, 1),
]


def pinned(source, commit, name, files):
    if text_output(run(["git", "-C", str(source), "rev-parse", "HEAD"])) != commit:
        raise RuntimeError(f"wrong {name} source revision")
    if run(["git", "-C", str(source), "status", "--porcelain"]).stdout:
        raise RuntimeError(f"dirty {name} source")
    licenses = ["LICENSE", "PATENTS"] if name == "libaom" else ["LICENSE"]
    for filename in licenses:
        if digest_file(source / filename) != digest_file(ROOT / "third_party" / name / filename):
            raise RuntimeError(f"retained {name} {filename} differs")
    return {"commit": commit, "tree_sha256": git_tree_digest(source),
            "files": [{"path": f, "sha256": digest_file(source / f)} for f in files + licenses]}


def instrument(source):
    destination = source / "aom_dsp"
    shutil.copyfile(ASSETS / "trace.h", destination / "oracle_trace.h")
    shutil.copyfile(ASSETS / "trace.c", destination / "oracle_trace.c")
    header = destination / "bitwriter.h"
    implementation = destination / "bitwriter.c"
    replace_once(header, '#include "aom_dsp/entenc.h"',
                 '#include "aom_dsp/entenc.h"\n#include "oracle_trace.h"')
    replace_once(header, "  uint8_t allow_update_cdf;", "  uint8_t allow_update_cdf;\n"
                 "  unsigned oracle_id;\n  int oracle_in_symbol;")
    replace_once(header, "  od_ec_encode_bool_q15(&w->ec, bit, p);",
                 '  oracle_before(w->oracle_id, "bool", &w->ec, bit, p, NULL, 0, 0);\n'
                 "  od_ec_encode_bool_q15(&w->ec, bit, p);\n  oracle_after(&w->ec, NULL, 0);")
    replace_once(header, "  od_ec_encode_cdf_q15(&w->ec, symb, cdf, nsymbs);",
                 '  if (!w->oracle_in_symbol) oracle_before(w->oracle_id, "cdf", &w->ec, '
                 "symb, 0, cdf, nsymbs, 0);\n"
                 "  od_ec_encode_cdf_q15(&w->ec, symb, cdf, nsymbs);\n"
                 "  if (!w->oracle_in_symbol) oracle_after(&w->ec, cdf, nsymbs);")
    replace_once(header, "  aom_write_cdf(w, symb, cdf, nsymbs);\n"
                 "  if (w->allow_update_cdf) update_cdf(cdf, symb, nsymbs);",
                 '  oracle_before(w->oracle_id, "symbol", &w->ec, symb, 0, cdf, nsymbs + 1, w->allow_update_cdf);\n'
                 "  w->oracle_in_symbol = 1;\n  aom_write_cdf(w, symb, cdf, nsymbs);\n"
                 "  w->oracle_in_symbol = 0;\n"
                 "  if (w->allow_update_cdf) update_cdf(cdf, symb, nsymbs);\n"
                 "  oracle_after(&w->ec, cdf, nsymbs + 1);")
    replace_once(implementation, '#include "aom_dsp/bitwriter.h"',
                 '#include "aom_dsp/bitwriter.h"\n#include "oracle_trace.c"\n'
                 "static unsigned oracle_next_id;")
    replace_once(implementation, "  uint32_t bytes;", "  uint32_t bytes = 0;")
    replace_once(implementation, "  od_ec_enc_init(&w->ec, 62025);",
                 "  od_ec_enc_init(&w->ec, 62025);\n"
                 "  w->oracle_id = ++oracle_next_id;\n  w->oracle_in_symbol = 0;\n"
                 "  oracle_start(w->oracle_id, &w->ec);")
    replace_once(implementation, "  data = od_ec_enc_done(&w->ec, &bytes);",
                 "  oracle_finish_before(w->oracle_id, &w->ec);\n"
                 "  data = od_ec_enc_done(&w->ec, &bytes);\n"
                 "  oracle_finish_after(&w->ec, data, bytes);")


def build(args, work, bundle):
    trace_source = work / "aom-trace-source"
    shutil.copytree(args.aom_source, trace_source, ignore=shutil.ignore_patterns(".git"))
    instrument(trace_source)
    patch = bytearray()
    for name in ("bitwriter.h", "bitwriter.c"):
        # A portable patch includes only source-relative paths.
        patch.extend("".join(difflib.unified_diff(
            (args.aom_source / "aom_dsp" / name).read_text().splitlines(True),
            (trace_source / "aom_dsp" / name).read_text().splitlines(True),
            fromfile=f"a/{name}", tofile=f"b/{name}", n=0)).encode())
    (bundle / "instrumentation.patch").write_bytes(patch)
    binaries = {}
    for label, source in (("plain", args.aom_source), ("trace", trace_source)):
        directory = work / f"aom-{label}"
        prefix = work / f"prefix-{label}"
        command = [args.cmake, "-S", str(source), "-B", str(directory), "-G", "Ninja",
                   *AOM_OPTIONS, f"-DCMAKE_C_COMPILER={args.cc}", f"-DCMAKE_CXX_COMPILER={args.cxx}",
                   f"-DCMAKE_INSTALL_PREFIX={prefix}"]
        run(command)
        run([args.cmake, "--build", str(directory), "-j", "4"])
        run([args.cmake, "--install", str(directory)])
        (bundle / f"aom-{label}-config.h").write_bytes((directory / "config/aom_config.h").read_bytes())
        binaries[label] = directory / "libaom.a"
    avif_build = work / "avif"
    run([args.cmake, "-S", str(args.libavif_source), "-B", str(avif_build), "-G", "Ninja",
         *AVIF_OPTIONS, f"-DCMAKE_C_COMPILER={args.cc}",
         f"-DCMAKE_PREFIX_PATH={work / 'prefix-plain'}", f"-DAOM_LIBRARY={binaries['plain']}",
         f"-DAOM_INCLUDE_DIR={args.aom_source}"])
    run([args.cmake, "--build", str(avif_build), "-j", "4"])
    for label in ("plain", "trace"):
        target = work / f"encode-{label}"
        run([args.cc, "-std=c99", "-O2", "-Wall", "-Wextra", "-Werror",
             f"-I{args.libavif_source / 'include'}", str(ASSETS / "encode.c"),
             str(avif_build / "libavif.a"), str(binaries[label]), "-lm", "-o", str(target)])
        binaries[f"encode-{label}"] = target
    utility = work / "utility"
    utility.mkdir()
    shutil.copyfile(ASSETS / "trace.h", utility / "oracle_trace.h")
    replay = work / "replay"
    run([args.cc, "-std=c99", "-O2", "-Wall", "-Wextra", "-Werror",
         f"-I{utility}", f"-I{args.aom_source}", f"-I{work / 'aom-plain'}",
         str(ASSETS / "replay.c"), str(ASSETS / "trace.c"), str(binaries["plain"]),
         "-lm", "-o", str(replay)])
    binaries["replay"] = replay
    identities = {name: digest_file(path) for name, path in binaries.items()}
    identities["libavif.a"] = digest_file(avif_build / "libavif.a")
    return binaries, identities


def read_trace(path):
    if path.stat().st_size > 40 * 1024 * 1024:
        raise RuntimeError("trace exceeds collector limit")
    records = [json.loads(line) for line in path.read_text().splitlines()]
    streams = {}
    active = None
    for record in records:
        identity = record["writer"]
        if record["kind"] == "start":
            if active is not None or identity in streams:
                raise RuntimeError("overlapping/reused writer lifetime")
            active = identity
            streams[identity] = []
        if active != identity:
            raise RuntimeError("operation outside writer lifetime")
        previous = streams[identity]
        if previous and previous[-1]["after"] != record["before"]:
            raise RuntimeError("unobserved mutation between entropy operations")
        previous.append(record)
        if record["kind"] == "finish":
            if len(bytes.fromhex(record["bytes"])) != record["length"]:
                raise RuntimeError("final byte count mismatch")
            active = None
    if active is not None or not streams:
        raise RuntimeError("unterminated/missing native writer")
    return records, streams


def replay_input(records):
    lines = []
    for record in records:
        kind = record["kind"]
        if kind == "start": lines.append(f"S {record['writer']}")
        elif kind == "finish": lines.append("F")
        elif kind == "bool": lines.append(f"B {record['value']} {record['probability']}")
        elif kind in ("cdf", "symbol"):
            values = record["cdf_before"]
            symbols = len(values) - (kind == "symbol")
            lines.append(f"{'A' if kind == 'symbol' else 'C'} {record['value']} {symbols} "
                         f"{record['update']} " + " ".join(map(str, values)))
        else: raise RuntimeError(f"unknown trace operation: {kind}")
    return ("\n".join(lines) + "\n").encode()


def replay(binaries, directory, records):
    inputs = directory / "replay.txt"
    inputs.write_bytes(replay_input(records))
    observed = directory / "replayed.jsonl"
    run([str(binaries["replay"]), str(inputs)],
        env={**os.environ, "IMAGE_SLASH_STAR_ENCODER_TRACE": str(observed)})
    repeated, _ = read_trace(observed)
    if repeated != records:
        mismatch = next((i for i, (a, b) in enumerate(zip(records, repeated)) if a != b), None)
        raise RuntimeError(f"unmodified native replay differs at {mismatch}")


def planes(case):
    name, width, height, depth, fmt, alpha, *_ = case
    descriptions, output = [], bytearray()
    for channel in range(4):
        if (channel in (1, 2) and fmt == 4) or (channel == 3 and not alpha): continue
        w = (width + 1) // 2 if channel in (1, 2) and fmt in (2, 3) else width
        h = (height + 1) // 2 if channel in (1, 2) and fmt == 3 else height
        descriptions.append({"channel": channel, "width": w, "height": h, "offset": len(output)})
        for y in range(h):
            for x in range(w):
                # Two flat but distinct tiles avoid enormous per-symbol buffer histories.
                value = (47 if x < 64 else 193) if name == "two_tiles" else (
                    29 + x * 37 + y * 61 + channel * 73 + x * y * 7) % 256
                value = (value << (depth - 8)) | ((x * 3 + y * 5 + channel) & ((1 << (depth - 8)) - 1))
                output.extend(struct.pack("<H", value))
    return output, descriptions


def bind_tiles(path, streams):
    data = path.read_bytes()
    report = inspect(path)
    bindings = []
    used = set()
    for sample in report["samples"]:
        for obu in sample["obus"]:
            for tile in obu.get("tile_group", {}).get("tiles", []):
                payload = b"".join(data[s["offset"]:s["offset"] + s["length"]]
                                   for s in tile["physical_spans"])
                matches = [identity for identity, events in streams.items()
                           if bytes.fromhex(events[-1]["bytes"]) == payload]
                if len(matches) != 1 or matches[0] in used:
                    raise RuntimeError(f"unresolved tile-to-writer binding: {matches}")
                used.add(matches[0])
                bindings.append({"sample_role": sample["role"], "tile": tile,
                                 "writer": matches[0], "candidate_writers": matches})
    if not bindings:
        raise RuntimeError("no complete committed tiles inspected")
    return report, bindings, sorted(set(streams) - used)


def collect(binaries, bundle, work):
    cases = []
    all_records = []
    for case in CASES:
        name, width, height, depth, fmt, alpha, quality, speed, columns = case
        directory = bundle / name
        directory.mkdir()
        raw, descriptions = planes(case)
        plane_path = directory / "planes.u16le"
        plane_path.write_bytes(raw)
        native = directory / "encoded.avif"
        trace_path = directory / "trace.jsonl"
        settings = list(map(str, case[1:]))
        for repetition in range(2):
            plain = work / f"{name}-plain-{repetition}.avif"
            traced = work / f"{name}-trace-{repetition}.avif"
            trace = work / f"{name}-{repetition}.jsonl"
            run([str(binaries["encode-plain"]), str(plane_path), str(plain), *settings])
            run([str(binaries["encode-trace"]), str(plane_path), str(traced), *settings],
                env={**os.environ, "IMAGE_SLASH_STAR_ENCODER_TRACE": str(trace)})
            if plain.read_bytes() != traced.read_bytes():
                raise RuntimeError(f"instrumentation changes complete AVIF: {name}")
            if repetition == 0:
                shutil.copyfile(plain, native)
                shutil.copyfile(trace, trace_path)
            elif native.read_bytes() != plain.read_bytes() or trace_path.read_bytes() != trace.read_bytes():
                raise RuntimeError(f"native repeat differs: {name}")
        records, streams = read_trace(trace_path)
        replay(binaries, directory, records)
        report, bindings, discarded = bind_tiles(native, streams)
        roles = [s["role"] for s in report["samples"]]
        if roles != (["item_color", "item_alpha"] if alpha else ["item_color"]):
            raise RuntimeError("coded image roles differ from input")
        for sample in report["samples"]:
            headers = [o["sequence_header"] for o in sample["obus"] if "sequence_header" in o]
            if len(headers) != 1:
                raise RuntimeError("expected one sequence header per coded item")
            header = headers[0]
            monochrome = sample["role"] == "item_alpha" or fmt == 4
            sampling = (1, 1) if monochrome else {1: (0, 0), 2: (1, 0), 3: (1, 1)}[fmt]
            expected = {"bit_depth": depth, "max_width": width, "max_height": height,
                        "monochrome": monochrome, "subsampling_x": sampling[0],
                        "subsampling_y": sampling[1], "color_range": 1}
            if any(header[key] != value for key, value in expected.items()):
                raise RuntimeError("native sequence declarations differ from input")
            if sample["role"] == "item_color" and [header[k] for k in
                    ("color_primaries", "transfer_characteristics", "matrix_coefficients")] != [1, 13, 6]:
                raise RuntimeError("native color declaration differs from input")
        if sum(b["sample_role"] == "item_color" for b in bindings) != 1 << columns:
            raise RuntimeError("requested tiling differs from actual color tiles")
        write_json(directory / "syntax.json", report)
        with Image.open(native) as image:
            image.load()
            if image.size != (width, height): raise RuntimeError("roundtrip dimensions differ")
            pixels = image.tobytes()
            (directory / "pixels.bin").write_bytes(pixels)
            mode = image.mode
        with Image.open(native) as image:
            if image.tobytes() != pixels or image.mode != mode: raise RuntimeError("decode repeat differs")
        cases.append({"id": name, "dimensions": [width, height], "depth": depth,
                      "pixel_format": fmt, "alpha": bool(alpha), "quality": quality,
                      "quality_alpha": quality, "speed": speed, "tile_cols_log2": columns,
                      "tile_rows_log2": 0, "auto_tiling": False, "threads": 1,
                      "range": "full", "cicp": [1, 13, 6], "input_planes": descriptions,
                      "decoded_mode": mode, "bindings": bindings, "uncommitted_writers": discarded,
                      "operations": len(records), "native_repetitions": 2,
                      "instrumentation_noninterference": True, "unmodified_replay_equal": True,
                      "scope": "standalone libavif/libaom encode; pinned Pillow/dav1d decode"})
        committed = {binding["writer"] for binding in bindings}
        all_records.extend(record for record in records if record["writer"] in committed)
    boundary_dir = bundle / "boundaries"
    boundary_dir.mkdir()
    boundary_path = boundary_dir / "trace.jsonl"
    run([str(binaries["replay"]), "--boundaries"],
        env={**os.environ, "IMAGE_SLASH_STAR_ENCODER_TRACE": str(boundary_path)})
    boundaries, _ = read_trace(boundary_path)
    replay(binaries, boundary_dir, boundaries)
    return cases, observations(all_records), observations(boundaries)


def observations(records):
    counts = collections.Counter()
    alphabets, probabilities, termination = set(), set(), set()
    for r in records:
        counts[r["kind"]] += 1
        if r["kind"] == "start": continue
        before, after = r["before"], r["after"]
        if after["offset"] > before["offset"]: counts["normalization_flush"] += 1
        old, new = bytes.fromhex(before["buffer"]), bytes.fromhex(after["buffer"])
        if new[:len(old)] != old:
            counts["carry_prefix_change"] += 1
            changed = sum(a != b for a, b in zip(old, new))
            counts["longest_carry_prefix_change"] = max(counts["longest_carry_prefix_change"], changed)
        if r["kind"] == "finish":
            termination.add(r["length"] - before["offset"])
        if r["kind"] == "bool": probabilities.add(r["probability"])
        if r["kind"] in ("cdf", "symbol"):
            cdf = r["cdf_before"]
            n = len(cdf) - (r["kind"] == "symbol")
            alphabets.add(n)
            if any(cdf[i] == cdf[i + 1] for i in range(n - 1)): counts["cdf_plateau"] += 1
            if r["kind"] == "symbol":
                if not r["update"]: counts["adaptation_disabled"] += 1
                elif (cdf[-1], r["cdf_after"][-1]) in ((15, 16), (31, 32), (32, 32)):
                    counts[f"adaptation_{cdf[-1]}_{r['cdf_after'][-1]}"] += 1
    return {"counts": dict(counts), "alphabets": sorted(alphabets),
            "bool_q15_min": min(probabilities, default=None),
            "bool_q15_max": max(probabilities, default=None),
            "termination_appended_bytes": sorted(termination)}


def compress_traces(bundle):
    records = []
    for path in sorted(bundle.rglob("*.jsonl")):
        data = path.read_bytes()
        compressed = path.with_suffix(".jsonl.gz")
        compressed.write_bytes(gzip.compress(data, mtime=0))
        records.append({"path": str(compressed.relative_to(bundle)),
                        "expanded_bytes": len(data),
                        "expanded_sha256": hashlib.sha256(data).hexdigest()})
        path.unlink()
    return records


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--aom-source", required=True, type=Path)
    parser.add_argument("--libavif-source", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--cmake", default="cmake")
    parser.add_argument("--cc", default="clang")
    parser.add_argument("--cxx", default="clang++")
    args = parser.parse_args()
    args.aom_source, args.libavif_source = args.aom_source.resolve(), args.libavif_source.resolve()
    output = args.output.resolve()
    if not output.is_relative_to(ROOT / "target/oracle-staging") or output.exists():
        raise RuntimeError("output must be fresh and beneath target/oracle-staging")
    sources = {
        "libaom": pinned(args.aom_source, AOM_COMMIT, "libaom",
                         ["aom_dsp/entenc.c", "aom_dsp/entenc.h", "aom_dsp/entcode.c",
                          "aom_dsp/bitwriter.c", "aom_dsp/bitwriter.h", "aom_dsp/prob.h"]),
        "libavif": pinned(args.libavif_source, LIBAVIF_COMMIT, "libavif",
                          ["src/codec_aom.c", "src/write.c", "include/avif/avif.h"]),
    }
    manifest = yaml.safe_load(oracle.MANIFEST.read_bytes())
    oracle.verify_primary_oracle(manifest, yaml.safe_load(oracle.ORACLE_LOCK.read_bytes()))
    if _avif.codec_versions() != "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2":
        raise RuntimeError("pinned Pillow native codec identity differs")
    for name in ("cc", "cxx", "cmake"):
        resolved = shutil.which(getattr(args, name))
        if not resolved: raise RuntimeError(f"missing build tool: {name}")
        setattr(args, name, resolved)
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".av1-encoder-", dir=output.parent) as temporary:
        work = Path(temporary)
        bundle = work / "bundle"
        bundle.mkdir()
        print("Building pinned original and instrumented native encoders", flush=True)
        binaries, identities = build(args, work, bundle)
        print("Collecting full-file encodes, tile bindings and native replay", flush=True)
        cases, full_file, boundary = collect(binaries, bundle, work)
        required = ("normalization_flush", "carry_prefix_change", "cdf_plateau",
                    "adaptation_disabled", "adaptation_15_16", "adaptation_31_32", "adaptation_32_32")
        if any(not boundary["counts"].get(name) for name in required) or boundary["alphabets"] != list(range(2, 17)):
            raise RuntimeError(f"native boundaries not actually observed: {boundary}")
        if boundary["bool_q15_min"] != 1 or boundary["bool_q15_max"] != 32767:
            raise RuntimeError("probability endpoints not observed")
        if full_file["counts"].get("longest_carry_prefix_change", 0) < 2:
            raise RuntimeError("committed native tiles did not exercise a multi-byte carry")
        traces = compress_traces(bundle)
        index = {"schema": "image-slash-star/av1-encoder-entropy-oracle@1", "sources": sources,
                 "encoder": "standalone pinned libavif/libaom; not Pillow save() parity",
                 "decoder": {"oracle": oracle.oracle_identity(manifest),
                             "codecs": _avif.codec_versions(), "binary_sha256": digest_file(Path(_avif.__file__))},
                 "build": {"aom_options": AOM_OPTIONS, "avif_options": AVIF_OPTIONS,
                           "binary_sha256": identities,
                           "tools": {name: text_output(run([getattr(args, name), "--version"]))
                                     for name in ("cc", "cxx", "cmake")}},
                 "inputs": [{"path": str(path.relative_to(ROOT)), "sha256": digest_file(path)}
                            for path in [Path(__file__).resolve(), *sorted(ASSETS.iterdir()),
                                         ROOT / "scripts/inspect_av1_obus.py",
                                         ROOT / "scripts/inspect_avif_bitstreams.py",
                                         ROOT / "scripts/generate_decode_refs.py",
                                         ROOT / "scripts/generate_av1_sequence_refs.py",
                                         ROOT / "scripts/generate_av1_reconstruction_refs.py",
                                         oracle.MANIFEST, oracle.ORACLE_LOCK]],
                 "cases": cases, "full_file_observations": full_file,
                 "gzip_traces": traces,
                 "boundary_models": {"scope": "valid native algorithm models, not full-file syntax witnesses",
                                     "observations": boundary},
                 "limits": {"events_per_process": 20000, "logical_bytes_per_snapshot": 65536,
                            "sum_logical_snapshot_bytes": 16777216, "trace_file_bytes": 41943040},
                 "rust_execution": "deferred", "artifacts": artifact_records(bundle)}
        write_json(bundle / "index.json", index)
        shutil.move(bundle, output)
    print(f"Collected {len(cases)} complete native files: {output}")


if __name__ == "__main__":
    main()
