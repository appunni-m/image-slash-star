# Supported formats and limitations

<!-- release:summary -->
**Latest release: [0.1.3](https://github.com/appunni-m/image-slash-star/releases/tag/v0.1.3).**
<!-- /release:summary -->

image-slash-star is an early codec library. Support depends on the format,
pixel mode, operation, and enabled Cargo features. It does not implement every
part of every image-format specification.

| Area | Available | Limits |
| --- | --- | --- |
| JPEG, PNG, GIF, BMP, TIFF, WebP | Selected decode, encode, detection, and inspection paths | Modes, options, and metadata vary by codec |
| ICO/CUR | Selected container and embedded PNG/BMP paths | Requires the `ico` feature, which enables PNG and BMP |
| Multi-frame images | Selected sequence operations | Check frame, timing, disposal, and encoding support for the chosen format |
| AVIF | Opt-in decoding and inspection, including selected high-depth, HDR and animated files | No encoder; supported syntax and layouts are limited to tested paths |
| Resource policies | Input, output, dimension, sequence, and cooperative work limits | Defaults unlimited; no guarantee covering every allocation or immediate cancellation |
| Image editing | Not provided | Resize, rotate, crop, drawing, and filtering belong in a separate library |
| JavaScript and Python packages | Not provided | This project distributes a Rust crate |

The high-depth, HDR and animated AVIF additions are validated on `main` and
await the next release; version 0.1.3 retains its earlier partial AVIF scope.

The default codec features are `jpeg`, `png`, `gif`, `bmp`, `tiff`, `webp`, and
`ico`. AVIF must be enabled explicitly. There is no native codec fallback.

Start with [API usage](USAGE.md). If you need a particular mode or sequence
operation, consult the [detailed codec reference](capabilities.md) and
[AVIF scope](avif.md). These references include incomplete paths rather than
assuming that a format name means every operation is supported.

Try representative application inputs before adopting the crate. It is not
advertised as hardened for arbitrary hostile files. Contributor measurements
and implementation progress are recorded in [evidence](EVIDENCE.md) and the
[roadmap](roadmap-new.md).
