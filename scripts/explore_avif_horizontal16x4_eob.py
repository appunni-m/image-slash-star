#!/usr/bin/env python3
"""Search the proven H16x4 H_DCT topology for a new luma EOB sentence.

This is an input-only oracle campaign.  It reuses the maintained following
H16x4 generator and scalar dav1d trace path, but varies the target luma signal
instead of only varying the preceding edge.  A candidate is promoted by this
campaign only when it retains the exact eight-leaf following topology, has a
non-empty H_DCT target luma payload, and emits an EOB-bin/base sentence not
present in the committed reconstruction references.  Repository Rust is never
invoked.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
import tempfile
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPT_DIR))

import explore_avif_horizontal16x4_following as following
from PIL import Image, _avif, features


FAMILY_NAMES = (
    "F01_contrast_ramp",
    "F02_contrast_floor",
    "F03_horizontal_phase",
    "F04_vertical_phase",
    "F05_sparse_high_frequency",
    "F06_low_frequency_rows",
    "F07_edge_preserving",
    "F08_checker_perturbation",
    "F09_deterministic_noise",
    "F10_control_to_flat",
)
QUALITY = following.QUALITY
SPEED = following.SPEED
SIZE = following.SIZE
HALF_SIZE = following.HALF_SIZE
SUBSAMPLING = following.SUBSAMPLING
BASE_RGB = following.BASE_RGB
BASE_RGB_SHA256 = following.BASE_RGB_SHA256
ADVANCED = following.ADVANCED
EXPECTED_YUV_BYTES = following.EXPECTED_YUV_BYTES

EOB_BIN_PATTERN = re.compile(
    r"^Post-eob_bin_(?P<base>\d+)(?P<args>(?:\[-?\d+\])+):"
)
EOB_HI_PATTERN = re.compile(r"^Post-eob_hi_bit(?P<args>(?:\[-?\d+\])+):")
EOB_PATTERN = re.compile(r"^Post-eob\[(?P<eob>-?\d+)\]:")


def sha256(data: bytes) -> str:
    """Return the lowercase SHA-256 digest of ``data``."""

    return hashlib.sha256(data).hexdigest()


def channel_means() -> tuple[int, int, int]:
    """Return deterministic per-channel centers for contrast scaling."""

    return tuple(sum(BASE_RGB[channel::3]) // (HALF_SIZE[0] * HALF_SIZE[1]) for channel in range(3))


MEANS = channel_means()


def blend(value: int, center: int, numerator: int, denominator: int = 10) -> int:
    """Blend one byte toward its center using integer arithmetic."""

    delta = value - center
    return max(0, min(255, center + (delta * numerator + denominator // 2) // denominator))


def target_pixels(family: int, index: int) -> bytes:
    """Create one target half while retaining a controlled H_DCT signal."""

    output = bytearray(BASE_RGB)
    scale = index + 1
    for pixel in range(HALF_SIZE[0] * HALF_SIZE[1]):
        x = pixel % HALF_SIZE[0]
        y = pixel // HALF_SIZE[0]
        for channel in range(3):
            offset = 3 * pixel + channel
            value = BASE_RGB[offset]
            if family in (0, 9):
                value = blend(value, MEANS[channel], scale)
            elif family == 1:
                value = blend(value, MEANS[channel], max(1, 2 * scale - 1))
            elif family == 2:
                value = blend(value, MEANS[channel], scale)
                value = max(0, min(255, value + ((x + index) % 3 - 1)))
            elif family == 3:
                value = blend(value, MEANS[channel], scale)
                value = max(0, min(255, value + ((y + index) % 3 - 1)))
            elif family == 4:
                value = blend(value, MEANS[channel], scale)
                if x in (0, 7, 15):
                    value = max(0, min(255, value + (2 if index % 2 else -2)))
            elif family == 5:
                row_scale = max(1, (scale + (y % 4)) // 2)
                value = blend(value, MEANS[channel], row_scale)
            elif family == 6:
                value = blend(value, MEANS[channel], scale)
                if x == 15:
                    value = max(0, min(255, value + (index % 5 - 2)))
            elif family == 7:
                value = blend(value, MEANS[channel], scale)
                if (x + y + index) % 2 == 0:
                    value = max(0, min(255, value + 1))
            elif family == 8:
                value = blend(value, MEANS[channel], scale)
                value = max(0, min(255, value + ((pixel * 17 + index) % 5 - 2)))
            output[offset] = value
    return bytes(output)


def candidate_pixels(family: int, index: int) -> bytes:
    """Join the existing varied left half with the new target half."""

    left = following.half_pixels(family, index, False)
    right = target_pixels(family, index)
    output = bytearray()
    row_bytes = HALF_SIZE[0] * 3
    for row in range(HALF_SIZE[1]):
        output.extend(left[row * row_bytes : (row + 1) * row_bytes])
        output.extend(right[row * row_bytes : (row + 1) * row_bytes])
    return bytes(output)


def candidates() -> list[dict[str, object]]:
    """Return exactly ten deterministic families with ten cases each."""

    if sha256(BASE_RGB) != BASE_RGB_SHA256:
        raise AssertionError("base H_DCT control digest changed")
    result = []
    for family, family_name in enumerate(FAMILY_NAMES):
        for index in range(10):
            result.append(
                {
                    "id": f"h16x4-eob-f{family + 1:02d}-n{index:02d}",
                    "family": family_name,
                    "family_index": family,
                    "candidate_index": index,
                    "seed": 3_164_000 + 100 * family + index,
                    "quality": QUALITY,
                    "speed": SPEED,
                    "pixels": candidate_pixels(family, index),
                }
            )
    if len(result) != 100:
        raise AssertionError(f"candidate corpus must contain 100 cases, found {len(result)}")
    if any(sum(candidate["family"] == family for candidate in result) != 10 for family in FAMILY_NAMES):
        raise AssertionError("each campaign family must contain exactly ten cases")
    return result


def bracket_values(value: str) -> list[int]:
    """Parse an integer bracket suffix from an oracle trace line."""

    return [int(item) for item in re.findall(r"\[(-?\d+)\]", value)]


def eob_sentence(group: list[str]) -> dict[str, object] | None:
    """Return the target luma EOB-bin/base sentence from one leaf group."""

    luma_payloads = [
        following.LUMA_PATTERN.match(line)
        for line in group
        if following.LUMA_PATTERN.match(line) is not None
    ]
    if not any(match is not None and int(match["txtp"]) == 11 for match in luma_payloads):
        return None
    bins = []
    hi_bits = []
    eobs = []
    for line in group:
        if match := EOB_BIN_PATTERN.match(line):
            bins.append(
                {
                    "base": int(match["base"]),
                    "args": bracket_values(match["args"]),
                }
            )
        if match := EOB_HI_PATTERN.match(line):
            hi_bits.append(bracket_values(match["args"]))
        if match := EOB_PATTERN.match(line):
            eobs.append(int(match["eob"]))
    payload = next(
        match for match in luma_payloads if match is not None and int(match["txtp"]) == 11
    )
    eob = int(payload["eob"])
    if eob < 0 or not bins:
        return None
    return {
        "bin": bins,
        "hi_bits": hi_bits,
        "eob": eob,
        "signature": (
            tuple((entry["base"], tuple(entry["args"])) for entry in bins),
            tuple(tuple(values) for values in hi_bits),
            eob,
        ),
        "payload": {name: int(value) for name, value in payload.groupdict().items()},
        "operation_lines": [
            line
            for line in group
            if "eob_bin_" in line or line.startswith("Post-eob_hi_bit[") or line.startswith("Post-eob[")
        ],
    }


def log_groups(log: list[object]) -> list[list[str]]:
    """Split a committed reconstruction debug log into leaf groups."""

    groups: list[list[str]] = []
    for line in log:
        if not isinstance(line, str):
            continue
        if line.startswith("Post-skip["):
            groups.append([])
        if groups:
            groups[-1].append(line)
    return groups


def existing_signatures(path: Path) -> set[object]:
    """Collect active H_DCT luma EOB signatures from reconstruction references."""

    data = json.loads(path.read_text())
    signatures = set()
    for case in data["cases"]:
        for group in log_groups(case.get("dav1d_debug_log", [])):
            sentence = eob_sentence(group)
            if sentence is not None:
                signatures.add(sentence["signature"])
    return signatures


def target_trace(
    executable: Path, environment: dict[str, str], work: Path, candidate_id: str
) -> tuple[list[list[str]], str]:
    """Trace the retained candidate item once for EOB sentence evidence."""

    item_path = work / f"{candidate_id}.obu"
    yuv_path = work / f"{candidate_id}-eob.yuv"
    result = following.run(
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
    if len(yuv_path.read_bytes()) != EXPECTED_YUV_BYTES:
        raise RuntimeError(f"unexpected YUV length for {candidate_id}")
    _, groups, _ = following.parse_trace(result.stdout)
    return groups, result.stdout


def decode_candidate(
    executable: Path,
    environment: dict[str, str],
    work: Path,
    candidate: dict[str, object],
    retain_dir: Path | None,
    known_signatures: set[object],
) -> dict[str, object]:
    """Run the maintained topology checks and classify EOB novelty."""

    report = following.decode_candidate(executable, environment, work, candidate, retain_dir)
    groups, trace = target_trace(executable, environment, work, str(candidate["id"]))
    target = eob_sentence(groups[4]) if len(groups) == 8 else None
    topology_qualifies = bool(report["qualifies"])
    novel = target is not None and target["signature"] not in known_signatures
    report.update(
        {
            "topology_qualifies": topology_qualifies,
            "eob_trace_sha256": sha256(trace.encode()),
            "target_luma_eob_sentence": target,
            "novel_luma_eob_sentence": novel,
            "qualifies": topology_qualifies and novel,
            "rejection_reasons": [
                *report["rejection_reasons"],
                *([] if target is not None else ["target_luma_h_dct_nonempty"]),
                *([] if novel else ["target_luma_eob_sentence_already_represented"]),
            ],
        }
    )
    return report


def main() -> None:
    """Run the bounded one-hundred-candidate input-only campaign."""

    parser = argparse.ArgumentParser(description=__doc__)
    decoder = parser.add_mutually_exclusive_group(required=True)
    decoder.add_argument("--dav1d", type=Path)
    decoder.add_argument("--dav1d-source", type=Path)
    parser.add_argument("--meson", default="meson")
    parser.add_argument("--ninja", default="ninja")
    parser.add_argument("--python-path", type=Path)
    parser.add_argument("--active-reference", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--retain-dir", type=Path)
    args = parser.parse_args()

    if features.version("avif") != "1.4.1":
        raise RuntimeError(f"expected libavif 1.4.1, found {features.version('avif')}")
    codecs = _avif.codec_versions()
    for expected in ("dav1d [dec]:1.5.3", "aom [enc]:3.13.2"):
        if expected not in codecs:
            raise RuntimeError(f"Pillow AVIF oracle lacks {expected}: {codecs}")
    known = existing_signatures(args.active_reference.resolve())

    with tempfile.TemporaryDirectory(prefix="image-star-avif-h16x4-eob-") as name:
        work = Path(name)
        if args.dav1d_source is not None:
            source = args.dav1d_source.resolve()
            following.verify_source(source)
            executable, environment = following.build_dav1d(
                source,
                work,
                following.resolve_tool(args.meson, "Meson"),
                following.resolve_tool(args.ninja, "Ninja"),
                args.python_path.resolve() if args.python_path else None,
                broaden_vertical_following=False,
                broaden_horizontal_square16=True,
            )
        else:
            executable = args.dav1d.resolve()
            environment = {}
        version_result = following.run([str(executable), "--version"], env=environment)
        version = (version_result.stdout + version_result.stderr).strip()
        if not version.startswith("1.5.3-0-gb546257"):
            raise RuntimeError(f"unexpected dav1d executable version: {version}")
        reports = [
            decode_candidate(executable, environment, work, candidate, args.retain_dir, known)
            for candidate in candidates()
        ]

    reason_names = sorted({reason for report in reports for reason in report["rejection_reasons"]})
    report = {
        "format_version": 1,
        "oracle": {
            "pillow": Image.__version__,
            "libavif": features.version("avif"),
            "codecs": codecs,
            "dav1d": version,
            "dav1d_commit": following.DAV1D_COMMIT,
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
            "target_id": "h16x4_h_dct_novel_eob_sentence",
            "target": (
                "32x16 4:2:0 root PARTITION_SPLIT with two 16x16 H4 children, "
                "eight ordered unsplit Horizontal16x4 leaves, and a non-empty "
                "H_DCT target luma sentence whose EOB-bin/base signature is "
                "absent from active H16x4 reconstruction references"
            ),
            "families": list(FAMILY_NAMES),
            "known_active_luma_eob_signature_count": len(known),
            "known_active_luma_eob_signatures": [repr(signature) for signature in sorted(known, key=repr)],
        },
        "counts": {
            "qualified": sum(bool(report["qualifies"]) for report in reports),
            "topology_qualified": sum(bool(report["topology_qualifies"]) for report in reports),
            "novel_sentence_candidates": sum(bool(report["novel_luma_eob_sentence"]) for report in reports),
            "qualified_candidates": [report["id"] for report in reports if report["qualifies"]],
            "by_rejection_reason": {
                reason: sum(reason in report["rejection_reasons"] for report in reports)
                for reason in reason_names
            },
            "by_target_signature": {
                repr(signature): sum(
                    report.get("target_luma_eob_sentence", {}).get("signature") == signature
                    for report in reports
                    if report.get("target_luma_eob_sentence") is not None
                )
                for signature in sorted(
                    {
                        report["target_luma_eob_sentence"]["signature"]
                        for report in reports
                        if report.get("target_luma_eob_sentence") is not None
                    },
                    key=repr,
                )
            },
        },
        "active_reference": {
            "path": str(args.active_reference),
            "sha256": sha256(args.active_reference.read_bytes()),
        },
        "cases": reports,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(f"Written {len(reports)} deterministic H16x4 EOB traces: {args.output}")


if __name__ == "__main__":
    main()
