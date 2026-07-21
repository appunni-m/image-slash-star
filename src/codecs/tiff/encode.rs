//! Classic TIFF encoder with Pillow-compatible compression and predictor options.

use crate::codecs::compression::deflate::compress_zlib_tiff;
use crate::encode_options::EncodeOptions;
use crate::types::{ColorType, DecodedImage, ImageMode};
use std::collections::HashMap;

const COMPRESSION_NONE: u16 = 1;
const COMPRESSION_LZW: u16 = 5;
const COMPRESSION_DEFLATE: u16 = 8;
const COMPRESSION_PACKBITS: u16 = 32_773;

/// Encode an image as a single-strip classic TIFF.
pub fn encode(img: &DecodedImage, opts: &EncodeOptions) -> Option<Vec<u8>> {
    img.validate().ok()?;
    let width = usize::try_from(img.width).ok()?;
    let height = usize::try_from(img.height).ok()?;
    let (photometric, channels, bits_per_sample, extra_sample, row_len) = match img.mode {
        ImageMode::L1 => (1u16, 1u16, 1u16, false, width.div_ceil(8)),
        ImageMode::La8 => (1, 2, 8, true, width.checked_mul(2)?),
        ImageMode::L16 => (1, 1, 16, false, width.checked_mul(2)?),
        ImageMode::F32 => (1, 1, 32, false, width.checked_mul(4)?),
        ImageMode::I32 => (1, 1, 32, false, width.checked_mul(4)?),
        _ => match img.color {
            ColorType::L8 => (1, 1, 8, false, width),
            ColorType::Rgb8 => (2, 3, 8, false, width.checked_mul(3)?),
            ColorType::Rgba8 => (2, 4, 8, true, width.checked_mul(4)?),
            ColorType::Cmyk8 => (5, 4, 8, false, width.checked_mul(4)?),
            _ => return None,
        },
    };
    // Pillow 12.2.0 accepts byte_order but always emits little-endian TIFF.
    let endian = Endian::Little;
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
    if predictor == 2 && !matches!(bits_per_sample, 8 | 16 | 32) {
        return None;
    }

    let mut raw = img.pixels.clone();
    if predictor == 2 && matches!(compression, COMPRESSION_LZW | COMPRESSION_DEFLATE) {
        apply_horizontal_predictor(&mut raw, row_len, usize::from(channels), bits_per_sample);
    }
    let encoded = if compression == COMPRESSION_NONE {
        raw
    } else if compression == COMPRESSION_LZW {
        encode_lzw(&raw)?
    } else if compression == COMPRESSION_DEFLATE {
        let input_chunks = vec![row_len; height];
        compress_zlib_tiff(&raw, &input_chunks)?
    } else {
        encode_packbits(&raw, row_len)
    };

    let has_sample_format = matches!(img.mode, ImageMode::F32 | ImageMode::I32);
    let entry_count = if bits_per_sample == 1 { 8u16 } else { 9u16 }
        .checked_add(u16::from(channels > 1))?
        .checked_add(u16::from(extra_sample))?
        .checked_add(u16::from(predictor == 2))?
        .checked_add(u16::from(has_sample_format))?;
    let ifd_size = 2usize
        .checked_add(usize::from(entry_count).checked_mul(12)?)?
        .checked_add(4)?;
    let bits_len = if channels <= 2 {
        0
    } else {
        usize::from(channels).checked_mul(2)?
    };
    let compressed_layout = compression != COMPRESSION_NONE;
    let ifd_offset = if compressed_layout {
        8usize.checked_add(encoded.len())?.next_multiple_of(2)
    } else {
        8
    };
    let bits_offset = ifd_offset.checked_add(ifd_size)?;
    let pixel_offset = if compressed_layout {
        8
    } else {
        bits_offset.checked_add(bits_len)?.next_multiple_of(2)
    };

    let output_len = if compressed_layout {
        bits_offset.checked_add(bits_len)?
    } else {
        pixel_offset.checked_add(encoded.len())?
    };
    let mut output = Vec::with_capacity(output_len);
    output.extend_from_slice(match endian {
        Endian::Little => b"II",
    });
    endian.push_u16(&mut output, 42);
    endian.push_u32(&mut output, u32::try_from(ifd_offset).ok()?);
    if compressed_layout {
        output.extend_from_slice(&encoded);
        output.resize(ifd_offset, 0);
    }
    endian.push_u16(&mut output, entry_count);

    if compressed_layout {
        write_short_entry(&mut output, endian, 256, u16::try_from(img.width).ok()?);
        write_short_entry(&mut output, endian, 257, u16::try_from(img.height).ok()?);
    } else {
        write_entry(&mut output, endian, 256, 4, 1, img.width);
        write_entry(&mut output, endian, 257, 4, 1, img.height);
    }
    if bits_per_sample == 1 {
        // Pillow leaves the default BitsPerSample=1 implicit for bilevel TIFF.
    } else if channels == 1 {
        write_short_entry(&mut output, endian, 258, bits_per_sample);
    } else if channels == 2 {
        write_entry(
            &mut output,
            endian,
            258,
            3,
            2,
            u32::from(bits_per_sample) | (u32::from(bits_per_sample) << 16),
        );
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
    if compressed_layout {
        write_short_entry(&mut output, endian, 278, u16::try_from(img.height).ok()?);
    } else {
        write_entry(&mut output, endian, 278, 4, 1, img.height);
    }
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
    match img.mode {
        ImageMode::F32 => write_short_entry(&mut output, endian, 339, 3),
        ImageMode::I32 => write_short_entry(&mut output, endian, 339, 2),
        _ => {}
    }
    endian.push_u32(&mut output, 0);

    if channels > 2 {
        for _ in 0..channels {
            endian.push_u16(&mut output, bits_per_sample);
        }
    }
    if !compressed_layout {
        output.resize(pixel_offset, 0);
        output.extend_from_slice(&encoded);
    }
    Some(output)
}

fn apply_horizontal_predictor(
    data: &mut [u8],
    row_len: usize,
    channels: usize,
    bits_per_sample: u16,
) {
    let sample_bytes = usize::from(bits_per_sample / 8);
    let stride = channels * sample_bytes;
    for row in data.chunks_exact_mut(row_len) {
        for offset in (stride..row.len()).step_by(sample_bytes).rev() {
            let previous = offset - stride;
            let mut borrow = 0u16;
            for byte in 0..sample_bytes {
                let value = u16::from(row[offset + byte]);
                let subtrahend = u16::from(row[previous + byte]) + borrow;
                row[offset + byte] = value.wrapping_sub(subtrahend) as u8;
                borrow = u16::from(value < subtrahend);
            }
        }
    }
}

fn encode_packbits(data: &[u8], row_len: usize) -> Vec<u8> {
    let mut output = Vec::with_capacity(data.len().saturating_add(data.len().div_ceil(128)));
    for row in data.chunks_exact(row_len) {
        encode_packbits_row(row, &mut output);
    }
    output
}

fn encode_packbits_row(row: &[u8], output: &mut Vec<u8>) {
    #[derive(Clone, Copy)]
    enum State {
        Base,
        Literal,
        Run,
        LiteralRun,
    }

    let mut state = State::Base;
    let mut last_literal = 0usize;
    let mut position = 0usize;
    while position < row.len() {
        let byte = row[position];
        position += 1;
        let mut run_len = 1usize;
        while position < row.len() && row[position] == byte {
            position += 1;
            run_len += 1;
        }

        loop {
            let mut again = false;
            match state {
                State::Base => {
                    if run_len > 1 {
                        state = State::Run;
                        again = run_len > 128;
                        emit_packbits_run(output, byte, &mut run_len);
                    } else {
                        last_literal = output.len();
                        output.extend_from_slice(&[0, byte]);
                        state = State::Literal;
                    }
                }
                State::Literal => {
                    if run_len > 1 {
                        state = State::LiteralRun;
                        again = run_len > 128;
                        emit_packbits_run(output, byte, &mut run_len);
                    } else {
                        output[last_literal] += 1;
                        if output[last_literal] == 127 {
                            state = State::Base;
                        }
                        output.push(byte);
                    }
                }
                State::Run => {
                    if run_len > 1 {
                        again = run_len > 128;
                        emit_packbits_run(output, byte, &mut run_len);
                    } else {
                        last_literal = output.len();
                        output.extend_from_slice(&[0, byte]);
                        state = State::Literal;
                    }
                }
                State::LiteralRun => {
                    if run_len == 1
                        && output[output.len() - 2] == u8::MAX
                        && output[last_literal] < 126
                    {
                        output[last_literal] += 2;
                        state = if output[last_literal] == 127 {
                            State::Base
                        } else {
                            State::Literal
                        };
                        let repeated = output[output.len() - 1];
                        let control = output.len() - 2;
                        output[control] = repeated;
                    } else {
                        state = State::Run;
                    }
                    continue;
                }
            }

            if !again {
                break;
            }
        }
    }
}

fn emit_packbits_run(output: &mut Vec<u8>, byte: u8, run_len: &mut usize) {
    let emitted = (*run_len).min(128);
    output.push((1i16 - emitted as i16) as i8 as u8);
    output.push(byte);
    *run_len -= emitted;
}

fn encode_lzw(data: &[u8]) -> Option<Vec<u8>> {
    const CLEAR: u16 = 256;
    const END: u16 = 257;
    const FIRST: u16 = 258;
    const MAX_CODE: u16 = 4095;
    const CHECK_GAP: usize = 10_000;

    let (&first, rest) = data.split_first()?;
    let mut writer = MsbWriter::default();

    let mut dictionary = HashMap::<(u16, u8), u16>::with_capacity(4096);
    let mut width = 9u8;
    let mut max_code = (1u16 << width) - 1;
    let mut free_entry = FIRST;
    let mut input_count = 1usize;
    let mut output_bits = 0usize;
    let mut checkpoint = CHECK_GAP;
    let mut ratio = 0usize;

    writer.write(CLEAR, width);
    output_bits += usize::from(width);
    let mut entry = u16::from(first);

    for &byte in rest {
        input_count += 1;
        if let Some(&code) = dictionary.get(&(entry, byte)) {
            entry = code;
            continue;
        }

        let prefix = entry;
        writer.write(prefix, width);
        output_bits += usize::from(width);
        entry = u16::from(byte);
        dictionary.insert((prefix, byte), free_entry);
        free_entry += 1;

        if free_entry == MAX_CODE - 1 {
            dictionary.clear();
            ratio = 0;
            input_count = 0;
            output_bits = 0;
            free_entry = FIRST;
            writer.write(CLEAR, width);
            output_bits += usize::from(width);
            width = 9;
            max_code = (1u16 << width) - 1;
        } else if free_entry > max_code {
            width += 1;
            max_code = (1u16 << width) - 1;
        } else if input_count >= checkpoint {
            checkpoint = input_count + CHECK_GAP;
            let current_ratio = (input_count << 8) / output_bits;
            if current_ratio <= ratio {
                dictionary.clear();
                ratio = 0;
                input_count = 0;
                output_bits = 0;
                free_entry = FIRST;
                writer.write(CLEAR, width);
                output_bits += usize::from(width);
                width = 9;
                max_code = (1u16 << width) - 1;
            } else {
                ratio = current_ratio;
            }
        }
    }

    writer.write(entry, width);
    free_entry += 1;
    if free_entry == MAX_CODE - 1 {
        writer.write(CLEAR, width);
        width = 9;
    } else if free_entry > max_code {
        width += 1;
    }
    writer.write(END, width);
    Some(writer.finish())
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
        if self.used != 0 { self.current <<= 8 - self.used; self.bytes.push(self.current); }
        self.bytes
    }
}

#[derive(Clone, Copy)]
enum Endian {
    Little,
}

impl Endian {
    fn push_u16(self, output: &mut Vec<u8>, value: u16) {
        output.extend_from_slice(&match self {
            Self::Little => value.to_le_bytes(),
        });
    }

    fn push_u32(self, output: &mut Vec<u8>, value: u32) {
        output.extend_from_slice(&match self {
            Self::Little => value.to_le_bytes(),
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
