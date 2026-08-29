#!/usr/bin/env python3
"""Search for a following 4:2:0 AV1 Horizontal16x4 transform witness.

This is a bounded input-only campaign.  It creates exactly one hundred
deterministic 32x16 RGB candidates in ten named families.  Each candidate is
encoded twice through the pinned Pillow/libavif/libaom oracle and its color
item is decoded twice through an independently built scalar dav1d diagnostic
binary.  A candidate qualifies only when the trace proves a 32x16 split into
two 16x16 H4 children, eight ordered unsplit Horizontal16x4 luma leaves, and
an H_DCT/non-empty luma payload in the top following leaf (the right-hand
16x4 leaf corresponding to the top left-hand leaf).

The report retains every candidate, rejection reason, exact scalar trace/YUV
hash, and the reconstructed left edge used to describe the following leaf's
spatial context.  Repository Rust is never invoked.  Encoded files are
temporary unless ``--retain-dir`` is supplied.
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
    portable_color_reference,
    resolve_tool,
    run,
    verify_source,
)


SIZE = (32, 16)
HALF_SIZE = (16, 16)
SUBSAMPLING = "4:2:0"
EXPECTED_YUV_BYTES = SIZE[0] * SIZE[1] + 2 * (SIZE[0] // 2) * (SIZE[1] // 2)
QUALITY = 99
SPEED = 0
FAMILY_NAMES = (
    "F01_exact_h_dct_pair",
    "F02_left_checker_1",
    "F03_left_checker_2",
    "F04_left_sawtooth",
    "F05_left_prbs",
    "F06_left_row_phase",
    "F07_left_edge_bias",
    "F08_left_sparse_detail",
    "F09_left_banded_detail",
    "F10_left_deterministic_noise",
)
ADVANCED = {
    "min-partition-size": "4",
    "max-partition-size": "16",
    "use-intra-dct-only": "0",
    "enable-filter-intra": "0",
    "enable-intra-edge-filter": "0",
    "enable-smooth-intra": "0",
    "enable-paeth-intra": "0",
    "enable-directional-intra": "0",
    "enable-cfl-intra": "1",
    "enable-cdef": "0",
    "enable-restoration": "0",
    "loopfilter-control": "0",
    "aq-mode": "0",
    "deltaq-mode": "0",
}

# This is the RGB8 source normalization of the already promoted 16x16 H_DCT
# witness.  Keeping the base literal here makes the campaign independent of
# generated AVIF bytes while giving every family a known transform-enabled
# control.  Its digest is checked before any candidate is made.
BASE_RGB = bytes.fromhex(
    "13aa6991f9ce3a4445c8a4b0654037b17c80d381b186285abd626f3c1400a6dd9c71c47457873f667554bfb5da8c75bc34b08463b79d868a93a57f96b38b96deb3c4a9789d86557292655fe8dbb96a9d6ca9eab48baf8144513f453e5db2a1d8"
    "0038367faab1443a5569406b62366b624973b0c2d0719a8a5e885ab9e3b5668d7a203e3e3d4c535e64706b6c7e3b394e647b9d101e3b817e934d3e5b58457b292556789aa61c5345b9f5d16da08250696e262c448c87a55c596a6a7169b5c2ae"
    "5e4e8f716f96b7d1c4416855244c5e8da5c92a29496c66826d788e9dacbfced7e92e2b40301d316f6160b4c092a0b9774d34767e789e94b3a3447558184a4d748ea56b57788161886e668f3134551c1e2dc8c2c477625de5d5beadb5839cb170"
    "ababcf9498b3111b244a585b3d4c53626470d6c2cd705b6a47455a575f6c969e93babb9d463b0feaddb3a29d87a0a09447565b555e659999a3b1a9b8796c7ec7b2c392777ec7b2b536353da6b2b06d7c652b3b1412200091936b3b2927725567"
    "63755bbfcbb5c3c1b489787e3614376a406650293b5a3f448d84856670688fab95add8ba5e947494af9c86656eecacc49bbb8ac3d5ad7d77616b4e50eabadabc8aafa47f916958629ca4af667d8592aba84c6e654174678eaba7785d6ef4bed8"
    "83b87a9ab789857868ffdde198667185586559444b4d56676da1c72b5b89787ca175697f999da040484b7a7b904f4c6a326022849e71c1afa38e656bf4c4d05f3f4c9799a6355268204f795a7dadc5bae57a587bcdadc2a195a1455d67325d64"
    "778456babe9b7c6b61573742cea6c84331596182a5456b88828193c2adbe3921398b6888662f5660445a618f7aa2f5cb967967624d3ab3b29ecccccc7a6a912d285e5c7da82e4a6749363a6f49466e5457a68c97ccacc3a59ea591c3a0a9f6c0"
    "8f3950704244b9d4b180b599263e5c6772a2afb5d96265787a7675f4e8da957e6a9d977d9fc2a437674783a686bdd8b982103bcd919da2cda179c8a076a2bb233364e1daf9685d6bbfc1be84837169543574774c7bbe8858a571829c7f777a67"
)
BASE_RGB_SHA256 = "f3fb754117962b22ac3705b4f18996f1cf6deb1a8728106dfabe65296581dda8"

BLOCK_PATTERN = re.compile(
    r"^poc=(?P<poc>-?\d+),y=(?P<y>-?\d+),x=(?P<x>-?\d+),"
    r"bl=(?P<level>\d+),ctx=(?P<context>\d+),bp=(?P<partition>\d+): "
    r"r=(?P<range>\d+)$"
)
LUMA_PATTERN = re.compile(
    r"^Post-y-cf-blk\[tx=(?P<tx>\d+),txtp=(?P<txtp>-?\d+),"
    r"eob=(?P<eob>-?\d+)\]"
)
CHROMA_PATTERN = re.compile(
    r"^Post-uv-cf-blk\[pl=(?P<plane>\d+),tx=(?P<tx>\d+),"
    r"txtp=(?P<txtp>-?\d+),eob=(?P<eob>-?\d+)\]"
)
YMODE_PATTERN = re.compile(r"^Post-ymode\[(?P<mode>\d+)\]")
UVMODE_PATTERN = re.compile(r"^Post-uvmode\[(?P<mode>\d+)\]")
TX_PATTERN = re.compile(r"^Post-tx\[(?P<tx>\d+)\]")
TXTP_PATTERN = re.compile(
    r"^Post-txtp-intra\[(?P<maximum>\d+)->(?P<minimum>\d+)\]"
    r"\[(?P<context>\d+)\]\[(?P<symbol>\d+)->(?P<txtp>-?\d+)\]"
)


def sha256(data: bytes) -> str:
    """Return the lowercase SHA-256 digest of ``data``."""

    return hashlib.sha256(data).hexdigest()


def clamp(value: int) -> int:
    """Clamp one generated component to the eight-bit range."""

    return max(0, min(255, value))


def half_pixels(family: int, index: int, right: bool) -> bytes:
    """Return one deterministic 16x16 grayscale half."""

    if not right and family == 0 and index == 0:
        return BASE_RGB
    state = random.Random(2_164_000 + 100 * family + index + int(right))
    output = bytearray(BASE_RGB)
    for pixel in range(HALF_SIZE[0] * HALF_SIZE[1]):
        x = pixel % HALF_SIZE[0]
        y = pixel // HALF_SIZE[0]
        if family == 0:
            delta = ((x + index) % 3) - 1
        elif family == 1:
            delta = 1 if (x + index) % 2 else -1
        elif family == 2:
            delta = 2 if (x // 2 + index) % 2 else -2
        elif family == 3:
            delta = ((x + 2 * index) % 5) - 2
        elif family == 4:
            delta = state.choice((-2, -1, 1, 2))
        elif family == 5:
            delta = 1 if (y + index) % 2 else -1
        elif family == 6:
            delta = (3 + index % 3) if x in (0, 7, 15) else -1
        elif family == 7:
            delta = 4 if (x, y) in {(index % 16, (index * 3) % 16)} else 0
        elif family == 8:
            delta = ((y + index) % 4) - 2
        else:
            delta = state.randrange(-3, 4)
        # The right-hand target is kept as the exact H_DCT control.  All
        # variation is on the preceding half so the campaign changes the
        # actual edge supplied to the following block.
        if right:
            delta = 0
        if delta:
            for channel in range(3):
                output[3 * pixel + channel] = clamp(
                    output[3 * pixel + channel] + delta
                )
    return bytes(output)


def candidate_pixels(family: int, index: int) -> bytes:
    """Join a varied preceding half with the exact target half."""

    left = half_pixels(family, index, False)
    right = half_pixels(family, index, True)
    pixels = bytearray()
    row_bytes = HALF_SIZE[0] * 3
    for y in range(HALF_SIZE[1]):
        pixels.extend(left[y * row_bytes : (y + 1) * row_bytes])
        pixels.extend(right[y * row_bytes : (y + 1) * row_bytes])
    return bytes(pixels)


def candidates() -> list[dict[str, object]]:
    """Return exactly ten named families with ten candidates each."""

    if len(BASE_RGB) != HALF_SIZE[0] * HALF_SIZE[1] * 3:
        raise AssertionError("base control must be one 16x16 RGB image")
    if sha256(BASE_RGB) != BASE_RGB_SHA256:
        raise AssertionError("base H_DCT control digest changed")
    result = []
    for family, family_name in enumerate(FAMILY_NAMES):
        for index in range(10):
            result.append(
                {
                    "id": f"h16x4-following-f{family + 1:02d}-n{index:02d}",
                    "family": family_name,
                    "family_index": family,
                    "candidate_index": index,
                    "seed": 2_164_000 + 100 * family + index,
                    "quality": QUALITY,
                    "speed": SPEED,
                    "pixels": candidate_pixels(family, index),
                }
            )
    if len(result) != 100:
        raise AssertionError(f"candidate corpus must contain 100 cases, found {len(result)}")
    if any(
        sum(candidate["family"] == family for candidate in result) != 10
        for family in FAMILY_NAMES
    ):
        raise AssertionError("each campaign family must contain exactly ten cases")
    return result


def encode(pixels: bytes) -> bytes:
    """Encode one candidate through the pinned Pillow AVIF oracle."""

    output = BytesIO()
    Image.frombytes("RGB", SIZE, pixels).save(
        output,
        format="AVIF",
        quality=QUALITY,
        speed=SPEED,
        max_threads=1,
        subsampling=SUBSAMPLING,
        autotiling=False,
        advanced=ADVANCED,
    )
    return output.getvalue()


def parse_trace(output: str) -> tuple[list[dict[str, int]], list[list[str]], int]:
    """Parse partition headers, terminal groups, and contiguous scalar entropy."""

    lines = [line.rstrip() for line in output.splitlines() if line.strip()]
    entropy = [
        json.loads(line.removeprefix("@MSAC "))
        for line in lines
        if line.startswith("@MSAC ")
    ]
    if not entropy or entropy[0].get("operation") != "init":
        raise RuntimeError("missing scalar MSAC trace")
    if [operation["step"] for operation in entropy] != list(range(len(entropy))):
        raise RuntimeError("non-contiguous scalar MSAC trace")
    debug = [line for line in lines if not line.startswith("@MSAC ")]
    blocks = []
    for line in debug:
        if match := BLOCK_PATTERN.fullmatch(line):
            blocks.append({name: int(value) for name, value in match.groupdict().items()})
    groups: list[list[str]] = []
    for line in debug:
        if line.startswith("Post-skip["):
            groups.append([])
        if groups:
            groups[-1].append(line)
    if not blocks:
        raise RuntimeError("missing partition trace")
    return blocks, groups, len(entropy)


def parse_group(group: list[str]) -> dict[str, object]:
    """Extract public modes, transform syntax, and residual payloads."""

    y_modes = [
        int(match["mode"])
        for line in group
        if (match := YMODE_PATTERN.match(line)) is not None
    ]
    uv_modes = [
        int(match["mode"])
        for line in group
        if (match := UVMODE_PATTERN.match(line)) is not None
    ]
    tx_sizes = [
        int(match["tx"])
        for line in group
        if (match := TX_PATTERN.match(line)) is not None
    ]
    transform_records = [
        {name: int(value) for name, value in match.groupdict().items()}
        for line in group
        if (match := TXTP_PATTERN.match(line)) is not None
    ]
    luma_payloads = [
        {name: int(value) for name, value in match.groupdict().items()}
        for line in group
        if (match := LUMA_PATTERN.match(line)) is not None
    ]
    chroma_payloads = [
        {name: int(value) for name, value in match.groupdict().items()}
        for line in group
        if (match := CHROMA_PATTERN.match(line)) is not None
    ]
    forbidden = any(
        line.startswith(
            (
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
        )
        for line in group
    )
    return {
        "y_modes": y_modes,
        "uv_modes": uv_modes,
        "tx_sizes": tx_sizes,
        "transform_records": transform_records,
        "luma_payloads": luma_payloads,
        "chroma_payloads": chroma_payloads,
        "forbidden_syntax": forbidden,
    }


def h_dct_nonempty(leaf: dict[str, object]) -> bool:
    """Return whether one terminal group proves unsplit H_DCT luma."""

    return (
        leaf["tx_sizes"] == [14]
        and len(leaf["luma_payloads"]) == 1
        and leaf["luma_payloads"][0]["tx"] == 14
        and leaf["luma_payloads"][0]["txtp"] == 11
        and leaf["luma_payloads"][0]["eob"] >= 0
        and any(
            record["symbol"] == 3 and record["txtp"] == 11
            for record in leaf["transform_records"]
        )
    )


def classify(
    blocks: list[dict[str, int]],
    groups: list[list[str]],
    yuv: bytes,
    portable_color: dict[str, object],
) -> dict[str, object]:
    """Apply exact frame, topology, and following-leaf predicates."""

    parsed = [parse_group(group) for group in groups]
    root = next(
        (
            block
            for block in blocks
            if block["level"] == 2
            and block["x"] == 0
            and block["y"] == 0
            and block["partition"] == 3
        ),
        None,
    )
    children = sorted(
        (
            block
            for block in blocks
            if block["level"] == 3 and block["y"] == 0 and block["partition"] == 8
        ),
        key=lambda block: block["x"],
    )
    left_groups = parsed[:4] if len(parsed) == 8 else []
    right_groups = parsed[4:] if len(parsed) == 8 else []
    target = right_groups[0] if right_groups else {}
    common = {
        "frame_is_32x16_8bit_420": (
            portable_color.get("width") == SIZE[0]
            and portable_color.get("height") == SIZE[1]
            and portable_color.get("bit_depth") == 8
            and portable_color.get("monochrome") is False
            and portable_color.get("subsampling_x") is True
            and portable_color.get("subsampling_y") is True
        ),
        "root_is_32x16_partition_split": (
            root is not None
            and len(blocks) == 3
            and root["poc"] == 0
            and root["context"] == 0
        ),
        "two_16x16_h4_children": (
            [(block["x"], block["context"]) for block in children]
            == [(0, 0), (4, 2)]
            and len(children) == 2
        ),
        "eight_ordered_leaf_groups": len(groups) == 8,
        "all_leaves_unsplit_h16x4": bool(parsed)
        and all(leaf["tx_sizes"] == [14] for leaf in parsed),
        "no_filter_palette_or_angle_syntax": bool(parsed)
        and not any(leaf["forbidden_syntax"] for leaf in parsed),
        "following_target_is_top_right_correspondent": bool(target)
        and len(left_groups) == 4
        and len(right_groups) == 4,
        "following_target_h_dct_nonempty": bool(target) and h_dct_nonempty(target),
        "full_yuv_output": len(yuv) == EXPECTED_YUV_BYTES,
        "nonconstant_luma": len(set(yuv[: SIZE[0] * SIZE[1]])) > 1,
    }
    target_index = 4
    y_plane = yuv[: SIZE[0] * SIZE[1]]
    target_y = 0
    target_left_edge = [y_plane[row * SIZE[0] + 15] for row in range(4)]
    chroma_plane_size = (SIZE[0] // 2) * (SIZE[1] // 2)
    u_plane = yuv[SIZE[0] * SIZE[1] : SIZE[0] * SIZE[1] + chroma_plane_size]
    v_plane = yuv[SIZE[0] * SIZE[1] + chroma_plane_size :]
    chroma_left_edges = {
        "u": [u_plane[row * (SIZE[0] // 2) + 7] for row in range(2)],
        "v": [v_plane[row * (SIZE[0] // 2) + 7] for row in range(2)],
    }
    return {
        "root_partition": root,
        "child_partitions": children,
        "group_count": len(groups),
        "left_group_count": len(left_groups),
        "right_group_count": len(right_groups),
        "left_transforms": [leaf["transform_records"] for leaf in left_groups],
        "right_transforms": [leaf["transform_records"] for leaf in right_groups],
        "left_luma_payloads": [leaf["luma_payloads"] for leaf in left_groups],
        "right_luma_payloads": [leaf["luma_payloads"] for leaf in right_groups],
        "right_chroma_payloads": [leaf["chroma_payloads"] for leaf in right_groups],
        "target_leaf_group": target_index,
        "target_leaf_geometry": {
            "pixel_x": 16,
            "pixel_y": target_y,
            "width": 16,
            "height": 4,
            "left_correspondent_group": 0,
            "right_group": target_index,
            "order": "left H4 rows 0..3, then right H4 rows 0..3",
        },
        "following_edge_context": {
            "has_top": False,
            "has_left": True,
            "top_left": target_left_edge[0],
            "left_luma_edge_x15_rows0_to3": target_left_edge,
            "top_edge_extension": [target_left_edge[0]] * 16,
            "chroma_left_edges_x7_rows0_to1": chroma_left_edges,
            "single_tile_controls": True,
        },
        "common_predicates": common,
        "rejection_reasons": [name for name, passed in common.items() if not passed],
        "qualifies": all(common.values()),
    }


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
    encoded = encode(pixels)
    encoded_second = encode(pixels)
    if encoded != encoded_second:
        raise RuntimeError(f"nondeterministic encoding for {candidate['id']}")
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
        raise RuntimeError(f"nondeterministic color item for {candidate['id']}")
    portable_color = portable_color_reference(path)
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
    blocks, groups, entropy_count = parse_trace(trace_a)
    with Image.open(BytesIO(encoded)) as decoded:
        pillow_rgb = decoded.convert("RGB").tobytes()
    with Image.open(BytesIO(encoded_second)) as decoded:
        pillow_rgb_second = decoded.convert("RGB").tobytes()
    if pillow_rgb != pillow_rgb_second:
        raise RuntimeError(f"nondeterministic Pillow RGB decode for {candidate['id']}")
    if len(yuv_a) != EXPECTED_YUV_BYTES:
        raise RuntimeError(f"unexpected 4:2:0 YUV length for {candidate['id']}: {len(yuv_a)}")
    y_plane_bytes = SIZE[0] * SIZE[1]
    chroma_plane_bytes = (SIZE[0] // 2) * (SIZE[1] // 2)
    return {
        "id": candidate["id"],
        "family": candidate["family"],
        "family_index": candidate["family_index"],
        "candidate_index": candidate["candidate_index"],
        "seed": candidate["seed"],
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
        "decoded_y_plane_sha256": sha256(yuv_a[:y_plane_bytes]),
        "decoded_u_plane_sha256": sha256(
            yuv_a[y_plane_bytes : y_plane_bytes + chroma_plane_bytes]
        ),
        "decoded_v_plane_sha256": sha256(yuv_a[y_plane_bytes + chroma_plane_bytes :]),
        "entropy_operation_count": entropy_count,
        "partition_blocks": blocks,
        "repository_rust_invoked": False,
        **classify(blocks, groups, yuv_a, portable_color),
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

    with tempfile.TemporaryDirectory(prefix="image-star-avif-h16x4-following-") as name:
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
                broaden_vertical_following=False,
                broaden_horizontal_square16=True,
            )
        else:
            executable = args.dav1d.resolve()
            environment = {}
        version_result = run([str(executable), "--version"], env=environment)
        version = (version_result.stdout + version_result.stderr).strip()
        if not version.startswith("1.5.3-0-gb546257"):
            raise RuntimeError(f"unexpected dav1d executable version: {version}")
        reports = [
            decode_candidate(executable, environment, work, candidate, args.retain_dir)
            for candidate in candidates()
        ]

    common_reasons = (
        "frame_is_32x16_8bit_420",
        "root_is_32x16_partition_split",
        "two_16x16_h4_children",
        "eight_ordered_leaf_groups",
        "all_leaves_unsplit_h16x4",
        "no_filter_palette_or_angle_syntax",
        "following_target_is_top_right_correspondent",
        "following_target_h_dct_nonempty",
        "full_yuv_output",
        "nonconstant_luma",
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
            "subsampling": SUBSAMPLING,
            "quality": QUALITY,
            "speed": SPEED,
            "max_threads": 1,
            "autotiling": False,
            "advanced": ADVANCED,
            "base_control_rgb_sha256": BASE_RGB_SHA256,
        },
        "search": {
            "input_only": True,
            "repository_rust_invoked": False,
            "candidate_count": len(reports),
            "family_count": len(FAMILY_NAMES),
            "candidates_per_family": 10,
            "target_id": "following_horizontal16x4_h_dct",
            "target": (
                "32x16 4:2:0 root PARTITION_SPLIT with two level-3 16x16 "
                "PARTITION_H4 children and eight ordered unsplit Horizontal16x4 "
                "luma leaves; the top right-hand leaf is a non-empty H_DCT "
                "correspondent to the top left-hand leaf"
            ),
            "families": list(FAMILY_NAMES),
            "edge_observation": (
                "For the top following leaf, has_top=false and has_left=true; "
                "the report records the reconstructed x=15 left edge and the "
                "synthetic top extension derived from its top-left sample."
            ),
        },
        "counts": {
            "qualified": sum(bool(report["qualifies"]) for report in reports),
            "qualified_candidates": [
                report["id"] for report in reports if report["qualifies"]
            ],
            "promoted_candidate": (
                min(
                    (report for report in reports if report["qualifies"]),
                    key=lambda report: (
                        int(report["entropy_operation_count"]),
                        int(report["encoded_item_length"]),
                        str(report["id"]),
                    ),
                    default=None,
                )
                or {}
            ).get("id"),
            "by_common_rejection_reason": {
                reason: sum(reason in report["rejection_reasons"] for report in reports)
                for reason in common_reasons
            },
        },
        "cases": reports,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(f"Written {len(reports)} deterministic following Horizontal16x4 traces: {args.output}")


if __name__ == "__main__":
    main()
