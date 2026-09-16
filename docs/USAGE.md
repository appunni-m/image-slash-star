# API usage

<!-- release:summary -->
**Latest release: [0.1.3](https://github.com/appunni-m/image-slash-star/releases/tag/v0.1.3).**
<!-- /release:summary -->

The library handles codecs only. Applications provide complete byte slices or
use the documented partial-input and sequence interfaces. Filesystem paths,
network requests, and image editing belong outside the codec layer.

## Install

<!-- release:cargo -->
```toml
[dependencies]
image-slash-star = "=0.1.3"
```
<!-- /release:cargo -->

Requires Rust 1.96.1 or newer.

## Select the operation

| Task | Public entry | Meaning |
| --- | --- | --- |
| Recognize a signature | `detect_format` | Format recognition, not complete-file validation |
| Inspect a complete input | `inspect` | Structural metadata under the format's inspection contract |
| Decode a still image | `decode` | Auto-detect and return format plus decoded content |
| Decode a known candidate | `decode_with_format` | Validate the complete signature against the supplied format |
| Encode with defaults | `encode_default` | Explicit output format, codec-default options |
| Encode with options | `encode` | Format-specific supported options |
| Apply limits | Policy variants | Enforce documented admission and work/result limits |

<!-- release:rust-api -->
[Rust API reference](https://docs.rs/image-slash-star/0.1.3/image_slash_star/).
<!-- /release:rust-api -->

The reference defines exact argument types, returned structures, and errors.
[Capabilities](capabilities.md) separates still decode, sequence decode, still
encode, sequence encode, feature lanes, modes, and target evidence.

## Pixels and ownership

`DecodedImage` owns a pixel buffer and retains mode, palette, source facts,
and metadata where supported. Use `try_new` or `try_with_mode` to validate
caller-built content. Unchecked constructors and direct struct literals do not
prove valid dimensions or buffer length; encoders validate their inputs.

Do not reinterpret indexed or packed pixels as RGBA. Some malformed indexed inputs can have no palette. Preserve
metadata and sequence semantics only where the selected codec operation
documents them.

The [README example](../README.md#encode-and-decode-a-png) requires no external
files.

## Error recovery

Match typed `ImageError` categories, stages, and reasons. A malformed stream,
unsupported mode, disabled feature, and resource-limit rejection are different
outcomes. Human-readable diagnostics may change; do not parse or compare their
wording as a stable protocol.

Detection only recognizes a signature. Inspection and verification have
format-specific depths. A valid container header is not proof of decoded pixel
correctness or complete compressed-payload validation.

## Resource policy

Decode limits cover the fields documented by `DecodePolicy`, including encoded
input, inspected dimensions/results, frame/sequence bounds, and cooperative
work. `EncodePolicy::with_max_output_bytes` limits the complete admitted output
before return or delivery. Default policies are unlimited for compatibility.

An output cap does not cap all intermediate allocations. Cooperative checkpoints
are deterministic work units, not milliseconds. Cancellation and work limits
apply at implemented checkpoints; they do not guarantee immediate interruption
inside every codec stage. No recoverable out-of-memory contract is claimed.

## Platform and feature integration

Disable unused codec features explicitly. ICO enables BMP and PNG because its
payloads use them. The library has no native codec runtime dependency.
`wasm32-unknown-unknown` compilation and WASI execution are different evidence
lanes; do not infer a JavaScript application API or full browser parity from
a successful cross-compile.

For AVIF, use the [dedicated scope guide](avif.md). Unknown or unsupported AV1
states remain errors; there is no native fallback.
