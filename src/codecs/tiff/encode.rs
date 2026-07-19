//! Classic TIFF encoder with selectable byte order, compression, and predictor.

use crate::codecs::compression::deflate::compress_zlib;
use crate::encode_options::EncodeOptions;
use crate::types::{ColorType, DecodedImage};

const COMPRESSION_NONE: u16 = 1;
const COMPRESSION_LZW: u16 = 5;
const COMPRESSION_DEFLATE: u16 = 8;
const COMPRESSION_PACKBITS: u16 = 32_773;

/// Encode an image as a single-strip classic TIFF.
pub fn encode(img: &DecodedImage, opts: &EncodeOptions) -> Option<Vec<u8>> {
    if img.width == 0 || img.height == 0 {
        return None;
    }
    let (photometric, channels, extra_sample) = match img.color {
        ColorType::L8 => (1u16, 1u16, false),
        ColorType::Rgb8 => (2, 3, false),
        ColorType::Rgba8 => (2, 4, true),
        _ => return None,
    };
    let expected = usize::try_from(img.width)
        .ok()?
        .checked_mul(usize::try_from(img.height).ok()?)?
        .checked_mul(usize::from(channels))?;
    if img.pixels.len() != expected {
        return None;
    }

    let endian = match opts.extra.get("byte_order").map(String::as_str) {
        Some("be" | "big" | "MM") => Endian::Big,
        Some("le" | "little" | "II") | None => Endian::Little,
        Some(_) => return None,
    };
    let compression = match opts.extra.get("compression").map(String::as_str) {
        Some("lzw" | "tiff_lzw") => COMPRESSION_LZW,
        Some("deflate" | "tiff_adobe_deflate") => COMPRESSION_DEFLATE,
        Some("packbits") => COMPRESSION_PACKBITS,
        Some("none" | "raw") | None => COMPRESSION_NONE,
        Some(_) => return None,
    };
    let predictor = match opts.extra.get("predictor").map(String::as_str) {
        Some("horizontal" | "2") => 2u16,
        Some("none" | "1") | None => 1,
        Some(_) => return None,
    };

    let mut raw = img.pixels.clone();
    if predictor == 2 && matches!(compression, COMPRESSION_LZW | COMPRESSION_DEFLATE) {
        apply_horizontal_predictor(
            &mut raw,
            usize::try_from(img.width).ok()?,
            usize::from(channels),
        )?;
    }
    let encoded = match compression {
        COMPRESSION_NONE => raw,
        COMPRESSION_LZW => encode_lzw_literals(&raw),
        COMPRESSION_DEFLATE => compress_zlib(&raw, 6)?,
        COMPRESSION_PACKBITS => encode_packbits_literals(&raw),
        _ => return None,
    };

    let entry_count = 9u16
        .checked_add(u16::from(channels > 1))?
        .checked_add(u16::from(extra_sample))?
        .checked_add(u16::from(predictor == 2))?;
    let ifd_size = 2usize
        .checked_add(usize::from(entry_count).checked_mul(12)?)?
        .checked_add(4)?;
    let bits_offset = 8usize.checked_add(ifd_size)?;
    let bits_len = if channels == 1 {
        0
    } else {
        usize::from(channels).checked_mul(2)?
    };
    let pixel_offset = bits_offset.checked_add(bits_len)?.next_multiple_of(2);

    let mut output = Vec::with_capacity(pixel_offset.checked_add(encoded.len())?);
    output.extend_from_slice(match endian {
        Endian::Little => b"II",
        Endian::Big => b"MM",
    });
    endian.push_u16(&mut output, 42);
    endian.push_u32(&mut output, 8);
    endian.push_u16(&mut output, entry_count);

    write_entry(&mut output, endian, 256, 4, 1, img.width);
    write_entry(&mut output, endian, 257, 4, 1, img.height);
    if channels == 1 {
        write_short_entry(&mut output, endian, 258, 8);
    } else {
        write_entry(
            &mut output,
            endian,
            258,
            3,
            u32::from(channels),
            u32::try_from(bits_offset).ok()?,
        );
    }
    write_short_entry(&mut output, endian, 259, compression);
    write_short_entry(&mut output, endian, 262, photometric);
    write_entry(
        &mut output,
        endian,
        273,
        4,
        1,
        u32::try_from(pixel_offset).ok()?,
    );
    if channels > 1 {
        write_short_entry(&mut output, endian, 277, channels);
    }
    write_entry(&mut output, endian, 278, 4, 1, img.height);
    write_entry(
        &mut output,
        endian,
        279,
        4,
        1,
        u32::try_from(encoded.len()).ok()?,
    );
    write_short_entry(&mut output, endian, 284, 1);
    if predictor == 2 {
        write_short_entry(&mut output, endian, 317, predictor);
    }
    if extra_sample {
        write_short_entry(&mut output, endian, 338, 2);
    }
    endian.push_u32(&mut output, 0);

    if channels != 1 {
        for _ in 0..channels {
            endian.push_u16(&mut output, 8);
        }
    }
    output.resize(pixel_offset, 0);
    output.extend_from_slice(&encoded);
    Some(output)
}

fn apply_horizontal_predictor(data: &mut [u8], width: usize, channels: usize) -> Option<()> {
    let row_len = width.checked_mul(channels)?;
    if row_len == 0 || !data.len().is_multiple_of(row_len) {
        return None;
    }
    for row in data.chunks_exact_mut(row_len) {
        for index in (channels..row.len()).rev() {
            row[index] = row[index].wrapping_sub(row[index - channels]);
        }
    }
    Some(())
}

fn encode_packbits_literals(data: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(data.len().saturating_add(data.len().div_ceil(128)));
    for chunk in data.chunks(128) {
        output.push((chunk.len() - 1) as u8);
        output.extend_from_slice(chunk);
    }
    output
}

/// Emit literal-only TIFF LZW packets. Periodic CLEAR codes keep the code
/// width at nine bits, making the stream simple while remaining interoperable.
fn encode_lzw_literals(data: &[u8]) -> Vec<u8> {
    const CLEAR: u16 = 256;
    const END: u16 = 257;
    const LITERALS_PER_DICTIONARY: usize = 200;

    let mut writer = MsbWriter::default();
    if data.is_empty() {
        writer.write(CLEAR, 9);
    } else {
        for chunk in data.chunks(LITERALS_PER_DICTIONARY) {
            writer.write(CLEAR, 9);
            for &byte in chunk {
                writer.write(u16::from(byte), 9);
            }
        }
    }
    writer.write(END, 9);
    writer.finish()
}

#[derive(Default)]
struct MsbWriter {
    bytes: Vec<u8>,
    current: u8,
    used: u8,
}

impl MsbWriter {
    fn write(&mut self, value: u16, width: u8) {
        for shift in (0..width).rev() {
            self.current = (self.current << 1) | ((value >> shift) as u8 & 1);
            self.used += 1;
            if self.used == 8 {
                self.bytes.push(self.current);
                self.current = 0;
                self.used = 0;
            }
        }
    }

    fn finish(mut self) -> Vec<u8> {
        if self.used != 0 {
            self.current <<= 8 - self.used;
            self.bytes.push(self.current);
        }
        self.bytes
    }
}

#[derive(Clone, Copy)]
enum Endian {
    Little,
    Big,
}

impl Endian {
    fn push_u16(self, output: &mut Vec<u8>, value: u16) {
        output.extend_from_slice(&match self {
            Self::Little => value.to_le_bytes(),
            Self::Big => value.to_be_bytes(),
        });
    }

    fn push_u32(self, output: &mut Vec<u8>, value: u32) {
        output.extend_from_slice(&match self {
            Self::Little => value.to_le_bytes(),
            Self::Big => value.to_be_bytes(),
        });
    }
}

fn write_short_entry(output: &mut Vec<u8>, endian: Endian, tag: u16, value: u16) {
    endian.push_u16(output, tag);
    endian.push_u16(output, 3);
    endian.push_u32(output, 1);
    endian.push_u16(output, value);
    endian.push_u16(output, 0);
}

fn write_entry(
    output: &mut Vec<u8>,
    endian: Endian,
    tag: u16,
    field_type: u16,
    count: u32,
    value: u32,
) {
    endian.push_u16(output, tag);
    endian.push_u16(output, field_type);
    endian.push_u32(output, count);
    endian.push_u32(output, value);
}

#[cfg(test)]
mod tests {
    use super::encode;
    use crate::decode::tiff::decode;
    use crate::encode_options::EncodeOptions;
    use crate::types::{ColorType, DecodedImage};

    #[test]
    fn native_tiff_roundtrips_rgba() {
        let image = DecodedImage::new(
            3,
            2,
            (0..24).map(|value| value * 7).collect(),
            ColorType::Rgba8,
        );
        let encoded = encode(&image, &EncodeOptions::default()).expect("TIFF should encode");
        let decoded = decode(&encoded).expect("native TIFF should decode");
        assert_eq!(decoded.pixels, image.pixels);
    }
}
