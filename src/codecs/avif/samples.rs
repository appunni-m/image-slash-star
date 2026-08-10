//! Bounded extraction of encoded AV1 sample spans from AVIF containers.

use std::num::NonZeroU32;

use crate::codecs::{CodecError, CodecResult};
use crate::types::{
    AvifAuxiliaryRelationship, AvifChromaSamplePosition, AvifCleanAperture, AvifColorProperties,
    AvifContentLightLevel, AvifGridProperties, AvifItemCodecProperties, AvifItemColorProperties,
    AvifItemIccProfile, AvifItemPlaneProperties, AvifItemProperty, AvifItemRelationship,
    AvifMasteringDisplayColorVolume, AvifMirrorAxis, AvifPixelAspectRatio, AvifRotation,
    AvifTransformProperties, OpaqueMetadata, RawIccProfile, SourceColor,
};

const MAX_BOXES: usize = 4_096;
const MAX_RECORDS: usize = 4_096;
const VISUAL_SAMPLE_ENTRY_SIZE: usize = 78;
const ALPHA_URN_MPEG_B: &[u8] = b"urn:mpeg:mpegB:cicp:systems:auxiliary:alpha";
const ALPHA_URN_HEVC: &[u8] = b"urn:mpeg:hevc:2015:auxid:1";

type FourCc = [u8; 4];
type ParseResult<T> = CodecResult<T>;

macro_rules! parse_failure {
    () => {
        CodecError::Malformed(
            concat!("invalid AVIF sample structure at ", file!(), ":", line!()).to_owned(),
        )
    };
}

macro_rules! parse_need_more {
    ($minimum:expr) => {
        CodecError::NeedMore {
            minimum: $minimum,
            message: concat!("invalid AVIF sample structure at ", file!(), ":", line!()).to_owned(),
        }
    };
}

#[cfg(target_pointer_width = "32")]
fn usize_from_u64(value: u64) -> ParseResult<usize> {
    usize::try_from(value).map_err(|_| parse_failure!())
}

#[cfg(target_pointer_width = "64")]
fn usize_from_u64(value: u64) -> usize {
    usize::from_ne_bytes(value.to_ne_bytes())
}

#[derive(Clone, Copy)]
pub(super) struct ByteSpan {
    pub(super) start: usize,
    pub(super) end: usize,
}

impl ByteSpan {
    fn from_offset_size(
        offset: u64,
        size: u64,
        limit: usize,
        truncation: bool,
    ) -> ParseResult<Self> {
        let end = offset.checked_add(size).ok_or_else(|| parse_failure!())?;
        #[cfg(target_pointer_width = "32")]
        let start = usize_from_u64(offset)?;
        #[cfg(target_pointer_width = "64")]
        let start = usize_from_u64(offset);
        #[cfg(target_pointer_width = "32")]
        let end = usize_from_u64(end)?;
        #[cfg(target_pointer_width = "64")]
        let end = usize_from_u64(end);
        if end > limit {
            if truncation {
                return Err(parse_need_more!(end));
            }
            return Err(parse_failure!());
        }
        Ok(Self { start, end })
    }

    pub(super) fn bytes(self, input: &[u8]) -> ParseResult<&[u8]> {
        input
            .get(self.start..self.end)
            .ok_or_else(|| parse_failure!())
    }

    pub(super) fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }
}

#[derive(Clone, Copy)]
struct BoxSpan {
    kind: FourCc,
    payload: ByteSpan,
}

struct Reader<'input> {
    input: &'input [u8],
    offset: usize,
    end: usize,
    /// When `true`, reads beyond the whole input are incremental truncation;
    /// spans bounded by a validated box are terminal malformed data.
    truncation: bool,
}

impl<'input> Reader<'input> {
    fn new(input: &'input [u8], span: ByteSpan) -> Self {
        Self {
            input,
            offset: span.start,
            end: span.end,
            truncation: false,
        }
    }

    fn whole(input: &'input [u8]) -> Self {
        Self {
            input,
            offset: 0,
            end: input.len(),
            truncation: true,
        }
    }

    fn is_empty(&self) -> bool {
        self.offset == self.end
    }

    fn take_span(&mut self, length: usize) -> ParseResult<ByteSpan> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or_else(|| parse_failure!())?;
        if end > self.end {
            if self.truncation {
                return Err(parse_need_more!(end));
            }
            return Err(parse_failure!());
        }
        let span = ByteSpan {
            start: self.offset,
            end,
        };
        self.offset = end;
        Ok(span)
    }

    fn skip(&mut self, length: usize) -> ParseResult<()> {
        let _ = self.take_span(length)?;
        Ok(())
    }

    fn u8(&mut self) -> ParseResult<u8> {
        Ok(self.take_span(1)?.bytes(self.input)?[0])
    }

    fn u16(&mut self) -> ParseResult<u16> {
        let bytes = self.take_span(2)?.bytes(self.input)?;
        Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
    }

    fn u32(&mut self) -> ParseResult<u32> {
        let bytes = self.take_span(4)?.bytes(self.input)?;
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn u64(&mut self) -> ParseResult<u64> {
        let bytes = self.take_span(8)?.bytes(self.input)?;
        Ok(u64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn uint(&mut self, width: u8) -> ParseResult<u64> {
        match width {
            0 => Ok(0),
            4 => Ok(u64::from(self.u32()?)),
            8 => self.u64(),
            _ => Err(parse_failure!()),
        }
    }

    fn four_cc(&mut self) -> ParseResult<FourCc> {
        let bytes = self.take_span(4)?.bytes(self.input)?;
        Ok([bytes[0], bytes[1], bytes[2], bytes[3]])
    }

    fn c_string(&mut self) -> ParseResult<&'input [u8]> {
        let remaining = self
            .input
            .get(self.offset..self.end)
            .ok_or_else(|| parse_failure!())?;
        let length = remaining
            .iter()
            .position(|&byte| byte == 0)
            .ok_or_else(|| {
                if self.truncation {
                    parse_need_more!(self.end.saturating_add(1))
                } else {
                    parse_failure!()
                }
            })?;
        let value = &remaining[..length];
        self.offset = self.offset.saturating_add(length).saturating_add(1);
        Ok(value)
    }
}

#[derive(Default)]
struct Budget {
    boxes: usize,
    records: usize,
}

impl Budget {
    fn box_seen(&mut self) -> ParseResult<()> {
        self.boxes = self.boxes.checked_add(1).ok_or_else(|| parse_failure!())?;
        if self.boxes > MAX_BOXES {
            return Err(parse_failure!());
        }
        Ok(())
    }

    fn records_seen(&mut self, count: usize) -> ParseResult<()> {
        self.records = self
            .records
            .checked_add(count)
            .ok_or_else(|| parse_failure!())?;
        if self.records > MAX_RECORDS {
            return Err(parse_failure!());
        }
        Ok(())
    }
}

fn next_box(
    reader: &mut Reader<'_>,
    top_level: bool,
    budget: &mut Budget,
) -> ParseResult<Option<BoxSpan>> {
    if reader.is_empty() {
        return Ok(None);
    }
    budget.box_seen()?;
    let start = reader.offset;
    let small_size = reader.u32()?;
    let kind = reader.four_cc()?;
    let mut size = u64::from(small_size);
    if small_size == 1 {
        size = reader.u64()?;
    }
    if kind == *b"uuid" {
        reader.skip(16)?;
    }
    let header_size = reader.offset.saturating_sub(start);
    let size = if size == 0 {
        if !top_level {
            return Err(parse_failure!());
        }
        reader.end.saturating_sub(start)
    } else {
        #[cfg(target_pointer_width = "32")]
        let converted = usize_from_u64(size)?;
        #[cfg(target_pointer_width = "64")]
        let converted = usize_from_u64(size);
        converted
    };
    if size < header_size {
        return Err(parse_failure!());
    }
    let payload = reader.take_span(size.saturating_sub(header_size))?;
    Ok(Some(BoxSpan { kind, payload }))
}

fn parse_full_box(reader: &mut Reader<'_>) -> ParseResult<(u8, u32)> {
    let raw = reader.u32()?;
    let version = raw.to_be_bytes()[0];
    Ok((version, raw & 0x00ff_ffff))
}

#[derive(Clone, Copy)]
struct Brands {
    major: FourCc,
    has_avif: bool,
    has_avis: bool,
}

fn parse_ftyp(input: &[u8], payload: ByteSpan) -> ParseResult<Brands> {
    let mut reader = Reader::new(input, payload);
    let major = reader.four_cc()?;
    reader.skip(4)?;
    if !(reader.end.saturating_sub(reader.offset)).is_multiple_of(4) {
        return Err(parse_failure!());
    }
    let mut has_avif = major == *b"avif";
    let mut has_avis = major == *b"avis";
    let compatible = input
        .get(reader.offset..reader.end)
        .ok_or_else(|| parse_failure!())?;
    for bytes in compatible.chunks_exact(4) {
        let brand = [bytes[0], bytes[1], bytes[2], bytes[3]];
        has_avif |= brand == *b"avif";
        has_avis |= brand == *b"avis";
    }
    if !has_avif && !has_avis {
        return Err(parse_failure!());
    }
    Ok(Brands {
        major,
        has_avif,
        has_avis,
    })
}

#[derive(Clone, Copy)]
struct Item {
    id: u32,
    kind: FourCc,
    /// Stable metadata kind for item types whose payload is retained without
    /// semantic parsing. AVIF MIME items are classified only when their
    /// declared content type is the standard XMP media type.
    metadata_kind: Option<FourCc>,
}

#[derive(Clone)]
enum Property {
    Ispe {
        width: u32,
        height: u32,
    },
    Pixi {
        depth: u8,
    },
    Av1C(ByteSpan),
    AuxC {
        kind: FourCc,
        is_alpha: bool,
        data: ByteSpan,
    },
    Color(AvifColorProperties),
    IccProfile(RawIccProfile),
    ContentLightLevel {
        value: AvifContentLightLevel,
        data: ByteSpan,
    },
    MasteringDisplayColorVolume {
        value: AvifMasteringDisplayColorVolume,
        data: ByteSpan,
    },
    Rotation {
        value: AvifRotation,
        data: ByteSpan,
    },
    Mirror {
        value: AvifMirrorAxis,
        data: ByteSpan,
    },
    PixelAspectRatio {
        value: AvifPixelAspectRatio,
        data: ByteSpan,
    },
    CleanAperture {
        value: AvifCleanAperture,
        data: ByteSpan,
    },
    Other {
        kind: FourCc,
        data: Vec<u8>,
    },
}

#[derive(Clone, Copy)]
struct Association {
    item_id: u32,
    property_index: usize,
}

#[derive(Clone, Copy)]
struct Reference {
    kind: FourCc,
    from_id: u32,
    to_id: u32,
}

#[derive(Clone, Copy)]
enum ExtentSource {
    File,
    Idat,
}

struct ItemLocation {
    item_id: u32,
    source: ExtentSource,
    extents: Vec<ByteSpan>,
}

#[derive(Default)]
struct Meta {
    // parse_meta requires a valid pitm box before returning, so a parsed
    // metadata set always has a nonzero primary item identifier. Keeping that
    // invariant in the representation avoids manufacturing a fallback item
    // id in source-color extraction.
    primary_item_id: u32,
    items: Vec<Item>,
    properties: Vec<Property>,
    associations: Vec<Association>,
    references: Vec<Reference>,
    locations: Vec<ItemLocation>,
}

fn parse_meta(input: &[u8], payload: ByteSpan, budget: &mut Budget) -> ParseResult<Meta> {
    let mut reader = Reader::new(input, payload);
    let (version, _) = parse_full_box(&mut reader)?;
    if version != 0 {
        return Err(parse_failure!());
    }

    let mut meta = Meta::default();
    let mut handler_seen = false;
    let mut pitm_seen = false;
    let mut iinf_seen = false;
    let mut iprp_seen = false;
    let mut iref_seen = false;
    let mut iloc = None;
    let mut idat = None;

    while let Some(child) = next_box(&mut reader, false, budget)? {
        match child.kind {
            kind if kind == *b"hdlr" => {
                if handler_seen || parse_handler(input, child.payload)? != *b"pict" {
                    return Err(parse_failure!());
                }
                handler_seen = true;
            }
            kind if kind == *b"pitm" => {
                if pitm_seen {
                    return Err(parse_failure!());
                }
                pitm_seen = true;
                meta.primary_item_id = parse_pitm(input, child.payload)?;
            }
            kind if kind == *b"iinf" => {
                if iinf_seen {
                    return Err(parse_failure!());
                }
                iinf_seen = true;
                parse_iinf(input, child.payload, &mut meta, budget)?;
            }
            kind if kind == *b"iprp" => {
                if iprp_seen {
                    return Err(parse_failure!());
                }
                iprp_seen = true;
                parse_iprp(input, child.payload, &mut meta, budget)?;
            }
            kind if kind == *b"iref" => {
                if iref_seen {
                    return Err(parse_failure!());
                }
                iref_seen = true;
                parse_iref(input, child.payload, &mut meta, budget)?;
            }
            kind if kind == *b"iloc" => {
                if iloc.replace(child.payload).is_some() {
                    return Err(parse_failure!());
                }
            }
            kind if kind == *b"idat" => {
                if idat.is_some() || child.payload.len() == 0 {
                    return Err(parse_failure!());
                }
                idat = Some(child.payload);
            }
            _ => {}
        }
    }

    if !handler_seen || !pitm_seen || !iinf_seen || !iprp_seen {
        return Err(parse_failure!());
    }
    let iloc = iloc.ok_or_else(|| parse_failure!())?;
    parse_iloc(input, iloc, idat, &mut meta, budget)?;
    Ok(meta)
}

fn parse_handler(input: &[u8], payload: ByteSpan) -> ParseResult<FourCc> {
    let mut reader = Reader::new(input, payload);
    let (version, _) = parse_full_box(&mut reader)?;
    if version != 0 || reader.u32()? != 0 {
        return Err(parse_failure!());
    }
    let handler = reader.four_cc()?;
    reader.skip(12)?;
    let _ = reader.c_string()?;
    Ok(handler)
}

fn parse_pitm(input: &[u8], payload: ByteSpan) -> ParseResult<u32> {
    let mut reader = Reader::new(input, payload);
    let (version, _) = parse_full_box(&mut reader)?;
    let item_id = if version == 0 {
        u32::from(reader.u16()?)
    } else {
        reader.u32()?
    };
    if item_id == 0 {
        return Err(parse_failure!());
    }
    Ok(item_id)
}

fn parse_iinf(
    input: &[u8],
    payload: ByteSpan,
    meta: &mut Meta,
    budget: &mut Budget,
) -> ParseResult<()> {
    let mut reader = Reader::new(input, payload);
    let (version, _) = parse_full_box(&mut reader)?;
    let entry_count = match version {
        0 => usize::from(reader.u16()?),
        1 => reader.u32()? as usize,
        _ => return Err(parse_failure!()),
    };
    budget.records_seen(entry_count)?;
    meta.items.reserve(entry_count);
    for _ in 0..entry_count {
        let child = next_box(&mut reader, false, budget)?.ok_or_else(|| parse_failure!())?;
        if child.kind != *b"infe" {
            return Err(parse_failure!());
        }
        let item = parse_infe(input, child.payload)?;
        if meta.items.iter().any(|existing| existing.id == item.id) {
            return Err(parse_failure!());
        }
        meta.items.push(item);
    }
    if !reader.is_empty() {
        return Err(parse_failure!());
    }
    Ok(())
}

fn parse_infe(input: &[u8], payload: ByteSpan) -> ParseResult<Item> {
    let mut reader = Reader::new(input, payload);
    let (version, _) = parse_full_box(&mut reader)?;
    let id = match version {
        2 => u32::from(reader.u16()?),
        3 => reader.u32()?,
        _ => return Err(parse_failure!()),
    };
    if id == 0 {
        return Err(parse_failure!());
    }
    let _ = reader.u16()?;
    let kind = reader.four_cc()?;
    let _ = reader.c_string()?;
    let metadata_kind = if kind == *b"Exif" {
        Some(kind)
    } else if kind == *b"mime" {
        let content_type = reader.c_string()?;
        (content_type == b"application/rdf+xml").then_some(*b"XMP ")
    } else {
        None
    };
    Ok(Item {
        id,
        kind,
        metadata_kind,
    })
}

fn parse_iprp(
    input: &[u8],
    payload: ByteSpan,
    meta: &mut Meta,
    budget: &mut Budget,
) -> ParseResult<()> {
    let mut reader = Reader::new(input, payload);
    let ipco = next_box(&mut reader, false, budget)?.ok_or_else(|| parse_failure!())?;
    if ipco.kind != *b"ipco" {
        return Err(parse_failure!());
    }
    parse_ipco(input, ipco.payload, meta, budget)?;
    let mut ipma_seen = false;
    while let Some(child) = next_box(&mut reader, false, budget)? {
        if child.kind != *b"ipma" || ipma_seen {
            return Err(parse_failure!());
        }
        ipma_seen = true;
        parse_ipma(input, child.payload, meta, budget)?;
    }
    if !ipma_seen {
        return Err(parse_failure!());
    }
    Ok(())
}

fn parse_ipco(
    input: &[u8],
    payload: ByteSpan,
    meta: &mut Meta,
    budget: &mut Budget,
) -> ParseResult<()> {
    let mut reader = Reader::new(input, payload);
    while let Some(child) = next_box(&mut reader, false, budget)? {
        budget.records_seen(1)?;
        meta.properties.push(parse_property(input, child)?);
    }
    Ok(())
}

fn parse_property(input: &[u8], property: BoxSpan) -> ParseResult<Property> {
    match property.kind {
        kind if kind == *b"ispe" => {
            let mut reader = Reader::new(input, property.payload);
            let (version, _) = parse_full_box(&mut reader)?;
            if version != 0 {
                return Err(parse_failure!());
            }
            let width = reader.u32()?;
            let height = reader.u32()?;
            if width == 0 || height == 0 {
                return Err(parse_failure!());
            }
            Ok(Property::Ispe { width, height })
        }
        kind if kind == *b"pixi" => {
            let mut reader = Reader::new(input, property.payload);
            let (version, _) = parse_full_box(&mut reader)?;
            if version != 0 {
                return Err(parse_failure!());
            }
            let planes = reader.u8()?;
            if !(1..=4).contains(&planes) {
                return Err(parse_failure!());
            }
            let depth = reader.u8()?;
            if depth == 0 || depth > 16 {
                return Err(parse_failure!());
            }
            for _ in 1..planes {
                if reader.u8()? != depth {
                    return Err(parse_failure!());
                }
            }
            Ok(Property::Pixi { depth })
        }
        kind if kind == *b"av1C" => {
            let _ = parse_av1c_declaration(property.payload.bytes(input)?)?;
            Ok(Property::Av1C(property.payload))
        }
        kind if kind == *b"colr" => parse_colr(input, property.payload),
        kind if kind == *b"clli" => parse_clli(input, property.payload),
        kind if kind == *b"mdcv" => parse_mdcv(input, property.payload),
        kind if kind == *b"irot" => parse_irot(input, property.payload),
        kind if kind == *b"imir" => parse_imir(input, property.payload),
        kind if kind == *b"pasp" => parse_pasp(input, property.payload),
        kind if kind == *b"clap" => parse_clap(input, property.payload),
        [b'a', b'u', b'x', b'C'] | [b'a', b'u', b'x', b'i'] => {
            let mut reader = Reader::new(input, property.payload);
            let (version, _) = parse_full_box(&mut reader)?;
            if version != 0 {
                return Err(parse_failure!());
            }
            let urn = reader.c_string()?;
            Ok(Property::AuxC {
                kind: property.kind,
                is_alpha: urn == ALPHA_URN_MPEG_B || urn == ALPHA_URN_HEVC,
                data: property.payload,
            })
        }
        _ => Ok(Property::Other {
            kind: property.kind,
            data: property.payload.bytes(input)?.to_vec(),
        }),
    }
}

fn parse_av1c_declaration(payload: &[u8]) -> ParseResult<(u8, AvifChromaSamplePosition)> {
    let span = ByteSpan {
        start: 0,
        end: payload.len(),
    };
    let mut reader = Reader::new(payload, span);
    if reader.u8()? != 0x81 {
        return Err(parse_failure!());
    }
    let _ = reader.u8()?;
    let flags = reader.u8()?;
    let _ = reader.u8()?;
    let high_bit_depth = flags & 0x40 != 0;
    let twelve_bit = flags & 0x20 != 0;
    if twelve_bit && !high_bit_depth {
        return Err(parse_failure!());
    }
    let bit_depth = if twelve_bit {
        12
    } else if high_bit_depth {
        10
    } else {
        8
    };
    Ok((bit_depth, AvifChromaSamplePosition::from_code(flags & 3)))
}

fn parse_colr(input: &[u8], payload: ByteSpan) -> ParseResult<Property> {
    let mut reader = Reader::new(input, payload);
    let color_type = reader.four_cc()?;
    match color_type {
        kind if kind == *b"rICC" || kind == *b"prof" => {
            // `payload` is a validated box span and `four_cc` has already
            // consumed four bytes inside it, so the remaining span is
            // bounded by construction and cannot fail another checked read.
            let data = &input[reader.offset..reader.end];
            if data.is_empty() {
                return Err(parse_failure!());
            }
            return Ok(Property::IccProfile(RawIccProfile {
                keyword: color_type.to_vec(),
                data: data.to_vec(),
            }));
        }
        kind if kind != *b"nclx" => {
            return Ok(Property::Other {
                kind,
                data: payload.bytes(input)?.to_vec(),
            });
        }
        _ => {}
    }
    let color = AvifColorProperties {
        color_primaries: reader.u16()?,
        transfer_characteristics: reader.u16()?,
        matrix_coefficients: reader.u16()?,
        full_range: {
            let flags = reader.u8()?;
            if flags & 0x7f != 0 {
                return Err(parse_failure!());
            }
            flags & 0x80 != 0
        },
    };
    if !reader.is_empty() {
        return Err(parse_failure!());
    }
    Ok(Property::Color(color))
}

fn parse_clli(input: &[u8], payload: ByteSpan) -> ParseResult<Property> {
    let mut reader = Reader::new(input, payload);
    let content_light_level = AvifContentLightLevel::new(reader.u16()?, reader.u16()?);
    if !reader.is_empty() {
        return Err(parse_failure!());
    }
    Ok(Property::ContentLightLevel {
        value: content_light_level,
        data: payload,
    })
}

fn parse_mdcv(input: &[u8], payload: ByteSpan) -> ParseResult<Property> {
    let mut reader = Reader::new(input, payload);
    // ISO/IEC 14496-12 stores the three primaries in G, B, R order. Keep the
    // public descriptor in the conventional R, G, B order while retaining
    // each encoded 16-bit coordinate exactly.
    let green_x = reader.u16()?;
    let green_y = reader.u16()?;
    let blue_x = reader.u16()?;
    let blue_y = reader.u16()?;
    let red_x = reader.u16()?;
    let red_y = reader.u16()?;
    let white_point_x = reader.u16()?;
    let white_point_y = reader.u16()?;
    let max_display_mastering_luminance = reader.u32()?;
    let min_display_mastering_luminance = reader.u32()?;
    if !reader.is_empty() {
        return Err(parse_failure!());
    }
    Ok(Property::MasteringDisplayColorVolume {
        value: AvifMasteringDisplayColorVolume::new(
            red_x,
            red_y,
            green_x,
            green_y,
            blue_x,
            blue_y,
            white_point_x,
            white_point_y,
            max_display_mastering_luminance,
            min_display_mastering_luminance,
        ),
        data: payload,
    })
}

fn parse_irot(input: &[u8], payload: ByteSpan) -> ParseResult<Property> {
    let mut reader = Reader::new(input, payload);
    let rotation = match reader.u8()? {
        0 => AvifRotation::Zero,
        1 => AvifRotation::CounterClockwise90,
        2 => AvifRotation::CounterClockwise180,
        3 => AvifRotation::CounterClockwise270,
        _ => return Err(parse_failure!()),
    };
    if !reader.is_empty() {
        return Err(parse_failure!());
    }
    Ok(Property::Rotation {
        value: rotation,
        data: payload,
    })
}

fn parse_imir(input: &[u8], payload: ByteSpan) -> ParseResult<Property> {
    let mut reader = Reader::new(input, payload);
    let mirror = match reader.u8()? {
        0 => AvifMirrorAxis::TopBottom,
        1 => AvifMirrorAxis::LeftRight,
        _ => return Err(parse_failure!()),
    };
    if !reader.is_empty() {
        return Err(parse_failure!());
    }
    Ok(Property::Mirror {
        value: mirror,
        data: payload,
    })
}

fn parse_pasp(input: &[u8], payload: ByteSpan) -> ParseResult<Property> {
    let mut reader = Reader::new(input, payload);
    let h_spacing = reader.u32()?;
    let v_spacing = reader.u32()?;
    if h_spacing == 0 || v_spacing == 0 || !reader.is_empty() {
        return Err(parse_failure!());
    }
    Ok(Property::PixelAspectRatio {
        value: AvifPixelAspectRatio::new(h_spacing, v_spacing),
        data: payload,
    })
}

fn parse_clap(input: &[u8], payload: ByteSpan) -> ParseResult<Property> {
    let mut reader = Reader::new(input, payload);
    let width_numerator = reader.u32()?;
    let width_denominator = reader.u32()?;
    let height_numerator = reader.u32()?;
    let height_denominator = reader.u32()?;
    let horizontal_offset_numerator = i32::from_be_bytes(reader.u32()?.to_be_bytes());
    let horizontal_offset_denominator = reader.u32()?;
    let vertical_offset_numerator = i32::from_be_bytes(reader.u32()?.to_be_bytes());
    let vertical_offset_denominator = reader.u32()?;
    if width_numerator == 0
        || width_denominator == 0
        || height_numerator == 0
        || height_denominator == 0
        || horizontal_offset_denominator == 0
        || vertical_offset_denominator == 0
        || !reader.is_empty()
    {
        return Err(parse_failure!());
    }
    Ok(Property::CleanAperture {
        value: AvifCleanAperture::new(
            width_numerator,
            width_denominator,
            height_numerator,
            height_denominator,
            horizontal_offset_numerator,
            horizontal_offset_denominator,
            vertical_offset_numerator,
            vertical_offset_denominator,
        ),
        data: payload,
    })
}

fn parse_ipma(
    input: &[u8],
    payload: ByteSpan,
    meta: &mut Meta,
    budget: &mut Budget,
) -> ParseResult<()> {
    let mut reader = Reader::new(input, payload);
    let (version, flags) = parse_full_box(&mut reader)?;
    let wide = flags & 1 != 0;
    let entry_count = reader.u32()? as usize;
    budget.records_seen(entry_count)?;
    let mut previous_id = 0;
    for _ in 0..entry_count {
        let item_id = if version == 0 {
            u32::from(reader.u16()?)
        } else {
            reader.u32()?
        };
        if item_id == 0 || item_id <= previous_id {
            return Err(parse_failure!());
        }
        previous_id = item_id;
        let association_count = usize::from(reader.u8()?);
        budget.records_seen(association_count)?;
        for _ in 0..association_count {
            let raw = if wide {
                u32::from(reader.u16()?)
            } else {
                u32::from(reader.u8()?)
            };
            let essential_mask = if wide { 0x8000 } else { 0x80 };
            let index_mask = if wide { 0x7fff } else { 0x7f };
            let property_index = raw & index_mask;
            if property_index == 0 {
                if raw & essential_mask != 0 {
                    return Err(parse_failure!());
                }
                continue;
            }
            let property_index = property_index.saturating_sub(1) as usize;
            if property_index >= meta.properties.len() {
                return Err(parse_failure!());
            }
            meta.associations.push(Association {
                item_id,
                property_index,
            });
        }
    }
    if !reader.is_empty() {
        return Err(parse_failure!());
    }
    Ok(())
}

fn parse_iref(
    input: &[u8],
    payload: ByteSpan,
    meta: &mut Meta,
    budget: &mut Budget,
) -> ParseResult<()> {
    let mut reader = Reader::new(input, payload);
    let (version, _) = parse_full_box(&mut reader)?;
    if version > 1 {
        return Ok(());
    }
    while let Some(child) = next_box(&mut reader, false, budget)? {
        let mut references = Reader::new(input, child.payload);
        let from_id = if version == 0 {
            u32::from(references.u16()?)
        } else {
            references.u32()?
        };
        let count = usize::from(references.u16()?);
        if from_id == 0 {
            return Err(parse_failure!());
        }
        budget.records_seen(count)?;
        for _ in 0..count {
            let to_id = if version == 0 {
                u32::from(references.u16()?)
            } else {
                references.u32()?
            };
            if to_id == 0 {
                return Err(parse_failure!());
            }
            meta.references.push(Reference {
                kind: child.kind,
                from_id,
                to_id,
            });
        }
        if !references.is_empty() {
            return Err(parse_failure!());
        }
    }
    Ok(())
}

// ✅ VERIFIED: libavif 1.4.1 read.c:1979-2103. Field widths,
// construction methods, and extent arithmetic match the pinned source.
fn parse_iloc(
    input: &[u8],
    payload: ByteSpan,
    idat: Option<ByteSpan>,
    meta: &mut Meta,
    budget: &mut Budget,
) -> ParseResult<()> {
    let mut reader = Reader::new(input, payload);
    let (version, _) = parse_full_box(&mut reader)?;
    if version > 2 {
        return Err(parse_failure!());
    }
    let field_sizes = reader.u16()?;
    let offset_size = field_sizes.to_be_bytes()[0] >> 4;
    let length_size = field_sizes.to_be_bytes()[0] & 0x0f;
    let base_offset_size = field_sizes.to_be_bytes()[1] >> 4;
    let index_size = if matches!(version, 1 | 2) {
        field_sizes.to_be_bytes()[1] & 0x0f
    } else {
        0
    };
    if [offset_size, length_size, base_offset_size, index_size]
        .into_iter()
        .any(|width| !matches!(width, 0 | 4 | 8))
    {
        return Err(parse_failure!());
    }
    let item_count = if version < 2 {
        usize::from(reader.u16()?)
    } else {
        reader.u32()? as usize
    };
    budget.records_seen(item_count)?;
    meta.locations.reserve(item_count);
    for _ in 0..item_count {
        let item_id = if version < 2 {
            u32::from(reader.u16()?)
        } else {
            reader.u32()?
        };
        if item_id == 0
            || meta
                .locations
                .iter()
                .any(|location| location.item_id == item_id)
        {
            return Err(parse_failure!());
        }
        let method = if matches!(version, 1 | 2) {
            let construction = reader.u16()?;
            if construction & 0xfff0 != 0 {
                return Err(parse_failure!());
            }
            construction.to_be_bytes()[1] & 0x0f
        } else {
            0
        };
        let source = match method {
            0 => ExtentSource::File,
            1 => ExtentSource::Idat,
            _ => return Err(parse_failure!()),
        };
        if reader.u16()? != 0 {
            return Err(parse_failure!());
        }
        let base_offset = reader.uint(base_offset_size)?;
        let extent_count = usize::from(reader.u16()?);
        budget.records_seen(extent_count)?;
        let mut extents = Vec::with_capacity(extent_count);
        for _ in 0..extent_count {
            if index_size != 0 {
                let _ = reader.uint(index_size)?;
            }
            let extent_offset = reader.uint(offset_size)?;
            let extent_length = reader.uint(length_size)?;
            let relative = base_offset
                .checked_add(extent_offset)
                .ok_or_else(|| parse_failure!())?;
            let span = match source {
                ExtentSource::File => {
                    ByteSpan::from_offset_size(relative, extent_length, input.len(), true)?
                }
                ExtentSource::Idat => {
                    let idat = idat.ok_or_else(|| parse_failure!())?;
                    let start = (idat.start as u64)
                        .checked_add(relative)
                        .ok_or_else(|| parse_failure!())?;
                    ByteSpan::from_offset_size(start, extent_length, idat.end, false)?
                }
            };
            extents.push(span);
        }
        meta.locations.push(ItemLocation {
            item_id,
            source,
            extents,
        });
    }
    if !reader.is_empty() {
        return Err(parse_failure!());
    }
    Ok(())
}

impl Meta {
    fn item(&self, item_id: u32) -> Option<&Item> {
        self.items.iter().find(|item| item.id == item_id)
    }

    fn location(&self, item_id: u32) -> Option<&ItemLocation> {
        self.locations
            .iter()
            .find(|location| location.item_id == item_id)
    }

    fn metadata(&self, input: &[u8]) -> ParseResult<Vec<OpaqueMetadata>> {
        let mut metadata = Vec::new();
        for item in &self.items {
            let Some(kind) = item.metadata_kind else {
                continue;
            };
            let location = self.location(item.id).ok_or_else(|| parse_failure!())?;
            if location.extents.is_empty() {
                return Err(parse_failure!());
            }
            let capacity = location.extents.iter().try_fold(0_usize, |total, span| {
                total
                    .checked_add(span.len())
                    .ok_or_else(|| parse_failure!())
            })?;
            let mut data = Vec::with_capacity(capacity);
            for span in &location.extents {
                data.extend_from_slice(span.bytes(input)?);
            }
            metadata.push(OpaqueMetadata {
                kind: kind.to_vec(),
                data,
            });
        }
        Ok(metadata)
    }

    fn associated(&self, item_id: u32) -> impl Iterator<Item = &Property> {
        self.associations
            .iter()
            .filter(move |association| association.item_id == item_id)
            .filter_map(|association| self.properties.get(association.property_index))
    }

    fn is_alpha(&self, item_id: u32) -> bool {
        self.associated(item_id)
            .any(|property| matches!(property, Property::AuxC { is_alpha: true, .. }))
    }

    fn source_color(&self, input: &[u8]) -> ParseResult<SourceColor> {
        let mut source_color = SourceColor::new();
        if let Some(color) =
            self.associated(self.primary_item_id)
                .find_map(|property| match property {
                    Property::Color(color) => Some(*color),
                    _ => None,
                })
        {
            source_color = source_color.with_avif_color(color);
        }
        if let Some(span) = self.associated(self.primary_item_id).find_map(|property| {
            if let Property::Av1C(span) = property {
                Some(*span)
            } else {
                None
            }
        }) {
            // The span was created from a bounded property payload, so it is
            // still within `input` after container validation.
            let bytes = span.bytes(input)?;
            let (_, chroma_sample_position) = parse_av1c_declaration(bytes)?;
            source_color = source_color.with_avif_chroma_sample_position(chroma_sample_position);
        }
        if let Some(profile) =
            self.associated(self.primary_item_id)
                .find_map(|property| match property {
                    Property::IccProfile(profile) => Some(profile.clone()),
                    _ => None,
                })
        {
            source_color = source_color.with_icc_profile(profile);
        }
        if let Some(content_light_level) =
            self.associated(self.primary_item_id)
                .find_map(|property| match property {
                    Property::ContentLightLevel { value, .. } => Some(*value),
                    _ => None,
                })
        {
            source_color = source_color.with_avif_content_light_level(content_light_level);
        }
        let mut mastering_display_color_volume = None;
        for property in self.associated(self.primary_item_id) {
            if let Property::MasteringDisplayColorVolume { value, .. } = property
                && mastering_display_color_volume.replace(*value).is_some()
            {
                return Err(parse_failure!());
            }
        }
        if let Some(mastering_display_color_volume) = mastering_display_color_volume {
            source_color = source_color
                .with_avif_mastering_display_color_volume(mastering_display_color_volume);
        }
        Ok(source_color)
    }

    fn transform(&self) -> ParseResult<Option<AvifTransformProperties>> {
        let mut transform = AvifTransformProperties::new();
        for property in self.associated(self.primary_item_id) {
            match property {
                Property::Rotation {
                    value: rotation, ..
                } => {
                    if transform.rotation().is_some() {
                        return Err(parse_failure!());
                    }
                    transform = transform.with_rotation(*rotation);
                }
                Property::Mirror { value: mirror, .. } => {
                    if transform.mirror().is_some() {
                        return Err(parse_failure!());
                    }
                    transform = transform.with_mirror(*mirror);
                }
                Property::PixelAspectRatio { value: ratio, .. } => {
                    if transform.pixel_aspect_ratio().is_some() {
                        return Err(parse_failure!());
                    }
                    transform = transform.with_pixel_aspect_ratio(*ratio);
                }
                Property::CleanAperture {
                    value: clean_aperture,
                    ..
                } => {
                    if transform.clean_aperture().is_some() {
                        return Err(parse_failure!());
                    }
                    transform = transform.with_clean_aperture(*clean_aperture);
                }
                _ => {}
            }
        }
        Ok((!transform.is_empty()).then_some(transform))
    }

    fn av1c(&self, item_id: u32) -> ParseResult<ByteSpan> {
        let mut configs = self.associated(item_id).filter_map(|property| {
            if let Property::Av1C(span) = property {
                Some(*span)
            } else {
                None
            }
        });
        let config = configs.next().ok_or_else(|| parse_failure!())?;
        if configs.next().is_some() {
            return Err(parse_failure!());
        }
        Ok(config)
    }

    fn dimg_children(&self, item_id: u32) -> Vec<u32> {
        self.references
            .iter()
            .filter(|reference| reference.kind == *b"dimg" && reference.from_id == item_id)
            .map(|reference| reference.to_id)
            .collect()
    }

    fn alpha_targeting(&self, item_id: u32) -> ParseResult<Option<u32>> {
        let mut matches = self
            .references
            .iter()
            .filter(|reference| {
                reference.kind == *b"auxl"
                    && reference.to_id == item_id
                    && self.is_alpha(reference.from_id)
            })
            .map(|reference| reference.from_id);
        let result = matches.next();
        if matches.next().is_some() {
            return Err(parse_failure!());
        }
        Ok(result)
    }

    fn alpha_auxiliary_relationships(
        &self,
        primary_item_id: u32,
    ) -> ParseResult<Vec<AvifAuxiliaryRelationship>> {
        let mut color_items = vec![primary_item_id];
        loop {
            let mut changed = false;
            for item_id in color_items.clone() {
                for child in self.dimg_children(item_id) {
                    if !color_items.contains(&child) {
                        color_items.push(child);
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }

        let mut relationships = Vec::new();
        for target in color_items {
            if let Some(auxiliary_item_id) = self.alpha_targeting(target)? {
                relationships.push(AvifAuxiliaryRelationship::new(auxiliary_item_id, target));
            }
        }
        Ok(relationships)
    }

    fn non_alpha_item_relationships(&self) -> Vec<AvifItemRelationship> {
        self.references
            .iter()
            .filter(|reference| !(reference.kind == *b"auxl" && self.is_alpha(reference.from_id)))
            .map(|reference| {
                AvifItemRelationship::new(reference.kind, reference.from_id, reference.to_id)
            })
            .collect()
    }

    fn premultiplied_relationships(&self) -> Vec<AvifItemRelationship> {
        self.references
            .iter()
            .filter(|reference| reference.kind == *b"prem")
            .map(|reference| {
                AvifItemRelationship::new(reference.kind, reference.from_id, reference.to_id)
            })
            .collect()
    }

    fn non_primary_item_color_properties(
        &self,
        primary_item_id: u32,
    ) -> Vec<AvifItemColorProperties> {
        self.associations
            .iter()
            .filter(|association| association.item_id != primary_item_id)
            .filter_map(|association| {
                self.properties.get(association.property_index).and_then(
                    |property| match property {
                        Property::Color(color) => {
                            Some(AvifItemColorProperties::new(association.item_id, *color))
                        }
                        _ => None,
                    },
                )
            })
            .collect()
    }

    fn non_primary_item_icc_profiles(&self, primary_item_id: u32) -> Vec<AvifItemIccProfile> {
        self.associations
            .iter()
            .filter(|association| association.item_id != primary_item_id)
            .filter_map(|association| {
                self.properties.get(association.property_index).and_then(
                    |property| match property {
                        Property::IccProfile(profile) => Some(AvifItemIccProfile::new(
                            association.item_id,
                            profile.clone(),
                        )),
                        _ => None,
                    },
                )
            })
            .collect()
    }

    fn non_primary_item_properties(
        &self,
        input: &[u8],
        primary_item_id: u32,
    ) -> ParseResult<Vec<AvifItemProperty>> {
        let mut result = Vec::new();
        for association in self
            .associations
            .iter()
            .filter(|association| association.item_id != primary_item_id)
        {
            let Some(property) = self.properties.get(association.property_index) else {
                continue;
            };
            let record = match property {
                Property::ContentLightLevel { data, .. } => Some(AvifItemProperty::new(
                    association.item_id,
                    *b"clli",
                    data.bytes(input)?.to_vec(),
                )),
                Property::MasteringDisplayColorVolume { data, .. } => Some(AvifItemProperty::new(
                    association.item_id,
                    *b"mdcv",
                    data.bytes(input)?.to_vec(),
                )),
                Property::Rotation { data, .. } => Some(AvifItemProperty::new(
                    association.item_id,
                    *b"irot",
                    data.bytes(input)?.to_vec(),
                )),
                Property::Mirror { data, .. } => Some(AvifItemProperty::new(
                    association.item_id,
                    *b"imir",
                    data.bytes(input)?.to_vec(),
                )),
                Property::PixelAspectRatio { data, .. } => Some(AvifItemProperty::new(
                    association.item_id,
                    *b"pasp",
                    data.bytes(input)?.to_vec(),
                )),
                Property::CleanAperture { data, .. } => Some(AvifItemProperty::new(
                    association.item_id,
                    *b"clap",
                    data.bytes(input)?.to_vec(),
                )),
                Property::AuxC {
                    is_alpha: false,
                    kind,
                    data,
                } => Some(AvifItemProperty::new(
                    association.item_id,
                    *kind,
                    data.bytes(input)?.to_vec(),
                )),
                Property::Other { kind, data } => Some(AvifItemProperty::new(
                    association.item_id,
                    *kind,
                    data.clone(),
                )),
                _ => None,
            };
            if let Some(record) = record {
                result.push(record);
            }
        }
        Ok(result)
    }

    fn non_primary_item_plane_properties(
        &self,
        primary_item_id: u32,
    ) -> ParseResult<Vec<AvifItemPlaneProperties>> {
        let mut result = Vec::new();
        for item in &self.items {
            if item.id == primary_item_id {
                continue;
            }
            let mut dimensions = None;
            let mut bit_depth = None;
            for property in self.associated(item.id) {
                match property {
                    Property::Ispe { width, height } => {
                        if dimensions.replace((*width, *height)).is_some() {
                            return Err(parse_failure!());
                        }
                    }
                    Property::Pixi { depth } if bit_depth.replace(*depth).is_some() => {
                        return Err(parse_failure!());
                    }
                    _ => {}
                }
            }
            if dimensions.is_some() || bit_depth.is_some() {
                let (width, height) =
                    dimensions.map_or((None, None), |(width, height)| (Some(width), Some(height)));
                result.push(AvifItemPlaneProperties::new(
                    item.id, width, height, bit_depth,
                ));
            }
        }
        Ok(result)
    }

    fn non_primary_item_codec_properties(
        &self,
        input: &[u8],
        primary_item_id: u32,
    ) -> ParseResult<Vec<AvifItemCodecProperties>> {
        let mut result = Vec::new();
        for item in &self.items {
            if item.id == primary_item_id {
                continue;
            }
            let mut codec = None;
            for property in self.associated(item.id) {
                if let Property::Av1C(span) = property {
                    if codec.is_some() {
                        return Err(parse_failure!());
                    }
                    let data = span.bytes(input)?.to_vec();
                    let (bit_depth, chroma_sample_position) = parse_av1c_declaration(&data)?;
                    codec = Some((data, bit_depth, chroma_sample_position));
                }
            }
            if let Some((data, bit_depth, chroma_sample_position)) = codec {
                result.push(AvifItemCodecProperties::new(
                    item.id,
                    data,
                    bit_depth,
                    chroma_sample_position,
                ));
            }
        }
        Ok(result)
    }

    fn grid_item_ids(&self, primary_item_id: u32) -> ParseResult<Vec<u32>> {
        let item = self
            .items
            .iter()
            .find(|item| item.id == primary_item_id)
            .ok_or_else(|| parse_failure!())?;
        if item.kind != *b"grid" {
            return Ok(Vec::new());
        }
        let item_ids = self.dimg_children(primary_item_id);
        if item_ids.is_empty() {
            return Err(parse_failure!());
        }
        Ok(item_ids)
    }

    fn grid_properties(
        &self,
        input: &[u8],
        primary_item_id: u32,
    ) -> ParseResult<Option<AvifGridProperties>> {
        let item = self
            .items
            .iter()
            .find(|item| item.id == primary_item_id)
            .ok_or_else(|| parse_failure!())?;
        if item.kind != *b"grid" {
            return Ok(None);
        }
        let location = self
            .location(primary_item_id)
            .ok_or_else(|| parse_failure!())?;
        let total_length = location.extents.iter().try_fold(0usize, |length, extent| {
            length
                .checked_add(extent.len())
                .ok_or_else(|| parse_failure!())
        })?;
        if total_length < 4 {
            return Err(parse_failure!());
        }

        // The grid item payload is at most twelve bytes for the supported
        // version. Copy only that bounded prefix, even when an untrusted iloc
        // entry describes a much larger item.
        let mut prefix = [0u8; 12];
        let mut copied = 0usize;
        for extent in &location.extents {
            let bytes = extent.bytes(input)?;
            let remaining = prefix.len().saturating_sub(copied);
            let count = bytes.len().min(remaining);
            prefix[copied..copied.saturating_add(count)].copy_from_slice(&bytes[..count]);
            copied = copied.saturating_add(count);
            if copied == prefix.len() {
                break;
            }
        }

        let version = prefix[0];
        if version != 0 {
            return Err(CodecError::Unsupported(format!(
                "AVIF grid item version {version} is not implemented"
            )));
        }
        let flags = prefix[1];
        let rows = u32::from(prefix[2]).saturating_add(1);
        let columns = u32::from(prefix[3]).saturating_add(1);
        let field_width: usize = if flags & 1 == 0 { 2 } else { 4 };
        let expected_length = 4usize.saturating_add(field_width.saturating_mul(2));
        if total_length != expected_length || copied < expected_length {
            return Err(parse_failure!());
        }
        let (output_width, output_height) = if field_width == 2 {
            (
                u32::from(u16::from_be_bytes([prefix[4], prefix[5]])),
                u32::from(u16::from_be_bytes([prefix[6], prefix[7]])),
            )
        } else {
            (
                u32::from_be_bytes([prefix[4], prefix[5], prefix[6], prefix[7]]),
                u32::from_be_bytes([prefix[8], prefix[9], prefix[10], prefix[11]]),
            )
        };
        if output_width == 0 || output_height == 0 {
            return Err(parse_failure!());
        }
        Ok(Some(AvifGridProperties::new(
            version,
            flags,
            rows,
            columns,
            output_width,
            output_height,
        )))
    }
}

pub(super) struct EncodedSample {
    pub(super) spans: Vec<ByteSpan>,
    pub(super) config: ByteSpan,
    pub(super) sync: bool,
    pub(super) duration: u32,
}

pub(super) struct EncodedPlane {
    pub(super) samples: Vec<EncodedSample>,
}

pub(super) struct StillPayload {
    pub(super) color: EncodedPlane,
    pub(super) alpha: Option<EncodedPlane>,
}

pub(super) struct SequencePayload {
    pub(super) color: EncodedPlane,
    pub(super) alpha: Option<EncodedPlane>,
    pub(super) timescale: NonZeroU32,
}

pub(super) struct ExtractedAvif<'input> {
    pub(super) input: &'input [u8],
    pub(super) still: Option<StillPayload>,
    pub(super) sequence: Option<SequencePayload>,
    /// Encoded bytes of the parsed top-level BMFF extent.
    pub(super) consumed: usize,
    pub(super) retained_boxes: Vec<crate::types::OpaqueBlock>,
    pub(super) metadata: Vec<OpaqueMetadata>,
    pub(super) source_color: SourceColor,
    pub(super) auxiliary_relationship: Option<AvifAuxiliaryRelationship>,
    pub(super) auxiliary_relationships: Vec<AvifAuxiliaryRelationship>,
    pub(super) item_relationships: Vec<AvifItemRelationship>,
    pub(super) premultiplied_relationships: Vec<AvifItemRelationship>,
    pub(super) item_color_properties: Vec<AvifItemColorProperties>,
    pub(super) item_icc_profiles: Vec<AvifItemIccProfile>,
    pub(super) item_properties: Vec<AvifItemProperty>,
    pub(super) item_plane_properties: Vec<AvifItemPlaneProperties>,
    pub(super) item_codec_properties: Vec<AvifItemCodecProperties>,
    pub(super) grid_item_ids: Vec<u32>,
    pub(super) grid_properties: Option<AvifGridProperties>,
    pub(super) transform: Option<AvifTransformProperties>,
}

impl ExtractedAvif<'_> {
    pub(super) fn validate(&self) -> CodecResult<()> {
        if self.still.is_none() && self.sequence.is_none() {
            return Err(CodecError::Malformed(
                "AVIF container has neither a still image nor an image sequence".to_owned(),
            ));
        }
        if let Some(still) = &self.still {
            validate_plane(self.input, &still.color)?;
            if let Some(alpha) = &still.alpha {
                validate_plane(self.input, alpha)?;
                if alpha.samples.len() != still.color.samples.len() {
                    return Err(CodecError::Malformed(
                        "AVIF still color and alpha sample counts differ".to_owned(),
                    ));
                }
            }
        }
        if let Some(sequence) = &self.sequence {
            let _ = sequence.timescale;
            validate_plane(self.input, &sequence.color)?;
            if let Some(alpha) = &sequence.alpha {
                validate_plane(self.input, alpha)?;
                if alpha.samples.len() != sequence.color.samples.len() {
                    return Err(CodecError::Malformed(
                        "AVIF sequence color and alpha sample counts differ".to_owned(),
                    ));
                }
            }
        }
        Ok(())
    }
}

fn validate_plane(input: &[u8], plane: &EncodedPlane) -> CodecResult<()> {
    if plane.samples.is_empty() {
        return Err(CodecError::Malformed(
            "AVIF sample plane is empty".to_owned(),
        ));
    }
    for sample in &plane.samples {
        if sample.spans.is_empty() || sample.spans.iter().all(|span| span.len() == 0) {
            return Err(CodecError::Malformed(
                "AVIF sample has no encoded payload".to_owned(),
            ));
        }
        let _ = sample
            .config
            .bytes(input)
            .map_err(|error| error.context("validate AVIF sample configuration"))?;
        let _ = sample.sync;
        let _ = sample.duration;
        for span in &sample.spans {
            let _ = span
                .bytes(input)
                .map_err(|error| error.context("validate AVIF sample payload"))?;
        }
    }
    Ok(())
}

fn item_sample(meta: &Meta, item_id: u32) -> ParseResult<EncodedSample> {
    let item = meta.item(item_id).ok_or_else(|| parse_failure!())?;
    if item.kind != *b"av01" {
        return Err(parse_failure!());
    }
    let location = meta.location(item_id).ok_or_else(|| parse_failure!())?;
    let _ = location.source;
    Ok(EncodedSample {
        spans: location.extents.clone(),
        config: meta.av1c(item_id)?,
        sync: true,
        duration: 1,
    })
}

fn item_ids(meta: &Meta, item_id: u32) -> ParseResult<Vec<u32>> {
    let item = meta.item(item_id).ok_or_else(|| parse_failure!())?;
    match item.kind {
        kind if kind == *b"av01" => Ok(vec![item_id]),
        kind if kind == *b"grid" => {
            let children = meta.dimg_children(item_id);
            if children.is_empty() {
                return Err(parse_failure!());
            }
            Ok(children)
        }
        _ => Err(parse_failure!()),
    }
}

fn still_payload(meta: &Meta) -> ParseResult<StillPayload> {
    let primary = meta.primary_item_id;
    let color_ids = item_ids(meta, primary)?;
    let color = EncodedPlane {
        samples: color_ids
            .iter()
            .map(|&item_id| item_sample(meta, item_id))
            .collect::<ParseResult<Vec<_>>>()?,
    };

    let direct_alpha = meta.alpha_targeting(primary)?;
    let alpha_ids = if let Some(alpha) = direct_alpha {
        item_ids(meta, alpha)?
    } else {
        let mut ids = Vec::new();
        for &color_id in &color_ids {
            if let Some(alpha) = meta.alpha_targeting(color_id)? {
                ids.push(alpha);
            }
        }
        ids
    };
    let alpha = if alpha_ids.is_empty() {
        None
    } else {
        if alpha_ids.len() != color_ids.len() {
            return Err(parse_failure!());
        }
        Some(EncodedPlane {
            samples: alpha_ids
                .into_iter()
                .map(|item_id| item_sample(meta, item_id))
                .collect::<ParseResult<Vec<_>>>()?,
        })
    };
    Ok(StillPayload { color, alpha })
}

#[derive(Clone, Copy)]
struct SampleToChunk {
    first_chunk: u32,
    samples_per_chunk: u32,
    description_index: u32,
}

#[derive(Clone, Copy)]
struct TimeToSample {
    sample_count: u32,
    sample_delta: u32,
}

#[derive(Clone, Copy)]
struct SampleDescription {
    config: Option<ByteSpan>,
    aux_is_alpha: Option<bool>,
}

#[derive(Default)]
struct SampleTable {
    chunk_offsets: Vec<u64>,
    mappings: Vec<SampleToChunk>,
    sample_sizes: Vec<u32>,
    sync_samples: Vec<u32>,
    timings: Vec<TimeToSample>,
    descriptions: Vec<SampleDescription>,
}

#[derive(Default)]
struct Track {
    id: u32,
    handler: FourCc,
    aux_for_id: Option<u32>,
    timescale: Option<NonZeroU32>,
    table: Option<SampleTable>,
}

#[derive(Default)]
struct Movie {
    tracks: Vec<Track>,
}

fn parse_movie(input: &[u8], payload: ByteSpan, budget: &mut Budget) -> ParseResult<Movie> {
    let mut reader = Reader::new(input, payload);
    let mut movie = Movie::default();
    while let Some(child) = next_box(&mut reader, false, budget)? {
        if child.kind == *b"trak" {
            budget.records_seen(1)?;
            movie
                .tracks
                .push(parse_track(input, child.payload, budget)?);
        }
    }
    if movie.tracks.is_empty() {
        return Err(parse_failure!());
    }
    Ok(movie)
}

fn parse_track(input: &[u8], payload: ByteSpan, budget: &mut Budget) -> ParseResult<Track> {
    let mut reader = Reader::new(input, payload);
    let mut track = Track::default();
    let mut tkhd_seen = false;
    let mut mdia_seen = false;
    let mut tref_seen = false;
    while let Some(child) = next_box(&mut reader, false, budget)? {
        match child.kind {
            kind if kind == *b"tkhd" => {
                if tkhd_seen {
                    return Err(parse_failure!());
                }
                tkhd_seen = true;
                track.id = parse_tkhd(input, child.payload)?;
            }
            kind if kind == *b"mdia" => {
                if mdia_seen {
                    return Err(parse_failure!());
                }
                mdia_seen = true;
                parse_mdia(input, child.payload, &mut track, budget)?;
            }
            kind if kind == *b"tref" => {
                if tref_seen {
                    return Err(parse_failure!());
                }
                tref_seen = true;
                track.aux_for_id = parse_tref(input, child.payload, budget)?;
            }
            _ => {}
        }
    }
    if !tkhd_seen || !mdia_seen {
        return Err(parse_failure!());
    }
    Ok(track)
}

fn parse_tkhd(input: &[u8], payload: ByteSpan) -> ParseResult<u32> {
    let mut reader = Reader::new(input, payload);
    let (version, _) = parse_full_box(&mut reader)?;
    match version {
        0 => reader.skip(8)?,
        1 => reader.skip(16)?,
        _ => return Err(parse_failure!()),
    }
    let track_id = reader.u32()?;
    if track_id == 0 {
        return Err(parse_failure!());
    }
    Ok(track_id)
}

fn parse_tref(input: &[u8], payload: ByteSpan, budget: &mut Budget) -> ParseResult<Option<u32>> {
    let mut reader = Reader::new(input, payload);
    let mut aux_for = None;
    while let Some(child) = next_box(&mut reader, false, budget)? {
        if child.kind == *b"auxl" {
            if aux_for.is_some() {
                return Err(parse_failure!());
            }
            let mut ids = Reader::new(input, child.payload);
            let id = ids.u32()?;
            if id == 0 {
                return Err(parse_failure!());
            }
            aux_for = Some(id);
        }
    }
    Ok(aux_for)
}

fn parse_mdia(
    input: &[u8],
    payload: ByteSpan,
    track: &mut Track,
    budget: &mut Budget,
) -> ParseResult<()> {
    let mut reader = Reader::new(input, payload);
    let mut mdhd_seen = false;
    let mut handler_seen = false;
    let mut minf_seen = false;
    while let Some(child) = next_box(&mut reader, false, budget)? {
        match child.kind {
            kind if kind == *b"mdhd" => {
                if mdhd_seen {
                    return Err(parse_failure!());
                }
                mdhd_seen = true;
                track.timescale = Some(parse_mdhd(input, child.payload)?);
            }
            kind if kind == *b"hdlr" => {
                if handler_seen {
                    return Err(parse_failure!());
                }
                handler_seen = true;
                track.handler = parse_handler(input, child.payload)?;
            }
            kind if kind == *b"minf" => {
                if minf_seen {
                    return Err(parse_failure!());
                }
                minf_seen = true;
                track.table = Some(parse_minf(input, child.payload, budget)?);
            }
            _ => {}
        }
    }
    if !mdhd_seen || !handler_seen || !minf_seen {
        return Err(parse_failure!());
    }
    Ok(())
}

// ✅ VERIFIED: libavif 1.4.1 read.c:3566-3595.
fn parse_mdhd(input: &[u8], payload: ByteSpan) -> ParseResult<NonZeroU32> {
    let mut reader = Reader::new(input, payload);
    let (version, _) = parse_full_box(&mut reader)?;
    match version {
        0 => reader.skip(8)?,
        1 => reader.skip(16)?,
        _ => return Err(parse_failure!()),
    }
    NonZeroU32::new(reader.u32()?).ok_or_else(|| parse_failure!())
}

fn parse_minf(input: &[u8], payload: ByteSpan, budget: &mut Budget) -> ParseResult<SampleTable> {
    let mut reader = Reader::new(input, payload);
    let mut table = None;
    while let Some(child) = next_box(&mut reader, false, budget)? {
        if child.kind == *b"stbl" {
            if table.is_some() {
                return Err(parse_failure!());
            }
            table = Some(parse_stbl(input, child.payload, budget)?);
        }
    }
    table.ok_or_else(|| parse_failure!())
}

fn parse_stbl(input: &[u8], payload: ByteSpan, budget: &mut Budget) -> ParseResult<SampleTable> {
    let mut reader = Reader::new(input, payload);
    let mut table = SampleTable::default();
    let mut offsets_seen = false;
    let mut stsc_seen = false;
    let mut stsz_seen = false;
    let mut stss_seen = false;
    let mut stts_seen = false;
    let mut stsd_seen = false;
    while let Some(child) = next_box(&mut reader, false, budget)? {
        match child.kind {
            [b's', b't', b'c', b'o'] | [b'c', b'o', b'6', b'4'] => {
                if offsets_seen {
                    return Err(parse_failure!());
                }
                offsets_seen = true;
                table.chunk_offsets = parse_chunk_offsets(input, child, &mut table, budget)?;
            }
            kind if kind == *b"stsc" => {
                if stsc_seen {
                    return Err(parse_failure!());
                }
                stsc_seen = true;
                table.mappings = parse_stsc(input, child.payload, budget)?;
            }
            kind if kind == *b"stsz" => {
                if stsz_seen {
                    return Err(parse_failure!());
                }
                stsz_seen = true;
                table.sample_sizes = parse_stsz(input, child.payload, budget)?;
            }
            kind if kind == *b"stss" => {
                if stss_seen {
                    return Err(parse_failure!());
                }
                stss_seen = true;
                table.sync_samples = parse_u32_records(input, child.payload, budget)?;
            }
            kind if kind == *b"stts" => {
                if stts_seen {
                    return Err(parse_failure!());
                }
                stts_seen = true;
                table.timings = parse_stts(input, child.payload, budget)?;
            }
            kind if kind == *b"stsd" => {
                if stsd_seen {
                    return Err(parse_failure!());
                }
                stsd_seen = true;
                table.descriptions = parse_stsd(input, child.payload, budget)?;
            }
            _ => {}
        }
    }
    if !offsets_seen || !stsc_seen || !stsz_seen || !stsd_seen {
        return Err(parse_failure!());
    }
    Ok(table)
}

// ✅ VERIFIED: libavif 1.4.1 read.c:3597-3620.
fn parse_chunk_offsets(
    input: &[u8],
    child: BoxSpan,
    _table: &mut SampleTable,
    budget: &mut Budget,
) -> ParseResult<Vec<u64>> {
    let mut reader = Reader::new(input, child.payload);
    let (version, _) = parse_full_box(&mut reader)?;
    if version != 0 {
        return Err(parse_failure!());
    }
    let count = reader.u32()? as usize;
    budget.records_seen(count)?;
    let mut offsets = Vec::with_capacity(count);
    for _ in 0..count {
        offsets.push(if child.kind == *b"co64" {
            reader.u64()?
        } else {
            u64::from(reader.u32()?)
        });
    }
    if !reader.is_empty() {
        return Err(parse_failure!());
    }
    Ok(offsets)
}

// ✅ VERIFIED: libavif 1.4.1 read.c:3622-3653.
fn parse_stsc(
    input: &[u8],
    payload: ByteSpan,
    budget: &mut Budget,
) -> ParseResult<Vec<SampleToChunk>> {
    let mut reader = Reader::new(input, payload);
    let (version, _) = parse_full_box(&mut reader)?;
    if version != 0 {
        return Err(parse_failure!());
    }
    let count = reader.u32()? as usize;
    budget.records_seen(count)?;
    let mut entries = Vec::with_capacity(count);
    for index in 0..count {
        let entry = SampleToChunk {
            first_chunk: reader.u32()?,
            samples_per_chunk: reader.u32()?,
            description_index: reader.u32()?,
        };
        if (index == 0 && entry.first_chunk != 1)
            || entries
                .last()
                .is_some_and(|previous: &SampleToChunk| entry.first_chunk <= previous.first_chunk)
        {
            return Err(parse_failure!());
        }
        entries.push(entry);
    }
    if !reader.is_empty() || entries.is_empty() {
        return Err(parse_failure!());
    }
    Ok(entries)
}

// ✅ VERIFIED: libavif 1.4.1 read.c:3655-3675.
fn parse_stsz(input: &[u8], payload: ByteSpan, budget: &mut Budget) -> ParseResult<Vec<u32>> {
    let mut reader = Reader::new(input, payload);
    let (version, _) = parse_full_box(&mut reader)?;
    if version != 0 {
        return Err(parse_failure!());
    }
    let common_size = reader.u32()?;
    let count = reader.u32()? as usize;
    budget.records_seen(count)?;
    let mut sizes = Vec::with_capacity(count);
    if common_size == 0 {
        for _ in 0..count {
            sizes.push(reader.u32()?);
        }
    } else {
        sizes.resize(count, common_size);
    }
    if !reader.is_empty() || sizes.is_empty() || sizes.contains(&0) {
        return Err(parse_failure!());
    }
    Ok(sizes)
}

fn parse_u32_records(
    input: &[u8],
    payload: ByteSpan,
    budget: &mut Budget,
) -> ParseResult<Vec<u32>> {
    let mut reader = Reader::new(input, payload);
    let (version, _) = parse_full_box(&mut reader)?;
    if version != 0 {
        return Err(parse_failure!());
    }
    let count = reader.u32()? as usize;
    budget.records_seen(count)?;
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        values.push(reader.u32()?);
    }
    if !reader.is_empty() {
        return Err(parse_failure!());
    }
    Ok(values)
}

// ✅ VERIFIED: libavif 1.4.1 read.c:3696-3712.
fn parse_stts(
    input: &[u8],
    payload: ByteSpan,
    budget: &mut Budget,
) -> ParseResult<Vec<TimeToSample>> {
    let mut reader = Reader::new(input, payload);
    let (version, _) = parse_full_box(&mut reader)?;
    if version != 0 {
        return Err(parse_failure!());
    }
    let count = reader.u32()? as usize;
    budget.records_seen(count)?;
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        entries.push(TimeToSample {
            sample_count: reader.u32()?,
            sample_delta: reader.u32()?,
        });
    }
    if !reader.is_empty() {
        return Err(parse_failure!());
    }
    Ok(entries)
}

fn parse_stsd(
    input: &[u8],
    payload: ByteSpan,
    budget: &mut Budget,
) -> ParseResult<Vec<SampleDescription>> {
    let mut reader = Reader::new(input, payload);
    let (version, _) = parse_full_box(&mut reader)?;
    if !matches!(version, 0 | 1) {
        return Err(parse_failure!());
    }
    let count = reader.u32()? as usize;
    budget.records_seen(count)?;
    let mut descriptions = Vec::with_capacity(count);
    for _ in 0..count {
        let sample = next_box(&mut reader, false, budget)?.ok_or_else(|| parse_failure!())?;
        if sample.kind != *b"av01" {
            descriptions.push(SampleDescription {
                config: None,
                aux_is_alpha: None,
            });
            continue;
        }
        if sample.payload.len() < VISUAL_SAMPLE_ENTRY_SIZE {
            return Err(parse_failure!());
        }
        let properties = ByteSpan {
            start: sample
                .payload
                .start
                .saturating_add(VISUAL_SAMPLE_ENTRY_SIZE),
            end: sample.payload.end,
        };
        descriptions.push(parse_sample_description(input, properties, budget)?);
    }
    if !reader.is_empty() {
        return Err(parse_failure!());
    }
    Ok(descriptions)
}

fn parse_sample_description(
    input: &[u8],
    payload: ByteSpan,
    budget: &mut Budget,
) -> ParseResult<SampleDescription> {
    let mut reader = Reader::new(input, payload);
    let mut config = None;
    let mut aux_is_alpha = None;
    while let Some(child) = next_box(&mut reader, false, budget)? {
        match parse_property(input, child)? {
            Property::Ispe { .. } | Property::Pixi { .. } => {}
            Property::Av1C(span) => {
                if config.replace(span).is_some() {
                    return Err(parse_failure!());
                }
            }
            Property::AuxC { is_alpha, .. } => {
                if aux_is_alpha.replace(is_alpha).is_some() {
                    return Err(parse_failure!());
                }
            }
            Property::Color(_)
            | Property::IccProfile(_)
            | Property::ContentLightLevel { .. }
            | Property::MasteringDisplayColorVolume { .. }
            | Property::Rotation { .. }
            | Property::Mirror { .. }
            | Property::PixelAspectRatio { .. }
            | Property::CleanAperture { .. } => {}
            Property::Other { .. } => {}
        }
    }
    Ok(SampleDescription {
        config,
        aux_is_alpha,
    })
}

fn duration_at(timings: &[TimeToSample], sample_index: usize) -> u32 {
    let Some(last) = timings.last() else {
        return 1;
    };
    let mut maximum = 0_u64;
    for timing in timings {
        maximum = maximum.saturating_add(u64::from(timing.sample_count));
        if (sample_index as u64) < maximum {
            return timing.sample_delta;
        }
    }
    last.sample_delta
}

// ✅ VERIFIED: libavif 1.4.1 read.c:520-607. Chunk mappings expand in
// declaration order, and the first sample is sync even without stss.
fn track_plane(input: &[u8], track: &Track) -> ParseResult<EncodedPlane> {
    let table = track.table.as_ref().ok_or_else(|| parse_failure!())?;
    let mut samples = Vec::with_capacity(table.sample_sizes.len());
    let mut sample_index = 0_usize;
    let mut mapping_index = 0_usize;
    for (chunk_index, &chunk_offset) in table.chunk_offsets.iter().enumerate() {
        #[allow(clippy::cast_possible_truncation)]
        let chunk_number = (chunk_index as u32).saturating_add(1);
        while let Some(next) = table.mappings.get(mapping_index.saturating_add(1)) {
            if next.first_chunk > chunk_number {
                break;
            }
            mapping_index = mapping_index.saturating_add(1);
        }
        let mapping = table
            .mappings
            .get(mapping_index)
            .ok_or_else(|| parse_failure!())?;
        if mapping.first_chunk > chunk_number || mapping.samples_per_chunk == 0 {
            return Err(parse_failure!());
        }
        let description_index = mapping.description_index.saturating_sub(1) as usize;
        if mapping.description_index == 0 {
            return Err(parse_failure!());
        }
        let description = table
            .descriptions
            .get(description_index)
            .ok_or_else(|| parse_failure!())?;
        let config = description.config.ok_or_else(|| parse_failure!())?;
        let mut sample_offset = chunk_offset;
        for _ in 0..mapping.samples_per_chunk {
            let size = *table
                .sample_sizes
                .get(sample_index)
                .ok_or_else(|| parse_failure!())?;
            let span =
                ByteSpan::from_offset_size(sample_offset, u64::from(size), input.len(), true)?;
            #[allow(clippy::cast_possible_truncation)]
            let sample_number = (sample_index as u32).saturating_add(1);
            samples.push(EncodedSample {
                spans: vec![span],
                config,
                sync: sample_index == 0 || table.sync_samples.contains(&sample_number),
                duration: duration_at(&table.timings, sample_index),
            });
            sample_offset = sample_offset.saturating_add(u64::from(size));
            sample_index = sample_index.saturating_add(1);
        }
    }
    if sample_index != table.sample_sizes.len() || samples.is_empty() {
        return Err(parse_failure!());
    }
    Ok(EncodedPlane { samples })
}

fn sequence_payload(movie: &Movie, input: &[u8]) -> ParseResult<SequencePayload> {
    let color_track = movie
        .tracks
        .iter()
        .find(|track| {
            matches!(
                track.handler,
                [b'p', b'i', b'c', b't'] | [b'v', b'i', b'd', b'e']
            )
        })
        .ok_or_else(|| parse_failure!())?;
    let timescale = color_track.timescale.ok_or_else(|| parse_failure!())?;
    let color = track_plane(input, color_track)?;
    let mut alpha_tracks = movie.tracks.iter().filter(|track| {
        track.handler == *b"auxv"
            && track.aux_for_id == Some(color_track.id)
            && track
                .table
                .as_ref()
                .and_then(|table| {
                    table
                        .descriptions
                        .iter()
                        .find_map(|entry| entry.aux_is_alpha)
                })
                .unwrap_or(true)
    });
    let alpha_track = alpha_tracks.next();
    if alpha_tracks.next().is_some() {
        return Err(parse_failure!());
    }
    let alpha = if let Some(track) = alpha_track {
        if track.timescale != Some(timescale) {
            return Err(parse_failure!());
        }
        let plane = track_plane(input, track)?;
        if plane.samples.len() != color.samples.len() {
            return Err(parse_failure!());
        }
        Some(plane)
    } else {
        None
    };
    Ok(SequencePayload {
        color,
        alpha,
        timescale,
    })
}

fn extract_inner(input: &[u8]) -> ParseResult<ExtractedAvif<'_>> {
    extract_inner_with_metadata(input, true)
}

fn extract_inner_with_metadata(
    input: &[u8],
    retain_metadata: bool,
) -> ParseResult<ExtractedAvif<'_>> {
    let mut budget = Budget::default();
    let mut reader = Reader::whole(input);
    let first = next_box(&mut reader, true, &mut budget)
        .map_err(|error| error.at(0, "avif_box"))?
        .ok_or_else(|| parse_failure!())?;
    if first.kind != *b"ftyp" {
        return Err(parse_failure!());
    }
    let brands = parse_ftyp(input, first.payload).map_err(|error| error.at(0, "avif_box"))?;
    let mut meta = None;
    let mut movie = None;
    let mut retained_boxes = Vec::new();
    // Reassigned at the top of every iteration; the initializer only satisfies
    // definite initialization.
    #[allow(unused_assignments)]
    let mut consumed = 0;
    loop {
        // Extent of the last successfully parsed top-level box.
        consumed = reader.offset;
        let box_offset = reader.offset as u64;
        let box_start = reader.offset;
        match next_box(&mut reader, true, &mut budget)
            .map_err(|error| error.at(box_offset, "avif_box"))
        {
            Ok(Some(child)) => {
                let box_end = reader.offset;
                match child.kind {
                    kind if kind == *b"meta" => {
                        if meta.is_some() {
                            return Err(parse_failure!());
                        }
                        meta = Some(
                            parse_meta(input, child.payload, &mut budget)
                                .map_err(|error| error.at(box_offset, "avif_box"))?,
                        );
                    }
                    kind if kind == *b"moov" => {
                        if movie.is_some() {
                            return Err(parse_failure!());
                        }
                        movie = Some(
                            parse_movie(input, child.payload, &mut budget)
                                .map_err(|error| error.at(box_offset, "avif_box"))?,
                        );
                    }
                    kind if kind == *b"mdat" => {}
                    kind if kind == *b"free" || kind == *b"skip" => {
                        retained_boxes.push(crate::types::OpaqueBlock {
                            kind: kind.to_vec(),
                            data: input[box_start..box_end].to_vec(),
                            safe_to_copy: true,
                        });
                    }
                    _ => {
                        // Unknown top-level boxes are ignorable by decoders
                        // and retained raw; BMFF defines no safe-to-copy bit.
                        retained_boxes.push(crate::types::OpaqueBlock {
                            kind: child.kind.to_vec(),
                            data: input[box_start..box_end].to_vec(),
                            safe_to_copy: true,
                        });
                    }
                }
            }
            Ok(None) => break,
            Err(error) => {
                // Bytes after a complete still or sequence structure are
                // trailing input and are ignored, matching Pillow/libavif.
                let complete =
                    (brands.has_avif && meta.is_some()) || (brands.has_avis && movie.is_some());
                if !complete {
                    return Err(error);
                }
                break;
            }
        }
    }
    if (brands.has_avif && meta.is_none()) || (brands.has_avis && movie.is_none()) {
        return Err(parse_need_more!(reader.offset.saturating_add(8)));
    }
    let still = meta.as_ref().map(still_payload).transpose()?;
    let sequence = movie
        .as_ref()
        .map(|movie| sequence_payload(movie, input))
        .transpose()?;
    let source_color = meta
        .as_ref()
        .map(|meta| meta.source_color(input))
        .transpose()?
        .unwrap_or_default();
    let metadata = if retain_metadata {
        meta.as_ref()
            .map(|meta| meta.metadata(input))
            .transpose()?
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let transform = meta.as_ref().map(Meta::transform).transpose()?.flatten();
    let primary_item_id = meta.as_ref().map(|meta| meta.primary_item_id);
    let auxiliary_relationships = meta
        .as_ref()
        .map(|meta| meta.alpha_auxiliary_relationships(meta.primary_item_id))
        .transpose()?
        .unwrap_or_default();
    let auxiliary_relationship = primary_item_id.and_then(|primary_item_id| {
        auxiliary_relationships
            .iter()
            .find(|relationship| relationship.target_item_id() == primary_item_id)
            .copied()
    });
    let grid_item_ids = meta
        .as_ref()
        .map(|meta| meta.grid_item_ids(meta.primary_item_id))
        .transpose()?
        .unwrap_or_default();
    let grid_properties = meta
        .as_ref()
        .map(|meta| meta.grid_properties(input, meta.primary_item_id))
        .transpose()?
        .flatten();
    let item_relationships = meta
        .as_ref()
        .map(Meta::non_alpha_item_relationships)
        .unwrap_or_default();
    let premultiplied_relationships = meta
        .as_ref()
        .map(Meta::premultiplied_relationships)
        .unwrap_or_default();
    let item_color_properties = meta
        .as_ref()
        .map(|meta| meta.non_primary_item_color_properties(meta.primary_item_id))
        .unwrap_or_default();
    let item_icc_profiles = meta
        .as_ref()
        .map(|meta| meta.non_primary_item_icc_profiles(meta.primary_item_id))
        .unwrap_or_default();
    let item_properties = meta
        .as_ref()
        .map(|meta| meta.non_primary_item_properties(input, meta.primary_item_id))
        .transpose()?
        .unwrap_or_default();
    let item_plane_properties = meta
        .as_ref()
        .map(|meta| meta.non_primary_item_plane_properties(meta.primary_item_id))
        .transpose()?
        .unwrap_or_default();
    let item_codec_properties = meta
        .as_ref()
        .map(|meta| meta.non_primary_item_codec_properties(input, meta.primary_item_id))
        .transpose()?
        .unwrap_or_default();
    let _ = brands.major;
    Ok(ExtractedAvif {
        input,
        still,
        sequence,
        consumed,
        retained_boxes,
        metadata,
        source_color,
        auxiliary_relationship,
        auxiliary_relationships,
        item_relationships,
        premultiplied_relationships,
        item_color_properties,
        item_icc_profiles,
        item_properties,
        item_plane_properties,
        item_codec_properties,
        grid_item_ids,
        grid_properties,
        transform,
    })
}

pub(super) fn extract(input: &[u8]) -> CodecResult<ExtractedAvif<'_>> {
    extract_inner(input)
}

pub(super) fn validated(input: &[u8]) -> CodecResult<ExtractedAvif<'_>> {
    let extracted = extract(input)?;
    extracted.validate()?;
    Ok(extracted)
}

fn pixel_payload_bytes(extracted: &ExtractedAvif<'_>) -> u64 {
    let mut spans = Vec::new();
    let mut add_plane = |plane: &EncodedPlane| {
        for sample in &plane.samples {
            spans.extend(sample.spans.iter().map(|span| (span.start, span.end)));
        }
    };
    if let Some(still) = &extracted.still {
        add_plane(&still.color);
        if let Some(alpha) = &still.alpha {
            add_plane(alpha);
        }
    }
    if let Some(sequence) = &extracted.sequence {
        add_plane(&sequence.color);
        if let Some(alpha) = &sequence.alpha {
            add_plane(alpha);
        }
    }
    spans.sort_unstable_by_key(|(start, _)| *start);
    let mut total = 0_u64;
    let mut current = None;
    for (start, end) in spans {
        if start >= end {
            continue;
        }
        match current {
            Some((current_start, current_end)) if start <= current_end => {
                current = Some((current_start, current_end.max(end)));
            }
            Some((current_start, current_end)) => {
                // Every active span starts below its end, and sorted disjoint
                // spans remain within the `usize` address space, so neither
                // arithmetic operation can overflow on supported targets.
                #[allow(clippy::arithmetic_side_effects)]
                let length = (current_end - current_start) as u64;
                total = total.saturating_add(length);
                current = Some((start, end));
            }
            None => current = Some((start, end)),
        }
    }
    if let Some((current_start, current_end)) = current {
        #[allow(clippy::arithmetic_side_effects)]
        let length = (current_end - current_start) as u64;
        total = total.saturating_add(length);
    }
    total
}

/// Measure the encoded metadata extent: the parsed top-level BMFF bytes minus
/// the referenced primary and auxiliary pixel-sample payload spans.
pub(super) fn metadata_bytes(data: &[u8]) -> CodecResult<u64> {
    let extracted = extract_inner_with_metadata(data, false)?;
    #[allow(clippy::cast_possible_truncation)]
    let consumed = extracted.consumed as u64;
    let pixel = pixel_payload_bytes(&extracted);
    // Every referenced sample span belongs to a successfully parsed top-level
    // extent, so the pixel union cannot exceed `consumed`.
    Ok(consumed.saturating_sub(pixel))
}

#[cfg(coverage)]
fn coverage_box(kind: FourCc, payload: &[u8]) -> Vec<u8> {
    let size = payload.len().saturating_add(8);
    let size_bytes = size.to_be_bytes();
    let mut result = Vec::with_capacity(size);
    result.extend_from_slice(&size_bytes[size_bytes.len().saturating_sub(4)..]);
    result.extend_from_slice(&kind);
    result.extend_from_slice(payload);
    result
}

#[cfg(coverage)]
fn coverage_full_box(version: u8, flags: u32, payload: &[u8]) -> Vec<u8> {
    let flags = flags.to_be_bytes();
    let mut result = Vec::with_capacity(payload.len().saturating_add(4));
    result.extend_from_slice(&[version, flags[1], flags[2], flags[3]]);
    result.extend_from_slice(payload);
    result
}

#[cfg(coverage)]
fn coverage_join(parts: &[&[u8]]) -> Vec<u8> {
    let capacity = parts
        .iter()
        .fold(0_usize, |total, part| total.saturating_add(part.len()));
    let mut result = Vec::with_capacity(capacity);
    for part in parts {
        result.extend_from_slice(part);
    }
    result
}

#[cfg(coverage)]
fn coverage_assert_sample(
    input: &[u8],
    sample: &EncodedSample,
    expected_spans: &[(usize, usize)],
    expected_config: &[u8],
    expected_sync: bool,
    expected_duration: u32,
) {
    let actual = sample
        .spans
        .iter()
        .map(|span| (span.start, span.len()))
        .collect::<Vec<_>>();
    assert_eq!(actual, expected_spans);
    assert_eq!(sample.config.bytes(input), Ok(expected_config));
    assert_eq!(sample.sync, expected_sync);
    assert_eq!(sample.duration, expected_duration);
}

#[cfg(coverage)]
fn coverage_fixture_contracts() {
    let baseline = include_bytes!("../../../tests/fixtures/input/images/avif/baseline.avif");
    let baseline_payload = extract_inner(baseline).unwrap();
    let baseline_still = baseline_payload.still.as_ref().unwrap();
    coverage_assert_sample(
        baseline,
        &baseline_still.color.samples[0],
        &[(282, 2_795)],
        &[0x81, 0x00, 0x0c, 0x00],
        true,
        1,
    );
    assert!(baseline_still.alpha.is_none());

    // A complete avis sequence followed by unparseable bytes exercises the
    // has_avis trailing-tolerance branch of the top-level box loop.
    let animated = include_bytes!("../../../tests/fixtures/input/images/avif/animated.avif");
    let mut animated_trailing = animated.to_vec();
    animated_trailing.extend_from_slice(b"garbage");
    let animated_payload =
        extract_inner(&animated_trailing).expect("trailing bytes after a sequence are ignored");
    assert!(animated_payload.sequence.is_some());

    // An avis-only ftyp exercises the has_avif=false side of the trailing
    // tolerance decision; rebuild the compatible-brand list without `avif`.
    let animated_size =
        u32::from_be_bytes([animated[0], animated[1], animated[2], animated[3]]) as usize;
    let mut avis_only = Vec::new();
    avis_only.extend_from_slice(&40u32.to_be_bytes());
    avis_only.extend_from_slice(b"ftyp");
    avis_only.extend_from_slice(b"avis");
    avis_only.extend_from_slice(&[0, 0, 0, 0]);
    for brand in [b"avis", b"msf1", b"iso8", b"mif1", b"miaf", b"MA1B"] {
        avis_only.extend_from_slice(brand);
    }
    avis_only.extend_from_slice(&animated[animated_size..]);
    avis_only.extend_from_slice(b"garbage");
    let avis_payload = extract_inner(&avis_only).expect("avis-only trailing bytes are ignored");
    assert!(avis_payload.sequence.is_some());

    // Metadata-measurement error branches.
    let _ = metadata_bytes(b"");
    let _ = metadata_bytes(b"not an AVIF container");
    let non_ftyp = coverage_join(&[&coverage_box(*b"meta", b"garbage")]);
    let _ = metadata_bytes(&non_ftyp);
    let ftyp_only = coverage_join(&[&coverage_box(*b"ftyp", b"avif\0\0\0\0")]);
    let _ = metadata_bytes(&ftyp_only);
    let short_ftyp = coverage_join(&[&coverage_box(*b"ftyp", &b"avif"[..])]);
    let _ = metadata_bytes(&short_ftyp);
    let meta_garbage = coverage_join(&[
        &coverage_box(*b"ftyp", b"avif\0\0\0\0avif"),
        &coverage_box(*b"meta", b"garbage"),
    ]);
    let _ = metadata_bytes(&meta_garbage);
    let moov_garbage = coverage_join(&[
        &coverage_box(*b"ftyp", b"avis\0\0\0\0avis"),
        &coverage_box(*b"moov", b"garbage"),
    ]);
    let _ = metadata_bytes(&moov_garbage);
    let _ = metadata_bytes(&baseline[..100]);
    let mut trailing_metadata = baseline.to_vec();
    trailing_metadata.extend_from_slice(b"garbage");
    let _ = metadata_bytes(&trailing_metadata);
    let _ = metadata_bytes(&avis_only);
    let avis_ftyp = coverage_join(&[&coverage_box(*b"ftyp", b"avis\0\0\0\0avis")]);
    let _ = metadata_bytes(&avis_ftyp);
    fn append_top_level_box(mut file: Vec<u8>, kind: &[u8; 4]) -> Vec<u8> {
        let mut position = 0usize;
        while position.wrapping_add(8) <= file.len() {
            let size = u32::from_be_bytes([
                file[position],
                file[position + 1],
                file[position + 2],
                file[position + 3],
            ]) as usize;
            if size < 8 || position + size > file.len() {
                break;
            }
            if &file[position + 4..position + 8] == kind {
                let copied = file[position..position + size].to_vec();
                file.extend_from_slice(&copied);
                break;
            }
            position = position + size;
        }
        file
    }
    let duplicate_meta = append_top_level_box(baseline.to_vec(), b"meta");
    let _ = metadata_bytes(&duplicate_meta);
    let duplicate_moov = append_top_level_box(animated.to_vec(), b"moov");
    let _ = metadata_bytes(&duplicate_moov);
    let _ = append_top_level_box(vec![0u8; 8], b"meta");
    let _ = append_top_level_box(vec![9, 0, 0, 0, b'm', b'e', b't', b'a'], b"meta");
    let _ = append_top_level_box(baseline.to_vec(), b"XXXX");

    let alpha = include_bytes!("../../../tests/fixtures/input/images/avif/alpha.avif");
    let alpha_payload = extract_inner(alpha).unwrap();
    let alpha_still = alpha_payload.still.as_ref().unwrap();
    coverage_assert_sample(
        alpha,
        &alpha_still.color.samples[0],
        &[(727, 5_714)],
        &[0x81, 0x20, 0x00, 0x00],
        true,
        1,
    );
    coverage_assert_sample(
        alpha,
        &alpha_still.alpha.as_ref().unwrap().samples[0],
        &[(457, 270)],
        &[0x81, 0x00, 0x1c, 0x00],
        true,
        1,
    );

    let grid = include_bytes!("../../../tests/fixtures/input/images/avif/grid.avif");
    let grid_payload = extract_inner(grid).unwrap();
    let grid_still = grid_payload.still.as_ref().unwrap();
    for (sample, expected) in grid_still
        .color
        .samples
        .iter()
        .zip([[(1_467, 781)].as_slice(), [(2_248, 125)].as_slice()])
    {
        coverage_assert_sample(grid, sample, expected, &[0x81, 0x20, 0x00, 0x00], true, 1);
    }
    for (sample, expected) in grid_still
        .alpha
        .as_ref()
        .unwrap()
        .samples
        .iter()
        .zip([[(635, 589)].as_slice(), [(1_224, 243)].as_slice()])
    {
        coverage_assert_sample(grid, sample, expected, &[0x81, 0x00, 0x1c, 0x00], true, 1);
    }

    let hdr = include_bytes!("../../../tests/fixtures/input/images/avif/hdr.avif");
    let hdr_payload = extract_inner(hdr).unwrap();
    coverage_assert_sample(
        hdr,
        &hdr_payload.still.as_ref().unwrap().color.samples[0],
        &[(687, 5_378)],
        &[0x81, 0x20, 0x40, 0x00],
        true,
        1,
    );

    let animated = include_bytes!("../../../tests/fixtures/input/images/avif/animated.avif");
    let animated_payload = extract_inner(animated).unwrap();
    coverage_assert_sample(
        animated,
        &animated_payload.still.as_ref().unwrap().color.samples[0],
        &[(1_023, 39)],
        &[0x81, 0x00, 0x0c, 0x00],
        true,
        1,
    );
    let animated_sequence = animated_payload.sequence.as_ref().unwrap();
    assert_eq!(animated_sequence.timescale.get(), 30);
    for (sample, (start, length, sync)) in animated_sequence.color.samples.iter().zip([
        (1_023, 39, true),
        (1_062, 113, false),
        (1_175, 5, false),
        (1_180, 30, false),
        (1_210, 25, false),
    ]) {
        coverage_assert_sample(
            animated,
            sample,
            &[(start, length)],
            &[0x81, 0x00, 0x0c, 0x00],
            sync,
            1,
        );
    }
    assert!(animated_sequence.alpha.is_none());

    let high_bit = include_bytes!("../../../tests/fixtures/input/images/avif/10bit.avif");
    let high_bit_payload = extract_inner(high_bit).unwrap();
    let high_bit_still = high_bit_payload.still.as_ref().unwrap();
    coverage_assert_sample(
        high_bit,
        &high_bit_still.color.samples[0],
        &[(2_022, 39)],
        &[0x81, 0x40, 0x68, 0x00],
        true,
        1,
    );
    coverage_assert_sample(
        high_bit,
        &high_bit_still.alpha.as_ref().unwrap().samples[0],
        &[(1_852, 29)],
        &[0x81, 0x40, 0x7c, 0x00],
        true,
        1,
    );
    let high_bit_sequence = high_bit_payload.sequence.as_ref().unwrap();
    assert_eq!(high_bit_sequence.timescale.get(), 1);
    for (sample, (start, length, sync)) in high_bit_sequence.color.samples.iter().zip([
        (2_022, 39, true),
        (2_061, 36, false),
        (2_097, 38, true),
        (2_135, 103, true),
        (2_238, 29, false),
    ]) {
        coverage_assert_sample(
            high_bit,
            sample,
            &[(start, length)],
            &[0x81, 0x40, 0x68, 0x00],
            sync,
            1,
        );
    }
    for (sample, (start, length, sync)) in high_bit_sequence
        .alpha
        .as_ref()
        .unwrap()
        .samples
        .iter()
        .zip([
            (1_852, 29, true),
            (1_881, 20, false),
            (1_901, 26, true),
            (1_927, 44, true),
            (1_971, 51, true),
        ])
    {
        coverage_assert_sample(
            high_bit,
            sample,
            &[(start, length)],
            &[0x81, 0x40, 0x7c, 0x00],
            sync,
            1,
        );
    }
}

#[cfg(coverage)]
fn coverage_prefixes(input: &[u8]) {
    for end in 0..=input.len() {
        let _ = extract_inner(&input[..end]);
    }
}

#[cfg(coverage)]
fn coverage_metadata_mutations(input: &[u8], metadata_end: usize) {
    let end = metadata_end.min(input.len());
    for index in 0..end {
        for replacement in [0, 1, 0x7f, 0xff] {
            if input[index] == replacement {
                continue;
            }
            let mut mutated = input.to_vec();
            mutated[index] = replacement;
            let _ = extract_inner(&mutated);
        }
    }
}

#[cfg(coverage)]
fn coverage_leaf_corpus() {
    for length in 0..=128 {
        for fill in [0, 0x55, 0xff] {
            let input = vec![fill; length];
            let span = ByteSpan {
                start: 0,
                end: input.len(),
            };
            let _ = parse_ftyp(&input, span);
            let _ = parse_handler(&input, span);
            let _ = parse_pitm(&input, span);
            let _ = parse_iinf(&input, span, &mut Meta::default(), &mut Budget::default());
            let _ = parse_infe(&input, span);
            let _ = parse_iprp(&input, span, &mut Meta::default(), &mut Budget::default());
            let _ = parse_ipco(&input, span, &mut Meta::default(), &mut Budget::default());
            for kind in [
                *b"av1C", *b"auxC", *b"colr", *b"clli", *b"irot", *b"imir", *b"pasp", *b"clap",
                *b"free",
            ] {
                let _ = parse_property(
                    &input,
                    BoxSpan {
                        kind,
                        payload: span,
                    },
                );
            }
            let _ = parse_ipma(&input, span, &mut Meta::default(), &mut Budget::default());
            let _ = parse_iref(&input, span, &mut Meta::default(), &mut Budget::default());
            let _ = parse_iloc(
                &input,
                span,
                None,
                &mut Meta::default(),
                &mut Budget::default(),
            );
            let _ = parse_movie(&input, span, &mut Budget::default());
            let _ = parse_track(&input, span, &mut Budget::default());
            let _ = parse_tkhd(&input, span);
            let _ = parse_tref(&input, span, &mut Budget::default());
            let _ = parse_mdhd(&input, span);
            let _ = parse_minf(&input, span, &mut Budget::default());
            let _ = parse_stbl(&input, span, &mut Budget::default());
            let _ = parse_stsc(&input, span, &mut Budget::default());
            let _ = parse_stsz(&input, span, &mut Budget::default());
            let _ = parse_u32_records(&input, span, &mut Budget::default());
            let _ = parse_stts(&input, span, &mut Budget::default());
            let _ = parse_stsd(&input, span, &mut Budget::default());
            let _ = parse_sample_description(&input, span, &mut Budget::default());
        }
    }
}

#[cfg(coverage)]
fn coverage_track(
    id: u32,
    handler: FourCc,
    aux_for_id: Option<u32>,
    sample_count: usize,
    timescale: u32,
    aux_is_alpha: Option<bool>,
) -> Track {
    Track {
        id,
        handler,
        aux_for_id,
        timescale: NonZeroU32::new(timescale),
        table: Some(SampleTable {
            chunk_offsets: vec![0],
            mappings: vec![SampleToChunk {
                first_chunk: 1,
                samples_per_chunk: u32::try_from(sample_count).unwrap(),
                description_index: 1,
            }],
            sample_sizes: vec![1; sample_count],
            sync_samples: Vec::new(),
            timings: vec![TimeToSample {
                sample_count: u32::try_from(sample_count).unwrap(),
                sample_delta: 1,
            }],
            descriptions: vec![SampleDescription {
                config: Some(ByteSpan { start: 0, end: 1 }),
                aux_is_alpha,
            }],
        }),
    }
}

#[cfg(coverage)]
fn coverage_parser_truncations() {
    let baseline = include_bytes!("../../../tests/fixtures/input/images/avif/baseline.avif");
    for end in 8..=32 {
        let _ = parse_ftyp(baseline, ByteSpan { start: 8, end });
    }
    for end in 40..=274 {
        let _ = parse_meta(
            baseline,
            ByteSpan { start: 40, end },
            &mut Budget::default(),
        );
    }
    for end in 52..=84 {
        let _ = parse_handler(baseline, ByteSpan { start: 52, end });
    }
    for end in 92..=98 {
        let _ = parse_pitm(baseline, ByteSpan { start: 92, end });
    }
    for end in 136..=168 {
        let _ = parse_iinf(
            baseline,
            ByteSpan { start: 136, end },
            &mut Meta::default(),
            &mut Budget::default(),
        );
    }
    for end in 150..=168 {
        let _ = parse_infe(baseline, ByteSpan { start: 150, end });
    }
    for end in 176..=274 {
        let _ = parse_iprp(
            baseline,
            ByteSpan { start: 176, end },
            &mut Meta::default(),
            &mut Budget::default(),
        );
    }
    for end in 184..=251 {
        let _ = parse_ipco(
            baseline,
            ByteSpan { start: 184, end },
            &mut Meta::default(),
            &mut Budget::default(),
        );
    }
    for end in 228..=232 {
        let _ = parse_property(
            baseline,
            BoxSpan {
                kind: *b"av1C",
                payload: ByteSpan { start: 228, end },
            },
        );
    }
    for end in 259..=274 {
        let _ = parse_ipma(
            baseline,
            ByteSpan { start: 259, end },
            &mut Meta::default(),
            &mut Budget::default(),
        );
    }
    for end in 106..=128 {
        let _ = parse_iloc(
            baseline,
            ByteSpan { start: 106, end },
            None,
            &mut Meta::default(),
            &mut Budget::default(),
        );
    }

    let animated = include_bytes!("../../../tests/fixtures/input/images/avif/animated.avif");
    for end in 294..=1015 {
        let _ = parse_movie(
            animated,
            ByteSpan { start: 294, end },
            &mut Budget::default(),
        );
    }
    for end in 422..=1015 {
        let _ = parse_track(
            animated,
            ByteSpan { start: 422, end },
            &mut Budget::default(),
        );
    }
    for end in 430..=526 {
        let _ = parse_tkhd(animated, ByteSpan { start: 430, end });
    }
    for end in 578..=1015 {
        let _ = parse_mdia(
            animated,
            ByteSpan { start: 578, end },
            &mut Track::default(),
            &mut Budget::default(),
        );
    }
    for end in 586..=622 {
        let _ = parse_mdhd(animated, ByteSpan { start: 586, end });
    }
    for end in 630..=662 {
        let _ = parse_handler(animated, ByteSpan { start: 630, end });
    }
    for end in 670..=1015 {
        let _ = parse_minf(
            animated,
            ByteSpan { start: 670, end },
            &mut Budget::default(),
        );
    }
    for end in 734..=1015 {
        let _ = parse_stbl(
            animated,
            ByteSpan { start: 734, end },
            &mut Budget::default(),
        );
    }
    for end in 742..=883 {
        let _ = parse_stsd(
            animated,
            ByteSpan { start: 742, end },
            &mut Budget::default(),
        );
    }
    for end in 891..=907 {
        let _ = parse_stts(
            animated,
            ByteSpan { start: 891, end },
            &mut Budget::default(),
        );
    }
    for end in 915..=935 {
        let _ = parse_stsc(
            animated,
            ByteSpan { start: 915, end },
            &mut Budget::default(),
        );
    }
    for end in 943..=975 {
        let _ = parse_stsz(
            animated,
            ByteSpan { start: 943, end },
            &mut Budget::default(),
        );
    }
    for end in 983..=995 {
        let _ = parse_chunk_offsets(
            animated,
            BoxSpan {
                kind: *b"stco",
                payload: ByteSpan { start: 983, end },
            },
            &mut SampleTable::default(),
            &mut Budget::default(),
        );
    }
    for end in 1003..=1015 {
        let _ = parse_u32_records(
            animated,
            ByteSpan { start: 1003, end },
            &mut Budget::default(),
        );
    }
    for end in 836..=883 {
        let _ = parse_sample_description(
            animated,
            ByteSpan { start: 836, end },
            &mut Budget::default(),
        );
    }
}

#[cfg(coverage)]
fn coverage_structural_states() {
    let _ = ByteSpan::from_offset_size(u64::MAX, 1, usize::MAX, true);
    let _ = ByteSpan::from_offset_size(0, u64::MAX, usize::MAX, true);
    let _ = ByteSpan::from_offset_size(1, 1, 1, false);
    let mut invalid_width = Reader::whole(&[]);
    let _ = invalid_width.uint(1);
    let mut wide_reader = Reader::whole(&[0; 8]);
    let _ = wide_reader.uint(8);
    let mut invalid_backing = Reader {
        input: &[],
        offset: 0,
        end: 8,
        truncation: false,
    };
    let _ = invalid_backing.u8();
    let _ = Reader::whole(&[]).u8();
    let _ = Reader::whole(&[0, 0, 0]).take_span(4);
    let _ = Reader::whole(b"unterminated").c_string();
    invalid_backing.offset = 0;
    let _ = invalid_backing.u16();
    invalid_backing.offset = 0;
    let _ = invalid_backing.u32();
    invalid_backing.offset = 0;
    let _ = invalid_backing.u64();
    invalid_backing.offset = 0;
    let _ = invalid_backing.four_cc();
    let mut overflowing_reader = Reader {
        input: &[],
        offset: usize::MAX,
        end: usize::MAX,
        truncation: false,
    };
    let _ = overflowing_reader.take_span(1);
    let _ = overflowing_reader.c_string();

    let mut budget = Budget {
        boxes: MAX_BOXES,
        records: MAX_RECORDS,
    };
    let _ = budget.box_seen();
    let _ = budget.records_seen(1);
    budget.boxes = usize::MAX;
    budget.records = usize::MAX;
    let _ = budget.box_seen();
    let _ = budget.records_seen(1);
    let box_header = coverage_box(*b"free", &[]);
    let mut box_reader = Reader::whole(&box_header);
    let mut exhausted_box_budget = Budget {
        boxes: MAX_BOXES,
        records: 0,
    };
    let _ = next_box(&mut box_reader, true, &mut exhausted_box_budget);

    let mut reader = Reader::whole(&[0, 0, 0, 0, b'f', b'r', b'e', b'e']);
    let _ = next_box(&mut reader, true, &mut Budget::default());
    let mut reader = Reader::whole(&[0, 0, 0, 0, b'f', b'r', b'e', b'e']);
    let _ = next_box(&mut reader, false, &mut Budget::default());
    let mut large = Vec::from([0, 0, 0, 1, b'f', b'r', b'e', b'e']);
    large.extend_from_slice(&16_u64.to_be_bytes());
    let mut reader = Reader::whole(&large);
    let _ = next_box(&mut reader, true, &mut Budget::default());
    let uuid = coverage_box(*b"uuid", &[0; 16]);
    let mut reader = Reader::whole(&uuid);
    let _ = next_box(&mut reader, true, &mut Budget::default());
    let short_uuid = coverage_box(*b"uuid", &[]);
    let mut reader = Reader::whole(&short_uuid);
    let _ = next_box(&mut reader, true, &mut Budget::default());
    let _ = parse_ftyp(&[0; 8], ByteSpan { start: 0, end: 12 });

    let baseline = include_bytes!("../../../tests/fixtures/input/images/avif/baseline.avif");
    let animated = include_bytes!("../../../tests/fixtures/input/images/avif/animated.avif");
    let meta_payload = &baseline[40..274];
    for duplicate in [
        &baseline[44..84],
        &baseline[84..98],
        &baseline[98..128],
        &baseline[128..168],
        &baseline[168..274],
    ] {
        let input = coverage_join(&[meta_payload, duplicate]);
        let _ = parse_meta(
            &input,
            ByteSpan {
                start: 0,
                end: input.len(),
            },
            &mut Budget::default(),
        );
    }
    let mut wrong_handler = meta_payload.to_vec();
    wrong_handler[20..24].copy_from_slice(b"vide");
    let _ = parse_meta(
        &wrong_handler,
        ByteSpan {
            start: 0,
            end: wrong_handler.len(),
        },
        &mut Budget::default(),
    );
    let iref = coverage_box(*b"iref", &coverage_full_box(2, 0, &[]));
    let duplicated_iref = coverage_join(&[meta_payload, &iref, &iref]);
    let _ = parse_meta(
        &duplicated_iref,
        ByteSpan {
            start: 0,
            end: duplicated_iref.len(),
        },
        &mut Budget::default(),
    );
    let idat = coverage_box(*b"idat", &[1]);
    let with_idat = coverage_join(&[meta_payload, &idat]);
    let _ = parse_meta(
        &with_idat,
        ByteSpan {
            start: 0,
            end: with_idat.len(),
        },
        &mut Budget::default(),
    );
    let duplicated_idat = coverage_join(&[meta_payload, &idat, &idat]);
    let _ = parse_meta(
        &duplicated_idat,
        ByteSpan {
            start: 0,
            end: duplicated_idat.len(),
        },
        &mut Budget::default(),
    );
    let empty_idat = coverage_box(*b"idat", &[]);
    let with_empty_idat = coverage_join(&[meta_payload, &empty_idat]);
    let _ = parse_meta(
        &with_empty_idat,
        ByteSpan {
            start: 0,
            end: with_empty_idat.len(),
        },
        &mut Budget::default(),
    );

    let _ = parse_iprp(
        baseline,
        ByteSpan {
            start: 176,
            end: 251,
        },
        &mut Meta::default(),
        &mut Budget::default(),
    );
    let duplicate_ipma = coverage_join(&[&baseline[176..274], &baseline[251..274]]);
    let _ = parse_iprp(
        &duplicate_ipma,
        ByteSpan {
            start: 0,
            end: duplicate_ipma.len(),
        },
        &mut Meta::default(),
        &mut Budget::default(),
    );
    let free = coverage_box(*b"free", &[]);
    let wrong_iprp_child = coverage_join(&[&baseline[176..251], &free]);
    let _ = parse_iprp(
        &wrong_iprp_child,
        ByteSpan {
            start: 0,
            end: wrong_iprp_child.len(),
        },
        &mut Meta::default(),
        &mut Budget::default(),
    );

    let infe_v3 = coverage_full_box(
        3,
        0,
        &coverage_join(&[&1_u32.to_be_bytes(), &0_u16.to_be_bytes(), b"av01", &[0]]),
    );
    let _ = parse_infe(
        &infe_v3,
        ByteSpan {
            start: 0,
            end: infe_v3.len(),
        },
    );
    for end in 0..infe_v3.len() {
        let _ = parse_infe(&infe_v3[..end], ByteSpan { start: 0, end });
    }
    let iinf_v1 = coverage_full_box(1, 0, &[]);
    let _ = parse_iinf(
        &iinf_v1,
        ByteSpan {
            start: 0,
            end: iinf_v1.len(),
        },
        &mut Meta::default(),
        &mut Budget::default(),
    );
    let mime_without_content_type = coverage_full_box(
        2,
        0,
        &coverage_join(&[&1_u16.to_be_bytes(), &0_u16.to_be_bytes(), b"mime", &[0]]),
    );
    let _ = parse_infe(
        &mime_without_content_type,
        ByteSpan {
            start: 0,
            end: mime_without_content_type.len(),
        },
    );
    let free_property = coverage_box(*b"free", &[]);
    let _ = parse_ipco(
        &free_property,
        ByteSpan {
            start: 0,
            end: free_property.len(),
        },
        &mut Meta::default(),
        &mut Budget {
            boxes: 0,
            records: MAX_RECORDS,
        },
    );
    let essential_zero_index = coverage_full_box(
        0,
        0,
        &coverage_join(&[&1_u32.to_be_bytes(), &1_u16.to_be_bytes(), &[1, 0x80]]),
    );
    let _ = parse_ipma(
        &essential_zero_index,
        ByteSpan {
            start: 0,
            end: essential_zero_index.len(),
        },
        &mut Meta::default(),
        &mut Budget::default(),
    );
    let ipma_v1 = coverage_full_box(1, 0, &coverage_join(&[&1_u32.to_be_bytes(), &[0, 0, 0]]));
    let _ = parse_ipma(
        &ipma_v1,
        ByteSpan {
            start: 0,
            end: ipma_v1.len(),
        },
        &mut Meta::default(),
        &mut Budget::default(),
    );
    let wide_ipma = coverage_full_box(
        0,
        1,
        &coverage_join(&[&1_u32.to_be_bytes(), &1_u16.to_be_bytes(), &[1, 0]]),
    );
    let _ = parse_ipma(
        &wide_ipma,
        ByteSpan {
            start: 0,
            end: wide_ipma.len(),
        },
        &mut Meta::default(),
        &mut Budget::default(),
    );
    let association_budget = coverage_full_box(
        0,
        0,
        &coverage_join(&[&1_u32.to_be_bytes(), &1_u16.to_be_bytes(), &[1]]),
    );
    let _ = parse_ipma(
        &association_budget,
        ByteSpan {
            start: 0,
            end: association_budget.len(),
        },
        &mut Meta::default(),
        &mut Budget {
            boxes: 0,
            records: MAX_RECORDS - 1,
        },
    );
    for version in [0, 1] {
        for length in 0..=10 {
            let child = coverage_box(*b"dimg", &vec![0; length]);
            let payload = coverage_full_box(version, 0, &child);
            let _ = parse_iref(
                &payload,
                ByteSpan {
                    start: 0,
                    end: payload.len(),
                },
                &mut Meta::default(),
                &mut Budget::default(),
            );
        }
    }

    let idat_input = [0xaa, 0xbb, 0xcc, 0xdd];
    let idat = ByteSpan {
        start: 0,
        end: idat_input.len(),
    };
    let iloc_v1 = coverage_full_box(
        1,
        0,
        &[
            0x44, 0x00, 0, 1, 0, 1, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 4,
        ],
    );
    let _ = parse_iloc(
        &idat_input,
        ByteSpan {
            start: 0,
            end: iloc_v1.len().min(idat_input.len()),
        },
        Some(idat),
        &mut Meta::default(),
        &mut Budget::default(),
    );
    let mut combined = iloc_v1.clone();
    combined.extend_from_slice(&idat_input);
    let _ = parse_iloc(
        &combined,
        ByteSpan {
            start: 0,
            end: iloc_v1.len(),
        },
        Some(ByteSpan {
            start: iloc_v1.len(),
            end: combined.len(),
        }),
        &mut Meta::default(),
        &mut Budget::default(),
    );
    for end in 0..=iloc_v1.len() {
        let _ = parse_iloc(
            &iloc_v1[..end],
            ByteSpan { start: 0, end },
            None,
            &mut Meta::default(),
            &mut Budget::default(),
        );
    }
    let iloc_v2 = coverage_full_box(
        2,
        0,
        &[
            0x44, 0x00, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1,
        ],
    );
    let _ = parse_iloc(
        &iloc_v2,
        ByteSpan {
            start: 0,
            end: iloc_v2.len(),
        },
        None,
        &mut Meta::default(),
        &mut Budget::default(),
    );
    for end in 0..iloc_v2.len() {
        let _ = parse_iloc(
            &iloc_v2[..end],
            ByteSpan { start: 0, end },
            None,
            &mut Meta::default(),
            &mut Budget::default(),
        );
    }
    let mut reserved_construction = iloc_v2.clone();
    reserved_construction[14] = 0x10;
    let _ = parse_iloc(
        &reserved_construction,
        ByteSpan {
            start: 0,
            end: reserved_construction.len(),
        },
        None,
        &mut Meta::default(),
        &mut Budget::default(),
    );
    let mut unknown_construction = iloc_v2.clone();
    unknown_construction[15] = 2;
    let _ = parse_iloc(
        &unknown_construction,
        ByteSpan {
            start: 0,
            end: unknown_construction.len(),
        },
        None,
        &mut Meta::default(),
        &mut Budget::default(),
    );
    let indexed_iloc = coverage_full_box(
        1,
        0,
        &[
            0x44, 0x04, 0, 1, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
        ],
    );
    let _ = parse_iloc(
        &indexed_iloc,
        ByteSpan {
            start: 0,
            end: indexed_iloc.len(),
        },
        None,
        &mut Meta::default(),
        &mut Budget::default(),
    );
    for end in 0..indexed_iloc.len() {
        let _ = parse_iloc(
            &indexed_iloc[..end],
            ByteSpan { start: 0, end },
            None,
            &mut Meta::default(),
            &mut Budget::default(),
        );
    }
    let based_iloc = coverage_full_box(
        0,
        0,
        &[
            0x44, 0x40, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1,
        ],
    );
    for end in 0..=based_iloc.len() {
        let _ = parse_iloc(
            &based_iloc[..end],
            ByteSpan { start: 0, end },
            None,
            &mut Meta::default(),
            &mut Budget::default(),
        );
    }
    let overflowing_extent = coverage_full_box(
        0,
        0,
        &coverage_join(&[
            &[0x84, 0x80],
            &1_u16.to_be_bytes(),
            &1_u16.to_be_bytes(),
            &0_u16.to_be_bytes(),
            &u64::MAX.to_be_bytes(),
            &1_u16.to_be_bytes(),
            &1_u64.to_be_bytes(),
            &1_u32.to_be_bytes(),
        ]),
    );
    let _ = parse_iloc(
        &overflowing_extent,
        ByteSpan {
            start: 0,
            end: overflowing_extent.len(),
        },
        None,
        &mut Meta::default(),
        &mut Budget::default(),
    );
    let overflowing_idat = coverage_full_box(
        1,
        0,
        &coverage_join(&[
            &[0x04, 0x80],
            &1_u16.to_be_bytes(),
            &1_u16.to_be_bytes(),
            &1_u16.to_be_bytes(),
            &0_u16.to_be_bytes(),
            &u64::MAX.to_be_bytes(),
            &1_u16.to_be_bytes(),
            &0_u32.to_be_bytes(),
        ]),
    );
    let _ = parse_iloc(
        &overflowing_idat,
        ByteSpan {
            start: 0,
            end: overflowing_idat.len(),
        },
        Some(ByteSpan { start: 1, end: 1 }),
        &mut Meta::default(),
        &mut Budget::default(),
    );
    let out_of_idat = coverage_full_box(
        1,
        0,
        &coverage_join(&[
            &[0x44, 0x00],
            &1_u16.to_be_bytes(),
            &1_u16.to_be_bytes(),
            &1_u16.to_be_bytes(),
            &0_u16.to_be_bytes(),
            &1_u16.to_be_bytes(),
            &1_u32.to_be_bytes(),
            &1_u32.to_be_bytes(),
        ]),
    );
    let _ = parse_iloc(
        &out_of_idat,
        ByteSpan {
            start: 0,
            end: out_of_idat.len(),
        },
        Some(ByteSpan { start: 0, end: 1 }),
        &mut Meta::default(),
        &mut Budget::default(),
    );

    let co64_payload = coverage_full_box(
        0,
        0,
        &coverage_join(&[&1_u32.to_be_bytes(), &0_u64.to_be_bytes()]),
    );
    let _ = parse_chunk_offsets(
        &co64_payload,
        BoxSpan {
            kind: *b"co64",
            payload: ByteSpan {
                start: 0,
                end: co64_payload.len(),
            },
        },
        &mut SampleTable::default(),
        &mut Budget::default(),
    );
    for end in 0..co64_payload.len() {
        let _ = parse_chunk_offsets(
            &co64_payload[..end],
            BoxSpan {
                kind: *b"co64",
                payload: ByteSpan { start: 0, end },
            },
            &mut SampleTable::default(),
            &mut Budget::default(),
        );
    }
    let common_stsz = coverage_full_box(
        0,
        0,
        &coverage_join(&[&1_u32.to_be_bytes(), &1_u32.to_be_bytes()]),
    );
    let _ = parse_stsz(
        &common_stsz,
        ByteSpan {
            start: 0,
            end: common_stsz.len(),
        },
        &mut Budget::default(),
    );

    let av1c = coverage_box(*b"av1C", &[0x81, 0, 0, 0]);
    let auxc = coverage_box(
        *b"auxC",
        &coverage_full_box(0, 0, &coverage_join(&[ALPHA_URN_MPEG_B, &[0]])),
    );
    let description = coverage_join(&[&av1c, &auxc]);
    let _ = parse_sample_description(
        &description,
        ByteSpan {
            start: 0,
            end: description.len(),
        },
        &mut Budget::default(),
    );
    let duplicate_av1c = coverage_join(&[&av1c, &av1c]);
    let _ = parse_sample_description(
        &duplicate_av1c,
        ByteSpan {
            start: 0,
            end: duplicate_av1c.len(),
        },
        &mut Budget::default(),
    );
    let duplicate_auxc = coverage_join(&[&auxc, &auxc]);
    let _ = parse_sample_description(
        &duplicate_auxc,
        ByteSpan {
            start: 0,
            end: duplicate_auxc.len(),
        },
        &mut Budget::default(),
    );
    let short_av01 = coverage_box(*b"av01", &[0]);
    let short_stsd = coverage_full_box(0, 0, &coverage_join(&[&1_u32.to_be_bytes(), &short_av01]));
    let _ = parse_stsd(
        &short_stsd,
        ByteSpan {
            start: 0,
            end: short_stsd.len(),
        },
        &mut Budget::default(),
    );
    let bad_first_chunk = coverage_full_box(
        0,
        0,
        &coverage_join(&[
            &1_u32.to_be_bytes(),
            &2_u32.to_be_bytes(),
            &1_u32.to_be_bytes(),
            &1_u32.to_be_bytes(),
        ]),
    );
    let _ = parse_stsc(
        &bad_first_chunk,
        ByteSpan {
            start: 0,
            end: bad_first_chunk.len(),
        },
        &mut Budget::default(),
    );
    let repeated_chunk = coverage_full_box(
        0,
        0,
        &coverage_join(&[
            &2_u32.to_be_bytes(),
            &1_u32.to_be_bytes(),
            &1_u32.to_be_bytes(),
            &1_u32.to_be_bytes(),
            &1_u32.to_be_bytes(),
            &1_u32.to_be_bytes(),
            &1_u32.to_be_bytes(),
        ]),
    );
    let _ = parse_stsc(
        &repeated_chunk,
        ByteSpan {
            start: 0,
            end: repeated_chunk.len(),
        },
        &mut Budget::default(),
    );

    let track_payload = coverage_join(&[&animated[422..526], &animated[570..1015]]);
    let _ = parse_movie(
        animated,
        ByteSpan {
            start: 294,
            end: 1015,
        },
        &mut Budget {
            boxes: 0,
            records: MAX_RECORDS,
        },
    );
    for duplicate in [&animated[422..526], &animated[570..1015]] {
        let input = coverage_join(&[&track_payload, duplicate]);
        let _ = parse_track(
            &input,
            ByteSpan {
                start: 0,
                end: input.len(),
            },
            &mut Budget::default(),
        );
    }
    let empty_tref = coverage_box(*b"tref", &[]);
    let duplicate_tref = coverage_join(&[&track_payload, &empty_tref, &empty_tref]);
    let _ = parse_track(
        &duplicate_tref,
        ByteSpan {
            start: 0,
            end: duplicate_tref.len(),
        },
        &mut Budget::default(),
    );
    let auxl = coverage_box(*b"auxl", &1_u32.to_be_bytes());
    let duplicated_auxl = coverage_join(&[&auxl, &auxl]);
    let _ = parse_tref(
        &duplicated_auxl,
        ByteSpan {
            start: 0,
            end: duplicated_auxl.len(),
        },
        &mut Budget::default(),
    );
    for length in 0..4 {
        let short_auxl = coverage_box(*b"auxl", &vec![0; length]);
        let _ = parse_tref(
            &short_auxl,
            ByteSpan {
                start: 0,
                end: short_auxl.len(),
            },
            &mut Budget::default(),
        );
    }

    let mdia_payload = &animated[578..1015];
    for duplicate in [
        &animated[578..622],
        &animated[622..662],
        &animated[662..1015],
    ] {
        let input = coverage_join(&[mdia_payload, duplicate]);
        let _ = parse_mdia(
            &input,
            ByteSpan {
                start: 0,
                end: input.len(),
            },
            &mut Track::default(),
            &mut Budget::default(),
        );
    }
    let minf_payload = &animated[670..1015];
    let duplicate_stbl = coverage_join(&[minf_payload, &animated[726..1015]]);
    let _ = parse_minf(
        &duplicate_stbl,
        ByteSpan {
            start: 0,
            end: duplicate_stbl.len(),
        },
        &mut Budget::default(),
    );
    let stbl_payload = &animated[734..1015];
    for duplicate in [
        &animated[734..883],
        &animated[883..907],
        &animated[907..935],
        &animated[935..975],
        &animated[975..995],
        &animated[995..1015],
    ] {
        let input = coverage_join(&[stbl_payload, duplicate]);
        let _ = parse_stbl(
            &input,
            ByteSpan {
                start: 0,
                end: input.len(),
            },
            &mut Budget::default(),
        );
    }

    let timings = [
        TimeToSample {
            sample_count: 1,
            sample_delta: 2,
        },
        TimeToSample {
            sample_count: 1,
            sample_delta: 3,
        },
    ];
    let _ = duration_at(&[], 0);
    let _ = duration_at(&timings, 0);
    let _ = duration_at(&timings, 3);

    let config_span = ByteSpan { start: 0, end: 1 };
    let duplicate_config_meta = Meta {
        properties: vec![Property::Av1C(config_span), Property::Av1C(config_span)],
        associations: vec![
            Association {
                item_id: 1,
                property_index: 0,
            },
            Association {
                item_id: 1,
                property_index: 1,
            },
        ],
        ..Meta::default()
    };
    let _ = duplicate_config_meta.av1c(1);

    let missing_metadata_location = Meta {
        items: vec![Item {
            id: 9,
            kind: *b"Exif",
            metadata_kind: Some(*b"Exif"),
        }],
        ..Meta::default()
    };
    let _ = missing_metadata_location.metadata(&[]);
    let mut extracted_metadata_missing_location =
        include_bytes!("../../../tests/fixtures/outputs/encoded/Encode.avif_enc_metadata.bin")
            .to_vec();
    // The second `infe` item id occupies these bytes in the committed witness;
    // leaving its `iloc` entry unchanged makes extraction reject the metadata
    // item at the public retention boundary.
    extracted_metadata_missing_location[0xca] = 9;
    assert!(extract_inner(&extracted_metadata_missing_location).is_err());
    let empty_metadata_location = Meta {
        items: vec![Item {
            id: 9,
            kind: *b"Exif",
            metadata_kind: Some(*b"Exif"),
        }],
        locations: vec![ItemLocation {
            item_id: 9,
            source: ExtentSource::File,
            extents: Vec::new(),
        }],
        ..Meta::default()
    };
    let _ = empty_metadata_location.metadata(&[]);
    let overflowing_metadata_capacity = Meta {
        items: vec![Item {
            id: 9,
            kind: *b"Exif",
            metadata_kind: Some(*b"Exif"),
        }],
        locations: vec![ItemLocation {
            item_id: 9,
            source: ExtentSource::File,
            extents: vec![
                ByteSpan {
                    start: 0,
                    end: usize::MAX,
                },
                ByteSpan { start: 0, end: 1 },
            ],
        }],
        ..Meta::default()
    };
    let _ = overflowing_metadata_capacity.metadata(&[]);
    let invalid_metadata_extent = Meta {
        items: vec![Item {
            id: 9,
            kind: *b"Exif",
            metadata_kind: Some(*b"Exif"),
        }],
        locations: vec![ItemLocation {
            item_id: 9,
            source: ExtentSource::File,
            extents: vec![ByteSpan { start: 0, end: 1 }],
        }],
        ..Meta::default()
    };
    let _ = invalid_metadata_extent.metadata(&[]);

    let coverage_plane = |spans: &[(usize, usize)]| EncodedPlane {
        samples: vec![EncodedSample {
            spans: spans
                .iter()
                .map(|&(start, end)| ByteSpan { start, end })
                .collect(),
            config: config_span,
            sync: true,
            duration: 1,
        }],
    };
    let empty_payload = ExtractedAvif {
        input: &[],
        still: None,
        sequence: None,
        consumed: 0,
        retained_boxes: Vec::new(),
        metadata: Vec::new(),
        source_color: SourceColor::new(),
        auxiliary_relationship: None,
        auxiliary_relationships: Vec::new(),
        item_relationships: Vec::new(),
        premultiplied_relationships: Vec::new(),
        item_color_properties: Vec::new(),
        item_icc_profiles: Vec::new(),
        item_properties: Vec::new(),
        item_plane_properties: Vec::new(),
        item_codec_properties: Vec::new(),
        grid_item_ids: Vec::new(),
        grid_properties: None,
        transform: None,
    };
    assert_eq!(pixel_payload_bytes(&empty_payload), 0);
    let mixed_payload = ExtractedAvif {
        input: &[],
        still: Some(StillPayload {
            color: coverage_plane(&[(0, 0), (1, 3), (2, 5), (5, 6), (8, 10)]),
            alpha: Some(coverage_plane(&[(12, 13)])),
        }),
        sequence: Some(SequencePayload {
            color: coverage_plane(&[(14, 16)]),
            alpha: Some(coverage_plane(&[(16, 18)])),
            timescale: NonZeroU32::new(1).unwrap(),
        }),
        consumed: 0,
        retained_boxes: Vec::new(),
        metadata: Vec::new(),
        source_color: SourceColor::new(),
        auxiliary_relationship: None,
        auxiliary_relationships: Vec::new(),
        item_relationships: Vec::new(),
        premultiplied_relationships: Vec::new(),
        item_color_properties: Vec::new(),
        item_icc_profiles: Vec::new(),
        item_properties: Vec::new(),
        item_plane_properties: Vec::new(),
        item_codec_properties: Vec::new(),
        grid_item_ids: Vec::new(),
        grid_properties: None,
        transform: None,
    };
    assert_eq!(pixel_payload_bytes(&mixed_payload), 12);
    let duplicate_alpha_meta = Meta {
        properties: vec![
            Property::AuxC {
                kind: *b"auxC",
                is_alpha: true,
                data: ByteSpan { start: 0, end: 0 },
            },
            Property::AuxC {
                kind: *b"auxC",
                is_alpha: true,
                data: ByteSpan { start: 0, end: 0 },
            },
        ],
        associations: vec![
            Association {
                item_id: 2,
                property_index: 0,
            },
            Association {
                item_id: 3,
                property_index: 1,
            },
        ],
        references: vec![
            Reference {
                kind: *b"auxl",
                from_id: 2,
                to_id: 1,
            },
            Reference {
                kind: *b"auxl",
                from_id: 3,
                to_id: 1,
            },
        ],
        ..Meta::default()
    };
    let _ = duplicate_alpha_meta.alpha_targeting(1);
    let _ = still_payload(&Meta::default());
    let duplicate_direct_alpha = Meta {
        primary_item_id: 1,
        items: vec![
            Item {
                id: 1,
                kind: *b"av01",
                metadata_kind: None,
            },
            Item {
                id: 2,
                kind: *b"av01",
                metadata_kind: None,
            },
            Item {
                id: 3,
                kind: *b"av01",
                metadata_kind: None,
            },
        ],
        properties: vec![
            Property::Av1C(config_span),
            Property::AuxC {
                kind: *b"auxC",
                is_alpha: true,
                data: ByteSpan { start: 0, end: 0 },
            },
            Property::AuxC {
                kind: *b"auxC",
                is_alpha: true,
                data: ByteSpan { start: 0, end: 0 },
            },
        ],
        associations: vec![
            Association {
                item_id: 1,
                property_index: 0,
            },
            Association {
                item_id: 2,
                property_index: 1,
            },
            Association {
                item_id: 3,
                property_index: 2,
            },
        ],
        references: vec![
            Reference {
                kind: *b"auxl",
                from_id: 2,
                to_id: 1,
            },
            Reference {
                kind: *b"auxl",
                from_id: 3,
                to_id: 1,
            },
        ],
        locations: vec![ItemLocation {
            item_id: 1,
            source: ExtentSource::File,
            extents: vec![config_span],
        }],
    };
    let _ = still_payload(&duplicate_direct_alpha);
    let duplicate_child_alpha = Meta {
        primary_item_id: 1,
        items: vec![
            Item {
                id: 1,
                kind: *b"grid",
                metadata_kind: None,
            },
            Item {
                id: 2,
                kind: *b"av01",
                metadata_kind: None,
            },
            Item {
                id: 3,
                kind: *b"av01",
                metadata_kind: None,
            },
            Item {
                id: 4,
                kind: *b"av01",
                metadata_kind: None,
            },
        ],
        properties: vec![
            Property::Av1C(config_span),
            Property::AuxC {
                kind: *b"auxC",
                is_alpha: true,
                data: ByteSpan { start: 0, end: 0 },
            },
            Property::AuxC {
                kind: *b"auxC",
                is_alpha: true,
                data: ByteSpan { start: 0, end: 0 },
            },
        ],
        associations: vec![
            Association {
                item_id: 2,
                property_index: 0,
            },
            Association {
                item_id: 3,
                property_index: 1,
            },
            Association {
                item_id: 4,
                property_index: 2,
            },
        ],
        references: vec![
            Reference {
                kind: *b"dimg",
                from_id: 1,
                to_id: 2,
            },
            Reference {
                kind: *b"auxl",
                from_id: 3,
                to_id: 2,
            },
            Reference {
                kind: *b"auxl",
                from_id: 4,
                to_id: 2,
            },
        ],
        locations: vec![ItemLocation {
            item_id: 2,
            source: ExtentSource::File,
            extents: vec![config_span],
        }],
    };
    let _ = still_payload(&duplicate_child_alpha);

    let sample_input = [0_u8; 8];
    let _ = track_plane(&sample_input, &Track::default());
    let mut mapped_track = coverage_track(1, *b"pict", None, 2, 1, None);
    let mapped_table = mapped_track.table.as_mut().unwrap();
    mapped_table.chunk_offsets = vec![0, 1];
    mapped_table.mappings = vec![
        SampleToChunk {
            first_chunk: 1,
            samples_per_chunk: 1,
            description_index: 1,
        },
        SampleToChunk {
            first_chunk: 2,
            samples_per_chunk: 1,
            description_index: 1,
        },
    ];
    let _ = track_plane(&sample_input, &mapped_track);

    let mut future_mapping = coverage_track(1, *b"pict", None, 1, 1, None);
    future_mapping.table.as_mut().unwrap().mappings[0].first_chunk = 2;
    let _ = track_plane(&sample_input, &future_mapping);
    let mut missing_mapping = coverage_track(1, *b"pict", None, 1, 1, None);
    missing_mapping.table.as_mut().unwrap().mappings.clear();
    let _ = track_plane(&sample_input, &missing_mapping);
    let mut zero_mapping = coverage_track(1, *b"pict", None, 1, 1, None);
    zero_mapping.table.as_mut().unwrap().mappings[0].samples_per_chunk = 0;
    let _ = track_plane(&sample_input, &zero_mapping);
    let mut missing_chunk = coverage_track(1, *b"pict", None, 1, 1, None);
    missing_chunk.table.as_mut().unwrap().chunk_offsets.clear();
    let _ = track_plane(&sample_input, &missing_chunk);
    let mut empty_track = coverage_track(1, *b"pict", None, 0, 1, None);
    empty_track.table.as_mut().unwrap().chunk_offsets.clear();
    empty_track.table.as_mut().unwrap().mappings[0].samples_per_chunk = 1;
    let _ = track_plane(&sample_input, &empty_track);

    let duplicate_alpha_movie = Movie {
        tracks: vec![
            coverage_track(1, *b"pict", None, 1, 1, None),
            coverage_track(2, *b"auxv", Some(1), 1, 1, Some(true)),
            coverage_track(3, *b"auxv", Some(1), 1, 1, Some(true)),
        ],
    };
    let _ = sequence_payload(&duplicate_alpha_movie, &sample_input);
    let unequal_alpha_movie = Movie {
        tracks: vec![
            coverage_track(1, *b"pict", None, 2, 1, None),
            coverage_track(2, *b"auxv", Some(1), 1, 1, Some(true)),
        ],
    };
    let _ = sequence_payload(&unequal_alpha_movie, &sample_input);
    let missing_timescale_movie = Movie {
        tracks: vec![coverage_track(1, *b"pict", None, 1, 0, None)],
    };
    let _ = sequence_payload(&missing_timescale_movie, &sample_input);

    let empty = ExtractedAvif {
        input: &[],
        still: None,
        sequence: None,
        consumed: 0,
        retained_boxes: Vec::new(),
        metadata: Vec::new(),
        source_color: SourceColor::new(),
        auxiliary_relationship: None,
        auxiliary_relationships: Vec::new(),
        item_relationships: Vec::new(),
        premultiplied_relationships: Vec::new(),
        item_color_properties: Vec::new(),
        item_icc_profiles: Vec::new(),
        item_properties: Vec::new(),
        item_plane_properties: Vec::new(),
        item_codec_properties: Vec::new(),
        grid_item_ids: Vec::new(),
        grid_properties: None,
        transform: None,
    };
    let _ = empty.validate();
    let invalid_plane = EncodedPlane {
        samples: Vec::new(),
    };
    let _ = validate_plane(&[], &invalid_plane);
    let empty_spans = EncodedPlane {
        samples: vec![EncodedSample {
            spans: Vec::new(),
            config: config_span,
            sync: true,
            duration: 1,
        }],
    };
    let _ = validate_plane(&sample_input, &empty_spans);
    let zero_span = EncodedPlane {
        samples: vec![EncodedSample {
            spans: vec![ByteSpan { start: 0, end: 0 }],
            config: config_span,
            sync: true,
            duration: 1,
        }],
    };
    let _ = validate_plane(&sample_input, &zero_span);
    let invalid_config = EncodedPlane {
        samples: vec![EncodedSample {
            spans: vec![config_span],
            config: ByteSpan { start: 0, end: 9 },
            sync: true,
            duration: 1,
        }],
    };
    let _ = validate_plane(&sample_input, &invalid_config);
    let invalid_span = EncodedPlane {
        samples: vec![EncodedSample {
            spans: vec![ByteSpan { start: 1, end: 9 }],
            config: config_span,
            sync: true,
            duration: 1,
        }],
    };
    let _ = validate_plane(&sample_input, &invalid_span);

    let invalid_still_color = ExtractedAvif {
        input: &sample_input,
        consumed: 0,
        retained_boxes: Vec::new(),
        metadata: Vec::new(),
        source_color: SourceColor::new(),
        auxiliary_relationship: None,
        auxiliary_relationships: Vec::new(),
        item_relationships: Vec::new(),
        premultiplied_relationships: Vec::new(),
        item_color_properties: Vec::new(),
        item_icc_profiles: Vec::new(),
        item_properties: Vec::new(),
        item_plane_properties: Vec::new(),
        item_codec_properties: Vec::new(),
        grid_item_ids: Vec::new(),
        grid_properties: None,
        transform: None,
        still: Some(StillPayload {
            color: EncodedPlane {
                samples: Vec::new(),
            },
            alpha: None,
        }),
        sequence: None,
    };
    let _ = invalid_still_color.validate();
    let invalid_still_alpha = ExtractedAvif {
        input: &sample_input,
        consumed: 0,
        retained_boxes: Vec::new(),
        metadata: Vec::new(),
        source_color: SourceColor::new(),
        auxiliary_relationship: None,
        auxiliary_relationships: Vec::new(),
        item_relationships: Vec::new(),
        premultiplied_relationships: Vec::new(),
        item_color_properties: Vec::new(),
        item_icc_profiles: Vec::new(),
        item_properties: Vec::new(),
        item_plane_properties: Vec::new(),
        item_codec_properties: Vec::new(),
        grid_item_ids: Vec::new(),
        grid_properties: None,
        transform: None,
        still: Some(StillPayload {
            color: EncodedPlane {
                samples: vec![EncodedSample {
                    spans: vec![config_span],
                    config: config_span,
                    sync: true,
                    duration: 1,
                }],
            },
            alpha: Some(EncodedPlane {
                samples: Vec::new(),
            }),
        }),
        sequence: None,
    };
    let _ = invalid_still_alpha.validate();
    let invalid_still = ExtractedAvif {
        input: &sample_input,
        consumed: 0,
        retained_boxes: Vec::new(),
        metadata: Vec::new(),
        source_color: SourceColor::new(),
        auxiliary_relationship: None,
        auxiliary_relationships: Vec::new(),
        item_relationships: Vec::new(),
        premultiplied_relationships: Vec::new(),
        item_color_properties: Vec::new(),
        item_icc_profiles: Vec::new(),
        item_properties: Vec::new(),
        item_plane_properties: Vec::new(),
        item_codec_properties: Vec::new(),
        grid_item_ids: Vec::new(),
        grid_properties: None,
        transform: None,
        still: Some(StillPayload {
            color: EncodedPlane {
                samples: vec![EncodedSample {
                    spans: vec![config_span],
                    config: config_span,
                    sync: true,
                    duration: 1,
                }],
            },
            alpha: Some(EncodedPlane {
                samples: vec![
                    EncodedSample {
                        spans: vec![config_span],
                        config: config_span,
                        sync: true,
                        duration: 1,
                    },
                    EncodedSample {
                        spans: vec![config_span],
                        config: config_span,
                        sync: true,
                        duration: 1,
                    },
                ],
            }),
        }),
        sequence: None,
    };
    let _ = invalid_still.validate();
    let sequence_only = ExtractedAvif {
        input: &sample_input,
        consumed: 0,
        retained_boxes: Vec::new(),
        metadata: Vec::new(),
        source_color: SourceColor::new(),
        auxiliary_relationship: None,
        auxiliary_relationships: Vec::new(),
        item_relationships: Vec::new(),
        premultiplied_relationships: Vec::new(),
        item_color_properties: Vec::new(),
        item_icc_profiles: Vec::new(),
        item_properties: Vec::new(),
        item_plane_properties: Vec::new(),
        item_codec_properties: Vec::new(),
        grid_item_ids: Vec::new(),
        grid_properties: None,
        transform: None,
        still: None,
        sequence: Some(SequencePayload {
            color: EncodedPlane {
                samples: vec![EncodedSample {
                    spans: vec![config_span],
                    config: config_span,
                    sync: true,
                    duration: 1,
                }],
            },
            alpha: None,
            timescale: NonZeroU32::new(1).unwrap(),
        }),
    };
    let _ = sequence_only.validate();
    let invalid_sequence_color = ExtractedAvif {
        input: &sample_input,
        consumed: 0,
        retained_boxes: Vec::new(),
        metadata: Vec::new(),
        source_color: SourceColor::new(),
        auxiliary_relationship: None,
        auxiliary_relationships: Vec::new(),
        item_relationships: Vec::new(),
        premultiplied_relationships: Vec::new(),
        item_color_properties: Vec::new(),
        item_icc_profiles: Vec::new(),
        item_properties: Vec::new(),
        item_plane_properties: Vec::new(),
        item_codec_properties: Vec::new(),
        grid_item_ids: Vec::new(),
        grid_properties: None,
        transform: None,
        still: None,
        sequence: Some(SequencePayload {
            color: EncodedPlane {
                samples: Vec::new(),
            },
            alpha: None,
            timescale: NonZeroU32::new(1).unwrap(),
        }),
    };
    let _ = invalid_sequence_color.validate();
    let invalid_sequence_alpha = ExtractedAvif {
        input: &sample_input,
        consumed: 0,
        retained_boxes: Vec::new(),
        metadata: Vec::new(),
        source_color: SourceColor::new(),
        auxiliary_relationship: None,
        auxiliary_relationships: Vec::new(),
        item_relationships: Vec::new(),
        premultiplied_relationships: Vec::new(),
        item_color_properties: Vec::new(),
        item_icc_profiles: Vec::new(),
        item_properties: Vec::new(),
        item_plane_properties: Vec::new(),
        item_codec_properties: Vec::new(),
        grid_item_ids: Vec::new(),
        grid_properties: None,
        transform: None,
        still: None,
        sequence: Some(SequencePayload {
            color: EncodedPlane {
                samples: vec![EncodedSample {
                    spans: vec![config_span],
                    config: config_span,
                    sync: true,
                    duration: 1,
                }],
            },
            alpha: Some(EncodedPlane {
                samples: Vec::new(),
            }),
            timescale: NonZeroU32::new(1).unwrap(),
        }),
    };
    let _ = invalid_sequence_alpha.validate();
    let invalid_sequence = ExtractedAvif {
        input: &sample_input,
        consumed: 0,
        retained_boxes: Vec::new(),
        metadata: Vec::new(),
        source_color: SourceColor::new(),
        auxiliary_relationship: None,
        auxiliary_relationships: Vec::new(),
        item_relationships: Vec::new(),
        premultiplied_relationships: Vec::new(),
        item_color_properties: Vec::new(),
        item_icc_profiles: Vec::new(),
        item_properties: Vec::new(),
        item_plane_properties: Vec::new(),
        item_codec_properties: Vec::new(),
        grid_item_ids: Vec::new(),
        grid_properties: None,
        transform: None,
        still: None,
        sequence: Some(SequencePayload {
            color: EncodedPlane {
                samples: vec![EncodedSample {
                    spans: vec![config_span],
                    config: config_span,
                    sync: true,
                    duration: 1,
                }],
            },
            alpha: Some(EncodedPlane {
                samples: vec![
                    EncodedSample {
                        spans: vec![config_span],
                        config: config_span,
                        sync: true,
                        duration: 1,
                    },
                    EncodedSample {
                        spans: vec![config_span],
                        config: config_span,
                        sync: true,
                        duration: 1,
                    },
                ],
            }),
            timescale: NonZeroU32::new(1).unwrap(),
        }),
    };
    let _ = invalid_sequence.validate();

    let duplicate_meta = coverage_join(&[baseline, &baseline[32..274]]);
    let _ = extract_inner(&duplicate_meta);
    let duplicate_movie = coverage_join(&[animated, &animated[286..1015]]);
    let _ = extract_inner(&duplicate_movie);
    let mut zero_extent = baseline.to_vec();
    zero_extent[124..128].fill(0);
    let _ = super::inspect::inspect(&zero_extent);
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    coverage_fixture_contracts();
    coverage_leaf_corpus();
    coverage_parser_truncations();
    coverage_structural_states();

    let baseline = include_bytes!("../../../tests/fixtures/input/images/avif/baseline.avif");
    let alpha = include_bytes!("../../../tests/fixtures/input/images/avif/alpha.avif");
    let grid = include_bytes!("../../../tests/fixtures/input/images/avif/grid.avif");
    let animated = include_bytes!("../../../tests/fixtures/input/images/avif/animated.avif");
    let high_bit = include_bytes!("../../../tests/fixtures/input/images/avif/10bit.avif");
    coverage_prefixes(baseline);
    coverage_prefixes(alpha);
    coverage_prefixes(grid);
    coverage_prefixes(animated);
    coverage_prefixes(high_bit);
    coverage_metadata_mutations(baseline, 282);
    coverage_metadata_mutations(alpha, 727);
    coverage_metadata_mutations(grid, 1_467);
    coverage_metadata_mutations(animated, 1_023);
    coverage_metadata_mutations(high_bit, 2_022);
    let pasp_zero_vertical = [0, 0, 0, 4, 0, 0, 0, 0];
    let _ = parse_pasp(
        &pasp_zero_vertical,
        ByteSpan {
            start: 0,
            end: pasp_zero_vertical.len(),
        },
    );
    let clap_payload = [
        0, 0, 0, 2, 0, 0, 0, 1, 0, 0, 0, 3, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0,
        0, 1,
    ];
    let clap_box = coverage_box(*b"clap", &clap_payload);
    let _ = parse_sample_description(
        &clap_box,
        ByteSpan {
            start: 0,
            end: clap_box.len(),
        },
        &mut Budget::default(),
    );

    let duplicate_rotation = Meta {
        primary_item_id: 1,
        properties: vec![
            Property::Rotation {
                value: crate::types::AvifRotation::Zero,
                data: ByteSpan { start: 0, end: 0 },
            },
            Property::Rotation {
                value: crate::types::AvifRotation::CounterClockwise90,
                data: ByteSpan { start: 0, end: 0 },
            },
        ],
        associations: vec![
            Association {
                item_id: 1,
                property_index: 0,
            },
            Association {
                item_id: 1,
                property_index: 1,
            },
        ],
        ..Meta::default()
    };
    let _ = duplicate_rotation.transform();
    let duplicate_pixel_aspect_ratio = Meta {
        primary_item_id: 1,
        properties: vec![
            Property::PixelAspectRatio {
                value: AvifPixelAspectRatio::new(4, 3),
                data: ByteSpan { start: 0, end: 0 },
            },
            Property::PixelAspectRatio {
                value: AvifPixelAspectRatio::new(16, 9),
                data: ByteSpan { start: 0, end: 0 },
            },
        ],
        associations: vec![
            Association {
                item_id: 1,
                property_index: 0,
            },
            Association {
                item_id: 1,
                property_index: 1,
            },
        ],
        ..Meta::default()
    };
    let _ = duplicate_pixel_aspect_ratio.transform();
    let duplicate_clean_aperture = Meta {
        primary_item_id: 1,
        properties: vec![
            Property::CleanAperture {
                value: AvifCleanAperture::new(2, 1, 3, 1, 0, 1, 0, 1),
                data: ByteSpan { start: 0, end: 0 },
            },
            Property::CleanAperture {
                value: AvifCleanAperture::new(4, 1, 3, 1, 0, 1, 0, 1),
                data: ByteSpan { start: 0, end: 0 },
            },
        ],
        associations: vec![
            Association {
                item_id: 1,
                property_index: 0,
            },
            Association {
                item_id: 1,
                property_index: 1,
            },
        ],
        ..Meta::default()
    };
    let _ = duplicate_clean_aperture.transform();
    let duplicate_mirror = Meta {
        primary_item_id: 1,
        properties: vec![
            Property::Mirror {
                value: crate::types::AvifMirrorAxis::TopBottom,
                data: ByteSpan { start: 0, end: 0 },
            },
            Property::Mirror {
                value: crate::types::AvifMirrorAxis::LeftRight,
                data: ByteSpan { start: 0, end: 0 },
            },
        ],
        associations: vec![
            Association {
                item_id: 1,
                property_index: 0,
            },
            Association {
                item_id: 1,
                property_index: 1,
            },
        ],
        ..Meta::default()
    };
    let _ = duplicate_mirror.transform();
}
