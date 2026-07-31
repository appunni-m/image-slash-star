#!/usr/bin/env python3
"""Generate authoritative Pillow references for decode and encode cases.

Decode: PIL open asset -> .tobytes() -> .bin reference -> matrix
Encode:  PIL open source -> .save(format, params) -> reopen -> .tobytes() -> .bin reference

The exact Pillow wheel and its bundled codec versions are pinned in
``manifest.yaml``. Only public Pillow-observable behavior is part of the oracle.
"""
import argparse
import hashlib
import io
import json
import platform
import re
import struct
import sys
import zlib
from pathlib import Path

import yaml

ROOT = Path(__file__).parent.parent
MANIFEST = ROOT / "manifest.yaml"
ORACLE_LOCK = ROOT / "pillow-oracle.lock.yaml"
MATRIX_PATH = ROOT / "tests" / "fixtures" / "coverage_matrix.json"
INPUT_JSONS = ROOT / "tests" / "fixtures" / "input" / "jsons"
OUTPUT_JSONS = ROOT / "tests" / "fixtures" / "outputs" / "jsons"
OUTPUT_RAWS = ROOT / "tests" / "fixtures" / "outputs" / "raws"
OUTPUT_ENCODED = ROOT / "tests" / "fixtures" / "outputs" / "encoded"
ASSETS_DIR = ROOT / "tests" / "fixtures" / "input" / "images"

ASSERTION_ORIGINS = {
    "pillow_fixture",
    "specification_reference",
    "independent_implementation",
    "defensive_model",
}
ALL_CODEC_FEATURES = ["jpeg", "png", "gif", "bmp", "tiff", "webp", "ico", "avif"]


def pillow_open_asset(path):
    """Open fixture bytes with a stable name for deterministic Pillow errors."""
    from PIL import Image

    stream = io.BytesIO(path.read_bytes())
    stream.name = path.relative_to(ROOT).as_posix()
    return Image.open(stream)


def stable_error_message(error):
    """Normalize only runtime object addresses from byte-stream errors."""
    return re.sub(
        r"<_io\.BytesIO object at 0x[0-9a-fA-F]+>",
        "<bytes>",
        str(error),
    )


def decode_error_kind(detects_format, error):
    """Map a Pillow decode/open failure to the canonical public error category."""
    if not detects_format:
        return "unknown_format"
    message = stable_error_message(error)
    qualified = f"{type(error).__module__}.{type(error).__name__}"
    if qualified == "PIL.Image.DecompressionBombError" or message in {
        "Invalid dimensions",
        "Invalid tile dimensions",
        "tile cannot extend outside image",
    }:
        return "dimensions"
    if message.startswith("Unsupported "):
        return "unsupported"
    return "malformed"


def encode_error_kind(row, error):
    """Map a Pillow save failure using its manifest-declared caller input."""
    params = row.get("params", {})
    message = stable_error_message(error)
    if "source_dimensions" in params:
        return "dimensions"
    if params.get("truncate_pixels"):
        return "dimensions"
    if (
        "cannot write mode " in message
        or "image has wrong mode" in message
        or message == "'CMYK'"
        or "encoder error" in message
    ):
        return "unsupported"
    return "parameter"


def avif_detection_oracle(data):
    """Apply the bounded AVIF compatibility rule used by common detection.

    Pillow's prefix predicate deliberately admits bare mif1/msf1 major brands
    and delegates the complete brand check to libavif. The public Rust detector
    has no plugin fallthrough stage, so generic HEIF majors require an avif or
    avis compatible brand here. avif/avis major brands retain Pillow's
    signature-level behavior so malformed codec bodies remain recognizable.
    """
    prefix = data[:16]
    if prefix[4:12] in (b"ftypavif", b"ftypavis"):
        return True
    if prefix[4:12] not in (b"ftypmif1", b"ftypmsf1"):
        return False
    size = int.from_bytes(prefix[:4], "big")
    if size < 20 or size > len(data) or size % 4:
        return False
    return any(
        brand in (b"avif", b"avis")
        for brand in (data[offset : offset + 4] for offset in range(16, size, 4))
    )


def oracle_detects_format(fmt_name, data):
    """Apply the pinned detection oracle for one common format."""
    from PIL import (
        AvifImagePlugin,
        BmpImagePlugin,
        CurImagePlugin,
        GifImagePlugin,
        IcoImagePlugin,
        JpegImagePlugin,
        PngImagePlugin,
        TiffImagePlugin,
        WebPImagePlugin,
    )

    prefix = data[:16]
    if fmt_name == "ico":
        return bool(IcoImagePlugin._accept(prefix) or CurImagePlugin._accept(prefix))
    if fmt_name == "avif":
        if not AvifImagePlugin.SUPPORTED:
            raise RuntimeError("AVIF detection oracle requires Pillow AVIF support")
        return avif_detection_oracle(data)
    accepts = {
        "jpeg": JpegImagePlugin._accept,
        "png": PngImagePlugin._accept,
        "gif": GifImagePlugin._accept,
        "bmp": BmpImagePlugin._accept,
        "tiff": TiffImagePlugin._accept,
        "webp": WebPImagePlugin._accept,
    }
    return bool(accepts[fmt_name](prefix))


def mode_name(img):
    m = {"L": "L8", "LA": "La8", "RGB": "Rgb8", "RGBA": "Rgba8", "1": "1", "P": "P"}
    return m.get(img.mode, img.mode)


def stable_id(value):
    return re.sub(r"[^a-z0-9]+", "_", value.lower()).strip("_")


def decode_row_id(case, asset_name):
    assets = case.get("test_assets", [])
    if not assets or assets[0] == asset_name:
        return case["id"]
    return f"{case['id']}_{stable_id(Path(asset_name).stem)}"


def ensure_decode_row(matrix, fmt_name, fmt_manifest, case, asset_name):
    fmt_matrix = matrix.setdefault("formats", {}).setdefault(fmt_name, {})
    rows = fmt_matrix.setdefault("decode", [])
    row_id = decode_row_id(case, asset_name)
    for row in rows:
        if row.get("id") == row_id:
            row.update(
                {
                    "asset": asset_name,
                    "format": fmt_name,
                    "type": "decode",
                    "category": case["id"].split("_", 1)[0],
                    "description": case.get("description", ""),
                    "expect_error": bool(case.get("expect_error", False)),
                    "expect_sequence_error": bool(
                        case.get("expect_sequence_error", False)
                    ),
                    "verification_scope": fmt_manifest["verification_scope"],
                    "rust_expect_sequence_error": bool(
                        case.get("rust_expect_sequence_error", False)
                    ),
                    "rust_sequence_error_kind": case.get(
                        "rust_sequence_error_kind"
                    ),
                    "rust_sequence_error_reason": case.get(
                        "rust_sequence_error_reason"
                    ),
                    "status": case.get("status", "active"),
                }
            )
            return row

    row = {
        "id": row_id,
        "type": "decode",
        "format": fmt_name,
        "category": case["id"].split("_", 1)[0],
        "description": case.get("description", ""),
        "asset": asset_name,
        "expect_error": bool(case.get("expect_error", False)),
        "expect_sequence_error": bool(case.get("expect_sequence_error", False)),
        "verification_scope": fmt_manifest["verification_scope"],
        "rust_expect_sequence_error": bool(
            case.get("rust_expect_sequence_error", False)
        ),
        "rust_sequence_error_kind": case.get("rust_sequence_error_kind"),
        "rust_sequence_error_reason": case.get("rust_sequence_error_reason"),
        "status": "active",
    }
    rows.append(row)
    return row


def sync_decode_rows(manifest, matrix):
    """Make manifest decode cases authoritative without deduplicating assets."""
    for fmt_name, fmt_manifest in manifest.get("formats", {}).items():
        fmt_matrix = matrix.setdefault("formats", {}).setdefault(fmt_name, {})
        existing = {row["id"]: row for row in fmt_matrix.get("decode", [])}
        synchronized = []
        seen = set()
        for case in fmt_manifest.get("edge_cases", []):
            # Keep format capabilities documented in manifest.yaml, but only
            # put operations Pillow can actually express into its oracle matrix.
            if case.get("status") == "planned" and case.get("oracle_gap"):
                continue
            for asset_name in case.get("test_assets", []):
                row_id = decode_row_id(case, asset_name)
                if row_id in seen:
                    raise RuntimeError(f"duplicate decode case id: {fmt_name}/{row_id}")
                seen.add(row_id)
                row = dict(existing.get(row_id, {}))
                case_status = case.get("status")
                if case_status is None:
                    case_status = (
                        "planned" if fmt_manifest.get("status") == "planned" else "active"
                    )
                row.update(
                    {
                        "id": row_id,
                        "type": "decode",
                        "format": fmt_name,
                        "category": case["id"].split("_", 1)[0],
                        "description": case.get("description", ""),
                        "asset": asset_name,
                        "expect_error": bool(case.get("expect_error", False)),
                        "expect_sequence_error": bool(
                            case.get("expect_sequence_error", False)
                        ),
                        "verification_scope": fmt_manifest["verification_scope"],
                        "rust_expect_sequence_error": bool(
                            case.get("rust_expect_sequence_error", False)
                        ),
                        "rust_sequence_error_kind": case.get(
                            "rust_sequence_error_kind"
                        ),
                        "rust_sequence_error_reason": case.get(
                            "rust_sequence_error_reason"
                        ),
                        "status": case_status,
                    }
                )
                if row["status"] == "planned":
                    row["gap"] = (
                        case.get("oracle_gap")
                        or case.get("gap")
                        or fmt_manifest.get("planned_gap_defaults", {}).get("decode")
                    )
                    if not row["gap"]:
                        raise RuntimeError(f"planned decode row has no gap reason: {fmt_name}/{row_id}")
                else:
                    row.pop("gap", None)
                if row["status"] == "planned" or row["expect_error"]:
                    clear_pixel_ref(row)
                synchronized.append(row)
        fmt_matrix["decode"] = synchronized
        fmt_matrix.setdefault("encode", [])


def fmt_pil(fmt):
    return {"jpeg": "JPEG", "png": "PNG", "gif": "GIF", "bmp": "BMP",
            "tiff": "TIFF", "webp": "WEBP", "ico": "ICO"}.get(fmt, fmt.upper())


def encode_params(fmt, params):
    """Map every semantic manifest parameter to one Pillow save operation.

    Unknown or unsupported parameters are errors. Pillow accepts and ignores
    arbitrary save keyword arguments, so passing them through would create
    false coverage rather than proving the requested behavior.
    """
    remaining = set(params)
    kwargs = {}

    def take(name, default=None):
        if name in remaining:
            remaining.remove(name)
            return params[name]
        return default

    # These are properties of the explicit source asset, not Image.save kwargs.
    for source_property in (
        "size",
        "color",
        "color_type",
        "bit_depth",
        "grayscale",
        "alpha",
        "truncate_pixels",
        "source_dimensions",
        "oversized_palette",
        "palette_on_nonindexed",
        "detach_source",
        "rust_unsupported_modes",
        "rust_invalid_color_mode",
        "encoded_only",
        "sequence_canvas_padding",
        "sequence_frame_offset",
        "sequence_frame_mode",
        "sequence_duration_ms",
        "sequence_duration_fraction",
        "sequence_disposal",
        "sequence_blend",
        "sequence_interlaced",
        "sequence_default_image",
        "sequence_pixel_layout",
        "sequence_loop_count",
        "sequence_clear_loop",
        "sequence_background_rgba",
        "sequence_background_palette",
        "sequence_clear_background",
    ):
        take(source_property)

    if fmt == "jpeg":
        for name in ("quality", "optimize", "progressive"):
            value = take(name)
            if value is not None:
                kwargs[name] = value
        subsampling = take("subsampling")
        if subsampling is not None:
            kwargs["subsampling"] = {
                "4:4:4": 0,
                "4:2:2": 1,
                "4:2:0": 2,
                "444": 0,
                "422": 1,
                "420": 2,
            }.get(subsampling, subsampling)
        restart_interval = take("restart_interval")
        if restart_interval is not None:
            kwargs["restart_marker_rows"] = restart_interval
        dct_method = take("dct_method")
        if dct_method is not None:
            kwargs["dct_method"] = dct_method
        exif = take("exif")
        if exif is False:
            kwargs["exif"] = b""
        elif exif is not None:
            raise RuntimeError("exif=true requires explicit EXIF bytes")
        exif_hex = take("exif_hex")
        if exif_hex is not None:
            kwargs["exif"] = bytes.fromhex(exif_hex)
    elif fmt == "png":
        compression = take("compression")
        if compression is not None:
            kwargs["compress_level"] = {
                "default": -1,
                "none": 0,
                "max": 9,
            }.get(compression, compression)
        optimize = take("optimize")
        if optimize is not None:
            kwargs["optimize"] = optimize
        row_filter = take("filter")
        if row_filter is not None:
            kwargs["filter"] = row_filter

        chunk_requests = {
            "text_chunks": (b"tEXt", b"Comment\x00pillow-rs"),
            "gamma": (b"gAMA", (45_455).to_bytes(4, "big")),
            "srgb": (b"sRGB", b"\x00"),
            "time": (b"tIME", bytes.fromhex("07ea0704000000")),
        }
        chunks = []
        for name, chunk in chunk_requests.items():
            if take(name) is True:
                chunks.append(chunk)
        if chunks:
            from PIL.PngImagePlugin import PngInfo

            pnginfo = PngInfo()
            for chunk_type, payload in chunks:
                pnginfo.add(chunk_type, payload)
            kwargs["pnginfo"] = pnginfo
        if take("physical") is True:
            kwargs["dpi"] = (72, 72)
        for name in ("interlace", "interlaced"):
            interlace = take(name)
            if interlace is not None:
                kwargs["interlace"] = interlace
    elif fmt == "gif":
        interlace = take("interlace")
        if interlace is not None:
            kwargs["interlace"] = interlace
        transparency = take("transparency")
        if transparency is True:
            kwargs["transparency"] = 0
        elif transparency not in (None, False):
            kwargs["transparency"] = transparency
        disposal = take("disposal")
        if disposal is not None:
            kwargs["disposal"] = {
                "none": 0,
                "background": 2,
                "previous": 3,
            }.get(disposal, disposal)
        loop = take("loop")
        if loop is True:
            kwargs["loop"] = 0
        animated = take("animated")
        frames = take("frames")
        if animated is not None:
            kwargs["_manifest_animated"] = animated
        if frames is not None:
            kwargs["_manifest_frames"] = frames
        preserve_disposal = take("preserve_disposal")
        if preserve_disposal is not None:
            kwargs["_manifest_preserve_disposal"] = preserve_disposal
        second_frame_mode = take("second_frame_mode")
        if second_frame_mode is not None:
            kwargs["_manifest_second_frame_mode"] = second_frame_mode
        color_table = take("color_table")
        if color_table == "local":
            kwargs["include_color_table"] = True
        if color_table not in (None, "global"):
            if color_table != "local":
                raise RuntimeError(f"unknown GIF color_table value {color_table!r}")
    elif fmt == "bmp":
        bit_depth = params.get("bit_depth")
        if bit_depth is not None:
            kwargs["bit_depth"] = bit_depth
        compression = take("compression")
        if compression is not None:
            kwargs["compression"] = compression
        top_down = take("top_down")
        if top_down is not None:
            kwargs["top_down"] = top_down
        header = take("header")
        if header is not None:
            kwargs["header"] = header
    elif fmt == "webp":
        for name in ("quality", "lossless", "method"):
            value = take(name)
            if value is not None:
                kwargs[name] = value
        animated = take("animated")
        frames = take("frames")
        preserve_duration = take("preserve_duration")
        if animated is not None:
            kwargs["_manifest_animated"] = animated
        if frames is not None:
            kwargs["_manifest_frames"] = frames
        if preserve_duration is not None:
            kwargs["_manifest_preserve_duration"] = preserve_duration
        for name in ("loop", "background", "minimize_size", "kmin", "kmax", "allow_mixed"):
            value = take(name)
            if value is not None:
                kwargs[name] = value
        hint = take("hint")
        if hint is not None:
            kwargs["hint"] = hint
        for name in ("exif", "xmp", "icc"):
            value = take(name)
            if value is True:
                raise RuntimeError(f"{name}=true requires explicit metadata bytes")
        metadata_options = {
            "exif_hex": "exif",
            "xmp_hex": "xmp",
            "icc_hex": "icc_profile",
        }
        for manifest_name, pillow_name in metadata_options.items():
            value = take(manifest_name)
            if value is not None:
                kwargs[pillow_name] = bytes.fromhex(value)
    elif fmt == "tiff":
        animated = take("animated")
        frames = take("frames")
        if animated is not None:
            kwargs["_manifest_animated"] = animated
        if frames is not None:
            kwargs["_manifest_frames"] = frames
        compression = take("compression")
        if compression is not None:
            kwargs["compression"] = {
                "none": "raw",
                "lzw": "tiff_lzw",
                "deflate": "tiff_adobe_deflate",
                "packbits": "packbits",
            }.get(compression, compression)
        byte_order = take("byte_order")
        if byte_order is not None:
            kwargs["byte_order"] = byte_order
        organization = take("organization")
        if organization is not None:
            kwargs["organization"] = organization
        pages = take("pages")
        if pages is not None:
            kwargs["pages"] = pages
        predictor = take("predictor")
        if predictor == "horizontal" or isinstance(predictor, int):
            from PIL.TiffImagePlugin import ImageFileDirectory_v2

            tiffinfo = ImageFileDirectory_v2()
            tiffinfo[317] = 2 if predictor == "horizontal" else predictor
            kwargs["tiffinfo"] = tiffinfo
        elif predictor not in (None, "none"):
            raise RuntimeError(f"unknown TIFF predictor value {predictor!r}")
    elif fmt == "ico":
        sizes = take("sizes")
        if sizes is not None:
            kwargs["sizes"] = [tuple(size) for size in sizes]
        entry_type = take("entry_type")
        if entry_type is not None:
            kwargs["bitmap_format"] = entry_type
        hotspot = take("hotspot")
        if hotspot is not None:
            kwargs["hotspot"] = tuple(hotspot) if isinstance(hotspot, list) else hotspot
    elif fmt == "avif":
        for name in (
            "quality",
            "subsampling",
            "speed",
            "max_threads",
            "codec",
            "range",
            "tile_rows",
            "tile_cols",
            "alpha_premultiplied",
            "autotiling",
        ):
            value = take(name)
            if value is not None:
                kwargs[name] = value
        advanced = take("advanced")
        if advanced is not None:
            kwargs["advanced"] = advanced
        animated = take("animated")
        frames = take("frames")
        preserve_duration = take("preserve_duration")
        sequence_time = take("sequence_time")
        if sequence_time is not None and (
            not isinstance(sequence_time, int) or sequence_time <= 0
        ):
            raise RuntimeError("AVIF sequence_time must be a positive Unix timestamp")
        if animated is not None:
            kwargs["_manifest_animated"] = animated
        if frames is not None:
            kwargs["_manifest_frames"] = frames
        if preserve_duration is not None:
            kwargs["_manifest_preserve_duration"] = preserve_duration
        metadata_options = {
            "icc_hex": "icc_profile",
            "exif_hex": "exif",
            "xmp_hex": "xmp",
        }
        for manifest_name, pillow_name in metadata_options.items():
            value = take(manifest_name)
            if value is not None:
                kwargs[pillow_name] = bytes.fromhex(value)
        exif_orientation = take("exif_orientation")
        if exif_orientation is not None:
            from PIL import Image

            exif = Image.Exif()
            exif[274] = exif_orientation
            kwargs["exif"] = exif

    if remaining:
        names = ", ".join(sorted(remaining))
        raise RuntimeError(f"{fmt} has no exact Pillow mapping for: {names}")
    return kwargs


def validate_source_params(image, params, fmt_name=None):
    """Prove manifest parameters that are represented by the source image."""
    expected_size = params.get("size")
    if expected_size is not None and list(image.size) != list(expected_size):
        raise RuntimeError(f"source size is {image.size}, expected {expected_size}")

    requested_mode = params.get("color_type", params.get("color"))
    mode_aliases = {
        "1bit": "1",
        "gray": "L",
        "L": "L",
        "gray_alpha": "LA",
        "LA": "LA",
        "rgb": "RGB",
        "RGB": "RGB",
        "rgba": "RGBA",
        "RGBA": "RGBA",
        "P": "P",
        "cmyk": "CMYK",
    }
    if requested_mode is not None and image.mode != mode_aliases.get(requested_mode, requested_mode):
        raise RuntimeError(f"source mode is {image.mode}, expected {requested_mode}")

    grayscale = params.get("grayscale")
    if grayscale is not None and (image.mode == "L") != grayscale:
        raise RuntimeError(f"source mode {image.mode} does not satisfy grayscale={grayscale}")
    alpha = params.get("alpha")
    if alpha is not None and ("A" in image.getbands()) != alpha:
        raise RuntimeError(f"source mode {image.mode} does not satisfy alpha={alpha}")

    bit_depth = params.get("bit_depth")
    if bit_depth is not None and fmt_name != "bmp":
        source_depth = {"1": 1, "L": 8, "P": 8, "I;16": 16, "I": 32, "F": 32, "RGB": 24, "RGBA": 32}.get(image.mode)
        if source_depth != bit_depth:
            raise RuntimeError(f"source mode {image.mode} has depth {source_depth}, expected {bit_depth}")


def prepare_multiframe_call(image, kwargs):
    """Resolve manifest-only animation markers into Pillow save kwargs."""
    animated = kwargs.pop("_manifest_animated", None)
    frame_count = kwargs.pop("_manifest_frames", None)
    second_frame_mode = kwargs.pop("_manifest_second_frame_mode", None)
    preserve_duration = kwargs.pop("_manifest_preserve_duration", False)
    preserve_disposal = kwargs.pop("_manifest_preserve_disposal", False)
    if animated is None:
        return image, kwargs
    if not animated:
        return image, kwargs

    from PIL import ImageSequence

    frames = []
    disposals = []
    for source_frame in ImageSequence.Iterator(image):
        disposals.append(int(getattr(source_frame, "disposal_method", 0)))
        frames.append(source_frame.copy())
    requested = frame_count or len(frames)
    if len(frames) < requested:
        raise RuntimeError(f"source has {len(frames)} frame(s), requested {requested}")
    if second_frame_mode is not None:
        if len(frames) < 2:
            raise RuntimeError("second_frame_mode requires an animated source")
        frames[1] = frames[1].convert(second_frame_mode)
    kwargs["save_all"] = True
    kwargs["append_images"] = frames[1:requested]
    if preserve_duration:
        kwargs["duration"] = [
            frame.info.get("duration", 0) for frame in frames[:requested]
        ]
    if preserve_disposal:
        requested_disposals = disposals[:requested]
        # Pillow's multi-frame writer accepts either a scalar or a per-frame
        # list. If it coalesces identical frames down to one image, however,
        # the single-frame fallback calls int() on this value. Preserve a
        # shared disposal as a scalar so that both writer paths remain valid.
        kwargs["disposal"] = (
            requested_disposals[0]
            if len(set(requested_disposals)) == 1
            else requested_disposals
        )
    return frames[0], kwargs


def canonicalize_avif_sequence_times(data, unix_time):
    """Pin libavif's otherwise wall-clock-dependent sequence timestamps."""
    encoded = bytearray(data)
    bmff_time = unix_time + 2_082_844_800
    timestamp = bmff_time.to_bytes(8, "big")
    replaced = []
    for box_type in (b"mvhd", b"tkhd", b"mdhd"):
        search_from = 0
        while True:
            type_offset = encoded.find(box_type, search_from)
            if type_offset < 0:
                break
            box_offset = type_offset - 4
            if box_offset >= 0:
                box_size = int.from_bytes(encoded[box_offset:type_offset], "big")
                version_offset = type_offset + 4
                if box_size >= 28 and encoded[version_offset] == 1:
                    encoded[version_offset + 4 : version_offset + 12] = timestamp
                    encoded[version_offset + 12 : version_offset + 20] = timestamp
                    replaced.append(box_type)
            search_from = type_offset + 4
    if sorted(replaced) != [b"mdhd", b"mvhd", b"tkhd"]:
        names = ", ".join(value.decode("ascii") for value in replaced)
        raise RuntimeError(f"unexpected AVIF sequence timestamp boxes: {names}")
    return bytes(encoded)


def parse_png_structure(data):
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise RuntimeError("invalid PNG signature")
    chunks = []
    offset = 8
    while offset + 12 <= len(data):
        length = struct.unpack_from(">I", data, offset)[0]
        end = offset + 12 + length
        if end > len(data):
            raise RuntimeError("truncated PNG chunk")
        kind = data[offset + 4 : offset + 8]
        payload = data[offset + 8 : offset + 8 + length]
        chunks.append((kind, payload))
        offset = end
        if kind == b"IEND":
            break
    if not chunks or chunks[0][0] != b"IHDR" or len(chunks[0][1]) != 13:
        raise RuntimeError("missing PNG IHDR")
    width, height, depth, color_type, _, _, interlace = struct.unpack(
        ">IIBBBBB", chunks[0][1]
    )
    return {
        "width": width,
        "height": height,
        "depth": depth,
        "color_type": color_type,
        "interlace": interlace,
        "chunks": chunks,
    }


def validate_png_claim(case_id, data):
    png = parse_png_structure(data)
    if case_id.startswith("depth_"):
        expected = int(case_id.removeprefix("depth_"))
        if png["depth"] != expected:
            raise RuntimeError(f"IHDR depth is {png['depth']}, expected {expected}")
    color_types = {
        "color_gray": 0,
        "color_gray_alpha": 4,
        "color_rgb": 2,
        "color_rgba": 6,
        "color_indexed": 3,
        "color_indexed_alpha": 3,
    }
    if case_id in color_types and png["color_type"] != color_types[case_id]:
        raise RuntimeError(
            f"IHDR color type is {png['color_type']}, expected {color_types[case_id]}"
        )
    kinds = [kind for kind, _ in png["chunks"]]
    if case_id == "color_indexed_alpha" and b"tRNS" not in kinds:
        raise RuntimeError("indexed-alpha fixture has no tRNS chunk")
    if case_id == "interlace_adam7" and png["interlace"] != 1:
        raise RuntimeError("IHDR is not Adam7 interlaced")
    if case_id == "no_interlace" and png["interlace"] != 0:
        raise RuntimeError("IHDR is interlaced")

    chunk_claims = {
        "chunk_gama": b"gAMA",
        "chunk_srgb": b"sRGB",
        "chunk_iccp": b"iCCP",
        "chunk_text": b"tEXt",
        "chunk_time": b"tIME",
        "chunk_background": b"bKGD",
        "chunk_phys": b"pHYs",
        "apng_animated": b"acTL",
    }
    expected_chunk = chunk_claims.get(case_id)
    if expected_chunk is not None and expected_chunk not in kinds:
        raise RuntimeError(f"fixture has no {expected_chunk.decode()} chunk")

    filter_claims = {
        "filter_none": {0},
        "filter_sub": {1},
        "filter_up": {2},
        "filter_average": {3},
        "filter_paeth": {4},
        "filter_mixed": {0, 1, 2, 3, 4},
    }
    expected_filters = filter_claims.get(case_id)
    if expected_filters is not None:
        if png["interlace"] != 0 or png["color_type"] != 2 or png["depth"] != 8:
            raise RuntimeError("filter fixture must be non-interlaced RGB8")
        compressed = b"".join(payload for kind, payload in png["chunks"] if kind == b"IDAT")
        scanlines = zlib.decompress(compressed)
        stride = png["width"] * 3 + 1
        if len(scanlines) != stride * png["height"]:
            raise RuntimeError("unexpected RGB8 scanline length")
        actual_filters = {scanlines[row * stride] for row in range(png["height"])}
        if actual_filters != expected_filters:
            raise RuntimeError(
                f"scanline filters are {sorted(actual_filters)}, expected {sorted(expected_filters)}"
            )


def validate_bmp_claim(case_id, data):
    if data[:2] != b"BM" or len(data) < 26:
        raise RuntimeError("invalid BMP file header")
    dib_size = struct.unpack_from("<I", data, 14)[0]
    if dib_size == 12:
        width, height, _, depth = struct.unpack_from("<HHHH", data, 18)
        compression = 0
    elif dib_size >= 40 and len(data) >= 54:
        width, height, _, depth, compression = struct.unpack_from("<iiHHI", data, 18)
    else:
        raise RuntimeError(f"unsupported DIB header size {dib_size}")
    del width
    if case_id.startswith("depth_"):
        expected = int(case_id.removeprefix("depth_"))
        if depth != expected:
            raise RuntimeError(f"BMP depth is {depth}, expected {expected}")
    compression_claims = {
        "compression_none": 0,
        "compression_rle8": 1,
        "compression_rle4": 2,
        "compression_bitfields": 3,
    }
    if case_id in compression_claims and compression != compression_claims[case_id]:
        raise RuntimeError(
            f"BMP compression is {compression}, expected {compression_claims[case_id]}"
        )
    if case_id == "top_down" and height >= 0:
        raise RuntimeError("BMP height is not negative")
    if case_id == "bottom_up" and height <= 0:
        raise RuntimeError("BMP height is not positive")
    header_claims = {"os2_v1": 12, "v4_header": 108, "v5_header": 124}
    if case_id in header_claims and dib_size != header_claims[case_id]:
        raise RuntimeError(f"DIB header size is {dib_size}, expected {header_claims[case_id]}")


def validate_tiff_claim(case_id, asset_name, data):
    from PIL import Image

    if len(data) < 8 or data[:2] not in (b"II", b"MM"):
        raise RuntimeError("invalid classic TIFF header")
    if case_id == "byte_order_le" and data[:2] != b"II":
        raise RuntimeError("TIFF is not little-endian")
    if case_id == "byte_order_be" and data[:2] != b"MM":
        raise RuntimeError("TIFF is not big-endian")
    with Image.open(io.BytesIO(data)) as image:
        compression_claims = {
            "compression_none": 1,
            "compression_lzw": 5,
            "compression_deflate": {8, 32946},
            "compression_packbits": 32773,
        }
        expected = compression_claims.get(case_id)
        actual = image.tag_v2.get(259, 1)
        if expected is not None and (
            actual not in expected if isinstance(expected, set) else actual != expected
        ):
            raise RuntimeError(f"TIFF compression is {actual}, expected {expected}")
        depth_claim = case_id.removeprefix("depth_")
        if case_id.startswith("depth_") and depth_claim.isdigit():
            expected_depth = int(depth_claim)
            actual_depth = image.tag_v2.get(258, (1,))
            actual_depth = actual_depth[0] if isinstance(actual_depth, tuple) else actual_depth
            if actual_depth != expected_depth:
                raise RuntimeError(
                    f"TIFF depth is {actual_depth}, expected {expected_depth}"
                )
        if case_id == "photometric_miniswhite" and image.tag_v2.get(262) != 0:
            raise RuntimeError("TIFF is not white-is-zero")
        if case_id == "palette_low_depth":
            depth = image.tag_v2.get(258, (8,))
            depth = depth[0] if isinstance(depth, tuple) else depth
            if image.tag_v2.get(262) != 3 or depth not in (2, 4):
                raise RuntimeError("TIFF is not a packed low-depth palette image")
        if case_id == "ycbcr" and (
            image.tag_v2.get(262) != 6 or image.tag_v2.get(530) != (1, 1)
        ):
            raise RuntimeError("TIFF is not un-sub-sampled baseline YCbCr")
        if case_id in ("tiled", "tiled_predictor") and not all(
            image.tag_v2.get(tag) is not None for tag in (322, 323, 324, 325)
        ):
            raise RuntimeError("TIFF has no tile organization tags")
        if case_id == "tiled_missing_byte_counts" and (
            not all(image.tag_v2.get(tag) is not None for tag in (322, 323, 324))
            or image.tag_v2.get(325) is not None
        ):
            raise RuntimeError("TIFF does not have an empty TileByteCounts entry")
        if case_id == "tiled_predictor" and (
            image.tag_v2.get(259) not in (8, 32946) or image.tag_v2.get(317) != 2
        ):
            raise RuntimeError("TIFF is not Deflate-tiled with horizontal prediction")
        if case_id == "tiled_lzw_predictor" and (
            image.tag_v2.get(259) != 5 or image.tag_v2.get(317) != 2
        ):
            raise RuntimeError("TIFF is not LZW-tiled with horizontal prediction")
        if case_id == "stripped" and len(image.tag_v2.get(273, ())) < 2:
            raise RuntimeError("TIFF does not contain multiple strips")
        if "predictor" in asset_name and image.tag_v2.get(317) != 2:
            raise RuntimeError("TIFF does not declare horizontal predictor 2")
        if case_id == "single_page" and image.n_frames != 1:
            raise RuntimeError(f"TIFF has {image.n_frames} pages, expected one")
        if case_id == "multi_page" and image.n_frames < 2:
            raise RuntimeError("TIFF does not contain multiple pages")


def validate_ico_claim(case_id, asset_name, data):
    if len(data) < 6:
        raise RuntimeError("truncated ICO header")
    reserved, icon_type, count = struct.unpack_from("<HHH", data)
    if reserved != 0 or icon_type not in (1, 2) or count == 0:
        raise RuntimeError("invalid ICO header")
    if len(data) < 6 + count * 16:
        raise RuntimeError("truncated ICO directory")
    entries = []
    for index in range(count):
        entry = data[6 + index * 16 : 22 + index * 16]
        size, offset = struct.unpack_from("<II", entry, 8)
        payload = data[offset : offset + size]
        entries.append((entry, payload))
    if case_id == "single_icon" and count != 1:
        raise RuntimeError(f"ICO has {count} entries, expected one")
    if case_id == "multi_res" and count < 2:
        raise RuntimeError("ICO does not contain multiple resolutions")
    if case_id == "cursor" and icon_type != 2:
        raise RuntimeError("fixture is not a CUR container")
    if case_id == "png_entry" and not entries[0][1].startswith(b"\x89PNG"):
        raise RuntimeError("ICO entry is not PNG encoded")
    if case_id == "bmp_entry" and entries[0][1].startswith(b"\x89PNG"):
        raise RuntimeError("ICO entry is not BMP encoded")
    if case_id == "bmp_depths":
        expected_depth = int(asset_name.removeprefix("bmp_").removesuffix("bit.ico"))
        payload = entries[0][1]
        if len(payload) < 16 or struct.unpack_from("<H", payload, 14)[0] != expected_depth:
            raise RuntimeError(f"ICO BMP entry is not {expected_depth}-bit")


def preflight_decode_cases(manifest, target_format=None):
    """Prove active fixture structure and Pillow success/error behavior."""
    from PIL import Image

    failures = []
    for fmt_name, fmt_data in manifest.get("formats", {}).items():
        if target_format and fmt_name != target_format:
            continue
        for case in fmt_data.get("edge_cases", []):
            if fmt_data.get("status") == "planned" or case.get("status") == "planned":
                continue
            for asset_name in case.get("test_assets", []):
                case_name = f"{fmt_name}/{case['id']}/{asset_name}"
                path = ASSETS_DIR / fmt_name / asset_name
                if not path.exists():
                    failures.append(f"{case_name}: asset does not exist")
                    continue
                try:
                    with pillow_open_asset(path) as image:
                        image.load()
                    pillow_error = None
                except Exception as error:
                    pillow_error = error
                if case.get("expect_error"):
                    if pillow_error is None:
                        failures.append(f"{case_name}: Pillow accepts declared error input")
                    continue
                if pillow_error is not None:
                    failures.append(f"{case_name}: Pillow rejects active input: {pillow_error}")
                    continue
                if case.get("expect_sequence_error"):
                    try:
                        with pillow_open_asset(path) as image:
                            for index in range(image.n_frames):
                                image.seek(index)
                                image.load()
                    except Exception:
                        pass
                    else:
                        failures.append(
                            f"{case_name}: Pillow accepts declared sequence error input"
                        )
                    continue
                try:
                    data = path.read_bytes()
                    if fmt_name == "png":
                        validate_png_claim(case["id"], data)
                    elif fmt_name == "bmp":
                        validate_bmp_claim(case["id"], data)
                    elif fmt_name == "tiff":
                        validate_tiff_claim(case["id"], asset_name, data)
                    elif fmt_name == "ico":
                        validate_ico_claim(case["id"], asset_name, data)
                except Exception as error:
                    failures.append(f"{case_name}: {error}")
    if failures:
        detail = "\n  - ".join(failures)
        raise RuntimeError(f"active decode fixtures do not prove their manifest claims:\n  - {detail}")


def raw_ref_path(name):
    return Path("tests") / "fixtures" / "outputs" / "raws" / name


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def execution_contract():
    return {
        "target": "aarch64-apple-darwin",
        "features": ALL_CODEC_FEATURES,
        "suite": "native_all_features",
    }


def decode_operation_expectations(row):
    sequence = "ok"
    if (
        row.get("expect_error")
        or row.get("expect_sequence_error")
        or row.get("rust_expect_sequence_error")
    ):
        sequence = "error"
    return {
        "detect": "ok" if row.get("oracle_detects_format") else "error",
        "inspect": row.get("inspect_status"),
        "verify": row.get("verify_status"),
        "decode": row.get("oracle_status"),
        "decode_sequence": sequence,
    }


def encode_operation_expectations(row):
    expected = "error" if row.get("expect_error") or row.get("rust_expect_error") else "ok"
    params = row.get("params", {})
    direct_still = bool(params.get("truncate_pixels")) or "source_dimensions" in params
    public_mode_contract = bool(params.get("rust_unsupported_modes"))
    single_frame_success = (
        row.get("source_frame_count") == 1
        and expected == "ok"
        and not any(name.startswith("sequence_") for name in params)
    )
    return {
        "encode": (
            "error"
            if direct_still or public_mode_contract
            else "ok"
            if single_frame_success
            else "not_applicable"
        ),
        "encode_sequence": "not_applicable" if direct_still or public_mode_contract else expected,
    }


def rust_error_contract(
    fmt_name, kind, pillow_type=None, pillow_message=None, origin="pillow_fixture"
):
    """Separate exact Pillow evidence from the stable public Rust contract."""
    if kind not in {
        "unknown_format",
        "feature_disabled",
        "malformed",
        "unsupported",
        "dimensions",
        "parameter",
    }:
        raise ValueError(f"unknown Rust error kind {kind!r}")
    return {
        "pillow_type": pillow_type,
        "pillow_message": pillow_message,
        "rust_kind": kind,
        "rust_format": None if kind == "unknown_format" else fmt_name.upper(),
        "rust_message": (
            "none" if kind in {"unknown_format", "feature_disabled"} else "non_empty"
        ),
        "origin": origin,
    }


def decode_error_contracts(row, fmt_name):
    """Build one independently classified error contract per failed operation."""
    operations = decode_operation_expectations(row)
    contracts = {}
    if operations["detect"] == "error":
        contracts["detect"] = rust_error_contract(
            fmt_name,
            "unknown_format",
            origin=(
                "specification_reference"
                if fmt_name == "avif"
                else "pillow_fixture"
            ),
        )
    if operations["inspect"] == "error":
        contracts["inspect"] = rust_error_contract(
            fmt_name,
            row["inspect_error_kind"],
            row["inspect_error_type"],
            row["inspect_error_message"],
        )
    if operations["verify"] == "error":
        contracts["verify"] = rust_error_contract(
            fmt_name,
            row["verify_error_kind"],
            row["verify_error_type"],
            row["verify_error_message"],
        )
    if operations["decode"] == "error":
        contracts["decode"] = rust_error_contract(
            fmt_name,
            row["oracle_error_kind"],
            row["oracle_error_type"],
            row["oracle_error_message"],
        )
    if operations["decode_sequence"] == "error":
        if row.get("rust_expect_sequence_error"):
            contracts["decode_sequence"] = rust_error_contract(
                fmt_name,
                row["rust_sequence_error_kind"],
                origin="defensive_model",
            )
        elif row.get("expect_sequence_error"):
            contracts["decode_sequence"] = rust_error_contract(
                fmt_name,
                row["sequence_error_kind"],
                row["sequence_error_type"],
                row["sequence_error_message"],
            )
        else:
            contracts["decode_sequence"] = rust_error_contract(
                fmt_name,
                row["oracle_error_kind"],
                row["oracle_error_type"],
                row["oracle_error_message"],
            )
    return contracts


def encode_error_contracts(row, fmt_name):
    """Build the selected still/sequence encoder error contracts."""
    contracts = {}
    for operation, status in encode_operation_expectations(row).items():
        if status != "error":
            continue
        if row.get("rust_expect_error"):
            contracts[operation] = rust_error_contract(
                fmt_name,
                row["rust_error_kind"],
                origin="defensive_model",
            )
        else:
            contracts[operation] = rust_error_contract(
                fmt_name,
                row["oracle_error_kind"],
                row["oracle_error_type"],
                row["oracle_error_message"],
            )
    return contracts


def write_pixel_ref(row, image, ref_name):
    """Write raw PIL pixels and update one matrix/output row."""
    image.load()
    raw = image.tobytes()
    OUTPUT_RAWS.mkdir(parents=True, exist_ok=True)
    (OUTPUT_RAWS / ref_name).write_bytes(raw)
    row["ref_sha256"] = sha256(raw)
    row["ref_path"] = raw_ref_path(ref_name).as_posix()
    row["ref_bytes"] = len(raw)
    row["ref_mode"] = mode_name(image)
    row["ref_size"] = list(image.size)
    try:
        row["ref_frame_count"] = int(getattr(image, "n_frames", 1))
    except Exception:
        row["ref_frame_count"] = None
    fallback_animated = bool(
        row["ref_frame_count"] is not None and row["ref_frame_count"] > 1
    )
    row["ref_is_animated"] = bool(
        getattr(image, "is_animated", fallback_animated)
    )
    return raw


def first_png_transparency(data):
    """Return the first indexed PNG tRNS payload, matching Pillow leniency."""
    position = 8
    while position + 12 <= len(data):
        length = int.from_bytes(data[position : position + 4], "big")
        kind = data[position + 4 : position + 8]
        payload_start = position + 8
        payload_end = payload_start + length
        if payload_end + 4 > len(data):
            return b""
        if kind == b"tRNS":
            return data[payload_start:payload_end]
        if kind in {b"IDAT", b"IEND"}:
            return b""
        position = payload_end + 4
    return b""


def pillow_palette(image, fmt_name, image_path, *, decoded):
    """Return Pillow-observable palette state and crate transfer bytes."""
    if image.mode != "P":
        return "absent", b"", b""
    rgb = bytes(image.getpalette("RGB") or [])
    rgb = rgb[: len(rgb) // 3 * 3]
    if not rgb:
        return "implicit", b"", b""

    entries = len(rgb) // 3
    if decoded and fmt_name == "gif":
        pixels = image.tobytes()
        required_entries = max(pixels, default=-1) + 1
        if required_entries > entries:
            rgb += bytes((required_entries - entries) * 3)
            entries = required_entries

    alpha = b""
    if fmt_name == "gif":
        transparent = image.info.get("transparency")
        if isinstance(transparent, int) and 0 <= transparent < entries:
            values = bytearray([255] * entries)
            values[transparent] = 0
            alpha = bytes(values)
    elif fmt_name == "png":
        alpha = first_png_transparency(image_path.read_bytes())[:entries]
    return "table", rgb, alpha


def write_palette_bytes(name, data):
    path = raw_ref_path(name)
    (ROOT / path).write_bytes(data)
    return path.as_posix()


def palette_ref(state, origin, prefix, rgb, alpha):
    reference = {"state": state, "origin": origin}
    if state == "table":
        reference["rgb_path"] = write_palette_bytes(f"{prefix}.rgb.bin", rgb)
        reference["rgb_bytes"] = len(rgb)
        reference["rgb_sha256"] = sha256(rgb)
        if alpha:
            reference["alpha_path"] = write_palette_bytes(
                f"{prefix}.alpha.bin", alpha
            )
            reference["alpha_bytes"] = len(alpha)
            reference["alpha_sha256"] = sha256(alpha)
    return reference


def write_palette_refs(row, image, fmt_name, image_path, ref_name):
    """Record independent inspect and decoded palette representations."""
    stem = ref_name.removesuffix(".bin")
    inspect_state, inspect_rgb, inspect_alpha = pillow_palette(
        image, fmt_name, image_path, decoded=False
    )
    decoded_state, decoded_rgb, decoded_alpha = pillow_palette(
        image, fmt_name, image_path, decoded=True
    )
    row["inspect_palette"] = palette_ref(
        inspect_state,
        "pillow_fixture",
        f"{stem}.inspect-palette",
        inspect_rgb,
        inspect_alpha,
    )
    row["decoded_palette"] = palette_ref(
        decoded_state,
        "pillow_fixture",
        f"{stem}.decoded-palette",
        decoded_rgb,
        decoded_alpha,
    )


def webp_frame_dimensions(payload):
    """Return the effective nested VP8/VP8L dimensions used for compositing."""
    offset = 16
    while offset + 8 <= len(payload):
        kind = payload[offset : offset + 4]
        chunk_size = int.from_bytes(payload[offset + 4 : offset + 8], "little")
        chunk_start = offset + 8
        chunk_end = chunk_start + chunk_size
        if chunk_end > len(payload):
            break
        bitstream = payload[chunk_start:chunk_end]
        if (
            kind == b"VP8 "
            and len(bitstream) >= 10
            and bitstream[3:6] == b"\x9d\x01\x2a"
        ):
            width = int.from_bytes(bitstream[6:8], "little") & 0x3FFF
            height = int.from_bytes(bitstream[8:10], "little") & 0x3FFF
            return width, height
        if kind == b"VP8L" and len(bitstream) >= 5 and bitstream[0] == 0x2F:
            dimensions = int.from_bytes(bitstream[1:5], "little")
            width = (dimensions & 0x3FFF) + 1
            height = ((dimensions >> 14) & 0x3FFF) + 1
            return width, height
        offset = chunk_end + (chunk_size & 1)
    raise ValueError("animated WebP frame lacks a readable VP8/VP8L header")


def apng_sequence_source(data):
    """Read exact APNG controls while retaining Pillow's seekable default image."""
    png = parse_png_structure(data)
    chunks = png["chunks"]
    animation = None
    saw_idat = False
    sources = []
    controlled_frames = 0

    for kind, payload in chunks:
        if kind == b"IDAT":
            if animation is not None and not sources:
                sources.append(
                    {
                        "source_rect": [0, 0, png["width"], png["height"]],
                        "duration_num": 0,
                        "duration_den": 1,
                        "duration_origin": "specification_reference",
                        "disposal": "unspecified",
                        "blend": "unspecified",
                        "interlaced": bool(png["interlace"]),
                        "is_default_image": True,
                        "pixel_layout": "rendered_canvas",
                        "source_origin": "specification_reference",
                    }
                )
            saw_idat = True
        elif kind == b"acTL" and not saw_idat:
            if animation is not None:
                return None
            if len(payload) < 8:
                raise ValueError("APNG has a short animation control")
            frame_count, loop_count = struct.unpack(">II", payload[:8])
            if frame_count == 0 or frame_count > 0x8000_0000:
                return None
            animation = (frame_count, loop_count)
        elif kind == b"fcTL" and animation is not None:
            if len(payload) < 26:
                raise ValueError("APNG has a short frame control")
            (
                _sequence,
                width,
                height,
                left,
                top,
                delay_num,
                delay_den,
                disposal,
                blend,
            ) = struct.unpack(">IIIIIHHBB", payload[:26])
            controlled_frames += 1
            sources.append(
                {
                    "source_rect": [left, top, width, height],
                    "duration_num": delay_num,
                    "duration_den": delay_den or 100,
                    "duration_origin": "specification_reference",
                    "disposal": {
                        0: "keep",
                        1: "background",
                        2: "previous",
                    }.get(disposal, f"reserved:{disposal}"),
                    "blend": {
                        0: "source",
                        1: "over",
                    }.get(blend, f"reserved:{blend}"),
                    "interlaced": bool(png["interlace"]),
                    "is_default_image": not saw_idat and controlled_frames == 1,
                    "pixel_layout": "rendered_canvas",
                    "source_origin": "specification_reference",
                }
            )

    if animation is None:
        return None
    declared_frames, loop_count = animation
    if controlled_frames != declared_frames:
        raise ValueError(
            "APNG declared frame count differs from its frame controls"
        )
    return loop_count, sources


def webp_sequence_source(data):
    """Read exact ANIM/ANMF presentation fields from the standardized container."""
    frames = []
    background = None
    loop_count = None
    offset = 12
    while offset + 8 <= len(data):
        chunk_type = data[offset : offset + 4]
        chunk_size = int.from_bytes(data[offset + 4 : offset + 8], "little")
        payload_start = offset + 8
        payload_end = payload_start + chunk_size
        if payload_end > len(data):
            break
        if chunk_type == b"ANIM" and chunk_size >= 6:
            payload = data[payload_start:payload_end]
            background = {
                "rgba": [payload[2], payload[1], payload[0], payload[3]],
                "origin": "specification_reference",
            }
            loop_count = int.from_bytes(payload[4:6], "little")
        elif chunk_type == b"ANMF" and chunk_size >= 16:
            payload = data[payload_start:payload_end]
            left = int.from_bytes(payload[0:3], "little") * 2
            top = int.from_bytes(payload[3:6], "little") * 2
            width, height = webp_frame_dimensions(payload)
            duration_ms = int.from_bytes(payload[12:15], "little")
            flags = payload[15]
            frames.append(
                {
                    "source_rect": [left, top, width, height],
                    "duration_num": duration_ms,
                    "duration_den": 1000,
                    "duration_origin": "pillow_fixture",
                    "disposal": "background" if flags & 1 else "keep",
                    "blend": "source" if flags & 2 else "over",
                    "interlaced": False,
                    "is_default_image": False,
                    "pixel_layout": "rendered_canvas",
                    "source_origin": "specification_reference",
                }
            )
        offset = payload_end + (chunk_size & 1)
    if background is None or loop_count is None or not frames:
        raise ValueError("animated WebP lacks complete ANIM/ANMF source metadata")
    return background, loop_count, frames


def bmff_boxes(data, start=0, end=None, path=()):
    """Yield bounded ISO BMFF payloads recursively for timing evidence."""
    end = len(data) if end is None else end
    containers = {
        b"moov",
        b"trak",
        b"mdia",
        b"minf",
        b"stbl",
        b"edts",
        b"dinf",
        b"meta",
        b"iprp",
        b"ipco",
        b"iinf",
        b"iref",
    }
    offset = start
    while offset + 8 <= end:
        size = int.from_bytes(data[offset : offset + 4], "big")
        kind = data[offset + 4 : offset + 8]
        header = 8
        if size == 1:
            if offset + 16 > end:
                return
            size = int.from_bytes(data[offset + 8 : offset + 16], "big")
            header = 16
        elif size == 0:
            size = end - offset
        if size < header or offset + size > end:
            return
        payload_start = offset + header
        payload_end = offset + size
        payload = data[payload_start:payload_end]
        current_path = path + (kind,)
        yield current_path, payload
        if kind in containers:
            prefix = 4 if kind == b"meta" else 0
            yield from bmff_boxes(
                data, payload_start + prefix, payload_end, current_path
            )
        offset += size


def avif_frame_durations(data, frame_count):
    """Read exact track timescale/sample deltas without float conversion."""
    timescales = []
    sample_tables = []
    for path, payload in bmff_boxes(data):
        if path[-1] == b"mdhd" and len(payload) >= 24:
            version = payload[0]
            timescale_offset = 20 if version == 1 else 12
            if timescale_offset + 4 <= len(payload):
                timescales.append(
                    int.from_bytes(
                        payload[timescale_offset : timescale_offset + 4], "big"
                    )
                )
        elif path[-1] == b"stts" and len(payload) >= 8:
            entry_count = int.from_bytes(payload[4:8], "big")
            entries = []
            cursor = 8
            for _ in range(entry_count):
                if cursor + 8 > len(payload):
                    entries = []
                    break
                count = int.from_bytes(payload[cursor : cursor + 4], "big")
                delta = int.from_bytes(payload[cursor + 4 : cursor + 8], "big")
                entries.extend([delta] * count)
                cursor += 8
            if entries:
                sample_tables.append(entries)
    for timescale in timescales:
        if timescale == 0:
            continue
        for durations in sample_tables:
            if len(durations) == frame_count:
                return [(duration, timescale) for duration in durations]
    raise ValueError("AVIF frame timing table does not match Pillow frame count")


def gif_frame_source(image):
    """Capture Pillow plugin metadata before load consumes the frame tile."""
    if not image.tile:
        raise ValueError("GIF frame lacks a decoder tile")
    tile = image.tile[0]
    left, top, right, bottom = tile.extents
    duration_ms = int(image.info.get("duration", 0))
    if duration_ms % 10:
        raise ValueError("GIF duration is not an exact centisecond")
    disposal = int(getattr(image, "disposal_method", 0))
    disposal_name = {
        0: "unspecified",
        1: "keep",
        2: "background",
        3: "previous",
    }.get(disposal, f"reserved:{disposal}")
    return {
        "source_rect": [left, top, right - left, bottom - top],
        "duration_num": duration_ms // 10,
        "duration_den": 100,
        "duration_origin": "pillow_fixture",
        "disposal": disposal_name,
        "blend": "unspecified",
        "interlaced": bool(tile.args[1]),
        "is_default_image": False,
        "pixel_layout": "source_rectangle",
        "source_origin": "pillow_fixture",
    }


def write_sequence_ref_from_data(row, image, fmt_name, asset_name, source_data):
    """Write exact frame pixels where layouts align and all source metadata."""
    frame_count = int(getattr(image, "n_frames", 1))
    if fmt_name not in {"png", "gif", "tiff", "webp", "avif"}:
        row.pop("sequence", None)
        return

    if fmt_name == "png":
        parsed = apng_sequence_source(source_data)
        if parsed is None:
            row.pop("sequence", None)
            return
        parsed_loop_count, sources = parsed
        background = None
        if len(sources) != frame_count:
            raise ValueError("APNG source controls differ from Pillow frame count")
    elif fmt_name != "gif" and frame_count <= 1:
        row.pop("sequence", None)
        return
    elif fmt_name == "webp":
        background, parsed_loop_count, sources = webp_sequence_source(source_data)
        pillow_background = list(image.info.get("background", ()))
        pillow_loop_count = image.info.get("loop")
        if (
            background["rgba"] != pillow_background
            or parsed_loop_count != pillow_loop_count
        ):
            raise ValueError("WebP container metadata differs from Pillow")
    elif fmt_name == "avif":
        durations = avif_frame_durations(source_data, image.n_frames)
        background = None
        sources = [
            {
                "source_rect": [0, 0, image.width, image.height],
                "duration_num": duration,
                "duration_den": timescale,
                "duration_origin": "independent_implementation",
                "disposal": "unspecified",
                "blend": "unspecified",
                "interlaced": False,
                "is_default_image": False,
                "pixel_layout": "rendered_canvas",
                "source_origin": "independent_implementation",
            }
            for duration, timescale in durations
        ]
    elif fmt_name == "tiff":
        if frame_count <= 1:
            row.pop("sequence", None)
            return
        page_sizes = []
        for index in range(frame_count):
            image.seek(index)
            page_sizes.append(image.size)
        canvas_size = [
            max(size[0] for size in page_sizes),
            max(size[1] for size in page_sizes),
        ]
        background = None
        sources = [
            {
                "source_rect": [0, 0, size[0], size[1]],
                "duration_num": 0,
                "duration_den": 1,
                "duration_origin": "specification_reference",
                "disposal": "unspecified",
                "blend": "unspecified",
                "interlaced": False,
                "is_default_image": False,
                "pixel_layout": "source_rectangle",
                "source_origin": "pillow_fixture",
            }
            for size in page_sizes
        ]
    else:
        background = {
            "palette_index": int(image.info.get("background", 0)),
            "origin": "pillow_fixture",
        }
        sources = None

    frames = []
    for index in range(image.n_frames):
        image.seek(index)
        source = gif_frame_source(image) if fmt_name == "gif" else sources[index]
        image.load()
        frame = {"index": index, **source}
        if fmt_name != "gif":
            raw = image.tobytes()
            ref_name = (
                f"Decode.{fmt_name}_{asset_name.replace('.', '_')}_frame_{index}.bin"
            )
            OUTPUT_RAWS.mkdir(parents=True, exist_ok=True)
            (OUTPUT_RAWS / ref_name).write_bytes(raw)
            frame.update(
                {
                    "ref_path": raw_ref_path(ref_name).as_posix(),
                    "ref_bytes": len(raw),
                    "ref_mode": mode_name(image),
                    "ref_size": list(image.size),
                    "ref_sha256": sha256(raw),
                    "pixel_assertion": "exact",
                    "pixel_origin": "pillow_fixture",
                }
            )
        else:
            frame["pixel_assertion"] = "not_asserted_source_layout"
        frames.append(frame)
    row["sequence"] = {
        "canvas_size": (
            canvas_size if fmt_name == "tiff" else list(image.size)
        ),
        "canvas_origin": "pillow_fixture",
        "loop_count": (
            parsed_loop_count if fmt_name in {"png", "webp"} else image.info.get("loop")
        ),
        "loop_origin": (
            "specification_reference"
            if fmt_name in {"png", "tiff"}
            else "pillow_fixture"
        ),
        "background": background,
        "frames": frames,
    }


def write_sequence_ref(row, image, fmt_name, asset_name):
    source_data = (ASSETS_DIR / fmt_name / asset_name).read_bytes()
    write_sequence_ref_from_data(row, image, fmt_name, asset_name, source_data)


def write_sequence_error_ref(row, image_path):
    """Record a Pillow error that appears only while materializing later frames."""
    row.pop("sequence", None)
    try:
        with pillow_open_asset(image_path) as image:
            for index in range(image.n_frames):
                image.seek(index)
                image.load()
    except Exception as error:
        row["sequence_status"] = "error"
        row["sequence_error_type"] = f"{type(error).__module__}.{type(error).__name__}"
        row["sequence_error_message"] = stable_error_message(error)
        row["sequence_error_kind"] = decode_error_kind(
            row["oracle_detects_format"], error
        )
    else:
        row["sequence_status"] = "ok"
        row.pop("sequence_error_type", None)
        row.pop("sequence_error_message", None)
        row.pop("sequence_error_kind", None)


def clear_pixel_ref(row):
    row.pop("ref_sha256", None)
    row.pop("ref_path", None)
    row.pop("ref_bytes", None)
    row.pop("ref_mode", None)
    row.pop("ref_size", None)
    row.pop("ref_frame_count", None)
    row.pop("ref_is_animated", None)
    row.pop("inspect_palette", None)
    row.pop("decoded_palette", None)
    row.pop("sequence", None)


def ico_bit_depth(data):
    """Read storage depth from the best-resolution ICO or CUR payload."""
    if len(data) < 6:
        raise ValueError("truncated ICO header")
    count = int.from_bytes(data[4:6], "little")
    directory_end = 6 + count * 16
    if count == 0 or directory_end > len(data):
        raise ValueError("truncated ICO directory")
    entries = [data[6 + index * 16 : 22 + index * 16] for index in range(count)]
    best = max(
        entries,
        key=lambda entry: (entry[0] or 256) * (entry[1] or 256),
    )
    length = int.from_bytes(best[8:12], "little")
    offset = int.from_bytes(best[12:16], "little")
    payload = data[offset : offset + length]
    if len(payload) != length:
        raise ValueError("truncated ICO payload")
    if payload.startswith(b"\x89PNG\r\n\x1a\n"):
        if len(payload) < 25 or payload[12:16] != b"IHDR":
            raise ValueError("truncated ICO PNG header")
        return payload[24]
    if len(payload) < 16:
        raise ValueError("truncated ICO DIB header")
    return int.from_bytes(payload[14:16], "little")


def avif_bit_depth(image_path):
    """Read AV1 configuration depth with the independent ISO-BMFF inspector."""
    from inspect_avif_bitstreams import inspect as inspect_avif

    report = inspect_avif(image_path)
    color_items = report.get("items", {}).get("color", [])
    configurations = [
        item.get("av1c") for item in color_items if item.get("av1c") is not None
    ]
    if not configurations:
        configurations = [
            track.get("av1c")
            for track in report.get("tracks", [])
            if track.get("handler") == "pict" and track.get("av1c") is not None
        ]
    if not configurations:
        raise ValueError("AVIF has no color AV1CodecConfigurationBox")
    payload = bytes.fromhex(configurations[0]["hex"])
    if len(payload) < 3 or payload[0] != 0x81:
        raise ValueError("invalid AV1CodecConfigurationBox")
    high_bit_depth = bool(payload[2] & 0x40)
    twelve_bit = bool(payload[2] & 0x20)
    if twelve_bit and not high_bit_depth:
        raise ValueError("invalid AV1CodecConfigurationBox depth flags")
    return 12 if twelve_bit else 10 if high_bit_depth else 8


def inspect_bit_depth(fmt_name, image_path, image):
    """Return encoded storage depth and the independent evidence category."""
    data = image_path.read_bytes()
    if fmt_name == "png":
        if len(data) < 25 or data[12:16] != b"IHDR":
            raise ValueError("truncated PNG IHDR")
        return data[24], "specification_reference"
    if fmt_name == "jpeg":
        return int(image.bits), "pillow_fixture"
    if fmt_name == "gif":
        if image.palette is None:
            return 8, "pillow_fixture"
        palette_bytes = image.palette.getdata()[1]
        entries = max(1, len(palette_bytes) // 3)
        return max(1, (entries - 1).bit_length()), "pillow_fixture"
    if fmt_name == "bmp":
        if len(data) < 26:
            raise ValueError("truncated BMP DIB header")
        dib_size = int.from_bytes(data[14:18], "little")
        bit_depth_offset = 24 if dib_size == 12 else 28
        if bit_depth_offset + 2 > len(data):
            raise ValueError("truncated BMP bit-depth field")
        return (
            int.from_bytes(data[bit_depth_offset : bit_depth_offset + 2], "little"),
            "specification_reference",
        )
    if fmt_name == "webp":
        return 8, "specification_reference"
    if fmt_name == "tiff":
        values = image.tag_v2.get(258, (1,))
        if not isinstance(values, tuple):
            values = (values,)
        if not values:
            raise ValueError("TIFF has empty BitsPerSample")
        return int(values[0]), "pillow_fixture"
    if fmt_name == "ico":
        return ico_bit_depth(data), "specification_reference"
    if fmt_name == "avif":
        return avif_bit_depth(image_path), "independent_implementation"
    raise ValueError(f"no bit-depth oracle for {fmt_name}")


def write_inspect_ref(row, image_path, fmt_name):
    """Record Pillow's lazy Image.open outcome without materializing pixels."""
    try:
        with pillow_open_asset(image_path) as image:
            row["inspect_container_format"] = image.format
            bit_depth, origin = inspect_bit_depth(fmt_name, image_path, image)
            row["ref_bit_depth"] = bit_depth
            row["ref_bit_depth_origin"] = origin
            if image.format == "CUR":
                data = image_path.read_bytes()
                count = int.from_bytes(data[4:6], "little")
                entries = [
                    data[6 + index * 16 : 22 + index * 16]
                    for index in range(count)
                ]
                best = max(
                    entries,
                    key=lambda entry: (entry[0] or 256) * (entry[1] or 256),
                )
                row["inspect_cursor_hotspot"] = [
                    int.from_bytes(best[4:6], "little"),
                    int.from_bytes(best[6:8], "little"),
                ]
            else:
                row["inspect_cursor_hotspot"] = None
    except Exception as error:
        row["inspect_status"] = "error"
        row.pop("inspect_container_format", None)
        row.pop("inspect_cursor_hotspot", None)
        row.pop("ref_bit_depth", None)
        row.pop("ref_bit_depth_origin", None)
        row["inspect_error_type"] = f"{type(error).__module__}.{type(error).__name__}"
        row["inspect_error_message"] = stable_error_message(error)
        row["inspect_error_kind"] = decode_error_kind(
            row["oracle_detects_format"], error
        )
    else:
        row["inspect_status"] = "ok"
        row.pop("inspect_error_type", None)
        row.pop("inspect_error_message", None)
        row.pop("inspect_error_kind", None)


def write_verify_ref(row, image_path):
    """Record the pinned Pillow plugin's exact Image.verify outcome."""
    try:
        with pillow_open_asset(image_path) as image:
            image.verify()
    except Exception as error:
        row["verify_status"] = "error"
        row["verify_error_type"] = f"{type(error).__module__}.{type(error).__name__}"
        row["verify_error_message"] = stable_error_message(error)
        row["verify_error_kind"] = decode_error_kind(
            row["oracle_detects_format"], error
        )
    else:
        row["verify_status"] = "ok"
        row.pop("verify_error_type", None)
        row.pop("verify_error_message", None)
        row.pop("verify_error_kind", None)


def clear_encoded_ref(row):
    row.pop("encoded_ref_path", None)
    row.pop("encoded_ref_bytes", None)
    row.pop("encoded_ref_sha256", None)


def oracle_identity(manifest):
    oracle = manifest["reference_oracles"]["primary"]
    return {
        "implementation": oracle["implementation"],
        "version": str(oracle["version"]),
        "profile": oracle["profile"],
        "wheel_sha256": oracle["wheel_sha256"],
        "imaging_extension_sha256": oracle["imaging_extension_sha256"],
    }


def clear_generated_outputs(manifest, target_format=None):
    """Remove only derived files that this invocation will regenerate."""
    format_names = [target_format] if target_format else manifest.get("formats", {})
    for fmt_name in format_names:
        for directory, patterns in (
            (OUTPUT_RAWS, (f"Decode.{fmt_name}_*.bin", f"Encode.{fmt_name}_*.bin")),
            (OUTPUT_ENCODED, (f"Encode.{fmt_name}_*.bin",)),
        ):
            for pattern in patterns:
                for path in directory.glob(pattern):
                    path.unlink()


def json_pillow_value(value):
    if isinstance(value, bytes):
        return {"type": "bytes", "hex": value.hex()}
    if isinstance(value, tuple):
        return [json_pillow_value(item) for item in value]
    if isinstance(value, list):
        return [json_pillow_value(item) for item in value]
    if isinstance(value, dict):
        return {str(key): json_pillow_value(item) for key, item in value.items()}
    if hasattr(value, "chunks"):
        return {
            "type": type(value).__name__,
            "chunks": [
                {
                    "chunk_type": chunk_type.decode("ascii"),
                    "data_hex": payload.hex(),
                    "after_idat": bool(after_idat),
                }
                for chunk_type, payload, after_idat in value.chunks
            ],
        }
    if type(value).__name__ == "ImageFileDirectory_v2":
        return {
            "type": type(value).__name__,
            "tags": {str(key): json_pillow_value(item) for key, item in dict(value).items()},
        }
    if type(value).__name__ == "Exif":
        return {
            "type": "Exif",
            "tags": {str(key): json_pillow_value(item) for key, item in dict(value).items()},
        }
    return value


def describe_encode_call(fmt_name, row):
    try:
        kwargs = encode_params(fmt_name, dict(row.get("params", {})))
    except Exception as error:
        if not row.get("expect_error"):
            raise
        return {
            "open": f"tests/fixtures/input/images/{row['source_format']}/{row['source_asset']}",
            "method": "manifest parameter adapter",
            "format": fmt_pil(fmt_name),
            "params": json_pillow_value(dict(row.get("params", {}))),
            "error": {
                "type": f"{type(error).__module__}.{type(error).__name__}",
                "message": stable_error_message(error),
            },
            "roundtrip": None,
        }
    animated = kwargs.pop("_manifest_animated", None)
    frame_count = kwargs.pop("_manifest_frames", None)
    preserve_duration = kwargs.pop("_manifest_preserve_duration", False)
    preserve_disposal = kwargs.pop("_manifest_preserve_disposal", False)
    if animated:
        kwargs["save_all"] = True
        kwargs["append_images"] = {
            "type": "frames_from_source",
            "start": 1,
            "count": (frame_count or 1) - 1,
        }
        if preserve_duration:
            kwargs["duration"] = {
                "type": "durations_from_source",
                "count": frame_count or 1,
            }
        if preserve_disposal:
            kwargs["disposal"] = {
                "type": "disposals_from_source",
                "count": frame_count or 1,
            }
    call = {
        "open": f"tests/fixtures/input/images/{row['source_format']}/{row['source_asset']}",
        "method": "PIL.Image.Image.save",
        "format": fmt_pil(fmt_name),
        "kwargs": json_pillow_value(kwargs),
    }
    if fmt_name == "avif" and row.get("params", {}).get("sequence_time"):
        call["output_canonicalization"] = {
            "boxes": ["mvhd", "tkhd", "mdhd"],
            "fields": ["creation_time", "modification_time"],
            "unix_time": row["params"]["sequence_time"],
        }
    if row.get("params", {}).get("encoded_only"):
        call["roundtrip"] = None
    else:
        call["roundtrip"] = ["PIL.Image.open", "load", "tobytes"]
    if row.get("params", {}).get("truncate_pixels"):
        call["source_transform"] = "PIL.Image.frombytes(mode, size, tobytes()[:-1])"
    if source_dimensions := row.get("params", {}).get("source_dimensions"):
        call["source_transform"] = {
            "method": "PIL.Image.new",
            "mode": "source mode",
            "size": source_dimensions,
        }
    if row.get("params", {}).get("oversized_palette"):
        call["source_transform"] = "PIL.Image.Image.putpalette(bytes(771))"
    if row.get("params", {}).get("palette_on_nonindexed"):
        call["source_transform"] = "PIL.Image.Image.putpalette(bytes(768))"
    if row.get("params", {}).get("detach_source"):
        call["source_transform"] = (
            "PIL.Image.frombytes(source.mode, source.size, source.tobytes())"
        )
    if row.get("params", {}).get("rust_unsupported_modes"):
        call["rust_source_transform"] = (
            "construct one valid zero-filled DecodedImage for every named mode"
        )
    if row.get("params", {}).get("rust_invalid_color_mode"):
        call["rust_invalid_transform"] = (
            "construct L8 bytes with a deliberately inconsistent Rgb8 ColorType"
        )
    sequence_fields = {
        name: value
        for name, value in row.get("params", {}).items()
        if name.startswith("sequence_")
    }
    if sequence_fields:
        call["retained_sequence_transform"] = {
            "scope": "image-slash-star DecodedSequence contract",
            "fields": sequence_fields,
            "pillow_model": "Pillow saves the already materialized still image",
        }
    return call


def sync_encode_rows(manifest, matrix):
    """Make manifest encode cases authoritative and reject ambiguous IDs."""
    for fmt_name, fmt_manifest in manifest.get("formats", {}).items():
        specifications = fmt_manifest.get("encode_edge_cases", [])
        seen = set()
        for specification in specifications:
            case_id = specification["id"]
            if case_id in seen:
                raise RuntimeError(f"duplicate encode case id in manifest: {fmt_name}/{case_id}")
            seen.add(case_id)

        fmt_matrix = matrix.setdefault("formats", {}).setdefault(fmt_name, {})
        fmt_matrix.setdefault("decode", [])
        existing = {}
        for row in fmt_matrix.get("encode", []):
            current = existing.get(row["id"])
            # Prefer the row carrying a reference when collapsing old duplicates.
            if current is None or (not current.get("ref_path") and row.get("ref_path")):
                existing[row["id"]] = row

        default_source = fmt_manifest.get("encode_source", {})
        default_source_format = default_source.get("format")
        default_source_asset = default_source.get("asset")
        synchronized = []
        for specification in specifications:
            # An oracle gap is not an implementation failure: Pillow has no
            # public call capable of producing the requested variant.
            if specification.get("status") == "planned" and specification.get("oracle_gap"):
                continue
            case_id = specification["id"]
            row = dict(existing.get(case_id, {}))
            row.update(
                {
                    "id": case_id,
                    "type": "encode",
                    "format": fmt_name,
                    "category": case_id.removeprefix("enc_").split("_", 1)[0],
                    "description": specification.get("description") or "",
                    "params": specification.get("params", {}),
                }
            )
            if specification.get("expect_error"):
                row["expect_error"] = True
            else:
                row.pop("expect_error", None)
            if specification.get("rust_expect_error"):
                row["rust_expect_error"] = True
                row["rust_error_kind"] = specification["rust_error_kind"]
                row["rust_error_reason"] = specification["rust_error_reason"]
            else:
                row.pop("rust_expect_error", None)
                row.pop("rust_error_kind", None)
                row.pop("rust_error_reason", None)
            row["status"] = specification.get("status", "active")
            if row["status"] == "planned":
                row["gap"] = (
                    specification.get("oracle_gap")
                    or specification.get("gap")
                    or fmt_manifest.get("planned_gap_defaults", {}).get("encode")
                )
                if not row["gap"]:
                    raise RuntimeError(f"planned encode row has no gap reason: {fmt_name}/{case_id}")
                clear_pixel_ref(row)
                clear_encoded_ref(row)
            else:
                row.pop("gap", None)
            row["source_format"] = specification.get(
                "source_format", default_source_format or fmt_name
            )
            if specification.get("source_asset"):
                row["source_asset"] = specification["source_asset"]
            elif default_source_asset:
                row["source_asset"] = default_source_asset
            else:
                row.pop("source_asset", None)
            synchronized.append(row)
        fmt_matrix["encode"] = synchronized


def update_summary(matrix):
    """Recompute matrix counts after manifest synchronization."""
    formats = matrix.get("formats", {})
    decode_rows = [row for value in formats.values() for row in value.get("decode", [])]
    encode_rows = [row for value in formats.values() for row in value.get("encode", [])]
    assets = {
        (row.get("format"), row.get("asset"))
        for row in decode_rows
        if row.get("asset")
        and (ASSETS_DIR / str(row.get("format")) / str(row.get("asset"))).exists()
    }
    matrix["summary"] = {
        "total_rows": len(decode_rows) + len(encode_rows),
        "decode_rows": len(decode_rows),
        "encode_rows": len(encode_rows),
        "formats": len(formats),
        "assets_available": len(assets),
        "decode_active": sum(row.get("status") == "active" for row in decode_rows),
        "decode_planned": sum(row.get("status") == "planned" for row in decode_rows),
        "encode_not_wired": sum(row.get("status") == "planned" for row in encode_rows),
    }


def validate_sequence_reference(sequence, case_name, expected_frame_count):
    """Return every schema or artifact defect in one decoded-sequence reference."""
    failures = []
    canvas = sequence.get("canvas_size")
    if (
        not isinstance(canvas, list)
        or len(canvas) != 2
        or any(not isinstance(value, int) or value <= 0 for value in canvas)
    ):
        failures.append(f"{case_name}: sequence canvas evidence is invalid")
        canvas = None
    if sequence.get("canvas_origin") not in ASSERTION_ORIGINS:
        failures.append(f"{case_name}: sequence canvas origin is invalid")
    loop_count = sequence.get("loop_count")
    if loop_count is not None and (
        not isinstance(loop_count, int) or isinstance(loop_count, bool) or loop_count < 0
    ):
        failures.append(f"{case_name}: sequence loop count is invalid")
    if sequence.get("loop_origin") not in ASSERTION_ORIGINS:
        failures.append(f"{case_name}: sequence loop origin is invalid")

    background = sequence.get("background")
    if background is not None:
        if not isinstance(background, dict):
            failures.append(f"{case_name}: sequence background is not an object")
        else:
            palette_index = background.get("palette_index")
            rgba = background.get("rgba")
            palette_valid = (
                isinstance(palette_index, int)
                and not isinstance(palette_index, bool)
                and 0 <= palette_index <= 255
            )
            rgba_valid = (
                isinstance(rgba, list)
                and len(rgba) == 4
                and all(
                    isinstance(value, int)
                    and not isinstance(value, bool)
                    and 0 <= value <= 255
                    for value in rgba
                )
            )
            if palette_valid == rgba_valid:
                failures.append(
                    f"{case_name}: sequence background must contain exactly one valid representation"
                )
            if background.get("origin") not in ASSERTION_ORIGINS:
                failures.append(f"{case_name}: sequence background origin is invalid")

    frames = sequence.get("frames")
    if not isinstance(frames, list) or not frames:
        failures.append(f"{case_name}: sequence has no frame evidence")
        return failures
    if isinstance(expected_frame_count, int) and len(frames) != expected_frame_count:
        failures.append(
            f"{case_name}: sequence has {len(frames)} frames but Pillow reports {expected_frame_count}"
        )
    for index, frame in enumerate(frames):
        frame_name = f"{case_name}: frame {index}"
        if not isinstance(frame, dict):
            failures.append(f"{frame_name} evidence is not an object")
            continue
        if frame.get("index") != index:
            failures.append(f"{frame_name} has a stale index")
        rect = frame.get("source_rect")
        if (
            not isinstance(rect, list)
            or len(rect) != 4
            or any(not isinstance(value, int) or value < 0 for value in rect)
            or rect[2] == 0
            or rect[3] == 0
        ):
            failures.append(f"{frame_name} source rectangle is invalid")
        elif canvas is not None and (
            rect[0] + rect[2] > canvas[0] or rect[1] + rect[3] > canvas[1]
        ):
            failures.append(f"{frame_name} source rectangle exceeds the canvas")
        duration_num = frame.get("duration_num")
        duration_den = frame.get("duration_den")
        if (
            not isinstance(duration_num, int)
            or isinstance(duration_num, bool)
            or duration_num < 0
            or not isinstance(duration_den, int)
            or isinstance(duration_den, bool)
            or duration_den <= 0
        ):
            failures.append(f"{frame_name} exact duration is invalid")
        for field in ("duration_origin", "source_origin"):
            if frame.get(field) not in ASSERTION_ORIGINS:
                failures.append(f"{frame_name} {field} is invalid")
        disposal = frame.get("disposal")
        if disposal not in {"unspecified", "keep", "background", "previous"} and not (
            isinstance(disposal, str)
            and re.fullmatch(r"reserved:(?:[0-9]|[1-9][0-9]{1,2})", disposal)
            and int(disposal.split(":", 1)[1]) <= 255
        ):
            failures.append(f"{frame_name} disposal is invalid")
        blend = frame.get("blend")
        if blend not in {"unspecified", "source", "over"} and not (
            isinstance(blend, str)
            and re.fullmatch(r"reserved:(?:[0-9]|[1-9][0-9]{1,2})", blend)
            and int(blend.split(":", 1)[1]) <= 255
        ):
            failures.append(f"{frame_name} blend is invalid")
        if not isinstance(frame.get("interlaced"), bool):
            failures.append(f"{frame_name} interlace flag is invalid")
        if not isinstance(frame.get("is_default_image"), bool):
            failures.append(f"{frame_name} default-image flag is invalid")
        if frame.get("pixel_layout") not in {"source_rectangle", "rendered_canvas"}:
            failures.append(f"{frame_name} pixel layout is invalid")

        pixel_assertion = frame.get("pixel_assertion")
        pixel_fields = {
            "pixel_origin",
            "ref_path",
            "ref_bytes",
            "ref_sha256",
            "ref_mode",
            "ref_size",
        }
        if pixel_assertion == "not_asserted_source_layout":
            if any(frame.get(field) is not None for field in pixel_fields):
                failures.append(f"{frame_name} unasserted pixels retain evidence")
            continue
        if pixel_assertion != "exact":
            failures.append(f"{frame_name} pixel assertion is invalid")
            continue
        if frame.get("pixel_origin") not in ASSERTION_ORIGINS:
            failures.append(f"{frame_name} exact pixel origin is invalid")
        frame_path = ROOT / str(frame.get("ref_path", ""))
        ref_size = frame.get("ref_size")
        if (
            not frame_path.is_file()
            or frame_path.stat().st_size != frame.get("ref_bytes")
            or frame.get("ref_sha256") != sha256(frame_path.read_bytes())
            or not isinstance(frame.get("ref_mode"), str)
            or not isinstance(ref_size, list)
            or len(ref_size) != 2
            or any(not isinstance(value, int) or value <= 0 for value in ref_size)
        ):
            failures.append(f"{frame_name} exact pixel evidence is invalid")
    return failures


def validate_generated_outputs(matrix, target_format=None):
    """Require complete evidence for every active row and none for planned rows."""
    failures = []
    for fmt_name, fmt_data in matrix.get("formats", {}).items():
        if target_format and fmt_name != target_format:
            continue
        for row in fmt_data.get("decode", []):
            case_name = f"{fmt_name}/{row['id']}"
            if row.get("status") == "planned":
                if not row.get("gap"):
                    failures.append(f"{case_name}: planned decode row has no gap reason")
                if row.get("ref_path"):
                    failures.append(f"{case_name}: planned decode row retains a reference")
                continue
            if row.get("execution") != execution_contract():
                failures.append(f"{case_name}: native execution contract is missing")
            if row.get("operations") != decode_operation_expectations(row):
                failures.append(f"{case_name}: decode operation contract is missing or stale")
            if row.get("error_contracts") != decode_error_contracts(row, fmt_name):
                failures.append(f"{case_name}: decode error contracts are missing or stale")
            origins = row.get("assertion_origins")
            if (
                not isinstance(origins, dict)
                or not {"detection", "inspection", "verification", "decode"}.issubset(
                    origins
                )
                or any(origin not in ASSERTION_ORIGINS for origin in origins.values())
            ):
                failures.append(f"{case_name}: assertion origins are missing or invalid")
            asset_path = ASSETS_DIR / fmt_name / str(row.get("asset", ""))
            if (
                not asset_path.is_file()
                or row.get("asset_sha256") != sha256(asset_path.read_bytes())
            ):
                failures.append(f"{case_name}: asset SHA-256 is missing or stale")
            if row.get("verify_status") not in {"ok", "error"}:
                failures.append(f"{case_name}: active decode row lacks Pillow verify evidence")
            if row.get("inspect_status") not in {"ok", "error"}:
                failures.append(f"{case_name}: active decode row lacks Pillow inspect evidence")
            if row.get("inspect_status") == "error" and (
                not row.get("inspect_error_type") or not row.get("inspect_error_kind")
            ):
                failures.append(f"{case_name}: inspect error lacks Pillow exception mapping")
            if row.get("verify_status") == "error" and (
                not row.get("verify_error_type") or not row.get("verify_error_kind")
            ):
                failures.append(f"{case_name}: verify error lacks Pillow exception mapping")
            if row.get("verification_scope") not in {"header_only", "structure"}:
                failures.append(f"{case_name}: verification scope is missing or invalid")
            if row.get("inspect_status") == "ok" and not row.get(
                "inspect_container_format"
            ):
                failures.append(f"{case_name}: inspect container format is missing")
            if row.get("inspect_status") == "ok" and (
                not isinstance(row.get("ref_bit_depth"), int)
                or not 1 <= row["ref_bit_depth"] <= 32
            ):
                failures.append(f"{case_name}: inspect bit-depth evidence is missing")
            if row.get("inspect_status") == "ok" and row.get(
                "ref_bit_depth_origin"
            ) not in ASSERTION_ORIGINS:
                failures.append(f"{case_name}: inspect bit-depth origin is missing")
            if row.get("inspect_container_format") == "CUR" and (
                not isinstance(row.get("inspect_cursor_hotspot"), list)
                or len(row["inspect_cursor_hotspot"]) != 2
            ):
                failures.append(f"{case_name}: CUR hotspot evidence is missing")
            if row.get("expect_error"):
                if (
                    row.get("oracle_status") != "error"
                    or not row.get("oracle_error_type")
                    or not row.get("oracle_error_kind")
                ):
                    failures.append(f"{case_name}: error row lacks Pillow exception mapping")
                continue
            reference = row.get("ref_path")
            if not reference:
                failures.append(f"{case_name}: active decode row lacks pixel evidence")
                continue
            path = ROOT / reference
            if (
                not path.exists()
                or path.stat().st_size != row.get("ref_bytes")
                or row.get("ref_sha256") != sha256(path.read_bytes())
            ):
                failures.append(f"{case_name}: decode pixel evidence is missing or has wrong size")
            for field in ("inspect_palette", "decoded_palette"):
                palette = row.get(field)
                if not isinstance(palette, dict):
                    failures.append(f"{case_name}: {field} evidence is missing")
                    continue
                state = palette.get("state")
                if state not in {"absent", "implicit", "table"}:
                    failures.append(f"{case_name}: {field} state is invalid")
                if palette.get("origin") not in ASSERTION_ORIGINS:
                    failures.append(f"{case_name}: {field} origin is missing")
                if state == "table":
                    rgb_path = ROOT / palette.get("rgb_path", "")
                    rgb_bytes = palette.get("rgb_bytes")
                    if (
                        not rgb_path.is_file()
                        or not isinstance(rgb_bytes, int)
                        or rgb_bytes < 3
                        or rgb_bytes % 3
                        or rgb_path.stat().st_size != rgb_bytes
                        or palette.get("rgb_sha256") != sha256(rgb_path.read_bytes())
                    ):
                        failures.append(f"{case_name}: {field} RGB evidence is invalid")
                    alpha_path = palette.get("alpha_path")
                    alpha_bytes = palette.get("alpha_bytes")
                    if (alpha_path is None) != (alpha_bytes is None):
                        failures.append(f"{case_name}: {field} alpha evidence is incomplete")
                    elif alpha_path is not None and (
                        not (ROOT / alpha_path).is_file()
                        or not isinstance(alpha_bytes, int)
                        or alpha_bytes < 1
                        or alpha_bytes > rgb_bytes // 3
                        or (ROOT / alpha_path).stat().st_size != alpha_bytes
                        or palette.get("alpha_sha256")
                        != sha256((ROOT / alpha_path).read_bytes())
                    ):
                        failures.append(f"{case_name}: {field} alpha evidence is invalid")
                elif any(
                    key in palette
                    for key in (
                        "rgb_path",
                        "rgb_bytes",
                        "rgb_sha256",
                        "alpha_path",
                        "alpha_bytes",
                        "alpha_sha256",
                    )
                ):
                    failures.append(f"{case_name}: {field} non-table state retains bytes")
            if row.get("expect_sequence_error"):
                if (
                    row.get("sequence_status") != "error"
                    or not row.get("sequence_error_type")
                    or not row.get("sequence_error_kind")
                ):
                    failures.append(
                        f"{case_name}: sequence error row lacks Pillow exception mapping"
                    )
                if row.get("sequence"):
                    failures.append(
                        f"{case_name}: sequence error row retains successful frame evidence"
                    )
                continue
            if row.get("rust_expect_sequence_error") and (
                row.get("rust_sequence_error_kind") != "unsupported"
                or not row.get("rust_sequence_error_reason")
                or row.get("ref_is_animated") is not True
                or (
                    row.get("ref_frame_count") is not None
                    and row["ref_frame_count"] <= 1
                )
            ):
                failures.append(
                    f"{case_name}: Rust sequence error row lacks a multi-frame oracle and contract"
                )
            sequence = row.get("sequence")
            if sequence:
                if not {
                    "sequence_canvas",
                    "sequence_source",
                }.issubset(origins):
                    failures.append(
                        f"{case_name}: sequence assertion origins are incomplete"
                    )
                if any(
                    frame.get("pixel_assertion") == "exact"
                    for frame in sequence.get("frames", [])
                    if isinstance(frame, dict)
                ) and "sequence_pixels" not in origins:
                    failures.append(
                        f"{case_name}: exact sequence pixels lack a row-level origin"
                    )
                failures.extend(
                    validate_sequence_reference(
                        sequence, case_name, row.get("ref_frame_count")
                    )
                )

        for row in fmt_data.get("encode", []):
            case_name = f"{fmt_name}/{row['id']}"
            if row.get("status") == "planned":
                if not row.get("gap"):
                    failures.append(f"{case_name}: planned encode row has no gap reason")
                if row.get("ref_path") or row.get("encoded_ref_path"):
                    failures.append(f"{case_name}: planned encode row retains oracle evidence")
                continue
            if row.get("execution") != execution_contract():
                failures.append(f"{case_name}: native execution contract is missing")
            if row.get("operations") != encode_operation_expectations(row):
                failures.append(f"{case_name}: encode operation contract is missing or stale")
            if row.get("error_contracts") != encode_error_contracts(row, fmt_name):
                failures.append(f"{case_name}: encode error contracts are missing or stale")
            origins = row.get("assertion_origins")
            if (
                not isinstance(origins, dict)
                or not {"source", "encode"}.issubset(origins)
                or any(origin not in ASSERTION_ORIGINS for origin in origins.values())
            ):
                failures.append(f"{case_name}: assertion origins are missing or invalid")
            source_path = (
                ASSETS_DIR
                / str(row.get("source_format") or fmt_name)
                / str(row.get("source_asset", ""))
            )
            if (
                not source_path.is_file()
                or row.get("source_sha256") != sha256(source_path.read_bytes())
            ):
                failures.append(f"{case_name}: source SHA-256 is missing or stale")
            if not row.get("source_mode"):
                failures.append(f"{case_name}: active encode row lacks source-mode evidence")
            if row.get("expect_error"):
                if (
                    row.get("oracle_status") != "error"
                    or not row.get("oracle_error_type")
                    or not row.get("oracle_error_kind")
                ):
                    failures.append(f"{case_name}: error row lacks Pillow exception mapping")
                continue
            if row.get("rust_expect_error") and (
                row.get("rust_error_kind")
                not in {
                    "unknown_format",
                    "feature_disabled",
                    "malformed",
                    "unsupported",
                    "dimensions",
                    "parameter",
                }
                or not row.get("rust_error_reason")
            ):
                failures.append(
                    f"{case_name}: Rust-only error row lacks a supported kind and reason"
                )
            sequence = row.get("sequence")
            if sequence:
                if not {
                    "sequence_canvas",
                    "sequence_source",
                    "sequence_pixels",
                }.issubset(origins):
                    failures.append(
                        f"{case_name}: encoded sequence assertion origins are incomplete"
                    )
                failures.extend(
                    validate_sequence_reference(
                        sequence, case_name, row.get("source_frame_count")
                    )
                )
            evidence = [("encoded_ref_path", "encoded_ref_bytes", "encoded bytes")]
            if not row.get("params", {}).get("encoded_only"):
                evidence.insert(0, ("ref_path", "ref_bytes", "roundtrip pixels"))
            for path_field, size_field, label in evidence:
                reference = row.get(path_field)
                if not reference:
                    failures.append(f"{case_name}: active encode row lacks {label}")
                    continue
                path = ROOT / reference
                checksum_field = (
                    "encoded_ref_sha256"
                    if path_field == "encoded_ref_path"
                    else "ref_sha256"
                )
                if (
                    not path.exists()
                    or path.stat().st_size != row.get(size_field)
                    or row.get(checksum_field) != sha256(path.read_bytes())
                ):
                    failures.append(f"{case_name}: {label} evidence is missing or has wrong size")
    if failures:
        detail = "\n  - ".join(failures)
        raise RuntimeError(f"generated oracle evidence is incomplete:\n  - {detail}")


def exact_encode_parity_supported(fmt_name, row):
    """Pinned Pillow makes every active encode roundtrip deterministic."""
    if row.get("params", {}).get("encoded_only"):
        return False
    return True


def preflight_encode_cases(matrix, target_format=None):
    """Reject false active coverage before rewriting any derived references."""
    from PIL import Image

    failures = []
    for fmt_name, fmt_data in matrix.get("formats", {}).items():
        if target_format and fmt_name != target_format:
            continue
        for row in fmt_data.get("encode", []):
            if row.get("status") != "active":
                continue
            case_name = f"{fmt_name}/{row.get('id', '?')}"
            source_format = row.get("source_format")
            source_asset = row.get("source_asset")
            if not source_format or not source_asset:
                failures.append(f"{case_name}: no explicit source asset")
                continue
            source_path = ASSETS_DIR / source_format / source_asset
            if not source_path.exists():
                failures.append(f"{case_name}: source asset does not exist: {source_path}")
                continue
            try:
                kwargs = encode_params(fmt_name, dict(row.get("params", {})))
                with pillow_open_asset(source_path) as image:
                    validate_source_params(image, row.get("params", {}), fmt_name)
                    prepare_multiframe_call(image, kwargs)
            except Exception as error:
                if row.get("expect_error"):
                    continue
                failures.append(f"{case_name}: {error}")
    if failures:
        detail = "\n  - ".join(failures)
        raise RuntimeError(f"active encode cases do not map exactly to Pillow:\n  - {detail}")


def generate_decode(manifest, matrix, target_format=None):
    """Generate Decode refs: raw pixel bytes from PIL."""
    generated = 0
    for fmt_name, fmt_data in manifest["formats"].items():
        if target_format and fmt_name != target_format:
            continue
        for case in fmt_data.get("edge_cases", []):
            if case.get("status") == "planned" and case.get("oracle_gap"):
                continue
            for asset_name in case.get("test_assets", []):
                row = ensure_decode_row(matrix, fmt_name, fmt_data, case, asset_name)
                if fmt_data.get("status") == "planned" or case.get("status") == "planned":
                    row["status"] = "planned"
                    clear_pixel_ref(row)
                    row.pop("verify_status", None)
                    row.pop("verify_error_type", None)
                    row.pop("verify_error_message", None)
                    row.pop("verify_error_kind", None)
                    row.pop("inspect_status", None)
                    row.pop("inspect_error_type", None)
                    row.pop("inspect_error_message", None)
                    row.pop("inspect_error_kind", None)
                    row.pop("ref_bit_depth", None)
                    row.pop("ref_bit_depth_origin", None)
                    continue
                img_path = ASSETS_DIR / fmt_name / asset_name
                if not img_path.exists():
                    continue
                asset_bytes = img_path.read_bytes()
                row["asset_sha256"] = sha256(asset_bytes)
                row["execution"] = execution_contract()
                row["oracle_detects_format"] = oracle_detects_format(fmt_name, asset_bytes)
                write_inspect_ref(row, img_path, fmt_name)
                write_verify_ref(row, img_path)
                if row.get("expect_error"):
                    clear_pixel_ref(row)
                    try:
                        from PIL import Image

                        with pillow_open_asset(img_path) as image:
                            image.load()
                    except Exception as error:
                        row["oracle_status"] = "error"
                        row["oracle_error_type"] = (
                            f"{type(error).__module__}.{type(error).__name__}"
                        )
                        row["oracle_error_message"] = stable_error_message(error)
                        row["oracle_error_kind"] = decode_error_kind(
                            row["oracle_detects_format"], error
                        )
                    continue
                try:
                    from PIL import Image
                    img = pillow_open_asset(img_path)
                    ref_name = f"Decode.{fmt_name}_{asset_name.replace('.', '_')}.bin"

                    row["status"] = "active"
                    row["oracle_status"] = "ok"
                    row.pop("oracle_error_type", None)
                    row.pop("oracle_error_message", None)
                    row.pop("oracle_error_kind", None)
                    write_pixel_ref(row, img, ref_name)
                    write_palette_refs(row, img, fmt_name, img_path, ref_name)
                    if row.get("expect_sequence_error"):
                        write_sequence_error_ref(row, img_path)
                    else:
                        row.pop("sequence_status", None)
                        row.pop("sequence_error_type", None)
                        row.pop("sequence_error_message", None)
                        row.pop("sequence_error_kind", None)
                        if fmt_name == "gif":
                            with pillow_open_asset(img_path) as sequence_image:
                                write_sequence_ref(
                                    row, sequence_image, fmt_name, asset_name
                                )
                        else:
                            write_sequence_ref(row, img, fmt_name, asset_name)
                    generated += 1
                except Exception as e:
                    print(f"  SKIP decode {asset_name}: {e}", file=sys.stderr)

        for row in matrix["formats"][fmt_name].get("decode", []):
            if row.get("status") != "active":
                continue
            origins = {
                "detection": (
                    "specification_reference"
                    if fmt_name == "avif"
                    else "pillow_fixture"
                ),
                "inspection": "pillow_fixture",
                "verification": "pillow_fixture",
                "decode": "pillow_fixture",
                "operations": "pillow_fixture",
            }
            if row.get("sequence"):
                origins["sequence_canvas"] = "pillow_fixture"
                origins["sequence_source"] = {
                    "png": "specification_reference",
                    "gif": "pillow_fixture",
                    "tiff": "pillow_fixture",
                    "webp": "specification_reference",
                    "avif": "independent_implementation",
                }[fmt_name]
                if any(
                    frame.get("pixel_assertion") == "exact"
                    for frame in row["sequence"]["frames"]
                ):
                    origins["sequence_pixels"] = "pillow_fixture"
            if row.get("rust_expect_sequence_error"):
                origins["rust_sequence_contract"] = "defensive_model"
            row["assertion_origins"] = origins
            row["operations"] = decode_operation_expectations(row)
            row["error_contracts"] = decode_error_contracts(row, fmt_name)

        # Also write input/output JSONs
        dec_cases = [r for r in matrix["formats"][fmt_name].get("decode", [])
                     if r.get("status") == "active" and r.get("asset")]
        inp_data = [
            {
                "id": r["id"],
                "asset": r["asset"],
                "asset_sha256": r.get("asset_sha256"),
                "execution": r.get("execution"),
                "assertion_origins": r.get("assertion_origins"),
                "operations": r.get("operations"),
                "error_contracts": r.get("error_contracts"),
                "expect_error": bool(r.get("expect_error", False)),
                "expect_sequence_error": bool(
                    r.get("expect_sequence_error", False)
                ),
                "verification_scope": r.get("verification_scope"),
                "rust_expect_sequence_error": bool(
                    r.get("rust_expect_sequence_error", False)
                ),
                "rust_sequence_error_kind": r.get("rust_sequence_error_kind"),
                "rust_sequence_error_reason": r.get("rust_sequence_error_reason"),
                "pillow_call": {
                    "open": f"tests/fixtures/input/images/{fmt_name}/{r['asset']}",
                    "operations": ["PIL.Image.open", "load", "tobytes"],
                },
                "pillow_verify_call": ["PIL.Image.open", "verify"],
            }
            for r in dec_cases
        ]
        inp = {
            "format_version": 3,
            "oracle": oracle_identity(manifest),
            "operation": {"module": "Decode", "target": fmt_name},
            "cases": inp_data,
        }
        INPUT_JSONS.mkdir(parents=True, exist_ok=True)
        (INPUT_JSONS / f"Decode.{fmt_name}.json").write_text(json.dumps(inp, indent=2) + "\n")

        out_data = [
            {
                "id": r["id"],
                "status": r.get("oracle_status"),
                "error_type": r.get("oracle_error_type"),
                "error_message": r.get("oracle_error_message"),
                "error_kind": r.get("oracle_error_kind"),
                "inspect_status": r.get("inspect_status"),
                "inspect_error_type": r.get("inspect_error_type"),
                "inspect_error_message": r.get("inspect_error_message"),
                "inspect_error_kind": r.get("inspect_error_kind"),
                "verify_status": r.get("verify_status"),
                "verify_error_type": r.get("verify_error_type"),
                "verify_error_message": r.get("verify_error_message"),
                "verify_error_kind": r.get("verify_error_kind"),
                "verification_scope": r.get("verification_scope"),
                "inspect_container_format": r.get("inspect_container_format"),
                "inspect_cursor_hotspot": r.get("inspect_cursor_hotspot"),
                "asset_sha256": r.get("asset_sha256"),
                "execution": r.get("execution"),
                "assertion_origins": r.get("assertion_origins"),
                "operations": r.get("operations"),
                "error_contracts": r.get("error_contracts"),
                "ref_bit_depth": r.get("ref_bit_depth"),
                "ref_bit_depth_origin": r.get("ref_bit_depth_origin"),
                "ref_path": r.get("ref_path"),
                "ref_bytes": r.get("ref_bytes"),
                "ref_sha256": r.get("ref_sha256"),
                "ref_mode": r.get("ref_mode"),
                "ref_size": r.get("ref_size"),
                "ref_frame_count": r.get("ref_frame_count"),
                "ref_is_animated": r.get("ref_is_animated"),
                "inspect_palette": r.get("inspect_palette"),
                "decoded_palette": r.get("decoded_palette"),
                "sequence_status": r.get("sequence_status"),
                "sequence_error_type": r.get("sequence_error_type"),
                "sequence_error_message": r.get("sequence_error_message"),
                "sequence_error_kind": r.get("sequence_error_kind"),
                "rust_expect_sequence_error": bool(
                    r.get("rust_expect_sequence_error", False)
                ),
                "rust_sequence_error_kind": r.get("rust_sequence_error_kind"),
                "rust_sequence_error_reason": r.get("rust_sequence_error_reason"),
                **({"sequence": r["sequence"]} if r.get("sequence") else {}),
            }
            for r in dec_cases
        ]
        out = {
            "format_version": 3,
            "oracle": oracle_identity(manifest),
            "operation": {"module": "Decode", "target": fmt_name},
            "cases": out_data,
        }
        OUTPUT_JSONS.mkdir(parents=True, exist_ok=True)
        (OUTPUT_JSONS / f"Decode.{fmt_name}.json").write_text(json.dumps(out, indent=2) + "\n")

    return generated


def generate_encode(manifest, matrix, target_format=None):
    """Generate Encode refs: PIL roundtrip pixel bytes."""
    from PIL import Image
    generated = 0

    for fmt_name, fmt_data in matrix["formats"].items():
        if target_format and fmt_name != target_format:
            continue
        for row in fmt_data.get("encode", []):
            if row.get("status") != "active":
                continue
            # References are derived state. Clear stale metadata before every
            # attempt so a missing or newly invalid source cannot retain an old
            # green result in the authoritative matrix.
            clear_pixel_ref(row)
            clear_encoded_ref(row)
            row.pop("sequence", None)
            row.pop("oracle_status", None)
            row.pop("oracle_error_type", None)
            row.pop("oracle_error_message", None)
            row.pop("oracle_error_kind", None)
            src_fmt = row.get("source_format") or fmt_name
            src_asset = row.get("source_asset")
            if not src_asset:
                continue
            src_path = ASSETS_DIR / src_fmt / src_asset
            if not src_path.exists():
                continue

            try:
                img = pillow_open_asset(src_path)
                source_bytes = src_path.read_bytes()
                row["source_sha256"] = sha256(source_bytes)
                row["execution"] = execution_contract()
                row["source_mode"] = mode_name(img)
                row["source_frame_count"] = int(getattr(img, "n_frames", 1))
                params = row.get("params", {})
                validate_source_params(img, params, fmt_name)
                kwargs = encode_params(fmt_name, dict(params))
                if params.get("detach_source"):
                    img = Image.frombytes(img.mode, img.size, img.tobytes())
                if params.get("truncate_pixels"):
                    img = Image.frombytes(img.mode, img.size, img.tobytes()[:-1])
                if source_dimensions := params.get("source_dimensions"):
                    img = Image.new(img.mode, tuple(source_dimensions))
                if params.get("oversized_palette"):
                    img.putpalette(bytes(771))
                if params.get("palette_on_nonindexed"):
                    img.putpalette(bytes(768))
                image_to_save, kwargs = prepare_multiframe_call(img, kwargs)
                buf = io.BytesIO()
                image_to_save.save(buf, format=fmt_pil(fmt_name), **kwargs)
                encoded = buf.getvalue()
                if fmt_name == "avif" and params.get("sequence_time"):
                    encoded = canonicalize_avif_sequence_times(
                        encoded, params["sequence_time"]
                    )
                if row.get("expect_error"):
                    row["oracle_status"] = "ok"
                    continue
                if row.get("rust_expect_error"):
                    row["oracle_status"] = "ok"
                encoded_name = f"Encode.{fmt_name}_{row['id']}.bin"
                OUTPUT_ENCODED.mkdir(parents=True, exist_ok=True)
                (OUTPUT_ENCODED / encoded_name).write_bytes(encoded)
                row["encoded_ref_path"] = (
                    Path("tests") / "fixtures" / "outputs" / "encoded" / encoded_name
                ).as_posix()
                row["encoded_ref_bytes"] = len(encoded)
                row["encoded_ref_sha256"] = sha256(encoded)
                buf.seek(0)
                rt = Image.open(buf)
                if exact_encode_parity_supported(fmt_name, row):
                    ref_name = f"Encode.{fmt_name}_{row['id']}.bin"
                    write_pixel_ref(row, rt, ref_name)
                    if row["source_frame_count"] > 1 and fmt_name in {"tiff", "webp"}:
                        write_sequence_ref_from_data(
                            row,
                            rt,
                            fmt_name,
                            f"encoded_{row['id']}.{fmt_name}",
                            encoded,
                        )
                    generated += 1
                else:
                    clear_pixel_ref(row)
            except Exception as e:
                if row.get("expect_error"):
                    row["oracle_status"] = "error"
                    row["oracle_error_type"] = f"{type(e).__module__}.{type(e).__name__}"
                    row["oracle_error_message"] = stable_error_message(e)
                    row["oracle_error_kind"] = encode_error_kind(row, e)
                    continue
                # Lossy formats or unsupported params — skip ref, just verify dimensions
                print(f"  SKIP encode {row.get('id')}: {e}", file=sys.stderr)

        for row in fmt_data.get("encode", []):
            if row.get("status") != "active":
                continue
            origins = {
                "source": "pillow_fixture",
                "encode": "pillow_fixture",
                "operations": "pillow_fixture",
            }
            if row.get("rust_expect_error"):
                origins["rust_contract"] = "defensive_model"
            if row.get("sequence"):
                origins.update(
                    {
                        "sequence_canvas": "pillow_fixture",
                        "sequence_source": "pillow_fixture",
                        "sequence_pixels": "pillow_fixture",
                    }
                )
            row["assertion_origins"] = origins
            row["operations"] = encode_operation_expectations(row)
            row["error_contracts"] = encode_error_contracts(row, fmt_name)

        # Encode input/output JSONs
        enc_cases = [r for r in fmt_data.get("encode", [])
                     if r.get("status") == "active" and r.get("source_asset")]
        if enc_cases:
            inp_data = [
                {
                    "id": r["id"],
                    "source_asset": r["source_asset"],
                    "source_format": r.get("source_format", fmt_name),
                    "source_mode": r.get("source_mode"),
                    "source_sha256": r.get("source_sha256"),
                    "execution": r.get("execution"),
                    "assertion_origins": r.get("assertion_origins"),
                    "operations": r.get("operations"),
                    "error_contracts": r.get("error_contracts"),
                    **({"sequence": r["sequence"]} if r.get("sequence") else {}),
                    "params": r.get("params", {}),
                    **({"expect_error": True} if r.get("expect_error") else {}),
                    **(
                        {
                            "rust_expect_error": True,
                            "rust_error_kind": r.get("rust_error_kind"),
                            "rust_error_reason": r.get("rust_error_reason"),
                        }
                        if r.get("rust_expect_error")
                        else {}
                    ),
                    "pillow_call": describe_encode_call(fmt_name, r),
                }
                for r in enc_cases
            ]
            inp = {
                "format_version": 3,
                "oracle": oracle_identity(manifest),
                "operation": {"module": "Encode", "target": fmt_name},
                "cases": inp_data,
            }
            (INPUT_JSONS / f"Encode.{fmt_name}.json").write_text(json.dumps(inp, indent=2) + "\n")

            out_data = [
                {
                    "id": r["id"],
                    "ref_path": r.get("ref_path"),
                    "ref_bytes": r.get("ref_bytes"),
                    "ref_mode": r.get("ref_mode"),
                    "ref_size": r.get("ref_size"),
                    "ref_sha256": r.get("ref_sha256"),
                    "encoded_ref_path": r.get("encoded_ref_path"),
                    "encoded_ref_bytes": r.get("encoded_ref_bytes"),
                    "encoded_ref_sha256": r.get("encoded_ref_sha256"),
                    "source_mode": r.get("source_mode"),
                    "source_sha256": r.get("source_sha256"),
                    "execution": r.get("execution"),
                    "assertion_origins": r.get("assertion_origins"),
                    "operations": r.get("operations"),
                    "error_contracts": r.get("error_contracts"),
                    **({"sequence": r["sequence"]} if r.get("sequence") else {}),
                    **(
                        {
                            "oracle_status": r.get("oracle_status"),
                            "error_type": r.get("oracle_error_type"),
                            "error_message": r.get("oracle_error_message"),
                            "error_kind": r.get("oracle_error_kind"),
                        }
                        if r.get("expect_error")
                        else {}
                    ),
                    **(
                        {
                            "oracle_status": r.get("oracle_status"),
                            "rust_expect_error": True,
                            "rust_error_kind": r.get("rust_error_kind"),
                            "rust_error_reason": r.get("rust_error_reason"),
                        }
                        if r.get("rust_expect_error")
                        else {}
                    ),
                }
                for r in enc_cases
            ]
            out = {
                "format_version": 3,
                "oracle": oracle_identity(manifest),
                "operation": {"module": "Encode", "target": fmt_name},
                "cases": out_data,
            }
            OUTPUT_JSONS.mkdir(parents=True, exist_ok=True)
            (OUTPUT_JSONS / f"Encode.{fmt_name}.json").write_text(json.dumps(out, indent=2) + "\n")

    return generated


def generate(target_format=None):
    # Load
    manifest = yaml.safe_load(MANIFEST.read_text())
    verify_primary_oracle(manifest)
    preflight_decode_cases(manifest, target_format)
    matrix = json.loads(MATRIX_PATH.read_text()) if MATRIX_PATH.exists() else {"formats": {}}
    sync_decode_rows(manifest, matrix)
    sync_encode_rows(manifest, matrix)
    preflight_encode_cases(matrix, target_format)
    clear_generated_outputs(manifest, target_format)

    # Decode
    n_dec = generate_decode(manifest, matrix, target_format)
    print(f"Decode: {n_dec} refs")

    # Encode
    n_enc = generate_encode(manifest, matrix, target_format)
    print(f"Encode: {n_enc} refs")
    matrix.pop("operations", None)
    update_summary(matrix)
    validate_generated_outputs(matrix, target_format)

    # Save matrix
    MATRIX_PATH.write_text(json.dumps(matrix, indent=2))
    print(f"Written: {MATRIX_PATH}")

    # Commit outputs
    print("\nAuthoritative Pillow refs generated in tests/fixtures/outputs/.")


def verify_primary_oracle(manifest):
    """Refuse to rewrite references with an unpinned Pillow build."""
    import PIL
    import PIL._imaging
    from PIL import features

    oracle = manifest.get("reference_oracles", {}).get("primary", {})
    locked = yaml.safe_load(ORACLE_LOCK.read_text()).get("oracle", {})
    for field in ("implementation", "version", "python", "platform", "wheel_sha256", "imaging_extension_sha256"):
        if str(oracle.get(field, "")) != str(locked.get(field, "")):
            raise RuntimeError(f"manifest and pillow-oracle.lock.yaml disagree on {field}")

    expected_name = oracle.get("implementation")
    expected_version = str(oracle.get("version", ""))
    if expected_name != "Pillow" or not expected_version:
        raise RuntimeError("manifest.yaml must pin the primary Pillow oracle version")
    if PIL.__version__ != expected_version:
        raise RuntimeError(
            "Pillow oracle version mismatch: "
            f"manifest requires {expected_version}, installed version is {PIL.__version__}"
        )
    expected_python = str(oracle.get("python"))
    actual_python = f"{sys.version_info.major}.{sys.version_info.minor}"
    if actual_python != expected_python:
        raise RuntimeError(f"Pillow oracle requires Python {expected_python}, found {actual_python}")
    if sys.platform != "darwin" or platform.machine() != "arm64":
        raise RuntimeError("Pillow oracle requires macOS arm64")

    imaging_path = Path(PIL._imaging.__file__)
    imaging_sha256 = hashlib.sha256(imaging_path.read_bytes()).hexdigest()
    expected_imaging_sha256 = str(oracle.get("imaging_extension_sha256", ""))
    if imaging_sha256 != expected_imaging_sha256:
        raise RuntimeError(
            "Pillow _imaging binary mismatch: "
            f"expected {expected_imaging_sha256}, found {imaging_sha256}"
        )

    for format_name, format_oracle in manifest.get("reference_oracles", {}).get("formats", {}).items():
        feature = format_oracle.get("pillow_feature")
        expected = str(format_oracle.get("pillow_feature_version", ""))
        if not feature:
            continue
        actual = features.version(feature)
        if actual != expected:
            raise RuntimeError(
                f"{format_name} oracle mismatch: Pillow feature {feature} "
                f"must be {expected}, installed build reports {actual or 'unavailable'}"
            )

    from PIL import _avif

    avif_codecs = _avif.codec_versions()
    for expected_codec in ("dav1d [dec]:1.5.3", "aom [enc]:3.13.2"):
        if expected_codec not in avif_codecs:
            raise RuntimeError(
                f"AVIF oracle codec mismatch: expected {expected_codec}, found {avif_codecs}"
            )


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("--format", help="Specific format only")
    args = p.parse_args()
    generate(args.format)
