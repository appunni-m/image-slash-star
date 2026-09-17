#!/usr/bin/env python3
"""Collect full-file HDR color evidence using only pinned native oracles.

Pillow's bundled libavif decodes the unchanged AVIF and converts its planes to
RGB. A separately built, unmodified dav1d must produce identical planes; the
pinned libyuv scalar source must produce identical RGB. No Rust is executed.
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

from generate_av1_reconstruction_refs import (
    DAV1D_COMMIT, resolve_tool, sha256, tool_environment, verify_source,
)
from generate_av1_sequence_refs import (
    COPYING_SHA256, ROOT, artifact_records, build_dav1d, copy_source,
    digest_file, git_tree_digest, inspect_avif, json_bytes, make_ivf, run,
)
from generate_av1_temporal_refs import decode

LIBAVIF_COMMIT = "6543b22b5bc706c53f038a16fe515f921556d9b3"
LIBYUV_COMMIT = "6067afde563c3946eebd94f146b3824ab7a97a9c"
FIXTURE = ROOT / "tests/fixtures/input/images/avif/hdr.avif"
FIXTURE_SHA256 = "9980e58ddf718a923f1738c34aad1c72f8e5795ec07e68f1a5f9bd216ca19740"
SCHEMA = "image-slash-star/avif-hdr-color-oracle@1"

# Compiled against the pinned header, avoiding a handwritten ctypes ABI.
# dlsym accesses Pillow's already loaded native oracle, never repository code.
HARNESS = r'''
#include <avif/avif.h>
#include <libyuv/row.h>
#include <libyuv/convert_argb.h>
#include <dlfcn.h>
#include <cstring>
#include <vector>

extern "C" int observe(void * library, const uint8_t * input, size_t size,
                       uint16_t * planes, uint8_t * rgb, uint8_t * scalar,
                       uint32_t * strides) {
#define LOAD(name) \
    auto p_##name = reinterpret_cast<decltype(&name)>(dlsym(library, #name)); \
    if (!p_##name) return 1
    LOAD(avifImageCreateEmpty);
    LOAD(avifImageDestroy);
    LOAD(avifDecoderCreate);
    LOAD(avifDecoderDestroy);
    LOAD(avifDecoderReadMemory);
    LOAD(avifRGBImageSetDefaults);
    LOAD(avifImageYUVToRGB);
#undef LOAD
    avifImage * image = p_avifImageCreateEmpty();
    avifDecoder * decoder = p_avifDecoderCreate();
    auto finish = [&](int status) {
        if (image) p_avifImageDestroy(image);
        if (decoder) p_avifDecoderDestroy(decoder);
        return status;
    };
    if (!image || !decoder) return finish(2);
    decoder->codecChoice = AVIF_CODEC_CHOICE_DAV1D;
    decoder->maxThreads = 1;
    if (p_avifDecoderReadMemory(decoder, image, input, size) != AVIF_RESULT_OK)
        return finish(3);
    if (image->width != 200 || image->height != 200 || image->depth != 10 ||
        image->yuvFormat != AVIF_PIXEL_FORMAT_YUV444 || image->yuvRange != AVIF_RANGE_FULL ||
        image->colorPrimaries != 9 || image->transferCharacteristics != 16 ||
        image->matrixCoefficients != 9 || image->alphaPlane || image->icc.size)
        return finish(4);
    constexpr size_t count = 200 * 200;
    for (size_t plane = 0; plane < 3; ++plane) {
        if (!image->yuvPlanes[plane] || image->yuvRowBytes[plane] < 400)
            return finish(5);
        strides[plane] = image->yuvRowBytes[plane];
        for (size_t y = 0; y < 200; ++y) {
            for (size_t x = 0; x < 200; ++x) {
                uint16_t sample;
                std::memcpy(&sample, image->yuvPlanes[plane] + y * strides[plane] + x * 2, 2);
                if (sample > 1023) return finish(6);
                planes[plane * count + y * 200 + x] = sample;
            }
        }
    }
    avifRGBImage output;
    p_avifRGBImageSetDefaults(&output, image);
    output.depth = 8;
    output.format = AVIF_RGB_FORMAT_RGB;
    output.pixels = rgb;
    output.rowBytes = 200 * 3;
    if (p_avifImageYUVToRGB(image, &output) != AVIF_RESULT_OK) return finish(7);
    std::vector<uint8_t> downshifted(count * 3);
    for (size_t plane = 0; plane < 3; ++plane) {
        libyuv::Convert16To8Row_C(planes + plane * count,
                                  downshifted.data() + plane * count, 16384, count);
    }
    // libavif RGB uses the RGB24 row with U/V and the matrix both reversed.
    libyuv::I444ToRGB24Row_C(downshifted.data(), downshifted.data() + count * 2,
                             downshifted.data() + count, scalar,
                             &libyuv::kYvuV2020Constants, count);
    return finish(0);
}
'''


def pinned_source(path: Path, commit: str, files: tuple[str, ...]) -> dict:
    revision = run(["git", "-C", str(path), "rev-parse", "HEAD"]).stdout.decode().strip()
    dirty = run(["git", "-C", str(path), "status", "--porcelain"]).stdout
    if revision != commit or dirty:
        raise RuntimeError(f"source must be a clean checkout of {commit}: {path}")
    return {
        "commit": revision,
        "tree_sha256": git_tree_digest(path),
        "files": [{"path": name, "sha256": digest_file(path / name)} for name in files],
    }


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
        raise RuntimeError("libyuv version differs from the pinned oracle")
    sources = {
        "libavif": pinned_source(args.libavif_source, LIBAVIF_COMMIT,
                                  ("include/avif/avif.h", "src/reformat_libyuv.c", "LICENSE")),
        "libyuv": pinned_source(args.libyuv_source, LIBYUV_COMMIT,
                                 ("source/row_common.cc", "include/libyuv/version.h", "LICENSE")),
        "dav1d": pinned_source(args.dav1d_source, DAV1D_COMMIT, ("COPYING",)),
    }
    verify_source(args.dav1d_source)
    if digest_file(args.dav1d_source / "COPYING") != COPYING_SHA256:
        raise RuntimeError("dav1d license differs from the pinned source")
    for name in ("libavif", "libyuv"):
        if digest_file(getattr(args, f"{name}_source") / "LICENSE") != digest_file(ROOT / "third_party" / name / "LICENSE"):
            raise RuntimeError(f"{name} license differs from the retained license")
    data = FIXTURE.read_bytes()
    if sha256(data) != FIXTURE_SHA256:
        raise RuntimeError("HDR AVIF differs from the registered full-file input")
    container = inspect_avif(FIXTURE)
    item, = container["items"]["color"]
    span, = item["spans"]
    if container["items"]["alpha"]:
        raise RuntimeError("unexpected alpha input")
    output = args.output.resolve()
    staging = (ROOT / "target/oracle-staging").resolve()
    if staging not in output.parents or output.exists():
        raise RuntimeError("output must be a fresh directory below target/oracle-staging")
    output.parent.mkdir(parents=True, exist_ok=True)
    env = tool_environment(args.meson, args.ninja, None)
    with tempfile.TemporaryDirectory(prefix=".hdr-color-", dir=output.parent) as temporary:
        work = Path(temporary)
        bundle = work / "evidence"
        bundle.mkdir()
        harness = work / "observe.cc"
        harness.write_text(HARNESS)
        shared = work / "observe.so"
        command = [str(args.cxx), "-std=c++17", "-O2", "-Wall", "-Wextra", "-Werror", "-shared", "-fPIC",
                   f"-I{args.libavif_source / 'include'}", f"-I{args.libyuv_source / 'include'}",
                   str(harness), str(args.libyuv_source / "source/row_common.cc"), "-o", str(shared)]
        if platform.system() == "Linux":
            command.append("-ldl")
        run(command, env=env)
        observer = ctypes.CDLL(str(shared)).observe
        observer.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_size_t,
                             ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p]
        observer.restype = ctypes.c_int
        planes = (ctypes.c_uint16 * 120000)()
        rgb = (ctypes.c_uint8 * 120000)()
        scalar = (ctypes.c_uint8 * 120000)()
        strides = (ctypes.c_uint32 * 3)()
        observations = []
        for _ in range(2):
            status = observer(library._handle, data, len(data), planes, rgb, scalar, strides)
            if status:
                raise RuntimeError(f"native HDR observer failed at stage {status}")
            observations.append((struct.pack("<120000H", *planes), bytes(rgb), bytes(scalar)))
        if observations[0] != observations[1]:
            raise RuntimeError("native HDR observations are not deterministic")
        yuv, native_rgb, scalar_rgb = observations[0]
        with Image.open(FIXTURE) as image:
            image.load()
            if image.mode != "RGB" or image.size != (200, 200) or image.n_frames != 1:
                raise RuntimeError("unexpected Pillow output declaration")
            pillow_rgb = image.tobytes()
        if not (native_rgb == scalar_rgb == pillow_rgb):
            raise RuntimeError("libavif, scalar libyuv, and Pillow RGB differ")
        ivf = work / "hdr.ivf"
        ivf_record = make_ivf(data, [{**span, "sha256": item["sha256"]}], 200, 200, ivf)
        source = work / "dav1d-source"
        copy_source(args.dav1d_source, source)
        binary, build = build_dav1d(source, work / "dav1d-build", args.meson, args.ninja, env)
        native_yuv_path = work / "dav1d.yuv"
        if decode(binary, ivf, native_yuv_path, env):
            raise RuntimeError("unmodified dav1d emitted unexpected stdout")
        if native_yuv_path.read_bytes() != yuv:
            raise RuntimeError("standalone dav1d and Pillow libavif planes differ")
        (bundle / "display.yuv").write_bytes(yuv)
        (bundle / "display.rgb").write_bytes(pillow_rgb)
        (bundle / "observer.cc").write_text(HARNESS)
        index = {
            "schema": SCHEMA,
            "fixture": {"path": str(FIXTURE.relative_to(ROOT)), "bytes": len(data), "sha256": sha256(data)},
            "declaration": {"width": 200, "height": 200, "bit_depth": 10, "monochrome": False,
                            "color_primaries": 9, "transfer_characteristics": 16, "matrix_coefficients": 9,
                            "color_range": True, "subsampling_x": False, "subsampling_y": False, "alpha": False},
            "sources": sources,
            "oracle": {"pillow": Image.__version__, "libavif": features.version("avif"), "codecs": codecs,
                       "libyuv": version(), "pillow_avif_sha256": digest_file(Path(_avif.__file__)),
                       "pillow_imaging_sha256": digest_file(Path(_imaging.__file__)),
                       "platform": platform.platform(), "machine": platform.machine(),
                       "compiler": run([str(args.cxx), "--version"]).stdout.decode().strip(),
                       "observer_compile_argv": [
                           value.replace(str(args.libavif_source), "<libavif-source>")
                           .replace(str(args.libyuv_source), "<libyuv-source>")
                           .replace(str(work), "<work>") for value in command
                       ],
                       "observer_sha256": digest_file(shared), "dav1d_binary_sha256": digest_file(binary),
                       "dav1d_build": build,
                       "dav1d_compilers": json.loads((work / "dav1d-build/meson-info/intro-compilers.json").read_text())},
            "layout": {"yuv": "planar Y/U/V, little-endian u16, 200 samples per row, no padding",
                       "libavif_row_bytes": list(strides), "rgb": "interleaved RGB8, 600 bytes per row",
                       "downshift": "libyuv Convert16To8Row_C scale=16384 before I444ToRGB24Row_C",
                       "matrix": "kYvuV2020Constants with reversed U/V for RGB output",
                       "sample_ranges": [
                           [min(planes[start:start + 40000]), max(planes[start:start + 40000])]
                           for start in (0, 40000, 80000)
                       ],
                       "cpu_dispatch": "Pillow/libavif default; standalone libyuv scalar C; standalone dav1d cpumask=0"},
            "sample": ivf_record,
            "checks": {"repeat_equal": True, "pillow_libavif_scalar_rgb_equal": True,
                       "dav1d_libavif_yuv_equal": True, "rust_executed": False},
            "artifacts": artifact_records(bundle),
            "limitations": ["Color boundary evidence only; Rust reconstruction and public decode are unexecuted.",
                            "No alpha, subsampling, limited range, other depth, or animation witness.",
                            "The observed sample ranges do not witness nominal extrema or RGB clipping.",
                            "Pillow RGB preserves PQ-encoded values; no tone or gamut mapping is added."],
        }
        (bundle / "index.json").write_bytes(json_bytes(index))
        bundle.rename(output)
    print(json.dumps({"output": str(output), "yuv_sha256": sha256(yuv), "rgb_sha256": sha256(pillow_rgb)}))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("dav1d", "libavif", "libyuv"):
        parser.add_argument(f"--{name}-source", type=Path, required=True)
    parser.add_argument("--meson", type=Path, required=True)
    parser.add_argument("--ninja", type=Path, required=True)
    parser.add_argument("--cxx", type=Path, default=Path("/usr/bin/clang++"))
    parser.add_argument("--output", type=Path, default=ROOT / "target/oracle-staging/avif-hdr-color/hdr")
    args = parser.parse_args()
    for name in ("meson", "ninja", "cxx"):
        setattr(args, name, resolve_tool(getattr(args, name), name))
    collect(args)


if __name__ == "__main__":
    main()
