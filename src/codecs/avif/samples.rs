//! Bounded extraction of encoded AV1 sample spans from AVIF containers.

use std::num::NonZeroU32;

use crate::codecs::{CodecError, CodecResult};
use crate::types::{
    AnimationLoop, AvifAuxiliaryRelationship, AvifChromaSamplePosition, AvifCleanAperture,
    AvifColorProperties, AvifContentLightLevel, AvifFileTypeProperties, AvifGridProperties,
    AvifItemCodecProperties, AvifItemColorProperties, AvifItemExtent, AvifItemIccProfile,
    AvifItemLocation, AvifItemLocationSource, AvifItemPlaneProperties, AvifItemProperty,
    AvifItemRelationship, AvifMasteringDisplayColorVolume, AvifMirrorAxis, AvifPixelAspectRatio,
    AvifRotation, AvifTransformProperties, OpaqueMetadata, RawIccProfile, SourceColor,
};

const MAX_BOXES: usize = 4_096;
const MAX_RECORDS: usize = 4_096;
// Item-property entries are retained as owned records. Keep their table
// bounded independently from the enclosing-box and association budgets so a
// validly-shaped `ipco` box cannot consume the entire parser budget first.
const MAX_PROPERTIES: usize = 2_048;
const MAX_COMPATIBLE_BRANDS: usize = 1_024;
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

#[derive(Clone)]
struct Brands {
    major: FourCc,
    minor_version: u32,
    compatible_brands: Vec<FourCc>,
    has_avif: bool,
    has_avis: bool,
}

fn parse_ftyp(input: &[u8], payload: ByteSpan) -> ParseResult<Brands> {
    let mut reader = Reader::new(input, payload);
    let major = reader.four_cc()?;
    let minor_version = reader.u32()?;
    if !(reader.end.saturating_sub(reader.offset)).is_multiple_of(4) {
        return Err(parse_failure!());
    }
    let mut has_avif = major == *b"avif";
    let mut has_avis = major == *b"avis";
    let mut compatible_brands = Vec::new();
    let compatible = input
        .get(reader.offset..reader.end)
        .ok_or_else(|| parse_failure!())?;
    for bytes in compatible.as_chunks::<4>().0 {
        if compatible_brands.len() >= MAX_COMPATIBLE_BRANDS {
            return Err(parse_failure!());
        }
        let brand = [bytes[0], bytes[1], bytes[2], bytes[3]];
        has_avif |= brand == *b"avif";
        has_avis |= brand == *b"avis";
        compatible_brands.push(brand);
    }
    if !has_avif && !has_avis {
        return Err(parse_failure!());
    }
    Ok(Brands {
        major,
        minor_version,
        compatible_brands,
        has_avif,
        has_avis,
    })
}

pub(super) fn file_type(input: &[u8]) -> CodecResult<AvifFileTypeProperties> {
    let mut budget = Budget::default();
    let mut reader = Reader::whole(input);
    let first = next_box(&mut reader, true, &mut budget)
        .map_err(|error| error.at(0, "avif_box"))?
        .ok_or_else(|| parse_failure!())?;
    if first.kind != *b"ftyp" {
        return Err(parse_failure!());
    }
    let brands = parse_ftyp(input, first.payload).map_err(|error| error.at(0, "avif_box"))?;
    Ok(AvifFileTypeProperties::new(
        brands.major,
        brands.minor_version,
        brands.compatible_brands,
    ))
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
    Av1C {
        data: ByteSpan,
        depth: u8,
    },
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
    essential: bool,
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
    declared_extents: Vec<AvifItemExtent>,
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
        if meta.properties.len() >= MAX_PROPERTIES {
            return Err(parse_failure!());
        }
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
            let (depth, _) = parse_av1c_declaration(property.payload.bytes(input)?)?;
            Ok(Property::Av1C {
                data: property.payload,
                depth,
            })
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
    // libavif 1.4.1 read.c:380-390 selects twelveBit first, even when
    // highBitdepth is clear. Actual AV1 sample depth is checked independently.
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
    // The pinned Pillow/libavif oracle accepts trailing bytes on this fixed
    // metadata property. Preserve the specified fields and leave the bounded
    // extension bytes uninterpreted, as the container projection does.
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
    if !reader.is_empty() {
        // Pillow/libavif accepts trailing clean-aperture payload bytes but
        // ignores the property and decodes the coded item unchanged.
        return Ok(Property::Other {
            kind: *b"clap",
            data: payload.bytes(input)?.to_vec(),
        });
    }
    if width_numerator == 0
        || width_denominator == 0
        || height_numerator == 0
        || height_denominator == 0
        || horizontal_offset_denominator == 0
        || vertical_offset_denominator == 0
    {
        // Pillow/libavif ignores these unusable clean-aperture values and
        // decodes the coded primary item without cropping.
        return Ok(Property::Other {
            kind: *b"clap",
            data: payload.bytes(input)?.to_vec(),
        });
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
            let essential = raw & essential_mask != 0;
            let property_index = raw & index_mask;
            if property_index == 0 {
                if essential {
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
                essential,
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
        let mut declared_extents = Vec::with_capacity(extent_count);
        for _ in 0..extent_count {
            if index_size != 0 {
                let _ = reader.uint(index_size)?;
            }
            let extent_offset = reader.uint(offset_size)?;
            let extent_length = reader.uint(length_size)?;
            let relative = base_offset
                .checked_add(extent_offset)
                .ok_or_else(|| parse_failure!())?;
            declared_extents.push(AvifItemExtent::new(relative, extent_length));
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
            declared_extents,
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

    fn item_locations(&self) -> Vec<AvifItemLocation> {
        self.locations
            .iter()
            .map(|location| {
                let source = match location.source {
                    ExtentSource::File => AvifItemLocationSource::File,
                    ExtentSource::Idat => AvifItemLocationSource::Idat,
                };
                AvifItemLocation::new(location.item_id, source, location.declared_extents.clone())
            })
            .collect()
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
            if let Property::Av1C { data, .. } = property {
                Some(*data)
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
            if let Property::Av1C { data, depth } = property {
                Some((*data, *depth))
            } else {
                None
            }
        });
        let (config, config_depth) = configs.next().ok_or_else(|| parse_failure!())?;
        if configs.next().is_some() {
            return Err(parse_failure!());
        }
        // Pillow/libavif validates the first associated pixi property's
        // depth against av1C before opening the item; later duplicates do not
        // change that selection. Reuse the already parsed configuration depth.
        let pixel_depth = self.associated(item_id).find_map(|property| {
            if let Property::Pixi { depth } = property {
                Some(*depth)
            } else {
                None
            }
        });
        if pixel_depth.is_some_and(|depth| depth != config_depth) {
            return Err(CodecError::Malformed(
                "AVIF pixel-information depth disagrees with its AV1 codec configuration"
                    .to_owned(),
            ));
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
                Property::ContentLightLevel { data, .. } => {
                    Some(AvifItemProperty::new_with_essential(
                        association.item_id,
                        *b"clli",
                        data.bytes(input)?.to_vec(),
                        association.essential,
                    ))
                }
                Property::MasteringDisplayColorVolume { data, .. } => {
                    Some(AvifItemProperty::new_with_essential(
                        association.item_id,
                        *b"mdcv",
                        data.bytes(input)?.to_vec(),
                        association.essential,
                    ))
                }
                Property::Rotation { data, .. } => Some(AvifItemProperty::new_with_essential(
                    association.item_id,
                    *b"irot",
                    data.bytes(input)?.to_vec(),
                    association.essential,
                )),
                Property::Mirror { data, .. } => Some(AvifItemProperty::new_with_essential(
                    association.item_id,
                    *b"imir",
                    data.bytes(input)?.to_vec(),
                    association.essential,
                )),
                Property::PixelAspectRatio { data, .. } => {
                    Some(AvifItemProperty::new_with_essential(
                        association.item_id,
                        *b"pasp",
                        data.bytes(input)?.to_vec(),
                        association.essential,
                    ))
                }
                Property::CleanAperture { data, .. } => Some(AvifItemProperty::new_with_essential(
                    association.item_id,
                    *b"clap",
                    data.bytes(input)?.to_vec(),
                    association.essential,
                )),
                Property::AuxC {
                    is_alpha: false,
                    kind,
                    data,
                } => Some(AvifItemProperty::new_with_essential(
                    association.item_id,
                    *kind,
                    data.bytes(input)?.to_vec(),
                    association.essential,
                )),
                Property::Other { kind, data } => Some(AvifItemProperty::new_with_essential(
                    association.item_id,
                    *kind,
                    data.clone(),
                    association.essential,
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
                if let Property::Av1C { data: span, .. } = property {
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
            return Err(CodecError::NotImplemented(format!(
                "AVIF grid item version {version} is not implemented"
            )));
        }
        let flags = prefix[1];
        let rows = u32::from(prefix[2]).saturating_add(1);
        let columns = u32::from(prefix[3]).saturating_add(1);
        let field_width: usize = if flags & 1 == 0 { 2 } else { 4 };
        let expected_length = 4usize.saturating_add(field_width.saturating_mul(2));
        // `ByteSpan::bytes` returns every extent at its declared length, so
        // once the total extent length matches the bounded grid payload,
        // `copied` necessarily reaches the same length.
        if total_length != expected_length {
            return Err(parse_failure!());
        }
        debug_assert!(copied >= expected_length);
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

#[derive(Clone)]
pub(super) struct EncodedSample {
    pub(super) spans: Vec<ByteSpan>,
    pub(super) config: ByteSpan,
    pub(super) sync: bool,
    pub(super) duration: u32,
}

#[derive(Clone)]
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
    pub(super) loop_count: AnimationLoop,
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
    pub(super) item_locations: Vec<AvifItemLocation>,
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

struct Track {
    id: u32,
    handler: FourCc,
    aux_for_id: Option<u32>,
    timescale: Option<NonZeroU32>,
    track_duration: u64,
    repetition: AnimationLoop,
    table: Option<SampleTable>,
}

impl Default for Track {
    fn default() -> Self {
        Self {
            id: 0,
            handler: [0; 4],
            aux_for_id: None,
            timescale: None,
            track_duration: 0,
            repetition: AnimationLoop::Unspecified,
            table: None,
        }
    }
}

#[derive(Clone, Copy)]
struct EditList {
    repeating: bool,
    segment_duration: u64,
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
    let mut edit_list = None;
    let mut edts_seen = false;
    while let Some(child) = next_box(&mut reader, false, budget)? {
        match child.kind {
            kind if kind == *b"tkhd" => {
                if tkhd_seen {
                    return Err(parse_failure!());
                }
                tkhd_seen = true;
                let (id, duration) = parse_tkhd(input, child.payload)?;
                track.id = id;
                track.track_duration = duration;
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
            kind if kind == *b"edts" => {
                if edts_seen {
                    return Err(parse_failure!());
                }
                edts_seen = true;
                edit_list = Some(parse_edit_box(input, child.payload, budget)?);
            }
            _ => {}
        }
    }
    if !tkhd_seen || !mdia_seen {
        return Err(parse_failure!());
    }
    track.repetition = match edit_list {
        None => AnimationLoop::Unspecified,
        Some(edit) if !edit.repeating => AnimationLoop::Finite { total_plays: 1 },
        Some(_edit) if track.track_duration == u64::MAX => AnimationLoop::Infinite,
        Some(_edit) if track.track_duration == 0 => return Err(parse_failure!()),
        Some(edit) => {
            let quotient = track
                .track_duration
                .checked_div(edit.segment_duration)
                .ok_or_else(|| parse_failure!())?;
            let plays = quotient
                .checked_add(u64::from(
                    !track.track_duration.is_multiple_of(edit.segment_duration),
                ))
                .ok_or_else(|| parse_failure!())?;
            // libavif stores repetitions (total plays minus one) as a signed
            // 32-bit value and normalizes larger values to infinite playback.
            // Keep INT_MAX + 1 total plays finite; avoid rounding by addition
            // to the untrusted duration, which could overflow before division.
            if plays > 1_u64 << 31 {
                AnimationLoop::Infinite
            } else {
                AnimationLoop::Finite {
                    total_plays: u32::try_from(plays).map_err(|_| parse_failure!())?,
                }
            }
        }
    };
    Ok(track)
}

fn parse_tkhd(input: &[u8], payload: ByteSpan) -> ParseResult<(u32, u64)> {
    let mut reader = Reader::new(input, payload);
    let (version, _) = parse_full_box(&mut reader)?;
    let (track_id, duration) = match version {
        0 => {
            reader.skip(8)?;
            let track_id = reader.u32()?;
            reader.skip(4)?;
            let duration = reader.u32()?;
            let duration = if duration == u32::MAX {
                u64::MAX
            } else {
                u64::from(duration)
            };
            (track_id, duration)
        }
        1 => {
            reader.skip(16)?;
            let track_id = reader.u32()?;
            reader.skip(4)?;
            let duration = reader.u64()?;
            (track_id, duration)
        }
        _ => return Err(parse_failure!()),
    };
    if track_id == 0 {
        return Err(parse_failure!());
    }
    Ok((track_id, duration))
}

fn parse_edit_box(input: &[u8], payload: ByteSpan, budget: &mut Budget) -> ParseResult<EditList> {
    let mut reader = Reader::new(input, payload);
    let mut edit_list = None;
    while let Some(child) = next_box(&mut reader, false, budget)? {
        if child.kind == *b"elst" {
            if edit_list.is_some() {
                return Err(parse_failure!());
            }
            edit_list = Some(parse_edit_list_box(input, child.payload)?);
        }
    }
    edit_list.ok_or_else(|| parse_failure!())
}

fn parse_edit_list_box(input: &[u8], payload: ByteSpan) -> ParseResult<EditList> {
    let mut reader = Reader::new(input, payload);
    let (version, flags) = parse_full_box(&mut reader)?;
    // Pinned libavif only interprets bit zero. Nonrepeating edit lists do
    // not read entries or validate their version, duration, or media fields.
    if flags & 1 == 0 {
        return Ok(EditList {
            repeating: false,
            segment_duration: 0,
        });
    }
    let entry_count = reader.u32()?;
    if entry_count != 1 {
        return Err(parse_failure!());
    }
    let segment_duration = match version {
        0 => u64::from(reader.u32()?),
        1 => reader.u64()?,
        _ => return Err(parse_failure!()),
    };
    if segment_duration == 0 {
        return Err(parse_failure!());
    }
    // The complete-file native witnesses show that media time/rate and any
    // remaining payload are ignored, including their absence. Box extents
    // remain validated by the caller before these semantic fields are read.
    Ok(EditList {
        repeating: true,
        segment_duration,
    })
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
            Property::Av1C { data: span, .. } => {
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

/// Prove that a track's decode-time table supplies one positive duration for
/// every sample. `stts` is optional for still-image item tables, but a movie
/// track must not silently invent a duration when the table is absent or
/// under-filled. Keep this proof local to `track_plane`, which is used only by
/// sequence tracks.
fn validate_track_timings(table: &SampleTable) -> ParseResult<()> {
    if table.timings.is_empty() {
        return Err(parse_failure!());
    }
    let mut covered = 0_usize;
    for timing in &table.timings {
        if timing.sample_count == 0 || timing.sample_delta == 0 {
            return Err(parse_failure!());
        }
        covered = covered
            .checked_add(usize::try_from(timing.sample_count).map_err(|_| parse_failure!())?)
            .ok_or_else(|| parse_failure!())?;
    }
    if covered != table.sample_sizes.len() {
        return Err(parse_failure!());
    }
    Ok(())
}

fn duration_at(timings: &[TimeToSample], sample_index: usize) -> Option<u32> {
    let mut covered = 0_usize;
    for timing in timings {
        covered = covered.checked_add(usize::try_from(timing.sample_count).ok()?)?;
        if sample_index < covered {
            return Some(timing.sample_delta);
        }
    }
    None
}

// ✅ VERIFIED: libavif 1.4.1 read.c:520-607. Chunk mappings expand in
// declaration order, and the first sample is sync even without stss.
fn track_plane(input: &[u8], track: &Track) -> ParseResult<EncodedPlane> {
    let table = track.table.as_ref().ok_or_else(|| parse_failure!())?;
    validate_track_timings(table)?;
    let mut samples = Vec::with_capacity(table.sample_sizes.len());
    let mut sample_index = 0_usize;
    let mut mapping_index = 0_usize;
    for (chunk_index, &chunk_offset) in table.chunk_offsets.iter().enumerate() {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "The chunk-offset table count is encoded as u32, bounding every chunk index."
        )]
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
            #[allow(
                clippy::cast_possible_truncation,
                reason = "The sample-size table count is encoded as u32, bounding every sample index."
            )]
            let sample_number = (sample_index as u32).saturating_add(1);
            samples.push(EncodedSample {
                spans: vec![span],
                config,
                sync: sample_index == 0 || table.sync_samples.contains(&sample_number),
                duration: duration_at(&table.timings, sample_index)
                    .ok_or_else(|| parse_failure!())?,
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
        loop_count: color_track.repetition,
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
    #[allow(
        unused_assignments,
        reason = "The loop overwrites consumed at its start before any value is read."
    )]
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

    let item_locations = meta.as_ref().map(Meta::item_locations).unwrap_or_default();
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
        item_locations,
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
                #[allow(
                    clippy::arithmetic_side_effects,
                    reason = "Sorted valid spans guarantee current_start is no greater than current_end."
                )]
                let length = (current_end - current_start) as u64;
                total = total.saturating_add(length);
                current = Some((start, end));
            }
            None => current = Some((start, end)),
        }
    }
    if let Some((current_start, current_end)) = current {
        #[allow(
            clippy::arithmetic_side_effects,
            reason = "The final valid span also guarantees current_start is no greater than current_end."
        )]
        let length = (current_end - current_start) as u64;
        total = total.saturating_add(length);
    }
    total
}

/// Measure the encoded metadata extent: the parsed top-level BMFF bytes minus
/// the referenced primary and auxiliary pixel-sample payload spans.
pub(super) fn metadata_bytes(data: &[u8]) -> CodecResult<u64> {
    let extracted = extract_inner_with_metadata(data, false)?;
    #[allow(
        clippy::cast_possible_truncation,
        reason = "Supported targets have usize no wider than u64."
    )]
    let consumed = extracted.consumed as u64;
    let pixel = pixel_payload_bytes(&extracted);
    // Every referenced sample span belongs to a successfully parsed top-level
    // extent, so the pixel union cannot exceed `consumed`.
    Ok(consumed.saturating_sub(pixel))
}
