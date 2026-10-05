#!/usr/bin/env python3
"""Generate complete JPEGs with invalid first or later SOS declarations.

The mutation preserves every entropy and tail byte. Pinned live Pillow
establishes metadata/verification success followed by pixel-decode failure,
except the UINT16_MAX declaration, which fails while opening the image.
"""

from __future__ import annotations

import argparse
from dataclasses import dataclass
import hashlib
from io import BytesIO
from pathlib import Path
import struct

import yaml
from PIL import Image

from generate_decode_refs import stable_error_message, verify_primary_oracle
from generate_test_assets import jpeg_scan_segments


ROOT = Path(__file__).resolve().parent.parent
DEFAULT_DIRECTORY = ROOT / "tests" / "fixtures" / "input" / "images" / "jpeg"
SOURCE_PINS = {
    "baseline_default.jpg": (4564, "2230f7e115c61b27bd33518c7f773e5987bad274e310597516c87e0cf5aa10f8"),
    "progressive_spectral.jpg": (3939, "13588c6b896b425d2a0e2798e29702053f9f8a912d0b2b47efb2990d75ad392e"),
    "baseline_444_multiscan_interleaved_restart.jpg": (649, "ddbcf29eb1549aae0356661ff2b879b7b2b23b7aae924adbb424af22246e20d7"),
}


@dataclass(frozen=True)
class Recipe:
    """One bounded declaration mutation and its pinned complete input."""

    name: str
    sha256: str
    length: int
    count: int | None = None
    source: str = "baseline_default.jpg"
    scan_index: int = 0
    open_error: bool = False


RECIPES = (
    Recipe("error_sos_length_0", "729419da5fe9727a0f6d751a6865c55e44bcd95094bf153c06cc7e382ce2e4d0", 0),
    Recipe("error_sos_length_1", "a744fc417d07b52a16f2dd1f3ac0f7e784803c64b88423c2342f07c1d1ff7223", 1),
    Recipe("error_sos_length_2", "066f03bcef810cb346ff6f55ffdb1cbab465b375d1832e894a542d2706062929", 2),
    Recipe("error_sos_length_11", "5f235865d0e748c1f67a12d63ac883a43c73eea66cd255ae44f7e47a1534e960", 11),
    Recipe("error_sos_length_13", "121633f20c3b9b7e4fc33e80ea817fbf6d7e4cab6a6eab2d4ab791cad029ed3d", 13),
    Recipe("error_sos_length_65535", "63c54401762bd11614feb80554c736d1d8c80ba8be003c9d570224deefd47edb", 65535, open_error=True),
    Recipe("error_sos_zero_components_length_6", "538b1e21c0c658ef42b5d053ff6c379c0f7ea9a5f40ee2f53140ae3a4f8754fa", 6, count=0),
    Recipe("error_sos_five_components_length_16", "e192d29496bf30ab3f4ea34e2f8dc5fb3608ad66942475153695dc10b5d2c393", 16, count=5),
    Recipe("error_sos_255_components_length_516", "fffe6aba1f8e5ed5d232db2bc6eded2ddaaf20c4b4b13c11d1577cf6dbd3e4b8", 516, count=255),
    Recipe(
        "error_progressive_second_sos_length_7",
        "eb1f33e9234cb7b8ab4ebabfb199c3eb255107a67b3e9ce468eaf81b07c0298e",
        7,
        source="progressive_spectral.jpg",
        scan_index=1,
    ),
    Recipe(
        "error_baseline_multiscan_second_sos_length_7",
        "d5dd86cf6d57ea40449b16e137de0a9403512c831f5d4bcee6c995ea8ee6c232",
        7,
        source="baseline_444_multiscan_interleaved_restart.jpg",
        scan_index=1,
    ),
)


def metadata(image: Image.Image) -> tuple:
    """Read Pillow's public static-image metadata without loading pixels."""
    return (
        image.format, image.mode, image.size, getattr(image, "n_frames", 1),
        getattr(image, "is_animated", False), dict(image.info),
    )


def observe(data: bytes) -> dict:
    """Perform fresh public opens for inspection, verify and both decodes."""
    results = {}
    for operation in ("inspect", "verify", "decode", "decode_sequence"):
        try:
            image = Image.open(BytesIO(data))
        except Exception as error:
            results[operation] = (
                "error", "open", type(error).__module__, type(error).__name__,
                stable_error_message(error),
            )
            continue
        with image:
            header = metadata(image)
            if operation == "inspect":
                results[operation] = ("ok", header)
                continue
            try:
                if operation == "verify":
                    image.verify()
                else:
                    count = header[3] if operation == "decode_sequence" else 1
                    for index in range(count):
                        image.seek(index)
                        image.load()
                        image.tobytes()
            except Exception as error:
                results[operation] = (
                    "error", "verify" if operation == "verify" else "materialize",
                    type(error).__module__, type(error).__name__, stable_error_message(error),
                )
                continue
            results[operation] = ("ok", header)
    return results


def mutate(source: bytes, recipe: Recipe) -> bytes:
    """Locate the selected scan structurally and change only its declaration."""
    # The shared walker bounds segment extents and skips stuffed FF bytes and
    # restart markers in entropy before finding a subsequent scan header.
    scans, eoi = jpeg_scan_segments(source)
    if eoi + 2 != len(source) or recipe.scan_index >= len(scans):
        raise RuntimeError("SOS source must be complete and contain the selected scan")
    marker = scans[recipe.scan_index][0]
    length_offset = marker + 2
    expected_length, expected_count = (12, 3) if recipe.scan_index == 0 else (8, 1)
    if (
        source[marker:marker + 2] != b"\xff\xda"
        or struct.unpack_from(">H", source, length_offset)[0] != expected_length
        or source[length_offset + 2] != expected_count
    ):
        raise RuntimeError("pinned SOS declaration changed")
    candidate = bytearray(source)
    candidate[length_offset:length_offset + 2] = struct.pack(">H", recipe.length)
    if recipe.count is not None:
        candidate[length_offset + 2] = recipe.count
    allowed = set(range(length_offset, length_offset + 2))
    if recipe.count is not None:
        allowed.add(length_offset + 2)
    if any(a != b and index not in allowed for index, (a, b) in enumerate(zip(source, candidate))):
        raise RuntimeError("SOS mutation changed media or an unrelated field")
    output = bytes(candidate)
    if len(output) != len(source) or hashlib.sha256(output).hexdigest() != recipe.sha256:
        raise RuntimeError(f"complete SOS fixture hash changed: {recipe.name}")
    return output


def generate_fixtures(
    output_dir: Path = DEFAULT_DIRECTORY,
    *,
    source_dir: Path = DEFAULT_DIRECTORY,
    check: bool = False,
) -> tuple[Path, ...]:
    """Validate all eleven live outcomes before publishing or checking inputs."""
    verify_primary_oracle(yaml.safe_load((ROOT / "manifest.yaml").read_text()))
    sources = {}
    source_metadata = {}
    for name, (size, digest) in SOURCE_PINS.items():
        source = (source_dir / name).read_bytes()
        if len(source) != size or hashlib.sha256(source).hexdigest() != digest:
            raise RuntimeError(f"SOS source differs from its pin: {name}")
        outcomes = observe(source)
        if any(outcome[0] != "ok" for outcome in outcomes.values()):
            raise RuntimeError(f"pinned Pillow must completely decode SOS source: {name}")
        header = outcomes["inspect"][1]
        if any(outcome[1] != header for outcome in outcomes.values()):
            raise RuntimeError(f"SOS source lifecycle metadata changed: {name}")
        sources[name] = source
        source_metadata[name] = header

    expected_decode = ("error", "materialize", "builtins", "OSError", "broken data stream when reading image file")
    expected_open = ("error", "open", "builtins", "OSError", "Truncated File Read")
    planned = []
    for recipe in RECIPES:
        data = mutate(sources[recipe.source], recipe)
        outcomes = observe(data)
        if recipe.open_error:
            expected = dict.fromkeys(outcomes, expected_open)
        else:
            success = ("ok", source_metadata[recipe.source])
            expected = {
                "inspect": success, "verify": success,
                "decode": expected_decode, "decode_sequence": expected_decode,
            }
        if outcomes != expected:
            raise RuntimeError(f"pinned Pillow SOS lifecycle changed: {recipe.name}: {outcomes!r}")
        path = output_dir / f"{recipe.name}.jpg"
        if path.exists() and path.read_bytes() != data:
            raise RuntimeError(f"refusing to replace different existing SOS input: {path}")
        if check and not path.exists():
            raise RuntimeError(f"SOS input missing in check mode: {path}")
        planned.append((path, data))

    if not check:
        output_dir.mkdir(parents=True, exist_ok=True)
        for path, data in planned:
            if not path.exists():
                path.write_bytes(data)
    return tuple(path for path, _ in planned)


def main() -> None:
    """Generate inputs or compare existing files without rewriting them."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_DIRECTORY)
    parser.add_argument("--source-dir", type=Path, default=DEFAULT_DIRECTORY)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    for path in generate_fixtures(args.output_dir, source_dir=args.source_dir, check=args.check):
        print(path)


if __name__ == "__main__":
    main()
