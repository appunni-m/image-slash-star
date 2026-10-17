//! BMP codec.

use crate::codecs::{CodecError, CodecResult, need_slice};

pub mod decode;
pub mod encode;
pub mod inspect;

// Match Pillow's recognized Windows INFO-family variants instead of accepting
// arbitrary extension lengths as a known header layout.
pub(super) const fn is_supported_info_dib_header_size(header_size: u32) -> bool {
    matches!(header_size, 40 | 52 | 56 | 64 | 108 | 124)
}

#[derive(Clone, Copy)]
pub(super) struct BitfieldMasks {
    pub(super) red: u32,
    pub(super) green: u32,
    pub(super) blue: u32,
    pub(super) alpha: u32,
}

/// Validate the BI_BITFIELDS layouts accepted by Pillow 12.2.0.
pub(super) fn normalize_bitfield_masks(
    bit_depth: u16,
    masks: BitfieldMasks,
) -> CodecResult<BitfieldMasks> {
    let rgb = (masks.red, masks.green, masks.blue);
    let supported = match bit_depth {
        16 => matches!(rgb, (0xF800, 0x07E0, 0x001F) | (0x7C00, 0x03E0, 0x001F)),
        24 => rgb == (0x00FF_0000, 0x0000_FF00, 0x0000_00FF),
        32 => matches!(
            (masks.red, masks.green, masks.blue, masks.alpha),
            (0x00FF_0000, 0x0000_FF00, 0x0000_00FF, 0)
                | (0xFF00_0000, 0x00FF_0000, 0x0000_FF00, 0)
                | (0xFF00_0000, 0x0000_FF00, 0x0000_00FF, 0)
                | (0xFF00_0000, 0x00FF_0000, 0x0000_FF00, 0x0000_00FF)
                | (0x0000_00FF, 0x0000_FF00, 0x00FF_0000, 0xFF00_0000)
                | (0x00FF_0000, 0x0000_FF00, 0x0000_00FF, 0xFF00_0000)
                | (0xFF00_0000, 0x0000_FF00, 0x0000_00FF, 0x00FF_0000)
                | (0, 0, 0, 0)
        ),
        _ => false,
    };
    if !supported {
        return Err(
            CodecError::Unsupported("unsupported BMP bitfields layout".to_owned())
                .at(54, "bmp_bitfields"),
        );
    }

    if bit_depth == 32 && rgb == (0, 0, 0) {
        // Pillow treats the all-zero 32-bit tuple as byte-ordered BGRA.
        // The supported-layout check above also guarantees a zero alpha mask.
        return Ok(BitfieldMasks {
            red: 0x00FF_0000,
            green: 0x0000_FF00,
            blue: 0x0000_00FF,
            alpha: 0xFF00_0000,
        });
    }

    Ok(BitfieldMasks {
        alpha: if bit_depth == 32 { masks.alpha } else { 0 },
        ..masks
    })
}

/// Reject compressed payload families before either caller reads a palette or
/// treats the payload as raw scanlines. Both callers validate pixel depth first.
fn validate_compression(
    data: &[u8],
    header_size: u32,
    bit_depth: u16,
    compression: u32,
) -> CodecResult<()> {
    // Pillow 12.2.0 BmpImagePlugin._bitmap rejects codes outside 0..=3 before
    // reading the palette. Its 64-byte OS/2 path nevertheless accepts the
    // Windows-style bitfield layouts at 16/24/32 bits; keep those exact paths.
    // Indexed OS/2 code 3 cannot be a supported bitfield layout, including the
    // normative one-bit Huffman case and invalid four/eight-bit variants.
    let (label, identity) = match (header_size, compression, bit_depth) {
        (64, 3, 1 | 4 | 8) => ("OS/2 Huffman 1D", "bmp_compression_os2_huffman_1d"),
        (_, 0..=3, _) => return Ok(()),
        (64, 4, _) => ("OS/2 RLE24", "bmp_compression_os2_rle24"),
        (40 | 52 | 56 | 108 | 124, 4, _) => ("embedded JPEG", "bmp_compression_jpeg"),
        (40 | 52 | 56 | 108 | 124, 5, _) => ("embedded PNG", "bmp_compression_png"),
        // OS/2 defines no PNG code. Unknown header identities do not establish
        // a Windows compression namespace either.
        _ => ("unknown compression", "bmp_compression_unknown"),
    };

    // Pillow reads the complete declared DIB before selecting compression.
    // Check only the new rejection paths so established raw/RLE/bitfield
    // parsing and its existing truncation precedence remain unchanged.
    let header_end = usize::try_from(header_size)
        .ok()
        .and_then(|size| 14_usize.checked_add(size))
        .ok_or_else(|| {
            CodecError::Malformed("BMP DIB header extent exceeds usize".to_owned())
                .at(14, "bmp_dib_header")
        })?;
    need_slice(data, 14, header_end, "truncated BMP DIB header")
        .map_err(|error| error.at(14, "bmp_dib_header"))?;
    Err(CodecError::Unsupported(format!(
        "unsupported BMP {label} (compression {compression}, DIB header {header_size})"
    ))
    .at(30, identity))
}
