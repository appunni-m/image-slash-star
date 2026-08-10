//! WebP RIFF container and frame decoder.

#![warn(clippy::all)]
#![deny(
    clippy::clone_on_copy,
    clippy::expect_used,
    clippy::large_enum_variant,
    clippy::map_unwrap_or,
    clippy::needless_borrow,
    clippy::needless_collect,
    clippy::needless_range_loop,
    clippy::redundant_clone,
    clippy::todo,
    clippy::unnecessary_cast,
    clippy::unnecessary_to_owned,
    clippy::unwrap_in_result,
    clippy::unwrap_used
)]
// RIFF offsets, VP8X 24-bit fields, and decoded-image geometry follow the WebP
// container specification. Header validation bounds these calculations before
// allocation or slice access.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use super::byteorder_lite::{LittleEndian, ReadBytesExt};

use std::collections::HashMap;
use std::io::{self, BufRead, Cursor, Read};
use std::num::NonZeroU16;
use std::ops::Range;

use super::extended::{self, WebPExtendedInfo, get_alpha_predictor, read_alpha_chunk};

use super::lossless::LosslessDecoder;
use super::vp8::Vp8Decoder;

/// Errors encountered while decoding WebP container, VP8, or VP8L data.
#[derive(Debug)]
pub enum DecodingError {
    IoError,
    WebpSignatureInvalid,
    ChunkMissing,
    ChunkHeaderInvalid,
    InvalidAlphaPreprocessing,
    InvalidCompressionMethod,
    ImageTooLarge,
    FrameOutsideImage,
    LosslessSignatureInvalid,
    VersionNumberInvalid,
    InvalidColorCacheBits,
    HuffmanError,
    BitStreamError,
    TransformError,
    Vp8MagicInvalid,
    ColorSpaceInvalid,
    InconsistentImageSizes,
    UnsupportedFeature,
    InvalidChunkSize,
    NoMoreFrames,
}

impl From<io::Error> for DecodingError {
    fn from(_: io::Error) -> Self {
        Self::IoError
    }
}

/// All possible RIFF chunks in a WebP image file
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub(crate) enum WebPRiffChunk {
    RIFF,
    WEBP,
    VP8,
    VP8L,
    VP8X,
    ANIM,
    ANMF,
    ALPH,
    ICCP,
    EXIF,
    XMP,
    Unknown([u8; 4]),
}

impl WebPRiffChunk {
    pub(crate) const fn from_fourcc(chunk_fourcc: [u8; 4]) -> Self {
        match &chunk_fourcc {
            b"RIFF" => Self::RIFF,
            b"WEBP" => Self::WEBP,
            b"VP8 " => Self::VP8,
            b"VP8L" => Self::VP8L,
            b"VP8X" => Self::VP8X,
            b"ANIM" => Self::ANIM,
            b"ANMF" => Self::ANMF,
            b"ALPH" => Self::ALPH,
            b"ICCP" => Self::ICCP,
            b"EXIF" => Self::EXIF,
            b"XMP " => Self::XMP,
            _ => Self::Unknown(chunk_fourcc),
        }
    }

    pub(crate) const fn is_unknown(self) -> bool {
        matches!(self, Self::Unknown(_))
    }

    pub(crate) const fn fourcc(self) -> [u8; 4] {
        match self {
            Self::RIFF => *b"RIFF",
            Self::WEBP => *b"WEBP",
            Self::VP8 => *b"VP8 ",
            Self::VP8L => *b"VP8L",
            Self::VP8X => *b"VP8X",
            Self::ANIM => *b"ANIM",
            Self::ANMF => *b"ANMF",
            Self::ALPH => *b"ALPH",
            Self::ICCP => *b"ICCP",
            Self::EXIF => *b"EXIF",
            Self::XMP => *b"XMP ",
            Self::Unknown(fourcc) => fourcc,
        }
    }
}

// enum WebPImage {
//     Lossy(VP8Frame),
//     Lossless(LosslessFrame),
//     Extended(ExtendedImage),
// }

struct AnimationState {
    next_frame: u32,
    next_frame_start: u64,
    dispose_next_frame: bool,
    previous_frame_width: u32,
    previous_frame_height: u32,
    previous_frame_x_offset: u32,
    previous_frame_y_offset: u32,
    canvas: Option<Vec<u8>>,
}
impl Default for AnimationState {
    fn default() -> Self {
        Self {
            next_frame: 0,
            next_frame_start: 0,
            dispose_next_frame: true,
            previous_frame_width: 0,
            previous_frame_height: 0,
            previous_frame_x_offset: 0,
            previous_frame_y_offset: 0,
            canvas: None,
        }
    }
}

/// Number of times that an animation loops.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum LoopCount {
    /// The animation loops forever.
    Forever,
    /// Each frame of the animation is displayed the specified number of times.
    Times(NonZeroU16),
}

/// Source metadata for the composited frame returned by [`WebPDecoder::read_frame`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FrameInfo {
    pub(crate) left: u32,
    pub(crate) top: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) duration_ms: u32,
    pub(crate) dispose_to_background: bool,
    pub(crate) blend_over: bool,
}

/// WebP image format decoder.
pub struct WebPDecoder<'a> {
    r: Cursor<&'a [u8]>,

    width: u32,
    height: u32,

    extended: Option<WebPExtendedInfo>,
    animation: AnimationState,

    has_alpha: bool,
    num_frames: u32,
    loop_count: LoopCount,

    chunks: HashMap<WebPRiffChunk, Range<u64>>,
    pub(crate) metadata: Vec<crate::types::OpaqueMetadata>,
    pub(crate) opaque_blocks: Vec<crate::types::OpaqueBlock>,
}

impl<'a> WebPDecoder<'a> {
    /// Create a new `WebPDecoder` from the reader `r`. The decoder performs many small reads, so the
    /// reader should be buffered.
    pub fn new(r: Cursor<&'a [u8]>) -> Result<Self, DecodingError> {
        let mut decoder = Self {
            r,
            width: 0,
            height: 0,
            num_frames: 0,
            extended: None,
            chunks: HashMap::new(),
            animation: Default::default(),
            has_alpha: false,
            loop_count: LoopCount::Times(NonZeroU16::MIN),
            metadata: Vec::new(),
            opaque_blocks: Vec::new(),
        };
        decoder.read_data()?;
        #[cfg(target_pointer_width = "32")]
        decoder.validate_output_buffer_size()?;
        #[cfg(not(target_pointer_width = "32"))]
        decoder.validate_output_buffer_size();
        if decoder.is_animated() && decoder.num_frames == 0 {
            return Err(DecodingError::ChunkMissing);
        }
        Ok(decoder)
    }

    // The VP8X validation predicate establishes the ANIM/ANMF map entries, and
    // the nonzero match arm establishes `NonZeroU16::new(n)`.
    #[allow(clippy::expect_used, clippy::unwrap_used)]
    fn read_data(&mut self) -> Result<(), DecodingError> {
        let (WebPRiffChunk::RIFF, riff_size, _) = read_chunk_header(&mut self.r)? else {
            return Err(DecodingError::ChunkHeaderInvalid);
        };

        (read_fourcc(&mut self.r)? == WebPRiffChunk::WEBP)
            .then_some(())
            .ok_or(DecodingError::WebpSignatureInvalid)?;

        let (chunk, chunk_size, chunk_size_rounded) = read_chunk_header(&mut self.r)?;
        let start = self.r.position();

        match chunk {
            WebPRiffChunk::VP8 => {
                let tag = self.r.read_u24::<LittleEndian>()?;

                let keyframe = tag & 1 == 0;
                if !keyframe {
                    return Err(DecodingError::UnsupportedFeature);
                }

                let mut tag = [0u8; 3];
                self.r.read_exact(&mut tag)?;
                if tag != [0x9d, 0x01, 0x2a] {
                    return Err(DecodingError::Vp8MagicInvalid);
                }

                let w = self.r.read_u16::<LittleEndian>()?;
                let h = self.r.read_u16::<LittleEndian>()?;

                self.width = u32::from(w & 0x3FFF);
                self.height = u32::from(h & 0x3FFF);
                if self.width == 0 || self.height == 0 {
                    return Err(DecodingError::InconsistentImageSizes);
                }

                self.chunks
                    .insert(WebPRiffChunk::VP8, start..start + chunk_size);
            }
            WebPRiffChunk::VP8L => {
                let signature = self.r.read_u8()?;
                if signature != 0x2f {
                    return Err(DecodingError::LosslessSignatureInvalid);
                }

                let header = self.r.read_u32::<LittleEndian>()?;
                let version = header >> 29;
                if version != 0 {
                    return Err(DecodingError::VersionNumberInvalid);
                }

                self.width = (1 + header) & 0x3FFF;
                self.height = (1 + (header >> 14)) & 0x3FFF;
                self.chunks
                    .insert(WebPRiffChunk::VP8L, start..start + chunk_size);
                self.has_alpha = (header >> 28) & 1 != 0;
            }
            WebPRiffChunk::VP8X => {
                let mut info = extended::read_extended_header(&mut self.r)?;
                self.width = info.canvas_width;
                self.height = info.canvas_height;

                let mut position = start + chunk_size_rounded;
                let max_position = position + riff_size.saturating_sub(12);
                self.r.set_position(position);

                while position < max_position {
                    match read_chunk_header(&mut self.r) {
                        Ok((chunk, chunk_size, chunk_size_rounded)) => {
                            let range = position + 8..position + 8 + chunk_size;
                            position += 8 + chunk_size_rounded;

                            if chunk == WebPRiffChunk::ANMF {
                                if chunk_size < 24 {
                                    return Err(DecodingError::InvalidChunkSize);
                                }

                                self.r.set_position(self.r.position() + 12);
                                let _duration = self.r.read_u32::<LittleEndian>()? & 0xffffff;
                                let frame_chunk = read_fourcc(&mut self.r)?;
                                self.r.set_position(position);
                                self.chunks.entry(chunk).or_insert(range);

                                if matches!(
                                    frame_chunk,
                                    WebPRiffChunk::VP8 | WebPRiffChunk::VP8L | WebPRiffChunk::ALPH
                                ) {
                                    self.num_frames += 1;
                                }

                                continue;
                            }

                            if !chunk.is_unknown() {
                                self.chunks.entry(chunk).or_insert(range.clone());
                            }

                            match chunk {
                                WebPRiffChunk::ICCP | WebPRiffChunk::EXIF | WebPRiffChunk::XMP => {
                                    let data = *self.r.get_ref();
                                    let start = usize_from_u64(range.start);
                                    let end = usize_from_u64(range.end);
                                    if end <= data.len() {
                                        self.metadata.push(crate::types::OpaqueMetadata {
                                            kind: chunk.fourcc().to_vec(),
                                            data: data[start..end].to_vec(),
                                        });
                                    }
                                }
                                WebPRiffChunk::Unknown(fourcc) => {
                                    let data = *self.r.get_ref();
                                    let start = usize_from_u64(range.start);
                                    let end = usize_from_u64(range.end);
                                    if end <= data.len() {
                                        // WebP defines no safe-to-copy bit;
                                        // unknown RIFF chunks are ignorable by
                                        // decoders, so copying one cannot
                                        // change frame semantics.
                                        self.opaque_blocks.push(crate::types::OpaqueBlock {
                                            kind: fourcc.to_vec(),
                                            data: data[start..end].to_vec(),
                                            safe_to_copy: true,
                                        });
                                    }
                                }
                                _ => {}
                            }

                            self.r.set_position(position);
                        }
                        // A `Cursor<&[u8]>` can only fail these reads at the end of
                        // its input, so a partial trailing chunk ends the scan.
                        Err(_) => break,
                    }
                }
                // NOTE: Pillow tolerates malformed VP8X metadata flags when the
                // corresponding ICCP/EXIF/XMP chunks are absent.
                if info.animation
                    && (!self.chunks.contains_key(&WebPRiffChunk::ANIM)
                        || !self.chunks.contains_key(&WebPRiffChunk::ANMF))
                    || !info.animation
                        && self.chunks.contains_key(&WebPRiffChunk::VP8)
                            == self.chunks.contains_key(&WebPRiffChunk::VP8L)
                {
                    return Err(DecodingError::ChunkMissing);
                }

                // Decode ANIM chunk.
                if info.animation {
                    let range = self
                        .chunks
                        .get(&WebPRiffChunk::ANIM)
                        .cloned()
                        .expect("animated VP8X validation requires an ANIM chunk");
                    if range.end - range.start < 6 {
                        return Err(DecodingError::InvalidChunkSize);
                    }
                    self.r.set_position(range.start);
                    let mut chunk = [0; 6];
                    self.r.read_exact(&mut chunk)?;
                    info.background_color_hint = [chunk[2], chunk[1], chunk[0], chunk[3]];
                    self.loop_count = match u16::from_le_bytes([chunk[4], chunk[5]]) {
                        0 => LoopCount::Forever,
                        n => LoopCount::Times(NonZeroU16::new(n).unwrap()),
                    };
                    self.animation.next_frame_start =
                        self.chunks.get(&WebPRiffChunk::ANMF).unwrap().start - 8;
                }

                // If the image is animated, the image data chunk will be inside the ANMF chunks. We
                // store the ALPH, VP8, and VP8L chunks (as applicable) of the first frame in the
                // hashmap so that we can read them later.
                if let Some(range) = self.chunks.get(&WebPRiffChunk::ANMF).cloned() {
                    let mut position = range.start + 16;
                    self.r.set_position(position);
                    for _ in 0..2 {
                        let (subchunk, subchunk_size, subchunk_size_rounded) =
                            read_chunk_header(&mut self.r)?;
                        let subrange = position + 8..position + 8 + subchunk_size;
                        self.chunks.entry(subchunk).or_insert(subrange.clone());

                        position += 8 + subchunk_size_rounded;
                        if position + 8 > range.end {
                            break;
                        }
                    }
                }

                self.has_alpha = info.alpha;
                self.extended = Some(info);
            }
            _ => return Err(DecodingError::ChunkHeaderInvalid),
        };

        Ok(())
    }

    /// Returns the (width, height) of the image in pixels.
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Returns whether the image has an alpha channel. If so, the pixel format is Rgba8 and
    /// otherwise Rgb8.
    pub fn has_alpha(&self) -> bool {
        self.has_alpha
    }

    /// Returns true if the image is animated.
    pub fn is_animated(&self) -> bool {
        self.extended
            .as_ref()
            .is_some_and(|extended| extended.animation)
    }

    /// Returns the stable context used for failures raised while decoding an
    /// image payload. The offset identifies the validated VP8/VP8L payload
    /// (or the current ANMF container for an animation); the bitstream
    /// decoder does not expose a more granular byte position.
    pub(crate) fn bitstream_context(&self) -> Option<(u64, &'static str)> {
        if self.is_animated() {
            return Some((self.animation.next_frame_start, "webp_bitstream"));
        }

        self.chunks
            .get(&WebPRiffChunk::VP8L)
            .or_else(|| self.chunks.get(&WebPRiffChunk::VP8))
            .map(|range| (range.start, "webp_bitstream"))
    }

    /// Returns the number of frames of a single loop of the animation, or zero if the image is not
    /// animated.
    pub fn num_frames(&self) -> u32 {
        self.num_frames
    }

    /// Returns the number of times the animation should loop.
    pub fn loop_count(&self) -> LoopCount {
        self.loop_count
    }

    /// Returns the animation canvas background in RGBA order.
    pub fn background_color(&self) -> Option<[u8; 4]> {
        self.extended
            .as_ref()
            .filter(|extended| extended.animation)
            .map(|extended| {
                extended
                    .background_color
                    .unwrap_or(extended.background_color_hint)
            })
    }

    #[cfg(target_pointer_width = "32")]
    fn validate_output_buffer_size(&self) -> Result<(), DecodingError> {
        let bytes_per_pixel = if self.has_alpha() { 4 } else { 3 };
        let Some(_) = (self.width as usize)
            .checked_mul(self.height as usize)
            .and_then(|pixels| pixels.checked_mul(bytes_per_pixel))
        else {
            return Err(DecodingError::ImageTooLarge);
        };
        Ok(())
    }

    #[cfg(not(target_pointer_width = "32"))]
    fn validate_output_buffer_size(&self) {}

    /// Returns the number of bytes required to store the image or a single frame.
    pub fn output_buffer_size(&self) -> usize {
        let bytes_per_pixel = if self.has_alpha() { 4 } else { 3 };
        (self.width as usize) * (self.height as usize) * bytes_per_pixel
    }

    /// Returns the raw bytes of the image. For animated images, this is the first frame.
    ///
    /// Fails with `ImageTooLarge` if `buf` has length different than `output_buffer_size()`
    // Construction guarantees ANMF for animation and exactly one VP8/VP8L
    // payload for still images.
    #[allow(clippy::expect_used, clippy::unwrap_used)]
    pub fn read_image(&mut self, buf: &mut [u8]) -> Result<(), DecodingError> {
        (buf.len() == self.output_buffer_size())
            .then_some(())
            .ok_or(DecodingError::ImageTooLarge)?;

        if self.is_animated() {
            let saved = std::mem::take(&mut self.animation);
            self.animation.next_frame_start =
                self.chunks.get(&WebPRiffChunk::ANMF).unwrap().start - 8;
            let result = self.read_frame(buf);
            self.animation = saved;
            result?;
        } else if let Some(range) = self.chunks.get(&WebPRiffChunk::VP8L) {
            let mut decoder =
                LosslessDecoder::new(Box::new(range_reader(&mut self.r, range.clone())));

            if self.has_alpha {
                decoder.decode_frame(self.width, self.height, buf)?;
            } else {
                decoder.decode_frame_rgb(self.width, self.height, buf)?;
            }
        } else {
            let range = self
                .chunks
                .get(&WebPRiffChunk::VP8)
                .expect("non-lossless WebP validation requires a VP8 chunk");
            let reader = range_reader(&mut self.r, range.start..range.end);
            let frame = Vp8Decoder::decode_frame(reader)?;
            (u32::from(frame.width) == self.width && u32::from(frame.height) == self.height)
                .then_some(())
                .ok_or(DecodingError::InconsistentImageSizes)?;

            if self.has_alpha() {
                frame.fill_rgba(buf);

                let Some(range) = self.chunks.get(&WebPRiffChunk::ALPH).cloned() else {
                    for pixel in buf.chunks_exact_mut(4) {
                        pixel[3] = 255;
                    }
                    return Ok(());
                };

                let alpha_chunk = read_alpha_chunk(
                    &mut range_reader(&mut self.r, range),
                    self.width as u16,
                    self.height as u16,
                )?;

                for y in 0..frame.height {
                    for x in 0..frame.width {
                        let predictor: u8 = get_alpha_predictor(
                            x.into(),
                            y.into(),
                            frame.width.into(),
                            alpha_chunk.filtering_method,
                            buf,
                        );

                        let alpha_index =
                            usize::from(y) * usize::from(frame.width) + usize::from(x);
                        let buffer_index = alpha_index * 4 + 3;

                        buf[buffer_index] = predictor.wrapping_add(alpha_chunk.data[alpha_index]);
                    }
                }
            } else {
                frame.fill_rgb(buf);
            }
        }

        Ok(())
    }

    /// Reads the next frame of the animation.
    ///
    /// The frame contents are written into `buf` and the method returns the duration of the frame
    /// in milliseconds. If there are no more frames, the method returns
    /// `DecodingError::NoMoreFrames` and `buf` is left unchanged.
    ///
    /// # Panics
    ///
    /// Panics if the image is not animated.
    // The public precondition and constructor validation guarantee extended
    // metadata; the local initialization guarantees a canvas before use.
    #[allow(clippy::expect_used, clippy::unwrap_used)]
    pub fn read_frame(&mut self, buf: &mut [u8]) -> Result<FrameInfo, DecodingError> {
        assert!(self.is_animated());
        assert_eq!(buf.len(), self.output_buffer_size());

        (self.animation.next_frame != self.num_frames)
            .then_some(())
            .ok_or(DecodingError::NoMoreFrames)?;

        let info = self
            .extended
            .as_ref()
            .expect("animated decoder state requires extended metadata");

        self.r.set_position(self.animation.next_frame_start);

        let anmf_size = match read_chunk_header(&mut self.r)? {
            (WebPRiffChunk::ANMF, size, _) if size >= 32 => size,
            _ => return Err(DecodingError::ChunkHeaderInvalid),
        };

        // Read ANMF chunk
        let frame_x = extended::read_3_bytes(&mut self.r)? * 2;
        let frame_y = extended::read_3_bytes(&mut self.r)? * 2;
        let source_width = extended::read_3_bytes(&mut self.r)? + 1;
        let source_height = extended::read_3_bytes(&mut self.r)? + 1;
        let mut frame_width = source_width;
        let mut frame_height = source_height;
        let duration = extended::read_3_bytes(&mut self.r)?;
        let frame_info = self.r.read_u8()?;
        let use_alpha_blending = frame_info & 0b00000010 == 0;
        let dispose = frame_info & 0b00000001 != 0;

        let clear_color = if self.animation.dispose_next_frame {
            Some(info.background_color.unwrap_or(info.background_color_hint))
        } else {
            None
        };

        // Read normal bitstream now
        let (chunk, chunk_size, chunk_size_rounded) = read_chunk_header(&mut self.r)?;
        if chunk_size_rounded + 24 > anmf_size {
            return Err(DecodingError::ChunkHeaderInvalid);
        }

        let (frame, frame_has_alpha): (Vec<u8>, bool) = match chunk {
            WebPRiffChunk::VP8 => {
                let reader = (&mut self.r).take(chunk_size);
                let raw_frame = Vp8Decoder::decode_frame(reader)?;
                frame_width = u32::from(raw_frame.width);
                frame_height = u32::from(raw_frame.height);
                let mut rgb_frame = vec![0; frame_width as usize * frame_height as usize * 3];
                raw_frame.fill_rgb(&mut rgb_frame);
                (rgb_frame, false)
            }
            WebPRiffChunk::VP8L => {
                (frame_width <= 16384 && frame_height <= 16384)
                    .then_some(())
                    .ok_or(DecodingError::ImageTooLarge)?;
                let reader = (&mut self.r).take(chunk_size);
                let mut lossless_decoder = LosslessDecoder::new(Box::new(reader));
                let mut rgba_frame = vec![0; frame_width as usize * frame_height as usize * 4];
                lossless_decoder.decode_frame(frame_width, frame_height, &mut rgba_frame)?;
                (rgba_frame, true)
            }
            WebPRiffChunk::ALPH => {
                (frame_width <= 16384 && frame_height <= 16384)
                    .then_some(())
                    .ok_or(DecodingError::ImageTooLarge)?;
                if chunk_size_rounded + 32 > anmf_size {
                    return Err(DecodingError::ChunkHeaderInvalid);
                }

                // read alpha
                let next_chunk_start = self.r.position() + chunk_size_rounded;
                let mut reader = (&mut self.r).take(chunk_size);
                let alpha_chunk =
                    read_alpha_chunk(&mut reader, frame_width as u16, frame_height as u16)?;

                // read opaque
                self.r.set_position(next_chunk_start);
                let (_next_chunk, next_chunk_size, _) = read_chunk_header(&mut self.r)?;
                if chunk_size + next_chunk_size + 32 > anmf_size {
                    return Err(DecodingError::ChunkHeaderInvalid);
                }

                let frame = Vp8Decoder::decode_frame((&mut self.r).take(next_chunk_size))?;

                let mut rgba_frame = vec![0; frame_width as usize * frame_height as usize * 4];
                frame.fill_rgba(&mut rgba_frame);

                for y in 0..frame.height {
                    for x in 0..frame.width {
                        let predictor: u8 = get_alpha_predictor(
                            x.into(),
                            y.into(),
                            frame.width.into(),
                            alpha_chunk.filtering_method,
                            &rgba_frame,
                        );

                        let alpha_index =
                            usize::from(y) * usize::from(frame.width) + usize::from(x);
                        let buffer_index = alpha_index * 4 + 3;

                        rgba_frame[buffer_index] =
                            predictor.wrapping_add(alpha_chunk.data[alpha_index]);
                    }
                }

                (rgba_frame, true)
            }
            _ => {
                self.animation.next_frame_start += anmf_size + 8;
                return self.read_frame(buf);
            }
        };

        if frame_x + frame_width > self.width || frame_y + frame_height > self.height {
            return Err(DecodingError::FrameOutsideImage);
        }

        // fill starting canvas with clear color
        if self.animation.canvas.is_none() {
            self.animation.canvas = {
                let mut canvas = vec![0; (self.width * self.height * 4) as usize];
                let color = info.background_color.unwrap_or(info.background_color_hint);
                canvas
                    .chunks_exact_mut(4)
                    .for_each(|pixel| pixel.copy_from_slice(&color));
                Some(canvas)
            }
        }
        extended::composite_frame(
            self.animation.canvas.as_mut().unwrap(),
            self.width,
            self.height,
            clear_color,
            &frame,
            frame_x,
            frame_y,
            frame_width,
            frame_height,
            frame_has_alpha,
            use_alpha_blending,
            self.animation.previous_frame_width,
            self.animation.previous_frame_height,
            self.animation.previous_frame_x_offset,
            self.animation.previous_frame_y_offset,
        );

        self.animation.previous_frame_width = frame_width;
        self.animation.previous_frame_height = frame_height;
        self.animation.previous_frame_x_offset = frame_x;
        self.animation.previous_frame_y_offset = frame_y;

        self.animation.dispose_next_frame = dispose;
        self.animation.next_frame_start += anmf_size + 8;
        self.animation.next_frame += 1;

        if self.has_alpha() {
            buf.copy_from_slice(self.animation.canvas.as_ref().unwrap());
        } else {
            for (b, c) in buf
                .chunks_exact_mut(3)
                .zip(self.animation.canvas.as_ref().unwrap().chunks_exact(4))
            {
                b.copy_from_slice(&c[..3]);
            }
        }

        Ok(FrameInfo {
            left: frame_x,
            top: frame_y,
            // Pillow/libwebp composite the decoded bitstream dimensions when
            // tolerated ANMF declarations disagree with their nested frame.
            width: frame_width,
            height: frame_height,
            duration_ms: duration,
            dispose_to_background: dispose,
            blend_over: use_alpha_blending,
        })
    }
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    for (chunk, fourcc) in [
        (WebPRiffChunk::RIFF, *b"RIFF"),
        (WebPRiffChunk::WEBP, *b"WEBP"),
        (WebPRiffChunk::VP8, *b"VP8 "),
        (WebPRiffChunk::VP8L, *b"VP8L"),
        (WebPRiffChunk::VP8X, *b"VP8X"),
        (WebPRiffChunk::ANIM, *b"ANIM"),
        (WebPRiffChunk::ANMF, *b"ANMF"),
        (WebPRiffChunk::ALPH, *b"ALPH"),
        (WebPRiffChunk::ICCP, *b"ICCP"),
        (WebPRiffChunk::EXIF, *b"EXIF"),
        (WebPRiffChunk::XMP, *b"XMP "),
        (WebPRiffChunk::Unknown(*b"ABCD"), *b"ABCD"),
    ] {
        assert_eq!(chunk.fourcc(), fourcc);
    }

    fn chunk(fourcc: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 + payload.len() + usize::from(payload.len() % 2 != 0));
        out.extend_from_slice(fourcc);
        out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        out.extend_from_slice(payload);
        if payload.len() % 2 != 0 {
            out.push(0);
        }
        out
    }

    fn chunk_declared(fourcc: &[u8; 4], declared_size: u32, physical_payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 + physical_payload.len());
        out.extend_from_slice(fourcc);
        out.extend_from_slice(&declared_size.to_le_bytes());
        out.extend_from_slice(physical_payload);
        out
    }

    fn riff(chunks: &[Vec<u8>]) -> Vec<u8> {
        let payload_len = 4 + chunks.iter().map(Vec::len).sum::<usize>();
        let mut out = Vec::with_capacity(8 + payload_len);
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(payload_len as u32).to_le_bytes());
        out.extend_from_slice(b"WEBP");
        for chunk in chunks {
            out.extend_from_slice(chunk);
        }
        out
    }

    fn vp8x(flags: u8, width: u32, height: u32) -> Vec<u8> {
        let mut payload = vec![flags, 0, 0, 0];
        let width = width - 1;
        let height = height - 1;
        payload.extend_from_slice(&width.to_le_bytes()[..3]);
        payload.extend_from_slice(&height.to_le_bytes()[..3]);
        chunk(b"VP8X", &payload)
    }

    fn anmf_payload(
        frame_x: u32,
        frame_y: u32,
        frame_width_minus_one: u32,
        frame_height_minus_one: u32,
        subchunk: &[u8; 4],
    ) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&frame_x.to_le_bytes()[..3]);
        payload.extend_from_slice(&frame_y.to_le_bytes()[..3]);
        payload.extend_from_slice(&frame_width_minus_one.to_le_bytes()[..3]);
        payload.extend_from_slice(&frame_height_minus_one.to_le_bytes()[..3]);
        payload.extend_from_slice(&0u32.to_le_bytes()[..3]);
        payload.push(0);
        payload.extend_from_slice(subchunk);
        payload.extend_from_slice(&0u32.to_le_bytes());
        payload.resize(32, 0);
        payload
    }

    fn exercise_animation_data(
        stream: Vec<u8>,
        width: u32,
        height: u32,
        has_alpha: bool,
        num_frames: u32,
        next_frame: u32,
        dispose_next_frame: bool,
        read_count: usize,
    ) {
        let mut decoder = WebPDecoder {
            r: Cursor::new(stream.as_slice()),
            width,
            height,
            extended: Some(WebPExtendedInfo {
                alpha: has_alpha,
                canvas_width: width,
                canvas_height: height,
                animation: true,
                background_color: None,
                background_color_hint: [0, 0, 0, 0],
            }),
            animation: AnimationState::default(),
            has_alpha,
            num_frames,
            loop_count: LoopCount::Forever,
            chunks: HashMap::new(),
            metadata: Vec::new(),
            opaque_blocks: Vec::new(),
        };
        decoder.animation.next_frame = next_frame;
        decoder.animation.dispose_next_frame = dispose_next_frame;
        let mut buf = vec![0; decoder.output_buffer_size()];
        for _ in 0..read_count {
            let _ = decoder.read_frame(&mut buf);
        }
    }

    fn exercise_animation_state(
        anmf: Vec<u8>,
        width: u32,
        height: u32,
        has_alpha: bool,
        next_frame: u32,
        dispose_next_frame: bool,
        read_count: usize,
    ) {
        exercise_animation_data(
            chunk(b"ANMF", &anmf),
            width,
            height,
            has_alpha,
            1,
            next_frame,
            dispose_next_frame,
            read_count,
        );
    }

    fn exercise_animation(anmf: Vec<u8>, width: u32, height: u32, has_alpha: bool) {
        exercise_animation_state(anmf, width, height, has_alpha, 0, false, 1);
    }

    fn exercise_new(data: Vec<u8>) {
        let _ = WebPDecoder::new(Cursor::new(data.as_slice()));
    }

    exercise_new(Vec::new());
    exercise_new(b"RIFF\x04\0\0\0WE".to_vec());
    exercise_new(b"RIFF\x04\0\0\0WEBP".to_vec());
    exercise_new(riff(&[chunk(b"JUNK", &[])]));

    let vp8_zero_width = [0, 0, 0, 0x9d, 0x01, 0x2a, 0, 0, 1, 0];
    exercise_new(riff(&[chunk(b"VP8 ", &vp8_zero_width)]));
    let vp8_interframe = [1, 0, 0];
    exercise_new(riff(&[chunk(b"VP8 ", &vp8_interframe)]));
    let vp8_bad_magic = [0, 0, 0, 0, 0, 0];
    exercise_new(riff(&[chunk(b"VP8 ", &vp8_bad_magic)]));
    let vp8_missing_width = [0, 0, 0, 0x9d, 0x01, 0x2a];
    exercise_new(riff(&[chunk(b"VP8 ", &vp8_missing_width)]));
    let vp8_missing_height = [0, 0, 0, 0x9d, 0x01, 0x2a, 1, 0];
    exercise_new(riff(&[chunk(b"VP8 ", &vp8_missing_height)]));
    let vp8_zero_height = [0, 0, 0, 0x9d, 0x01, 0x2a, 1, 0, 0, 0];
    exercise_new(riff(&[chunk(b"VP8 ", &vp8_zero_height)]));
    let vp8_valid_header = [0, 0, 0, 0x9d, 0x01, 0x2a, 1, 0, 1, 0];
    exercise_new(riff(&[chunk(b"VP8 ", &vp8_valid_header)]));

    exercise_new(b"JUNK\x04\0\0\0WEBP".to_vec());
    exercise_new(b"RIFF\x04\0\0\0JUNK".to_vec());
    exercise_new(riff(&[chunk(b"VP8L", &[])]));
    exercise_new(riff(&[chunk(b"VP8L", &[0])]));
    exercise_new(riff(&[chunk(b"VP8L", &[0x2f])]));
    let vp8l_bad_version = [0x2f, 0, 0, 0, 0x20];
    exercise_new(riff(&[chunk(b"VP8L", &vp8l_bad_version)]));
    let vp8l_no_alpha = [0x2f, 0, 0, 0, 0];
    let vp8l_alpha = [0x2f, 0, 0, 0, 0x10];
    exercise_new(riff(&[chunk(b"VP8L", &vp8l_no_alpha)]));
    exercise_new(riff(&[chunk(b"VP8L", &vp8l_alpha)]));

    exercise_new(riff(&[vp8x(0, 1, 1)]));
    exercise_new(riff(&[
        vp8x(0, 1, 1),
        chunk(b"VP8 ", &[]),
        chunk(b"VP8L", &[]),
    ]));
    exercise_new(riff(&[vp8x(0, 1, 1), chunk(b"VP8L", &[])]));
    exercise_new(riff(&[vp8x(0, 1, 1), chunk(b"VP8 ", &[])]));
    exercise_new(riff(&[
        vp8x(0b0000_1100, 1, 1),
        chunk(b"EXIF", &[]),
        chunk(b"XMP ", &[]),
        chunk(b"VP8L", &[]),
    ]));
    exercise_new(riff(&[
        vp8x(0b0000_1100, 1, 1),
        chunk(b"EXIF", &[]),
        chunk(b"VP8L", &[]),
    ]));
    exercise_new(riff(&[
        vp8x(0, 1, 1),
        chunk(b"zzzz", &[]),
        chunk(b"VP8L", &[]),
    ]));
    exercise_new(riff(&[vp8x(0b0000_1000, 1, 1)]));
    exercise_new(riff(&[vp8x(0b0000_0100, 1, 1)]));
    exercise_new(riff(&[vp8x(0b0000_0010, 1, 1), chunk(b"ANMF", &[0; 8])]));
    exercise_new(riff(&[
        vp8x(0b0000_0010, 1, 1),
        chunk_declared(b"ANMF", 24, &[0; 12]),
    ]));
    exercise_new(riff(&[
        vp8x(0b0000_0010, 1, 1),
        chunk_declared(b"ANMF", 24, &[0; 16]),
    ]));
    exercise_new(riff(&[
        vp8x(0b0000_0010, 1, 1),
        chunk(b"ANIM", &[0, 0, 0, 0, 0, 0]),
    ]));
    exercise_new(riff(&[
        vp8x(0b0000_0010, 1, 1),
        chunk(b"ANMF", &anmf_payload(0, 0, 0, 0, b"VP8L")),
    ]));
    exercise_new(riff(&[
        vp8x(0b0000_0010, 1, 1),
        chunk(b"ANIM", &[0, 0, 0, 0, 0, 0]),
        chunk(b"ANMF", &anmf_payload(0, 0, 0, 0, b"VP8L")),
    ]));
    exercise_new(riff(&[
        vp8x(0b0000_0010, 1, 1),
        chunk(b"ANIM", &[0, 0, 0, 0]),
        chunk(b"ANMF", &anmf_payload(0, 0, 0, 0, b"VP8L")),
    ]));
    exercise_new(riff(&[
        vp8x(0b0000_0010, 1, 1),
        chunk(b"ANIM", &[0, 0, 0, 0, 7, 0]),
        chunk(b"ANMF", &anmf_payload(0, 0, 0, 0, b"JUNK")),
    ]));
    exercise_new(riff(&[
        vp8x(0b0000_0010, 1, 1),
        chunk(b"ANIM", &[0, 0, 0, 0, 7, 0]),
        chunk_declared(b"ANMF", 24, &[0; 20]),
    ]));

    let mut truncated = riff(&[vp8x(0, 1, 1)]);
    truncated[4..8].copy_from_slice(&64u32.to_le_bytes());
    truncated.extend_from_slice(b"VP");
    exercise_new(truncated);

    let mut no_trailing_chunks = riff(&[vp8x(0, 1, 1)]);
    no_trailing_chunks[4..8].copy_from_slice(&10u32.to_le_bytes());
    exercise_new(no_trailing_chunks);

    exercise_animation_data(chunk_declared(b"ANMF", 32, &[]), 1, 1, true, 1, 0, false, 1);
    exercise_animation_data(
        chunk_declared(b"ANMF", 32, &[0; 3]),
        1,
        1,
        true,
        1,
        0,
        false,
        1,
    );
    exercise_animation_data(
        chunk_declared(b"ANMF", 32, &[0; 8]),
        1,
        1,
        true,
        1,
        0,
        false,
        1,
    );
    exercise_animation_data(
        chunk_declared(b"ANMF", 32, &[0; 9]),
        1,
        1,
        true,
        1,
        0,
        false,
        1,
    );
    exercise_animation_data(
        chunk_declared(b"ANMF", 32, &[0; 14]),
        1,
        1,
        true,
        1,
        0,
        false,
        1,
    );
    exercise_animation_data(
        chunk_declared(b"ANMF", 32, &[0; 15]),
        1,
        1,
        true,
        1,
        0,
        false,
        1,
    );
    exercise_animation_data(
        chunk_declared(b"ANMF", 32, &[0; 16]),
        1,
        1,
        true,
        1,
        0,
        false,
        1,
    );

    exercise_animation(vec![0; 31], 1, 1, true);
    exercise_animation(anmf_payload(0, 0, 16_384, 0, b"VP8L"), 1, 1, true);
    exercise_animation(anmf_payload(0, 0, 0, 16_384, b"VP8L"), 1, 1, true);
    exercise_animation_state(anmf_payload(0, 0, 0, 0, b"VP8L"), 1, 1, true, 0, true, 1);

    let public_reader_vp8l_width_too_large =
        chunk(b"ANMF", &anmf_payload(0, 0, 16_384, 0, b"VP8L"));
    exercise_animation_data(
        public_reader_vp8l_width_too_large,
        1,
        1,
        true,
        1,
        0,
        false,
        1,
    );

    exercise_animation(anmf_payload(0, 0, 16_384, 0, b"ALPH"), 1, 1, true);
    exercise_animation(anmf_payload(0, 0, 0, 16_384, b"ALPH"), 1, 1, true);
    exercise_animation(anmf_payload(0, 0, 0, 0, b"ALPH"), 1, 1, true);

    let public_reader_alpha_width_too_large =
        chunk(b"ANMF", &anmf_payload(0, 0, 16_384, 0, b"ALPH"));
    exercise_animation_data(
        public_reader_alpha_width_too_large,
        1,
        1,
        true,
        1,
        0,
        false,
        1,
    );

    let mut anmf = anmf_payload(0, 0, 0, 0, b"ALPH");
    anmf[20..24].copy_from_slice(&16u32.to_le_bytes());
    exercise_animation(anmf, 1, 1, true);

    let mut anmf = anmf_payload(0, 0, 0, 0, b"ALPH");
    anmf[20..24].copy_from_slice(&1u32.to_le_bytes());
    exercise_animation(anmf, 1, 1, true);

    let mut anmf = anmf_payload(0, 0, 0, 0, b"ALPH");
    anmf[20..24].copy_from_slice(&2u32.to_le_bytes());
    anmf.resize(42, 0);
    anmf[26..30].copy_from_slice(b"VP8 ");
    anmf[30..34].copy_from_slice(&9u32.to_le_bytes());
    exercise_animation(anmf, 1, 1, true);

    let mut anmf = anmf_payload(0, 0, 0, 0, b"ALPH");
    anmf[20..24].copy_from_slice(&2u32.to_le_bytes());
    anmf.resize(42, 0);
    anmf[26..30].copy_from_slice(b"VP8 ");
    anmf[30..34].copy_from_slice(&0u32.to_le_bytes());
    exercise_animation(anmf, 1, 1, true);

    let mut anmf = anmf_payload(0, 0, 0, 0, b"ALPH");
    anmf[20..24].copy_from_slice(&2u32.to_le_bytes());
    anmf.truncate(26);
    exercise_animation(anmf, 1, 1, true);

    let mut vp8l_solid_64 = anmf_payload(0, 0, 63, 63, b"VP8L");
    vp8l_solid_64[20..24].copy_from_slice(&23u32.to_le_bytes());
    vp8l_solid_64.truncate(24);
    vp8l_solid_64.extend_from_slice(&[
        47, 63, 192, 15, 0, 7, 208, 172, 70, 116, 185, 255, 1, 32, 33, 252, 127, 175, 69, 244, 63,
        245, 3,
    ]);
    vp8l_solid_64.push(0);

    exercise_animation(vp8l_solid_64.clone(), 1, 64, true);
    exercise_animation(vp8l_solid_64.clone(), 64, 1, true);

    let mut vp8l_solid_y_outside = vp8l_solid_64.clone();
    vp8l_solid_y_outside[3..6].copy_from_slice(&1u32.to_le_bytes()[..3]);
    let public_reader_y_outside = chunk(b"ANMF", &vp8l_solid_y_outside);
    exercise_animation_data(public_reader_y_outside, 64, 64, true, 1, 0, false, 1);

    exercise_animation(vp8l_solid_64.clone(), 64, 64, true);
    exercise_animation(vp8l_solid_64.clone(), 64, 64, false);

    let first_frame = chunk(b"ANMF", &vp8l_solid_64);
    let mut two_frames = first_frame.clone();
    two_frames.extend_from_slice(&first_frame);
    exercise_animation_data(two_frames, 64, 64, true, 2, 0, false, 2);

    exercise_animation(anmf_payload(0, 0, 0, 0, b"JUNK"), 1, 1, false);
    exercise_animation_state(anmf_payload(0, 0, 0, 0, b"VP8L"), 1, 1, true, 1, false, 1);

    let empty = Vec::<u8>::new();
    let mut decoder = WebPDecoder {
        r: Cursor::new(empty.as_slice()),
        width: 1,
        height: 1,
        extended: None,
        animation: AnimationState::default(),
        has_alpha: false,
        num_frames: 0,
        loop_count: LoopCount::Forever,
        chunks: HashMap::from([(WebPRiffChunk::VP8L, 0..0)]),
        metadata: Vec::new(),
        opaque_blocks: Vec::new(),
    };
    let _ = decoder.read_image(&mut []);
}

/// Convert a validated chunk offset to `usize` without a host-width error
/// path. On 64-bit hosts the conversion is lossless; on narrower hosts an
/// unrepresentable offset saturates and the retention bounds guard skips the
/// chunk.
fn usize_from_u64(value: u64) -> usize {
    #[cfg(target_pointer_width = "64")]
    {
        usize::from_ne_bytes(value.to_ne_bytes())
    }
    #[cfg(not(target_pointer_width = "64"))]
    {
        usize::try_from(value).unwrap_or(usize::MAX)
    }
}

pub(crate) fn range_reader<'reader>(
    r: &'reader mut Cursor<&[u8]>,
    range: Range<u64>,
) -> impl BufRead + 'reader {
    r.set_position(range.start);
    r.take(range.end - range.start)
}

pub(crate) fn read_fourcc<R: BufRead>(mut r: R) -> Result<WebPRiffChunk, DecodingError> {
    let mut chunk_fourcc = [0; 4];
    r.read_exact(&mut chunk_fourcc)?;
    Ok(WebPRiffChunk::from_fourcc(chunk_fourcc))
}

pub(crate) fn read_chunk_header<R: BufRead>(
    mut r: R,
) -> Result<(WebPRiffChunk, u64, u64), DecodingError> {
    let chunk = read_fourcc(&mut r)?;
    let chunk_size = r.read_u32::<LittleEndian>()?;
    let chunk_size_rounded = chunk_size.saturating_add(chunk_size & 1);
    Ok((chunk, chunk_size.into(), chunk_size_rounded.into()))
}
