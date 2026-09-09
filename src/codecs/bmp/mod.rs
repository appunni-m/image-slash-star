//! BMP codec.

use crate::codecs::{CodecError, CodecResult, need_slice};

pub mod decode;
pub mod encode;
pub mod inspect;

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

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    decode::__coverage_exercise_private_branches();
    encode::__coverage_exercise_private_branches();
    inspect::__coverage_exercise_private_branches();
}
