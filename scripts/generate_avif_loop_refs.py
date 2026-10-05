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
    "opidc_0x101": ("animated_opidc_0x101.avif", "25b79a856ea2767e02e5a509f72e7af3f9563202301779be65b0745724ab23be"),
    "error_resilient": ("animated_error_resilient.avif", "06ea9771f8b46c3432c6c6cdf324f1c05e86a5fdccd774c8e3c9a8fce0b831f0"),
    "filmgrain_reference_reuse_i444_64x64": (
        "animated_filmgrain_reference_reuse_i444_64x64.avif",
        "bd947085ed6437edfd50a97b43506cc56e96af8b5450a8ef5cb8289b8ec62b34",
    ),
    "filmgrain_inactive_reference_reuse_i444_64x64": (
        "animated_filmgrain_inactive_reference_reuse_i444_64x64.avif",
        "d1c4c2b6c9c24571452ccb95387a4c18580d9e7b871e704217b17b58ae916536",
    ),
    "lossless_inter_420_b16x16": (
        "animated_lossless_inter_420_b16x16.avif",
        "6973cfff29185ffc7283ec539d9cf6736dfe4cba52850c46e4b196ba9db51ead",
    ),
    "lossless_inter_420_b8x16": (
        "animated_lossless_inter_420_b8x16.avif",
        "9e3fbac5e42c61413ec8c1f7f4f72bc1338fb7cfd21587e30826855732e369c0",
    ),
    "lossless_inter_intra_i444_b8x8_20x20": (
        "animated_lossless_inter_intra_i444_b8x8_20x20.avif",
        "7636f2de4078d5271941e6cf7de85f9c77205acce155c75e529e43fbc1d0acbf",
    ),
    "odd_dimensions_inter_intra_boundary_420": (
        "animated_odd_dimensions_inter_intra_boundary_420.avif",
        "736914091e00473af577a12de9910ce6abe2b038c84d2f580c5f528b0618c422",
    ),
    "lossy_interintra_420_nowedge_b16x16_64x64": (
        "animated_lossy_interintra_420_nowedge_b16x16_64x64.avif",
        "adc58d72626865406be3d0a048ecf0b9a1670d16a68f48babe4b9167a2dc2dea",
    ),
    "lossy_interintra_420_nowedge_mode1_b16x16_64x64": (
        "animated_lossy_interintra_420_nowedge_mode1_b16x16_64x64.avif",
        "dd4e06e559d83cc0863d65878799d2f7496c8f5d378eaeb639704fe3ac539f2f",
    ),
    "lossy_interintra_420_nowedge_mode2_b16x16_64x64": (
        "animated_lossy_interintra_420_nowedge_mode2_b16x16_64x64.avif",
        "08f616c2e14f9abadb6157c425653b5a7fd703f2797f3db04e23b58bc519eda8",
    ),
    "lossy_interintra_420_nowedge_mode3_b16x16_64x64": (
        "animated_lossy_interintra_420_nowedge_mode3_b16x16_64x64.avif",
        "7e035cef7293dfff72e50ff8c729bc0fad360ee14d7bf80d663b82b96ca7c4d7",
    ),
    "lossy_interintra_420_wedge_b16x16_64x64": (
        "animated_lossy_interintra_420_wedge_b16x16_64x64.avif",
        "1510b764d7e642f2caf02fff53e6dc966cfb92df8eca2e8f2d91c0b3bead1791",
    ),
    "lossless_inter_420_b32x32": (
        "animated_lossless_inter_420_b32x32.avif",
        "65e8617044f7f12db081f65276235296f8dc208a006451210ebb9603f27f20a7",
    ),
    "lossless_inter_420_b32x32_10bit": (
        "animated_lossless_inter_420_b32x32_10bit.avif",
        "c90bf9e75b0c091e19ab6b8aa70c17243ae1dbbf818493e815015f998f14e73a",
    ),
    "lossless_inter_420_b32x32_10bit_64x64": (
        "animated_lossless_inter_420_b32x32_10bit_64x64.avif",
        "62a154bb3e8a7c92816045d58f492aaa30c3aa215526f6c3d7433de942fb2448",
    ),
    "lossless_inter_420_clipped_b32x32_10bit_28x28": (
        "animated_lossless_inter_420_clipped_b32x32_10bit_28x28.avif",
        "f9d829f20968483dbd078a47f136b9c413205ce78d423fb0b5f1bdb2d2c0cce4",
    ),
    "lossless_inter_420_clipped_b32x32_10bit_partition32_28x64": (
        "animated_lossless_inter_420_clipped_b32x32_10bit_partition32_28x64.avif",
        "8f2b787faea151faf9c7f69c2b92e098dedd3bb70f28fd33546194c002b3456f",
    ),
    "lossless_inter_420_clipped_b32x32_10bit_partition32_56x64": (
        "animated_lossless_inter_420_clipped_b32x32_10bit_partition32_56x64.avif",
        "a819711996c804d7c5e4dcc26cee2e6a35bb82c48a88a6db8e34bd61d3b5f9fe",
    ),
    "lossless_inter_420_clipped_b32x32_17x17": (
        "animated_lossless_inter_420_clipped_b32x32_17x17.avif",
        "b29bedb6d7486dae5bed2abb74986a65e47bc737bfd26f85b1f69fa8bd666819",
    ),
    "lossless_inter_i444_clipped_b32x32_49x64": (
        "animated_lossless_inter_i444_clipped_b32x32_49x64.avif",
        "d45defceb497272879b1093f76e6a6c645f0bde8a6d10a1903e2322af422caae",
    ),
    "lossless_inter_i422_clipped_b32x32_49x64": (
        "animated_lossless_inter_i422_clipped_b32x32_49x64.avif",
        "fd18cdcef915327fd4aab6992c901e27f2efc1411d1d2d01eee93dd3f277d7b1",
    ),
    "lossless_inter_i422_clipped_b32x32_52x64": (
        "animated_lossless_inter_i422_clipped_b32x32_52x64.avif",
        "43bbcd3d92593e026d88ce4dea727074f74134175ee42a962886c6de697a4e05",
    ),
    "lossless_inter_i444_clipped_b32x32_64x56": (
        "animated_lossless_inter_i444_clipped_b32x32_64x56.avif",
        "9004ec840b2a413c0e15294f52c723ebe5c3689cbe3decbd4afd5575cad78e23",
    ),
    "lossless_inter_i422_clipped_b32x32_52x60": (
        "animated_lossless_inter_i422_clipped_b32x32_52x60.avif",
        "53e06d79c586151d920b94cc6173eebcc1f9869ebb0f1fa5bec673df11c45e50",
    ),
    "lossless_inter_i444_clipped_b32x32_52x60": (
        "animated_lossless_inter_i444_clipped_b32x32_52x60.avif",
        "9fdd099eb28fed9eaff2760b154e42d91933758fd6b01e532e441646f564993a",
    ),
    "lossless_inter_420_clipped_b32x32_60x64": (
        "animated_lossless_inter_420_clipped_b32x32_60x64.avif",
        "a60dde64c6b7bf9f6fbb3afe7e75a74374d446f8106613b00837300df4963bbc",
    ),
    "lossless_inter_420_clipped_b32x32_64x60": (
        "animated_lossless_inter_420_clipped_b32x32_64x60.avif",
        "7602fd062d0aefdaa0221b35a3a6c9403fecc27bf8a66cb00c0b4f9b8763c0d6",
    ),
    "lossless_inter_420_clipped_b32x32_184x64": (
        "animated_lossless_inter_420_clipped_b32x32_184x64.avif",
        "0a8c935fe67694fe576e7f21064eec4c428cffc2a05a3ff2d0be33e401a20c1d",
    ),
    "lossless_inter_420_clipped_b32x32_185x64": (
        "animated_lossless_inter_420_clipped_b32x32_185x64.avif",
        "701d247ebbe2666b1f28f447fa1a086573e7dc6d88b0b29405f9f7a6f9460b59",
    ),
    "lossless_inter_420_clipped_b16x16_60x64": (
        "animated_lossless_inter_420_clipped_b16x16_60x64.avif",
        "7c8bd0f88cd6654fb1150d2220ecb33c6ec5874b26a2250909b51cab4ed2e17f",
    ),
    "lossless_inter_monochrome_b16x16": (
        "animated_lossless_inter_monochrome_b16x16.avif",
        "d29aaab346465bf7ac413954e3c862b2222d3ac510299f865fe579899d063fb5",
    ),
    "lossy_split_inter_420_b16x16": (
        "animated_lossy_split_inter_420_b16x16.avif",
        "9eb50f5a45dc2eb549c62ac9bcb67abd931a977076685bcf3c0f69e6827ce323",
    ),
    "lossy_b16_mixed_topology_inter_420_b16x16": (
        "animated_lossy_b16_mixed_topology_inter_420_b16x16.avif",
        "3264482e2a50d80bd39be178b843fdfe9997e947d54b4466c745751837821d74",
    ),
    "lossy_wide_monochrome_b128x128": (
        "animated_lossy_wide_monochrome_b128x128.avif",
        "eb8dda5000882ffd03c182a817e08fc944ef110afa1e78aaf1678bef5efc65bf",
    ),
    "lossy_wide_i444_b128x128": (
        "animated_lossy_wide_i444_b128x128.avif",
        "b4693fd2fb43d1c9406fe4ac8d782d74709bf8eed1b2d36771c0bd80763d84e8",
    ),
    "lossy_wide_i444_mode2_unsplit_b128x128": (
        "animated_lossy_wide_i444_mode2_unsplit_b128x128.avif",
        "ea87cfc19c427135396b44ea6b287b24e5301437abb5bd6769666d9a24144565",
    ),
    "lossy_wide_i444_mode2_split32_b128x128": (
        "animated_lossy_wide_i444_mode2_split32_b128x128.avif",
        "f48a6d235c7ab245e8d57aedcc6135b897cea1bcbc579c3d630aa7c20e1a61aa",
    ),
    "lossy_global_rotzoom_compound_i444_b128x128": (
        "animated_lossy_global_rotzoom_compound_i444_b128x128.avif",
        "14ffb1529c54f07a397a1fe94d02135f9def3a7730416cb80d5acbf3c54c144e",
    ),
    "lossy_global_halfblend_spatial_i444_b256x256": (
        "animated_lossy_global_halfblend_spatial_i444_b256x256.avif",
        "ec2f145f64fa87c5b7c255b13b6009fb58760be06070724ed367cd4ccd9a6d7b",
    ),
    "motion_chroma": ("animated_motion_chroma.avif", "473469335e58cff3c442df51b4bbdca00b9467e8f352319812f50cf789748535"),
    "has_chroma_4x4": ("animated_has_chroma_4x4.avif", "f9434d10ee465784ccb6c09f03c98813282f7033e3f06b90739e7815ec1eb75b"),
    "motion_chroma_422": ("animated_motion_chroma_422.avif", "5a83530dcc60f75f0bec7d36cb4db67ae7258b2acc481cbb08cc0987247ea66a"),
    "lossy_inter_422_checker_random_b32x32": (
        "animated_lossy_inter_422_checker_random_b32x32.avif",
        "e46aef4aed9076da48627a449e53c6f318a0494ba5d94a32a4f13edc548ab134",
    ),
    "motion_chroma_wide": ("animated_motion_chroma_wide.avif", "54ca9823b7b3f6f64af19d3b7ab7dbc0be0b9db6583d85fedcc310dd62d28311"),
    "tx64_root_split_inter_420_64x64": (
        "animated_tx64_root_split_inter_420_64x64.avif",
        "9f4450d4d9c7c2738d4f9f34eafb02100c5fb15b85ac121a057c3eef367f6d7c",
    ),
    "motion_temporal_window_left_512x128": (
        "animated_motion_temporal_window_left_512x128.avif",
        "29e856c6c8a117c764bb0a176ccc1d82bac982fdf7287eb461f827ebdc31b399",
    ),
    "motion_temporal_window_right_512x128": (
        "animated_motion_temporal_window_right_512x128.avif",
        "10d8e5514c1d8d9cad355f9a912b3e3075bf8bd661ee6af5ce44a9ce62d7cad2",
    ),
    "lossy_inter_i444_mode2_b32x32": (
        "animated_lossy_inter_i444_mode2_b32x32.avif",
        "43aac6364eebb113b128861f0e1c2cfb976295d7b5e9e226fa8f0eaa93704af4",
    ),
    "lossy_inter_i444_split_b16x32_mode2": (
        "animated_lossy_inter_i444_split_b16x32_mode2.avif",
        "ad7ce564a11440b91237e0dfffedbde1053bc663ae0e9bf0894f87e05e669bd0",
    ),
    "lossy_inter_i444_obmc_mixed_b16x32": (
        "animated_lossy_inter_i444_obmc_mixed_b16x32.avif",
        "71e737f7b196173d98ae6447925cd8f6f77d95ced3eaa80d924ad81febcb3743",
    ),
    "motion_large_420": ("animated_motion_large_420.avif", "5b9ea2b9d552e8ff44f2818a7ae2b73a9dbe1eda84ead9c24c9f8a957c6e546f"),
    "highdepth": ("10bit.avif", "3bf9f91da471749e7df639ba7945d4d94c1c3e3968c26f3619fbbcfc92790576"),
    "lo8_superres_sgr_inter_160x56": (
        "animated_lossy_inter_420_superres_sgr_8bit_160x56.avif",
        "c294163610d4a45852fe374e0345c878979bb81e5ea94596960ef64411180fd7",
    ),
    "hi10_superres_sgr_inter_160x56": (
        "animated_lossy_inter_420_superres_sgr_10bit_160x56.avif",
        "016e4a8433002b60899744fba6f26a7c10af82c192f65b4c5c19233b15c3cb11",
    ),
    "hi10_superres_select_inter_160x56": (
        "animated_lossy_inter_420_superres_select_10bit_160x56.avif",
        "04e31a3ac36c25ef77061a2ed09b79fd8fb885a5ff14d6ab3a82e2d32ad3b02e",
    ),
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
        message = str(error)
        if message.startswith("cannot identify image file <_io.BytesIO object at 0x"):
            message = "cannot identify image file <bytes>"
        return {"status": "error", "type": f"{type(error).__module__}.{type(error).__name__}",
                "message": message}


def collect(bundle, observe, only=None):
    originals, records = {}, []
    fixtures = FIXTURES.items() if only is None else [(only, FIXTURES[only])]
    for name, (filename, expected_hash) in fixtures:
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
    ] if only is None else []
    for name, parameters in cases:
        records.append((name, "animated", mutate(originals["animated"], **parameters), parameters))
    if only is None:
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
    parser.add_argument(
        "--only", choices=sorted(FIXTURES), help="collect one unchanged fixture only"
    )
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
        cases = collect(
            bundle,
            lambda *values: native(library._handle, *values),
            only=args.only,
        )
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
