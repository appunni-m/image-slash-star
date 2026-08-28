#!/usr/bin/env python3
"""Search a bounded corpus for a legal 8x8 luma EOB-bin/base sentence.

The campaign creates exactly one hundred deterministic 4x4 RGB candidates in
ten families.  Every candidate is encoded twice with the pinned
Pillow/libavif/libaom oracle and its extracted AV1 item is decoded twice with
an independently built scalar dav1d.  A candidate qualifies only when the
oracle produces the already-supported one-tile 8-bit 4:2:0 origin topology,
an unsplit TX8x8 DCT_DCT luma block, EOB-bin symbol two (EOB three), direct
EOB-base zero, a non-empty AC sentence, and skipped 4x4 chroma.  The campaign
never invokes repository Rust code; the selected AVIF is retained separately
for later fixture promotion.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
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
from inspect_av1_obus import inspect as inspect_av1


SIZE = (4, 4)
SUBSAMPLING = "4:2:0"
EXPECTED_YUV_BYTES = SIZE[0] * SIZE[1] + 2 * (SIZE[0] // 2) * (SIZE[1] // 2)
FAMILY_NAMES = (
    "F01_negative_impulse_base127",
    "F02_negative_impulse_base129",
    "F03_positive_impulse_base127",
    "F04_positive_impulse_base129",
    "F05_two_impulses",
    "F06_horizontal_ramp",
    "F07_vertical_ramp",
    "F08_checkerboard",
    "F09_cross",
    "F10_low_amplitude_noise",
)
QUALITY = 99
SPEED = 8
BLOCK_PATTERN = re.compile(
    r"^poc=(?P<poc>-?\d+),y=(?P<y>-?\d+),x=(?P<x>-?\d+),"
    r"bl=(?P<level>\d+),ctx=(?P<context>\d+),bp=(?P<partition>\d+): "
    r"r=(?P<range>\d+)$"
)
EOB_BIN_PATTERN = re.compile(
    r"^Post-eob_bin_64\[0\]\[0\]\[(?P<value>\d+)\]: "
    r"r=(?P<range>\d+)$"
)
EOB_PATTERN = re.compile(r"^Post-eob\[(?P<value>-?\d+)\]: r=(?P<range>\d+)$")
LUMA_PATTERN = re.compile(
    r"^Post-y-cf-blk\[tx=(?P<tx>\d+),txtp=(?P<txtp>-?\d+),"
    r"eob=(?P<eob>-?\d+)\]"
)
CHROMA_PATTERN = re.compile(
    r"^Post-uv-cf-blk\[pl=(?P<plane>\d+),tx=(?P<tx>\d+),"
    r"txtp=(?P<txtp>-?\d+),eob=(?P<eob>-?\d+)\]"
)
TX_PATTERN = re.compile(r"^Post-tx\[(?P<tx>\d+)\]")
TXTP_PATTERN = re.compile(
    r"^Post-txtp-intra\[(?P<maximum>\d+)->(?P<minimum>\d+)\]"
    r"\[(?P<symbol>\d+)\]\[(?P<symbol_value>\d+)->(?P<txtp>-?\d+)\]"
)
YMODE_PATTERN = re.compile(r"^Post-ymode\[(?P<mode>\d+)\]")
UVMODE_PATTERN = re.compile(r"^Post-uvmode\[(?P<mode>\d+)\]")
LO_TOKEN_PATTERN = re.compile(r"^Post-lo_tok\[.*=(?P<token>\d+)\]:")


def sha256(data: bytes) -> str:
    """Return the lowercase SHA-256 digest of ``data``."""

    return hashlib.sha256(data).hexdigest()


def clamp(value: int) -> int:
    """Clamp one generated sample to the eight-bit range."""

    return max(0, min(255, value))


def candidate_pixels(family: int, index: int) -> tuple[bytes, dict[str, object]]:
    """Build one deterministic RGB candidate and its generation metadata."""

    seed = 901_000 + family * 100 + index
    state = random.Random(seed)
    base = 127 if family in (0, 2, 4, 5, 6, 7, 8) else 129
    pixels = [base] * (SIZE[0] * SIZE[1])

    if family in (0, 1, 2, 3):
        delta = -1 if family in (0, 1) else 1
        position = index
        pixels[position] = clamp(base + delta)
        pattern = "single_impulse"
    elif family == 4:
        first = index
        second = (index * 5 + 3) % len(pixels)
        pixels[first] = clamp(base - 1)
        pixels[second] = clamp(base + 1)
        pattern = "two_impulses"
    elif family == 5:
        for y in range(SIZE[1]):
            for x in range(SIZE[0]):
                pixels[y * SIZE[0] + x] = clamp(base + ((x + index) % 3) - 1)
        pattern = "horizontal_ramp"
    elif family == 6:
        for y in range(SIZE[1]):
            for x in range(SIZE[0]):
                pixels[y * SIZE[0] + x] = clamp(base + ((y + index) % 3) - 1)
        pattern = "vertical_ramp"
    elif family == 7:
        for y in range(SIZE[1]):
            for x in range(SIZE[0]):
                pixels[y * SIZE[0] + x] = clamp(
                    base + (1 if (x + y + index) % 2 else -1)
                )
        pattern = "checkerboard"
    elif family == 8:
        for y in range(SIZE[1]):
            for x in range(SIZE[0]):
                if x == SIZE[0] // 2 or y == SIZE[1] // 2:
                    pixels[y * SIZE[0] + x] = clamp(
                        base + (1 if (x + y + index) % 2 else -1)
                    )
        pattern = "cross"
    else:
        pixels = [clamp(base + state.choice((-2, -1, 0, 1, 2))) for _ in pixels]
        pattern = "low_amplitude_noise"

    rgb = b"".join(bytes((value, value, value)) for value in pixels)
    return rgb, {
        "family": FAMILY_NAMES[family],
        "family_index": family,
        "candidate_index": index,
        "seed": seed,
        "base": base,
        "pattern": pattern,
    }


def candidates() -> list[dict[str, object]]:
    """Return ten deterministic families with ten candidates each."""

    result = []
    for family in range(len(FAMILY_NAMES)):
        for index in range(10):
            pixels, metadata = candidate_pixels(family, index)
            result.append(
                {
                    "id": f"luma-eob-bin-f{family + 1:02d}-n{index:02d}",
                    "pixels": pixels,
                    **metadata,
                }
            )
    if len(result) != 100:
        raise AssertionError(f"candidate corpus must contain 100 cases, found {len(result)}")
    return result


def encode(pixels: bytes) -> bytes:
    """Encode one deterministic 4x4 grayscale RGB image as 4:2:0 AVIF."""

    output = BytesIO()
    Image.frombytes("RGB", SIZE, pixels).save(
        output,
        format="AVIF",
        quality=QUALITY,
        speed=SPEED,
        max_threads=1,
        subsampling=SUBSAMPLING,
        autotiling=False,
    )
    return output.getvalue()


def frame_header(path: Path) -> dict[str, object]:
    """Return the single AV1 frame header from one generated AVIF."""

    report = inspect_av1(path)
    samples = report.get("samples", [])
    if len(samples) != 1:
        raise RuntimeError("candidate must contain one AV1 sample")
    headers = [obu["frame_header"] for obu in samples[0]["obus"] if "frame_header" in obu]
    if len(headers) != 1:
        raise RuntimeError("candidate must contain one AV1 frame header")
    return headers[0]


def operation_after_eob_bin(
    entropy: list[dict[str, object]], eob_bin: dict[str, object]
) -> dict[str, object] | None:
    """Return the EOB-base operation following an EOB-bin-two operation."""

    for operation in entropy:
        if (
            operation["step"] > eob_bin["step"]
            and operation["operation"] == "adaptive_symbol"
            and operation["parameter"] == 2
            and len(operation["cdf"]) == 3
        ):
            return operation
    return None


def classify(
    blocks: list[dict[str, int]],
    lines: list[str],
    entropy: list[dict[str, object]],
    yuv: bytes,
    color: dict[str, object],
    header: dict[str, object],
) -> dict[str, object]:
    """Apply the fixed legal syntax predicate to one oracle result."""

    eob_bin_matches = [
        {name: int(value) for name, value in match.groupdict().items()}
        for line in lines
        if (match := EOB_BIN_PATTERN.fullmatch(line)) is not None
    ]
    eob_matches = [
        {name: int(value) for name, value in match.groupdict().items()}
        for line in lines
        if (match := EOB_PATTERN.fullmatch(line)) is not None
    ]
    luma = [
        {name: int(value) for name, value in match.groupdict().items()}
        for line in lines
        if (match := LUMA_PATTERN.match(line)) is not None
    ]
    chroma = [
        {name: int(value) for name, value in match.groupdict().items()}
        for line in lines
        if (match := CHROMA_PATTERN.match(line)) is not None
    ]
    tx_sizes = [
        int(match["tx"])
        for line in lines
        if (match := TX_PATTERN.match(line)) is not None
    ]
    transforms = [
        {name: int(value) for name, value in match.groupdict().items()}
        for line in lines
        if (match := TXTP_PATTERN.match(line)) is not None
    ]
    y_modes = [
        int(match["mode"])
        for line in lines
        if (match := YMODE_PATTERN.match(line)) is not None
    ]
    uv_modes = [
        int(match["mode"])
        for line in lines
        if (match := UVMODE_PATTERN.match(line)) is not None
    ]
    luma_tokens = [
        int(match["token"])
        for line in lines
        if (match := LO_TOKEN_PATTERN.match(line)) is not None
    ]
    first_eob_bin = eob_bin_matches[0] if eob_bin_matches else None
    eob_bin_operation = None
    if first_eob_bin is not None:
        eob_bin_operation = next(
            (
                operation
                for operation in entropy
                if operation["operation"] == "adaptive_symbol"
                and operation["parameter"] == 6
                and len(operation["cdf"]) == 7
                and operation["range"] == first_eob_bin["range"]
            ),
            None,
        )
    eob_base_operation = (
        operation_after_eob_bin(entropy, eob_bin_operation)
        if eob_bin_operation is not None and first_eob_bin["value"] == 2
        else None
    )
    forbidden_prefixes = (
        "Post-filterintramode[",
        "Post-yangle-symbol[",
        "Post-uvangle-symbol[",
        "Post-y_pal[",
        "Post-pal[",
        "Post-y-pal-indices",
        "y-pal-pred",
        "Post-uv_pal[",
        "Post-uv-pal-indices",
        "uv-pal-pred",
    )
    predicates = {
        "frame_is_4x4_8bit_420": (
            color.get("width") == SIZE[0]
            and color.get("height") == SIZE[1]
            and color.get("bit_depth") == 8
            and color.get("monochrome") is False
            and color.get("subsampling_x") is True
            and color.get("subsampling_y") is True
        ),
        "one_tile": (
            header.get("tiling", {}).get("columns") == 1
            and header.get("tiling", {}).get("rows") == 1
        ),
        "origin_unsplit_block": (
            len(blocks) == 1
            and blocks[0]["x"] == 0
            and blocks[0]["y"] == 0
            and blocks[0]["level"] == 4
            and blocks[0]["partition"] == 0
        ),
        "no_optional_prediction_syntax": not any(
            line.startswith(forbidden_prefixes) for line in lines
        ),
        "dc_luma_and_chroma_modes": y_modes == [1] and uv_modes == [0],
        "tx8x8_dct_dct_luma": (
            tx_sizes == [1]
            and transforms
            == [
                {
                    "maximum": 1,
                    "minimum": 1,
                    "symbol": 1,
                    "symbol_value": 1,
                    "txtp": 0,
                }
            ]
            and luma == [{"tx": 1, "txtp": 0, "eob": 3}]
        ),
        "luma_eob_bin_two": first_eob_bin is not None and first_eob_bin["value"] == 2,
        "luma_eob_three": len(eob_matches) == 1 and eob_matches[0]["value"] == 3,
        "luma_eob_base_zero": (
            eob_base_operation is not None and eob_base_operation["value"] == 0
        ),
        "nonempty_luma_ac": any(token > 0 for token in luma_tokens),
        "two_skipped_4x4_chroma": (
            len(chroma) == 2
            and [item["plane"] for item in chroma] == [0, 1]
            and all(item["tx"] == 0 and item["txtp"] == 0 and item["eob"] == -1 for item in chroma)
        ),
        "complete_yuv_output": len(yuv) == EXPECTED_YUV_BYTES,
    }
    reasons = [name for name, passed in predicates.items() if not passed]
    return {
        "partition_blocks": blocks,
        "luma_payloads": luma,
        "chroma_payloads": chroma,
        "transform_records": transforms,
        "luma_tokens": luma_tokens,
        "eob_bin": first_eob_bin,
        "eob_bin_operation": eob_bin_operation,
        "eob_base_operation": eob_base_operation,
        "predicates": predicates,
        "rejection_reasons": reasons,
        "qualifies": not reasons,
    }


def decode_candidate(
    executable: Path,
    environment: dict[str, str],
    work: Path,
    candidate: dict[str, object],
    retain_dir: Path | None,
) -> dict[str, object]:
    """Double-encode and double-decode one candidate through the oracles."""

    pixels = candidate["pixels"]
    assert isinstance(pixels, bytes)
    encoded = encode(pixels)
    encoded_second = encode(pixels)
    if encoded != encoded_second:
        raise RuntimeError(f"nondeterministic Pillow encoding for {candidate['id']}")
    path = work / f"{candidate['id']}.avif"
    second_path = work / f"{candidate['id']}-second.avif"
    path.write_bytes(encoded)
    second_path.write_bytes(encoded_second)
    if retain_dir is not None:
        retain_dir.mkdir(parents=True, exist_ok=True)
        (retain_dir / path.name).write_bytes(encoded)
    item, _ = extract_color_item(path)
    second_item, _ = extract_color_item(second_path)
    if item != second_item:
        raise RuntimeError(f"nondeterministic AV1 item for {candidate['id']}")
    color = portable_color_reference(path)
    header = frame_header(path)
    item_path = work / f"{candidate['id']}.obu"
    item_path.write_bytes(item)

    def trace_once(ordinal: int) -> tuple[str, bytes]:
        yuv_path = work / f"{candidate['id']}-{ordinal}.yuv"
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

    trace_a, yuv_a = trace_once(1)
    trace_b, yuv_b = trace_once(2)
    if trace_a != trace_b or yuv_a != yuv_b:
        raise RuntimeError(f"nondeterministic dav1d trace or YUV for {candidate['id']}")
    lines, _, entropy, blocks, _ = parse_debug_log(trace_a)
    with Image.open(BytesIO(encoded)) as decoded:
        pillow_rgb = decoded.convert("RGB").tobytes()
    with Image.open(BytesIO(encoded_second)) as decoded:
        pillow_rgb_second = decoded.convert("RGB").tobytes()
    if pillow_rgb != pillow_rgb_second:
        raise RuntimeError(f"nondeterministic Pillow RGB decode for {candidate['id']}")
    classification = classify(blocks, lines, entropy, yuv_a, color, header)
    return {
        "id": candidate["id"],
        "family": candidate["family"],
        "family_index": candidate["family_index"],
        "candidate_index": candidate["candidate_index"],
        "seed": candidate["seed"],
        "base": candidate["base"],
        "pattern": candidate["pattern"],
        "quality": QUALITY,
        "speed": SPEED,
        "input_rgb_sha256": sha256(pixels),
        "encoded_file_sha256": sha256(encoded),
        "encoded_file_second_sha256": sha256(encoded_second),
        "encoded_item_sha256": sha256(item),
        "encoded_item_second_sha256": sha256(second_item),
        "encoded_item_length": len(item),
        "pillow_rgb_sha256": sha256(pillow_rgb),
        "pillow_rgb_second_sha256": sha256(pillow_rgb_second),
        "dav1d_trace_sha256": sha256(trace_a.encode()),
        "dav1d_trace_second_sha256": sha256(trace_b.encode()),
        "decoded_yuv_sha256": sha256(yuv_a),
        "decoded_y_plane_sha256": sha256(yuv_a[: SIZE[0] * SIZE[1]]),
        "decoded_u_plane_sha256": sha256(yuv_a[SIZE[0] * SIZE[1] : SIZE[0] * SIZE[1] + 4]),
        "decoded_v_plane_sha256": sha256(yuv_a[SIZE[0] * SIZE[1] + 4 :]),
        "entropy_operation_count": len(entropy),
        "portable_color": color,
        "frame_header": {
            "tiling": header["tiling"],
            "quantization": header["quantization"],
            "delta_q": header["delta_q"],
        },
        "repository_rust_invoked": False,
        **classification,
    }


def main() -> None:
    """Run the pinned one-hundred-candidate input-only campaign."""

    parser = argparse.ArgumentParser(description=__doc__)
    decoder = parser.add_mutually_exclusive_group(required=True)
    decoder.add_argument("--dav1d", type=Path)
    decoder.add_argument("--dav1d-source", type=Path)
    parser.add_argument("--meson", default="meson")
    parser.add_argument("--ninja", default="ninja")
    parser.add_argument("--python-path", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--retain-dir", type=Path)
    args = parser.parse_args()

    if features.version("avif") != "1.4.1":
        raise RuntimeError(f"expected libavif 1.4.1, found {features.version('avif')}")
    codecs = _avif.codec_versions()
    for expected in ("dav1d [dec]:1.5.3", "aom [enc]:3.13.2"):
        if expected not in codecs:
            raise RuntimeError(f"Pillow AVIF oracle lacks {expected}: {codecs}")

    with tempfile.TemporaryDirectory(prefix="image-star-avif-luma-eob-bin-") as name:
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
            )
        else:
            executable = args.dav1d.resolve()
            environment = dict(os.environ)
        version_result = run([str(executable), "--version"], env=environment)
        version = (version_result.stdout + version_result.stderr).strip()
        if not version.startswith("1.5.3-0-gb546257"):
            raise RuntimeError(f"unexpected dav1d executable version: {version}")
        reports = [
            decode_candidate(executable, environment, work, candidate, args.retain_dir)
            for candidate in candidates()
        ]

    common_reasons = (
        "frame_is_4x4_8bit_420",
        "one_tile",
        "origin_unsplit_block",
        "no_optional_prediction_syntax",
        "dc_luma_and_chroma_modes",
        "tx8x8_dct_dct_luma",
        "luma_eob_bin_two",
        "luma_eob_three",
        "luma_eob_base_zero",
        "nonempty_luma_ac",
        "two_skipped_4x4_chroma",
        "complete_yuv_output",
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
        "encoding": {
            "size": list(SIZE),
            "quality": QUALITY,
            "speed": SPEED,
            "max_threads": 1,
            "subsampling": SUBSAMPLING,
            "autotiling": False,
        },
        "search": {
            "input_only": True,
            "repository_rust_invoked": False,
            "candidate_count": len(reports),
            "family_count": len(FAMILY_NAMES),
            "candidates_per_family": 10,
            "target_id": "luma_tx8x8_eob_bin2_eob3_base0",
            "target": (
                "4x4 8-bit 4:2:0 single-frame one-tile origin block; unsplit "
                "TX8x8 DCT_DCT luma with EOB-bin symbol two, EOB three, "
                "direct EOB-base zero, non-empty AC, and skipped 4x4 U/V"
            ),
            "families": list(FAMILY_NAMES),
        },
        "counts": {
            "qualified": sum(bool(case["qualifies"]) for case in reports),
            "qualified_candidates": [case["id"] for case in reports if case["qualifies"]],
            "by_common_rejection_reason": {
                reason: sum(reason in case["rejection_reasons"] for case in reports)
                for reason in common_reasons
            },
        },
        "cases": reports,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(f"Written {len(reports)} deterministic luma EOB-bin traces: {args.output}")


if __name__ == "__main__":
    main()
