//! Decoding of lossless WebP images
//!
//! [Lossless spec](https://developers.google.com/speed/webp/docs/webp_lossless_bitstream_specification)

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
// VP8L prefix codes, bit-buffer shifts, distance mapping, and decoded geometry
// are bounded by the format's bit widths and validated image dimensions.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "VP8L code fields are fixed-width and decoded dimensions are validated before conversion."
)]

use std::io::BufRead;

use super::decoder::DecodingError;
use super::lossless_transform::{
    apply_color_indexing_transform, apply_color_transform, apply_predictor_transform,
    apply_subtract_green_transform,
};

use super::huffman::HuffmanTree;
use super::lossless_transform::TransformType;

const CODE_LENGTH_CODES: usize = 19;
const CODE_LENGTH_CODE_ORDER: [usize; CODE_LENGTH_CODES] = [
    17, 18, 0, 1, 2, 3, 4, 5, 16, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
];

#[rustfmt::skip]
const DISTANCE_MAP: [(i8, i8); 120] = [
    (0, 1),  (1, 0),  (1, 1),  (-1, 1), (0, 2),  (2, 0),  (1, 2),  (-1, 2),
    (2, 1),  (-2, 1), (2, 2),  (-2, 2), (0, 3),  (3, 0),  (1, 3),  (-1, 3),
    (3, 1),  (-3, 1), (2, 3),  (-2, 3), (3, 2),  (-3, 2), (0, 4),  (4, 0),
    (1, 4),  (-1, 4), (4, 1),  (-4, 1), (3, 3),  (-3, 3), (2, 4),  (-2, 4),
    (4, 2),  (-4, 2), (0, 5),  (3, 4),  (-3, 4), (4, 3),  (-4, 3), (5, 0),
    (1, 5),  (-1, 5), (5, 1),  (-5, 1), (2, 5),  (-2, 5), (5, 2),  (-5, 2),
    (4, 4),  (-4, 4), (3, 5),  (-3, 5), (5, 3),  (-5, 3), (0, 6),  (6, 0),
    (1, 6),  (-1, 6), (6, 1),  (-6, 1), (2, 6),  (-2, 6), (6, 2),  (-6, 2),
    (4, 5),  (-4, 5), (5, 4),  (-5, 4), (3, 6),  (-3, 6), (6, 3),  (-6, 3),
    (0, 7),  (7, 0),  (1, 7),  (-1, 7), (5, 5),  (-5, 5), (7, 1),  (-7, 1),
    (4, 6),  (-4, 6), (6, 4),  (-6, 4), (2, 7),  (-2, 7), (7, 2),  (-7, 2),
    (3, 7),  (-3, 7), (7, 3),  (-7, 3), (5, 6),  (-5, 6), (6, 5),  (-6, 5),
    (8, 0),  (4, 7),  (-4, 7), (7, 4),  (-7, 4), (8, 1),  (8, 2),  (6, 6),
    (-6, 6), (8, 3),  (5, 7),  (-5, 7), (7, 5),  (-7, 5), (8, 4),  (6, 7),
    (-6, 7), (7, 6),  (-7, 6), (8, 5),  (7, 7),  (-7, 7), (8, 6),  (8, 7)
];

const GREEN: usize = 0;
const RED: usize = 1;
const BLUE: usize = 2;
const ALPHA: usize = 3;
const DIST: usize = 4;

const HUFFMAN_CODES_PER_META_CODE: usize = 5;

type HuffmanCodeGroup = [HuffmanTree; HUFFMAN_CODES_PER_META_CODE];

const ALPHABET_SIZE: [u16; HUFFMAN_CODES_PER_META_CODE] = [256 + 24, 256, 256, 256, 40];
// The ordinary VP8L alphabets fit in 280 symbols. The green alphabet grows
// beyond this only when the optional color cache is enabled. Its maximum is
// still format-bounded by the 11-bit color-cache field, so one fixed temporary
// workspace covers both the ordinary and enlarged cases.
const MAX_STACK_HUFFMAN_SYMBOLS: usize = 256 + 24;
const MAX_COLOR_CACHE_BITS: u8 = 11;

const MAX_HUFFMAN_SYMBOLS_WITH_COLOR_CACHE: usize =
    MAX_STACK_HUFFMAN_SYMBOLS + (1usize << MAX_COLOR_CACHE_BITS);

const NUM_TRANSFORM_TYPES: usize = 4;
const MAX_COLOR_INDEXING_TABLE_BYTES: usize = 256 * 4;

//Decodes lossless WebP images
pub(crate) struct LosslessDecoder<'a> {
    bit_reader: BitReader<Box<dyn BufRead + 'a>>,
    transforms: [Option<TransformType>; NUM_TRANSFORM_TYPES],
    // The bitstream permits at most one instance of each transform type.
    transform_order: [u8; NUM_TRANSFORM_TYPES],
    transform_order_len: usize,
    // VP8L permits one color-indexing transform with at most 256 RGBA entries.
    // Keep its retained table in decoder-owned fixed storage instead of making
    // a short-lived heap allocation while the main image stream is decoded.
    color_indexing_table: [u8; MAX_COLOR_INDEXING_TABLE_BYTES],
    width: u16,
    height: u16,
}

impl<'a> LosslessDecoder<'a> {
    /// Create a new decoder
    pub(crate) fn new(r: Box<dyn BufRead + 'a>) -> Self {
        Self {
            bit_reader: BitReader::new(r),
            transforms: [None, None, None, None],
            transform_order: [0; NUM_TRANSFORM_TYPES],
            transform_order_len: 0,
            color_indexing_table: [0; MAX_COLOR_INDEXING_TABLE_BYTES],
            width: 0,
            height: 0,
        }
    }

    /// Decodes a VP8L frame whose payload includes the VP8L signature and
    /// dimension header.
    // Reading the preceding 14-bit height leaves the following four header
    // bits buffered by the reader contract.
    #[allow(
        clippy::expect_used,
        clippy::unwrap_in_result,
        reason = "Reading the VP8L height header buffers the following alpha and version bits."
    )]
    pub(crate) fn decode_frame(
        &mut self,
        width: u32,
        height: u32,
        buf: &mut [u8],
    ) -> Result<(), DecodingError> {
        self.read_frame_header(width, height)?;
        self.decode_frame_body(buf)
    }

    /// Decodes a no-alpha VP8L frame directly into an RGB output buffer when
    /// the frame has no transforms. Transformed or alpha-bearing frames keep
    /// the ordinary RGBA workspace because their internal pixels are four
    /// bytes wide.
    #[allow(
        clippy::expect_used,
        clippy::unwrap_in_result,
        reason = "Reading the VP8L height header buffers the following alpha and version bits."
    )]
    pub(crate) fn decode_frame_rgb(
        &mut self,
        width: u32,
        height: u32,
        buf: &mut [u8],
    ) -> Result<(), DecodingError> {
        let alpha_used = self.read_frame_header(width, height)?;
        let transformed_width = self.read_transforms()?;

        if !alpha_used && self.transform_order_len == 0 {
            return self.decode_image_stream_rgb(transformed_width, self.height, buf);
        }

        let transformed_size = usize::from(transformed_width) * usize::from(self.height) * 4;
        let mut data = vec![0; usize::from(self.width) * usize::from(self.height) * 4];
        self.decode_image_stream(
            transformed_width,
            self.height,
            true,
            &mut data[..transformed_size],
        )?;
        self.apply_transforms(&mut data, transformed_width, transformed_size);
        for (rgba_val, chunk) in data
            .as_chunks::<4>()
            .0
            .iter()
            .zip(buf.as_chunks_mut::<3>().0.iter_mut())
        {
            chunk.copy_from_slice(&rgba_val[..3]);
        }
        Ok(())
    }

    #[allow(
        clippy::expect_used,
        clippy::unwrap_in_result,
        reason = "The height read leaves the VP8L alpha and version bits in the bit-reader buffer."
    )]
    fn read_frame_header(&mut self, width: u32, height: u32) -> Result<bool, DecodingError> {
        self.width = width as u16;
        self.height = height as u16;

        let signature = self.bit_reader.read_bits::<u8>(8)?;
        if signature != 0x2f {
            return Err(DecodingError::LosslessSignatureInvalid);
        }

        self.width = self.bit_reader.read_bits::<u16>(14)? + 1;
        self.height = self.bit_reader.read_bits::<u16>(14)? + 1;
        if u32::from(self.width) != width || u32::from(self.height) != height {
            return Err(DecodingError::InconsistentImageSizes);
        }

        let alpha_used = self
            .bit_reader
            .read_bits::<u8>(1)
            .expect("VP8L height read success proves the alpha bit is buffered");
        let version_num = self
            .bit_reader
            .read_bits::<u8>(3)
            .expect("VP8L height read success proves the version bits are buffered");
        if version_num != 0 {
            return Err(DecodingError::VersionNumberInvalid);
        }

        Ok(alpha_used != 0)
    }

    /// Decodes an ALPH lossless payload whose dimensions are supplied by the
    /// enclosing WebP chunk.
    pub(crate) fn decode_frame_implicit_dimensions(
        &mut self,
        width: u32,
        height: u32,
        buf: &mut [u8],
    ) -> Result<(), DecodingError> {
        self.width = width as u16;
        self.height = height as u16;
        self.decode_frame_body(buf)
    }

    // `transform_order` only records slots populated by `read_transforms`.
    #[allow(
        clippy::unwrap_in_result,
        clippy::unwrap_used,
        reason = "read_transforms records only initialized transform slots before this slice is read."
    )]
    fn decode_frame_body(&mut self, buf: &mut [u8]) -> Result<(), DecodingError> {
        let transformed_width = self.read_transforms()?;
        let transformed_size = usize::from(transformed_width) * usize::from(self.height) * 4;
        self.decode_image_stream(
            transformed_width,
            self.height,
            true,
            &mut buf[..transformed_size],
        )?;

        self.apply_transforms(buf, transformed_width, transformed_size);

        Ok(())
    }

    // `transform_order` only records slots populated by `read_transforms`.
    #[allow(
        clippy::unwrap_in_result,
        clippy::unwrap_used,
        reason = "read_transforms bounds transform_order_len to initialized transform slots."
    )]
    fn apply_transforms(&self, buf: &mut [u8], transformed_width: u16, transformed_size: usize) {
        let mut image_size = transformed_size;
        let mut width = transformed_width;
        for &trans_index in self.transform_order[..self.transform_order_len]
            .iter()
            .rev()
        {
            let transform = self.transforms[usize::from(trans_index)].as_ref().unwrap();
            match transform {
                TransformType::PredictorTransform {
                    size_bits,
                    predictor_data,
                } => apply_predictor_transform(
                    &mut buf[..image_size],
                    width,
                    self.height,
                    *size_bits,
                    predictor_data,
                ),
                TransformType::ColorTransform {
                    size_bits,
                    transform_data,
                } => {
                    apply_color_transform(
                        &mut buf[..image_size],
                        width,
                        *size_bits,
                        transform_data,
                    );
                }
                TransformType::SubtractGreen => {
                    apply_subtract_green_transform(&mut buf[..image_size]);
                }
                TransformType::ColorIndexingTransform { table_size } => {
                    width = self.width;
                    image_size = usize::from(width) * usize::from(self.height) * 4;
                    apply_color_indexing_transform(
                        buf,
                        width,
                        self.height,
                        *table_size,
                        &self.color_indexing_table[..usize::from(*table_size) * 4],
                    );
                }
            }
        }
    }

    /// Reads Image data from the bitstream
    ///
    /// Can be in any of the 5 roles described in the Specification. ARGB Image role has different
    /// behaviour to the other 4. xsize and ysize describe the size of the blocks where each block
    /// has its own entropy code
    fn decode_image_stream(
        &mut self,
        xsize: u16,
        ysize: u16,
        is_argb_img: bool,
        data: &mut [u8],
    ) -> Result<(), DecodingError> {
        let color_cache_bits = self.read_color_cache()?;
        let color_cache = color_cache_bits.map(|bits| ColorCache {
            color_cache_bits: bits,
            color_cache: vec![[0; 4]; 1 << bits],
        });

        let huffman_info = self.read_huffman_codes(is_argb_img, xsize, ysize, color_cache)?;
        self.decode_image_data(xsize, ysize, huffman_info, data)
    }

    fn decode_image_stream_rgb(
        &mut self,
        xsize: u16,
        ysize: u16,
        data: &mut [u8],
    ) -> Result<(), DecodingError> {
        self.decode_image_stream_with_pixel_size::<3>(xsize, ysize, true, data)
    }

    fn decode_image_stream_with_pixel_size<const PIXEL_SIZE: usize>(
        &mut self,
        xsize: u16,
        ysize: u16,
        is_argb_img: bool,
        data: &mut [u8],
    ) -> Result<(), DecodingError> {
        let color_cache_bits = self.read_color_cache()?;
        let color_cache = color_cache_bits.map(|bits| ColorCache {
            color_cache_bits: bits,
            color_cache: vec![[0; 4]; 1 << bits],
        });

        let huffman_info = self.read_huffman_codes(is_argb_img, xsize, ysize, color_cache)?;
        self.decode_image_data_with_pixel_size::<PIXEL_SIZE>(xsize, ysize, huffman_info, data)
    }

    /// Reads transforms and their data from the bitstream
    fn read_transforms(&mut self) -> Result<u16, DecodingError> {
        let mut xsize = self.width;

        while self.bit_reader.read_bits::<u8>(1)? == 1 {
            let transform_type_val = self.bit_reader.read_bits::<u8>(2)?;

            if self.transforms[usize::from(transform_type_val)].is_some() {
                //can only have one of each transform, error
                return Err(DecodingError::TransformError);
            }

            let transform_order_index = self.transform_order_len;
            self.transform_order[transform_order_index] = transform_type_val;
            self.transform_order_len += 1;

            let transform_type = match transform_type_val {
                0 => {
                    //predictor

                    let size_bits = self.bit_reader.read_bits::<u8>(3)? + 2;

                    let block_xsize =
                        ((u32::from(xsize) + (1u32 << size_bits) - 1) >> size_bits) as u16;
                    let block_ysize =
                        ((u32::from(self.height) + (1u32 << size_bits) - 1) >> size_bits) as u16;

                    let mut predictor_data =
                        vec![0; usize::from(block_xsize) * usize::from(block_ysize) * 4];
                    self.decode_image_stream(block_xsize, block_ysize, false, &mut predictor_data)?;

                    TransformType::PredictorTransform {
                        size_bits,
                        predictor_data,
                    }
                }
                1 => {
                    //color transform

                    let size_bits = self.bit_reader.read_bits::<u8>(3)? + 2;

                    let block_xsize =
                        ((u32::from(xsize) + (1u32 << size_bits) - 1) >> size_bits) as u16;
                    let block_ysize =
                        ((u32::from(self.height) + (1u32 << size_bits) - 1) >> size_bits) as u16;

                    let mut transform_data =
                        vec![0; usize::from(block_xsize) * usize::from(block_ysize) * 4];
                    self.decode_image_stream(block_xsize, block_ysize, false, &mut transform_data)?;

                    TransformType::ColorTransform {
                        size_bits,
                        transform_data,
                    }
                }
                2 => {
                    //subtract green

                    TransformType::SubtractGreen
                }
                _ => {
                    debug_assert_eq!(transform_type_val, 3);
                    let color_table_size = self.bit_reader.read_bits::<u16>(8)? + 1;
                    let table_bytes = usize::from(color_table_size) * 4;
                    let mut color_map = std::mem::replace(
                        &mut self.color_indexing_table,
                        [0; MAX_COLOR_INDEXING_TABLE_BYTES],
                    );
                    match self.decode_image_stream(
                        color_table_size,
                        1,
                        false,
                        &mut color_map[..table_bytes],
                    ) {
                        Ok(()) => {}
                        Err(error) => {
                            self.color_indexing_table = color_map;
                            return Err(error);
                        }
                    }

                    let bits = if color_table_size <= 2 {
                        3
                    } else if color_table_size <= 4 {
                        2
                    } else if color_table_size <= 16 {
                        1
                    } else {
                        0
                    };
                    xsize = ((u32::from(xsize) + (1u32 << bits) - 1) >> bits) as u16;

                    Self::adjust_color_map(&mut color_map[..table_bytes]);
                    self.color_indexing_table = color_map;

                    TransformType::ColorIndexingTransform {
                        table_size: color_table_size,
                    }
                }
            };

            self.transforms[usize::from(transform_type_val)] = Some(transform_type);
        }

        Ok(xsize)
    }

    /// Adjusts the color map since it's subtraction coded
    fn adjust_color_map(color_map: &mut [u8]) {
        for i in 4..color_map.len() {
            color_map[i] = color_map[i].wrapping_add(color_map[i - 4]);
        }
    }

    /// Reads huffman codes associated with an image
    fn read_huffman_codes(
        &mut self,
        read_meta: bool,
        xsize: u16,
        ysize: u16,
        color_cache: Option<ColorCache>,
    ) -> Result<HuffmanInfo, DecodingError> {
        let mut num_huff_groups = 1u32;

        let mut huffman_bits = 0;
        let mut huffman_xsize = 1;
        let mut huffman_ysize = 1;
        let mut entropy_image = Vec::new();

        if read_meta && self.bit_reader.read_bits::<u8>(1)? == 1 {
            //meta huffman codes
            huffman_bits = self.bit_reader.read_bits::<u8>(3)? + 2;
            huffman_xsize =
                ((u32::from(xsize) + (1u32 << huffman_bits) - 1) >> huffman_bits) as u16;
            huffman_ysize =
                ((u32::from(ysize) + (1u32 << huffman_bits) - 1) >> huffman_bits) as u16;

            // The decoded metadata image is four bytes per pixel, but only
            // its first two bytes become the retained Huffman-group index.
            // Keep one typed allocation, decode through its byte view, and
            // compact each selected u16 in place before returning it in
            // HuffmanInfo. This removes the transient byte Vec and the second
            // allocation/copy while preserving the source-byte interpretation.
            let pixel_count = usize::from(huffman_xsize) * usize::from(huffman_ysize);
            entropy_image.resize(pixel_count * 2, 0);
            {
                let data = bytemuck::cast_slice_mut::<u16, u8>(&mut entropy_image);
                self.decode_image_stream(huffman_xsize, huffman_ysize, false, data)?;
                for index in 0..pixel_count {
                    let source = index * 4;
                    let meta_huff_code = u16::from_be_bytes([data[source], data[source + 1]]);
                    let destination = index * 2;
                    data[destination..destination + 2]
                        .copy_from_slice(&meta_huff_code.to_ne_bytes());
                    if u32::from(meta_huff_code) >= num_huff_groups {
                        num_huff_groups = u32::from(meta_huff_code) + 1;
                    }
                }
            }
            entropy_image.truncate(pixel_count);
        }

        // The metadata image has already established the exact group count;
        // reserve the bounded group workspace once instead of growing it as
        // each Huffman group is parsed.
        let mut hufftree_groups = Vec::with_capacity(num_huff_groups as usize);
        // Reuse the format-bounded code-length workspace across the sequential
        // trees instead of allocating once per non-simple tree.
        let mut dynamic_code_lengths = [0u16; MAX_HUFFMAN_SYMBOLS_WITH_COLOR_CACHE];

        for _i in 0..num_huff_groups {
            let mut group: HuffmanCodeGroup = Default::default();
            for j in 0..HUFFMAN_CODES_PER_META_CODE {
                let mut alphabet_size = ALPHABET_SIZE[j];
                if j == 0
                    && let Some(color_cache) = color_cache.as_ref()
                {
                    alphabet_size += 1 << color_cache.color_cache_bits;
                }

                let tree = self.read_huffman_code(alphabet_size, &mut dynamic_code_lengths)?;
                group[j] = tree;
            }
            hufftree_groups.push(group);
        }

        let huffman_mask = if huffman_bits == 0 {
            !0
        } else {
            (1 << huffman_bits) - 1
        };

        let info = HuffmanInfo {
            xsize: huffman_xsize,
            _ysize: huffman_ysize,
            color_cache,
            image: entropy_image,
            bits: huffman_bits,
            mask: huffman_mask,
            huffman_code_groups: hufftree_groups,
        };

        Ok(info)
    }

    /// Decodes and returns a single huffman tree
    fn read_huffman_code(
        &mut self,
        alphabet_size: u16,
        dynamic_code_lengths: &mut [u16; MAX_HUFFMAN_SYMBOLS_WITH_COLOR_CACHE],
    ) -> Result<HuffmanTree, DecodingError> {
        let simple = self.bit_reader.read_bits::<u8>(1)? == 1;

        if simple {
            let num_symbols = self.bit_reader.read_bits::<u8>(1)? + 1;

            let is_first_8bits = self.bit_reader.read_bits::<u8>(1)?;
            let zero_symbol = self.bit_reader.read_bits::<u16>(1 + 7 * is_first_8bits)?;

            if zero_symbol >= alphabet_size {
                return Err(DecodingError::BitStreamError);
            }

            if num_symbols == 1 {
                Ok(HuffmanTree::build_single_node(zero_symbol))
            } else {
                let one_symbol = self.bit_reader.read_bits::<u16>(8)?;
                // libwebp accepts an out-of-range secondary symbol when the
                // corresponding branch is never selected by the image data.
                Ok(HuffmanTree::build_two_node(zero_symbol, one_symbol))
            }
        } else {
            let mut code_length_code_lengths = [0u16; CODE_LENGTH_CODES];

            let num_code_lengths = 4 + self.bit_reader.read_bits::<usize>(4)?;
            for i in 0..num_code_lengths {
                code_length_code_lengths[CODE_LENGTH_CODE_ORDER[i]] =
                    self.bit_reader.read_bits(3)?;
            }

            if usize::from(alphabet_size) <= MAX_STACK_HUFFMAN_SYMBOLS {
                let mut code_lengths = [0u16; MAX_STACK_HUFFMAN_SYMBOLS];
                self.read_huffman_code_lengths(
                    &code_length_code_lengths,
                    alphabet_size,
                    &mut code_lengths,
                )?;
                HuffmanTree::build_implicit(&code_lengths[..usize::from(alphabet_size)])
            } else {
                // A color-cache green alphabet can reach 2,328 symbols, which
                // is bounded by the format's 11-bit color-cache field. Reuse
                // this fixed workspace across the sequential trees; the
                // Huffman builder copies the lengths into its owned tree before
                // the scratch is used again.
                let dynamic_code_lengths = &mut dynamic_code_lengths[..usize::from(alphabet_size)];
                dynamic_code_lengths.fill(0);
                self.read_huffman_code_lengths(
                    &code_length_code_lengths,
                    alphabet_size,
                    dynamic_code_lengths,
                )?;
                HuffmanTree::build_implicit(dynamic_code_lengths)
            }
        }
    }

    /// Reads huffman code lengths
    fn read_huffman_code_lengths(
        &mut self,
        code_length_code_lengths: &[u16],
        num_symbols: u16,
        code_lengths: &mut [u16],
    ) -> Result<(), DecodingError> {
        let table = HuffmanTree::build_implicit(code_length_code_lengths)?;

        let mut max_symbol = if self.bit_reader.read_bits::<u8>(1)? == 1 {
            let length_nbits = 2 + 2 * self.bit_reader.read_bits::<u8>(3)?;
            let max_minus_two = self.bit_reader.read_bits::<u16>(length_nbits)?;
            if max_minus_two > num_symbols - 2 {
                return Err(DecodingError::BitStreamError);
            }
            2 + max_minus_two
        } else {
            num_symbols
        };

        let mut prev_code_len = 8; //default code length

        let mut symbol = 0;
        while symbol < num_symbols {
            if max_symbol == 0 {
                break;
            }
            max_symbol -= 1;

            self.bit_reader.fill()?;
            let code_len = table.read_symbol(&mut self.bit_reader)?;

            if code_len < 16 {
                code_lengths[usize::from(symbol)] = code_len;
                symbol += 1;
                if code_len != 0 {
                    prev_code_len = code_len;
                }
            } else {
                let use_prev = code_len == 16;
                let slot = code_len - 16;
                let extra_bits = match slot {
                    0 => 2,
                    1 => 3,
                    _ => {
                        debug_assert_eq!(slot, 2);
                        7
                    }
                };
                let repeat_offset = match slot {
                    0 | 1 => 3,
                    _ => 11,
                };

                let mut repeat = self.bit_reader.read_bits::<u16>(extra_bits)? + repeat_offset;

                if symbol + repeat > num_symbols {
                    return Err(DecodingError::BitStreamError);
                }

                let length = if use_prev { prev_code_len } else { 0 };
                while repeat > 0 {
                    repeat -= 1;
                    code_lengths[usize::from(symbol)] = length;
                    symbol += 1;
                }
            }
        }

        Ok(())
    }

    /// Decodes the image data using the huffman trees and either of the 3 methods of decoding
    // All converted pixel slices are exact four-byte chunks established by
    // validated copy bounds.
    #[allow(
        clippy::unwrap_in_result,
        clippy::unwrap_used,
        reason = "Validated copy bounds make every decoded RGBA output sample an exact four-byte chunk."
    )]
    fn decode_image_data(
        &mut self,
        width: u16,
        height: u16,
        huffman_info: HuffmanInfo,
        data: &mut [u8],
    ) -> Result<(), DecodingError> {
        self.decode_image_data_with_pixel_size::<4>(width, height, huffman_info, data)
    }

    /// Decodes image data with either the ordinary RGBA or the direct RGB
    /// pixel width selected by the caller.
    #[allow(
        clippy::unwrap_in_result,
        clippy::unwrap_used,
        reason = "The checked pixel count bounds writes to the caller-sized RGB or RGBA output buffer."
    )]
    fn decode_image_data_with_pixel_size<const PIXEL_SIZE: usize>(
        &mut self,
        width: u16,
        height: u16,
        mut huffman_info: HuffmanInfo,
        data: &mut [u8],
    ) -> Result<(), DecodingError> {
        let num_values = usize::from(width) * usize::from(height);

        let huff_index = huffman_info.get_huff_index(0, 0);
        let mut tree = &huffman_info.huffman_code_groups[huff_index];
        let mut index = 0;

        let mut next_block_start = 0;
        while index < num_values {
            self.bit_reader.fill()?;

            if index >= next_block_start {
                let x = index % usize::from(width);
                let y = index / usize::from(width);
                next_block_start = (x | usize::from(huffman_info.mask)).min(usize::from(width - 1))
                    + y * usize::from(width)
                    + 1;

                let huff_index = huffman_info.get_huff_index(x as u16, y as u16);
                tree = &huffman_info.huffman_code_groups[huff_index];

                // Fast path: If all the codes each contain only a single
                // symbol, then the pixel data isn't written to the bitstream
                // and we can just fill the output buffer with the symbol
                // directly.
                if let (Some(code), Some(red), Some(blue), Some(alpha)) = (
                    tree[GREEN].single_symbol(),
                    tree[RED].single_symbol(),
                    tree[BLUE].single_symbol(),
                    tree[ALPHA].single_symbol(),
                ) && code < 256
                {
                    let n = if huffman_info.bits == 0 {
                        num_values
                    } else {
                        next_block_start - index
                    };

                    let value = [red as u8, code as u8, blue as u8, alpha as u8];

                    for i in 0..n {
                        write_pixel::<PIXEL_SIZE>(data, index + i, value);
                    }

                    if let Some(color_cache) = huffman_info.color_cache.as_mut() {
                        color_cache.insert(value);
                    }

                    index += n;
                    continue;
                }
            }

            let code = tree[GREEN].read_symbol(&mut self.bit_reader)?;

            //check code
            if code < 256 {
                //literal, so just use huffman codes and read as argb
                let green = code as u8;
                let red = tree[RED].read_symbol(&mut self.bit_reader)? as u8;
                let blue = tree[BLUE].read_symbol(&mut self.bit_reader)? as u8;
                if self.bit_reader.nbits < 15 {
                    self.bit_reader.fill()?;
                }
                let alpha = tree[ALPHA].read_symbol(&mut self.bit_reader)? as u8;

                write_pixel::<PIXEL_SIZE>(data, index, [red, green, blue, alpha]);

                if let Some(color_cache) = huffman_info.color_cache.as_mut() {
                    color_cache.insert([red, green, blue, alpha]);
                }
                index += 1;
            } else if code < 256 + 24 {
                //backward reference, so go back and use that to add image data
                let length_symbol = code - 256;
                let length = Self::get_copy_distance(&mut self.bit_reader, length_symbol)?;

                let dist_symbol = tree[DIST].read_symbol(&mut self.bit_reader)?;
                let dist_code = Self::get_copy_distance(&mut self.bit_reader, dist_symbol)?;
                let dist = Self::plane_code_to_distance(width, dist_code);

                if copy_is_out_of_bounds(index, dist, num_values, length) {
                    return Err(DecodingError::BitStreamError);
                }

                if dist == 1 {
                    let value = read_pixel::<PIXEL_SIZE>(data, index - dist);
                    for i in 0..length {
                        write_pixel::<PIXEL_SIZE>(data, index + i, value);
                    }
                } else {
                    let pixel_bytes = PIXEL_SIZE;
                    let copy_chunk_bytes = pixel_bytes * 4;
                    if index + length + 3 <= num_values {
                        let start = (index - dist) * pixel_bytes;
                        data.copy_within(start..start + copy_chunk_bytes, index * pixel_bytes);

                        if copy_needs_overlap_expansion(length, dist) {
                            for i in (0..length * pixel_bytes)
                                .step_by((dist * pixel_bytes).min(copy_chunk_bytes))
                                .skip(1)
                            {
                                data.copy_within(
                                    start + i..start + i + copy_chunk_bytes,
                                    index * pixel_bytes + i,
                                );
                            }
                        }
                    } else {
                        for i in 0..length * pixel_bytes {
                            data[index * pixel_bytes + i] =
                                data[index * pixel_bytes + i - dist * pixel_bytes];
                        }
                    }

                    if let Some(color_cache) = huffman_info.color_cache.as_mut() {
                        for pixel in data[index * pixel_bytes..][..length * pixel_bytes]
                            .chunks_exact(pixel_bytes)
                        {
                            color_cache.insert(pixel_to_rgba(pixel));
                        }
                    }
                }
                index += length;
            } else {
                //color cache, so use previously stored pixels to get this pixel
                let color_cache = huffman_info
                    .color_cache
                    .as_mut()
                    .ok_or(DecodingError::BitStreamError)?;
                let color = color_cache.lookup((code - 280).into());
                write_pixel::<PIXEL_SIZE>(data, index, color);
                index += 1;

                if index < next_block_start
                    && let Some((bits, code)) = tree[GREEN].peek_symbol(&self.bit_reader)
                    && code >= 280
                {
                    self.bit_reader.consume(bits)?;
                    write_pixel::<PIXEL_SIZE>(data, index, color_cache.lookup((code - 280).into()));
                    index += 1;
                }
            }
        }

        Ok(())
    }

    /// Reads color cache data from the bitstream
    fn read_color_cache(&mut self) -> Result<Option<u8>, DecodingError> {
        if self.bit_reader.read_bits::<u8>(1)? == 1 {
            let code_bits = self.bit_reader.read_bits::<u8>(4)?;

            if !(1..=MAX_COLOR_CACHE_BITS).contains(&code_bits) {
                return Err(DecodingError::InvalidColorCacheBits);
            }

            Ok(Some(code_bits))
        } else {
            Ok(None)
        }
    }

    /// Gets the copy distance from the prefix code and bitstream.
    // VP8L prefix symbols bound the derived extra-bit count to `u8`.
    #[allow(
        clippy::unwrap_in_result,
        clippy::unwrap_used,
        reason = "VP8L distance-prefix symbols keep the derived extra-bit count within u8."
    )]
    fn get_copy_distance(
        bit_reader: &mut BitReader<Box<dyn BufRead + 'a>>,
        prefix_code: u16,
    ) -> Result<usize, DecodingError> {
        if prefix_code < 4 {
            return Ok(usize::from(prefix_code + 1));
        }
        let extra_bits: u8 = ((prefix_code - 2) >> 1).try_into().unwrap();
        let offset = (2 + (usize::from(prefix_code) & 1)) << extra_bits;

        let bits = bit_reader.peek(extra_bits) as usize;
        bit_reader.consume(extra_bits)?;

        Ok(offset + bits + 1)
    }

    /// Gets distance to pixel.
    // The negative/zero case returns above, so the retained distance is
    // representable as `usize` on every supported target.
    #[allow(
        clippy::unwrap_used,
        reason = "The distance-map branch returns for nonpositive values before converting the bounded positive distance."
    )]
    fn plane_code_to_distance(xsize: u16, plane_code: usize) -> usize {
        if plane_code > 120 {
            plane_code - 120
        } else {
            let (xoffset, yoffset) = DISTANCE_MAP[plane_code - 1];

            let dist = i32::from(xoffset) + i32::from(yoffset) * i32::from(xsize);
            if dist < 1 {
                return 1;
            }
            dist.try_into().unwrap()
        }
    }
}

// `PIXEL_SIZE` is a closed internal const parameter: all legal callers use
// RGB (3) or RGBA (4).  The assertion's false branch is therefore not a
// reachable codec state, even though LLVM models the generic assertion as a
// branch for every monomorphization.
#[inline]
fn write_pixel<const PIXEL_SIZE: usize>(data: &mut [u8], index: usize, pixel: [u8; 4]) {
    debug_assert!(PIXEL_SIZE == 3 || PIXEL_SIZE == 4);
    let start = index * PIXEL_SIZE;
    data[start..start + PIXEL_SIZE].copy_from_slice(&pixel[..PIXEL_SIZE]);
}

#[inline]
fn read_pixel<const PIXEL_SIZE: usize>(data: &[u8], index: usize) -> [u8; 4] {
    let start = index * PIXEL_SIZE;
    pixel_to_rgba(&data[start..start + PIXEL_SIZE])
}

// The same closed internal representation reaches this helper: only 3-byte
// RGB and 4-byte RGBA slices are produced by the decoder.  The invalid-length
// assertion branch is not an executable codec state.
#[inline]
fn pixel_to_rgba(pixel: &[u8]) -> [u8; 4] {
    debug_assert!(pixel.len() == 3 || pixel.len() == 4);
    let mut rgba = [0, 0, 0, 255];
    rgba[..pixel.len()].copy_from_slice(pixel);
    rgba
}

fn copy_is_out_of_bounds(index: usize, dist: usize, num_values: usize, length: usize) -> bool {
    index < dist || num_values - index < length
}

fn copy_needs_overlap_expansion(length: usize, dist: usize) -> bool {
    length > 4 || dist < 4
}

#[derive(Debug, Clone)]
struct HuffmanInfo {
    xsize: u16,
    _ysize: u16,
    color_cache: Option<ColorCache>,
    image: Vec<u16>,
    bits: u8,
    mask: u16,
    huffman_code_groups: Vec<HuffmanCodeGroup>,
}

impl HuffmanInfo {
    fn get_huff_index(&self, x: u16, y: u16) -> usize {
        if self.bits == 0 {
            return 0;
        }
        let position =
            usize::from(y >> self.bits) * usize::from(self.xsize) + usize::from(x >> self.bits);
        let meta_huff_code: usize = usize::from(self.image[position]);
        meta_huff_code
    }
}

#[derive(Debug, Clone)]
struct ColorCache {
    color_cache_bits: u8,
    color_cache: Vec<[u8; 4]>,
}

impl ColorCache {
    #[inline(always)]
    fn insert(&mut self, color: [u8; 4]) {
        let [r, g, b, a] = color;
        let color_u32 =
            (u32::from(r) << 16) | (u32::from(g) << 8) | (u32::from(b)) | (u32::from(a) << 24);
        let index = (0x1e35a7bdu32.wrapping_mul(color_u32)) >> (32 - self.color_cache_bits);
        self.color_cache[index as usize] = color;
    }

    #[inline(always)]
    fn lookup(&self, index: usize) -> [u8; 4] {
        self.color_cache[index]
    }
}

#[derive(Debug, Clone)]
pub(crate) struct BitReader<R> {
    reader: R,
    buffer: u64,
    nbits: u8,
}

// This branch is entered only when `fill_buf()` exposes at least eight bytes.
#[allow(
    clippy::unwrap_in_result,
    clippy::unwrap_used,
    reason = "The eight-byte conversion follows the explicit fill_buf length guard."
)]
fn fill_bit_buffer(
    reader: &mut dyn BufRead,
    buffer: &mut u64,
    nbits: &mut u8,
) -> Result<(), DecodingError> {
    let mut buf = reader.fill_buf()?;
    if buf.len() >= 8 {
        let lookahead = u64::from_le_bytes(buf[..8].try_into().unwrap());
        reader.consume(usize::from((63 - *nbits) / 8));
        *buffer |= lookahead << *nbits;
        *nbits |= 56;
    } else {
        while !buf.is_empty() && *nbits < 56 {
            *buffer |= u64::from(buf[0]) << *nbits;
            *nbits += 8;
            reader.consume(1);
            buf = reader.fill_buf()?;
        }
    }

    Ok(())
}

fn consume_bits(buffer: &mut u64, nbits: &mut u8, num: u8) -> Result<(), DecodingError> {
    if *nbits < num {
        return Err(DecodingError::BitStreamError);
    }

    *buffer >>= num;
    *nbits -= num;
    Ok(())
}

fn read_bits_u32(
    reader: &mut dyn BufRead,
    buffer: &mut u64,
    nbits: &mut u8,
    num: u8,
) -> Result<u32, DecodingError> {
    if *nbits < num {
        fill_bit_buffer(reader, buffer, nbits)?;
    }
    let value = (*buffer & ((1 << num) - 1)) as u32;
    consume_bits(buffer, nbits, num)?;
    Ok(value)
}

impl<R: BufRead> BitReader<R> {
    const fn new(reader: R) -> Self {
        Self {
            reader,
            buffer: 0,
            nbits: 0,
        }
    }

    /// Fills the buffer with bits from the input stream.
    ///
    /// After this function, the internal buffer will contain 64-bits or have reached the end of
    /// the input stream.
    pub(crate) fn fill(&mut self) -> Result<(), DecodingError> {
        debug_assert!(self.nbits < 64);
        fill_bit_buffer(&mut self.reader, &mut self.buffer, &mut self.nbits)
    }

    /// Peeks at the next `num` bits in the buffer.
    pub(crate) const fn peek(&self, num: u8) -> u64 {
        self.buffer & ((1 << num) - 1)
    }

    /// Peeks at the full buffer.
    pub(crate) const fn peek_full(&self) -> u64 {
        self.buffer
    }

    /// Consumes `num` bits from the buffer returning an error if there are not enough bits.
    pub(crate) fn consume(&mut self, num: u8) -> Result<(), DecodingError> {
        consume_bits(&mut self.buffer, &mut self.nbits, num)
    }

    /// Convenience function to read a number of bits and convert them to a type.
    fn read_bits<T: LosslessBitValue>(&mut self, num: u8) -> Result<T, DecodingError> {
        debug_assert!(num <= T::BITS);
        debug_assert!(num <= 32);

        read_bits_u32(&mut self.reader, &mut self.buffer, &mut self.nbits, num).map(T::from_u32)
    }
}

trait LosslessBitValue {
    const BITS: u8;

    fn from_u32(value: u32) -> Self;
}

macro_rules! impl_lossless_bit_value {
    ($type:ty) => {
        impl LosslessBitValue for $type {
            const BITS: u8 = <$type>::BITS as u8;

            fn from_u32(value: u32) -> Self {
                value as Self
            }
        }
    };
}

impl_lossless_bit_value!(u8);
impl_lossless_bit_value!(u16);
impl_lossless_bit_value!(usize);
