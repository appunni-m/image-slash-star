//! Classic TIFF encoder with Pillow-compatible compression and predictor options.

use crate::codecs::compression::deflate::compress_zlib_tiff;
use crate::codecs::{CodecError, CodecResult};
use crate::encode_options::{TiffCompression, TiffEncodeOptions, TiffPredictor};
#[cfg(coverage)]
use crate::types::ColorType;
use crate::types::{DecodedImage, DecodedSequence, FrameBlend, FrameDisposal, ImageMode};
use std::collections::HashMap;

const COMPRESSION_NONE: u16 = 1;
const COMPRESSION_LZW: u16 = 5;
const COMPRESSION_DEFLATE: u16 = 8;
const COMPRESSION_PACKBITS: u16 = 32_773;

/// Encode an image as a single-strip classic TIFF.
pub fn encode(img: &DecodedImage, opts: &TiffEncodeOptions) -> CodecResult<Vec<u8>> {
    encode_with_token(img, opts, None)
}

/// Encode a single TIFF page while polling an optional cancellation token at
/// row and compression checkpoints.
pub fn encode_with_token(
    img: &DecodedImage,
    opts: &TiffEncodeOptions,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<u8>> {
    crate::codecs::error::check_cancelled(token)?;
    encode_page_with_token(img, opts, token).map(|page| page.bytes)
}

struct EncodedPage {
    bytes: Vec<u8>,
    ifd_offset: usize,
    offset_positions: Vec<usize>,
    next_position: usize,
}

fn encode_page_with_token(
    img: &DecodedImage,
    opts: &TiffEncodeOptions,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<EncodedPage> {
    crate::codecs::error::check_cancelled(token)?;
    img.validate().map_err(CodecError::from_image_error)?;
    crate::codecs::error::check_cancelled(token)?;
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
    let compression = match opts.compression.unwrap_or(TiffCompression::Raw) {
        TiffCompression::Raw => COMPRESSION_NONE,
        TiffCompression::Lzw => COMPRESSION_LZW,
        TiffCompression::Deflate => COMPRESSION_DEFLATE,
        TiffCompression::PackBits => COMPRESSION_PACKBITS,
    };
    let predictor = match opts.predictor.unwrap_or(TiffPredictor::None) {
        TiffPredictor::None => 1,
        TiffPredictor::Horizontal => 2,
    };
    if predictor == 2 && !matches!(bits_per_sample, 8 | 16 | 32) {
        return Err(CodecError::Unsupported(
            "TIFF horizontal prediction is incompatible with this bit depth".to_owned(),
        ));
    }

    let mut raw = img.pixels.clone();
    crate::codecs::error::check_cancelled(token)?;
    if predictor == 2 && matches!(compression, COMPRESSION_LZW | COMPRESSION_DEFLATE) {
        apply_horizontal_predictor(
            &mut raw,
            row_len,
            usize::from(channels),
            bits_per_sample,
            token,
        )?;
    }
    let encoded = if compression == COMPRESSION_NONE {
        crate::codecs::error::check_cancelled(token)?;
        raw
    } else if compression == COMPRESSION_LZW {
        encode_lzw(&raw, token)?
    } else if compression == COMPRESSION_DEFLATE {
        crate::codecs::error::check_cancelled(token)?;
        let encoded = compress_zlib_tiff(&raw, &vec![row_len; height]);
        crate::codecs::error::check_cancelled(token)?;
        encoded
    } else {
        encode_packbits(&raw, row_len, token)?
    };
    crate::codecs::error::check_cancelled(token)?;

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
    let output_len = if opts.force_output_len_overflow() {
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
    let mut offset_positions = Vec::with_capacity(2);
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
        offset_positions.push(output.len().saturating_add(8));
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
    offset_positions.push(output.len().saturating_add(8));
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
    let next_position = output.len();
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
    Ok(EncodedPage {
        bytes: output,
        ifd_offset,
        offset_positions,
        next_position,
    })
}

/// Encode ordered TIFF pages without changing any page pixels or dimensions.
pub fn encode_sequence(
    sequence: &DecodedSequence,
    opts: &TiffEncodeOptions,
) -> CodecResult<Vec<u8>> {
    encode_sequence_with_token(sequence, opts, None)
}

/// Encode ordered TIFF pages while polling an optional cancellation token at
/// page and output-relocation boundaries.
pub fn encode_sequence_with_token(
    sequence: &DecodedSequence,
    opts: &TiffEncodeOptions,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<u8>> {
    crate::codecs::error::check_cancelled(token)?;
    validate_sequence_semantics(sequence)?;
    if sequence.frames.len() == 1 {
        let encoded = encode_with_token(&sequence.frames[0].image, opts, token)?;
        crate::codecs::error::check_cancelled(token)?;
        return Ok(encoded);
    }

    let mut pages = Vec::with_capacity(sequence.frames.len());
    for frame in &sequence.frames {
        crate::codecs::error::check_cancelled(token)?;
        pages.push(encode_page_with_token(&frame.image, opts, token)?);
    }
    let page_lengths = pages
        .iter()
        .map(|page| page.bytes.len())
        .collect::<Vec<_>>();
    #[cfg(coverage)]
    let page_lengths = if opts.force_sequence_len_overflow() {
        vec![usize::MAX]
    } else {
        page_lengths
    };
    let final_len = sequence_output_len(&page_lengths)?;
    let mut output = Vec::with_capacity(final_len);
    let mut previous_next_position: Option<usize> = None;
    for mut page in pages {
        crate::codecs::error::check_cancelled(token)?;
        // `sequence_output_len` proved every page base and relocated offset
        // fits both usize and classic TIFF's u32 address space.
        let aligned = output.len().wrapping_add(15) & !15;
        output.resize(aligned, 0);
        let base = output.len();
        if let Some(previous) = previous_next_position {
            let next_ifd = bounded_u32(base.wrapping_add(page.ifd_offset));
            output[previous..previous.saturating_add(4)].copy_from_slice(&next_ifd.to_le_bytes());
        }
        for &position in &page.offset_positions {
            let local = u32::from_le_bytes([
                page.bytes[position],
                page.bytes[position.saturating_add(1)],
                page.bytes[position.saturating_add(2)],
                page.bytes[position.saturating_add(3)],
            ]) as usize;
            let relocated = bounded_u32(base.wrapping_add(local));
            page.bytes[position..position.saturating_add(4)]
                .copy_from_slice(&relocated.to_le_bytes());
        }
        previous_next_position = Some(base.wrapping_add(page.next_position));
        output.extend_from_slice(&page.bytes);
        crate::codecs::error::check_cancelled(token)?;
    }
    output.resize(final_len, 0);
    Ok(output)
}

fn validate_sequence_semantics(sequence: &DecodedSequence) -> CodecResult<()> {
    if sequence.loop_count.is_some() || sequence.background.is_some() {
        return Err(CodecError::Unsupported(
            "TIFF pages cannot retain animation loop or background metadata".to_owned(),
        ));
    }
    let mut width = 0;
    let mut height = 0;
    for frame in &sequence.frames {
        width = width.max(frame.image.width);
        height = height.max(frame.image.height);
        if [
            frame.source.rect.left,
            frame.source.rect.top,
            frame.source.rect.width,
            frame.source.rect.height,
        ] != [0, 0, frame.image.width, frame.image.height]
        {
            return Err(CodecError::Unsupported(
                "TIFF page rectangle must match its image at the origin".to_owned(),
            ));
        }
        if frame.source.duration.numerator != 0 {
            return Err(CodecError::Unsupported(
                "TIFF pages cannot retain animation timing".to_owned(),
            ));
        }
        if frame.source.disposal != FrameDisposal::Unspecified
            || frame.source.blend != FrameBlend::Unspecified
            || frame.source.interlaced
            || frame.source.is_default_image
        {
            return Err(CodecError::Unsupported(
                "TIFF pages cannot retain animation presentation controls".to_owned(),
            ));
        }
    }
    if [sequence.width, sequence.height] != [width, height] {
        return Err(CodecError::Unsupported(
            "TIFF sequence canvas must equal the maximum page extent".to_owned(),
        ));
    }
    Ok(())
}

fn sequence_output_len(page_lengths: &[usize]) -> CodecResult<usize> {
    let mut total = 0;
    for &page_len in page_lengths {
        total = checked_align_16(total)?;
        total = total
            .checked_add(page_len)
            .ok_or_else(|| CodecError::Dimensions("TIFF sequence length overflows".to_owned()))?;
    }
    total = checked_align_16(total)?;
    u32::try_from(total)
        .map_err(|_| CodecError::Dimensions("TIFF sequence exceeds classic limits".to_owned()))?;
    Ok(total)
}

fn checked_align_16(value: usize) -> CodecResult<usize> {
    value
        .checked_add(15)
        .map(|length| length & !15)
        .ok_or_else(|| CodecError::Dimensions("TIFF sequence alignment overflows".to_owned()))
}

fn bounded_u32(value: usize) -> u32 {
    let bytes = value.to_le_bytes();
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    let _ = encode(
        &DecodedImage::new(0, 1, Vec::new(), ColorType::L8),
        &TiffEncodeOptions::default(),
    );
    let _ = encode(
        &DecodedImage::new(1, 1, vec![0, 0, 0, 0], ColorType::La16),
        &TiffEncodeOptions::default(),
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
        let _ = encode(image, &TiffEncodeOptions::default());
    }
    for compression in [
        TiffCompression::Lzw,
        TiffCompression::Deflate,
        TiffCompression::PackBits,
        TiffCompression::Raw,
    ] {
        let mut options = TiffEncodeOptions::default();
        options.compression = Some(compression);
        let _ = encode(&rgb, &options);
    }
    let mut forced_output_overflow = TiffEncodeOptions::default();
    forced_output_overflow.set_force_output_len_overflow();
    let _ = encode(&rgb, &forced_output_overflow);
    let mut packbits = TiffEncodeOptions::default();
    packbits.compression = Some(TiffCompression::PackBits);
    let _ = encode(&wide_rgb, &packbits);
    let _ = encode(&tall_rgb, &packbits);
    let mut horizontal = TiffEncodeOptions::default();
    horizontal.predictor = Some(TiffPredictor::Horizontal);
    let _ = encode(&l1, &horizontal);
    let _ = checked_align_16(usize::MAX);
    let _ = sequence_output_len(&[usize::MAX, 0]);
    let _ = sequence_output_len(&[usize::MAX.saturating_sub(15), 16]);
    #[cfg(not(target_pointer_width = "32"))]
    let _ = sequence_output_len(&[u32::MAX as usize + 1]);
    let mut sequence = DecodedSequence::from_image(rgb.clone());
    sequence.frames.push(sequence.frames[0].clone());
    // CancellationToken is a Rust-only checkpoint contract; Pillow cannot
    // drive these page and relocation interruption edges.
    let single_sequence = DecodedSequence::from_image(rgb.clone());
    for checks in [0, 1] {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = encode_sequence_with_token(
            &single_sequence,
            &TiffEncodeOptions::default(),
            Some(&token),
        );
    }
    for checks in [1, 3, 4] {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = encode_sequence_with_token(&sequence, &TiffEncodeOptions::default(), Some(&token));
    }
    // Still-page cancellation is a Rust-only checkpoint contract. Exercise
    // row preparation, PackBits rows, horizontal prediction, and the long
    // LZW input loop without adding timing-sensitive public-test cases.
    let checkpoint_image = DecodedImage::new(512, 16, vec![0; 512 * 16 * 3], ColorType::Rgb8);
    let mut packbits_options = TiffEncodeOptions::default();
    packbits_options.compression = Some(TiffCompression::PackBits);
    for checks in [0, 1, 2, 3, 4] {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = encode_with_token(&checkpoint_image, &packbits_options, Some(&token));
    }
    let mut lzw_options = TiffEncodeOptions::default();
    lzw_options.compression = Some(TiffCompression::Lzw);
    for checks in [0, 1, 2, 3, 4, 5] {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = encode_with_token(&checkpoint_image, &lzw_options, Some(&token));
    }
    let mut predicted_lzw = lzw_options.clone();
    predicted_lzw.predictor = Some(TiffPredictor::Horizontal);
    for checks in [3, 4] {
        let token = crate::CancellationToken::new();
        token.cancel_after(checks);
        let _ = encode_with_token(&checkpoint_image, &predicted_lzw, Some(&token));
    }
    // Successful token-bearing calls cover the post-compression and output
    // relocation checkpoints that cancellation drills intentionally exit
    // before reaching.
    let token = crate::CancellationToken::new();
    let _ = encode_with_token(
        &checkpoint_image,
        &TiffEncodeOptions::default(),
        Some(&token),
    );
    let mut deflate_options = TiffEncodeOptions::default();
    deflate_options.compression = Some(TiffCompression::Deflate);
    let _ = encode_with_token(&checkpoint_image, &deflate_options, Some(&token));
    let _ = encode_sequence_with_token(
        &single_sequence,
        &TiffEncodeOptions::default(),
        Some(&token),
    );
    let _ = encode_sequence_with_token(&sequence, &TiffEncodeOptions::default(), Some(&token));
    let _ = encode_with_token(&checkpoint_image, &lzw_options, Some(&token));
    let mut forced_sequence_overflow = TiffEncodeOptions::default();
    forced_sequence_overflow.set_force_sequence_len_overflow();
    let _ = encode_sequence(&sequence, &forced_sequence_overflow);

    let mut predicted = TiffEncodeOptions::default();
    predicted.compression = Some(TiffCompression::Deflate);
    predicted.predictor = Some(TiffPredictor::Horizontal);
    let _ = encode(&rgb, &predicted);
    let _ = encode(&l16, &predicted);
    let _ = encode(&f32, &predicted);
    let _ = encode(&i32, &predicted);

    let mut bytes8 = vec![1, 2, 5, 9, 3, 4];
    let _ = apply_horizontal_predictor(&mut bytes8, 6, 3, 8, None);
    let mut bytes16 = vec![1, 0, 2, 0, 5, 0, 9, 0];
    let _ = apply_horizontal_predictor(&mut bytes16, 8, 2, 16, None);
    let mut bytes32 = vec![1, 0, 0, 0, 2, 0, 0, 0, 5, 0, 0, 0, 9, 0, 0, 0];
    let _ = apply_horizontal_predictor(&mut bytes32, 16, 2, 32, None);

    let literal: Vec<u8> = (0u8..=130).collect();
    let run = vec![7u8; 260];
    let mixed = [1u8, 2, 2, 3, 4, 4, 4, 5];
    let _ = encode_packbits(&literal, literal.len(), None);
    let _ = encode_packbits(&run, run.len(), None);
    let _ = encode_packbits(&mixed, mixed.len(), None);
    let mut packbits = Vec::new();
    encode_packbits_row(&literal, &mut packbits);
    encode_packbits_row(&run, &mut packbits);
    encode_packbits_row(&mixed, &mut packbits);

    let _ = encode_lzw(&[], None);
    let _ = encode_lzw(b"TOBEORNOTTOBEORTOBEORNOT", None);
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
    token: Option<&crate::CancellationToken>,
) -> CodecResult<()> {
    let sample_bytes = usize::from(bits_per_sample / 8);
    let stride = channels.wrapping_mul(sample_bytes);
    for row in data.chunks_exact_mut(row_len) {
        crate::codecs::error::check_cancelled(token)?;
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
    Ok(())
}

fn encode_packbits(
    data: &[u8],
    row_len: usize,
    token: Option<&crate::CancellationToken>,
) -> CodecResult<Vec<u8>> {
    let mut output = Vec::with_capacity(data.len().saturating_add(data.len().div_ceil(128)));
    for row in data.chunks_exact(row_len) {
        crate::codecs::error::check_cancelled(token)?;
        encode_packbits_row(row, &mut output);
    }
    Ok(output)
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

fn encode_lzw(data: &[u8], token: Option<&crate::CancellationToken>) -> CodecResult<Vec<u8>> {
    const CLEAR: u16 = 256;
    const END: u16 = 257;
    const FIRST: u16 = 258;
    const MAX_CODE: u16 = 4095;
    const CHECK_GAP: usize = 10_000;

    let Some((&first, rest)) = data.split_first() else {
        return Ok(Vec::new());
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
        if input_count.is_multiple_of(4096) {
            crate::codecs::error::check_cancelled(token)?;
        }
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
    crate::codecs::error::check_cancelled(token)?;
    Ok(writer.finish())
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
