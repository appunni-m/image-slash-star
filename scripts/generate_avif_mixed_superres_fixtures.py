#!/usr/bin/env python3
"""Generate input-only mixed lossless/lossy AVIF super-resolution sequences.

A pinned source-only native producer encodes a lossy key followed by a mixed
inter frame. Reference pixels belong to the standard oracle collectors.
"""
from __future__ import annotations

import argparse
import hashlib
from pathlib import Path
import subprocess
import tempfile

from generate_avif_10bit_lossless_inter_fixture import normalize_sequence_timestamps
from generate_avif_filmgrain_edge_fixtures import require_pinned_oracle
from inspect_av1_obus import inspect

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "tests/fixtures/input/images/avif"
FIXTURES = {
    "animated_mixed_lossless_superres_400_10bit_neutral_80x48": {
        "depth": 10,
        "input_sha256": "6748e3ac11be0ee1f4d06191e4981419de26a71f992ac3074cb4fcdcdbf46125",
        "layout": "400",
        "restoration": 0
    },
    "animated_mixed_lossless_superres_400_10bit_sgr_80x48": {
        "depth": 10,
        "input_sha256": "c6ec5a4851183d0d06c3af9b43f1aca419656b1c1e3eb1ea28624fd902d37eb2",
        "layout": "400",
        "restoration": 1
    },
    "animated_mixed_lossless_superres_400_10bit_filmgrain_80x48": {
        "depth": 10,
        "film_grain": True,
        "input_sha256": "eb12b32b20cf07fcf25191c18f710c04a62d4451cbd54094e2acb98ffc01d531",
        "layout": "400",
        "restoration": 0
    },
    "animated_mixed_lossless_superres_400_12bit_neutral_80x48": {
        "depth": 12,
        "input_sha256": "a538876e087283a7d2596e3f5ef87446869308895c34736f5887c8fab5006529",
        "layout": "400",
        "restoration": 0
    },
    "animated_mixed_lossless_superres_400_12bit_filmgrain_80x48": {
        "depth": 12,
        "film_grain": True,
        "input_sha256": "8b05f1891e4720bf3906e9905641e4f23137c5284a3cdb165cf5241e527000d2",
        "layout": "400",
        "restoration": 0
    },
    "animated_mixed_lossless_superres_400_8bit_neutral_80x48": {
        "depth": 8,
        "input_sha256": "aa2299060bc5f9b18ae88894bd29dfe06801606160dca1498a74fe6547faa47c",
        "layout": "400",
        "restoration": 0
    },
    "animated_mixed_lossless_superres_400_8bit_filmgrain_80x48": {
        "depth": 8,
        "film_grain": True,
        "input_sha256": "903594a10cea1383ddde6930d31119a46b9ba7f756793e47e351c2d158256196",
        "layout": "400",
        "restoration": 0
    },
    "animated_mixed_lossless_superres_400_8bit_sgr_80x48": {
        "depth": 8,
        "input_sha256": "8886acec3f9b44eab0bb9dffe033b862f17f39117188eb8ec2425922b401e301",
        "layout": "400",
        "restoration": 1
    },
    "animated_mixed_lossless_superres_420_10bit_neutral_80x48": {
        "depth": 10,
        "input_sha256": "9cef3d1c8cc2c8a7484ce6a04ec4a706de060cc0212d68409cd3e3b6e34f0e2d",
        "layout": "420",
        "restoration": 0
    },
    "animated_mixed_lossless_superres_420_10bit_filmgrain_80x48": {
        "depth": 10,
        "film_grain": True,
        "input_sha256": "aec952b0067e4b4a022ddb0427f7e87d9fed11ee235a454c60da6be92cb88f2f",
        "layout": "420",
        "restoration": 0
    },
    "animated_mixed_lossless_superres_420_10bit_sgr_80x48": {
        "depth": 10,
        "input_sha256": "dbe51bd886ae6b5077c33f836f269098d8904099235e8a46baa76774d0ec7b44",
        "layout": "420",
        "restoration": 1
    },
    "animated_mixed_lossless_superres_420_12bit_filmgrain_80x48": {
        "depth": 12,
        "film_grain": True,
        "input_sha256": "aaa8f784f24357bc84ce212b8e995f68b251d4a84152c3953e6e76af865a1589",
        "layout": "420",
        "restoration": 0
    },
    "animated_mixed_lossless_superres_420_12bit_neutral_80x48": {
        "depth": 12,
        "input_sha256": "8dd67ec59bd9fb4f40cf6a518ffca77905f7541044f10c5875e5b9c4c82bd40b",
        "layout": "420",
        "restoration": 0
    },
    "animated_mixed_lossless_superres_420_8bit_neutral_80x48": {
        "depth": 8,
        "input_sha256": "eb779d61beadb9e5e05ccb34834860a8ce39b451be13c207b3c1f0ea30395d21",
        "layout": "420",
        "restoration": 0
    },
    "animated_mixed_lossless_superres_420_8bit_filmgrain_80x48": {
        "depth": 8,
        "film_grain": True,
        "input_sha256": "4486713a624c8282f82f165aadacbf6a6dfda4244375dcae2878cb550cb4fea1",
        "layout": "420",
        "restoration": 0
    },
    "animated_mixed_lossless_superres_420_8bit_cdef_filmgrain_80x48": {
        "cdef": True,
        "depth": 8,
        "film_grain": True,
        "input_sha256": "dcc57c6d407a6c842ffc83e9e94855f548c58f7bc8a68d198f12847fa84628a2",
        "layout": "420",
        "restoration": 0
    },
    "animated_mixed_lossless_superres_420_8bit_loopfilter_80x48": {
        "depth": 8,
        "input_sha256": "d34b4d36352a5401c5320b9576b26ab3a6a6b7587b1e38e078549b367a4f0b3b",
        "inter_segment_qindex": [124, 0, 124, 124, 124, 124, 124, 124],
        "layout": "420",
        "loop_filter": True,
        "quality": 45,
        "restoration": 0,
        "speed": 10
    },
    "animated_mixed_lossless_superres_420_8bit_sgr_80x48": {
        "depth": 8,
        "input_sha256": "706483e6149e8b8aeab31f85344aaad8f38b3fda125cad40db65134d0db43048",
        "layout": "420",
        "restoration": 1
    },
    "animated_mixed_lossless_superres_420_8bit_sgr_filmgrain_80x48": {
        "depth": 8,
        "film_grain": True,
        "input_sha256": "c9bbded2b96c08a2a2e5792bee2f313858e7889727e7eb6c99eba059c3905563",
        "layout": "420",
        "restoration": 1
    },
    "animated_mixed_lossless_superres_422_10bit_neutral_80x48": {
        "depth": 10,
        "input_sha256": "af446607101962de4733065bc5770cd9a7ff66714cf6dc3861ee41e5dd20d3c2",
        "layout": "422",
        "restoration": 0
    },
    "animated_mixed_lossless_superres_422_10bit_sgr_80x48": {
        "depth": 10,
        "input_sha256": "707f9011d2bfc877ac0f51800ef8bc5661971294e7defacc5508eaa676470575",
        "layout": "422",
        "restoration": 1
    },
    "animated_mixed_lossless_superres_422_10bit_filmgrain_80x48": {
        "depth": 10,
        "film_grain": True,
        "input_sha256": "f9bc47d21bfa3e40d7724547603a25ae171170ca8b12264f437b20df7f7108ba",
        "layout": "422",
        "restoration": 0
    },
    "animated_mixed_lossless_superres_422_12bit_neutral_80x48": {
        "depth": 12,
        "input_sha256": "da6d31fc9ac3f7687c8273eda7f66f91995c6f1dc1e41bca3c98fd7d3d7450e9",
        "layout": "422",
        "restoration": 0
    },
    "animated_mixed_lossless_superres_422_12bit_filmgrain_80x48": {
        "depth": 12,
        "film_grain": True,
        "input_sha256": "7cd21564db3da0e82a20270034e9c85f7e7551e14f520cbe9b3bb141efa27e99",
        "layout": "422",
        "restoration": 0
    },
    "animated_mixed_lossless_superres_422_8bit_neutral_80x48": {
        "depth": 8,
        "input_sha256": "9c0b476ffff1a1665073179d6c8bddf5f8e87e22d817b871c6f6a1f538fadca1",
        "layout": "422",
        "restoration": 0
    },
    "animated_mixed_lossless_superres_422_8bit_filmgrain_80x48": {
        "depth": 8,
        "film_grain": True,
        "input_sha256": "41a52794082870ba454795331fbe40dcb1a9fa12fed53acd821150f7afb0998b",
        "layout": "422",
        "restoration": 0
    },
    "animated_mixed_lossless_superres_422_8bit_sgr_80x48": {
        "depth": 8,
        "input_sha256": "a6f709522cf9a32d8430f4fb1833f38a8e3905379369406da4dfdda21558d082",
        "layout": "422",
        "restoration": 1
    },
    "animated_mixed_lossless_superres_444_10bit_neutral_80x48": {
        "depth": 10,
        "input_sha256": "bb40405f9a5a057e09c9f17b02fba7be8a289e5f7b83dd8e24d5b275d1ed8f1f",
        "layout": "444",
        "restoration": 0
    },
    "animated_mixed_lossless_superres_444_10bit_sgr_80x48": {
        "depth": 10,
        "input_sha256": "4abbb1d3a14c6fa89f42501572349e277c12478d36678102163c4359da89a8d6",
        "layout": "444",
        "restoration": 1
    },
    "animated_mixed_lossless_superres_444_10bit_loopfilter_80x48": {
        "depth": 10,
        "input_sha256": "e7c75ed78f3ba1ef3bcd4c56c32e7ea832901cfcc41b0282a05c1bce4cfda646",
        "inter_segment_qindex": [124, 0, 124, 124, 124, 124, 124, 124],
        "layout": "444",
        "loop_filter": True,
        "quality": 45,
        "restoration": 0,
        "speed": 10
    },
    "animated_mixed_lossless_superres_444_10bit_filmgrain_128x64": {
        "coded_width": 64,
        "depth": 10,
        "denominator": 16,
        "film_grain": True,
        "height": 64,
        "input_sha256": "14d1effed3e3c34e17ca86936ba332d28cbcd25376f2a642ff7fd1d4d1e673c6",
        "layout": "444",
        "width": 128,
        "restoration": 0
    },
    "animated_mixed_lossless_superres_444_12bit_neutral_80x48": {
        "depth": 12,
        "input_sha256": "97f8d605402baade83f5889470ab9ba1dc4dd149efbfe4178d155a9b9c77c46d",
        "layout": "444",
        "restoration": 0
    },
    "animated_mixed_lossless_superres_444_12bit_filmgrain_128x64": {
        "coded_width": 64,
        "depth": 12,
        "denominator": 16,
        "film_grain": True,
        "height": 64,
        "input_sha256": "00c469a1475ddeffeefa69a9a314c3bf4f172596ebcf2b826bd457dbc70222da",
        "layout": "444",
        "width": 128,
        "restoration": 0
    },
    "animated_mixed_lossless_superres_444_8bit_neutral_80x48": {
        "depth": 8,
        "input_sha256": "3694d481225590893e003ea85295323eb223a1f1e304ad16572a7b39cf19deec",
        "layout": "444",
        "restoration": 0
    },
    "animated_mixed_lossless_superres_444_8bit_sgr_80x48": {
        "depth": 8,
        "input_sha256": "a49b14c52ccab5014531b7f7281c531bf093cba2cd338b2babb033436a2c9dab",
        "layout": "444",
        "restoration": 1
    }
}


def validate(path: Path, name: str) -> None:
    """Check complete input identity and independently parsed AV1 features."""
    recipe = FIXTURES[name]
    if hashlib.sha256(path.read_bytes()).hexdigest() != recipe["input_sha256"]:
        raise RuntimeError(f"{name}: complete input digest differs from pinned recipe")
    report = inspect(path)
    width = recipe.get("width", 80)
    height = recipe.get("height", 48)
    headers = [obu["sequence_header"] for sample in report["samples"]
               for obu in sample["obus"] if "sequence_header" in obu]
    layout = recipe["layout"]
    expected = {"bit_depth": recipe["depth"], "max_width": width, "max_height": height,
                "monochrome": layout == "400", "subsampling_x": int(layout != "444"),
                "subsampling_y": int(layout in ("400", "420")), "enable_superres": True,
                "enable_restoration": bool(recipe["restoration"]),
                "film_grain_present": bool(recipe.get("film_grain", False))}
    if not headers or any(any(h.get(k) != v for k, v in expected.items()) for h in headers):
        raise RuntimeError(f"{name}: sequence does not match its planar input recipe")
    frames = [obu["frame_header"] for sample in report["samples"]
              if sample.get("role") == "track_pict" for obu in sample["obus"]
              if "frame_header" in obu]
    if len(frames) != 2:
        raise RuntimeError(f"{name}: expected one key and one inter sample")
    key, inter = frames
    if recipe.get("film_grain") and any(
            not frame.get("film_grain", {}).get("y_points") for frame in frames):
        raise RuntimeError(f"{name}: key and inter frames must carry active film grain")
    if recipe.get("cdef"):
        cdef = frames[1].get("cdef")
        if (cdef is None or not cdef.get("y_strengths") or not cdef.get("uv_strengths")
                or not any(cdef["y_strengths"]) or not any(cdef["uv_strengths"])):
            raise RuntimeError(f"{name}: inter frame must carry active CDEF strengths")
    if recipe.get("loop_filter") and any(
            not any(frame.get("loop_filter", {}).get("level_y", []))
            or not frame.get("loop_filter", {}).get("level_u")
            or not frame.get("loop_filter", {}).get("level_v")
            for frame in frames):
        raise RuntimeError(f"{name}: key and inter frames need active Y, U and V loop filters")
    if (key.get("frame_type") != "key" or key.get("frame_width") != width
            or key.get("frame_height") != height or key.get("superres_enabled") is not False
            or key.get("segmentation", {}).get("enabled") is not False):
        raise RuntimeError(f"{name}: key frame must remain ordinary lossy input")
    segmentation = inter.get("segmentation", {})
    expected_restoration = 2 if recipe["restoration"] and recipe["depth"] < 12 else 0
    if (inter.get("frame_type") != "inter"
            or inter.get("frame_width") != recipe.get("coded_width", 64)
            or inter.get("frame_height") != height or inter.get("upscaled_width") != width
            or inter.get("superres_enabled") is not True or inter.get("all_lossless") is not False
            or segmentation.get("enabled") is not True
            or segmentation.get("update_map") is not True or segmentation.get("temporal") is not False
            or len(segmentation.get("segments", [])) != 8
            or any(segment.get("reference") != -1 or segment.get("skip") is not False
                   or segment.get("global_motion") is not False
                   for segment in segmentation.get("segments", []))
            or inter.get("segment_qindex") != recipe.get(
                "inter_segment_qindex", [140, 0, 140, 140, 140, 140, 140, 140]
            )
            or inter.get("segment_lossless") != [False, True, False, False, False, False, False, False]
            or inter.get("restoration", {}).get("types") != [expected_restoration, 0, 0]):
        raise RuntimeError(f"{name}: inter frame lacks the mixed super-resolution input features")


def generate(encoder: Path, destination: Path, name: str) -> None:
    recipe = FIXTURES[name]
    width = recipe.get("width", 80)
    height = recipe.get("height", 48)
    with tempfile.TemporaryDirectory(prefix="avif-mixed-superres-") as temporary:
        outputs = []
        for attempt in range(2):
            path = Path(temporary) / f"{attempt}.avif"
            command = [str(encoder), str(path), str(width), str(height), str(recipe["depth"]),
                       recipe["layout"], str(recipe.get("quality", 45)),
                       str(recipe.get("speed", 3)), str(recipe.get("denominator", 10)),
                       str(recipe["restoration"])]
            if recipe.get("cdef"):
                command.append("film-grain-cdef")
            elif recipe.get("film_grain"):
                command.append("film-grain")
            elif recipe.get("loop_filter"):
                command.append("loopfilter")
            result = subprocess.run(command, capture_output=True, text=True, check=False)
            if result.returncode != 0:
                raise RuntimeError(f"native producer failed ({result.returncode}): {result.stderr}")
            data = normalize_sequence_timestamps(path.read_bytes())
            path.write_bytes(data)
            validate(path, name)
            outputs.append(data)
        if outputs[0] != outputs[1]:
            raise RuntimeError(f"{name}: independent normalized encodes differ")
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(outputs[0])


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--encoder", type=Path)
    parser.add_argument("--output-dir", type=Path, default=OUTPUT)
    parser.add_argument("--variant", choices=tuple(FIXTURES))
    args = parser.parse_args()
    require_pinned_oracle()
    encoder = None
    if not args.check:
        if args.encoder is None:
            parser.error("--encoder is required to generate native inputs")
        encoder = args.encoder.expanduser().resolve(strict=True)
        version = subprocess.run([str(encoder), "--version"], capture_output=True, text=True, check=True)
        if version.stdout.strip() != "libavif 1.4.1 (aom [enc]:v3.13.2)":
            raise RuntimeError(f"pinned producer required: {version.stdout!r}")
    names = [args.variant] if args.variant else FIXTURES
    for name in names:
        path = args.output_dir / (name + ".avif")
        if args.check:
            validate(path, name)
        else:
            assert encoder is not None
            generate(encoder, path, name)
    print(f"mixed-segment super-resolution inputs: {len(names)} {'checked' if args.check else 'reproduced twice'}")


if __name__ == "__main__":
    main()
