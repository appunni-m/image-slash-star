//! ICO codec.

pub mod decode;
pub mod encode;
pub mod inspect;

/// Return Pillow's ICO entry color-depth sort key.
///
/// Pillow uses the directory's bit count when present. For older entries that
/// omit it, it derives a depth from the palette count and treats an absent or
/// one-color count as the maximum depth.
pub(super) fn entry_color_depth(palette_colors: u8, bit_count: u16) -> u32 {
    if bit_count != 0 {
        return u32::from(bit_count);
    }

    let palette_depth = u8::BITS.saturating_sub(palette_colors.saturating_sub(1).leading_zeros());
    if palette_depth == 0 {
        256
    } else {
        palette_depth
    }
}

/// Prefer larger ICO entries, then lower color depths, keeping the first exact
/// tie as Pillow's stable two-pass sort does.
pub(super) fn prefers_entry(
    score: u32,
    color_depth: u32,
    best_score: u32,
    best_color_depth: u32,
) -> bool {
    score > best_score || (score == best_score && color_depth < best_color_depth)
}

/// Match Pillow's CUR selection: replace the current entry only when a later
/// entry has both a strictly larger raw width and a strictly larger raw height.
pub(super) fn prefers_cursor_entry(width: u8, height: u8, best_width: u8, best_height: u8) -> bool {
    width > best_width && height > best_height
}
