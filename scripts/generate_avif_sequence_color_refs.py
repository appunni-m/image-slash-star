#!/usr/bin/env python3
"""Observe native planes and pixels for the registered high-depth AVIF track.

The entire existing file is decoded independently by pinned Pillow/libavif and
unmodified scalar dav1d. No Rust code or Python color conversion is executed.
"""
from __future__ import annotations

import argparse
import ctypes
import json
import platform
import struct
import tempfile
from pathlib import Path

from PIL import Image, _avif, _imaging, features

from generate_av1_reconstruction_refs import DAV1D_COMMIT, resolve_tool, tool_environment
from generate_av1_sequence_refs import (
    ROOT, artifact_records, build_dav1d, copy_source, digest_file, inspect_avif,
    json_bytes, make_ivf, run, sha256, track_report,
)
from generate_av1_temporal_refs import decode
from generate_avif_hdr_color_refs import LIBAVIF_COMMIT, LIBYUV_COMMIT, pinned_source

FIXTURE = ROOT / "tests/fixtures/input/images/avif/10bit.avif"
FIXTURE_SHA256 = "3bf9f91da471749e7df639ba7945d4d94c1c3e3968c26f3619fbbcfc92790576"
SCHEMA = "image-slash-star/avif-sequence-color-oracle@1"
HARNESS = r'''
#include <avif/avif.h>
#include <libyuv/row.h>
#include <libyuv/scale_row.h>
#include <libyuv/convert_argb.h>
#include <dlfcn.h>
#include <cstring>
#include <array>
#if defined(LIBYUV_UNLIMITED_DATA) || defined(LIBYUV_UNLIMITED_BT601)
#error This oracle requires the pinned default I601 constants
#endif

extern "C" int observe(void * library, const uint8_t * input, size_t size,
                       uint32_t frame, uint16_t * planes, uint8_t * rgba,
                       uint64_t * timing) {
#define LOAD(name) \
    auto p_##name = reinterpret_cast<decltype(&name)>(dlsym(library, #name)); \
    if (!p_##name) return 1
    LOAD(avifDecoderCreate);
    LOAD(avifDecoderDestroy);
    LOAD(avifDecoderSetIOMemory);
    LOAD(avifDecoderParse);
    LOAD(avifDecoderNthImage);
    LOAD(avifRGBImageSetDefaults);
    LOAD(avifImageYUVToRGB);
#undef LOAD
    avifDecoder * decoder = p_avifDecoderCreate();
    if (!decoder) return 2;
    auto finish = [&](int status) { p_avifDecoderDestroy(decoder); return status; };
    decoder->codecChoice = AVIF_CODEC_CHOICE_DAV1D;
    decoder->maxThreads = 1;
    if (p_avifDecoderSetIOMemory(decoder, input, size) != AVIF_RESULT_OK ||
        p_avifDecoderParse(decoder) != AVIF_RESULT_OK) return finish(3);
    if (decoder->imageCount != 5 || frame >= 5 ||
        p_avifDecoderNthImage(decoder, frame) != AVIF_RESULT_OK) return finish(4);
    const avifImage * image = decoder->image;
    if (image->width != 64 || image->height != 64 || image->depth != 12 ||
        image->yuvFormat != AVIF_PIXEL_FORMAT_YUV422 || image->yuvRange != AVIF_RANGE_LIMITED ||
        image->colorPrimaries != 2 || image->transferCharacteristics != 2 ||
        image->matrixCoefficients != 2 || !image->alphaPlane || image->alphaPremultiplied ||
        image->icc.size || image->transformFlags) return finish(5);
    size_t offset = 0;
    for (size_t plane = 0; plane < 4; ++plane) {
        const uint8_t * pixels = plane == 3 ? image->alphaPlane : image->yuvPlanes[plane];
        const uint32_t stride = plane == 3 ? image->alphaRowBytes : image->yuvRowBytes[plane];
        const size_t width = (plane == 1 || plane == 2) ? 32 : 64;
        if (!pixels || stride < width * 2) return finish(6);
        for (size_t y = 0; y < 64; ++y) {
            for (size_t x = 0; x < width; ++x) {
                uint16_t sample;
                std::memcpy(&sample, pixels + y * stride + x * 2, 2);
                if (sample > 4095) return finish(7);
                planes[offset++] = sample;
            }
        }
    }
    avifRGBImage output;
    p_avifRGBImageSetDefaults(&output, image);
    output.depth = 8;
    output.format = AVIF_RGB_FORMAT_RGBA;
    output.rowBytes = 64 * 4;
    output.pixels = rgba;
    if (p_avifImageYUVToRGB(image, &output) != AVIF_RESULT_OK) return finish(8);
    // Call the pinned scalar primitives with libyuv's endpoint convention.
    // Arithmetic and expected pixels are supplied by upstream, not a port.
    std::array<uint8_t, 12288> downshifted;
    libyuv::Convert16To8Row_C(planes, downshifted.data(), 4096, 12288);
    for (size_t row = 0; row < 64; ++row) {
        std::array<std::array<uint8_t, 64>, 2> chroma;
        for (size_t plane = 0; plane < 2; ++plane) {
            const uint8_t * src = downshifted.data() + 4096 + plane * 2048 + row * 32;
            chroma[plane][0] = src[0];
            libyuv::ScaleRowUp2_Linear_C(src, chroma[plane].data() + 1, 62);
            chroma[plane][63] = src[31];
        }
        std::array<uint8_t, 256> scalar;
        libyuv::I444AlphaToARGBRow_C(downshifted.data() + row * 64,
                                     chroma[1].data(), chroma[0].data(),
                                     downshifted.data() + 8192 + row * 64,
                                     scalar.data(), &libyuv::kYvuI601Constants, 64);
        if (std::memcmp(scalar.data(), rgba + row * 256, 256)) return finish(9);
    }
    timing[0] = decoder->imageTiming.timescale;
    timing[1] = decoder->imageTiming.ptsInTimescales;
    timing[2] = decoder->imageTiming.durationInTimescales;
    // Preserve the signed repetition value as two fields, without narrowing.
    timing[3] = decoder->repetitionCount < 0;
    timing[4] = timing[3] ? -int64_t(decoder->repetitionCount) : decoder->repetitionCount;
    return finish(0);
}
'''


def collect(args: argparse.Namespace) -> None:
    codecs = _avif.codec_versions()
    if (Image.__version__ != "12.2.0" or features.version("avif") != "1.4.1"
            or codecs != "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2"):
        raise RuntimeError("Pillow/libavif/codec identity differs from the pinned oracle")
    library = ctypes.CDLL(_avif.__file__)
    version = library.avifLibYUVVersion
    version.argtypes = []
    version.restype = ctypes.c_uint
    if version() != 1922:
        raise RuntimeError("libyuv differs from the pinned oracle")
    sources = {
        "libavif": pinned_source(args.libavif_source, LIBAVIF_COMMIT,
                                  ("include/avif/avif.h", "src/reformat_libyuv.c", "LICENSE")),
        "libyuv": pinned_source(args.libyuv_source, LIBYUV_COMMIT,
                                 ("source/row_common.cc", "source/scale_common.cc", "source/scale_any.cc", "source/cpu_id.cc", "LICENSE")),
        "dav1d": pinned_source(args.dav1d_source, DAV1D_COMMIT, ("COPYING",)),
    }
    for name, filename in (("libavif", "LICENSE"), ("libyuv", "LICENSE"), ("dav1d", "COPYING")):
        if digest_file(getattr(args, f"{name}_source") / filename) != digest_file(ROOT / "third_party" / name / filename):
            raise RuntimeError(f"{name} license differs from the retained license")
    data = FIXTURE.read_bytes()
    if sha256(data) != FIXTURE_SHA256:
        raise RuntimeError("registered high-depth input has changed")
    container = inspect_avif(FIXTURE)
    output = args.output.resolve()
    if (ROOT / "target/oracle-staging").resolve() not in output.parents or output.exists():
        raise RuntimeError("output must be a fresh directory below target/oracle-staging")
    output.parent.mkdir(parents=True, exist_ok=True)
    env = tool_environment(args.meson, args.ninja, None)
    with tempfile.TemporaryDirectory(prefix=".sequence-color-", dir=output.parent) as temporary:
        work = Path(temporary)
        bundle = work / "evidence"
        bundle.mkdir()
        harness = work / "observer.cc"
        harness.write_text(HARNESS)
        shared = work / "observer.so"
        command = [str(args.cxx), "-std=c++17", "-O2", "-Wall", "-Wextra", "-Werror", "-shared", "-fPIC",
                   "-DLIBYUV_DISABLE_NEON", "-DLIBYUV_DISABLE_X86",
                   f"-I{args.libavif_source / 'include'}", f"-I{args.libyuv_source / 'include'}", str(harness),
                   str(args.libyuv_source / "source/row_common.cc"), str(args.libyuv_source / "source/scale_common.cc"),
                   str(args.libyuv_source / "source/cpu_id.cc"), "-o", str(shared)]
        if platform.system() == "Linux": command.append("-ldl")
        run(command, env=env)
        observer = ctypes.CDLL(str(shared)).observe
        observer.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_size_t, ctypes.c_uint,
                             ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p]
        observer.restype = ctypes.c_int
        native_planes = (ctypes.c_uint16 * 12288)()
        native_rgba = (ctypes.c_uint8 * 16384)()
        timing = (ctypes.c_uint64 * 5)()
        frames = []
        color_yuv, alpha_yuv = bytearray(), bytearray()
        with Image.open(FIXTURE) as image:
            if image.n_frames != 5: raise RuntimeError("Pillow frame count changed")
            for frame in range(5):
                observations = []
                for _ in range(2):
                    status = observer(library._handle, data, len(data), frame, native_planes, native_rgba, timing)
                    if status: raise RuntimeError(f"native frame {frame} observer failed at stage {status}")
                    observations.append((struct.pack("<12288H", *native_planes), bytes(native_rgba), list(timing)))
                if observations[0] != observations[1]: raise RuntimeError("native observation is not deterministic")
                yuva, rgba, observed_timing = observations[0]
                image.seek(frame)
                image.load()
                if image.mode != "RGBA" or image.size != (64, 64) or image.tobytes() != rgba:
                    raise RuntimeError("Pillow and native libavif frame pixels differ")
                color_yuv.extend(yuva[:16384])
                alpha_yuv.extend(yuva[16384:])
                (bundle / f"frame_{frame}.yuva").write_bytes(yuva)
                (bundle / f"frame_{frame}.rgba").write_bytes(rgba)
                frames.append({"index": frame, "timescale": observed_timing[0], "pts": observed_timing[1],
                               "duration": observed_timing[2], "repetition_count": (-1 if observed_timing[3] else 1) * observed_timing[4],
                               "pillow_duration_ms": image.info.get("duration"), "pillow_loop": image.info.get("loop"),
                               "sample_ranges": [[min(native_planes[a:b]), max(native_planes[a:b])] for a,b in [(0,4096),(4096,6144),(6144,8192),(8192,12288)]]})
        source = work / "dav1d-source"
        copy_source(args.dav1d_source, source)
        binary, build = build_dav1d(source, work / "dav1d-build", args.meson, args.ninja, env)
        tracks = {}
        for role, handler, key, expected in [("color", "pict", "color_track_id", color_yuv), ("alpha", "auxv", "alpha_track_id", alpha_yuv)]:
            track = track_report(container, handler, container[key])
            if track is None or len(track["samples"]) != 5: raise RuntimeError("track samples changed")
            ivf = work / f"{role}.ivf"
            tracks[role] = make_ivf(data, track["samples"], 64, 64, ivf)
            raw = work / f"{role}.yuv"
            if decode(binary, ivf, raw, env) or raw.read_bytes() != expected:
                raise RuntimeError(f"standalone dav1d and libavif {role} planes differ")
        (bundle / "observer.cc").write_text(HARNESS)
        index = {
            "schema": SCHEMA, "fixture": {"path": str(FIXTURE.relative_to(ROOT)), "bytes": len(data), "sha256": sha256(data)},
            "declaration": {"width":64,"height":64,"bit_depth":12,"monochrome":False,"color_primaries":2,"transfer_characteristics":2,"matrix_coefficients":2,"color_range":False,"subsampling_x":True,"subsampling_y":False,"alpha":True},
            "frames": frames, "tracks": tracks, "sources": sources,
            "oracle": {"pillow": Image.__version__, "libavif": features.version("avif"), "libyuv":version(), "codecs":codecs,
                       "pillow_avif_sha256":digest_file(Path(_avif.__file__)),"pillow_imaging_sha256":digest_file(Path(_imaging.__file__)),
                       "platform":platform.platform(),"compiler":run([str(args.cxx),"--version"]).stdout.decode().strip(),
                       "compile_argv":[v.replace(str(args.libavif_source),"<libavif-source>").replace(str(args.libyuv_source),"<libyuv-source>").replace(str(work),"<work>") for v in command],
                       "libyuv_unlimited_data":False,"libyuv_unlimited_bt601":False,
                       "observer_sha256":digest_file(shared),"dav1d_binary_sha256":digest_file(binary),"dav1d_build":build,
                       "dav1d_compilers":json.loads((work/'dav1d-build/meson-info/intro-compilers.json').read_text())},
            "layout": {"yuva":"little-endian u16 planes Y64x64, U32x64, V32x64, A64x64; no padding", "rgba":"interleaved RGBA8; straight alpha", "cpu_dispatch":"Pillow/libavif default; standalone dav1d cpumask=0; standalone libyuv scalar C primitives, NEON/X86 disabled"},
            "checks":{"native_repeat_equal":True,"pillow_libavif_rgba_equal":True,"scalar_libyuv_libavif_rgba_equal":True,"dav1d_libavif_color_alpha_equal":True,"rust_executed":False},
            "artifacts":artifact_records(bundle),
            "limitations":["Native color/alpha and timing evidence only; Rust reconstruction, sequence presentation and color execution are unverified.","Only the existing 12-bit limited-range I422 CICP2/2/2 auxiliary-alpha declaration is witnessed.","Partial alpha distinguishes straight from premultiplied output; the fixture contains no zero-alpha pixels."]}
        (bundle / "index.json").write_bytes(json_bytes(index))
        bundle.rename(output)
    print(json.dumps({"output":str(output),"frames":5,"color_yuv_sha256":sha256(color_yuv),"alpha_yuv_sha256":sha256(alpha_yuv)}))


def main() -> None:
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ("dav1d","libavif","libyuv"): parser.add_argument(f"--{name}-source",type=Path,required=True)
    parser.add_argument("--meson",type=Path,required=True)
    parser.add_argument("--ninja",type=Path,required=True)
    parser.add_argument("--cxx",type=Path,default=Path("/usr/bin/clang++"))
    parser.add_argument("--output",type=Path,default=ROOT/'target/oracle-staging/avif-sequence-color/high_bitdepth')
    args=parser.parse_args()
    for name in ("meson","ninja","cxx"): setattr(args,name,resolve_tool(getattr(args,name),name))
    collect(args)

if __name__ == "__main__": main()
