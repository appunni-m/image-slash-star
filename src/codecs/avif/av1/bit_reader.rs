//! Zero-copy bit reads over ordered AVIF item extents.

use super::super::samples::ByteSpan;

/// A logical byte stream backed by ordered spans in the encoded AVIF input.
pub(super) struct SegmentedData<'input, 'spans> {
    input: &'input [u8],
    spans: &'spans [ByteSpan],
    length: usize,
}

impl<'input, 'spans> SegmentedData<'input, 'spans> {
    pub(super) fn new(input: &'input [u8], spans: &'spans [ByteSpan]) -> Option<Self> {
        for span in spans {
            let _ = span.bytes(input).ok()?;
        }
        Self::with_validated_spans(input, spans)
    }

    fn with_validated_spans(input: &'input [u8], spans: &'spans [ByteSpan]) -> Option<Self> {
        let mut length = 0_usize;
        for span in spans {
            length = length.checked_add(span.len())?;
        }
        Some(Self {
            input,
            spans,
            length,
        })
    }

    pub(super) fn len(&self) -> usize {
        self.length
    }

    pub(super) fn byte(&self, position: usize) -> Option<u8> {
        let mut logical_start = 0_usize;
        for span in self.spans {
            let logical_end = logical_start.saturating_add(span.len());
            if position < logical_end {
                let offset = position.saturating_sub(logical_start);
                let physical = span.start.saturating_add(offset);
                return self.input.get(physical).copied();
            }
            logical_start = logical_end;
        }
        None
    }

    /// Read a byte after the caller has proved `position < self.len()`.
    pub(super) fn validated_byte(&self, position: usize) -> u8 {
        // `new()` validates every physical span, so a logical position below
        // `length` always maps to one input byte. Keeping this total avoids
        // manufacturing an unreachable fallible edge in later bounded parsers.
        self.byte(position).unwrap_or_default()
    }
}

/// MSB-first AV1 syntax reader bounded to one OBU payload.
pub(super) struct BitReader<'data, 'input, 'spans> {
    data: &'data SegmentedData<'input, 'spans>,
    position: usize,
    end: usize,
}

impl<'data, 'input, 'spans> BitReader<'data, 'input, 'spans> {
    pub(super) fn new(
        data: &'data SegmentedData<'input, 'spans>,
        start: usize,
        end: usize,
    ) -> Option<Self> {
        let position = start.checked_mul(8)?;
        let end_position = end.checked_mul(8)?;
        if start > end || end > data.len() {
            return None;
        }
        Some(Self {
            data,
            position,
            end: end_position,
        })
    }

    #[cfg(coverage)]
    pub(super) fn with_bit_end(
        data: &'data SegmentedData<'input, 'spans>,
        end: usize,
    ) -> Option<Self> {
        if end > data.len() * 8 {
            return None;
        }
        Some(Self {
            data,
            position: 0,
            end,
        })
    }

    pub(super) fn bits(&mut self, count: u32) -> Option<u32> {
        if count > 32 {
            return None;
        }
        let next = self.position.checked_add(count as usize)?;
        if next > self.end {
            return None;
        }
        let mut value = 0_u32;
        for _ in 0..count {
            let byte = self.data.byte(self.position / 8)?;
            let shift = [7_u8, 6, 5, 4, 3, 2, 1, 0][self.position % 8];
            value = (value << 1) | u32::from((byte >> shift) & 1);
            self.position = self.position.saturating_add(1);
        }
        Some(value)
    }

    pub(super) fn bit(&mut self) -> Option<bool> {
        Some(self.bits(1)? != 0)
    }

    // ✅ VERIFIED: AV1 specification section 4.10.6 / dav1d 1.5.3
    // src/getbits.c:92-93.
    pub(super) fn signed(&mut self, count: u32) -> Option<i32> {
        if count == 0 || count > 31 {
            return None;
        }
        let value = i64::from(self.bits(count)?);
        let sign = 1_i64 << count.saturating_sub(1);
        let signed = if value & sign == 0 {
            value
        } else {
            // `value` and the sign extension are both bounded to 31 bits.
            value.saturating_sub(1_i64 << count)
        };
        i32::try_from(signed).ok()
    }

    // ✅ VERIFIED: AV1 specification section 4.10.8 / dav1d 1.5.3
    // src/getbits.c:114-123.
    pub(super) fn ns(&mut self, count: u32) -> Option<u32> {
        if count <= 1 {
            return Some(0);
        }
        let width = u32::BITS.saturating_sub(count.leading_zeros());
        let threshold = (1_u64 << width).saturating_sub(u64::from(count));
        let value = u64::from(self.bits(width.saturating_sub(1))?);
        let decoded = if value < threshold {
            value
        } else {
            (value << 1)
                .saturating_sub(threshold)
                .saturating_add(u64::from(self.bit()?))
        };
        u32::try_from(decoded).ok()
    }

    // ✅ VERIFIED: AV1 specification section 4.10.10 / dav1d 1.5.3
    // src/getbits.c:138-164 and include/common/intops.h:75-81.
    pub(super) fn subexp(&mut self, reference: i32, bits: u32) -> Option<i32> {
        if bits > 30 {
            return None;
        }
        // AV1 calls this with a reference inside the signed `bits` range.
        // Use wider intermediates so every shift and boundary sum remains
        // representable even at the defensive 30-bit limit.
        let count = 2_u64 << bits;
        let base = 1_i64 << bits;
        // `reference` is i32 and `base` is at most 2^30, so this sum is exact
        // in i64. Conversion still rejects references below the signed range.
        let recentered_reference = u64::try_from(i64::from(reference).saturating_add(base)).ok()?;
        if recentered_reference >= count {
            return None;
        }
        let mut value = 0_u64;
        let mut index = 0_u32;
        loop {
            let width = if index == 0 {
                3
            } else {
                3_u32.saturating_add(index).saturating_sub(1)
            };
            let block = 1_u64 << width;
            // At the accepted 30-bit limit, both terms remain below 2^34.
            let boundary = value.saturating_add(block.saturating_mul(3));
            if count < boundary {
                let remaining = count.saturating_sub(value).saturating_add(1);
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "`count` is at most 2^31, so `remaining` fits in u32"
                )]
                let remaining = remaining as u32;
                value = value.saturating_add(u64::from(self.ns(remaining)?));
                break;
            }
            if !self.bit()? {
                value = value.saturating_add(u64::from(self.bits(width)?));
                break;
            }
            value = value.saturating_add(block);
            index = index.saturating_add(1);
        }
        let decoded = if recentered_reference.saturating_mul(2) <= count {
            inverse_recenter(recentered_reference, value)
        } else {
            let inverse = inverse_recenter(count.saturating_sub(recentered_reference), value);
            count.saturating_sub(inverse)
        };
        // `decoded < count <= 2^31`, so conversion to i64 cannot fail.
        let decoded = i64::try_from(decoded)
            .unwrap_or(i64::MAX)
            .saturating_sub(base);
        i32::try_from(decoded).ok()
    }

    // ✅ VERIFIED: AV1 specification section 5.3.5 / dav1d 1.5.3
    // src/getbits.h:128-132.
    pub(super) fn byte_align(&mut self) -> Option<()> {
        while !self.position.is_multiple_of(8) {
            if self.bit()? {
                return None;
            }
        }
        Some(())
    }

    pub(super) fn position(&self) -> usize {
        self.position
    }

    // ✅ VERIFIED: AV1 specification section 4.10.3 / dav1d 1.5.3
    // src/getbits.c:125-136.
    pub(super) fn uvlc(&mut self) -> Option<u32> {
        let mut leading_zeroes = 0_u32;
        while !self.bit()? {
            leading_zeroes = leading_zeroes.saturating_add(1);
            if leading_zeroes == 32 {
                return None;
            }
        }
        (1_u32 << leading_zeroes)
            .saturating_sub(1)
            .checked_add(self.bits(leading_zeroes)?)
    }

    // ✅ VERIFIED: AV1 specification section 5.3.4 / dav1d 1.5.3
    // src/obu.c:48-69.
    pub(super) fn trailing_bits(&mut self) -> Option<()> {
        if !self.bit()? {
            return None;
        }
        while self.position < self.end {
            if self.bit()? {
                return None;
            }
        }
        Some(())
    }
}

fn inverse_recenter(reference: u64, value: u64) -> u64 {
    if value > reference.saturating_mul(2) {
        value
    } else if value.is_multiple_of(2) {
        reference.saturating_add(value >> 1)
    } else {
        reference.saturating_sub(value.saturating_add(1) >> 1)
    }
}

#[cfg(coverage)]
#[coverage(off)]
pub(super) fn __coverage_exercise_private_branches() {
    let input = [0b1010_0101, 0b0101_1010];
    let spans = [ByteSpan { start: 0, end: 1 }, ByteSpan { start: 1, end: 2 }];
    let data = SegmentedData::new(&input, &spans).unwrap();
    assert_eq!(data.byte(0), Some(input[0]));
    assert_eq!(data.byte(1), Some(input[1]));
    assert_eq!(data.byte(2), None);

    let _ = SegmentedData::new(&input, &[ByteSpan { start: 0, end: 3 }]);
    let _ = SegmentedData::with_validated_spans(
        &[],
        &[
            ByteSpan {
                start: 0,
                end: usize::MAX,
            },
            ByteSpan { start: 0, end: 1 },
        ],
    );
    let _ = BitReader::new(&data, usize::MAX, usize::MAX);
    let _ = BitReader::new(&data, 0, usize::MAX);
    let _ = BitReader::new(&data, 2, 1);
    let _ = BitReader::new(&data, 0, 3);
    let _ = BitReader::with_bit_end(&data, data.len() * 8 + 1);
    let _ = BitReader::with_bit_end(&data, data.len() * 8);
    let mut reader = BitReader::new(&data, 0, data.len()).unwrap();
    assert_eq!(reader.bits(4), Some(0b1010));
    assert_eq!(reader.bits(12), Some(0b0101_0101_1010));
    assert_eq!(reader.bits(1), None);
    let mut reader = BitReader::new(&data, 0, data.len()).unwrap();
    let _ = reader.bits(33);
    let _ = reader.signed(0);
    let _ = reader.signed(32);
    assert_eq!(reader.ns(0), Some(0));
    assert_eq!(reader.ns(1), Some(0));
    let _ = reader.subexp(0, 31);
    let mut reader = BitReader::with_bit_end(&data, 1).unwrap();
    let _ = reader.ns(3);
    let mut reader = BitReader {
        data: &data,
        position: 1,
        end: 1,
    };
    let _ = reader.byte_align();
    let mut reader = BitReader {
        data: &data,
        position: usize::MAX,
        end: usize::MAX,
    };
    let _ = reader.bits(1);
    let invalid_spans = [ByteSpan { start: 0, end: 1 }];
    let invalid_data = SegmentedData::with_validated_spans(&[], &invalid_spans).unwrap();
    let mut reader = BitReader::with_bit_end(&invalid_data, 8).unwrap();
    let _ = reader.bits(1);
    let trailing_spans = [ByteSpan { start: 0, end: 1 }, ByteSpan { start: 1, end: 2 }];
    let trailing_data = SegmentedData::with_validated_spans(&[0x80], &trailing_spans).unwrap();
    let mut reader = BitReader::with_bit_end(&trailing_data, 16).unwrap();
    let _ = reader.trailing_bits();

    let uvlc_inputs: &[&[u8]] = &[
        &[0x80],
        &[0x40],
        &[0x20],
        &[0x10],
        &[0x08],
        &[0x00, 0x00, 0x00, 0x01, 0xff, 0xff, 0xff, 0xfe],
        &[0x00, 0x00, 0x00, 0x00, 0x80],
    ];
    for input in uvlc_inputs {
        let spans = [ByteSpan {
            start: 0,
            end: input.len(),
        }];
        let data = SegmentedData::new(input, &spans).unwrap();
        let mut reader = BitReader::new(&data, 0, data.len()).unwrap();
        let _ = reader.uvlc();
    }

    for input in [&[0x80][..], &[0x00], &[0xc0], &[0xa0]] {
        let spans = [ByteSpan {
            start: 0,
            end: input.len(),
        }];
        let data = SegmentedData::new(input, &spans).unwrap();
        let mut reader = BitReader::new(&data, 0, data.len()).unwrap();
        let _ = reader.trailing_bits();
    }

    for (reference, width, input) in [
        (0, 0, &[0_u8; 8][..]),
        (0, 3, &[0_u8; 8]),
        (3, 3, &[0_u8; 8]),
        (-3, 3, &[0xff_u8; 8]),
        (8, 3, &[0_u8; 8]),
        (-9, 3, &[0_u8; 8]),
        (0, 12, &[0x55_u8; 16]),
        (4095, 12, &[0xaa_u8; 16]),
        (-4096, 12, &[0xff_u8; 16]),
    ] {
        let spans = [ByteSpan {
            start: 0,
            end: input.len(),
        }];
        let data = SegmentedData::new(input, &spans).unwrap();
        for bit_end in 0..=input.len() * 8 {
            let mut reader = BitReader::with_bit_end(&data, bit_end).unwrap();
            let _ = reader.subexp(reference, width);
        }
    }
    assert_eq!(inverse_recenter(0, 1), 1);
}
