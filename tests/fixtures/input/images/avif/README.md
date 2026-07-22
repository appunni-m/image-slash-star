# AVIF fixture provenance

These files are fixed inputs for the Pillow 12.2.0 parity manifest. Names are
local semantic aliases; the bytes are unmodified from the listed upstream tag.

| Local file | Upstream file | Tag / commit | SHA-256 | License |
| --- | --- | --- | --- | --- |
| `baseline.avif` | Pillow `Tests/images/avif/hopper.avif` | Pillow 12.2.0 / `3c41c09` | `d4327b7ab11ed8f11d86978258fc04e5505bcfe511ca2c4efa4838c85d226fd2` | MIT-CMU (`third_party/pillow/LICENSE`) |
| `alpha.avif` | Pillow `Tests/images/avif/transparency.avif` | Pillow 12.2.0 / `3c41c09` | `b19f57d9421bbd3d0b0706c8fe79cef802aebf106afefa6bddde9de1a07509c9` | MIT-CMU (`third_party/pillow/LICENSE`) |
| `10bit.avif` | libavif `tests/data/colors-animated-12bpc-keyframes-0-2-3.avif` | libavif 1.4.1 / `6543b22` | `3bf9f91da471749e7df639ba7945d4d94c1c3e3968c26f3619fbbcfc92790576` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `hdr.avif` | libavif `tests/data/colors_hdr_rec2020.avif` | libavif 1.4.1 / `6543b22` | `9980e58ddf718a923f1738c34aad1c72f8e5795ec07e68f1a5f9bd216ca19740` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `grid.avif` | libavif `tests/data/color_grid_alpha_nogrid.avif` | libavif 1.4.1 / `6543b22` | `bae56368b348b1d847e2bfb662522599f0c63dfe62fb68826c9e42a300ff405d` | BSD-2-Clause (`third_party/libavif/LICENSE`) |
| `animated.avif` | libavif `tests/data/colors-animated-8bpc.avif` | libavif 1.4.1 / `6543b22` | `2f8683d21725261f37f86e115f0c212cc52d0fefd3a2ddfcc4fa648c1859906d` | BSD-2-Clause (`third_party/libavif/LICENSE`) |

The upstream libavif test-data README identifies the copied libavif files as
covered by libavif's own license. `10bit.avif` retains its historical manifest
name but is a 12-bit high-bit-depth animation; the manifest description states
the exact depth.
