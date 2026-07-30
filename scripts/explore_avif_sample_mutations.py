#!/usr/bin/env python3
"""Reverse-map AV1 parser states with deterministic one-byte AVIF mutations.

The script mutates only the extracted color AV1 item of one retained AVIF,
traces every candidate through the pinned scalar dav1d build, and retains the
first mutation for each requested Slice 35 rejection state. It is a diagnostic
fixture-selection tool, not a fuzzer and not part of the crate.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import tempfile
from io import BytesIO
from pathlib import Path

from PIL import Image, _avif, features

from generate_av1_reconstruction_refs import (
    DAV1D_COMMIT,
    build_dav1d,
    extract_color_item,
    resolve_tool,
    verify_source,
)
from inspect_av1_obus import inspect as inspect_av1


SLICE_35_PREFIX = (
    ("init", -1),
    ("adaptive_symbol", 0),
    ("fixed", 0),
    ("adaptive_bool", 0),
    ("adaptive_symbol", 2),
    ("equal", 1),
    ("adaptive_symbol", 1),
    ("adaptive_symbol", 3),
    ("adaptive_symbol", 0),
    ("fixed", 0),
    ("adaptive_bool", 0),
    ("adaptive_symbol", 1),
)
DC_RESIDUAL_PATTERN = re.compile(r"^Post-dc_residual\[\d+->(?P<token>\d+)\]:")
DC_SIGN_PATTERN = re.compile(r"^Post-dc_sign\[0\]\[0\]\[(?P<negative>[01])\]:")
LUMA_MODE_PATTERN = re.compile(r"^Post-ymode\[(?P<mode>[12])\]:")


def sha256(data: bytes) -> str:
    """Return the lowercase SHA-256 digest of ``data``."""

    return hashlib.sha256(data).hexdigest()


def entropy_operations(output: str) -> list[dict[str, object]]:
    """Parse the contiguous scalar entropy operations from dav1d output."""

    operations = [
        json.loads(line.removeprefix("@MSAC "))
        for line in output.splitlines()
        if line.startswith("@MSAC ")
    ]
    if operations and [
        operation["step"] for operation in operations
    ] != list(range(len(operations))):
        raise RuntimeError("non-contiguous scalar MSAC mutation trace")
    return operations


def rejection_stage(operations: list[dict[str, object]]) -> str | None:
    """Classify a trace that reaches one of the Slice 35 rejection states."""

    if len(operations) <= len(SLICE_35_PREFIX):
        return None
    actual_prefix = tuple(
        (str(operation["operation"]), int(operation["value"]))
        for operation in operations[: len(SLICE_35_PREFIX)]
    )
    if actual_prefix != SLICE_35_PREFIX:
        return None
    eob_bin = operations[len(SLICE_35_PREFIX)]
    if eob_bin["operation"] != "adaptive_symbol":
        return None
    if eob_bin["value"] != 0:
        return "eob_bin"
    if len(operations) <= len(SLICE_35_PREFIX) + 1:
        return None
    eob_base = operations[len(SLICE_35_PREFIX) + 1]
    if eob_base["operation"] == "adaptive_symbol" and eob_base["value"] in (0, 1):
        return "eob_base"
    return None


def closed_dc_final_token(output: str) -> int | None:
    """Return the first-leaf final DC token for the retained Slice 39 class."""

    lines = output.splitlines()
    required = (
        "Post-skip[0]:",
        "Post-cdef_idx[0]:",
        "Post-delta_q[-2->2]:",
        "Post-uvmode[0]:",
        "Post-tx[1]:",
        "Post-non-zero[1][0][0]:",
        "Post-eob_bin_64[0][0][0]:",
        "Post-dc_lo_tok[1][0][0][3]:",
        "Post-dc_hi_tok[1][0][0][15]:",
        "Post-y-cf-blk[tx=1,txtp=0,eob=0]:",
        "Post-uv-cf-blk[pl=0,tx=0,txtp=0,eob=-1]:",
        "Post-uv-cf-blk[pl=1,tx=0,txtp=0,eob=-1]:",
    )
    if not all(any(line.startswith(prefix) for line in lines) for prefix in required):
        return None
    if not any(LUMA_MODE_PATTERN.match(line) for line in lines):
        return None
    if not any(
        line.startswith("Post-txtp-intra[")
        and "->1][1][1->0]:" in line
        for line in lines
    ):
        return None
    if not any(DC_SIGN_PATTERN.match(line) for line in lines):
        return None
    for line in lines:
        if match := DC_RESIDUAL_PATTERN.match(line):
            return int(match.group("token"))
    return None


def run_dav1d(
    executable: Path,
    environment: dict[str, str],
    sample_path: Path,
    output_path: Path,
) -> tuple[subprocess.CompletedProcess[str], list[dict[str, object]]]:
    """Decode one possibly malformed AV1 sample and retain its scalar trace."""

    if output_path.exists():
        output_path.unlink()
    result = subprocess.run(
        [
            str(executable),
            "--input",
            str(sample_path),
            "--demuxer",
            "section5",
            "--output",
            str(output_path),
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
        check=False,
        env=environment,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    return result, entropy_operations(result.stdout)


def pillow_outcome(data: bytes) -> dict[str, object]:
    """Return an exact Pillow success or error classification."""

    try:
        with Image.open(BytesIO(data)) as image:
            image.load()
            pixels = image.tobytes()
            return {
                "status": "ok",
                "format": image.format,
                "mode": image.mode,
                "size": list(image.size),
                "pixels_sha256": sha256(pixels),
            }
    except Exception as error:  # Pillow's plugin boundary exposes several error types.
        return {
            "status": "error",
            "type": type(error).__name__,
            "message": str(error),
        }


def candidate_record(
    executable: Path,
    environment: dict[str, str],
    work: Path,
    original_file: bytes,
    original_sample: bytes,
    item_offset: int,
    sample_offset: int,
    replacement: int,
) -> tuple[bytes, dict[str, object]]:
    """Re-run and fully classify one selected deterministic mutation."""

    mutated_sample = bytearray(original_sample)
    old = mutated_sample[sample_offset]
    mutated_sample[sample_offset] = replacement
    mutated_file = bytearray(original_file)
    mutated_file[item_offset + sample_offset] = replacement
    sample_path = work / "selected.obu"
    output_path = work / "selected.yuv"
    sample_path.write_bytes(mutated_sample)
    result, operations = run_dav1d(
        executable,
        environment,
        sample_path,
        output_path,
    )
    decoded = output_path.read_bytes() if output_path.exists() else b""
    dc_final_token = closed_dc_final_token(result.stdout)
    record = {
        "stage": rejection_stage(operations),
        "dc_final_token": dc_final_token,
        "sample_offset": sample_offset,
        "file_offset": item_offset + sample_offset,
        "old_byte": old,
        "new_byte": replacement,
        "file_length": len(mutated_file),
        "file_sha256": sha256(mutated_file),
        "sample_length": len(mutated_sample),
        "sample_sha256": sha256(mutated_sample),
        "dav1d_returncode": result.returncode,
        "dav1d_stdout_sha256": sha256(result.stdout.encode()),
        "dav1d_stderr": result.stderr.strip(),
        "decoded_yuv_length": len(decoded),
        "decoded_yuv_sha256": sha256(decoded),
        "entropy_operations": operations,
        "pillow": pillow_outcome(mutated_file),
    }
    return bytes(mutated_file), record


def main() -> None:
    """Run the deterministic AV1 sample mutation sweep."""

    parser = argparse.ArgumentParser(description=__doc__)
    decoder = parser.add_mutually_exclusive_group(required=True)
    decoder.add_argument("--dav1d", type=Path)
    decoder.add_argument("--dav1d-source", type=Path)
    parser.add_argument("--meson", default="meson")
    parser.add_argument("--ninja", default="ninja")
    parser.add_argument("--python-path", type=Path)
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--retain-dir", type=Path)
    parser.add_argument(
        "--dc-token-min",
        type=int,
        help=(
            "select a successful closed-class final DC token above this value "
            "instead of the Slice 35 EOB controls"
        ),
    )
    parser.add_argument(
        "--dc-token-selection",
        choices=("smallest", "largest"),
        default="smallest",
        help=(
            "choose the smallest or largest matching final DC token; applies "
            "only with --dc-token-min (default: smallest)"
        ),
    )
    args = parser.parse_args()
    if args.dc_token_min is not None and args.dc_token_min < 0:
        parser.error("--dc-token-min must be non-negative")
    if args.dc_token_min is None and args.dc_token_selection != "smallest":
        parser.error("--dc-token-selection requires --dc-token-min")

    if features.version("avif") != "1.4.1":
        raise RuntimeError(f"expected libavif 1.4.1, found {features.version('avif')}")
    codecs = _avif.codec_versions()
    for expected in ("dav1d [dec]:1.5.3", "aom [enc]:3.13.2"):
        if expected not in codecs:
            raise RuntimeError(f"Pillow AVIF oracle lacks {expected}: {codecs}")

    input_path = args.input.resolve()
    original_file = input_path.read_bytes()
    original_sample, inspection = extract_color_item(input_path)
    color_item = inspection["items"]["color"][0]
    spans = color_item["spans"]
    if len(spans) != 1 or spans[0]["length"] != len(original_sample):
        raise RuntimeError("mutation input must store its color item in one extent")
    item_offset = int(spans[0]["offset"])
    if original_file[item_offset : item_offset + len(original_sample)] != original_sample:
        raise RuntimeError("mutation input item extent does not match extracted bytes")
    obu_inspection = inspect_av1(input_path)
    samples = obu_inspection["samples"]
    if len(samples) != 1:
        raise RuntimeError("mutation input must contain exactly one AV1 sample")
    tile_offsets: set[int] = set()
    for obu in samples[0]["obus"]:
        for tile in (obu.get("tile_group") or {}).get("tiles", []):
            for span in tile["physical_spans"]:
                start = int(span["offset"]) - item_offset
                end = start + int(span["length"])
                if start < 0 or end > len(original_sample):
                    raise RuntimeError("coded tile span falls outside the color item")
                tile_offsets.update(range(start, end))
    if not tile_offsets:
        raise RuntimeError("mutation input has no coded tile bytes")

    with tempfile.TemporaryDirectory(prefix="image-star-avif-mutations-") as name:
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

        sample_path = work / "candidate.obu"
        output_path = work / "candidate.yuv"
        selected: dict[str, tuple[int, int]] = {}
        matches = (
            {"dc_token": 0}
            if args.dc_token_min is not None
            else {"eob_bin": 0, "eob_base": 0}
        )
        tile_matches = dict.fromkeys(matches, 0)
        selected_dc_token: int | None = None
        candidates_run = 0
        for sample_offset, old in enumerate(original_sample):
            for replacement in range(256):
                if replacement == old:
                    continue
                mutated = bytearray(original_sample)
                mutated[sample_offset] = replacement
                sample_path.write_bytes(mutated)
                result, operations = run_dav1d(
                    executable,
                    environment,
                    sample_path,
                    output_path,
                )
                candidates_run += 1
                if args.dc_token_min is not None:
                    token = closed_dc_final_token(result.stdout)
                    if (
                        result.returncode == 0
                        and token is not None
                        and token > args.dc_token_min
                    ):
                        matches["dc_token"] += 1
                        if sample_offset in tile_offsets:
                            tile_matches["dc_token"] += 1
                            select_token = (
                                selected_dc_token is None
                                or (
                                    args.dc_token_selection == "smallest"
                                    and token < selected_dc_token
                                )
                                or (
                                    args.dc_token_selection == "largest"
                                    and token > selected_dc_token
                                )
                            )
                            if select_token:
                                selected_dc_token = token
                                selected["dc_token"] = (sample_offset, replacement)
                elif (stage := rejection_stage(operations)) is not None:
                    matches[stage] += 1
                    if sample_offset in tile_offsets:
                        tile_matches[stage] += 1
                        selected.setdefault(stage, (sample_offset, replacement))

        records: dict[str, dict[str, object]] = {}
        retained: dict[str, bytes] = {}
        for stage, mutation in selected.items():
            first_file, first_record = candidate_record(
                executable,
                environment,
                work,
                original_file,
                original_sample,
                item_offset,
                *mutation,
            )
            second_file, second_record = candidate_record(
                executable,
                environment,
                work,
                original_file,
                original_sample,
                item_offset,
                *mutation,
            )
            if first_file != second_file or first_record != second_record:
                raise RuntimeError(f"nondeterministic selected {stage} mutation")
            records[stage] = first_record
            retained[stage] = first_file

    expected_records = (
        {"dc_token"} if args.dc_token_min is not None else {"eob_bin", "eob_base"}
    )
    if set(records) != expected_records:
        raise RuntimeError(f"mutation sweep did not find required states: {records}")
    if args.retain_dir is not None:
        args.retain_dir.mkdir(parents=True, exist_ok=True)
        for stage, data in retained.items():
            prefix = "slice39" if stage == "dc_token" else "slice35"
            (args.retain_dir / f"{prefix}_{stage}_control.avif").write_bytes(data)

    report = {
        "oracle": {
            "pillow": Image.__version__,
            "libavif": features.version("avif"),
            "codecs": codecs,
            "dav1d_commit": DAV1D_COMMIT,
        },
        "source": {
            "fixture": input_path.name,
            "file_length": len(original_file),
            "file_sha256": sha256(original_file),
            "sample_length": len(original_sample),
            "sample_sha256": sha256(original_sample),
            "item_offset": item_offset,
            "tile_sample_offsets": sorted(tile_offsets),
        },
        "sweep": {
            "strategy": "every AV1 item byte replaced by every other byte value",
            "candidates_run": candidates_run,
            "matches": matches,
            "tile_matches": tile_matches,
        },
        "selected": records,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    print(f"Written {candidates_run} deterministic sample mutations: {args.output}")


if __name__ == "__main__":
    main()
