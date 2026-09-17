#!/usr/bin/env python3
"""Record temporal candidates from the existing animated AVIF with pinned dav1d.

Builds two disposable scalar decoders: an unchanged decoder and a decoder with
a read-only wrapper around add_temporal_candidate. The wrapper calls the
original function exactly once. Both displayed YUV streams must agree byte for
byte, and two traced runs must agree. No repository Rust is executed.
"""

from __future__ import annotations

import argparse
import json
import platform
import subprocess
import tempfile
from pathlib import Path

from PIL import Image, _avif, _imaging, features

from generate_av1_reconstruction_refs import (
    DAV1D_COMMIT,
    resolve_tool,
    sha256,
    tool_environment,
    verify_source,
)
from generate_av1_sequence_refs import (
    COPYING_SHA256,
    FIXTURE,
    ROOT,
    build_dav1d,
    copy_source,
    digest_file,
    git_tree_digest,
    inspect_avif,
    json_bytes,
    make_ivf,
    pillow_report,
    replace_once,
    run,
    track_report,
)

FIXTURE_SHA256 = "2f8683d21725261f37f86e115f0c212cc52d0fefd3a2ddfcc4fa648c1859906d"
DEFAULT_OUTPUT = ROOT / "target/oracle-staging/av1-temporal/animated"
SCHEMA = "image-slash-star/av1-temporal-oracle@1"

# Only initialized fields are read: the unused second vector in a single
# candidate and the denominator of an invalid temporal entry are JSON null.
WRAPPER = r'''
static unsigned temporal_event_index;

static void temporal_vector(const union mv value) {
    printf("[%d,%d]", value.x, value.y);
}

static void temporal_stack(const refmvs_candidate *const stack,
                           const int count, const int single) {
    putchar('[');
    for (int i = 0; i < count; i++) {
        if (i) putchar(',');
        printf("{\"vectors\":[");
        temporal_vector(stack[i].mv.mv[0]);
        putchar(',');
        if (single) fputs("null", stdout);
        else temporal_vector(stack[i].mv.mv[1]);
        printf("],\"weight\":%d}", stack[i].weight);
    }
    putchar(']');
}

static void add_temporal_candidate(const refmvs_frame *const rf,
                                   refmvs_candidate *const mvstack, int *const cnt,
                                   const refmvs_temporal_block *const rb,
                                   const union refmvs_refpair ref, int *const globalmv_ctx,
                                   const union mv gmv[])
{
    const int single = ref.ref[1] == -1;
    const int valid = rb->mv.n != INVALID_MV;
    printf("@TEMPORAL {\"event_index\":%u,\"frame_offset\":%u,"
           "\"target_references\":[%d,%d],\"valid\":%s,"
           "\"force_integer\":%s,\"high_precision\":%s,\"vector\":",
           temporal_event_index++, rf->frm_hdr->frame_offset,
           ref.ref[0], ref.ref[1], valid ? "true" : "false",
           rf->frm_hdr->force_integer_mv ? "true" : "false",
           rf->frm_hdr->hp ? "true" : "false");
    temporal_vector(rb->mv);
    fputs(",\"denominator\":", stdout);
    if (valid) printf("%u", rb->ref);
    else fputs("null", stdout);
    fputs(",\"distances\":[", stdout);
    for (int i = 0; i < 7; i++) {
        if (i) putchar(',');
        printf("%d", rf->pocdiff[i]);
    }
    fputs("],\"global_vector\":", stdout);
    if (single && globalmv_ctx) temporal_vector(gmv[0]);
    else fputs("null", stdout);
    fputs(",\"global_context_before\":", stdout);
    if (globalmv_ctx) printf("%d", *globalmv_ctx);
    else fputs("null", stdout);
    fputs(",\"stack_before\":", stdout);
    temporal_stack(mvstack, *cnt, single);

    add_temporal_candidate_untraced(rf, mvstack, cnt, rb, ref, globalmv_ctx, gmv);

    fputs(",\"stack_after\":", stdout);
    temporal_stack(mvstack, *cnt, single);
    fputs(",\"global_context_after\":", stdout);
    if (globalmv_ctx) printf("%d", *globalmv_ctx);
    else fputs("null", stdout);
    puts("}");
}

'''


def instrument(source: Path) -> bytes:
    path = source / "src/refmvs.c"
    replace_once(path, "#include <stdlib.h>", "#include <stdlib.h>\n#include <stdio.h>")
    replace_once(path, "static void add_temporal_candidate(",
                 "static void add_temporal_candidate_untraced(")
    anchor = "static void add_compound_extended_candidate("
    replace_once(path, anchor, WRAPPER + anchor)
    return run(["git", "-C", str(source), "diff", "--unified=0", "--", "src/refmvs.c"]).stdout


def decode(binary: Path, ivf: Path, output: Path, env: dict[str, str]) -> bytes:
    argv = [str(binary), "--input", str(ivf), "--demuxer", "ivf",
            "--output", str(output), "--muxer", "yuv", "--threads", "1",
            "--framedelay", "1", "--cpumask", "0", "--quiet"]
    result = subprocess.run(argv, env=env, capture_output=True, timeout=120, check=True)
    if result.stderr:
        raise RuntimeError(f"unexpected decoder diagnostics: {result.stderr!r}")
    return result.stdout


def collect(source: Path, meson: Path, ninja: Path, output: Path) -> None:
    verify_source(source)
    if digest_file(source / "COPYING") != COPYING_SHA256:
        raise RuntimeError("dav1d COPYING differs from the pinned license")
    data = FIXTURE.read_bytes()
    if sha256(data) != FIXTURE_SHA256:
        raise RuntimeError("animated AVIF differs from the registered full-file input")
    codecs = _avif.codec_versions()
    if (Image.__version__ != "12.2.0" or features.version("avif") != "1.4.1"
            or codecs != "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2"):
        raise RuntimeError("Pillow/libavif/codec identity differs from the pinned oracle")
    container = inspect_avif(FIXTURE)
    track = track_report(container, "pict", container["color_track_id"])
    if track is None or len(track["samples"]) != 5:
        raise RuntimeError("animated AVIF must contain five primary samples")
    if output.exists():
        raise RuntimeError(f"refusing to replace existing evidence: {output}")
    output.parent.mkdir(parents=True, exist_ok=True)
    env = tool_environment(meson, ninja, None)
    with tempfile.TemporaryDirectory(prefix=".temporal-", dir=output.parent) as temporary:
        work = Path(temporary)
        bundle = work / "evidence"
        bundle.mkdir()
        ivf = work / "animated.ivf"
        ivf_record = make_ivf(data, list(track["samples"]), 150, 150, ivf)
        builds = {}
        stdout = {}
        yuv = {}
        for mode in ("plain", "trace"):
            copied = work / f"{mode}-source"
            copy_source(source, copied)
            if mode == "trace":
                (bundle / "instrumentation.patch").write_bytes(instrument(copied))
            build = work / f"{mode}-build"
            binary, info = build_dav1d(copied, build, meson, ninja, env)
            info["binary_sha256"] = digest_file(binary)
            info["refmvs_source_sha256"] = digest_file(copied / "src/refmvs.c")
            info["compiler"] = json.loads((build / "meson-info/intro-compilers.json").read_text())
            info["libraries"] = [
                {"name": path.name, "sha256": digest_file(path)}
                for path in sorted((build / "src").glob("*dav1d*"))
                if path.is_file() and not path.is_symlink()
            ]
            builds[mode] = info
            labels = ("plain",) if mode == "plain" else ("trace_1", "trace_2")
            for label in labels:
                yuv_file = work / f"{label}.yuv"
                stdout[label] = decode(binary, ivf, yuv_file, env)
                yuv[label] = yuv_file.read_bytes()
        if stdout["plain"]:
            raise RuntimeError("unmodified decoder emitted unexpected stdout")
        if stdout["trace_1"] != stdout["trace_2"]:
            raise RuntimeError("temporal trace is not deterministic")
        if not (yuv["plain"] == yuv["trace_1"] == yuv["trace_2"]):
            raise RuntimeError("instrumentation changed displayed YUV bytes")
        if len(yuv["plain"]) != 168750:
            raise RuntimeError("decoded output does not contain five 150x150 I420 frames")
        events = []
        for line in stdout["trace_1"].splitlines():
            if not line.startswith(b"@TEMPORAL "):
                raise RuntimeError("unrecognized trace transport")
            event = json.loads(line.removeprefix(b"@TEMPORAL "))
            if event["event_index"] != len(events):
                raise RuntimeError("noncontiguous temporal event index")
            events.append(event)
        valid = [event for event in events if event["valid"]]
        if not valid or not any(event["target_references"][1] == -1 for event in valid):
            raise RuntimeError("no valid single-reference candidate was observed")
        event_bytes = b"".join(
            (json.dumps(event, sort_keys=True, separators=(",", ":")) + "\n").encode()
            for event in events
        )
        (bundle / "events.jsonl").write_bytes(event_bytes)
        (bundle / "display.yuv").write_bytes(yuv["plain"])
        verify_source(source)
        index = {
            "schema": SCHEMA,
            "fixture": {"path": FIXTURE.relative_to(ROOT).as_posix(),
                        "sha256": sha256(data), "bytes": len(data)},
            "source": {"implementation": "dav1d", "version": "1.5.3",
                       "commit": DAV1D_COMMIT, "tree_sha256": git_tree_digest(source),
                       "copying_sha256": COPYING_SHA256, "clean_before_and_after": True,
                       "refmvs_source_sha256": digest_file(source / "src/refmvs.c")},
            "collector": {"path": "scripts/generate_av1_temporal_refs.py",
                          "sha256": digest_file(Path(__file__))},
            "pillow": {"version": Image.__version__, "libavif": features.version("avif"),
                       "codecs": codecs, "python": platform.python_version(),
                       "imaging_sha256": digest_file(Path(_imaging.__file__)),
                       "observations": pillow_report(data)},
            "ivf": ivf_record,
            "builds": builds,
            "counts": {"events": len(events), "valid": len(valid),
                       "single": sum(e["target_references"][1] == -1 for e in valid),
                       "compound": sum(e["target_references"][1] != -1 for e in valid)},
            "noninterference": {"raw_yuv_byte_equal": True, "trace_repeat_byte_equal": True,
                                "stdout_sha256": sha256(stdout["trace_1"]),
                                "display_bytes": len(yuv["plain"]),
                                "display_sha256": sha256(yuv["plain"])},
            "scope": {"target_execution": "not_run",
                      "public_sequence_parity": "not_proven",
                      "observation": "private temporal candidate transitions from a full file",
                      "limitations": "Only observed vectors, references, and precisions are covered."},
            "artifacts": [{"path": path.name, "bytes": path.stat().st_size,
                           "sha256": digest_file(path)} for path in sorted(bundle.iterdir())],
        }
        (bundle / "index.json").write_bytes(json_bytes(index))
        bundle.rename(output)
    print(json.dumps({"output": str(output), "counts": index["counts"]}, sort_keys=True))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dav1d-source", type=Path, required=True)
    parser.add_argument("--meson", default="meson")
    parser.add_argument("--ninja", default="ninja")
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    args = parser.parse_args()
    collect(args.dav1d_source.resolve(), resolve_tool(args.meson, "Meson"),
            resolve_tool(args.ninja, "Ninja"), args.output.resolve())


if __name__ == "__main__":
    main()
