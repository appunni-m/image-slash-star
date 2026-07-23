# Image Backend Migration Completion Audit

Status: accepted for the native backend migration and lazy-loading correctness
slice. The later JS/WASM core-extra packaging and binding runtime matrices are
separate follow-on work.

This audit maps every requirement in the migration goal to current,
authoritative evidence. A requirement is complete only when its listed
acceptance evidence exists and passes. Compile-only checks are not treated as
runtime parity evidence.

## Requirement Evidence

| Requirement | Current evidence | Audit result |
|---|---|---|
| Reviewed worktree findings, API decision, format/mode model, compatibility impact, and slice criteria are documented | `docs/image-backend-migration-spec.md` and downstream `docs/image-backend-migration-status.md` | complete |
| Auto-detecting decode retains source `ImageFormat` separately from exact pixels | `src/lib.rs::decode`, `src/types/mod.rs::Decoded`, and `DecodedImage::{mode,color,pixels,palette}` | complete |
| Format identity has one owner | `ImageFormat::{as_str,from_name,from_path}` and `detect_format` live in `image-slash-star`; downstream has no detector or format registry | complete |
| Canonical detect/decode/sequence/encode APIs return structured `Result` values without duplicate `try_*` APIs | `src/lib.rs`, `src/types/error.rs`; repository search finds no canonical `try_detect`, `try_decode`, or `try_encode` surface | complete |
| AVIF still and sequence behavior survives the Result migration | AVIF manifest rows and the all-feature coverage suite exercise decode, sequence decode, encode, and sequence encode | complete |
| Encoding keeps target format explicit | `encode`, `encode_sequence`, and `encode_default` all require `ImageFormat`; no ambiguous same-source convenience was added | complete |
| Internal callers and manifests verify detected format, exact mode/pixels, successful output, and structured failures | `tests/coverage_matrix_tests.rs` and `tests/fixtures/coverage_matrix.json` | complete upstream |
| Formatting and 100% line/branch/function/region coverage | Coverage MCP run `dbd8a9cf-1e51-49aa-94b5-476235da7f3a`, snapshot `bc41e67e-4be2-4eac-9444-abe318a0a151`: 28,702/28,702 lines, 3,824/3,824 branches, 1,751/1,751 functions, 45,458/45,458 regions | complete upstream |
| `ImageInfo` has a stable metadata contract | `src/types/mod.rs::ImageInfo` records format, dimensions, mode, bit depth, palette, animation, and frame count | complete |
| Inspection is feature-gated for PNG, JPEG, GIF, BMP, WebP, TIFF, ICO, and AVIF | each codec owns `inspect.rs`; dispatch and feature failures are centralized in `src/codecs/mod.rs` | complete |
| Every inspection slice has exact Pillow-oracle manifest coverage and 100% Coverage MCP results | all eight formats participate in the manifest-driven inspect loop; upstream snapshot above is exact 100% | complete upstream |
| `pillow-rs` explicitly forwards codec features and preserves format/mode/metadata/palette/alpha | downstream `pillow-rs/Cargo.toml`, `pillow-rs/src/image.rs::{LoadedData,PalettedData}`, `make image-backend-test`, and `make image-backend-feature-test` | complete downstream |
| PNG uses the generic backend and the direct PNG dependency is removed | downstream direct decoder is removed, lockfile no longer carries it, and indexed PNG fixtures retain exact RGB palette plus alpha | complete downstream |
| Metadata access is cached, `verify` is non-mutating, and `load` persists materialization while retaining mode/format | canonical `EncodedImage`, downstream shared source/pipeline caches, and lifecycle fixtures cover repeated, cloned, concurrent, and path-replacement access | complete downstream |
| Palette-safe operations are permitted only after individual Pillow proof | all 19 downstream operation rows match exact mode, dimensions, indices, palette, alpha, and PNG bytes before/after load; two `putpixel` rows prove copy-on-write isolation | complete downstream |

## Palette Acceptance Matrix

The downstream operation oracle contains 19 deterministic rows. Every row
requires exact Pillow 12.2.0 mode, dimensions, raw palette indices, palette RGB,
transparency, and encoded PNG bytes before and after persistent `load()`.

Allowed only with those rows:

- crop;
- nearest resize and nearest thumbnail;
- rotation without a custom fill;
- all seven transpose variants;
- `ImageOps.flip`, `ImageOps.mirror`, and `ImageOps.crop`;
- `ImageChops.offset` and direct-copy duplicate;
- nearest affine transform with zero fill;
- direct indexed `putpixel` and crop-then-`putpixel`, with clone isolation.

Not palette-safe without new oracle evidence:

- randomized effect spread;
- mesh transforms and custom rotate/transform fills;
- filters, enhancements, conversions, drawing, lookup tables, point/eval,
  composition, alpha mutation, and unproven pixel mutations.

## Acceptance Evidence

Backend coverage is measured only in the registered `image-slash-star`
Coverage MCP project. Downstream `pillow-rs` is intentionally not registered
there and is validated through its maintained manual targets:

```text
make image-backend-test
make image-backend-feature-test
make fmt
make repo-map-check
```

The migration and reduced-feature targets pass. The Python and JavaScript
binding crates also compile against the new API. A full `make pillow-rs-test`
run passes the core and image suites but still reports three pre-existing
FreeType scalar `getlength` parity mismatches; its 7,632-case pixel matrix has
zero failures. Those font-only mismatches are outside this image migration and
are not hidden as image failures.
