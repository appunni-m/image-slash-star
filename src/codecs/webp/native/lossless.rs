//! Decoding of lossless WebP images
//!
//! [Lossless spec](https://developers.google.com/speed/webp/docs/webp_lossless_bitstream_specification)

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

const NUM_TRANSFORM_TYPES: usize = 4;

//Decodes lossless WebP images
pub(crate) struct LosslessDecoder<'a> {
    bit_reader: BitReader<Box<dyn BufRead + 'a>>,
    transforms: [Option<TransformType>; NUM_TRANSFORM_TYPES],
    transform_order: Vec<u8>,
    width: u16,
    height: u16,
}

impl<'a> LosslessDecoder<'a> {
    /// Create a new decoder
    pub(crate) fn new(r: Box<dyn BufRead + 'a>) -> Self {
        Self {
            bit_reader: BitReader::new(r),
            transforms: [None, None, None, None],
            transform_order: Vec::new(),
            width: 0,
            height: 0,
        }
    }

    /// Decodes a VP8L frame whose payload includes the VP8L signature and
    /// dimension header.
    pub(crate) fn decode_frame(
        &mut self,
        width: u32,
        height: u32,
        buf: &mut [u8],
    ) -> Result<(), DecodingError> {
        self.width = width as u16;
        self.height = height as u16;

        let signature = self.bit_reader.read_bits::<u8>(8)?;
        debug_assert_eq!(signature, 0x2f);

        self.width = self.bit_reader.read_bits::<u16>(14)? + 1;
        self.height = self.bit_reader.read_bits::<u16>(14)? + 1;
        debug_assert_eq!(u32::from(self.width), width);
        debug_assert_eq!(u32::from(self.height), height);

        let _alpha_used = self
            .bit_reader
            .read_bits::<u8>(1)
            .expect("VP8L height read success proves the alpha bit is buffered");
        let version_num = self
            .bit_reader
            .read_bits::<u8>(3)
            .expect("VP8L height read success proves the version bits are buffered");
        debug_assert_eq!(version_num, 0);

        self.decode_frame_body(buf)
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

    fn decode_frame_body(&mut self, buf: &mut [u8]) -> Result<(), DecodingError> {
        let transformed_width = self.read_transforms()?;
        let transformed_size = usize::from(transformed_width) * usize::from(self.height) * 4;
        self.decode_image_stream(
            transformed_width,
            self.height,
            true,
            &mut buf[..transformed_size],
        )?;

        let mut image_size = transformed_size;
        let mut width = transformed_width;
        for &trans_index in self.transform_order.iter().rev() {
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
                TransformType::ColorIndexingTransform {
                    table_size,
                    table_data,
                } => {
                    width = self.width;
                    image_size = usize::from(width) * usize::from(self.height) * 4;
                    apply_color_indexing_transform(
                        buf,
                        width,
                        self.height,
                        *table_size,
                        table_data,
                    );
                }
            }
        }

        Ok(())
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

    /// Reads transforms and their data from the bitstream
    fn read_transforms(&mut self) -> Result<u16, DecodingError> {
        let mut xsize = self.width;

        while self.bit_reader.read_bits::<u8>(1)? == 1 {
            let transform_type_val = self.bit_reader.read_bits::<u8>(2)?;

            if self.transforms[usize::from(transform_type_val)].is_some() {
                //can only have one of each transform, error
                return Err(DecodingError::TransformError);
            }

            self.transform_order.push(transform_type_val);

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

                    let mut color_map = vec![0; usize::from(color_table_size) * 4];
                    self.decode_image_stream(color_table_size, 1, false, &mut color_map)?;

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

                    Self::adjust_color_map(&mut color_map);

                    TransformType::ColorIndexingTransform {
                        table_size: color_table_size,
                        table_data: color_map,
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

            let mut data = vec![0; usize::from(huffman_xsize) * usize::from(huffman_ysize) * 4];
            self.decode_image_stream(huffman_xsize, huffman_ysize, false, &mut data)?;

            entropy_image = data
                .chunks_exact(4)
                .map(|pixel| {
                    let meta_huff_code = (u16::from(pixel[0]) << 8) | u16::from(pixel[1]);
                    if u32::from(meta_huff_code) >= num_huff_groups {
                        num_huff_groups = u32::from(meta_huff_code) + 1;
                    }
                    meta_huff_code
                })
                .collect::<Vec<u16>>();
        }

        let mut hufftree_groups = Vec::new();

        for _i in 0..num_huff_groups {
            let mut group: HuffmanCodeGroup = Default::default();
            for j in 0..HUFFMAN_CODES_PER_META_CODE {
                let mut alphabet_size = ALPHABET_SIZE[j];
                if j == 0 {
                    if let Some(color_cache) = color_cache.as_ref() {
                        alphabet_size += 1 << color_cache.color_cache_bits;
                    }
                }

                let tree = self.read_huffman_code(alphabet_size)?;
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
    fn read_huffman_code(&mut self, alphabet_size: u16) -> Result<HuffmanTree, DecodingError> {
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
            let mut code_length_code_lengths = vec![0; CODE_LENGTH_CODES];

            let num_code_lengths = 4 + self.bit_reader.read_bits::<usize>(4)?;
            for i in 0..num_code_lengths {
                code_length_code_lengths[CODE_LENGTH_CODE_ORDER[i]] =
                    self.bit_reader.read_bits(3)?;
            }

            let new_code_lengths =
                self.read_huffman_code_lengths(code_length_code_lengths, alphabet_size)?;

            HuffmanTree::build_implicit(new_code_lengths)
        }
    }

    /// Reads huffman code lengths
    fn read_huffman_code_lengths(
        &mut self,
        code_length_code_lengths: Vec<u16>,
        num_symbols: u16,
    ) -> Result<Vec<u16>, DecodingError> {
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

        let mut code_lengths = vec![0; usize::from(num_symbols)];
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

        Ok(code_lengths)
    }

    /// Decodes the image data using the huffman trees and either of the 3 methods of decoding
    fn decode_image_data(
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
                ) {
                    if code < 256 {
                        let n = if huffman_info.bits == 0 {
                            num_values
                        } else {
                            next_block_start - index
                        };

                        let value = [red as u8, code as u8, blue as u8, alpha as u8];

                        for i in 0..n {
                            data[index * 4 + i * 4..][..4].copy_from_slice(&value);
                        }

                        if let Some(color_cache) = huffman_info.color_cache.as_mut() {
                            color_cache.insert(value);
                        }

                        index += n;
                        continue;
                    }
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

                data[index * 4] = red;
                data[index * 4 + 1] = green;
                data[index * 4 + 2] = blue;
                data[index * 4 + 3] = alpha;

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
                    let value: [u8; 4] = data[(index - dist) * 4..][..4].try_into().unwrap();
                    for i in 0..length {
                        data[index * 4 + i * 4..][..4].copy_from_slice(&value);
                    }
                } else {
                    if index + length + 3 <= num_values {
                        let start = (index - dist) * 4;
                        data.copy_within(start..start + 16, index * 4);

                        if copy_needs_overlap_expansion(length, dist) {
                            for i in (0..length * 4).step_by((dist * 4).min(16)).skip(1) {
                                data.copy_within(start + i..start + i + 16, index * 4 + i);
                            }
                        }
                    } else {
                        for i in 0..length * 4 {
                            data[index * 4 + i] = data[index * 4 + i - dist * 4];
                        }
                    }

                    if let Some(color_cache) = huffman_info.color_cache.as_mut() {
                        for pixel in data[index * 4..][..length * 4].chunks_exact(4) {
                            color_cache.insert(pixel.try_into().unwrap());
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
                data[index * 4..][..4].copy_from_slice(&color);
                index += 1;

                if index < next_block_start {
                    if let Some((bits, code)) = tree[GREEN].peek_symbol(&self.bit_reader) {
                        if code >= 280 {
                            self.bit_reader.consume(bits)?;
                            data[index * 4..][..4]
                                .copy_from_slice(&color_cache.lookup((code - 280).into()));
                            index += 1;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Reads color cache data from the bitstream
    fn read_color_cache(&mut self) -> Result<Option<u8>, DecodingError> {
        if self.bit_reader.read_bits::<u8>(1)? == 1 {
            let code_bits = self.bit_reader.read_bits::<u8>(4)?;

            if !(1..=11).contains(&code_bits) {
                return Err(DecodingError::InvalidColorCacheBits);
            }

            Ok(Some(code_bits))
        } else {
            Ok(None)
        }
    }

    /// Gets the copy distance from the prefix code and bitstream.
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

fn copy_is_out_of_bounds(index: usize, dist: usize, num_values: usize, length: usize) -> bool {
    index < dist || num_values - index < length
}

fn copy_needs_overlap_expansion(length: usize, dist: usize) -> bool {
    length > 4 || dist < 4
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    use std::io::{self, BufRead, Cursor, Read};

    struct ErrorReader;

    impl Read for ErrorReader {
        fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::from(io::ErrorKind::Other))
        }
    }

    impl BufRead for ErrorReader {
        fn fill_buf(&mut self) -> io::Result<&[u8]> {
            Err(io::Error::from(io::ErrorKind::Other))
        }

        fn consume(&mut self, _amt: usize) {}
    }

    struct OneThenErrorReader {
        byte: [u8; 1],
        consumed: bool,
    }

    struct OneThenEofThenErrorReader {
        byte: [u8; 1],
        phase: u8,
    }

    impl Read for OneThenErrorReader {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.consumed {
                return Err(io::Error::from(io::ErrorKind::Other));
            }
            if buf.is_empty() {
                return Ok(0);
            }
            buf[0] = self.byte[0];
            self.consumed = true;
            Ok(1)
        }
    }

    impl Read for OneThenEofThenErrorReader {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            match self.phase {
                0 if !buf.is_empty() => {
                    buf[0] = self.byte[0];
                    self.phase = 1;
                    Ok(1)
                }
                0 => Ok(0),
                1 => {
                    self.phase = 2;
                    Ok(0)
                }
                _ => Err(io::Error::from(io::ErrorKind::Other)),
            }
        }
    }

    impl BufRead for OneThenErrorReader {
        fn fill_buf(&mut self) -> io::Result<&[u8]> {
            if self.consumed {
                Err(io::Error::from(io::ErrorKind::Other))
            } else {
                Ok(&self.byte)
            }
        }

        fn consume(&mut self, _amt: usize) {
            self.consumed = true;
        }
    }

    impl BufRead for OneThenEofThenErrorReader {
        fn fill_buf(&mut self) -> io::Result<&[u8]> {
            match self.phase {
                0 => Ok(&self.byte),
                1 => {
                    self.phase = 2;
                    Ok(&[])
                }
                _ => Err(io::Error::from(io::ErrorKind::Other)),
            }
        }

        fn consume(&mut self, amt: usize) {
            if amt != 0 && self.phase == 0 {
                self.phase = 1;
            }
        }
    }

    let make_color_cache = || {
        Some(ColorCache {
            color_cache_bits: 1,
            color_cache: vec![[3, 5, 7, 255]; 2],
        })
    };
    fn decoder_with_bits(
        reader: Box<dyn BufRead>,
        buffer: u64,
        nbits: u8,
        width: u16,
        height: u16,
    ) -> LosslessDecoder<'static> {
        LosslessDecoder {
            bit_reader: BitReader {
                reader,
                buffer,
                nbits,
            },
            transforms: [None, None, None, None],
            transform_order: Vec::new(),
            width,
            height,
        }
    }

    let mut scratch = [0u8; 1];
    let mut error_reader = ErrorReader;
    let _ = error_reader.read(&mut scratch);
    error_reader.consume(0);
    let mut one_then_error = OneThenErrorReader {
        byte: [0x55],
        consumed: false,
    };
    let _ = one_then_error.read(&mut []);
    let _ = one_then_error.read(&mut scratch);
    let _ = one_then_error.read(&mut scratch);
    let mut one_then_eof_then_error = OneThenEofThenErrorReader {
        byte: [0xaa],
        phase: 0,
    };
    one_then_eof_then_error.consume(0);
    let _ = one_then_eof_then_error.read(&mut []);
    let _ = one_then_eof_then_error.read(&mut scratch);
    let _ = one_then_eof_then_error.read(&mut scratch);
    let _ = one_then_eof_then_error.read(&mut scratch);
    one_then_eof_then_error.consume(1);

    let mut decoder = LosslessDecoder::new(Box::new(std::io::Cursor::new(Vec::<u8>::new())));
    let mut buf = [0u8; 4];
    let _ = decoder.decode_frame_implicit_dimensions(1, 1, &mut buf);
    let mut decoder = LosslessDecoder::new(Box::new(std::io::Cursor::new(vec![0x2f])));
    let _ = decoder.decode_frame(1, 1, &mut buf);
    let mut decoder = LosslessDecoder::new(Box::new(std::io::Cursor::new(vec![0x2f, 0, 0])));
    let _ = decoder.decode_frame(1, 1, &mut buf);
    let mut decoder = LosslessDecoder::new(Box::new(std::io::Cursor::new(vec![0x2f, 0, 0, 0, 0])));
    let _ = decoder.decode_frame(1, 1, &mut buf);

    let mut decoder = decoder_with_bits(Box::new(ErrorReader), 0b001, 3, 1, 1);
    let _ = decoder.read_transforms();
    let mut decoder = decoder_with_bits(Box::new(ErrorReader), 0b011, 3, 1, 1);
    let _ = decoder.read_transforms();
    let mut decoder = decoder_with_bits(Box::new(ErrorReader), 0b1, 1, 1, 1);
    let _ = decoder.read_transforms();

    let mut decoder = decoder_with_bits(Box::new(ErrorReader), 0, 0, 1, 1);
    let _ = decoder.read_huffman_codes(true, 1, 1, None);
    let mut decoder = decoder_with_bits(Box::new(ErrorReader), 0b1, 1, 1, 1);
    let _ = decoder.read_huffman_codes(true, 1, 1, None);

    let _ = copy_is_out_of_bounds(0, 1, 2, 1);
    let _ = copy_is_out_of_bounds(1, 1, 2, 2);
    let _ = copy_is_out_of_bounds(1, 1, 2, 1);
    let _ = copy_needs_overlap_expansion(5, 4);
    let _ = copy_needs_overlap_expansion(4, 3);
    let _ = copy_needs_overlap_expansion(4, 4);

    let mut code_length_code_lengths = vec![0; CODE_LENGTH_CODES];
    code_length_code_lengths[0] = 1;
    code_length_code_lengths[1] = 1;
    let mut decoder = decoder_with_bits(Box::new(ErrorReader), 0, 1, 1, 1);
    let _ = decoder.read_huffman_code_lengths(code_length_code_lengths, 4);

    let mut reader = BitReader::__coverage_new(Cursor::new([0u8; 8]));
    let _ = reader.fill();
    let _ = reader.consume(1);
    let _: Result<u8, _> = reader.read_bits(1);
    let mut reader = BitReader::__coverage_new(ErrorReader);
    let _ = reader.fill();
    let _: Result<u8, _> = reader.read_bits(1);
    let mut reader = BitReader::__coverage_new(OneThenErrorReader {
        byte: [0],
        consumed: false,
    });
    let _ = reader.fill();

    let mut reader = BitReader::__coverage_new(Cursor::new([0u8; 1]));
    let _ = reader.fill();
    let _ = reader.consume(8);
    let _ = reader.consume(1);

    let mut decoder = LosslessDecoder::new(Box::new(Cursor::new(vec![0u8; 1])));
    let _ = decoder.read_color_cache();
    let mut decoder = LosslessDecoder::new(Box::new(Cursor::new(vec![0xff; 1])));
    let _ = decoder.read_color_cache();
    let mut decoder = LosslessDecoder {
        bit_reader: BitReader {
            reader: Box::new(Cursor::new(Vec::<u8>::new())),
            buffer: 1,
            nbits: 1,
        },
        transforms: [None, None, None, None],
        transform_order: Vec::new(),
        width: 1,
        height: 1,
    };
    let _ = decoder.read_color_cache();
    let mut decoder = LosslessDecoder {
        bit_reader: BitReader {
            reader: Box::new(Cursor::new(Vec::<u8>::new())),
            buffer: 0b00011,
            nbits: 5,
        },
        transforms: [None, None, None, None],
        transform_order: Vec::new(),
        width: 1,
        height: 1,
    };
    let _ = decoder.read_color_cache();

    let mut reader =
        BitReader::__coverage_new(Box::new(Cursor::new([0b1010_1010u8; 8])) as Box<dyn BufRead>);
    let _ = reader.fill();
    let _ = LosslessDecoder::<'static>::get_copy_distance(&mut reader, 4);
    let mut reader = BitReader {
        reader: Box::new(Cursor::new(Vec::<u8>::new())) as Box<dyn BufRead>,
        buffer: 0,
        nbits: 0,
    };
    let _ = LosslessDecoder::<'static>::get_copy_distance(&mut reader, 4);
    let _ = LosslessDecoder::<'static>::plane_code_to_distance(1, 121);
    let _ = LosslessDecoder::<'static>::plane_code_to_distance(8, 1);
    let _ = LosslessDecoder::<'static>::plane_code_to_distance(8, 8);

    let info = HuffmanInfo {
        xsize: 1,
        _ysize: 1,
        color_cache: None,
        image: vec![0],
        bits: 0,
        mask: 0,
        huffman_code_groups: Vec::new(),
    };
    let _ = info.get_huff_index(0, 0);
    let info = HuffmanInfo {
        xsize: 1,
        _ysize: 1,
        color_cache: None,
        image: vec![3],
        bits: 1,
        mask: 1,
        huffman_code_groups: Vec::new(),
    };
    let _ = info.get_huff_index(1, 0);

    let mut decoder = decoder_with_bits(Box::new(ErrorReader), 0, 0, 1, 1);
    let mut data = [0u8; 4];
    let huffman_info = HuffmanInfo {
        xsize: 1,
        _ysize: 1,
        color_cache: None,
        image: vec![0],
        bits: 0,
        mask: 0,
        huffman_code_groups: vec![[
            HuffmanTree::build_single_node(7),
            HuffmanTree::build_single_node(11),
            HuffmanTree::build_single_node(13),
            HuffmanTree::build_two_node(255, 255),
            HuffmanTree::build_single_node(0),
        ]],
    };
    let _ = decoder.decode_image_data(1, 1, huffman_info, &mut data);

    let mut decoder = decoder_with_bits(
        Box::new(OneThenEofThenErrorReader {
            byte: [0],
            phase: 0,
        }),
        0,
        0,
        1,
        1,
    );
    let mut data = [0u8; 4];
    let huffman_info = HuffmanInfo {
        xsize: 1,
        _ysize: 1,
        color_cache: None,
        image: vec![0],
        bits: 0,
        mask: 0,
        huffman_code_groups: vec![[
            HuffmanTree::build_single_node(7),
            HuffmanTree::build_single_node(11),
            HuffmanTree::build_single_node(13),
            HuffmanTree::build_two_node(255, 255),
            HuffmanTree::build_single_node(0),
        ]],
    };
    let _ = decoder.decode_image_data(1, 1, huffman_info, &mut data);

    let mut decoder = LosslessDecoder::new(Box::new(Cursor::new(vec![0u8; 1])));
    let mut data = [0u8; 8];
    let huffman_info = HuffmanInfo {
        xsize: 1,
        _ysize: 1,
        color_cache: make_color_cache(),
        image: vec![0],
        bits: 1,
        mask: 1,
        huffman_code_groups: vec![[
            HuffmanTree::build_single_node(7),
            HuffmanTree::build_single_node(11),
            HuffmanTree::build_single_node(13),
            HuffmanTree::build_single_node(255),
            HuffmanTree::build_single_node(0),
        ]],
    };
    let _ = decoder.decode_image_data(2, 1, huffman_info, &mut data);

    let mut decoder = LosslessDecoder::new(Box::new(Cursor::new(vec![0u8; 1])));
    let mut data = [0u8; 4];
    let huffman_info = HuffmanInfo {
        xsize: 1,
        _ysize: 1,
        color_cache: make_color_cache(),
        image: vec![0],
        bits: 0,
        mask: 0,
        huffman_code_groups: vec![[
            HuffmanTree::build_two_node(17, 17),
            HuffmanTree::build_single_node(19),
            HuffmanTree::build_single_node(23),
            HuffmanTree::build_single_node(255),
            HuffmanTree::build_single_node(0),
        ]],
    };
    let _ = decoder.decode_image_data(1, 1, huffman_info, &mut data);

    let mut decoder = LosslessDecoder {
        bit_reader: BitReader {
            reader: Box::new(Cursor::new(Vec::<u8>::new())),
            buffer: 0,
            nbits: 1,
        },
        transforms: [None, None, None, None],
        transform_order: Vec::new(),
        width: 1,
        height: 1,
    };
    let mut data = [0u8; 4];
    let huffman_info = HuffmanInfo {
        xsize: 1,
        _ysize: 1,
        color_cache: None,
        image: vec![0],
        bits: 0,
        mask: 0,
        huffman_code_groups: vec![[
            HuffmanTree::build_two_node(17, 17),
            HuffmanTree::build_two_node(19, 19),
            HuffmanTree::build_single_node(23),
            HuffmanTree::build_single_node(255),
            HuffmanTree::build_single_node(0),
        ]],
    };
    let _ = decoder.decode_image_data(1, 1, huffman_info, &mut data);

    let mut decoder = LosslessDecoder {
        bit_reader: BitReader {
            reader: Box::new(Cursor::new(Vec::<u8>::new())),
            buffer: 0,
            nbits: 1,
        },
        transforms: [None, None, None, None],
        transform_order: Vec::new(),
        width: 1,
        height: 1,
    };
    let mut data = [0u8; 4];
    let huffman_info = HuffmanInfo {
        xsize: 1,
        _ysize: 1,
        color_cache: None,
        image: vec![0],
        bits: 0,
        mask: 0,
        huffman_code_groups: vec![[
            HuffmanTree::build_two_node(17, 17),
            HuffmanTree::build_single_node(19),
            HuffmanTree::build_two_node(23, 23),
            HuffmanTree::build_single_node(255),
            HuffmanTree::build_single_node(0),
        ]],
    };
    let _ = decoder.decode_image_data(1, 1, huffman_info, &mut data);

    let mut decoder = LosslessDecoder {
        bit_reader: BitReader {
            reader: Box::new(Cursor::new(Vec::<u8>::new())),
            buffer: 0,
            nbits: 1,
        },
        transforms: [None, None, None, None],
        transform_order: Vec::new(),
        width: 1,
        height: 1,
    };
    let mut data = [0u8; 4];
    let huffman_info = HuffmanInfo {
        xsize: 1,
        _ysize: 1,
        color_cache: None,
        image: vec![0],
        bits: 0,
        mask: 0,
        huffman_code_groups: vec![[
            HuffmanTree::build_two_node(17, 17),
            HuffmanTree::build_single_node(19),
            HuffmanTree::build_single_node(23),
            HuffmanTree::build_two_node(255, 255),
            HuffmanTree::build_single_node(0),
        ]],
    };
    let _ = decoder.decode_image_data(1, 1, huffman_info, &mut data);

    let mut decoder = LosslessDecoder::new(Box::new(Cursor::new(vec![0u8; 1])));
    let mut data = [0u8; 4];
    let huffman_info = HuffmanInfo {
        xsize: 1,
        _ysize: 1,
        color_cache: None,
        image: vec![0],
        bits: 0,
        mask: 0,
        huffman_code_groups: vec![[
            HuffmanTree::build_single_node(256),
            HuffmanTree::build_single_node(0),
            HuffmanTree::build_single_node(0),
            HuffmanTree::build_single_node(255),
            HuffmanTree::build_single_node(0),
        ]],
    };
    let _ = decoder.decode_image_data(1, 1, huffman_info, &mut data);

    let mut decoder = LosslessDecoder::new(Box::new(Cursor::new(Vec::<u8>::new())));
    let mut data = [0u8; 4];
    let huffman_info = HuffmanInfo {
        xsize: 1,
        _ysize: 1,
        color_cache: None,
        image: vec![0],
        bits: 0,
        mask: 0,
        huffman_code_groups: vec![[
            HuffmanTree::build_single_node(260),
            HuffmanTree::build_single_node(0),
            HuffmanTree::build_single_node(0),
            HuffmanTree::build_single_node(255),
            HuffmanTree::build_single_node(0),
        ]],
    };
    let _ = decoder.decode_image_data(1, 1, huffman_info, &mut data);

    let mut decoder = LosslessDecoder::new(Box::new(Cursor::new(Vec::<u8>::new())));
    let mut data = [0u8; 4];
    let huffman_info = HuffmanInfo {
        xsize: 1,
        _ysize: 1,
        color_cache: None,
        image: vec![0],
        bits: 0,
        mask: 0,
        huffman_code_groups: vec![[
            HuffmanTree::build_single_node(256),
            HuffmanTree::build_single_node(0),
            HuffmanTree::build_single_node(0),
            HuffmanTree::build_single_node(255),
            HuffmanTree::build_two_node(0, 0),
        ]],
    };
    let _ = decoder.decode_image_data(1, 1, huffman_info, &mut data);

    let mut decoder = LosslessDecoder::new(Box::new(Cursor::new(Vec::<u8>::new())));
    let mut data = [0u8; 4];
    let huffman_info = HuffmanInfo {
        xsize: 1,
        _ysize: 1,
        color_cache: None,
        image: vec![0],
        bits: 0,
        mask: 0,
        huffman_code_groups: vec![[
            HuffmanTree::build_single_node(256),
            HuffmanTree::build_single_node(0),
            HuffmanTree::build_single_node(0),
            HuffmanTree::build_single_node(255),
            HuffmanTree::build_single_node(4),
        ]],
    };
    let _ = decoder.decode_image_data(1, 1, huffman_info, &mut data);

    let mut decoder = LosslessDecoder::new(Box::new(Cursor::new(vec![0b0000_0010u8; 1])));
    let mut data = [0u8; 8];
    let huffman_info = HuffmanInfo {
        xsize: 1,
        _ysize: 1,
        color_cache: None,
        image: vec![0],
        bits: 0,
        mask: 0,
        huffman_code_groups: vec![[
            HuffmanTree::build_two_node(29, 256),
            HuffmanTree::build_single_node(31),
            HuffmanTree::build_single_node(37),
            HuffmanTree::build_single_node(255),
            HuffmanTree::build_single_node(0),
        ]],
    };
    let _ = decoder.decode_image_data(2, 1, huffman_info, &mut data);

    let mut decoder = LosslessDecoder::new(Box::new(Cursor::new(vec![0b0011_0000u8; 1])));
    let mut data = [0u8; 32];
    let huffman_info = HuffmanInfo {
        xsize: 1,
        _ysize: 1,
        color_cache: None,
        image: vec![0],
        bits: 0,
        mask: 0,
        huffman_code_groups: vec![[
            HuffmanTree::build_two_node(41, 256),
            HuffmanTree::build_single_node(43),
            HuffmanTree::build_single_node(47),
            HuffmanTree::build_single_node(255),
            HuffmanTree::build_single_node(4),
        ]],
    };
    let _ = decoder.decode_image_data(8, 1, huffman_info, &mut data);

    let mut decoder = LosslessDecoder::new(Box::new(Cursor::new(vec![0b0000_1100u8; 1])));
    let mut data = [0u8; 16];
    let huffman_info = HuffmanInfo {
        xsize: 1,
        _ysize: 1,
        color_cache: None,
        image: vec![0],
        bits: 0,
        mask: 0,
        huffman_code_groups: vec![[
            HuffmanTree::build_two_node(53, 256),
            HuffmanTree::build_single_node(59),
            HuffmanTree::build_single_node(61),
            HuffmanTree::build_single_node(255),
            HuffmanTree::build_single_node(4),
        ]],
    };
    let _ = decoder.decode_image_data(4, 1, huffman_info, &mut data);

    let mut decoder = LosslessDecoder::new(Box::new(Cursor::new(vec![0u8; 1])));
    let mut data = [0u8; 4];
    let huffman_info = HuffmanInfo {
        xsize: 1,
        _ysize: 1,
        color_cache: None,
        image: vec![0],
        bits: 0,
        mask: 0,
        huffman_code_groups: vec![[
            HuffmanTree::build_single_node(280),
            HuffmanTree::build_single_node(0),
            HuffmanTree::build_single_node(0),
            HuffmanTree::build_single_node(255),
            HuffmanTree::build_single_node(0),
        ]],
    };
    let _ = decoder.decode_image_data(1, 1, huffman_info, &mut data);

    let mut decoder = LosslessDecoder::new(Box::new(Cursor::new(vec![0u8; 1])));
    let mut data = [0u8; 8];
    let huffman_info = HuffmanInfo {
        xsize: 1,
        _ysize: 1,
        color_cache: make_color_cache(),
        image: vec![0],
        bits: 1,
        mask: 1,
        huffman_code_groups: vec![[
            HuffmanTree::build_single_node(280),
            HuffmanTree::build_single_node(0),
            HuffmanTree::build_single_node(0),
            HuffmanTree::build_single_node(255),
            HuffmanTree::build_single_node(0),
        ]],
    };
    let _ = decoder.decode_image_data(2, 1, huffman_info, &mut data);

    let mut decoder = LosslessDecoder {
        bit_reader: BitReader {
            reader: Box::new(Cursor::new(Vec::<u8>::new())),
            buffer: 0,
            nbits: 1,
        },
        transforms: [None, None, None, None],
        transform_order: Vec::new(),
        width: 2,
        height: 1,
    };
    let mut data = [0u8; 8];
    let huffman_info = HuffmanInfo {
        xsize: 1,
        _ysize: 1,
        color_cache: make_color_cache(),
        image: vec![0],
        bits: 1,
        mask: 1,
        huffman_code_groups: vec![[
            HuffmanTree::build_two_node(280, 280),
            HuffmanTree::build_single_node(0),
            HuffmanTree::build_single_node(0),
            HuffmanTree::build_single_node(255),
            HuffmanTree::build_single_node(0),
        ]],
    };
    let _ = decoder.decode_image_data(2, 1, huffman_info, &mut data);

    let mut decoder = LosslessDecoder {
        bit_reader: BitReader {
            reader: Box::new(ErrorReader),
            buffer: 0,
            nbits: 0,
        },
        transforms: [None, None, None, None],
        transform_order: Vec::new(),
        width: 1,
        height: 1,
    };
    let _ = decoder.read_color_cache();

    let mut decoder = LosslessDecoder::new(Box::new(Cursor::new(vec![0b0000_0010u8; 8])));
    let mut data = [0u8; 8];
    let huffman_info = HuffmanInfo {
        xsize: 1,
        _ysize: 1,
        color_cache: None,
        image: vec![0],
        bits: 0,
        mask: 0,
        huffman_code_groups: vec![[
            HuffmanTree::build_two_node(0, 257),
            HuffmanTree::build_single_node(0),
            HuffmanTree::build_single_node(0),
            HuffmanTree::build_single_node(255),
            HuffmanTree::build_single_node(1),
        ]],
    };
    let _ = decoder.decode_image_data(2, 1, huffman_info, &mut data);
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

    #[cfg(coverage)]
    pub(crate) const fn __coverage_new(reader: R) -> Self {
        Self::new(reader)
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
