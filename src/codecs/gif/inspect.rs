//! GIF container inspection without LZW pixel decoding.

use crate::types::{ImageFormat, ImageInfo, ImageMode, ImagePalette};

const IMAGE_SEPARATOR: u8 = 0x2c;
const EXTENSION_INTRODUCER: u8 = 0x21;
const TRAILER: u8 = 0x3b;

/// Inspect logical-screen, first-frame palette, and sequence metadata.
pub fn inspect(data: &[u8]) -> Option<ImageInfo> {
    let mut input = Input::new(data);
    let signature = input.bytes(6)?;
    if signature != b"GIF87a" && signature != b"GIF89a" {
        return None;
    }
    let logical_width = u32::from(input.u16()?);
    let logical_height = u32::from(input.u16()?);
    let screen_packed = input.u8()?;
    input.skip(2)?;
    let global_palette = read_color_table(&mut input, screen_packed)?;

    let mut transparent_index = None;
    let mut first_palette = None;
    let mut first_mode = None;
    let mut first_bit_depth = 8;
    let mut frame_count = 0u32;
    let mut fallback_width = 0u32;
    let mut fallback_height = 0u32;

    loop {
        match input.u8()? {
            EXTENSION_INTRODUCER => {
                let label = input.u8()?;
                if label == 0xf9 {
                    let _declared_size = input.u8()?;
                    let packed = input.u8()?;
                    input.skip(2)?;
                    let index = input.u8()?;
                    if input.u8()? != 0 {
                        return None;
                    }
                    transparent_index = (packed & 1 != 0).then_some(index);
                } else {
                    input.skip_sub_blocks()?;
                }
            }
            IMAGE_SEPARATOR => {
                let left = u32::from(input.u16()?);
                let top = u32::from(input.u16()?);
                let width = u32::from(input.u16()?);
                let height = u32::from(input.u16()?);
                if width == 0 || height == 0 {
                    return None;
                }
                let image_packed = input.u8()?;
                let local_palette = read_color_table(&mut input, image_packed)?;
                if first_mode.is_none() {
                    let palette = local_palette.as_ref().or(global_palette.as_ref());
                    first_bit_depth = if local_palette.is_some() {
                        (image_packed & 7).wrapping_add(1)
                    } else if global_palette.is_some() {
                        (screen_packed & 7).wrapping_add(1)
                    } else {
                        8
                    };
                    first_mode = Some(if palette.is_some() {
                        ImageMode::P8
                    } else {
                        ImageMode::L8
                    });
                    first_palette = palette.map(|rgb| palette_with_alpha(rgb, transparent_index));
                }
                input.skip(1)?;
                input.skip_sub_blocks()?;
                frame_count = frame_count.wrapping_add(1);
                fallback_width = fallback_width.max(left.wrapping_add(width));
                fallback_height = fallback_height.max(top.wrapping_add(height));
                transparent_index = None;
            }
            TRAILER => break,
            _ => return None,
        }
    }

    Some(ImageInfo {
        format: ImageFormat::Gif,
        width: logical_width.max(fallback_width),
        height: logical_height.max(fallback_height),
        mode: first_mode?,
        bit_depth: first_bit_depth,
        palette: first_palette,
        is_animated: frame_count > 1,
        frame_count: Some(frame_count),
    })
}

fn read_color_table(input: &mut Input<'_>, packed: u8) -> Option<Option<Vec<u8>>> {
    if packed & 0x80 == 0 {
        return Some(None);
    }
    let length = (3usize).wrapping_shl(u32::from((packed & 7).wrapping_add(1)));
    Some(Some(input.bytes(length)?.to_vec()))
}

fn palette_with_alpha(rgb: &[u8], transparent_index: Option<u8>) -> ImagePalette {
    let entries = rgb.len() / 3;
    let mut alpha = Vec::new();
    if let Some(index) = transparent_index
        && usize::from(index) < entries
    {
        alpha = vec![255; entries];
        alpha[usize::from(index)] = 0;
    }
    ImagePalette {
        rgb: rgb.to_vec(),
        alpha,
    }
}

struct Input<'a> {
    data: &'a [u8],
    position: usize,
}

impl<'a> Input<'a> {
    const fn new(data: &'a [u8]) -> Self {
        Self { data, position: 0 }
    }

    fn u8(&mut self) -> Option<u8> {
        let value = *self.data.get(self.position)?;
        self.position = self.position.wrapping_add(1);
        Some(value)
    }

    fn u16(&mut self) -> Option<u16> {
        let bytes = self.bytes(2)?;
        Some(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn bytes(&mut self, length: usize) -> Option<&'a [u8]> {
        let end = self.position.wrapping_add(length);
        let bytes = self.data.get(self.position..end)?;
        self.position = end;
        Some(bytes)
    }

    fn skip(&mut self, length: usize) -> Option<()> {
        self.bytes(length).map(|_| ())
    }

    fn skip_sub_blocks(&mut self) -> Option<()> {
        loop {
            let length = usize::from(self.u8()?);
            if length == 0 {
                return Some(());
            }
            self.skip(length)?;
        }
    }
}
