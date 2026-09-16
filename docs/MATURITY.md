# Maturity and compatibility

**Version 0.1.2 is an early release.** The goal is exact observable codec
behavior for maintained Pillow cases, with safe Rust runtime implementation.
The goal is broader than the currently measured subset.

| Label | Interpretation |
| --- | --- |
| Active fixture | Executable selected input; read the operation outcomes for evidence |
| Declared capability | Runtime table exposes the operation for that feature/target |
| Partial | Specific format states or API behavior are supported |
| Planned | No supported application path or completed comparison is claimed |
| Not applicable | The operation does not apply to that row; not a passed test |
| Unmeasured | No qualifying executed evidence for the requested combination |

The [generated capability tables](capabilities.md) preserve the runtime and
fixture denominators. Active fixture observations are native/all-features.
The runtime capability tables additionally describe native and WASI feature
lanes; those declarations are not substituted for exact pixel evidence.

## Supported and incomplete paths

Default JPEG, PNG, GIF, BMP, TIFF, WebP, and ICO/CUR paths cover selected
detection, inspection, decode, encode, and container behavior. The supported
mode/option/sequence combinations vary. Read the exact operation table instead
of treating a format name as a promise of complete specification coverage.

AVIF remains opt-in and partial: the maintained ledger has 340 active and
three planned decode/inspect/verify rows; all 32 encoder rows are planned.
High-bit-depth, HDR, and animation contain partial implementation work but
remain planned in the registered application evidence. See [AVIF](avif.md).

General image editing, system integration, font handling, and native codec
fallbacks are outside scope. There is no npm or PyPI package. Browser WASM
compilation does not imply a ready JavaScript facade.

## Versioned evidence

[Release 0.1.2](https://github.com/appunni-m/image-slash-star/actions/runs/35010129246)
and [main CI](https://github.com/appunni-m/image-slash-star/actions/runs/35007807946)
passed at `70190214a0711223302c76ab58e76c097288d80b`.
The [evidence guide](EVIDENCE.md) distinguishes that release from older
claim-ledger and coverage snapshots. Do not relabel either as a fresh
measurement after editing documentation.

The [public roadmap](roadmap-new.md) preserves the ledger's 244 finding rows at its recorded review.
A package release does not close those findings or establish hardened handling
of arbitrary hostile input.
