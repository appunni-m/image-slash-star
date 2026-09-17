#!/usr/bin/env python3
"""Collect pinned native AVIF repetition observations from complete files.

Only a fresh ignored staging directory is written. Mutations preserve file
length, every encoded sample, and primary-image data. Rust is never executed.
"""

from __future__ import annotations

import argparse
import ctypes
import io
import shutil
import struct
import sys
import tempfile
from pathlib import Path

from PIL import Image, _avif, _imaging, features

from generate_av1_sequence_refs import (
    LIBAVIF_COMMIT, artifact_records, digest_file, git_tree_digest,
    resolve_tool, run, sha256, text_output, write_json,
)
from inspect_avif_bitstreams import children, parse_boxes, unique_box

ROOT = Path(__file__).resolve().parent.parent
FIXTURES = {
    "animated": ("animated.avif", "2f8683d21725261f37f86e115f0c212cc52d0fefd3a2ddfcc4fa648c1859906d"),
    "error_resilient": ("animated_error_resilient.avif", "06ea9771f8b46c3432c6c6cdf324f1c05e86a5fdccd774c8e3c9a8fce0b831f0"),
    "highdepth": ("10bit.avif", "3bf9f91da471749e7df639ba7945d4d94c1c3e3968c26f3619fbbcfc92790576"),
}
OBSERVER = r'''
#include <avif/avif.h>
#include <dlfcn.h>
#include <cstdio>
extern "C" int observe(void * library, const uint8_t * input, size_t size,
                        int * fields, char * diagnostic, size_t capacity) {
#define LOAD(name) \
    auto p_##name = reinterpret_cast<decltype(&name)>(dlsym(library, #name)); \
    if (!p_##name) return 1
    LOAD(avifDecoderCreate); LOAD(avifDecoderDestroy);
    LOAD(avifDecoderSetIOMemory); LOAD(avifDecoderParse); LOAD(avifDecoderNextImage);
#undef LOAD
    avifDecoder * decoder = p_avifDecoderCreate();
    if (!decoder) return 2;
    decoder->codecChoice = AVIF_CODEC_CHOICE_DAV1D;
    decoder->maxThreads = 1;
    auto finish = [&](int result) { p_avifDecoderDestroy(decoder); return result; };
    avifResult result = p_avifDecoderSetIOMemory(decoder, input, size);
    if (result != AVIF_RESULT_OK) return finish(3);
    result = p_avifDecoderParse(decoder);
    fields[0] = result;
    if (result != AVIF_RESULT_OK) {
        std::snprintf(diagnostic, capacity, "%s", decoder->diag.error);
        return finish(0);
    }
    fields[1] = decoder->repetitionCount;
    fields[2] = decoder->imageCount;
    fields[3] = 0;
    while ((result = p_avifDecoderNextImage(decoder)) == AVIF_RESULT_OK) ++fields[3];
    fields[4] = result;
    fields[5] = decoder->imageSequenceTrackPresent;
    std::snprintf(diagnostic, capacity, "%s", decoder->diag.error);
    if (result != AVIF_RESULT_NO_IMAGES_REMAINING) return finish(4);
    return finish(0);
}
'''


def track_boxes(data, ordinal=0):
    moov = unique_box(parse_boxes(data, 0, len(data)), b"moov")
    track = [b for b in children(data, moov) if b.kind == b"trak"][ordinal]
    boxes = children(data, track)
    tkhd = unique_box(boxes, b"tkhd")
    edts = unique_box(boxes, b"edts")
    elst = unique_box(children(data, edts), b"elst")
    if data[tkhd.payload_start] != 1 or data[elst.payload_start] != 1 or elst.size != 36:
        raise RuntimeError("pinned version-one mutation shape changed")
    return tkhd, edts, elst


def mutate(data, *, duration=None, flags=1, count=1, segment=5, version=1,
           no_edts=False, tail=None, size=None, track=0):
    tkhd, edts, elst = track_boxes(data, track)
    output = bytearray(data)
    if duration is not None:
        struct.pack_into(">Q", output, tkhd.payload_start + 28, duration)
    struct.pack_into(">II", output, elst.payload_start, (version << 24) | flags, count)
    struct.pack_into(">Q", output, elst.payload_start + 8, segment)
    if tail is not None:
        output[elst.payload_start + 16:elst.end] = tail
    if no_edts:
        output[edts.start + 4:edts.start + 8] = b"free"
    if size is not None:
        if not 8 <= size <= elst.size - 8:
            raise RuntimeError("cannot preserve a valid trailing free box")
        struct.pack_into(">I", output, elst.start, size)
        struct.pack_into(">I4s", output, elst.start + size, elst.size - size, b"free")
    if len(output) != len(data):
        raise RuntimeError("mutation changed complete file length")
    return bytes(output)


def pillow_observation(data, destination=None):
    try:
        frames = []
        with Image.open(io.BytesIO(data)) as image:
            for index in range(image.n_frames):
                image.seek(index)
                image.load()
                raw = image.tobytes()
                if destination is not None:
                    destination.mkdir(parents=True, exist_ok=True)
                    (destination / f"frame_{index}.bin").write_bytes(raw)
                frames.append({"index": index, "mode": image.mode, "size": list(image.size),
                               "bytes": len(raw), "sha256": sha256(raw),
                               "duration_ms": image.info.get("duration")})
            return {"status": "ok", "loop_key": image.info.get("loop"), "frames": frames}
    except Exception as error:
        return {"status": "error", "type": f"{type(error).__module__}.{type(error).__name__}",
                "message": str(error)}


def collect(bundle, observe):
    originals, records = {}, []
    for name, (filename, expected_hash) in FIXTURES.items():
        source = ROOT / "tests/fixtures/input/images/avif" / filename
        data = source.read_bytes()
        if sha256(data) != expected_hash:
            raise RuntimeError(f"pinned {name} source changed")
        originals[name] = data
        record = pillow_observation(data, bundle / "pixels" / name)
        if record["status"] != "ok" or record != pillow_observation(data):
            raise RuntimeError("original Pillow frames failed or changed")
        records.append((name, name, data, None))
    cases = [
        ("no_edit_list", dict(no_edts=True)),
        ("no_edit_list_zero_duration", dict(no_edts=True, duration=0)),
        ("nonrepeating_zero_duration", dict(flags=0, duration=0)),
        ("nonrepeating_ignored_fields", dict(flags=0xFFFFFE, version=255, count=0, segment=0, tail=b"\xff" * 12)),
        ("nonrepeating_header_only", dict(flags=0, size=12)),
        ("repeating_exact", dict(duration=15)),
        ("repeating_rounded", dict(duration=16)),
        ("repeating_partial", dict(duration=1)),
        ("repeating_reserved_flags", dict(duration=15, flags=0xFFFFFF)),
        ("repeating_ignored_media", dict(duration=15, tail=b"\xff" * 12)),
        ("repeating_segment_only", dict(duration=15, size=24)),
        ("repeating_v0", dict(duration=15, version=0, segment=5 << 32)),
        ("largest_finite", dict(duration=(1 << 31) * 5)),
        ("first_infinite", dict(duration=(1 << 31) * 5 + 1)),
        ("huge_finite_duration", dict(duration=(1 << 64) - 2, segment=1)),
        ("indefinite", dict(duration=(1 << 64) - 1)),
        ("error_zero_duration", dict(duration=0)),
        ("error_zero_segment", dict(segment=0)),
        ("error_indefinite_zero_segment", dict(duration=(1 << 64) - 1, segment=0)),
        ("error_entry_count", dict(count=2)),
        ("error_version", dict(version=2)),
        ("error_missing_segment", dict(size=16)),
        ("error_missing_flags", dict(size=8)),
    ]
    for name, parameters in cases:
        records.append((name, "animated", mutate(originals["animated"], **parameters), parameters))
    records.append(("alpha_loop_disagreement", "highdepth",
                    mutate(originals["highdepth"], track=1, flags=0), {"track": 1, "flags": 0}))
    records.append(("error_alpha_segment", "highdepth",
                    mutate(originals["highdepth"], track=1, segment=0), {"track": 1, "segment": 0}))
    output = []
    for name, source, data, mutation in records:
        native_repeats = []
        for _ in range(2):
            fields = (ctypes.c_int * 6)(*([-999] * 6))
            diagnostic = ctypes.create_string_buffer(512)
            if observe(data, len(data), fields, diagnostic, len(diagnostic)):
                raise RuntimeError(f"native observer failed for {name}")
            native_repeats.append({"parse_result": fields[0], "repetition_count": fields[1],
                                   "frame_count": fields[2], "decoded_frames": fields[3],
                                   "terminal_result": fields[4], "sequence_track_present": fields[5],
                                   "diagnostic": diagnostic.value.decode("utf-8")})
        pillow = pillow_observation(data)
        if native_repeats[0] != native_repeats[1] or pillow != pillow_observation(data):
            raise RuntimeError(f"native or Pillow observation changed: {name}")
        native = native_repeats[0]
        if name.startswith("error_") and name != "error_resilient":
            if native["parse_result"] == 0 or pillow["status"] != "error":
                raise RuntimeError(f"expected independent malformed-file rejection: {name}")
        else:
            expected = pillow_observation(originals[source])
            if native["parse_result"] != 0 or native["decoded_frames"] != len(expected["frames"]) or pillow != expected:
                raise RuntimeError(f"full-frame mutation changed native/Pillow decoding: {name}")
        relative = f"inputs/{name}.avif"
        (bundle / "inputs").mkdir(exist_ok=True)
        (bundle / relative).write_bytes(data)
        source_data = originals[source]
        sample_payloads_equal = all(
            data[b.payload_start:b.end] == source_data[b.payload_start:b.end]
            for b in parse_boxes(source_data, 0, len(source_data)) if b.kind == b"mdat"
        )
        if not sample_payloads_equal:
            raise RuntimeError("mutation changed media bytes")
        output.append({"name": name, "source": source, "input_path": relative,
                       "input_bytes": len(data), "input_sha256": sha256(data),
                       "source_sha256": sha256(source_data), "mutation": {k: v.hex() if isinstance(v, bytes) else v for k, v in (mutation or {}).items()},
                       "native": native, "pillow": pillow, "repeat_equal": True,
                       "media_payloads_equal": True})
    return output


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--libavif-source", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    output, source = args.output.resolve(), args.libavif_source.resolve()
    if (ROOT / "target/oracle-staging").resolve() not in output.parents or output.exists():
        raise RuntimeError("output must be fresh and below target/oracle-staging")
    if text_output(run(["git", "-C", str(source), "rev-parse", "HEAD"])) != LIBAVIF_COMMIT or run(["git", "-C", str(source), "status", "--porcelain"]).stdout:
        raise RuntimeError("libavif source must be clean and pinned")
    if digest_file(source / "LICENSE") != digest_file(ROOT / "third_party/libavif/LICENSE"):
        raise RuntimeError("libavif retained license differs")
    if (Image.__version__, features.version("avif"), _avif.codec_versions()) != (
        "12.2.0", "1.4.1", "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2"
    ):
        raise RuntimeError("Pillow/native version differs from pins")
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".avif-loops-", dir=output.parent) as temporary:
        work = Path(temporary)
        bundle = work / "bundle"
        bundle.mkdir()
        observer = bundle / "observer.cc"
        observer.write_text(OBSERVER)
        compiler = resolve_tool("c++", "C++ compiler")
        shared = work / "observer.so"
        command = [str(compiler), "-std=c++17", "-O2", "-Wall", "-Wextra", "-Werror", "-shared", "-fPIC",
                   f"-I{source / 'include'}", str(observer), "-o", str(shared)]
        if sys.platform.startswith("linux"):
            command.append("-ldl")
        run(command)
        library, harness = ctypes.CDLL(_avif.__file__), ctypes.CDLL(str(shared))
        native = harness.observe
        native.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_size_t, ctypes.POINTER(ctypes.c_int), ctypes.c_void_p, ctypes.c_size_t]
        native.restype = ctypes.c_int
        cases = collect(bundle, lambda *values: native(library._handle, *values))
        index = {"schema": "image-slash-star/avif-loop-oracle@1", "cases": cases,
                 "origin": "libavif.avifDecoder.repetitionCount",
                 "source": {"commit": LIBAVIF_COMMIT, "tree_sha256": git_tree_digest(source),
                            "files": [{"path": name, "sha256": digest_file(source / name)} for name in ("include/avif/avif.h", "src/read.c", "LICENSE")]},
                 "oracle": {"pillow": Image.__version__, "libavif": features.version("avif"), "codecs": _avif.codec_versions(),
                            "avif_binary_sha256": digest_file(Path(_avif.__file__)), "imaging_binary_sha256": digest_file(Path(_imaging.__file__))},
                 "build": {"compiler": text_output(run([str(compiler), "--version"])), "binary_sha256": digest_file(shared),
                           "argv": [v.replace(str(source), "<source>").replace(str(work), "<work>") for v in command]},
                 "artifacts": artifact_records(bundle), "rust_execution": "deferred"}
        write_json(bundle / "index.json", index)
        shutil.move(bundle, output)
    print(f"Collected {len(cases)} complete AVIF loop cases: {output}")


if __name__ == "__main__":
    main()
