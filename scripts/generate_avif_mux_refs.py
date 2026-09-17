#!/usr/bin/env python3
"""Collect complete native still AVIF files and input-only mux descriptors.

The expected files come only from the pinned Pillow/native encoder. The
independent box reader extracts semantic inputs, never expected serialized
boxes, property indices, or output offsets for the Rust writer to consume.
"""

from __future__ import annotations

import argparse
import copy
import json
import shutil
import tempfile
from pathlib import Path

import yaml
from PIL import _avif

import generate_decode_refs as oracle
from generate_av1_sequence_refs import (
    LIBAVIF_COMMIT, artifact_records, digest_file, git_tree_digest, run,
    text_output, write_json,
)
from inspect_avif_bitstreams import (
    children, full_box_reader, parse_boxes, parse_iinf, parse_iloc,
    parse_iref, unique_box,
)

ROOT = Path(__file__).resolve().parent.parent


def extract(bundle, case):
    path = bundle / case["observations"]["encoded"]["path"]
    data = path.read_bytes()
    top = parse_boxes(data, 0, len(data))
    if [box.kind for box in top] != [b"ftyp", b"meta", b"mdat"]:
        raise RuntimeError("unexpected still top-level boxes")
    meta = children(data, unique_box(top, b"meta"), 4)
    kinds = parse_iinf(data, unique_box(meta, b"iinf"))
    locations = parse_iloc(data, unique_box(meta, b"iloc"), None)
    iprp = children(data, unique_box(meta, b"iprp"))
    props = children(data, unique_box(iprp, b"ipco"))
    version, flags, reader = full_box_reader(data, unique_box(iprp, b"ipma"))
    if version != 0 or flags != 0:
        raise RuntimeError("unexpected item association form")
    associations = {}
    for _ in range(reader.u32()):
        item = reader.u16()
        associations[item] = [props[(reader.u8() & 127) - 1] for _ in range(reader.u8())]
    if reader.offset != reader.end:
        raise RuntimeError("trailing item association bytes")
    refs_box = unique_box(meta, b"iref", required=False)
    refs = parse_iref(data, refs_box) if refs_box else []
    result = {"alpha": None, "icc": None, "exif": None, "xmp": None,
              "rotation": None, "mirror": None,
              "premultiplied": any(kind == b"prem" for kind, _, _ in refs)}
    source_spans = []

    def blob(label, payload, start):
        relative = f"inputs/{case['row_id']}/{label}.bin"
        record = oracle.stage_write_blob(bundle, relative, payload)
        source_spans.append({"input_path": relative, "native_offset": start,
                             "bytes": len(payload), "sha256": record["sha256"]})
        return record

    alpha_ids = {item for kind, item, _ in refs if kind == b"auxl"}
    for item_id, kind in kinds.items():
        construction, spans = locations[item_id]
        if construction != "file" or len(spans) != 1:
            raise RuntimeError("unexpected item extent form")
        start, end = spans[0]
        payload = data[start:end]
        if kind in (b"Exif", b"mime"):
            label = "exif" if kind == b"Exif" else "xmp"
            result[label] = blob(label, payload, start)
            continue
        if kind != b"av01":
            raise RuntimeError("unexpected coded item type")
        label = "alpha" if item_id in alpha_ids else "color"
        item_props = associations[item_id]
        config = unique_box(item_props, b"av1C")
        result[label] = {"payload": blob(label, payload, start),
                         "configuration": list(data[config.payload_start:config.end])}
        spatial = unique_box(item_props, b"ispe")
        _, _, spatial_reader = full_box_reader(data, spatial)
        dimensions = [spatial_reader.u32(), spatial_reader.u32()]
        if label == "alpha":
            if dimensions != result["dimensions"]:
                raise RuntimeError("alpha dimensions disagree")
            continue
        result["dimensions"] = dimensions
        for prop in item_props:
            raw = data[prop.payload_start:prop.end]
            if prop.kind == b"colr" and raw[:4] == b"prof":
                result["icc"] = blob("icc", raw[4:], prop.payload_start + 4)
            elif prop.kind == b"colr" and raw[:4] == b"nclx":
                result["cicp"] = [int.from_bytes(raw[i:i + 2], "big") for i in (4, 6, 8)]
                result["full_range"] = bool(raw[10] & 128)
            elif prop.kind in (b"irot", b"imir"):
                result["rotation" if prop.kind == b"irot" else "mirror"] = raw[0]
    return result, source_spans


def collect(bundle, repeat, manifest):
    matrix = {"formats": {}}
    oracle.sync_encode_rows(manifest, matrix)
    rows = matrix["formats"]["avif"]["encode"]
    specs = {spec["id"]: spec for spec in manifest["formats"]["avif"]["encode_edge_cases"]}
    cases = []

    def observe(row, spec, registered):
        first = oracle.stage_encode_case(bundle, row, spec)
        second = oracle.stage_encode_case(repeat, row, spec)
        if first != second or first["outcome"]["status"] != "success":
            raise RuntimeError(f"native encode differs or fails: {row['id']}")
        inputs, spans = extract(bundle, first)
        first.update({"mux_inputs": inputs, "input_provenance": spans,
                      "registered_matrix_row": registered, "native_observations": 2})
        cases.append(first)
        return first

    for row in rows:
        if row.get("rust_expect_error") or row.get("expect_error") or row.get("params", {}).get("animated"):
            continue
        observe(row, specs[row["id"]], True)
    if len(cases) != 16:
        raise RuntimeError("registered still selection changed; review evidence scope")
    rgb = next(row for row in rows if row["id"] == "enc_default_rgb")
    rgba = next(row for row in rows if row["id"] == "enc_rgba")
    base_case = next(case for case in cases if case["row_id"] == "enc_default_rgb")
    color = (bundle / base_case["mux_inputs"]["color"]["payload"]["path"]).read_bytes()
    valid_exif = bytes.fromhex("45786966000049492a0008000000000000000000")
    split = len(color) // 2
    supplements = [(f"mux_orientation_{orientation}", rgba, {"exif_orientation": orientation})
                   for orientation in range(1, 9)]
    supplements += [
        ("mux_alpha_metadata", rgba, {"icc_hex": "696363", "exif_hex": valid_exif.hex(), "xmp_hex": "786d70"}),
        ("mux_empty_metadata", rgb, {"icc_hex": "", "exif_hex": "", "xmp_hex": ""}),
        ("mux_substring_reuse", rgb, {"xmp_hex": (b"prefix" + color + b"suffix").hex()}),
        ("mux_cross_item_reuse", rgb, {"exif_hex": (valid_exif + color[:split]).hex(), "xmp_hex": color[split:].hex()}),
    ]
    for name, base, params in supplements:
        row = copy.deepcopy(base)
        row["id"] = name
        row["params"].update(params, max_threads=1)
        spec = {"id": name, "description": "Native still-container boundary witness",
                "source_asset": row["source_asset"], "source_format": row["source_format"],
                "params": row["params"]}
        observed = observe(row, spec, False)
        if "reuse" in name:
            spans = {Path(s["input_path"]).stem: s for s in observed["input_provenance"]}
            color_span = spans["color"]
            donor = spans["exif" if name == "mux_cross_item_reuse" else "xmp"]
            if not donor["native_offset"] <= color_span["native_offset"] < donor["native_offset"] + donor["bytes"]:
                raise RuntimeError("native file did not actually reuse the intended payload")
            if name == "mux_cross_item_reuse" and not color_span["native_offset"] + color_span["bytes"] > donor["native_offset"] + donor["bytes"]:
                raise RuntimeError("native reuse did not cross an item boundary")
    return cases


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--libavif-source", type=Path, required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    if (ROOT / "target/oracle-staging").resolve() not in output.parents or output.exists():
        raise RuntimeError("output must be fresh and under target/oracle-staging")
    source = args.libavif_source.resolve()
    if text_output(run(["git", "-C", str(source), "rev-parse", "HEAD"])) != LIBAVIF_COMMIT or run(["git", "-C", str(source), "status", "--porcelain"]).stdout:
        raise RuntimeError("libavif source must be clean and pinned")
    if digest_file(source / "LICENSE") != digest_file(ROOT / "third_party/libavif/LICENSE"):
        raise RuntimeError("retained libavif license differs")
    manifest = yaml.safe_load(oracle.MANIFEST.read_bytes())
    oracle.verify_primary_oracle(manifest, yaml.safe_load(oracle.ORACLE_LOCK.read_bytes()))
    if _avif.codec_versions() != "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2":
        raise RuntimeError("native codec versions differ from the pinned source")
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".avif-mux-", dir=output.parent) as temp:
        bundle, repeat = Path(temp) / "bundle", Path(temp) / "repeat"
        bundle.mkdir()
        repeat.mkdir()
        cases = collect(bundle, repeat, manifest)
        index = {"schema": "image-slash-star/avif-still-mux-oracle@1",
                 "oracle": oracle.oracle_identity(manifest),
                 "native_codecs": _avif.codec_versions(),
                 "avif_binary_sha256": digest_file(Path(_avif.__file__)),
                 "source": {"commit": LIBAVIF_COMMIT, "tree_sha256": git_tree_digest(source),
                            "files": [{"path": name, "sha256": digest_file(source / name)}
                                      for name in ("src/write.c", "include/avif/avif.h", "LICENSE")]},
                 "inputs": [{"path": str(p.relative_to(ROOT)), "sha256": digest_file(p)}
                            for p in (Path(__file__).resolve(), oracle.MANIFEST, oracle.ORACLE_LOCK,
                                      Path(oracle.__file__).resolve(), ROOT / "scripts/inspect_avif_bitstreams.py")],
                 "cases": cases, "artifacts": artifact_records(bundle),
                 "rust_execution": "deferred"}
        write_json(bundle / "index.json", index)
        shutil.move(bundle, output)
    print(f"Collected {len(cases)} complete native still files: {output}")


if __name__ == "__main__":
    main()
