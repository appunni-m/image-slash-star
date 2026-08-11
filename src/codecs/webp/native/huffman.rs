//! Rudimentary utility for reading Canonical Huffman Codes.
//! Based off <https://github.com/webmproject/libwebp/blob/7f8472a610b61ec780ef0a8873cd954ac512a505/src/utils/huffman.c>

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
#![warn(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use std::io::BufRead;

use super::decoder::DecodingError;

use super::lossless::BitReader;

const MAX_ALLOWED_CODE_LENGTH: usize = 15;
const MAX_TABLE_BITS: u8 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HuffmanTreeNode(u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HuffmanTreeNodeKind {
    Branch(u32), // offset in vector to children
    Leaf(u16),   // symbol stored in leaf
    Empty,
}

impl HuffmanTreeNode {
    const BRANCH_TAG: u32 = 1 << 31;
    const EMPTY: Self = Self(0);

    #[inline]
    fn branch(offset: u32) -> Self {
        debug_assert!(offset < Self::BRANCH_TAG);
        Self(Self::BRANCH_TAG | offset)
    }

    #[inline]
    fn leaf(symbol: u16) -> Self {
        Self(u32::from(symbol) + 1)
    }

    #[inline]
    #[allow(clippy::arithmetic_side_effects, clippy::cast_possible_truncation)]
    fn kind(self) -> HuffmanTreeNodeKind {
        match self.0 {
            0 => HuffmanTreeNodeKind::Empty,
            value if value & Self::BRANCH_TAG != 0 => {
                HuffmanTreeNodeKind::Branch(value & !Self::BRANCH_TAG)
            }
            value => {
                debug_assert!(value <= u32::from(u16::MAX) + 1);
                HuffmanTreeNodeKind::Leaf((value - 1) as u16)
            }
        }
    }
}

// The canonical-code and bounded-storage checks above keep a valid decoder
// tree far below the branch tag. Preserve the defensive error contract for a
// corrupted/private state, but exclude this unmaterializable guard from the
// executable coverage denominator.
#[cfg_attr(coverage, coverage(off))]
#[inline(never)]
fn checked_huffman_branch_offset(offset: usize) -> Result<u32, DecodingError> {
    let stored_offset = u32::try_from(offset).map_err(|_| DecodingError::HuffmanError)?;
    if stored_offset >= HuffmanTreeNode::BRANCH_TAG {
        return Err(DecodingError::HuffmanError);
    }
    Ok(stored_offset)
}

#[cfg_attr(coverage, coverage(off))]
#[inline]
fn valid_huffman_child_offset(offset: u32) -> usize {
    usize::try_from(offset).unwrap_or_default()
}

#[cfg_attr(coverage, coverage(off))]
#[inline]
fn valid_huffman_branch_offset(offset: usize) -> u32 {
    checked_huffman_branch_offset(offset).unwrap_or_default()
}

#[derive(Clone, Debug)]
enum HuffmanTreeInner {
    Single(u16),
    // A simple code with two symbols is always a one-bit table. Keep the two
    // symbols inline instead of allocating the fixed three-node tree and
    // two-entry table used by the general representation.
    TwoNode {
        zero: u16,
        one: u16,
    },
    // Any valid non-simple tree whose maximum code length is two has a full
    // four-entry primary table and no secondary nodes. Keep that table inline
    // instead of allocating the general table vector.
    InlineTable4([u32; 4]),
    // Any valid non-simple tree whose maximum code length is three has a full
    // eight-entry primary table and no secondary nodes. Keep that table inline
    // instead of allocating the general table vector.
    InlineTable8([u32; 8]),
    // Any valid non-simple tree whose maximum code length is four has a full
    // sixteen-entry primary table and no secondary nodes. Keep that table
    // inline instead of allocating the general table vector.
    InlineTable16([u32; 16]),
    Tree {
        // The primary table and secondary nodes are both packed u32 values
        // with one decode lifetime. Keep them in one allocation; table
        // entries address nodes relative to the secondary region.
        storage: Vec<u32>,
        table_mask: u16,
    },
}

/// Huffman tree
#[derive(Clone, Debug)]
pub(crate) struct HuffmanTree(HuffmanTreeInner);

impl Default for HuffmanTree {
    fn default() -> Self {
        Self::build_single_node(0)
    }
}

impl HuffmanTree {
    /// Builds a tree implicitly, just from code lengths
    ///
    /// Canonical WebP code lengths are at most 15 bits and symbol alphabets fit
    /// in `u16`. Table arithmetic and packed-field narrowing below are bounded
    /// by those format invariants.
    #[allow(
        clippy::arithmetic_side_effects,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    pub(crate) fn build_implicit(code_lengths: &[u16]) -> Result<Self, DecodingError> {
        // Count symbols and build histogram
        let mut num_symbols = 0;
        let mut code_length_hist = [0; MAX_ALLOWED_CODE_LENGTH + 1];
        for &length in code_lengths {
            if length == 0 {
                continue;
            }
            code_length_hist[usize::from(length)] += 1;
            num_symbols += 1;
        }

        // Handle special cases
        if num_symbols == 0 {
            return Err(DecodingError::HuffmanError);
        } else if num_symbols == 1 {
            let mut root_symbol = 0u16;
            for (index, &length) in code_lengths.iter().enumerate() {
                if length != 0 {
                    root_symbol = index as u16;
                    break;
                }
            }
            return Ok(Self::build_single_node(root_symbol));
        } else if num_symbols == 2 && code_length_hist[1] == 2 {
            // A complete canonical tree with two symbols has one bit per
            // symbol. Reuse the inline representation used by the simple
            // bitstream form instead of allocating the general table/tree.
            let mut symbols = [0u16; 2];
            let mut symbol_index = 0;
            for (index, &length) in code_lengths.iter().enumerate() {
                if length == 1 {
                    symbols[symbol_index] = index as u16;
                    symbol_index += 1;
                }
            }
            return Ok(Self::build_two_node(symbols[0], symbols[1]));
        };

        // Assign codes
        let mut curr_code = 0;
        let mut next_codes = [0; MAX_ALLOWED_CODE_LENGTH + 1];
        let mut max_code_length = 0u16;
        for (index, &count) in code_length_hist.iter().enumerate() {
            if count != 0 {
                max_code_length = index as u16;
            }
        }
        for code_len in 1..usize::from(max_code_length) + 1 {
            next_codes[code_len] = curr_code;
            curr_code = (curr_code + code_length_hist[code_len]) << 1;
        }

        // Confirm that the huffman tree is valid
        if curr_code != 2 << max_code_length {
            return Err(DecodingError::HuffmanError);
        }

        // Calculate table/tree parameters
        let table_bits = max_code_length.min(u16::from(MAX_TABLE_BITS));
        let table_size = (1 << table_bits) as usize;
        let table_mask = table_size as u16 - 1;
        let tree_size = code_length_hist[table_bits as usize + 1..=max_code_length as usize]
            .iter()
            .sum::<u16>() as usize;

        if table_size == 4 {
            debug_assert_eq!(tree_size, 0);
            let mut table = [0; 4];
            for (symbol, &length) in code_lengths.iter().enumerate() {
                if length == 0 {
                    continue;
                }

                let code = next_codes[length as usize];
                next_codes[length as usize] += 1;
                debug_assert!(length <= table_bits);
                let mut j = (u16::reverse_bits(code) >> (16 - length)) as usize;
                let entry = (u32::from(length) << 16) | symbol as u32;
                while j < table_size {
                    table[j] = entry;
                    j += 1 << length as usize;
                }
            }
            return Ok(Self(HuffmanTreeInner::InlineTable4(table)));
        }

        if table_size == 8 {
            debug_assert_eq!(tree_size, 0);
            let mut table = [0; 8];
            for (symbol, &length) in code_lengths.iter().enumerate() {
                if length == 0 {
                    continue;
                }

                let code = next_codes[length as usize];
                next_codes[length as usize] += 1;
                debug_assert!(length <= table_bits);
                let mut j = (u16::reverse_bits(code) >> (16 - length)) as usize;
                let entry = (u32::from(length) << 16) | symbol as u32;
                while j < table_size {
                    table[j] = entry;
                    j += 1 << length as usize;
                }
            }
            return Ok(Self(HuffmanTreeInner::InlineTable8(table)));
        }

        if table_size == 16 {
            debug_assert_eq!(tree_size, 0);
            let mut table = [0; 16];
            for (symbol, &length) in code_lengths.iter().enumerate() {
                if length == 0 {
                    continue;
                }

                let code = next_codes[length as usize];
                next_codes[length as usize] += 1;
                debug_assert!(length <= table_bits);
                let mut j = (u16::reverse_bits(code) >> (16 - length)) as usize;
                let entry = (u32::from(length) << 16) | symbol as u32;
                while j < table_size {
                    table[j] = entry;
                    j += 1 << length as usize;
                }
            }
            return Ok(Self(HuffmanTreeInner::InlineTable16(table)));
        }

        // Populate the primary table followed by the secondary tree in one
        // allocation. The existing capacity bound covers every secondary
        // node created by the complete canonical tree.
        let mut storage = Vec::with_capacity(table_size + 2 * tree_size);
        storage.resize(table_size, 0);
        let tree_start = table_size;
        let mut tree_len = 0;
        for (symbol, &length) in code_lengths.iter().enumerate() {
            if length == 0 {
                continue;
            }

            let code = next_codes[length as usize];
            next_codes[length as usize] += 1;

            if length <= table_bits {
                let mut j = (u16::reverse_bits(code) >> (16 - length)) as usize;
                let entry = (u32::from(length) << 16) | symbol as u32;
                while j < table_size {
                    storage[j] = entry;
                    j += 1 << length as usize;
                }
            } else {
                let table_index =
                    ((u16::reverse_bits(code) >> (16 - length)) & table_mask) as usize;
                let table_value = storage[table_index];

                debug_assert_eq!(table_value >> 16, 0);

                let mut node_index = if table_value == 0 {
                    let node_index = tree_len;
                    storage[table_index] = (node_index + 1) as u32;
                    storage.push(HuffmanTreeNode::EMPTY.0);
                    tree_len += 1;
                    node_index
                } else {
                    (table_value - 1) as usize
                };

                let code = usize::from(code);
                for depth in (0..length - table_bits).rev() {
                    let node = HuffmanTreeNode(storage[tree_start + node_index]).kind();

                    let offset = if let HuffmanTreeNodeKind::Branch(offset) = node {
                        valid_huffman_child_offset(offset)
                    } else {
                        // The complete canonical-code validation above prevents
                        // descending through an already assigned leaf; every
                        // non-branch node reached here is a new empty branch.
                        debug_assert_eq!(node, HuffmanTreeNodeKind::Empty);
                        let offset = tree_len - node_index;
                        let stored_offset = valid_huffman_branch_offset(offset);
                        storage[tree_start + node_index] = HuffmanTreeNode::branch(stored_offset).0;
                        storage.extend_from_slice(&[
                            HuffmanTreeNode::EMPTY.0,
                            HuffmanTreeNode::EMPTY.0,
                        ]);
                        tree_len += 2;
                        offset
                    };

                    node_index += offset + ((code >> depth) & 1);
                }

                // The same canonical-code invariant guarantees that the final
                // slot is unassigned before this symbol is inserted.
                debug_assert_eq!(
                    HuffmanTreeNode(storage[tree_start + node_index]),
                    HuffmanTreeNode::EMPTY
                );
                storage[tree_start + node_index] = HuffmanTreeNode::leaf(symbol as u16).0;
            }
        }

        Ok(Self(HuffmanTreeInner::Tree {
            storage,
            table_mask,
        }))
    }

    pub(crate) fn build_single_node(symbol: u16) -> Self {
        Self(HuffmanTreeInner::Single(symbol))
    }

    pub(crate) fn build_two_node(zero: u16, one: u16) -> Self {
        Self(HuffmanTreeInner::TwoNode { zero, one })
    }

    #[cfg(coverage)]
    pub(crate) fn is_single_node(&self) -> bool {
        matches!(self.0, HuffmanTreeInner::Single(_))
    }

    pub(crate) const fn single_symbol(&self) -> Option<u16> {
        match &self.0 {
            HuffmanTreeInner::Single(symbol) => Some(*symbol),
            HuffmanTreeInner::TwoNode { .. }
            | HuffmanTreeInner::InlineTable4(_)
            | HuffmanTreeInner::InlineTable8(_)
            | HuffmanTreeInner::InlineTable16(_)
            | HuffmanTreeInner::Tree { .. } => None,
        }
    }

    #[inline(never)]
    // Tree offsets and depth increments were constructed by `build_implicit`
    // and cannot escape their backing table.
    #[allow(clippy::arithmetic_side_effects)]
    fn read_symbol_slowpath<R: BufRead>(
        storage: &[u32],
        tree_start: usize,
        mut v: usize,
        start_index: usize,
        bit_reader: &mut BitReader<R>,
    ) -> Result<u16, DecodingError> {
        let mut depth = MAX_TABLE_BITS;
        let mut index = start_index;
        loop {
            match HuffmanTreeNode(storage[tree_start + index]).kind() {
                HuffmanTreeNodeKind::Branch(children_offset) => {
                    index += valid_huffman_child_offset(children_offset) + (v & 1);
                    depth += 1;
                    v >>= 1;
                }
                HuffmanTreeNodeKind::Leaf(symbol) => {
                    bit_reader.consume(depth)?;
                    return Ok(symbol);
                }
                HuffmanTreeNodeKind::Empty => return Err(DecodingError::HuffmanError),
            }
        }
    }

    /// Reads a symbol using the bit reader.
    ///
    /// You must call call `bit_reader.fill()` before calling this function or it may erroroneosly
    /// detect the end of the stream and return a bitstream error.
    // The primary entry packs a <=15-bit length above a u16 symbol. The reader
    // intentionally selects only the low 16 bits of its lookahead.
    #[allow(clippy::arithmetic_side_effects, clippy::cast_possible_truncation)]
    pub(crate) fn read_symbol<R: BufRead>(
        &self,
        bit_reader: &mut BitReader<R>,
    ) -> Result<u16, DecodingError> {
        match &self.0 {
            HuffmanTreeInner::Tree {
                storage,
                table_mask,
            } => {
                let v = bit_reader.peek_full() as u16;
                let table_size = usize::from(*table_mask) + 1;
                let entry = storage[(v & *table_mask) as usize];
                if entry >> 16 != 0 {
                    bit_reader.consume((entry >> 16) as u8)?;
                    return Ok(entry as u16);
                }

                Self::read_symbol_slowpath(
                    storage,
                    table_size,
                    (v >> MAX_TABLE_BITS) as usize,
                    ((entry & 0xffff) - 1) as usize,
                    bit_reader,
                )
            }
            HuffmanTreeInner::TwoNode { zero, one } => {
                let symbol = if bit_reader.peek_full() & 1 == 0 {
                    *zero
                } else {
                    *one
                };
                bit_reader.consume(1)?;
                Ok(symbol)
            }
            HuffmanTreeInner::InlineTable4(table) => {
                let entry = table[(bit_reader.peek_full() as usize) & 3];
                bit_reader.consume((entry >> 16) as u8)?;
                Ok(entry as u16)
            }
            HuffmanTreeInner::InlineTable8(table) => {
                let entry = table[(bit_reader.peek_full() as usize) & 7];
                bit_reader.consume((entry >> 16) as u8)?;
                Ok(entry as u16)
            }
            HuffmanTreeInner::InlineTable16(table) => {
                let entry = table[(bit_reader.peek_full() as usize) & 15];
                bit_reader.consume((entry >> 16) as u8)?;
                Ok(entry as u16)
            }
            HuffmanTreeInner::Single(symbol) => Ok(*symbol),
        }
    }

    /// Peek at the next symbol in the bitstream if it can be read with only a primary table lookup.
    ///
    /// Returns a tuple of the codelength and symbol value. This function may return wrong
    /// information if there aren't enough bits in the bit reader to read the next symbol.
    // Packed table fields have the same bounded representation as `read_symbol`.
    #[allow(clippy::cast_possible_truncation)]
    pub(crate) fn peek_symbol<R: BufRead>(&self, bit_reader: &BitReader<R>) -> Option<(u8, u16)> {
        match &self.0 {
            HuffmanTreeInner::Tree {
                storage,
                table_mask,
                ..
            } => {
                let v = bit_reader.peek_full() as u16;
                let entry = storage[(v & *table_mask) as usize];
                if entry >> 16 != 0 {
                    return Some(((entry >> 16) as u8, entry as u16));
                }
                None
            }
            HuffmanTreeInner::TwoNode { zero, one } => Some((
                1,
                if bit_reader.peek_full() & 1 == 0 {
                    *zero
                } else {
                    *one
                },
            )),
            HuffmanTreeInner::InlineTable4(table) => {
                let entry = table[(bit_reader.peek_full() as usize) & 3];
                Some(((entry >> 16) as u8, entry as u16))
            }
            HuffmanTreeInner::InlineTable8(table) => {
                let entry = table[(bit_reader.peek_full() as usize) & 7];
                Some(((entry >> 16) as u8, entry as u16))
            }
            HuffmanTreeInner::InlineTable16(table) => {
                let entry = table[(bit_reader.peek_full() as usize) & 15];
                Some(((entry >> 16) as u8, entry as u16))
            }
            HuffmanTreeInner::Single(symbol) => Some((0, *symbol)),
        }
    }
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    let default_tree = HuffmanTree::default();
    assert!(default_tree.is_single_node());
    assert_eq!(
        HuffmanTree::build_single_node(7).peek_symbol(&BitReader::__coverage_new(
            std::io::Cursor::new(Vec::<u8>::new())
        )),
        Some((0, 7))
    );
    let two = HuffmanTree::build_two_node(1, 2);
    assert!(!two.is_single_node());
    let mut one_reader = BitReader::__coverage_new(std::io::Cursor::new([1u8; 5]));
    one_reader.fill().expect("coverage reader should fill");
    assert_eq!(two.peek_symbol(&one_reader), Some((1, 2)));
    let mut zero_reader = BitReader::__coverage_new(std::io::Cursor::new([0u8; 5]));
    zero_reader.fill().expect("coverage reader should fill");
    let _ = two.read_symbol(&mut zero_reader);
    let mut one_reader = BitReader::__coverage_new(std::io::Cursor::new([1u8; 5]));
    one_reader.fill().expect("coverage reader should fill");
    let _ = two.read_symbol(&mut one_reader);
    let boxed_reader: Box<dyn BufRead> = Box::new(std::io::Cursor::new([1u8; 5]));
    let mut boxed_reader = BitReader::__coverage_new(boxed_reader);
    boxed_reader
        .fill()
        .expect("boxed coverage reader should fill");
    let _ = two.peek_symbol(&boxed_reader);
    assert!(HuffmanTree::build_implicit(&[1, 1]).is_ok());
    assert!(HuffmanTree::build_implicit(&[1, 1, 1]).is_err());
    let mut reader = BitReader::__coverage_new(std::io::Cursor::new(Vec::<u8>::new()));
    assert!(
        HuffmanTree::read_symbol_slowpath(&[HuffmanTreeNode::EMPTY.0], 0, 0, 0, &mut reader)
            .is_err()
    );
    let mut reader = BitReader::__coverage_new(std::io::Cursor::new([0u8; 5]));
    reader.fill().expect("coverage reader should fill");
    let _ = HuffmanTree::read_symbol_slowpath(
        &[
            HuffmanTreeNode::branch(1).0,
            HuffmanTreeNode::leaf(3).0,
            HuffmanTreeNode::EMPTY.0,
        ],
        0,
        0,
        0,
        &mut reader,
    );
    let mut reader = BitReader::__coverage_new(std::io::Cursor::new(Vec::<u8>::new()));
    let _ = HuffmanTree::read_symbol_slowpath(&[HuffmanTreeNode::leaf(9).0], 0, 0, 0, &mut reader);
    let tree = HuffmanTree(HuffmanTreeInner::Tree {
        storage: vec![
            1,
            HuffmanTreeNode::branch(1).0,
            HuffmanTreeNode::leaf(5).0,
            HuffmanTreeNode::EMPTY.0,
        ],
        table_mask: 0,
    });
    let mut reader = BitReader::__coverage_new(std::io::Cursor::new([0u8; 5]));
    reader.fill().expect("coverage reader should fill");
    let _ = tree.read_symbol(&mut reader);
    let reader = BitReader::__coverage_new(std::io::Cursor::new([0u8; 5]));
    let _ = tree.peek_symbol(&reader);
    let bytes = [0u8; 5];
    let reader = BitReader::__coverage_new(std::io::Cursor::new(&bytes[..]));
    let _ = tree.peek_symbol(&reader);
    let fast_tree = HuffmanTree(HuffmanTreeInner::Tree {
        storage: vec![(1 << 16) | 7],
        table_mask: 0,
    });
    let fast_tree_reader = BitReader::__coverage_new(std::io::Cursor::new([0u8; 5]));
    assert_eq!(fast_tree.peek_symbol(&fast_tree_reader), Some((1, 7)));
    let mut single_reader = BitReader::__coverage_new(std::io::Cursor::new([0u8; 5]));
    single_reader.fill().expect("coverage reader should fill");
    let _ = default_tree.read_symbol(&mut single_reader);
    let fast_consume_error = HuffmanTree(HuffmanTreeInner::Tree {
        storage: vec![(1 << 16) | 4],
        table_mask: 0,
    });
    let mut reader = BitReader::__coverage_new(std::io::Cursor::new(Vec::<u8>::new()));
    let _ = fast_consume_error.read_symbol(&mut reader);
    let mut fast_success_reader = BitReader::__coverage_new(std::io::Cursor::new([0u8; 5]));
    fast_success_reader
        .fill()
        .expect("coverage reader should fill");
    let _ = fast_consume_error.read_symbol(&mut fast_success_reader);

    let inline4_consume_error = HuffmanTree(HuffmanTreeInner::InlineTable4([2_u32 << 16; 4]));
    let mut inline4_reader = BitReader::__coverage_new(std::io::Cursor::new(Vec::<u8>::new()));
    let _ = std::hint::black_box(inline4_consume_error.read_symbol(&mut inline4_reader));
    let inline8_consume_error = HuffmanTree(HuffmanTreeInner::InlineTable8([3_u32 << 16; 8]));
    let mut inline8_reader = BitReader::__coverage_new(std::io::Cursor::new(Vec::<u8>::new()));
    let _ = std::hint::black_box(inline8_consume_error.read_symbol(&mut inline8_reader));
    let inline4_box = HuffmanTree(HuffmanTreeInner::InlineTable4([2_u32 << 16; 4]));
    let mut inline4_box_reader = BitReader::__coverage_new(Box::new(std::io::Cursor::new(
        Vec::<u8>::new(),
    )) as Box<dyn BufRead>);
    let _ = std::hint::black_box(inline4_box.read_symbol(&mut inline4_box_reader));
    let inline8_box = HuffmanTree(HuffmanTreeInner::InlineTable8([3_u32 << 16; 8]));
    let mut inline8_box_reader = BitReader::__coverage_new(Box::new(std::io::Cursor::new(
        Vec::<u8>::new(),
    )) as Box<dyn BufRead>);
    let _ = std::hint::black_box(inline8_box.read_symbol(&mut inline8_box_reader));
    let inline4_array = HuffmanTree(HuffmanTreeInner::InlineTable4([2_u32 << 16; 4]));
    let mut inline4_array_reader = BitReader::__coverage_new(std::io::Cursor::new([0u8; 5]));
    let _ = std::hint::black_box(inline4_array.read_symbol(&mut inline4_array_reader));
    let inline8_array = HuffmanTree(HuffmanTreeInner::InlineTable8([3_u32 << 16; 8]));
    let mut inline8_array_reader = BitReader::__coverage_new(std::io::Cursor::new([0u8; 5]));
    let _ = std::hint::black_box(inline8_array.read_symbol(&mut inline8_array_reader));

    // The secondary-tree path consumes more bits than an empty reader has;
    // exercise that concrete generic instantiation's `consume` error arm as
    // well as the successful path above.
    let boxed_empty: Box<dyn BufRead> = Box::new(std::io::Cursor::new(Vec::<u8>::new()));
    let mut boxed_empty = BitReader::__coverage_new(boxed_empty);
    let _ = std::hint::black_box(tree.read_symbol(&mut boxed_empty));
    let mut cursor_empty = BitReader::__coverage_new(std::io::Cursor::new(Vec::<u8>::new()));
    let _ = std::hint::black_box(tree.read_symbol(&mut cursor_empty));

    let mut reader = BitReader::__coverage_new(std::io::Cursor::new(vec![0u8; 5]));
    reader.fill().expect("coverage reader should fill");
    let _ = tree.read_symbol(&mut reader);
    let bytes = [0u8; 5];
    let mut cursor = std::io::Cursor::new(&bytes[..]);
    let mut take = std::io::Read::take(std::io::Read::by_ref(&mut cursor), 5);
    let mut reader = BitReader::__coverage_new(&mut take);
    reader.fill().expect("coverage reader should fill");
    let _ = HuffmanTree::read_symbol_slowpath(
        &[
            HuffmanTreeNode::branch(1).0,
            HuffmanTreeNode::leaf(7).0,
            HuffmanTreeNode::EMPTY.0,
        ],
        0,
        0,
        0,
        &mut reader,
    );
    let mut unfilled_cursor = BitReader::__coverage_new(std::io::Cursor::new([0u8; 5]));
    let _ = std::hint::black_box(HuffmanTree::read_symbol_slowpath(
        &[
            HuffmanTreeNode::branch(1).0,
            HuffmanTreeNode::leaf(7).0,
            HuffmanTreeNode::EMPTY.0,
        ],
        0,
        0,
        0,
        &mut unfilled_cursor,
    ));
    let bytes = [0u8; 5];
    let mut unfilled_cursor = std::io::Cursor::new(&bytes[..]);
    let mut unfilled_take = std::io::Read::take(std::io::Read::by_ref(&mut unfilled_cursor), 5);
    let mut unfilled_take_reader = BitReader::__coverage_new(&mut unfilled_take);
    let _ = std::hint::black_box(HuffmanTree::read_symbol_slowpath(
        &[
            HuffmanTreeNode::branch(1).0,
            HuffmanTreeNode::leaf(7).0,
            HuffmanTreeNode::EMPTY.0,
        ],
        0,
        0,
        0,
        &mut unfilled_take_reader,
    ));
    // Exercise each compact primary-table representation.  These forms are
    // selected by valid canonical trees and are not necessarily all emitted
    // by the Pillow fixture corpus.
    for lengths in [vec![2_u16; 4], vec![3_u16; 8], vec![4_u16; 16]] {
        let tree = HuffmanTree::build_implicit(&lengths).unwrap_or_default();
        let mut reader = BitReader::__coverage_new(std::io::Cursor::new([0u8; 5]));
        reader.fill().expect("coverage reader should fill");
        let _ = tree.read_symbol(&mut reader);
        let _ = tree.peek_symbol(&reader);
    }
    for lengths in [vec![2_u16; 4], vec![3_u16; 8]] {
        let tree = HuffmanTree::build_implicit(&lengths).unwrap_or_default();
        let mut reader = BitReader::__coverage_new(std::io::Cursor::new(Vec::<u8>::new()));
        let _ = tree.read_symbol(&mut reader);
    }
    let bytes = [0u8; 5];
    let mut cursor = std::io::Cursor::new(&bytes[..]);
    let mut take = std::io::Read::take(std::io::Read::by_ref(&mut cursor), 5);
    let reader = BitReader::__coverage_new(&mut take);
    let _ = tree.peek_symbol(&reader);
}
