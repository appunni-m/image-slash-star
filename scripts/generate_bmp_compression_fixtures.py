#!/usr/bin/env python3
"""Reproduce complete BMP compression mutations and pinned native observations.

Run with .oracle-venv/bin/python. The default is a read-only consistency check;
--write writes only these input assets and their native provenance report. This
recipe never runs Rust, promotes a manifest row, or writes active pixel refs.
"""

from __future__ import annotations

import argparse
import hashlib
import io
import json
from pathlib import Path
import struct

import yaml
from PIL import BmpImagePlugin, Image

import generate_decode_refs as refs

ROOT = Path(__file__).resolve().parent.parent
ASSETS = ROOT / "tests/fixtures/input/images/bmp"
PROVENANCE = ROOT / "tests/fixtures/bmp_compression_provenance.json"
PLUGIN_SHA256 = "b8390d9fd1e08b610ea49ea76ed447d479aeb4546868bb2c1293c346e797d7fd"
SOURCE_SHA256 = {
    "os2v1.bmp": "f81f3ce538a14f835a1c60b8986ef6763cd17a562017789ee07634e97caf484b",
    "1x1.bmp": "c7cbff4b5d8c5a89ff13a80681e0fcb8a66b998b321f7007269384906cb0cc30",
    "1bit.bmp": "8e5f50d39982f7cd0af624e98f7642948704ed850a4f5df95914088fdc609a84",
    "8bit.bmp": "961659cc5f3275f6845ef6406a3af1731e209d12df3de33f10128c88c9250bf9",
    "v4header.bmp": "27af926cc51e163dabf41e70f47516957608a389f4c5d8b123c02a0a363bae70",
    "16bit.bmp": "291e550a35a111edec21233115d238e8386358f02da5c4850bec6dc0807ddcde",
    "4bit.bmp": "d0ce444f15c20bac76357c9f0f495ad44c36fc0775aab52f7016bf8b603c8d9b",
}
EXISTING_CONTROLS = {"control_core_rgb", "control_info_rgb"}
PENDING_RUST = "Not executed by this native recipe; requires final Rust fixture verification."


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def read_u32(data: bytes, offset: int) -> int:
    return struct.unpack_from("<I", data, offset)[0]


def resize_dib(source: bytes, size: int) -> bytearray:
    """Replace only the DIB extent, preserving the full palette and raster."""
    old_size = read_u32(source, 14)
    if old_size == size:
        return bytearray(source)
    if old_size < 40 or size < 40:
        raise ValueError("DIB resizing requires two INFO-compatible header sizes")
    dib = bytearray(source[14 : 14 + min(size, old_size)])
    dib.extend(bytes(size - len(dib)))
    data = bytearray(source[:14]) + dib + source[14 + old_size :]
    for offset, value in (
        (14, size),
        (10, read_u32(source, 10) + size - old_size),
        (2, len(data)),
    ):
        struct.pack_into("<I", data, offset, value)
    return data


def build_inputs() -> list[tuple[dict, bytes]]:
    sources = {}
    for name, expected in SOURCE_SHA256.items():
        data = (ASSETS / name).read_bytes()
        if sha256(data) != expected:
            raise RuntimeError(f"source SHA-256 changed: {name}")
        sources[name] = data
    cases = []

    def emit(case_id, source, description, *, header=None, compression=None,
             words=(), shorts=(), truncate=None):
        original = sources[source]
        data = resize_dib(original, header) if header else bytearray(original)
        changes = []
        if header is not None:
            changes.append({"operation": "resize_dib", "from": read_u32(original, 14),
                            "to": header, "preserve": "complete palette and raster"})
        if compression is not None:
            words = ((30, compression), *words)
        for offset, value in words:
            struct.pack_into("<I", data, offset, value)
            changes.append({"operation": "write_u32_le", "offset": offset, "value": value})
        for offset, value in shorts:
            struct.pack_into("<H", data, offset, value)
            changes.append({"operation": "write_u16_le", "offset": offset, "value": value})
        complete_sha256 = sha256(data)
        complete_bytes = len(data)
        if truncate is not None:
            del data[truncate:]
            changes.append({"operation": "truncate_full_mutated_file", "length": truncate,
                            "complete_file_bytes": complete_bytes,
                            "complete_file_sha256": complete_sha256,
                            "preserve": "declared full header and file ranges"})
        existing = case_id in EXISTING_CONTROLS
        cases.append(({
            "id": case_id,
            "description": description,
            "asset": source if existing else f"{case_id}.bmp",
            "asset_sha256": sha256(data),
            "bytes": len(data),
            "source": f"tests/fixtures/input/images/bmp/{source}",
            "source_sha256": SOURCE_SHA256[source],
            "mutations": changes,
            "header_size": read_u32(data, 14),
            "compression": None if read_u32(data, 14) == 12 else read_u32(data, 30),
            "registration": "existing_control" if existing else "new_full_file",
        }, bytes(data)))

    emit("control_core_rgb", "os2v1.bmp", "Existing OS/2 core-header RGB control")
    emit("control_info_rgb", "1x1.bmp", "Existing Windows INFO-header RGB control")
    for header in (40, 52, 56, 64, 108, 124):
        for code in (4, 5, 6, 99, 0xFFFFFFFF):
            namespace = "OS/2" if header == 64 else "Windows"
            emit(f"header{header}_compression{code}", "1x1.bmp",
                 f"{namespace} DIB {header} compression {code} rejection before raw pixels",
                 header=header, compression=code)
    for depth, source in ((1, "1bit.bmp"), (8, "8bit.bmp")):
        for code in (3, 4):
            emit(f"os2_depth{depth}_compression{code}", source,
                 f"OS/2 DIB 64 depth {depth} compression {code} rejection",
                 header=64, compression=code)
    emit("control_os2_compatible_bitfields32", "v4header.bmp",
         "Pillow accepts Windows-compatible RGBA32 bitfields in OS/2 DIB 64", header=64)
    emit("control_os2_compatible_bitfields16", "16bit.bmp",
         "Pillow accepts Windows-compatible RGB555 bitfields in OS/2 DIB 64",
         header=64, compression=3, words=((54, 0x7C00), (58, 0x3E0), (62, 0x1F)))
    emit("control_os2_compatible_bitfields24", "1x1.bmp",
         "Pillow accepts Windows-compatible RGB888 bitfields in OS/2 DIB 64",
         header=64, compression=3, words=((54, 0xFF0000), (58, 0xFF00), (62, 0xFF)))
    for depth in (0, 3):
        for code in (4, 5):
            emit(f"info_depth{depth}_compression{code}", "1x1.bmp",
                 f"Invalid depth {depth} precedes Windows compression {code} rejection",
                 compression=code, shorts=((28, depth),))
    for code in (4, 5):
        emit(f"info_palette_claim_compression{code}", "1x1.bmp",
             f"Windows compression {code} rejects before an oversized palette claim",
             compression=code, words=((46, 256),))
        for dimension, offset in (("width", 18), ("height", 22)):
            emit(f"info_zero_{dimension}_compression{code}", "1x1.bmp",
                 f"Unresolved zero {dimension} versus compression {code} error precedence",
                 compression=code, words=((offset, 0),))
    for depth, source in ((1, "1bit.bmp"), (8, "8bit.bmp"), (4, "4bit.bmp")):
        emit(f"os2_depth{depth}_huffman_nonzero_tail", source,
             f"OS/2 depth {depth} Huffman rejects despite nonzero tail words",
             header=64, compression=3, words=((54, 1), (58, 1), (62, 1)))
    for header in (64, 108, 124):
        emit(f"header{header}_compression4_truncated_header", "1x1.bmp",
             f"Truncated full DIB {header} precedes compression 4 rejection",
             header=header, compression=4, truncate=54)
    return cases


def observe(data: bytes, operation: str) -> dict:
    """Use a fresh Pillow file for each lifecycle observation."""
    stage = "open"
    try:
        with Image.open(io.BytesIO(data)) as image:
            if operation == "open":
                return {"status": "ok", "format": image.format, "size": list(image.size),
                        "mode": image.mode, "compression": image.info.get("compression")}
            if operation == "verify":
                stage = "verify"
                image.verify()
                return {"status": "ok"}
            stage = "load"
            image.load()
            pixels = image.tobytes()
            return {"status": "ok", "format": image.format, "size": list(image.size),
                    "mode": image.mode, "pixel_bytes": len(pixels),
                    "pixel_sha256": sha256(pixels)}
    except Exception as error:
        return {"status": "error", "stage": stage,
                "class": f"{type(error).__module__}.{type(error).__name__}",
                "message": refs.stable_error_message(error)}


def pending_contract(row: dict) -> dict | None:
    """Repository-defined expected fields, never a claim about Rust execution."""
    outcome = row["pillow_decode"]
    if outcome["status"] != "error" or row["id"].startswith("info_zero_"):
        return None
    message = outcome["message"]
    kind, identity, offset = "unsupported", "bmp_compression_unknown", 30
    if message.startswith("Unsupported BMP pixel depth"):
        identity, offset = "bmp_dib_header", 28
    elif message == "Truncated File Read":
        kind, identity, offset = "malformed", "bmp_dib_header", 14
    elif row["header_size"] == 64 and row["compression"] == 3:
        identity = "bmp_compression_os2_huffman_1d"
    elif row["header_size"] == 64 and row["compression"] == 4:
        identity = "bmp_compression_os2_rle24"
    elif row["header_size"] != 64 and row["compression"] == 4:
        identity = "bmp_compression_jpeg"
    elif row["header_size"] != 64 and row["compression"] == 5:
        identity = "bmp_compression_png"
    return {"id": row["id"], "origin": "defensive_model", "verification": PENDING_RUST,
            "expected_kind": kind, "expected_identity": identity, "expected_offset": offset,
            "operations": ["inspect", "verify", "decode", "decode_sequence"],
            "authority": "Repository structured-error design; Pillow supplies class/message only."}


def build_report(manifest: dict, inputs: list[tuple[dict, bytes]]) -> dict:
    refs.verify_primary_oracle(manifest)
    plugin_sha = sha256(Path(BmpImagePlugin.__file__).read_bytes())
    if plugin_sha != PLUGIN_SHA256:
        raise RuntimeError("pinned BmpImagePlugin source SHA-256 changed")
    rows = []
    for metadata, data in inputs:
        decoded = observe(data, "decode")
        if decoded != observe(data, "decode"):
            raise RuntimeError(f"non-deterministic Pillow decode: {metadata['id']}")
        rows.append({**metadata,
                     "pillow_detection": {"format": "BMP" if BmpImagePlugin._accept(data[:16]) else None},
                     "pillow_open": observe(data, "open"), "pillow_decode": decoded,
                     "pillow_verify": observe(data, "verify")})
    unresolved = [{
        "id": row["id"], "asset": row["asset"], "asset_sha256": row["asset_sha256"],
        "status": "pending", "pillow_error": row["pillow_decode"],
        "source_review": "Rust currently checks malformed dimensions before compression.",
        "rust_verification": PENDING_RUST,
        "required_followup": "Resolve the dimension/compression precedence differential before parity acceptance.",
    } for row in rows if row["id"].startswith("info_zero_")]
    return {
        "format_version": 1,
        "generated_by": "scripts/generate_bmp_compression_fixtures.py",
        "oracle": {**refs.oracle_identity(manifest), "python": "3.12",
                   "plugin_source": "PIL/BmpImagePlugin.py", "plugin_source_sha256": plugin_sha},
        "verification": {"pillow": "independent native observations; decode repeated twice",
                         "rust": PENDING_RUST, "acceptance": "No roadmap finding is closed by this report."},
        "source_authorities": [
            {"scope": "Windows compression 4 (JPEG) and 5 (PNG)",
             "url": "https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-wmf/4e588f70-bd92-4a6f-b77f-35d0feaf7a57"},
            {"scope": "OS/2 BITMAPINFOHEADER2 Huffman/RLE24 header namespace",
             "url": "https://www.bitsavers.org/pdf/ibm/pc/os2/OS2_3.x/G25H-7191-00_OS2_WARP_V3_Presentation_Manager_Programming_Reference_Volume_2_Oct94.pdf"},
        ],
        "summary": {"observations": len(rows), "new_full_files": 53,
                    "existing_controls": 2, "new_successful_controls": 3,
                    "classification_and_control_rows": 49, "unresolved_precedence_rows": 4,
                    "pillow_successes": sum(row["pillow_decode"]["status"] == "ok" for row in rows),
                    "pillow_errors": sum(row["pillow_decode"]["status"] == "error" for row in rows)},
        "pillow_cases": rows,
        "pending_rust_contracts": [contract for row in rows if (contract := pending_contract(row))],
        "unresolved_precedence": unresolved,
    }


def planned_manifest_cases(report: dict) -> list[dict]:
    """Return initial planned declarations for the existing manifest generator."""
    cases = []
    for row in report["pillow_cases"]:
        if row["registration"] == "existing_control":
            continue
        if row["id"].startswith("info_zero_"):
            gap = ("Unresolved: Pillow rejects compression before zero dimensions; Rust currently "
                   "checks dimensions first. Resolve precedence and run final Rust verification.")
        else:
            gap = ("Native Pillow evidence is registered; Rust behavior and exact pixels/errors "
                   "remain unverified until all implementation work is complete.")
        cases.append({"id": row["id"], "description": row["description"],
                      "test_assets": [row["asset"]], "expect_error": row["pillow_decode"]["status"] == "error",
                      "status": "planned", "gap": gap, "pure_rust_work_item": "BMP-006/BMP-017"})
    return cases


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    action = parser.add_mutually_exclusive_group()
    action.add_argument("--write", action="store_true", help="write BMP input assets and native provenance only")
    action.add_argument("--check", action="store_true", help="check reproducibility without writing (default)")
    args = parser.parse_args()
    manifest = yaml.safe_load(refs.MANIFEST.read_text())
    inputs = build_inputs()
    report = build_report(manifest, inputs)
    artifacts = {ASSETS / row["asset"]: data for row, data in inputs
                 if row["registration"] == "new_full_file"}
    artifacts[PROVENANCE] = (json.dumps(report, indent=2) + "\n").encode()
    if args.write:
        for path, data in artifacts.items():
            path.write_bytes(data)
        print(f"Wrote {len(artifacts) - 1} full-file BMP inputs and native provenance; no Rust execution.")
    else:
        stale = [str(path.relative_to(ROOT)) for path, data in artifacts.items()
                 if not path.exists() or path.read_bytes() != data]
        if stale:
            raise RuntimeError("BMP compression artifacts are missing or stale: " + ", ".join(stale))
        print("BMP compression inputs and native provenance reproduce exactly; no Rust execution.")


if __name__ == "__main__":
    main()
