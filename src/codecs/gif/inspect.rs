//! GIF container inspection without LZW pixel decoding.

use crate::codecs::{CodecError, CodecResult, OptionCodecExt, codec_add_end, need_slice};
use crate::types::{ImageFormat, ImageInfo, ImageMode, ImagePalette};

const IMAGE_SEPARATOR: u8 = 0x2c;
const EXTENSION_INTRODUCER: u8 = 0x21;
const TRAILER: u8 = 0x3b;
const PILLOW_DECOMPRESSION_BOMB_ERROR_PIXELS: u64 = 178_956_970;

/// Inspect logical-screen, first-frame palette, and sequence metadata.
pub fn inspect(data: &[u8]) -> CodecResult<ImageInfo> {
    inspect_inner(data, false)
}

/// Inspect only the logical screen and first proven image.
///
/// Frame counting stops after the first image descriptor; the result reports
/// `frame_count_complete` only when the next byte is the trailer.
pub fn inspect_basic(data: &[u8]) -> CodecResult<ImageInfo> {
    inspect_inner(data, true)
}

fn inspect_inner(data: &[u8], basic: bool) -> CodecResult<ImageInfo> {
    // The private inspector is reached only after root signature detection,
    // which proves an exact six-byte GIF87a/GIF89a prefix.
    let mut input = Input::new(data);
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
    let mut complete = true;
    let mut recovering_from_bad_gce = false;

    loop {
        if input.is_eof() {
            if first_mode.is_some() {
                complete = false;
                break;
            }
            return Err(CodecError::NeedMore {
                minimum: input.position.wrapping_add(1),
                message: "GIF contains no image frame".to_owned(),
            });
        }
        // `is_eof()` above proves the marker byte is present.
        let block = input.data[input.position];
        input.position = input.position.wrapping_add(1);
        match block {
            EXTENSION_INTRODUCER => {
                let label = input.u8()?;
                if label == 0xf9 {
                    let _declared_size = input.u8()?;
                    let packed = input.u8()?;
                    input.skip(2)?;
                    let index = input.u8()?;
                    let terminator = input.u8()?;
                    if terminator != 0 {
                        // Pillow treats the nonzero byte as another data-block
                        // length, consumes that payload and its terminator, and
                        // then scans for the next recognized block marker.
                        input.skip(usize::from(terminator))?;
                        input.skip_sub_blocks()?;
                        recovering_from_bad_gce = true;
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
                frame_count = frame_count.wrapping_add(1);
                fallback_width = fallback_width.max(left.wrapping_add(width));
                fallback_height = fallback_height.max(top.wrapping_add(height));
                let canvas_width = logical_width.max(fallback_width);
                let canvas_height = logical_height.max(fallback_height);
                if u64::from(canvas_width).saturating_mul(u64::from(canvas_height))
                    > PILLOW_DECOMPRESSION_BOMB_ERROR_PIXELS
                {
                    return Err(CodecError::Dimensions(
                        "GIF frame expands beyond Pillow's decompression-bomb limit".to_owned(),
                    ));
                }
                transparent_index = None;
                recovering_from_bad_gce = false;
                input.skip(1)?;
                if input.skip_sub_blocks().is_err() {
                    complete = false;
                    break;
                }
                if basic {
                    complete = *input.data.get(input.position).unwrap_or(&0) == TRAILER;
                    break;
                }
            }
            TRAILER => break,
            _ if recovering_from_bad_gce => {}
            _ => {
                return Err(CodecError::Malformed("unknown GIF block marker".to_owned()));
            }
        }
    }

    let source = if first_palette
        .as_ref()
        .is_some_and(|palette| palette.alpha.iter().any(|&alpha| alpha != u8::MAX))
    {
        crate::types::SourceDescriptor::new().with_alpha(crate::types::SourceAlpha::BinaryMask)
    } else {
        crate::types::SourceDescriptor::new()
    };
    Ok(ImageInfo {
        format: ImageFormat::Gif,
        width: logical_width.max(fallback_width),
        height: logical_height.max(fallback_height),
        mode: first_mode.malformed("GIF contains no image frame")?,
        bit_depth: first_bit_depth,
        palette: first_palette,
        is_animated: frame_count > 1,
        frame_count: complete.then_some(frame_count),
        frame_count_complete: complete,
        cursor_hotspot: None,
        source,
    })
}

fn read_color_table(input: &mut Input<'_>, packed: u8) -> CodecResult<Option<Vec<u8>>> {
    if packed & 0x80 == 0 {
        return Ok(None);
    }
    let length = (3usize).wrapping_shl(u32::from((packed & 7).wrapping_add(1)));
    Ok(Some(input.bytes(length)?.to_vec()))
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
        Self { data, position: 6 }
    }

    fn is_eof(&self) -> bool {
        self.position >= self.data.len()
    }

    fn u8(&mut self) -> CodecResult<u8> {
        let here = self.position;
        let value = need_slice(
            self.data,
            here,
            codec_add_end(here, 1, "truncated GIF byte field")?,
            "truncated GIF byte field",
        )?[0];
        self.position = here.wrapping_add(1);
        Ok(value)
    }

    fn u16(&mut self) -> CodecResult<u16> {
        let bytes = self.bytes(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn bytes(&mut self, length: usize) -> CodecResult<&'a [u8]> {
        let here = self.position;
        let end = codec_add_end(here, length, "truncated GIF field")?;
        let bytes = need_slice(self.data, here, end, "truncated GIF field")?;
        self.position = end;
        Ok(bytes)
    }

    fn skip(&mut self, length: usize) -> CodecResult<()> {
        self.bytes(length).map(|_| ())
    }

    fn skip_sub_blocks(&mut self) -> CodecResult<()> {
        loop {
            let length = usize::from(self.u8()?);
            if length == 0 {
                return Ok(());
            }
            self.skip(length)?;
        }
    }
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    let _ = inspect(b"GIF89a");
    let mut image = b"GIF89a\x01\0\x01\0\0\0\0,\0\0\0\0\x01\0\x01\0\0\x02\0".to_vec();
    assert!(inspect(&image).is_ok());
    image.truncate(13);
    let _ = inspect(&image);
}
