#!/usr/bin/env python3
"""Reverse-map AV1 DC tokens across a bounded late-byte mutation grid.

The script concatenates deterministic mutations into small section-5 batches,
decodes them through the pinned scalar dav1d build, and retains only traces
that preserve the closed quality-99 DC-only syntax class. Selected candidates
are rerun individually and checked through Pillow. It is a diagnostic
fixture-selection tool, not a fuzzer and not part of the crate.
"""

from __future__ import annotations

import argparse
import hashlib
import itertools
import json
import os
import re
import subprocess
import tempfile
from collections import Counter
from pathlib import Path

from PIL import Image, _avif, features

from explore_avif_sample_mutations import (
    DC_SIGN_PATTERN,
    closed_dc_final_token,
    pillow_outcome,
    run_dav1d,
    sha256,
)
from generate_av1_reconstruction_refs import (
    DAV1D_COMMIT,
    build_dav1d,
    extract_color_item,
    resolve_tool,
    verify_source,
)
from inspect_av1_obus import inspect as inspect_av1


INIT_PATTERN = re.compile(
    r'^@MSAC \{"step":0,"operation":"init","value":-1,'
)


def trace_segments(output: str) -> list[str]:
    """Split one concatenated dav1d trace into one scalar tile per sample."""

    segments: list[list[str]] = []
    current: list[str] | None = None
    for line in output.splitlines():
        if INIT_PATTERN.match(line):
            current = []
            segments.append(current)
        if current is not None:
            current.append(line)
    return ["\n".join(segment) for segment in segments]


def dc_negative(output: str) -> bool | None:
    """Return the decoded DC sign for one retained-class trace."""

    for line in output.splitlines():
        if match := DC_SIGN_PATTERN.match(line):
            return bool(int(match.group("negative")))
    return None


def selected_record(
    executable: Path,
    environment: dict[str, str],
    work: Path,
    original_file: bytes,
    original_sample: bytes,
    item_offset: int,
    offsets: tuple[int, ...],
    values: tuple[int, ...],
) -> tuple[bytes, dict[str, object]]:
    """Rerun and fully classify one selected multi-byte mutation."""

    mutated_sample = bytearray(original_sample)
    mutations = []
    for offset, value in zip(offsets, values, strict=True):
        old = mutated_sample[offset]
        mutated_sample[offset] = value
        mutations.append(
            {
                "sample_offset": offset,
                "file_offset": item_offset + offset,
                "old_byte": old,
                "new_byte": value,
            }
        )
    mutated_file = bytearray(original_file)
    mutated_file[item_offset : item_offset + len(mutated_sample)] = mutated_sample

    sample_path = work / "selected-grid.obu"
    output_path = work / "selected-grid.yuv"
    sample_path.write_bytes(mutated_sample)
    result, operations = run_dav1d(
        executable,
        environment,
        sample_path,
        output_path,
    )
    decoded = output_path.read_bytes() if output_path.exists() else b""
    record = {
        "dc_final_token": closed_dc_final_token(result.stdout),
        "dc_negative": dc_negative(result.stdout),
        "mutations": mutations,
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
    """Run the deterministic late-byte DC-token grid."""

    parser = argparse.ArgumentParser(description=__doc__)
    decoder = parser.add_mutually_exclusive_group(required=True)
    decoder.add_argument("--dav1d", type=Path)
    decoder.add_argument("--dav1d-source", type=Path)
    parser.add_argument("--meson", default="meson")
    parser.add_argument("--ninja", default="ninja")
    parser.add_argument("--python-path", type=Path)
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument(
        "--sample-offset",
        type=int,
        action="append",
        required=True,
        help="AV1-item byte offset to enumerate; repeat for a two-byte grid",
    )
    parser.add_argument("--dc-token-min", type=int, required=True)
    parser.add_argument(
        "--dc-token-masked-max",
        type=int,
        help=(
            "retain only tokens whose low 20 bits are no greater than this "
            "value; used to prove the AV1 coefficient token mask"
        ),
    )
    parser.add_argument(
        "--dc-token-selection",
        choices=("smallest", "largest"),
        default="smallest",
    )
    parser.add_argument("--batch-size", type=int, default=256)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--retain-dir", type=Path)
    args = parser.parse_args()

    offsets = tuple(args.sample_offset)
    if not 1 <= len(offsets) <= 2:
        parser.error("repeat --sample-offset once or twice")
    if len(set(offsets)) != len(offsets):
        parser.error("--sample-offset values must be unique")
    if args.dc_token_min < 0:
        parser.error("--dc-token-min must be non-negative")
    if args.dc_token_masked_max is not None and not (
        0 <= args.dc_token_masked_max <= 0xF_FFFF
    ):
        parser.error("--dc-token-masked-max must be in 0..=0xfffff")
    if not 1 <= args.batch_size <= 1_024:
        parser.error("--batch-size must be in 1..=1024")

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
        raise RuntimeError("grid input must store its color item in one extent")
    item_offset = int(spans[0]["offset"])
    if original_file[item_offset : item_offset + len(original_sample)] != original_sample:
        raise RuntimeError("grid input item extent does not match extracted bytes")

    obu_inspection = inspect_av1(input_path)
    samples = obu_inspection["samples"]
    if len(samples) != 1:
        raise RuntimeError("grid input must contain exactly one AV1 sample")
    tile_offsets: set[int] = set()
    for obu in samples[0]["obus"]:
        for tile in (obu.get("tile_group") or {}).get("tiles", []):
            for span in tile["physical_spans"]:
                start = int(span["offset"]) - item_offset
                end = start + int(span["length"])
                if start < 0 or end > len(original_sample):
                    raise RuntimeError("coded tile span falls outside the color item")
                tile_offsets.update(range(start, end))
    if any(offset not in tile_offsets for offset in offsets):
        raise RuntimeError(
            f"grid offsets must be coded-tile bytes: offsets={offsets}, "
            f"tile={sorted(tile_offsets)}"
        )

    with tempfile.TemporaryDirectory(prefix="image-star-avif-dc-grid-") as name:
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

        sample_path = work / "grid.obu"
        yuv_path = work / "grid.yuv"
        value_product = itertools.product(range(256), repeat=len(offsets))
        token_counts: Counter[int] = Counter()
        boundary_candidates: list[tuple[int, tuple[int, ...]]] = []
        candidates_run = 0
        closed_matches = 0
        raw_boundary_matches = 0
        boundary_matches = 0
        batch_count = 0
        sample_digest = hashlib.sha256()
        trace_digest = hashlib.sha256()
        error_digest = hashlib.sha256()
        decoded_digest = hashlib.sha256()

        def consider(values: tuple[int, ...], segment: str) -> None:
            nonlocal boundary_matches
            nonlocal closed_matches
            nonlocal raw_boundary_matches

            token = closed_dc_final_token(segment)
            if token is None:
                return
            closed_matches += 1
            token_counts[token] += 1
            if token <= args.dc_token_min:
                return
            raw_boundary_matches += 1
            if (
                args.dc_token_masked_max is not None
                and token & 0xF_FFFF > args.dc_token_masked_max
            ):
                return
            boundary_matches += 1
            boundary_candidates.append((token, values))

        while batch := list(itertools.islice(value_product, args.batch_size)):
            batch_count += 1
            with sample_path.open("wb") as stream:
                for values in batch:
                    candidate = bytearray(original_sample)
                    for offset, value in zip(offsets, values, strict=True):
                        candidate[offset] = value
                    sample_digest.update(candidate)
                    stream.write(candidate)
            if yuv_path.exists():
                yuv_path.unlink()
            result = subprocess.run(
                [
                    str(executable),
                    "--input",
                    str(sample_path),
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
                check=False,
                env=environment,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
            segments = trace_segments(result.stdout)
            if not segments:
                candidates_run += len(batch)
                trace_digest.update(result.stdout.encode())
                error_digest.update(result.stderr.encode())
                if yuv_path.exists():
                    decoded_digest.update(yuv_path.read_bytes())
                continue
            if len(segments) == len(batch):
                candidates_run += len(batch)
                trace_digest.update(result.stdout.encode())
                error_digest.update(result.stderr.encode())
                if yuv_path.exists():
                    decoded_digest.update(yuv_path.read_bytes())
                for values, segment in zip(batch, segments, strict=True):
                    consider(values, segment)
                continue

            # A partial batch cannot be mapped positionally because rejected
            # frames produce no tile marker. Rerun only that batch one sample
            # at a time so every retained trace keeps an exact mutation.
            for values in batch:
                candidate = bytearray(original_sample)
                for offset, value in zip(offsets, values, strict=True):
                    candidate[offset] = value
                sample_path.write_bytes(candidate)
                individual, _ = run_dav1d(
                    executable,
                    environment,
                    sample_path,
                    yuv_path,
                )
                candidates_run += 1
                trace_digest.update(individual.stdout.encode())
                error_digest.update(individual.stderr.encode())
                if yuv_path.exists():
                    decoded_digest.update(yuv_path.read_bytes())
                consider(values, individual.stdout)

        selected_data: bytes | None = None
        selected: dict[str, object] | None = None
        selected_token: int | None = None
        selected_candidates_tried = 0
        reverse = args.dc_token_selection == "largest"
        ordered_candidates = sorted(
            boundary_candidates,
            key=lambda candidate: candidate[0],
            reverse=reverse,
        )
        trace_extreme_data: bytes | None = None
        trace_extreme: dict[str, object] | None = None
        trace_extreme_token: int | None = None
        if ordered_candidates:
            trace_extreme_token, trace_extreme_values = ordered_candidates[0]
            trace_extreme_data, trace_extreme = selected_record(
                executable,
                environment,
                work,
                original_file,
                original_sample,
                item_offset,
                offsets,
                trace_extreme_values,
            )
        for candidate_token, candidate_values in ordered_candidates:
            selected_candidates_tried += 1
            first_data, first_record = selected_record(
                executable,
                environment,
                work,
                original_file,
                original_sample,
                item_offset,
                offsets,
                candidate_values,
            )
            if (
                first_record["dc_final_token"] != candidate_token
                or first_record["dav1d_returncode"] != 0
                or first_record["decoded_yuv_length"] == 0
                or first_record["pillow"].get("status") != "ok"
            ):
                continue
            second_data, second_record = selected_record(
                executable,
                environment,
                work,
                original_file,
                original_sample,
                item_offset,
                offsets,
                candidate_values,
            )
            if first_data != second_data or first_record != second_record:
                raise RuntimeError("nondeterministic selected grid mutation")
            selected_token = candidate_token
            selected_data = first_data
            selected = first_record
            break

    if args.retain_dir is not None and selected_data is not None:
        args.retain_dir.mkdir(parents=True, exist_ok=True)
        (args.retain_dir / f"slice40_dc_token_{selected_token}.avif").write_bytes(
            selected_data
        )
    if args.retain_dir is not None and trace_extreme_data is not None:
        args.retain_dir.mkdir(parents=True, exist_ok=True)
        (
            args.retain_dir / f"slice40_trace_token_{trace_extreme_token}.avif"
        ).write_bytes(trace_extreme_data)

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
            "strategy": "Cartesian byte grid batched as concatenated AV1 samples",
            "sample_offsets": list(offsets),
            "batch_size": args.batch_size,
            "batch_count": batch_count,
            "candidates_run": candidates_run,
            "closed_matches": closed_matches,
            "raw_boundary_matches": raw_boundary_matches,
            "boundary_matches": boundary_matches,
            "selected_candidates_tried": selected_candidates_tried,
            "dc_token_min": args.dc_token_min,
            "dc_token_masked_max": args.dc_token_masked_max,
            "dc_token_selection": args.dc_token_selection,
            "distinct_tokens": len(token_counts),
            "minimum_token": min(token_counts, default=None),
            "maximum_token": max(token_counts, default=None),
            "token_counts": dict(sorted(token_counts.items())),
            "candidate_samples_sha256": sample_digest.hexdigest(),
            "dav1d_stdout_sha256": trace_digest.hexdigest(),
            "dav1d_stderr_sha256": error_digest.hexdigest(),
            "decoded_yuv_sha256": decoded_digest.hexdigest(),
        },
        "trace_extreme": trace_extreme,
        "selected": selected,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    print(
        f"Written {candidates_run} deterministic grid mutations "
        f"({closed_matches} closed, selected token {selected_token}): {args.output}"
    )


if __name__ == "__main__":
    main()
