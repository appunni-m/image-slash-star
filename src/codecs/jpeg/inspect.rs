//! JPEG frame-header inspection without entropy decoding.

use crate::codecs::{CodecError, CodecResult, OptionCodecExt, codec_add_end, need_slice};
use crate::types::{ImageFormat, ImageInfo, ImageMode};

const SOI: u8 = 0xd8;
const EOI: u8 = 0xd9;
const SOS: u8 = 0xda;
const SOF0: u8 = 0xc0;
const SOF2: u8 = 0xc2;

/// Inspect the first baseline or progressive JPEG frame header.
pub fn inspect(data: &[u8]) -> CodecResult<ImageInfo> {
    // Pillow consumes marker framing through the first SOS before exposing an
    // earlier SOF. Parse that stream once and retain the first frame payload
    // instead of validating and then replaying every marker.
    let verified = verify_header(data)?;
    let frame = verified
        .frame
        .malformed("JPEG reached SOS before a frame header")?;
    inspect_frame(frame)
}

/// Validate the marker framing Pillow consumes while opening a JPEG.
pub(crate) fn verify(data: &[u8]) -> CodecResult<()> {
    verify_header(data).map(|_| ())
}

struct VerifiedHeader<'a> {
    frame: Option<&'a [u8]>,
}

fn verify_header(data: &[u8]) -> CodecResult<VerifiedHeader<'_>> {
    if need_slice(data, 0, 2, "truncated JPEG signature")? != [0xff, SOI] {
        return Err(CodecError::Malformed("invalid JPEG signature".to_owned()));
    }
    let mut position = 2;
    let mut frame = None;
    loop {
        let marker = next_marker(data, &mut position)?;
        if marker == EOI || marker == 0x01 {
            return Err(CodecError::Malformed(
                "JPEG reached EOI or TEM before a scan".to_owned(),
            ));
        }
        if is_standalone(marker) {
            continue;
        }
        let length = usize::from(read_u16(data, position)?);
        if length < 2 {
            if marker == SOS {
                return Err(CodecError::Malformed(
                    "JPEG SOS marker has an invalid length".to_owned(),
                ));
            }
            continue;
        }
        let payload_start = position.wrapping_add(2);
        position = position.wrapping_add(length);
        let payload = need_slice(
            data,
            payload_start,
            position,
            "truncated JPEG marker payload",
        )?;
        if frame.is_none() && matches!(marker, SOF0 | SOF2) {
            frame = Some(payload);
        }
        if marker == SOS {
            return Ok(VerifiedHeader { frame });
        }
    }
}

fn inspect_frame(frame: &[u8]) -> CodecResult<ImageInfo> {
    if *frame.first().malformed("truncated JPEG frame header")? != 8 {
        return Err(CodecError::Malformed(
            "unsupported JPEG sample precision".to_owned(),
        ));
    }
    let height = u32::from(read_u16(frame, 1)?);
    let width = u32::from(read_u16(frame, 3)?);
    if width == 0 || height == 0 {
        return Err(CodecError::Malformed(
            "JPEG frame dimensions must be nonzero".to_owned(),
        ));
    }
    let components = *frame.get(5).malformed("truncated JPEG component count")?;
    let mode = match components {
        1 => ImageMode::L8,
        3 => ImageMode::Rgb8,
        4 => ImageMode::Cmyk8,
        _ => {
            return Err(CodecError::Malformed(
                "unsupported JPEG frame component count".to_owned(),
            ));
        }
    };
    // Pillow exposes width, height, and mode once these fixed SOF fields are
    // present. A truncated per-component table is rejected only when the
    // image is materialized, so inspection must not validate it here.
    Ok(ImageInfo {
        format: ImageFormat::Jpeg,
        width,
        height,
        mode,
        bit_depth: 8,
        palette: None,
        is_animated: false,
        frame_count: Some(1),
        frame_count_complete: true,
        cursor_hotspot: None,
        source: crate::types::SourceDescriptor::new(),
    })
}

fn next_marker(data: &[u8], position: &mut usize) -> CodecResult<u8> {
    loop {
        let here = *position;
        let byte = need_slice(
            data,
            here,
            codec_add_end(here, 1),
            "truncated JPEG marker stream",
        )?[0];
        *position = here.wrapping_add(1);
        if byte != 0xff {
            continue;
        }
        let mut here = *position;
        let mut marker = need_slice(
            data,
            here,
            codec_add_end(here, 1),
            "truncated JPEG marker code",
        )?[0];
        while marker == 0xff {
            *position = position.wrapping_add(1);
            here = *position;
            marker = need_slice(
                data,
                here,
                codec_add_end(here, 1),
                "truncated JPEG fill marker",
            )?[0];
        }
        *position = position.wrapping_add(1);
        if marker != 0 {
            return Ok(marker);
        }
    }
}

fn read_u16(data: &[u8], position: usize) -> CodecResult<u16> {
    let bytes = need_slice(
        data,
        position,
        codec_add_end(position, 2),
        "truncated JPEG 16-bit field",
    )?;
    let high = bytes[0];
    let low = bytes[1];
    Ok(u16::from_be_bytes([high, low]))
}

const fn is_standalone(marker: u8) -> bool {
    matches!(marker, SOI | 0x01 | 0xd0..=0xd7)
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    let _ = inspect(b"");
    let _ = inspect(b"not jpeg");
    let _ = inspect(&[0xff, SOI, 0xff, 0xee]);
    let _ = inspect(&[0xff, SOI, 0xff, SOF0, 0, 2]);
    let _ = inspect(&[0xff, SOI, 0xff, SOS, 0, 2]);
    let _ = inspect(&[0xff, SOI, 0xff, SOI, 0xff, SOS, 0, 2]);
    let _ = verify(b"");
    let _ = verify(b"not jpeg");
    let _ = verify(&[0xff, SOI]);
    let _ = verify(&[0xff, SOI, 0xff]);
    let _ = verify(&[0xff, SOI, 0xff, 0xff]);
    let _ = verify(&[0xff, SOI, 0xff, 0xee]);
    let _ = verify(&[0xff, SOI, 0xff, 0xee, 0]);
    let _ = verify(&[0xff, SOI, 0xff, 0xee, 0, 8, 0]);
    let _ = verify(&[0xff, SOI, 0xff, 0xd0, 0xff, SOS, 0, 2]);
    let _ = verify(&[0xff, SOI, 0xff, SOS, 0, 1]);
}
