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


def ensure_decode_row(matrix, fmt_name, case, asset_name):
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
        disposal = take("disposal")
        if disposal is not None:
            kwargs["disposal"] = {"none": 0, "background": 2, "previous": 3}[disposal]
        loop = take("loop")
        if loop is True:
            kwargs["loop"] = 0
        animated = take("animated")
        frames = take("frames")
        if animated is not None:
            kwargs["_manifest_animated"] = animated
        if frames is not None:
            kwargs["_manifest_frames"] = frames
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
    if animated is None:
        return image, kwargs
    if not animated:
        return image, kwargs

    from PIL import ImageSequence

    frames = [frame.copy() for frame in ImageSequence.Iterator(image)]
    requested = frame_count or len(frames)
    if len(frames) < requested:
        raise RuntimeError(f"source has {len(frames)} frame(s), requested {requested}")
    kwargs["save_all"] = True
    kwargs["append_images"] = frames[1:requested]
    return frames[0], kwargs


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


def write_pixel_ref(row, image, ref_name):
    """Write raw PIL pixels and update one matrix/output row."""
    image.load()
    raw = image.tobytes()
    OUTPUT_RAWS.mkdir(parents=True, exist_ok=True)
    (OUTPUT_RAWS / ref_name).write_bytes(raw)
    row.pop("ref_sha256", None)
    row["ref_path"] = raw_ref_path(ref_name).as_posix()
    row["ref_bytes"] = len(raw)
    row["ref_mode"] = mode_name(image)
    row["ref_size"] = list(image.size)
    return raw


def write_sequence_ref(row, image, fmt_name, asset_name):
    """Write every Pillow-composited WebP frame as exact oracle evidence."""
    if fmt_name != "webp" or getattr(image, "n_frames", 1) <= 1:
        row.pop("sequence", None)
        return

    frames = []
    for index in range(image.n_frames):
        image.seek(index)
        image.load()
        raw = image.tobytes()
        ref_name = (
            f"Decode.{fmt_name}_{asset_name.replace('.', '_')}_frame_{index}.bin"
        )
        OUTPUT_RAWS.mkdir(parents=True, exist_ok=True)
        (OUTPUT_RAWS / ref_name).write_bytes(raw)
        frames.append(
            {
                "index": index,
                "ref_path": raw_ref_path(ref_name).as_posix(),
                "ref_bytes": len(raw),
                "ref_mode": mode_name(image),
                "ref_size": list(image.size),
                "duration_ms": int(image.info.get("duration", 0)),
            }
        )
    row["sequence"] = {
        "loop_count": image.info.get("loop"),
        "frames": frames,
    }


def clear_pixel_ref(row):
    row.pop("ref_sha256", None)
    row.pop("ref_path", None)
    row.pop("ref_bytes", None)
    row.pop("ref_mode", None)
    row.pop("ref_size", None)
    row.pop("sequence", None)


def clear_encoded_ref(row):
    row.pop("encoded_ref_path", None)
    row.pop("encoded_ref_bytes", None)


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
    return value


def describe_encode_call(fmt_name, row):
    kwargs = encode_params(fmt_name, dict(row.get("params", {})))
    animated = kwargs.pop("_manifest_animated", None)
    frame_count = kwargs.pop("_manifest_frames", None)
    if animated:
        kwargs["save_all"] = True
        kwargs["append_images"] = {
            "type": "frames_from_source",
            "start": 1,
            "count": (frame_count or 1) - 1,
        }
    call = {
        "open": f"tests/fixtures/input/images/{row['source_format']}/{row['source_asset']}",
        "method": "PIL.Image.Image.save",
        "format": fmt_pil(fmt_name),
        "kwargs": json_pillow_value(kwargs),
        "roundtrip": ["PIL.Image.open", "load", "tobytes"],
    }
    if row.get("params", {}).get("truncate_pixels"):
        call["source_transform"] = "PIL.Image.frombytes(mode, size, tobytes()[:-1])"
    if source_dimensions := row.get("params", {}).get("source_dimensions"):
        call["source_transform"] = {
            "method": "PIL.Image.new",
            "mode": "source mode",
            "size": source_dimensions,
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
        "total_rows": len(decode_rows) + len(encode_rows) + len(matrix.get("operations", [])),
        "decode_rows": len(decode_rows),
        "encode_rows": len(encode_rows),
        "formats": len(formats),
        "assets_available": len(assets),
        "decode_active": sum(row.get("status") == "active" for row in decode_rows),
        "decode_planned": sum(row.get("status") == "planned" for row in decode_rows),
        "encode_not_wired": sum(row.get("status") == "planned" for row in encode_rows),
        "operation_rows": len(matrix.get("operations", [])),
    }


def generate_operations(manifest, matrix):
    """Generate exact Pillow results for public image operations."""
    from PIL import Image, ImageOps

    output_dir = OUTPUT_RAWS.parent / "operations"
    output_dir.mkdir(parents=True, exist_ok=True)
    rows = []
    transpose = {
        "fliph": Image.Transpose.FLIP_LEFT_RIGHT,
        "flipv": Image.Transpose.FLIP_TOP_BOTTOM,
        "rotate90": Image.Transpose.ROTATE_270,
        "rotate180": Image.Transpose.ROTATE_180,
        "rotate270": Image.Transpose.ROTATE_90,
    }
    for specification in manifest.get("operations", []):
        source = ASSETS_DIR / specification["source_format"] / specification["source_asset"]
        params = dict(specification.get("params", {}))
        with pillow_open_asset(source) as opened:
            image = opened.copy()
        action = specification["action"]
        if action == "convert":
            result = image.convert(params["mode"])
        elif action in transpose:
            result = image.transpose(transpose[action])
        elif action == "crop":
            x, y = params["x"], params["y"]
            result = image.crop((x, y, x + params["width"], y + params["height"]))
        elif action == "invert":
            if image.mode == "RGBA":
                rgb = ImageOps.invert(image.convert("RGB"))
                rgb.putalpha(image.getchannel("A"))
                result = rgb
            else:
                result = ImageOps.invert(image)
        else:
            raise RuntimeError(f"unknown Pillow operation {action!r}")
        if params.get("mode") and action != "convert":
            result = result.convert(params["mode"])
        reference = output_dir / f"{specification['id']}.bin"
        reference.write_bytes(result.tobytes())
        rows.append(
            {
                **specification,
                "status": "active",
                "ref_path": str(reference.relative_to(ROOT)),
                "ref_bytes": reference.stat().st_size,
                "ref_mode": result.mode,
                "ref_size": list(result.size),
            }
        )
    matrix["operations"] = rows
    return len(rows)


def validate_generated_outputs(matrix):
    """Require complete evidence for every active row and none for planned rows."""
    failures = []
    for fmt_name, fmt_data in matrix.get("formats", {}).items():
        for row in fmt_data.get("decode", []):
            case_name = f"{fmt_name}/{row['id']}"
            if row.get("status") == "planned":
                if not row.get("gap"):
                    failures.append(f"{case_name}: planned decode row has no gap reason")
                if row.get("ref_path"):
                    failures.append(f"{case_name}: planned decode row retains a reference")
                continue
            if row.get("expect_error"):
                if row.get("oracle_status") != "error" or not row.get("oracle_error_type"):
                    failures.append(f"{case_name}: error row lacks Pillow exception evidence")
                continue
            reference = row.get("ref_path")
            if not reference:
                failures.append(f"{case_name}: active decode row lacks pixel evidence")
                continue
            path = ROOT / reference
            if not path.exists() or path.stat().st_size != row.get("ref_bytes"):
                failures.append(f"{case_name}: decode pixel evidence is missing or has wrong size")
            sequence = row.get("sequence")
            if sequence:
                frames = sequence.get("frames", [])
                if not frames:
                    failures.append(f"{case_name}: sequence has no frame evidence")
                for frame in frames:
                    frame_path = ROOT / frame.get("ref_path", "")
                    if (
                        not frame_path.is_file()
                        or frame_path.stat().st_size != frame.get("ref_bytes")
                    ):
                        failures.append(
                            f"{case_name}: frame {frame.get('index')} evidence is missing or has wrong size"
                        )

        for row in fmt_data.get("encode", []):
            case_name = f"{fmt_name}/{row['id']}"
            if row.get("status") == "planned":
                if not row.get("gap"):
                    failures.append(f"{case_name}: planned encode row has no gap reason")
                if row.get("ref_path") or row.get("encoded_ref_path"):
                    failures.append(f"{case_name}: planned encode row retains oracle evidence")
                continue
            if row.get("expect_error"):
                if row.get("oracle_status") != "error" or not row.get("oracle_error_type"):
                    failures.append(f"{case_name}: error row lacks Pillow exception evidence")
                continue
            for path_field, size_field, label in (
                ("ref_path", "ref_bytes", "roundtrip pixels"),
                ("encoded_ref_path", "encoded_ref_bytes", "encoded bytes"),
            ):
                reference = row.get(path_field)
                if not reference:
                    failures.append(f"{case_name}: active encode row lacks {label}")
                    continue
                path = ROOT / reference
                if not path.exists() or path.stat().st_size != row.get(size_field):
                    failures.append(f"{case_name}: {label} evidence is missing or has wrong size")
    if failures:
        detail = "\n  - ".join(failures)
        raise RuntimeError(f"generated oracle evidence is incomplete:\n  - {detail}")


def exact_encode_parity_supported(fmt_name, row):
    """Pinned Pillow makes every active encode roundtrip deterministic."""
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
                row = ensure_decode_row(matrix, fmt_name, case, asset_name)
                if fmt_data.get("status") == "planned" or case.get("status") == "planned":
                    row["status"] = "planned"
                    clear_pixel_ref(row)
                    continue
                img_path = ASSETS_DIR / fmt_name / asset_name
                if not img_path.exists():
                    continue
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
                    continue
                try:
                    from PIL import Image
                    img = pillow_open_asset(img_path)
                    ref_name = f"Decode.{fmt_name}_{asset_name.replace('.', '_')}.bin"

                    row["status"] = "active"
                    row["oracle_status"] = "ok"
                    row.pop("oracle_error_type", None)
                    row.pop("oracle_error_message", None)
                    write_pixel_ref(row, img, ref_name)
                    write_sequence_ref(row, img, fmt_name, asset_name)
                    generated += 1
                except Exception as e:
                    print(f"  SKIP decode {asset_name}: {e}", file=sys.stderr)

        # Also write input/output JSONs
        dec_cases = [r for r in matrix["formats"][fmt_name].get("decode", [])
                     if r.get("status") == "active" and r.get("asset")]
        inp_data = [
            {
                "id": r["id"],
                "asset": r["asset"],
                "expect_error": bool(r.get("expect_error", False)),
                "pillow_call": {
                    "open": f"tests/fixtures/input/images/{fmt_name}/{r['asset']}",
                    "operations": ["PIL.Image.open", "load", "tobytes"],
                },
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
                "ref_path": r.get("ref_path"),
                "ref_bytes": r.get("ref_bytes"),
                "ref_mode": r.get("ref_mode"),
                "ref_size": r.get("ref_size"),
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
            row.pop("oracle_status", None)
            row.pop("oracle_error_type", None)
            row.pop("oracle_error_message", None)
            src_fmt = row.get("source_format") or fmt_name
            src_asset = row.get("source_asset")
            if not src_asset:
                continue
            src_path = ASSETS_DIR / src_fmt / src_asset
            if not src_path.exists():
                continue

            try:
                img = pillow_open_asset(src_path)
                params = row.get("params", {})
                validate_source_params(img, params, fmt_name)
                kwargs = encode_params(fmt_name, dict(params))
                if params.get("truncate_pixels"):
                    img = Image.frombytes(img.mode, img.size, img.tobytes()[:-1])
                if source_dimensions := params.get("source_dimensions"):
                    img = Image.new(img.mode, tuple(source_dimensions))
                image_to_save, kwargs = prepare_multiframe_call(img, kwargs)
                buf = io.BytesIO()
                image_to_save.save(buf, format=fmt_pil(fmt_name), **kwargs)
                encoded = buf.getvalue()
                if row.get("expect_error"):
                    row["oracle_status"] = "ok"
                    continue
                encoded_name = f"Encode.{fmt_name}_{row['id']}.bin"
                OUTPUT_ENCODED.mkdir(parents=True, exist_ok=True)
                (OUTPUT_ENCODED / encoded_name).write_bytes(encoded)
                row["encoded_ref_path"] = (
                    Path("tests") / "fixtures" / "outputs" / "encoded" / encoded_name
                ).as_posix()
                row["encoded_ref_bytes"] = len(encoded)
                buf.seek(0)
                rt = Image.open(buf)
                if exact_encode_parity_supported(fmt_name, row):
                    ref_name = f"Encode.{fmt_name}_{row['id']}.bin"
                    write_pixel_ref(row, rt, ref_name)
                    generated += 1
                else:
                    clear_pixel_ref(row)
            except Exception as e:
                if row.get("expect_error"):
                    row["oracle_status"] = "error"
                    row["oracle_error_type"] = f"{type(e).__module__}.{type(e).__name__}"
                    row["oracle_error_message"] = stable_error_message(e)
                    continue
                # Lossy formats or unsupported params — skip ref, just verify dimensions
                print(f"  SKIP encode {row.get('id')}: {e}", file=sys.stderr)

        # Encode input/output JSONs
        enc_cases = [r for r in fmt_data.get("encode", [])
                     if r.get("status") == "active" and r.get("source_asset")]
        if enc_cases:
            inp_data = [
                {
                    "id": r["id"],
                    "source_asset": r["source_asset"],
                    "source_format": r.get("source_format", fmt_name),
                    "params": r.get("params", {}),
                    **({"expect_error": True} if r.get("expect_error") else {}),
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
                    "encoded_ref_path": r.get("encoded_ref_path"),
                    "encoded_ref_bytes": r.get("encoded_ref_bytes"),
                    **(
                        {
                            "oracle_status": r.get("oracle_status"),
                            "error_type": r.get("oracle_error_type"),
                            "error_message": r.get("oracle_error_message"),
                        }
                        if r.get("expect_error")
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
    n_operations = generate_operations(manifest, matrix)
    print(f"Operations: {n_operations} refs")
    update_summary(matrix)
    validate_generated_outputs(matrix)

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


if __name__ == "__main__":
    p = argparse.ArgumentParser()
    p.add_argument("--format", help="Specific format only")
    args = p.parse_args()
    generate(args.format)
