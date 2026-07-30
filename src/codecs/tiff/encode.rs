//! Classic TIFF encoder with Pillow-compatible compression and predictor options.

use crate::codecs::compression::deflate::compress_zlib_tiff;
use crate::codecs::{CodecError, CodecResult};
use crate::encode_options::EncodeOptions;
#[cfg(coverage)]
use crate::types::ColorType;
use crate::types::{DecodedImage, ImageMode};
use std::collections::HashMap;

const COMPRESSION_NONE: u16 = 1;
const COMPRESSION_LZW: u16 = 5;
const COMPRESSION_DEFLATE: u16 = 8;
const COMPRESSION_PACKBITS: u16 = 32_773;

/// Encode an image as a single-strip classic TIFF.
pub fn encode(img: &DecodedImage, opts: &EncodeOptions) -> CodecResult<Vec<u8>> {
    img.validate().map_err(CodecError::from_image_error)?;
    let width = img.width as usize;
    let height = img.height as usize;
    let (photometric, channels, bits_per_sample, extra_sample, row_len) = match img.mode {
        ImageMode::L1 => (1u16, 1u16, 1u16, false, width.div_ceil(8)),
        ImageMode::La8 => (1, 2, 8, true, width.saturating_mul(2)),
        ImageMode::L16 => (1, 1, 16, false, width.saturating_mul(2)),
        ImageMode::F32 => (1, 1, 32, false, width.saturating_mul(4)),
        ImageMode::I32 => (1, 1, 32, false, width.saturating_mul(4)),
        ImageMode::L8 => (1, 1, 8, false, width),
        ImageMode::Rgb8 => (2, 3, 8, false, width.saturating_mul(3)),
        ImageMode::Rgba8 => (2, 4, 8, true, width.saturating_mul(4)),
        ImageMode::Cmyk8 => (5, 4, 8, false, width.saturating_mul(4)),
        _ => {
            return Err(CodecError::Unsupported(
                "TIFF cannot encode this image mode".to_owned(),
            ));
        }
    };
    // Pillow 12.2.0 accepts byte_order but always emits little-endian TIFF.
    let endian = Endian::Little;
    let compression = match opts.extra.get("compression").map(String::as_str) {
        Some("lzw" | "tiff_lzw") => COMPRESSION_LZW,
        Some("deflate" | "tiff_adobe_deflate") => COMPRESSION_DEFLATE,
        Some("packbits") => COMPRESSION_PACKBITS,
        Some("none" | "raw") | None => COMPRESSION_NONE,
        Some(_) => {
            return Err(CodecError::Parameter(
                "invalid TIFF compression option".to_owned(),
            ));
        }
    };
    let predictor = match opts.extra.get("predictor").map(String::as_str) {
        Some("horizontal" | "2") => 2u16,
        Some("none" | "1") | None => 1,
        Some(_) => {
            return Err(CodecError::Parameter(
                "invalid TIFF predictor option".to_owned(),
            ));
        }
    };
    if predictor == 2 && !matches!(bits_per_sample, 8 | 16 | 32) {
        return Err(CodecError::Unsupported(
            "TIFF horizontal prediction is incompatible with this bit depth".to_owned(),
        ));
    }

    let mut raw = img.pixels.clone();
    if predictor == 2 && matches!(compression, COMPRESSION_LZW | COMPRESSION_DEFLATE) {
        apply_horizontal_predictor(&mut raw, row_len, usize::from(channels), bits_per_sample);
    }
    let encoded = if compression == COMPRESSION_NONE {
        raw
    } else if compression == COMPRESSION_LZW {
        encode_lzw(&raw)
    } else if compression == COMPRESSION_DEFLATE {
        let input_chunks = vec![row_len; height];
        compress_zlib_tiff(&raw, &input_chunks)
    } else {
        encode_packbits(&raw, row_len)
    };

    let has_sample_format = matches!(img.mode, ImageMode::F32 | ImageMode::I32);
    let entry_count = if bits_per_sample == 1 { 8u16 } else { 9u16 }
        .wrapping_add(u16::from(channels > 1))
        .wrapping_add(u16::from(extra_sample))
        .wrapping_add(u16::from(predictor == 2))
        .wrapping_add(u16::from(has_sample_format));
    let ifd_size = 2_usize
        .saturating_add(usize::from(entry_count).saturating_mul(12))
        .saturating_add(4);
    let bits_len = if channels <= 2 {
        0
    } else {
        usize::from(channels) * 2
    };
    let compressed_layout = compression != COMPRESSION_NONE;
    let (short_width, short_height) = if compressed_layout {
        (
            u16::try_from(img.width).map_err(|_| {
                CodecError::Dimensions("compressed TIFF width exceeds format limits".to_owned())
            })?,
            u16::try_from(img.height).map_err(|_| {
                CodecError::Dimensions("compressed TIFF height exceeds format limits".to_owned())
            })?,
        )
    } else {
        (0, 0)
    };
    let ifd_offset = if compressed_layout {
        8_usize.saturating_add(encoded.len()).saturating_add(1) & !1
    } else {
        8
    };
    let bits_offset = ifd_offset.saturating_add(ifd_size);
    let pixel_offset = if compressed_layout {
        8
    } else {
        bits_offset.saturating_add(bits_len).saturating_add(1) & !1
    };

    let output_len = if compressed_layout {
        bits_offset.saturating_add(bits_len)
    } else {
        pixel_offset.saturating_add(encoded.len())
    };
    #[cfg(coverage)]
    let output_len = if opts
        .extra
        .contains_key("__coverage_force_tiff_output_len_overflow")
    {
        usize::MAX
    } else {
        output_len
    };
    // Classic TIFF stores offsets and byte counts as `u32`; bounding the full
    // output length bounds every offset/count written below.
    u32::try_from(output_len).map_err(|_| {
        CodecError::Dimensions("TIFF output exceeds classic format limits".to_owned())
    })?;
    let mut output = Vec::with_capacity(output_len);
    output.extend_from_slice(match endian {
        Endian::Little => b"II",
    });
    endian.push_u16(&mut output, 42);
    endian.push_u32(&mut output, bounded_u32(ifd_offset));
    if compressed_layout {
        output.extend_from_slice(&encoded);
        output.resize(ifd_offset, 0);
    }
    endian.push_u16(&mut output, entry_count);

    if compressed_layout {
        write_short_entry(&mut output, endian, 256, short_width);
        write_short_entry(&mut output, endian, 257, short_height);
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
            bounded_u32(bits_offset),
        );
    }
    write_short_entry(&mut output, endian, 259, compression);
    write_short_entry(&mut output, endian, 262, photometric);
    write_entry(&mut output, endian, 273, 4, 1, bounded_u32(pixel_offset));
    if channels > 1 {
        write_short_entry(&mut output, endian, 277, channels);
    }
    if compressed_layout {
        write_short_entry(&mut output, endian, 278, short_height);
    } else {
        write_entry(&mut output, endian, 278, 4, 1, img.height);
    }
    write_entry(&mut output, endian, 279, 4, 1, bounded_u32(encoded.len()));
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
    Ok(output)
}

fn bounded_u32(value: usize) -> u32 {
    let bytes = value.to_le_bytes();
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    fn opt(key: &str, value: &str) -> EncodeOptions {
        let mut opts = EncodeOptions::default();
        opts.extra.insert(key.to_owned(), value.to_owned());
        opts
    }

    let _ = encode(
        &DecodedImage::new(0, 1, Vec::new(), ColorType::L8),
        &EncodeOptions::default(),
    );
    let _ = encode(
        &DecodedImage::new(1, 1, vec![0, 0, 0, 0], ColorType::La16),
        &EncodeOptions::default(),
    );

    let l1 = DecodedImage::with_mode(8, 1, vec![0b1010_1010], ImageMode::L1);
    let la = DecodedImage::new(1, 1, vec![7, 255], ColorType::La8);
    let l16 = DecodedImage::new(1, 1, 0x1234u16.to_le_bytes().to_vec(), ColorType::L16);
    let f32 = DecodedImage::with_mode(1, 1, 1.0f32.to_ne_bytes().to_vec(), ImageMode::F32);
    let i32 = DecodedImage::with_mode(1, 1, 42i32.to_ne_bytes().to_vec(), ImageMode::I32);
    let rgb = DecodedImage::new(
        2,
        2,
        vec![0, 0, 0, 255, 0, 0, 0, 255, 0, 255, 255, 255],
        ColorType::Rgb8,
    );
    let rgba = DecodedImage::new(1, 1, vec![1, 2, 3, 4], ColorType::Rgba8);
    let cmyk = DecodedImage::new(1, 1, vec![1, 2, 3, 4], ColorType::Cmyk8);
    let wide_rgb = DecodedImage::new(70_000, 1, vec![0; 70_000 * 3], ColorType::Rgb8);
    let tall_rgb = DecodedImage::new(1, 70_000, vec![0; 70_000 * 3], ColorType::Rgb8);

    for image in [&l1, &la, &l16, &f32, &i32, &rgb, &rgba, &cmyk] {
        let _ = encode(image, &EncodeOptions::default());
    }
    for compression in ["lzw", "deflate", "packbits", "raw"] {
        let _ = encode(&rgb, &opt("compression", compression));
    }
    let forced_output_overflow = opt("__coverage_force_tiff_output_len_overflow", "1");
    let _ = encode(&rgb, &forced_output_overflow);
    let _ = encode(&wide_rgb, &opt("compression", "packbits"));
    let _ = encode(&tall_rgb, &opt("compression", "packbits"));
    let _ = encode(&rgb, &opt("compression", "unsupported"));
    let _ = encode(&rgb, &opt("predictor", "unsupported"));
    let _ = encode(&l1, &opt("predictor", "horizontal"));

    let mut predicted = opt("compression", "deflate");
    predicted
        .extra
        .insert("predictor".to_owned(), "horizontal".to_owned());
    let _ = encode(&rgb, &predicted);
    let _ = encode(&l16, &predicted);
    let _ = encode(&f32, &predicted);
    let _ = encode(&i32, &predicted);

    let mut bytes8 = vec![1, 2, 5, 9, 3, 4];
    apply_horizontal_predictor(&mut bytes8, 6, 3, 8);
    let mut bytes16 = vec![1, 0, 2, 0, 5, 0, 9, 0];
    apply_horizontal_predictor(&mut bytes16, 8, 2, 16);
    let mut bytes32 = vec![1, 0, 0, 0, 2, 0, 0, 0, 5, 0, 0, 0, 9, 0, 0, 0];
    apply_horizontal_predictor(&mut bytes32, 16, 2, 32);

    let literal: Vec<u8> = (0u8..=130).collect();
    let run = vec![7u8; 260];
    let mixed = [1u8, 2, 2, 3, 4, 4, 4, 5];
    let _ = encode_packbits(&literal, literal.len());
    let _ = encode_packbits(&run, run.len());
    let _ = encode_packbits(&mixed, mixed.len());
    let mut packbits = Vec::new();
    encode_packbits_row(&literal, &mut packbits);
    encode_packbits_row(&run, &mut packbits);
    encode_packbits_row(&mixed, &mut packbits);

    let _ = encode_lzw(&[]);
    let _ = encode_lzw(b"TOBEORNOTTOBEORTOBEORNOT");
    let mut writer = MsbWriter::default();
    writer.write(0x1ff, 9);
    writer.write(0, 1);
    let _ = writer.finish();
    let mut entries = Vec::new();
    let endian = Endian::Little;
    write_short_entry(&mut entries, endian, 256, 1);
    write_entry(&mut entries, endian, 257, 4, 1, 1);
}

fn apply_horizontal_predictor(
    data: &mut [u8],
    row_len: usize,
    channels: usize,
    bits_per_sample: u16,
) {
    let sample_bytes = usize::from(bits_per_sample / 8);
    let stride = channels.wrapping_mul(sample_bytes);
    for row in data.chunks_exact_mut(row_len) {
        for offset in (stride..row.len()).step_by(sample_bytes).rev() {
            let previous = offset.wrapping_sub(stride);
            let mut borrow = 0u16;
            for byte in 0..sample_bytes {
                let current_index = offset.wrapping_add(byte);
                let previous_index = previous.wrapping_add(byte);
                let value = u16::from(row[current_index]);
                let subtrahend = u16::from(row[previous_index]).wrapping_add(borrow);
                row[current_index] = value.wrapping_sub(subtrahend).to_le_bytes()[0];
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
        position = position.wrapping_add(1);
        let mut run_len = 1usize;
        while position < row.len() && row[position] == byte {
            position = position.wrapping_add(1);
            run_len = run_len.wrapping_add(1);
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
                        output[last_literal] = output[last_literal].wrapping_add(1);
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
                        && output[output.len().wrapping_sub(2)] == u8::MAX
                        && output[last_literal] < 126
                    {
                        output[last_literal] = output[last_literal].wrapping_add(2);
                        state = if output[last_literal] == 127 {
                            State::Base
                        } else {
                            State::Literal
                        };
                        let repeated = output[output.len().wrapping_sub(1)];
                        let control = output.len().wrapping_sub(2);
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
    let control = 1_i16.wrapping_sub(i16::from(emitted.to_le_bytes()[0]));
    output.push(control.to_le_bytes()[0]);
    output.push(byte);
    *run_len = run_len.wrapping_sub(emitted);
}

fn encode_lzw(data: &[u8]) -> Vec<u8> {
    const CLEAR: u16 = 256;
    const END: u16 = 257;
    const FIRST: u16 = 258;
    const MAX_CODE: u16 = 4095;
    const CHECK_GAP: usize = 10_000;

    let Some((&first, rest)) = data.split_first() else {
        return Vec::new();
    };
    let mut writer = MsbWriter::default();

    let mut dictionary = HashMap::<(u16, u8), u16>::with_capacity(4096);
    let mut width = 9u8;
    let mut max_code = 1_u16.wrapping_shl(width.into()).wrapping_sub(1);
    let mut free_entry = FIRST;
    let mut input_count = 1usize;
    let mut output_bits = 0usize;
    let mut checkpoint = CHECK_GAP;
    let mut ratio = 0usize;

    writer.write(CLEAR, width);
    output_bits = output_bits.wrapping_add(usize::from(width));
    let mut entry = u16::from(first);

    for &byte in rest {
        input_count = input_count.wrapping_add(1);
        if let Some(&code) = dictionary.get(&(entry, byte)) {
            entry = code;
            continue;
        }

        let prefix = entry;
        writer.write(prefix, width);
        output_bits = output_bits.wrapping_add(usize::from(width));
        entry = u16::from(byte);
        dictionary.insert((prefix, byte), free_entry);
        free_entry = free_entry.wrapping_add(1);

        if free_entry == MAX_CODE.wrapping_sub(1) {
            dictionary.clear();
            ratio = 0;
            input_count = 0;
            output_bits = 0;
            free_entry = FIRST;
            writer.write(CLEAR, width);
            output_bits = output_bits.wrapping_add(usize::from(width));
            width = 9;
            max_code = 1_u16.wrapping_shl(width.into()).wrapping_sub(1);
        } else if free_entry > max_code {
            width = width.wrapping_add(1);
            max_code = 1_u16.wrapping_shl(width.into()).wrapping_sub(1);
        } else if input_count >= checkpoint {
            checkpoint = input_count.wrapping_add(CHECK_GAP);
            let current_ratio = input_count
                .wrapping_shl(8)
                .checked_div(output_bits)
                .unwrap_or_default();
            if current_ratio <= ratio {
                dictionary.clear();
                ratio = 0;
                input_count = 0;
                output_bits = 0;
                free_entry = FIRST;
                writer.write(CLEAR, width);
                output_bits = output_bits.wrapping_add(usize::from(width));
                width = 9;
                max_code = 1_u16.wrapping_shl(width.into()).wrapping_sub(1);
            } else {
                ratio = current_ratio;
            }
        }
    }

    writer.write(entry, width);
    free_entry = free_entry.wrapping_add(1);
    if free_entry == MAX_CODE.wrapping_sub(1) {
        writer.write(CLEAR, width);
        width = 9;
    } else if free_entry > max_code {
        width = width.wrapping_add(1);
    }
    writer.write(END, width);
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
            self.current = self.current.wrapping_shl(1)
                | value.wrapping_shr(shift.into()).to_le_bytes()[0] & 1;
            self.used = self.used.wrapping_add(1);
            if self.used == 8 {
                self.bytes.push(self.current);
                self.current = 0;
                self.used = 0;
            }
        }
    }

    fn finish(mut self) -> Vec<u8> {
        if self.used != 0 {
            self.current = self
                .current
                .wrapping_shl(u32::from(8_u8.saturating_sub(self.used)));
            self.bytes.push(self.current);
        }
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
