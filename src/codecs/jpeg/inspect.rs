//! JPEG frame-header inspection without entropy decoding.

use crate::types::{ImageFormat, ImageInfo, ImageMode};

const SOI: u8 = 0xd8;
const EOI: u8 = 0xd9;
const SOS: u8 = 0xda;
const SOF0: u8 = 0xc0;
const SOF2: u8 = 0xc2;

/// Inspect the first baseline or progressive JPEG frame header.
pub fn inspect(data: &[u8]) -> Option<ImageInfo> {
    if data.get(..2)? != [0xff, SOI] {
        return None;
    }

    let mut position = 2;
    loop {
        let marker = next_marker(data, &mut position)?;
        if marker == EOI || marker == SOS {
            return None;
        }
        if is_standalone(marker) {
            continue;
        }

        let length = usize::from(read_u16(data, position)?);
        position = position.wrapping_add(2);
        if length < 2 {
            continue;
        }
        let payload_length = length.wrapping_sub(2);
        let payload = data.get(position..position.wrapping_add(payload_length))?;
        position = position.wrapping_add(payload_length);
        if marker == SOF0 || marker == SOF2 {
            return inspect_frame(payload);
        }
    }
}

fn inspect_frame(frame: &[u8]) -> Option<ImageInfo> {
    if *frame.first()? != 8 {
        return None;
    }
    let height = u32::from(read_u16(frame, 1)?);
    let width = u32::from(read_u16(frame, 3)?);
    if width == 0 || height == 0 {
        return None;
    }
    let components = *frame.get(5)?;
    let mode = match components {
        1 => ImageMode::L8,
        3 => ImageMode::Rgb8,
        4 => ImageMode::Cmyk8,
        _ => return None,
    };
    frame.get(..6usize.wrapping_add(usize::from(components).wrapping_mul(3)))?;
    Some(ImageInfo {
        format: ImageFormat::Jpeg,
        width,
        height,
        mode,
        bit_depth: 8,
        palette: None,
        is_animated: false,
        frame_count: Some(1),
    })
}

fn next_marker(data: &[u8], position: &mut usize) -> Option<u8> {
    loop {
        let byte = *data.get(*position)?;
        *position = position.wrapping_add(1);
        if byte != 0xff {
            continue;
        }
        let mut marker = *data.get(*position)?;
        while marker == 0xff {
            *position = position.wrapping_add(1);
            marker = *data.get(*position)?;
        }
        *position = position.wrapping_add(1);
        if marker != 0 {
            return Some(marker);
        }
    }
}

fn read_u16(data: &[u8], position: usize) -> Option<u16> {
    let high = *data.get(position)?;
    let low = *data.get(position.wrapping_add(1))?;
    Some(u16::from_be_bytes([high, low]))
}

const fn is_standalone(marker: u8) -> bool {
    matches!(marker, SOI | 0x01 | 0xd0..=0xd7)
}
