#!/usr/bin/env python3
"""Search a pinned AVIF oracle for an origin Square64 transform split.

The campaign is input-only. It creates exactly one hundred deterministic
64x64 RGB candidates in ten families, encodes each candidate twice through
Pillow 12.2.0/libavif/libaom, and classifies two independent traces from the
pinned scalar dav1d diagnostic build. A candidate qualifies only when the
oracle exposes one origin Square64 leaf with a depth-one TX32x32 luma split,
DC predictors, and the bounded 8-bit 4:2:0 syntax selected for the first
portable implementation slice. Repository Rust is never invoked.

Generated files are temporary unless --retain-dir is supplied. Qualified
reports retain the exact encoded bytes, YUV bytes, Pillow RGB bytes, and
complete scalar trace as hex/text evidence.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import random
import re
import tempfile
from io import BytesIO
from pathlib import Path

from PIL import Image, _avif, features

from generate_av1_reconstruction_refs import (
    DAV1D_COMMIT,
    build_dav1d,
    extract_color_item,
    parse_debug_log,
    portable_color_reference,
    resolve_tool,
    run,
    verify_source,
)


SIZE = (64, 64)
SUBSAMPLING = "4:2:0"
FAMILY_NAMES = (
    "F01_rgb_noise",
    "F02_gray_noise",
    "F03_checker_2",
    "F04_checker_4",
    "F05_quadrants",
    "F06_horizontal_bands",
    "F07_vertical_bands",
    "F08_luma_ripple",
    "F09_chroma_checker",
    "F10_mixed_noise",
)
TRACE_SCOPE = "origin64-v1"
TRACE_CONTRACT = (
    "origin64-v1|full-origin-window-0..16|companion-coordinates|tx-detail"
)
TRACE_CONTRACT_SHA256 = hashlib.sha256(TRACE_CONTRACT.encode()).hexdigest()
EXPECTED_LUMA_COORDINATES = ((0, 0), (32, 0), (0, 32), (32, 32))
ADVANCED = {
    "min-partition-size": "64",
    "max-partition-size": "64",
    "use-intra-dct-only": "1",
    "enable-filter-intra": "0",
    "enable-intra-edge-filter": "0",
    "enable-smooth-intra": "0",
    "enable-paeth-intra": "0",
    "enable-directional-intra": "0",
    "enable-cfl-intra": "0",
    "enable-cdef": "0",
    "enable-restoration": "0",
    "loopfilter-control": "0",
    "aq-mode": "0",
    "deltaq-mode": "0",
}
BLOCK_PATTERN = re.compile(
    r"^poc=(?P<poc>-?\d+),y=(?P<y>-?\d+),x=(?P<x>-?\d+),"
    r"bl=(?P<level>\d+),ctx=(?P<context>\d+),bp=(?P<partition>\d+): "
    r"r=(?P<range>\d+)$"
)
SKIP_PATTERN = re.compile(r"^Post-skip\[(?P<skip>[01])\]")
TX_PATTERN = re.compile(r"^Post-tx\[(?P<tx>\d+)\]")
LUMA_PATTERN = re.compile(
    r"^Post-y-cf-blk\[tx=(?P<tx>\d+),txtp=(?P<txtp>-?\d+),"
    r"eob=(?P<eob>-?\d+)\]"
)
CHROMA_PATTERN = re.compile(
    r"^Post-uv-cf-blk\[pl=(?P<plane>\d+),tx=(?P<tx>\d+),"
    r"txtp=(?P<txtp>-?\d+),eob=(?P<eob>-?\d+)\]"
)
LUMA_COORD_PATTERN = re.compile(
    r"^Post-y-cf-coord\[x=(?P<x>\d+),y=(?P<y>\d+)\]"
)
CHROMA_COORD_PATTERN = re.compile(
    r"^Post-uv-cf-coord\[pl=(?P<plane>\d+),x=(?P<x>\d+),"
    r"y=(?P<y>\d+)\]"
)
TRACE_PATTERN = re.compile(
    r"^Post-square64-trace\[scope=(?P<scope>[^\]]+)\]"
)
TX_DETAIL_PATTERN = re.compile(
    r"^Post-tx-detail\[max=(?P<max>\d+),selected=(?P<selected>\d+),"
    r"depth=(?P<depth>\d+),x=(?P<x>\d+),y=(?P<y>\d+)\]"
)


def sha256(data: bytes) -> str:
    """Return a lowercase SHA-256 digest."""

    return hashlib.sha256(data).hexdigest()


def clamp(value: int) -> int:
    """Clamp a generated component to the 8-bit sample range."""

    return max(0, min(255, value))


def candidate_pixels(family: int, index: int) -> bytes:
    """Generate one deterministic RGB candidate for a named family."""

    seed = 64_000 + family * 100 + index
    state = random.Random(seed)
    pixels = bytearray()
    for y in range(SIZE[1]):
        for x in range(SIZE[0]):
            if family == 0:
                values = tuple(state.randrange(256) for _ in range(3))
            elif family == 1:
                value = state.randrange(256)
                values = (value, value, value)
            elif family == 2:
                value = 224 if ((x + y + index) // 2) % 2 else 24
                values = (value, clamp(value + 15), clamp(value - 15))
            elif family == 3:
                value = 224 if ((x + y + index) // 4) % 2 else 24
                values = (clamp(value + 18), value, clamp(value - 18))
            elif family == 4:
                quadrant = (x >= 32) + 2 * (y >= 32)
                values = (
                    (32, 80, 176),
                    (224, 64, 32),
                    (48, 192, 80),
                    (208, 192, 48),
                )[quadrant]
            elif family == 5:
                base = (32, 96, 160, 224)[y // 16]
                ripple = ((7 * x + 11 * y + index) % 17) - 8
                values = (
                    clamp(base + ripple + 18),
                    clamp(base + ripple),
                    clamp(base + ripple - 18),
                )
            elif family == 6:
                base = (32, 96, 160, 224)[x // 16]
                ripple = ((11 * x + 5 * y + index) % 19) - 9
                values = (
                    clamp(base + ripple + 18),
                    clamp(base + ripple),
                    clamp(base + ripple - 18),
                )
            elif family == 7:
                base = 40 + ((5 * x + 9 * y + index) % 176)
                values = (clamp(base + 24), base, clamp(base - 24))
            elif family == 8:
                base = 128 + (32 if (x // 8 + y // 8 + index) % 2 else -32)
                chroma = 42 if (x + 2 * y + index) % 3 else -42
                values = (clamp(base + chroma), base, clamp(base - chroma))
            else:
                base = 32 + ((3 * x + 7 * y + index) % 192)
                noise = state.randrange(-32, 33)
                values = (
                    clamp(base + noise + 16),
                    clamp(base + noise),
                    clamp(base + noise - 16),
                )
            pixels.extend(values)
    return bytes(pixels)


def candidates(quality: int, speed: int) -> list[dict[str, object]]:
    """Return exactly ten families with ten deterministic candidates each."""

    result = [
        {
            "id": f"S64-F{family + 1:02d}-N{index:02d}",
            "family": FAMILY_NAMES[family],
            "family_index": family,
            "candidate_index": index,
            "seed": 64_000 + family * 100 + index,
            "pixels": candidate_pixels(family, index),
            "quality": quality,
            "speed": speed,
        }
        for family in range(len(FAMILY_NAMES))
        for index in range(10)
    ]
    if len(result) != 100:
        raise AssertionError(f"candidate corpus must contain 100 cases, found {len(result)}")
    return result


def encode(pixels: bytes, quality: int, speed: int) -> bytes:
    """Encode one candidate through the pinned Pillow AVIF oracle."""

    output = BytesIO()
    Image.frombytes("RGB", SIZE, pixels).save(
        output,
        format="AVIF",
        quality=quality,
        speed=speed,
        max_threads=1,
        subsampling=SUBSAMPLING,
        autotiling=False,
        advanced=ADVANCED,
    )
    return output.getvalue()


def groups_from_debug(lines: list[str]) -> list[list[str]]:
    """Group debug lines by dav1d's block skip sentence."""

    groups: list[list[str]] = []
    for line in lines:
        if line.startswith("Post-skip["):
            groups.append([])
        if groups:
            groups[-1].append(line)
    return groups


def parse_group(group: list[str]) -> dict[str, object]:
    """Parse one origin block while retaining coordinate associations."""

    parsed: dict[str, object] = {
        "skip": [],
        "tx": [],
        "transform_details": [],
        "trace_scopes": [],
        "luma": [],
        "chroma": [],
    }
    pending_luma: tuple[int, int] | None = None
    pending_chroma: tuple[int, int, int] | None = None
    for line in group:
        if match := SKIP_PATTERN.match(line):
            parsed["skip"].append(int(match["skip"]))
            continue
        if match := TX_PATTERN.match(line):
            parsed["tx"].append(int(match["tx"]))
            continue
        if match := TRACE_PATTERN.match(line):
            parsed["trace_scopes"].append(match["scope"])
            continue
        if match := TX_DETAIL_PATTERN.match(line):
            parsed["transform_details"].append(
                {
                    name: int(value)
                    for name, value in match.groupdict().items()
                }
            )
            continue
        if match := LUMA_COORD_PATTERN.match(line):
            pending_luma = (int(match["x"]), int(match["y"]))
            continue
        if match := CHROMA_COORD_PATTERN.match(line):
            pending_chroma = (
                int(match["plane"]),
                int(match["x"]),
                int(match["y"]),
            )
            continue
        if match := LUMA_PATTERN.match(line):
            payload = {
                name: int(value)
                for name, value in match.groupdict().items()
            }
            if pending_luma is not None:
                payload["x"], payload["y"] = pending_luma
            parsed["luma"].append(payload)
            pending_luma = None
            continue
        if match := CHROMA_PATTERN.match(line):
            payload = {
                name: int(value)
                for name, value in match.groupdict().items()
            }
            if pending_chroma is not None:
                payload["plane"], payload["x"], payload["y"] = pending_chroma
            parsed["chroma"].append(payload)
            pending_chroma = None
    return parsed


def classify(
    blocks: list[dict[str, int]],
    groups: list[list[str]],
    yuv: bytes,
    portable_color: dict[str, object],
) -> dict[str, object]:
    """Apply exact predicates for the origin Square64 split witness."""

    parsed = [parse_group(group) for group in groups]
    origin_roots = [
        block
        for block in blocks
        if block["level"] == 1 and block["x"] == 0 and block["y"] == 0
    ]
    root = origin_roots[0] if len(origin_roots) == 1 else None
    first = parsed[0] if len(parsed) == 1 else {}
    no_palette = not any(
        line.startswith(
            (
                "Post-y_pal[",
                "Post-pal[",
                "Post-y-pal-indices",
                "y-pal-pred",
                "Post-uv_pal[",
                "Post-uv-pal-indices",
                "uv-pal-pred",
            )
        )
        for group in groups
        for line in group
    )
    luma = first.get("luma", [])
    chroma = first.get("chroma", [])
    transform_details = first.get("transform_details", [])
    trace_scopes = first.get("trace_scopes", [])
    luma_coordinates = [
        (payload.get("x"), payload.get("y")) for payload in luma
    ]
    chroma_coordinates = [
        (payload.get("plane"), payload.get("x"), payload.get("y"))
        for payload in chroma
    ]
    predicates = {
        "frame_is_64x64_8bit_420": (
            portable_color.get("width") == 64
            and portable_color.get("height") == 64
            and portable_color.get("bit_depth") == 8
            and portable_color.get("monochrome") is False
            and portable_color.get("subsampling_x") is True
            and portable_color.get("subsampling_y") is True
        ),
        "root_is_single_square64": (
            len(origin_roots) == 1
            and root is not None
            and root["partition"] == 0
            and root["level"] == 1
            and root["x"] == 0
            and root["y"] == 0
        ),
        "single_origin_leaf_group": len(groups) == 1,
        "origin_block_is_not_skipped": first.get("skip") == [0],
        "one_tx32_transform_symbol": first.get("tx") == [3],
        "trace_scope_is_origin64_v1": trace_scopes == [TRACE_SCOPE],
        "one_origin_tx_detail": transform_details == [
            {"max": 4, "selected": 3, "depth": 1, "x": 0, "y": 0}
        ],
        "four_luma_tx32_dct_children": (
            len(luma) == 4
            and all(
                payload["tx"] == 3
                and payload["txtp"] == 0
                for payload in luma
            )
            and luma_coordinates == list(EXPECTED_LUMA_COORDINATES)
        ),
        "two_chroma_tx32_dct_planes": (
            len(chroma) == 2
            and {payload["plane"] for payload in chroma} == {0, 1}
            and all(
                payload["tx"] == 3 and payload["txtp"] == 0
                for payload in chroma
            )
            and all(
                payload.get("x") == 0 and payload.get("y") == 0
                for payload in chroma
            )
        ),
        "luma_split_has_residual": any(payload["eob"] >= 1 for payload in luma),
        "yuv_has_4096_byte_luma_and_1024_byte_chroma_planes": len(yuv) == 6144,
        "no_palette_syntax": no_palette,
    }
    return {
        "target": "origin_square64_tx32x32_split_dc_420",
        "root_partition": root,
        "group_count": len(groups),
        "skip_symbols": first.get("skip", []),
        "transform_symbols": first.get("tx", []),
        "transform_details": transform_details,
        "trace_scopes": trace_scopes,
        "trace_scope": trace_scopes[0] if len(trace_scopes) == 1 else None,
        "luma_payloads": luma,
        "chroma_payloads": chroma,
        "luma_coordinates": luma_coordinates,
        "chroma_coordinates": chroma_coordinates,
        "predicates": predicates,
        "rejection_reasons": [
            name for name, passed in predicates.items() if not passed
        ],
        "qualifies": all(predicates.values()),
    }


def trace_once(
    executable: Path,
    environment: dict[str, str],
    work: Path,
    item: bytes,
    stem: str,
    ordinal: int,
) -> tuple[str, bytes]:
    """Decode one AV1 item with the independent scalar dav1d binary."""

    item_path = work / f"{stem}-{ordinal}.obu"
    yuv_path = work / f"{stem}-{ordinal}.yuv"
    item_path.write_bytes(item)
    result = run(
        [
            str(executable),
            "--input",
            str(item_path),
            "--demuxer",
            "section5",
            "--output",
            str(yuv_path),
            "--muxer",
            "yuv",
            "--threads",
            "1",
            "--framedelay",
            "1",
            "--cpumask",
            "0",
            "--quiet",
        ],
        env=environment,
    )
    return result.stdout, yuv_path.read_bytes()


def decode_candidate(
    executable: Path,
    environment: dict[str, str],
    work: Path,
    candidate: dict[str, object],
    retain_dir: Path | None,
) -> dict[str, object]:
    """Double-encode, double-trace, and classify one candidate."""

    pixels = candidate["pixels"]
    if not isinstance(pixels, bytes):
        raise TypeError("candidate pixels must be bytes")
    quality = int(candidate["quality"])
    speed = int(candidate["speed"])
    encoded = encode(pixels, quality, speed)
    encoded_second = encode(pixels, quality, speed)
    if encoded != encoded_second:
        raise RuntimeError(f"nondeterministic AVIF encoding for {candidate['id']}")
    path = work / f"{candidate['id']}.avif"
    second_path = work / f"{candidate['id']}-second.avif"
    path.write_bytes(encoded)
    second_path.write_bytes(encoded_second)
    item, _ = extract_color_item(path)
    second_item, _ = extract_color_item(second_path)
    if item != second_item:
        raise RuntimeError(f"nondeterministic AV1 item for {candidate['id']}")
    portable_color = portable_color_reference(path)
    trace_a, yuv_a = trace_once(
        executable, environment, work, item, str(candidate["id"]), 1
    )
    trace_b, yuv_b = trace_once(
        executable, environment, work, second_item, str(candidate["id"]), 2
    )
    if trace_a != trace_b or yuv_a != yuv_b:
        raise RuntimeError(f"nondeterministic dav1d trace or YUV for {candidate['id']}")
    debug_lines, _, entropy_operations, blocks, _ = parse_debug_log(trace_a)
    groups = groups_from_debug(debug_lines)
    with Image.open(BytesIO(encoded)) as decoded:
        decoded.load()
        if decoded.mode != "RGB" or decoded.size != SIZE:
            raise RuntimeError(f"{candidate['id']} Pillow result is not 64x64 RGB")
        pillow_rgb = decoded.tobytes()
    classification = classify(blocks, groups, yuv_a, portable_color)
    predicates = classification["predicates"]
    if not isinstance(predicates, dict):
        raise TypeError("classification predicates must be a dictionary")
    predicates.update(
        {
            "double_encode_equal": encoded == encoded_second,
            "double_item_equal": item == second_item,
            "double_trace_equal": True,
            "pillow_rgb_is_12288_bytes": len(pillow_rgb) == 12_288,
        }
    )
    classification["rejection_reasons"] = [
        name for name, passed in predicates.items() if not passed
    ]
    classification["qualifies"] = all(predicates.values())
    report: dict[str, object] = {
        "id": candidate["id"],
        "family": candidate["family"],
        "family_index": candidate["family_index"],
        "candidate_index": candidate["candidate_index"],
        "seed": candidate["seed"],
        "quality": quality,
        "speed": speed,
        "input_rgb_sha256": sha256(pixels),
        "encoded_file_sha256": sha256(encoded),
        "encoded_file_sha256_second": sha256(encoded_second),
        "encoded_item_sha256": sha256(item),
        "encoded_item_length": len(item),
        "pillow_rgb_sha256": sha256(pillow_rgb),
        "yuv_sha256": sha256(yuv_a),
        "y_plane_sha256": sha256(yuv_a[:4096]),
        "u_plane_sha256": sha256(yuv_a[4096:5120]),
        "v_plane_sha256": sha256(yuv_a[5120:]),
        "trace_sha256": sha256(trace_a.encode()),
        "entropy_operation_count": len(entropy_operations),
        **classification,
    }
    if classification["qualifies"]:
        report.update(
            {
                "encoded_bytes_hex": encoded.hex(),
                "yuv_bytes_hex": yuv_a.hex(),
                "pillow_rgb_bytes_hex": pillow_rgb.hex(),
                "dav1d_trace": trace_a,
            }
        )
        if retain_dir is not None:
            retain_dir.mkdir(parents=True, exist_ok=True)
            (retain_dir / path.name).write_bytes(encoded)
            (retain_dir / f"{candidate['id']}.rgb").write_bytes(pixels)
    return report


def main() -> None:
    """Run the pinned 100-case input-only campaign."""

    parser = argparse.ArgumentParser(description=__doc__)
    decoder = parser.add_mutually_exclusive_group(required=True)
    decoder.add_argument("--dav1d", type=Path)
    decoder.add_argument("--dav1d-source", type=Path)
    parser.add_argument("--meson", default="meson")
    parser.add_argument("--ninja", default="ninja")
    parser.add_argument("--python-path", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--retain-dir", type=Path)
    parser.add_argument("--quality", type=int, default=76)
    parser.add_argument("--speed", type=int, default=0)
    args = parser.parse_args()
    if not 0 <= args.quality <= 100:
        raise RuntimeError("quality must be in 0..100")
    if not 0 <= args.speed <= 10:
        raise RuntimeError("speed must be in 0..10")
    if features.version("avif") != "1.4.1":
        raise RuntimeError(f"expected libavif 1.4.1, found {features.version('avif')}")
    codecs = _avif.codec_versions()
    for expected in ("dav1d [dec]:1.5.3", "aom [enc]:3.13.2"):
        if expected not in codecs:
            raise RuntimeError(f"Pillow AVIF oracle lacks {expected}: {codecs}")
    with tempfile.TemporaryDirectory(prefix="image-star-avif-square64-split-") as name:
        work = Path(name)
        if args.dav1d_source is not None:
            source = args.dav1d_source.resolve()
            verify_source(source)
            executable, environment = build_dav1d(
                source,
                work,
                resolve_tool(args.meson, "Meson"),
                resolve_tool(args.ninja, "Ninja"),
                args.python_path.resolve() if args.python_path else None,
                square64_origin=True,
            )
        else:
            executable = args.dav1d.resolve()
            environment = {}
        version_result = run([str(executable), "--version"], env=environment)
        version = (version_result.stdout + version_result.stderr).strip()
        if not version.startswith("1.5.3-0-gb546257"):
            raise RuntimeError(f"unexpected dav1d executable version: {version}")
        reports = [
            decode_candidate(
                executable,
                environment,
                work,
                candidate,
                args.retain_dir.resolve() if args.retain_dir else None,
            )
            for candidate in candidates(args.quality, args.speed)
        ]
    rejection_reasons = sorted(
        {
            reason
            for report in reports
            for reason in report["rejection_reasons"]
        }
    )
    report = {
        "format_version": 1,
        "oracle": {
            "pillow": Image.__version__,
            "libavif": features.version("avif"),
            "codecs": codecs,
            "dav1d": version,
            "dav1d_commit": DAV1D_COMMIT,
        },
        "instrumentation_contract": {
            "scope": TRACE_SCOPE,
            "description": TRACE_CONTRACT,
            "sha256": TRACE_CONTRACT_SHA256,
        },
        "encoding": {
            "size": list(SIZE),
            "subsampling": SUBSAMPLING,
            "quality": args.quality,
            "speed": args.speed,
            "max_threads": 1,
            "autotiling": False,
            "advanced": ADVANCED,
        },
        "search": {
            "input_only": True,
            "repository_rust_invoked": False,
            "candidate_count": len(reports),
            "family_count": len(FAMILY_NAMES),
            "candidates_per_family": 10,
            "target_id": "origin_square64_tx32x32_split_dc_420",
            "target": (
                "one origin 64x64 4:2:0 Square64 leaf with transform depth "
                "one, four TX32x32 luma DCT children, TX32x32 DCT chroma, "
                "DC predictors, and at least one coded luma residual"
            ),
            "families": list(FAMILY_NAMES),
            "trace_scope": TRACE_SCOPE,
            "instrumentation_contract_sha256": TRACE_CONTRACT_SHA256,
        },
        "counts": {
            "qualified": sum(bool(item["qualifies"]) for item in reports),
            "qualified_by_family": {
                family: sum(
                    bool(item["qualifies"]) and item["family"] == family
                    for item in reports
                )
                for family in FAMILY_NAMES
            },
            "by_rejection_reason": {
                reason: sum(reason in item["rejection_reasons"] for item in reports)
                for reason in rejection_reasons
            },
        },
        "cases": reports,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(f"Written {len(reports)} deterministic Square64 split traces: {args.output}")


if __name__ == "__main__":
    main()
