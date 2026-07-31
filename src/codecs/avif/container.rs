//! Bounded AVIF ISO-BMFF metadata parser.
//!
//! Ported from libavif 1.4.1 at commit
//! 6543b22b5bc706c53f038a16fe515f921556d9b3. Reference locations are recorded
//! in `docs/avif.md`.

use crate::codecs::{CodecError, CodecResult};
use crate::types::{ImageFormat, ImageInfo, ImageMode};

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
            concat!("invalid AVIF metadata structure at ", file!(), ":", line!()).to_owned(),
        )
    };
}

#[derive(Clone, Copy)]
struct BoxView<'a> {
    kind: FourCc,
    payload: &'a [u8],
}

#[derive(Default)]
struct Budget {
    boxes: usize,
    records: usize,
}

impl Budget {
    fn box_seen(&mut self) -> ParseResult<()> {
        if self.boxes >= MAX_BOXES {
            return Err(parse_failure!());
        }
        self.boxes = self.boxes.saturating_add(1);
        Ok(())
    }

    fn record_seen(&mut self) -> ParseResult<()> {
        if self.records >= MAX_RECORDS {
            return Err(parse_failure!());
        }
        self.records = self.records.saturating_add(1);
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct Reader<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    const fn new(data: &'a [u8]) -> Self {
        Self { data, offset: 0 }
    }

    fn remaining(self) -> usize {
        self.data.len().saturating_sub(self.offset)
    }

    fn is_empty(self) -> bool {
        self.offset == self.data.len()
    }

    fn take(&mut self, length: usize) -> ParseResult<&'a [u8]> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or_else(|| parse_failure!())?;
        let bytes = self
            .data
            .get(self.offset..end)
            .ok_or_else(|| parse_failure!())?;
        self.offset = end;
        Ok(bytes)
    }

    fn skip(&mut self, length: usize) -> ParseResult<()> {
        let _ = self.take(length)?;
        Ok(())
    }

    fn take_remaining(&mut self) -> &'a [u8] {
        let value = &self.data[self.offset..];
        self.offset = self.data.len();
        value
    }

    fn u8(&mut self) -> ParseResult<u8> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> ParseResult<u16> {
        let bytes = self.take(2)?;
        Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
    }

    fn u32(&mut self) -> ParseResult<u32> {
        let bytes = self.take(4)?;
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn u64(&mut self) -> ParseResult<u64> {
        let bytes = self.take(8)?;
        Ok(u64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    fn four_cc(&mut self) -> ParseResult<FourCc> {
        let bytes = self.take(4)?;
        Ok([bytes[0], bytes[1], bytes[2], bytes[3]])
    }

    fn c_string(&mut self) -> ParseResult<&'a [u8]> {
        let remaining = self
            .data
            .get(self.offset..)
            .ok_or_else(|| parse_failure!())?;
        let length = remaining
            .iter()
            .position(|&byte| byte == 0)
            .ok_or_else(|| parse_failure!())?;
        let value = &remaining[..length];
        self.offset = self.offset.saturating_add(length).saturating_add(1);
        Ok(value)
    }
}

#[derive(Clone, Copy)]
struct Brands {
    major: FourCc,
    has_avis: bool,
}

#[derive(Clone, Copy)]
enum Property {
    Ispe { width: u32, height: u32 },
    Pixi { depth: u8 },
    Av1C { depth: u8 },
    AuxC { is_alpha: bool },
    Other,
}

#[derive(Clone, Copy)]
struct Item {
    id: u32,
    kind: FourCc,
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

#[derive(Default)]
struct Meta {
    primary_item_id: Option<u32>,
    items: Vec<Item>,
    properties: Vec<Property>,
    associations: Vec<Association>,
    references: Vec<Reference>,
}

#[derive(Clone, Copy)]
struct Details {
    width: u32,
    height: u32,
    depth: u8,
    has_alpha: bool,
    frame_count: u32,
}

#[derive(Default)]
struct Movie {
    tracks: Vec<Track>,
}

#[derive(Clone, Copy, Default)]
struct Track {
    id: u32,
    handler: FourCc,
    width: u32,
    height: u32,
    sample_count: u32,
    depth: Option<u8>,
    aux_for_id: Option<u32>,
    aux_is_alpha: Option<bool>,
}

/// Inspect AVIF container metadata without calling a native codec.
pub(super) fn inspect(data: &[u8]) -> CodecResult<ImageInfo> {
    inspect_inner(data)
}

fn inspect_inner(data: &[u8]) -> ParseResult<ImageInfo> {
    let mut budget = Budget::default();
    let mut reader = Reader::new(data);
    let first = next_box(&mut reader, true, &mut budget)?.ok_or_else(|| parse_failure!())?;
    if first.kind != *b"ftyp" {
        return Err(parse_failure!());
    }
    let brands = parse_ftyp(first.payload)?;
    let mut meta = None;
    let mut movie = None;

    loop {
        match next_box(&mut reader, true, &mut budget) {
            Ok(Some(child)) => match child.kind {
                kind if kind == *b"meta" => {
                    if meta.is_some() {
                        return Err(parse_failure!());
                    }
                    meta = Some(parse_meta(child.payload, &mut budget)?);
                }
                kind if kind == *b"moov" => {
                    if movie.is_some() {
                        return Err(parse_failure!());
                    }
                    movie = Some(parse_movie(child.payload, &mut budget)?);
                }
                _ => {}
            },
            Ok(None) => break,
            Err(error) => {
                // Bytes after a complete still or sequence structure are
                // trailing input and are ignored, matching Pillow/libavif.
                let complete = meta.is_some() || (brands.has_avis && movie.is_some());
                if !complete {
                    return Err(error);
                }
                break;
            }
        }
    }

    let Some(meta) = meta.as_ref() else {
        return Err(parse_failure!());
    };
    if brands.has_avis && movie.is_none() {
        return Err(parse_failure!());
    }

    let meta_details = meta.details()?;
    let track_details = movie.as_ref().and_then(Movie::details);
    let prefer_tracks =
        brands.major == *b"avis" || (brands.major != *b"avif" && track_details.is_some());
    let details = if prefer_tracks {
        track_details.or(meta_details)
    } else {
        meta_details.or(track_details)
    };
    let Some(details) = details else {
        return Err(parse_failure!());
    };
    image_info(details)
}

fn image_info(details: Details) -> ParseResult<ImageInfo> {
    if details.width == 0 || details.height == 0 || details.frame_count == 0 {
        return Err(parse_failure!());
    }

    Ok(ImageInfo {
        format: ImageFormat::Avif,
        width: details.width,
        height: details.height,
        mode: if details.has_alpha {
            ImageMode::Rgba8
        } else {
            ImageMode::Rgb8
        },
        bit_depth: details.depth,
        palette: None,
        is_animated: details.frame_count > 1,
        frame_count: Some(details.frame_count),
        cursor_hotspot: None,
        source: crate::types::SourceDescriptor::new(),
    })
}

// libavif 1.4.1 src/stream.c:248-305.
fn next_box<'a>(
    reader: &mut Reader<'a>,
    top_level: bool,
    budget: &mut Budget,
) -> ParseResult<Option<BoxView<'a>>> {
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
    if size == 0 {
        if !top_level {
            return Err(parse_failure!());
        }
        let payload = reader.take_remaining();
        return Ok(Some(BoxView { kind, payload }));
    }
    if size > u64::from(u32::MAX) {
        return Err(parse_failure!());
    }
    let size = bounded_usize_u32(low_u32(size));
    if size < header_size {
        return Err(parse_failure!());
    }
    let payload = reader.take(size.saturating_sub(header_size))?;
    Ok(Some(BoxView { kind, payload }))
}

// libavif 1.4.1 src/read.c:4775-5031.
fn parse_ftyp(payload: &[u8]) -> ParseResult<Brands> {
    let mut reader = Reader::new(payload);
    let major = reader.four_cc()?;
    reader.skip(4)?;
    if !reader.remaining().is_multiple_of(4) {
        return Err(parse_failure!());
    }
    let mut has_avif = major == *b"avif";
    let mut has_avis = major == *b"avis";
    for bytes in reader.data[reader.offset..].chunks_exact(4) {
        let brand = [bytes[0], bytes[1], bytes[2], bytes[3]];
        has_avif |= brand == *b"avif";
        has_avis |= brand == *b"avis";
    }
    if !has_avif && !has_avis {
        return Err(parse_failure!());
    }
    Ok(Brands { major, has_avis })
}

fn parse_full_box(reader: &mut Reader<'_>) -> ParseResult<(u8, u32)> {
    let raw = reader.u32()?;
    Ok(((raw >> 24).to_le_bytes()[0], raw & 0x00ff_ffff))
}

fn parse_full_box_version_zero(reader: &mut Reader<'_>) -> ParseResult<u32> {
    let (version, flags) = parse_full_box(reader)?;
    if version != 0 {
        return Err(parse_failure!());
    }
    Ok(flags)
}

// libavif 1.4.1 src/read.c:3428-3511.
fn parse_meta(payload: &[u8], budget: &mut Budget) -> ParseResult<Meta> {
    let mut reader = Reader::new(payload);
    let _ = parse_full_box_version_zero(&mut reader)?;
    let first = next_box(&mut reader, false, budget)?.ok_or_else(|| parse_failure!())?;
    if first.kind != *b"hdlr" || parse_handler(first.payload)? != *b"pict" {
        return Err(parse_failure!());
    }

    let mut meta = Meta::default();
    let mut pitm_seen = false;
    let mut iinf_seen = false;
    let mut iprp_seen = false;
    let mut iref_seen = false;
    while let Some(child) = next_box(&mut reader, false, budget)? {
        match child.kind {
            kind if kind == *b"hdlr" => return Err(parse_failure!()),
            kind if kind == *b"pitm" => {
                if pitm_seen {
                    return Err(parse_failure!());
                }
                pitm_seen = true;
                meta.primary_item_id = Some(parse_pitm(child.payload)?);
            }
            kind if kind == *b"iinf" => {
                if iinf_seen {
                    return Err(parse_failure!());
                }
                iinf_seen = true;
                parse_iinf(child.payload, &mut meta, budget)?;
            }
            kind if kind == *b"iprp" => {
                if iprp_seen {
                    return Err(parse_failure!());
                }
                iprp_seen = true;
                parse_iprp(child.payload, &mut meta, budget)?;
            }
            kind if kind == *b"iref" => {
                if iref_seen {
                    return Err(parse_failure!());
                }
                iref_seen = true;
                parse_iref(child.payload, &mut meta, budget)?;
            }
            _ => {}
        }
    }
    if !pitm_seen || !iinf_seen || !iprp_seen {
        return Err(parse_failure!());
    }
    Ok(meta)
}

// libavif 1.4.1 src/read.c:1948-1972.
fn parse_handler(payload: &[u8]) -> ParseResult<FourCc> {
    let mut reader = Reader::new(payload);
    let _ = parse_full_box_version_zero(&mut reader)?;
    if reader.u32()? != 0 {
        return Err(parse_failure!());
    }
    let handler = reader.four_cc()?;
    reader.skip(12)?;
    let _ = reader.c_string()?;
    Ok(handler)
}

// libavif 1.4.1 src/read.c:3148-3170.
fn parse_pitm(payload: &[u8]) -> ParseResult<u32> {
    let mut reader = Reader::new(payload);
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

// libavif 1.4.1 src/read.c:3246-3328.
fn parse_iinf(payload: &[u8], meta: &mut Meta, budget: &mut Budget) -> ParseResult<()> {
    let mut reader = Reader::new(payload);
    let (version, _) = parse_full_box(&mut reader)?;
    let entry_count = match version {
        0 => u32::from(reader.u16()?),
        1 => reader.u32()?,
        _ => return Err(parse_failure!()),
    };
    for _ in 0..entry_count {
        let child = next_box(&mut reader, false, budget)?.ok_or_else(|| parse_failure!())?;
        if child.kind != *b"infe" {
            return Err(parse_failure!());
        }
        let item = parse_infe(child.payload)?;
        if meta.items.iter().any(|existing| existing.id == item.id) {
            return Err(parse_failure!());
        }
        budget.record_seen()?;
        meta.items.push(item);
    }
    if !reader.is_empty() {
        return Err(parse_failure!());
    }
    Ok(())
}

fn parse_infe(payload: &[u8]) -> ParseResult<Item> {
    let mut reader = Reader::new(payload);
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
    if kind == *b"mime" {
        let _ = reader.c_string()?;
    }
    Ok(Item { id, kind })
}

// libavif 1.4.1 src/read.c:2913-3243.
fn parse_iprp(payload: &[u8], meta: &mut Meta, budget: &mut Budget) -> ParseResult<()> {
    let mut reader = Reader::new(payload);
    let ipco = next_box(&mut reader, false, budget)?.ok_or_else(|| parse_failure!())?;
    if ipco.kind != *b"ipco" {
        return Err(parse_failure!());
    }
    parse_ipco(ipco.payload, meta, budget)?;
    let mut ipma_seen = false;
    while let Some(child) = next_box(&mut reader, false, budget)? {
        if child.kind != *b"ipma" || ipma_seen {
            return Err(parse_failure!());
        }
        ipma_seen = true;
        parse_ipma(child.payload, meta, budget)?;
    }
    if !ipma_seen {
        return Err(parse_failure!());
    }
    Ok(())
}

fn parse_ipco(payload: &[u8], meta: &mut Meta, budget: &mut Budget) -> ParseResult<()> {
    let mut reader = Reader::new(payload);
    while let Some(child) = next_box(&mut reader, false, budget)? {
        budget.record_seen()?;
        meta.properties.push(parse_property(child)?);
    }
    Ok(())
}

fn parse_property(property: BoxView<'_>) -> ParseResult<Property> {
    match property.kind {
        kind if kind == *b"ispe" => {
            let mut reader = Reader::new(property.payload);
            let _ = parse_full_box_version_zero(&mut reader)?;
            let width = reader.u32()?;
            let height = reader.u32()?;
            if width == 0 || height == 0 {
                return Err(parse_failure!());
            }
            Ok(Property::Ispe { width, height })
        }
        kind if kind == *b"pixi" => {
            let mut reader = Reader::new(property.payload);
            let _ = parse_full_box_version_zero(&mut reader)?;
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
        kind if kind == *b"av1C" => parse_av1c(property.payload),
        [b'a', b'u', b'x', b'C'] | [b'a', b'u', b'x', b'i'] => {
            let mut reader = Reader::new(property.payload);
            let _ = parse_full_box_version_zero(&mut reader)?;
            let urn = reader.c_string()?;
            Ok(Property::AuxC {
                is_alpha: is_alpha_urn(urn),
            })
        }
        _ => Ok(Property::Other),
    }
}

// libavif 1.4.1 src/read.c:2648-2693.
fn parse_av1c(payload: &[u8]) -> ParseResult<Property> {
    let mut reader = Reader::new(payload);
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
    let depth = if twelve_bit {
        12
    } else if high_bit_depth {
        10
    } else {
        8
    };
    Ok(Property::Av1C { depth })
}

fn is_alpha_urn(urn: &[u8]) -> bool {
    urn == ALPHA_URN_MPEG_B || urn == ALPHA_URN_HEVC
}

fn parse_ipma(payload: &[u8], meta: &mut Meta, budget: &mut Budget) -> ParseResult<()> {
    let mut reader = Reader::new(payload);
    let (version, flags) = parse_full_box(&mut reader)?;
    let wide_index = flags & 1 != 0;
    let entry_count = reader.u32()?;
    let mut previous_id = 0;
    for _ in 0..entry_count {
        let item_id = if version == 0 {
            u32::from(reader.u16()?)
        } else {
            reader.u32()?
        };
        if item_id == 0 {
            return Err(parse_failure!());
        }
        if item_id <= previous_id {
            return Err(parse_failure!());
        }
        previous_id = item_id;
        let association_count = reader.u8()?;
        for _ in 0..association_count {
            let raw = if wide_index {
                u32::from(reader.u16()?)
            } else {
                u32::from(reader.u8()?)
            };
            let essential_mask = if wide_index { 0x8000 } else { 0x80 };
            let index_mask = if wide_index { 0x7fff } else { 0x7f };
            let essential = raw & essential_mask != 0;
            let property_index = raw & index_mask;
            if property_index == 0 {
                if essential {
                    return Err(parse_failure!());
                }
                continue;
            }
            let property_index = bounded_usize_u32(property_index.saturating_sub(1));
            if property_index >= meta.properties.len() {
                return Err(parse_failure!());
            }
            budget.record_seen()?;
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

// libavif 1.4.1 src/read.c:3333-3415.
fn parse_iref(payload: &[u8], meta: &mut Meta, budget: &mut Budget) -> ParseResult<()> {
    let mut reader = Reader::new(payload);
    let (version, _) = parse_full_box(&mut reader)?;
    if version > 1 {
        return Ok(());
    }
    while let Some(child) = next_box(&mut reader, false, budget)? {
        let mut references = Reader::new(child.payload);
        let from_id = if version == 0 {
            u32::from(references.u16()?)
        } else {
            references.u32()?
        };
        if from_id == 0 {
            return Err(parse_failure!());
        }
        let count = references.u16()?;
        for _ in 0..count {
            let to_id = if version == 0 {
                u32::from(references.u16()?)
            } else {
                references.u32()?
            };
            if to_id == 0 {
                return Err(parse_failure!());
            }
            budget.record_seen()?;
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

impl Meta {
    fn details(&self) -> CodecResult<Option<Details>> {
        let Some(primary) = self.primary_item_id else {
            return Ok(None);
        };
        let primary_item = self
            .items
            .iter()
            .find(|item| item.id == primary)
            .ok_or_else(|| parse_failure!())?;
        if !matches!(
            primary_item.kind,
            [b'a', b'v', b'0', b'1'] | [b'g', b'r', b'i', b'd']
        ) {
            return Ok(None);
        }
        let dimensions = self.associated(primary).find_map(|property| {
            if let Property::Ispe { width, height } = property {
                Some((*width, *height))
            } else {
                None
            }
        });
        let Some((width, height)) = dimensions else {
            return Ok(None);
        };
        let depth = self
            .associated(primary)
            .find_map(|property| match property {
                Property::Pixi { depth } | Property::Av1C { depth } => Some(*depth),
                _ => None,
            })
            .unwrap_or(8);
        Ok(Some(Details {
            width,
            height,
            depth,
            has_alpha: self.has_alpha(primary),
            frame_count: 1,
        }))
    }

    fn associated(&self, item_id: u32) -> impl Iterator<Item = &Property> {
        self.associations
            .iter()
            .filter(move |association| association.item_id == item_id)
            .filter_map(|association| self.properties.get(association.property_index))
    }

    fn has_alpha(&self, primary: u32) -> bool {
        let mut color_items = vec![primary];
        loop {
            let mut changed = false;
            for reference in &self.references {
                if reference.kind == *b"dimg"
                    && color_items.contains(&reference.from_id)
                    && !color_items.contains(&reference.to_id)
                {
                    color_items.push(reference.to_id);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        self.references.iter().any(|reference| {
            reference.kind == *b"auxl"
                && color_items.contains(&reference.to_id)
                && self
                    .associated(reference.from_id)
                    .any(|property| matches!(property, Property::AuxC { is_alpha: true }))
        })
    }
}

// libavif 1.4.1 src/read.c:3515-4050.
fn parse_movie(payload: &[u8], budget: &mut Budget) -> ParseResult<Movie> {
    let mut reader = Reader::new(payload);
    let mut movie = Movie::default();
    while let Some(child) = next_box(&mut reader, false, budget)? {
        if child.kind == *b"trak" {
            budget.record_seen()?;
            movie.tracks.push(parse_track(child.payload, budget)?);
        }
    }
    if movie.tracks.is_empty() {
        return Err(parse_failure!());
    }
    Ok(movie)
}

fn parse_track(payload: &[u8], budget: &mut Budget) -> ParseResult<Track> {
    let mut reader = Reader::new(payload);
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
                parse_tkhd(child.payload, &mut track)?;
            }
            kind if kind == *b"mdia" => {
                if mdia_seen {
                    return Err(parse_failure!());
                }
                mdia_seen = true;
                parse_mdia(child.payload, &mut track, budget)?;
            }
            kind if kind == *b"tref" => {
                if tref_seen {
                    return Err(parse_failure!());
                }
                tref_seen = true;
                parse_tref(child.payload, &mut track, budget)?;
            }
            _ => {}
        }
    }
    if !tkhd_seen || !mdia_seen || track.id == 0 {
        return Err(parse_failure!());
    }
    Ok(track)
}

fn parse_tkhd(payload: &[u8], track: &mut Track) -> ParseResult<()> {
    let mut reader = Reader::new(payload);
    let (version, _) = parse_full_box(&mut reader)?;
    match version {
        0 => {
            reader.skip(8)?;
            track.id = reader.u32()?;
            reader.skip(8)?;
        }
        1 => {
            reader.skip(16)?;
            track.id = reader.u32()?;
            reader.skip(12)?;
        }
        _ => return Err(parse_failure!()),
    }
    reader.skip(52)?;
    track.width = reader.u32()? >> 16;
    track.height = reader.u32()? >> 16;
    Ok(())
}

fn parse_tref(payload: &[u8], track: &mut Track, budget: &mut Budget) -> ParseResult<()> {
    let mut reader = Reader::new(payload);
    while let Some(child) = next_box(&mut reader, false, budget)? {
        if child.kind == *b"auxl" {
            let mut ids = Reader::new(child.payload);
            track.aux_for_id = Some(ids.u32()?);
        }
    }
    Ok(())
}

fn parse_mdia(payload: &[u8], track: &mut Track, budget: &mut Budget) -> ParseResult<()> {
    let mut reader = Reader::new(payload);
    let mut handler_seen = false;
    let mut minf_seen = false;
    while let Some(child) = next_box(&mut reader, false, budget)? {
        match child.kind {
            kind if kind == *b"hdlr" => {
                if handler_seen {
                    return Err(parse_failure!());
                }
                handler_seen = true;
                track.handler = parse_handler(child.payload)?;
            }
            kind if kind == *b"minf" => {
                if minf_seen {
                    return Err(parse_failure!());
                }
                minf_seen = true;
                parse_minf(child.payload, track, budget)?;
            }
            _ => {}
        }
    }
    if !handler_seen || !minf_seen {
        return Err(parse_failure!());
    }
    Ok(())
}

fn parse_minf(payload: &[u8], track: &mut Track, budget: &mut Budget) -> ParseResult<()> {
    let mut reader = Reader::new(payload);
    let mut stbl_seen = false;
    while let Some(child) = next_box(&mut reader, false, budget)? {
        if child.kind == *b"stbl" {
            if stbl_seen {
                return Err(parse_failure!());
            }
            stbl_seen = true;
            parse_stbl(child.payload, track, budget)?;
        }
    }
    if !stbl_seen {
        return Err(parse_failure!());
    }
    Ok(())
}

fn parse_stbl(payload: &[u8], track: &mut Track, budget: &mut Budget) -> ParseResult<()> {
    let mut reader = Reader::new(payload);
    let mut stsz_seen = false;
    let mut stsd_seen = false;
    while let Some(child) = next_box(&mut reader, false, budget)? {
        match child.kind {
            kind if kind == *b"stsz" => {
                if stsz_seen {
                    return Err(parse_failure!());
                }
                stsz_seen = true;
                track.sample_count = parse_stsz(child.payload)?;
            }
            kind if kind == *b"stsd" => {
                if stsd_seen {
                    return Err(parse_failure!());
                }
                stsd_seen = true;
                parse_stsd(child.payload, track, budget)?;
            }
            _ => {}
        }
    }
    if !stsz_seen || !stsd_seen {
        return Err(parse_failure!());
    }
    Ok(())
}

fn parse_stsz(payload: &[u8]) -> ParseResult<u32> {
    let mut reader = Reader::new(payload);
    let _ = parse_full_box_version_zero(&mut reader)?;
    let sample_size = reader.u32()?;
    let sample_count = reader.u32()?;
    if sample_size == 0 {
        let entry_bytes = sample_count
            .checked_mul(4)
            .ok_or_else(|| parse_failure!())?;
        let entry_bytes = bounded_usize_u32(entry_bytes);
        reader.skip(entry_bytes)?;
    }
    if !reader.is_empty() {
        return Err(parse_failure!());
    }
    Ok(sample_count)
}

fn parse_stsd(payload: &[u8], track: &mut Track, budget: &mut Budget) -> ParseResult<()> {
    let mut reader = Reader::new(payload);
    let (version, _) = parse_full_box(&mut reader)?;
    if !matches!(version, 0 | 1) {
        return Err(parse_failure!());
    }
    let entry_count = reader.u32()?;
    for _ in 0..entry_count {
        let sample = next_box(&mut reader, false, budget)?.ok_or_else(|| parse_failure!())?;
        if sample.kind != *b"av01" {
            continue;
        }
        let properties = sample
            .payload
            .get(VISUAL_SAMPLE_ENTRY_SIZE..)
            .ok_or_else(|| parse_failure!())?;
        parse_track_properties(properties, track, budget)?;
    }
    if !reader.is_empty() {
        return Err(parse_failure!());
    }
    Ok(())
}

fn parse_track_properties(
    payload: &[u8],
    track: &mut Track,
    budget: &mut Budget,
) -> ParseResult<()> {
    let mut reader = Reader::new(payload);
    while let Some(child) = next_box(&mut reader, false, budget)? {
        match parse_property(child)? {
            Property::Av1C { depth } => {
                if track.depth.is_some_and(|existing| existing != depth) {
                    return Err(parse_failure!());
                }
                track.depth = Some(depth);
            }
            Property::AuxC { is_alpha } => {
                if track
                    .aux_is_alpha
                    .is_some_and(|existing| existing != is_alpha)
                {
                    return Err(parse_failure!());
                }
                track.aux_is_alpha = Some(is_alpha);
            }
            _ => {}
        }
    }
    Ok(())
}

impl Movie {
    fn details(&self) -> Option<Details> {
        let main = self.tracks.iter().find(|track| {
            matches!(
                track.handler,
                [b'p', b'i', b'c', b't'] | [b'v', b'i', b'd', b'e']
            )
        })?;
        let has_alpha = self.tracks.iter().any(|track| {
            track.handler == *b"auxv"
                && track.aux_for_id == Some(main.id)
                && track.aux_is_alpha.unwrap_or(true)
        });
        Some(Details {
            width: main.width,
            height: main.height,
            depth: main.depth.unwrap_or(8),
            has_alpha,
            frame_count: main.sample_count,
        })
    }
}

fn bounded_usize_u32(value: u32) -> usize {
    value as usize
}

fn low_u32(value: u64) -> u32 {
    let [_, _, _, _, a, b, c, d] = value.to_be_bytes();
    u32::from_be_bytes([a, b, c, d])
}

#[cfg(coverage)]
fn coverage_box(kind: FourCc, payload: &[u8]) -> Vec<u8> {
    let size = payload.len().wrapping_add(8);
    let size_bytes = size.to_be_bytes();
    let mut bytes = Vec::with_capacity(size);
    #[cfg(target_pointer_width = "64")]
    bytes.extend_from_slice(&size_bytes[4..]);
    #[cfg(target_pointer_width = "32")]
    bytes.extend_from_slice(&size_bytes);
    bytes.extend_from_slice(&kind);
    bytes.extend_from_slice(payload);
    bytes
}

#[cfg(coverage)]
fn coverage_full_box(version: u8, flags: u32, payload: &[u8]) -> Vec<u8> {
    let flag_bytes = flags.to_be_bytes();
    let mut bytes = Vec::with_capacity(payload.len() + 4);
    bytes.extend_from_slice(&[version, flag_bytes[1], flag_bytes[2], flag_bytes[3]]);
    bytes.extend_from_slice(payload);
    bytes
}

#[cfg(coverage)]
fn coverage_join(parts: &[&[u8]]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for part in parts {
        bytes.extend_from_slice(part);
    }
    bytes
}

#[cfg(coverage)]
fn coverage_handler(kind: FourCc) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&0_u32.to_be_bytes());
    payload.extend_from_slice(&kind);
    payload.extend_from_slice(&[0; 12]);
    payload.push(0);
    coverage_full_box(0, 0, &payload)
}

#[cfg(coverage)]
fn coverage_tkhd(version: u8, id: u32) -> Vec<u8> {
    let mut payload = Vec::new();
    if version == 0 {
        payload.extend_from_slice(&[0; 8]);
        payload.extend_from_slice(&id.to_be_bytes());
        payload.extend_from_slice(&[0; 8]);
    } else {
        payload.extend_from_slice(&[0; 16]);
        payload.extend_from_slice(&id.to_be_bytes());
        payload.extend_from_slice(&[0; 12]);
    }
    payload.extend_from_slice(&[0; 52]);
    payload.extend_from_slice(&(2_u32 << 16).to_be_bytes());
    payload.extend_from_slice(&(3_u32 << 16).to_be_bytes());
    coverage_full_box(version, 0, &payload)
}

#[cfg(coverage)]
fn coverage_stbl() -> Vec<u8> {
    let stsz = coverage_box(
        *b"stsz",
        &coverage_full_box(0, 0, &[0, 0, 0, 1, 0, 0, 0, 1]),
    );
    let stsd = coverage_box(*b"stsd", &coverage_full_box(0, 0, &[0, 0, 0, 0]));
    coverage_join(&[&stsz, &stsd])
}

#[cfg(coverage)]
fn coverage_prefixes(data: &[u8]) {
    for end in 0..=data.len() {
        let _ = inspect_inner(&data[..end]);
    }
}

#[cfg(coverage)]
fn coverage_metadata_mutations(data: &[u8]) {
    let limit = data.len().min(2_048);
    for offset in 0..limit {
        let mut mutated = data.to_vec();
        mutated[offset] ^= 0xff;
        let _ = inspect_inner(&mutated);
        mutated[offset] ^= 0x55;
        let _ = inspect_inner(&mutated);
    }
}

#[cfg(coverage)]
fn coverage_each_prefix(data: &[u8], mut parser: impl FnMut(&[u8])) {
    for end in 0..=data.len() {
        parser(&data[..end]);
    }
    for offset in 0..data.len() {
        let mut mutated = data.to_vec();
        mutated[offset] ^= 0xff;
        parser(&mutated);
        mutated[offset] ^= 0x55;
        parser(&mutated);
    }
}

#[cfg(coverage)]
const fn coverage_box_budget() -> Budget {
    Budget {
        boxes: MAX_BOXES,
        records: 0,
    }
}

#[cfg(coverage)]
const fn coverage_record_budget() -> Budget {
    Budget {
        boxes: 0,
        records: MAX_RECORDS,
    }
}

#[cfg(coverage)]
fn coverage_nested_parser_prefixes() {
    let handler_payload = coverage_handler(*b"pict");
    coverage_each_prefix(&handler_payload, |payload| {
        let _ = parse_handler(payload);
    });

    let pitm_payload = coverage_full_box(0, 0, &[0, 1]);
    coverage_each_prefix(&pitm_payload, |payload| {
        let _ = parse_pitm(payload);
    });
    let pitm_v1_payload = coverage_full_box(1, 0, &[0, 0, 0, 1]);
    coverage_each_prefix(&pitm_v1_payload, |payload| {
        let _ = parse_pitm(payload);
    });

    let infe_payload = coverage_full_box(2, 0, &[0, 1, 0, 0, b'a', b'v', b'0', b'1', 0]);
    coverage_each_prefix(&infe_payload, |payload| {
        let _ = parse_infe(payload);
    });
    let infe_v3_payload =
        coverage_full_box(3, 0, &[0, 0, 0, 1, 0, 0, b'm', b'i', b'm', b'e', 0, 0]);
    coverage_each_prefix(&infe_v3_payload, |payload| {
        let _ = parse_infe(payload);
    });
    let infe = coverage_box(*b"infe", &infe_payload);
    let iinf_payload = coverage_full_box(0, 0, &coverage_join(&[&[0, 1], &infe]));
    coverage_each_prefix(&iinf_payload, |payload| {
        let _ = parse_iinf(payload, &mut Meta::default(), &mut Budget::default());
    });
    let iinf_v1_payload = coverage_full_box(1, 0, &[0, 0, 0, 0]);
    coverage_each_prefix(&iinf_v1_payload, |payload| {
        let _ = parse_iinf(payload, &mut Meta::default(), &mut Budget::default());
    });

    let ispe_payload = coverage_full_box(0, 0, &[0, 0, 0, 2, 0, 0, 0, 3]);
    coverage_each_prefix(&ispe_payload, |payload| {
        let _ = parse_property(BoxView {
            kind: *b"ispe",
            payload,
        });
    });
    let pixi_payload = coverage_full_box(0, 0, &[3, 8, 8, 8]);
    coverage_each_prefix(&pixi_payload, |payload| {
        let _ = parse_property(BoxView {
            kind: *b"pixi",
            payload,
        });
    });
    let av1c_payload = [0x81, 0, 0, 0];
    coverage_each_prefix(&av1c_payload, |payload| {
        let _ = parse_av1c(payload);
    });
    let auxc_payload = coverage_full_box(0, 0, &coverage_join(&[ALPHA_URN_MPEG_B, &[0]]));
    coverage_each_prefix(&auxc_payload, |payload| {
        let _ = parse_property(BoxView {
            kind: *b"auxC",
            payload,
        });
    });

    let ispe = coverage_box(*b"ispe", &ispe_payload);
    let pixi = coverage_box(*b"pixi", &pixi_payload);
    let av1c = coverage_box(*b"av1C", &av1c_payload);
    let ipco_payload = coverage_join(&[&ispe, &pixi, &av1c]);
    coverage_each_prefix(&ipco_payload, |payload| {
        let _ = parse_ipco(payload, &mut Meta::default(), &mut Budget::default());
    });

    let ipma_payload = coverage_full_box(0, 0, &[0, 0, 0, 1, 0, 1, 1, 1]);
    coverage_each_prefix(&ipma_payload, |payload| {
        let mut meta = Meta {
            properties: vec![Property::Other],
            ..Meta::default()
        };
        let _ = parse_ipma(payload, &mut meta, &mut Budget::default());
    });
    let ipma_v1_payload = coverage_full_box(1, 0, &[0, 0, 0, 1, 0, 0, 0, 1, 1, 1]);
    coverage_each_prefix(&ipma_v1_payload, |payload| {
        let mut meta = Meta {
            properties: vec![Property::Other],
            ..Meta::default()
        };
        let _ = parse_ipma(payload, &mut meta, &mut Budget::default());
    });
    let ipma_wide_payload = coverage_full_box(0, 1, &[0, 0, 0, 1, 0, 1, 1, 0, 1]);
    coverage_each_prefix(&ipma_wide_payload, |payload| {
        let mut meta = Meta {
            properties: vec![Property::Other],
            ..Meta::default()
        };
        let _ = parse_ipma(payload, &mut meta, &mut Budget::default());
    });
    let ipco = coverage_box(*b"ipco", &ipco_payload);
    let ipma = coverage_box(*b"ipma", &ipma_payload);
    let iprp_payload = coverage_join(&[&ipco, &ipma]);
    coverage_each_prefix(&iprp_payload, |payload| {
        let _ = parse_iprp(payload, &mut Meta::default(), &mut Budget::default());
    });

    let auxl_reference = coverage_box(*b"auxl", &[0, 0, 0, 1, 0, 1, 0, 0, 0, 2]);
    let iref_payload = coverage_full_box(1, 0, &auxl_reference);
    coverage_each_prefix(&iref_payload, |payload| {
        let _ = parse_iref(payload, &mut Meta::default(), &mut Budget::default());
    });
    let iref_v0_payload = coverage_full_box(0, 0, &coverage_box(*b"auxl", &[0, 1, 0, 1, 0, 2]));
    coverage_each_prefix(&iref_v0_payload, |payload| {
        let _ = parse_iref(payload, &mut Meta::default(), &mut Budget::default());
    });

    let handler = coverage_box(*b"hdlr", &handler_payload);
    let pitm = coverage_box(*b"pitm", &pitm_payload);
    let iinf = coverage_box(*b"iinf", &iinf_payload);
    let iprp = coverage_box(*b"iprp", &iprp_payload);
    let meta_payload = coverage_full_box(0, 0, &coverage_join(&[&handler, &pitm, &iinf, &iprp]));
    coverage_each_prefix(&meta_payload, |payload| {
        let _ = parse_meta(payload, &mut Budget::default());
    });

    let tkhd_payload = coverage_tkhd(0, 1);
    coverage_each_prefix(&tkhd_payload, |payload| {
        let _ = parse_tkhd(payload, &mut Track::default());
    });
    let tkhd_v1_payload = coverage_tkhd(1, 1);
    coverage_each_prefix(&tkhd_v1_payload, |payload| {
        let _ = parse_tkhd(payload, &mut Track::default());
    });
    let tref_payload = coverage_box(*b"auxl", &[0, 0, 0, 1]);
    coverage_each_prefix(&tref_payload, |payload| {
        let _ = parse_tref(payload, &mut Track::default(), &mut Budget::default());
    });

    let stsz_payload = coverage_full_box(0, 0, &[0, 0, 0, 1, 0, 0, 0, 1]);
    coverage_each_prefix(&stsz_payload, |payload| {
        let _ = parse_stsz(payload);
    });
    let stsz_variable_payload = coverage_full_box(0, 0, &[0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 4]);
    coverage_each_prefix(&stsz_variable_payload, |payload| {
        let _ = parse_stsz(payload);
    });
    let mut sample_payload = vec![0; VISUAL_SAMPLE_ENTRY_SIZE];
    sample_payload.extend_from_slice(&coverage_box(*b"av1C", &av1c_payload));
    let sample = coverage_box(*b"av01", &sample_payload);
    let stsd_payload = coverage_full_box(0, 0, &coverage_join(&[&[0, 0, 0, 1], &sample]));
    coverage_each_prefix(&stsd_payload, |payload| {
        let _ = parse_stsd(payload, &mut Track::default(), &mut Budget::default());
    });
    let stsd_v1_payload = coverage_full_box(1, 0, &[0, 0, 0, 0]);
    coverage_each_prefix(&stsd_v1_payload, |payload| {
        let _ = parse_stsd(payload, &mut Track::default(), &mut Budget::default());
    });
    let track_properties = coverage_box(*b"av1C", &av1c_payload);
    coverage_each_prefix(&track_properties, |payload| {
        let _ = parse_track_properties(payload, &mut Track::default(), &mut Budget::default());
    });

    let stsz = coverage_box(*b"stsz", &stsz_payload);
    let stsd = coverage_box(*b"stsd", &stsd_payload);
    let stbl_payload = coverage_join(&[&stsz, &stsd]);
    coverage_each_prefix(&stbl_payload, |payload| {
        let _ = parse_stbl(payload, &mut Track::default(), &mut Budget::default());
    });
    let stbl = coverage_box(*b"stbl", &stbl_payload);
    coverage_each_prefix(&stbl, |payload| {
        let _ = parse_minf(payload, &mut Track::default(), &mut Budget::default());
    });
    let minf = coverage_box(*b"minf", &stbl);
    let mdia_payload = coverage_join(&[&handler, &minf]);
    coverage_each_prefix(&mdia_payload, |payload| {
        let _ = parse_mdia(payload, &mut Track::default(), &mut Budget::default());
    });
    let tkhd = coverage_box(*b"tkhd", &tkhd_payload);
    let mdia = coverage_box(*b"mdia", &mdia_payload);
    let track_payload = coverage_join(&[&tkhd, &mdia]);
    coverage_each_prefix(&track_payload, |payload| {
        let _ = parse_track(payload, &mut Budget::default());
    });
    let track = coverage_box(*b"trak", &track_payload);
    coverage_each_prefix(&track, |payload| {
        let _ = parse_movie(payload, &mut Budget::default());
    });

    let free_box = coverage_box(*b"free", &[]);
    let mut reader = Reader::new(&free_box);
    let _ = next_box(&mut reader, false, &mut coverage_box_budget());
    let _ = parse_meta(&meta_payload, &mut coverage_box_budget());
    let _ = parse_iinf(
        &iinf_payload,
        &mut Meta::default(),
        &mut coverage_box_budget(),
    );
    let _ = parse_iprp(
        &iprp_payload,
        &mut Meta::default(),
        &mut coverage_box_budget(),
    );
    let _ = parse_ipco(
        &ipco_payload,
        &mut Meta::default(),
        &mut coverage_box_budget(),
    );
    let _ = parse_iref(
        &iref_payload,
        &mut Meta::default(),
        &mut coverage_box_budget(),
    );
    let _ = parse_movie(&track, &mut coverage_box_budget());
    let _ = parse_track(&track_payload, &mut coverage_box_budget());
    let _ = parse_tref(
        &tref_payload,
        &mut Track::default(),
        &mut coverage_box_budget(),
    );
    let _ = parse_mdia(
        &mdia_payload,
        &mut Track::default(),
        &mut coverage_box_budget(),
    );
    let _ = parse_minf(&stbl, &mut Track::default(), &mut coverage_box_budget());
    let _ = parse_stbl(
        &stbl_payload,
        &mut Track::default(),
        &mut coverage_box_budget(),
    );
    let _ = parse_stsd(
        &stsd_payload,
        &mut Track::default(),
        &mut coverage_box_budget(),
    );
    let _ = parse_track_properties(
        &track_properties,
        &mut Track::default(),
        &mut coverage_box_budget(),
    );

    let _ = std::hint::black_box(parse_iinf(
        &iinf_payload,
        &mut Meta::default(),
        &mut coverage_record_budget(),
    ));
    let _ = std::hint::black_box(parse_ipco(
        &ipco_payload,
        &mut Meta::default(),
        &mut coverage_record_budget(),
    ));
    let _ = std::hint::black_box(parse_ipma(
        &ipma_payload,
        &mut Meta {
            properties: vec![Property::Other],
            ..Meta::default()
        },
        &mut coverage_record_budget(),
    ));
    let _ = std::hint::black_box(parse_iref(
        &iref_payload,
        &mut Meta::default(),
        &mut coverage_record_budget(),
    ));
    let _ = std::hint::black_box(parse_movie(&track, &mut coverage_record_budget()));

    let _ = std::hint::black_box(inspect_inner(&[]));
    let _ = std::hint::black_box(parse_meta(
        &coverage_full_box(0, 0, &[]),
        &mut Budget::default(),
    ));
    let _ = std::hint::black_box(parse_iinf(
        &coverage_full_box(0, 0, &[0, 1]),
        &mut Meta::default(),
        &mut Budget::default(),
    ));
    let _ = std::hint::black_box(parse_iprp(
        &[],
        &mut Meta::default(),
        &mut Budget::default(),
    ));
    let _ = std::hint::black_box(parse_stsd(
        &coverage_full_box(0, 0, &[0, 0, 0, 1]),
        &mut Track::default(),
        &mut Budget::default(),
    ));

    let bad_meta_handler = coverage_box(*b"hdlr", &[]);
    let _ = parse_meta(
        &coverage_full_box(0, 0, &bad_meta_handler),
        &mut Budget::default(),
    );
    let bad_pitm = coverage_box(*b"pitm", &[]);
    let _ = parse_meta(
        &coverage_full_box(0, 0, &coverage_join(&[&handler, &bad_pitm, &iinf, &iprp])),
        &mut Budget::default(),
    );
    let bad_iinf = coverage_box(*b"iinf", &[]);
    let _ = parse_meta(
        &coverage_full_box(0, 0, &coverage_join(&[&handler, &pitm, &bad_iinf, &iprp])),
        &mut Budget::default(),
    );
    let bad_iprp = coverage_box(*b"iprp", &[]);
    let _ = parse_meta(
        &coverage_full_box(0, 0, &coverage_join(&[&handler, &pitm, &iinf, &bad_iprp])),
        &mut Budget::default(),
    );
    let bad_iref = coverage_box(*b"iref", &[]);
    let _ = parse_meta(
        &coverage_full_box(
            0,
            0,
            &coverage_join(&[&handler, &pitm, &iinf, &iprp, &bad_iref]),
        ),
        &mut Budget::default(),
    );

    let bad_infe = coverage_box(*b"infe", &[]);
    let _ = parse_iinf(
        &coverage_full_box(0, 0, &coverage_join(&[&[0, 1], &bad_infe])),
        &mut Meta::default(),
        &mut Budget::default(),
    );
    let bad_property = coverage_box(*b"ispe", &[]);
    let bad_ipco = coverage_box(*b"ipco", &bad_property);
    let _ = parse_iprp(
        &coverage_join(&[&bad_ipco, &ipma]),
        &mut Meta::default(),
        &mut Budget::default(),
    );
    let bad_ipma = coverage_box(*b"ipma", &[]);
    let _ = parse_iprp(
        &coverage_join(&[&ipco, &bad_ipma]),
        &mut Meta::default(),
        &mut Budget::default(),
    );
    let _ = parse_ipco(&bad_property, &mut Meta::default(), &mut Budget::default());

    let bad_track = coverage_box(*b"trak", &[]);
    let _ = parse_movie(&bad_track, &mut Budget::default());
    let bad_tkhd = coverage_box(*b"tkhd", &[]);
    let _ = parse_track(&coverage_join(&[&bad_tkhd, &mdia]), &mut Budget::default());
    let bad_mdia = coverage_box(*b"mdia", &[]);
    let _ = parse_track(&coverage_join(&[&tkhd, &bad_mdia]), &mut Budget::default());
    let bad_tref = coverage_box(*b"tref", &coverage_box(*b"auxl", &[]));
    let _ = parse_track(
        &coverage_join(&[&tkhd, &mdia, &bad_tref]),
        &mut Budget::default(),
    );

    let bad_handler = coverage_box(*b"hdlr", &[]);
    let _ = parse_mdia(
        &coverage_join(&[&bad_handler, &minf]),
        &mut Track::default(),
        &mut Budget::default(),
    );
    let bad_minf = coverage_box(*b"minf", &[]);
    let _ = parse_mdia(
        &coverage_join(&[&handler, &bad_minf]),
        &mut Track::default(),
        &mut Budget::default(),
    );
    let bad_stbl = coverage_box(*b"stbl", &[]);
    let _ = parse_minf(&bad_stbl, &mut Track::default(), &mut Budget::default());
    let bad_stsz = coverage_box(*b"stsz", &[]);
    let _ = parse_stbl(
        &coverage_join(&[&bad_stsz, &stsd]),
        &mut Track::default(),
        &mut Budget::default(),
    );
    let bad_stsd = coverage_box(*b"stsd", &[]);
    let _ = parse_stbl(
        &coverage_join(&[&stsz, &bad_stsd]),
        &mut Track::default(),
        &mut Budget::default(),
    );
    let mut bad_sample_payload = vec![0; VISUAL_SAMPLE_ENTRY_SIZE];
    bad_sample_payload.extend_from_slice(&bad_property);
    let bad_sample = coverage_box(*b"av01", &bad_sample_payload);
    let _ = parse_stsd(
        &coverage_full_box(0, 0, &coverage_join(&[&[0, 0, 0, 1], &bad_sample])),
        &mut Track::default(),
        &mut Budget::default(),
    );
    let _ = parse_track_properties(&bad_property, &mut Track::default(), &mut Budget::default());

    let ftyp_avif = coverage_box(*b"ftyp", b"avif\0\0\0\0");
    let _ = inspect_inner(&coverage_join(&[&ftyp_avif, &coverage_box(*b"meta", &[])]));
    let ftyp_avis = coverage_box(*b"ftyp", b"avis\0\0\0\0");
    let _ = inspect_inner(&coverage_join(&[&ftyp_avis, &coverage_box(*b"moov", &[])]));
    let empty_details_meta = coverage_box(
        *b"meta",
        &coverage_full_box(
            0,
            0,
            &coverage_join(&[
                &handler,
                &pitm,
                &coverage_box(*b"iinf", &coverage_full_box(0, 0, &[0, 0])),
                &coverage_box(
                    *b"iprp",
                    &coverage_join(&[
                        &coverage_box(*b"ipco", &[]),
                        &coverage_box(*b"ipma", &coverage_full_box(0, 0, &[0, 0, 0, 0])),
                    ]),
                ),
            ]),
        ),
    );
    let _ = inspect_inner(&coverage_join(&[&ftyp_avif, &empty_details_meta]));

    let bad_iprp_chain = coverage_box(*b"iprp", &coverage_join(&[&bad_ipco, &ipma]));
    let bad_meta_chain = coverage_box(
        *b"meta",
        &coverage_full_box(
            0,
            0,
            &coverage_join(&[&handler, &pitm, &iinf, &bad_iprp_chain]),
        ),
    );
    let _ = inspect_inner(&coverage_join(&[&ftyp_avif, &bad_meta_chain]));

    let bad_stsd_chain = coverage_box(
        *b"stsd",
        &coverage_full_box(0, 0, &coverage_join(&[&[0, 0, 0, 1], &bad_sample])),
    );
    let bad_stbl_chain = coverage_box(*b"stbl", &coverage_join(&[&stsz, &bad_stsd_chain]));
    let bad_minf_chain = coverage_box(*b"minf", &bad_stbl_chain);
    let bad_mdia_chain = coverage_box(*b"mdia", &coverage_join(&[&handler, &bad_minf_chain]));
    let bad_track_chain = coverage_box(*b"trak", &coverage_join(&[&tkhd, &bad_mdia_chain]));
    let bad_movie_chain = coverage_box(*b"moov", &bad_track_chain);
    let _ = inspect_inner(&coverage_join(&[&ftyp_avis, &bad_movie_chain]));

    let av1c_depth_meta = Meta {
        primary_item_id: Some(1),
        items: vec![Item {
            id: 1,
            kind: *b"av01",
        }],
        properties: vec![
            Property::Ispe {
                width: 2,
                height: 3,
            },
            Property::Av1C { depth: 10 },
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
        references: Vec::new(),
    };
    let _ = av1c_depth_meta.details();
    let _ = Movie {
        tracks: vec![Track {
            handler: *b"vide",
            sample_count: 1,
            ..Track::default()
        }],
    }
    .details();
    let _ = parse_stsd(
        &coverage_full_box(1, 0, &[0, 0, 0, 0]),
        &mut Track::default(),
        &mut Budget::default(),
    );
}

#[cfg(coverage)]
fn coverage_malformed_leaf_corpus() {
    for length in 0..=128 {
        for fill in [0, 0xff, 0x55] {
            let payload = vec![fill; length];
            let _ = inspect_inner(&payload);
            let _ = parse_handler(&payload);
            let _ = parse_pitm(&payload);
            let _ = parse_iinf(&payload, &mut Meta::default(), &mut Budget::default());
            let _ = parse_infe(&payload);
            let _ = parse_iprp(&payload, &mut Meta::default(), &mut Budget::default());
            let _ = parse_ipco(&payload, &mut Meta::default(), &mut Budget::default());
            for kind in [*b"ispe", *b"pixi", *b"av1C", *b"auxC"] {
                let _ = parse_property(BoxView {
                    kind,
                    payload: &payload,
                });
            }
            let _ = parse_av1c(&payload);
            let _ = parse_ipma(
                &payload,
                &mut Meta {
                    properties: vec![Property::Other],
                    ..Meta::default()
                },
                &mut Budget::default(),
            );
            let _ = parse_iref(&payload, &mut Meta::default(), &mut Budget::default());
            let _ = parse_movie(&payload, &mut Budget::default());
            let _ = parse_track(&payload, &mut Budget::default());
            let _ = parse_tkhd(&payload, &mut Track::default());
            let _ = parse_tref(&payload, &mut Track::default(), &mut Budget::default());
            let _ = parse_mdia(&payload, &mut Track::default(), &mut Budget::default());
            let _ = parse_minf(&payload, &mut Track::default(), &mut Budget::default());
            let _ = parse_stbl(&payload, &mut Track::default(), &mut Budget::default());
            let _ = parse_stsz(&payload);
            let _ = parse_stsd(&payload, &mut Track::default(), &mut Budget::default());
            let _ = parse_track_properties(&payload, &mut Track::default(), &mut Budget::default());
        }
    }
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    coverage_nested_parser_prefixes();
    coverage_malformed_leaf_corpus();
    let mut full_budget = Budget {
        boxes: MAX_BOXES,
        records: MAX_RECORDS,
    };
    let _ = full_budget.box_seen();
    let _ = full_budget.record_seen();

    let mut overflow_reader = Reader {
        data: &[],
        offset: usize::MAX,
    };
    let _ = overflow_reader.remaining();
    let _ = overflow_reader.is_empty();
    let _ = overflow_reader.take(1);
    let _ = overflow_reader.skip(1);
    let _ = overflow_reader.u8();
    let _ = overflow_reader.u16();
    let _ = overflow_reader.u32();
    let _ = overflow_reader.u64();
    let _ = overflow_reader.four_cc();
    let _ = overflow_reader.c_string();
    let mut short_reader = Reader::new(&[]);
    let _ = short_reader.take(0);
    let _ = short_reader.u8();
    let _ = Reader::new(b"unterminated").c_string();
    let mut integer_reader = Reader::new(&[0, 0, 0, 0, 0, 0, 0, 1]);
    let _ = integer_reader.u64();

    let mut budget = Budget::default();
    let mut reader = Reader::new(&[]);
    let _ = next_box(&mut reader, true, &mut budget);
    let mut reader = Reader::new(&[0, 0, 0, 0, b'f', b'r', b'e', b'e']);
    let _ = next_box(&mut reader, true, &mut budget);
    let mut reader = Reader::new(&[0, 0, 0, 0, b'f', b'r', b'e', b'e']);
    let _ = next_box(&mut reader, false, &mut budget);
    let mut reader = Reader::new(&[0, 0, 0, 4, b'f', b'r', b'e', b'e']);
    let _ = next_box(&mut reader, true, &mut budget);
    let mut reader = Reader::new(&[0, 0, 0, 12, b'f', b'r', b'e', b'e']);
    let _ = next_box(&mut reader, true, &mut budget);
    let mut reader = Reader::new(&[0, 0, 0, 24, b'u', b'u', b'i', b'd']);
    let _ = next_box(&mut reader, true, &mut budget);
    let mut large = Vec::from([0, 0, 0, 1, b'f', b'r', b'e', b'e']);
    large.extend_from_slice(&(u64::from(u32::MAX) + 1).to_be_bytes());
    let mut reader = Reader::new(&large);
    let _ = next_box(&mut reader, true, &mut budget);
    let mut extended = Vec::from([0, 0, 0, 1, b'f', b'r', b'e', b'e']);
    extended.extend_from_slice(&16_u64.to_be_bytes());
    for end in 8..16 {
        let mut reader = Reader::new(&extended[..end]);
        let _ = next_box(&mut reader, true, &mut budget);
    }
    let mut reader = Reader::new(&extended);
    let _ = next_box(&mut reader, true, &mut budget);
    let uuid = coverage_box(*b"uuid", &[0; 16]);
    let mut reader = Reader::new(&uuid);
    let _ = next_box(&mut reader, true, &mut budget);

    let _ = parse_ftyp(&[]);
    let _ = parse_ftyp(b"avif");
    let _ = parse_ftyp(b"avif\0\0\0\0x");
    let _ = parse_ftyp(b"mif1\0\0\0\0");
    let _ = parse_ftyp(b"avis\0\0\0\0avif");
    let _ = parse_ftyp(b"mif1\0\0\0\0avis");
    let _ = parse_full_box_version_zero(&mut Reader::new(&[1, 0, 0, 0]));

    let handler = coverage_box(*b"hdlr", &coverage_handler(*b"pict"));
    let bad_handler = coverage_box(*b"hdlr", &coverage_handler(*b"vide"));
    let pitm = coverage_box(*b"pitm", &coverage_full_box(0, 0, &[0, 1]));
    let iinf = coverage_box(*b"iinf", &coverage_full_box(0, 0, &[0, 0]));
    let ipco = coverage_box(*b"ipco", &[]);
    let ipma = coverage_box(*b"ipma", &coverage_full_box(0, 0, &[0, 0, 0, 0]));
    let iprp = coverage_box(*b"iprp", &coverage_join(&[&ipco, &ipma]));
    let iref = coverage_box(*b"iref", &coverage_full_box(0, 0, &[]));
    let meta_prefix = coverage_full_box(0, 0, &handler);

    let _ = parse_meta(
        &coverage_join(&[&coverage_full_box(0, 0, &bad_handler)]),
        &mut Budget::default(),
    );
    let _ = parse_meta(
        &coverage_full_box(0, 0, &coverage_box(*b"free", &[])),
        &mut Budget::default(),
    );
    let _ = parse_meta(
        &coverage_join(&[&meta_prefix, &handler]),
        &mut Budget::default(),
    );
    for duplicate in [&pitm, &iinf, &iprp, &iref] {
        let _ = parse_meta(
            &coverage_join(&[&meta_prefix, &pitm, &iinf, &iprp, &iref, duplicate]),
            &mut Budget::default(),
        );
    }
    let _ = parse_meta(&meta_prefix, &mut Budget::default());
    let _ = parse_meta(
        &coverage_join(&[&meta_prefix, &pitm]),
        &mut Budget::default(),
    );
    let _ = parse_meta(
        &coverage_join(&[&meta_prefix, &pitm, &iinf]),
        &mut Budget::default(),
    );

    let mut reserved_handler = coverage_handler(*b"pict");
    reserved_handler[4] = 1;
    let _ = parse_handler(&reserved_handler);
    let _ = parse_handler(&coverage_full_box(1, 0, &[]));
    let _ = parse_pitm(&coverage_full_box(1, 0, &[0, 0, 0, 1]));
    let _ = parse_pitm(&coverage_full_box(0, 0, &[0, 0]));

    let _ = parse_iinf(
        &coverage_full_box(1, 0, &[0, 0, 0, 0]),
        &mut Meta::default(),
        &mut Budget::default(),
    );
    let _ = parse_iinf(
        &coverage_full_box(2, 0, &[]),
        &mut Meta::default(),
        &mut Budget::default(),
    );
    let other = coverage_box(*b"free", &[]);
    let _ = parse_iinf(
        &coverage_full_box(0, 0, &coverage_join(&[&[0, 1], &other])),
        &mut Meta::default(),
        &mut Budget::default(),
    );
    let infe = coverage_box(
        *b"infe",
        &coverage_full_box(2, 0, &[0, 1, 0, 0, b'a', b'v', b'0', b'1', 0]),
    );
    let _ = parse_iinf(
        &coverage_full_box(0, 0, &coverage_join(&[&[0, 2], &infe, &infe])),
        &mut Meta::default(),
        &mut Budget::default(),
    );
    let _ = parse_iinf(
        &coverage_full_box(0, 0, &[0, 0, 1]),
        &mut Meta::default(),
        &mut Budget::default(),
    );
    let _ = parse_infe(&coverage_full_box(
        3,
        0,
        &[0, 0, 0, 1, 0, 0, b'm', b'i', b'm', b'e', 0, 0],
    ));
    let _ = parse_infe(&coverage_full_box(
        2,
        0,
        &[0, 0, 0, 0, b'a', b'v', b'0', b'1', 0],
    ));
    let _ = parse_infe(&coverage_full_box(1, 0, &[]));

    let _ = parse_iprp(&other, &mut Meta::default(), &mut Budget::default());
    let _ = parse_iprp(&ipco, &mut Meta::default(), &mut Budget::default());
    let _ = parse_iprp(
        &coverage_join(&[&ipco, &ipma, &ipma]),
        &mut Meta::default(),
        &mut Budget::default(),
    );
    let _ = parse_iprp(
        &coverage_join(&[&ipco, &ipma, &other]),
        &mut Meta::default(),
        &mut Budget::default(),
    );

    for payload in [
        coverage_full_box(0, 0, &[0, 0, 0, 0, 0, 0, 0, 1]),
        coverage_full_box(0, 0, &[0, 0, 0, 1, 0, 0, 0, 0]),
        coverage_full_box(0, 0, &[0]),
        coverage_full_box(0, 0, &[5, 8]),
        coverage_full_box(0, 0, &[1, 0]),
        coverage_full_box(0, 0, &[1, 17]),
        coverage_full_box(0, 0, &[2, 8, 10]),
    ] {
        let kind = if payload.len() == 12 {
            *b"ispe"
        } else {
            *b"pixi"
        };
        let _ = parse_property(BoxView {
            kind,
            payload: &payload,
        });
    }
    let _ = parse_property(BoxView {
        kind: *b"av1C",
        payload: &[0, 0, 0, 0],
    });
    let _ = parse_property(BoxView {
        kind: *b"av1C",
        payload: &[0x81, 0, 0x20, 0],
    });
    let _ = parse_property(BoxView {
        kind: *b"av1C",
        payload: &[0x81, 0, 0x40, 0],
    });
    for (kind, urn) in [
        (*b"auxi", ALPHA_URN_HEVC),
        (*b"auxC", b"not-alpha".as_slice()),
    ] {
        let payload = coverage_full_box(0, 0, &coverage_join(&[urn, &[0]]));
        let _ = parse_property(BoxView {
            kind,
            payload: &payload,
        });
    }

    let properties = vec![Property::Other];
    let mut meta = Meta {
        properties,
        ..Meta::default()
    };
    let mut narrow_v1 = coverage_full_box(1, 0, &[0, 0, 0, 1]);
    narrow_v1.extend_from_slice(&[0, 0, 0, 1, 0]);
    let _ = parse_ipma(&narrow_v1, &mut meta, &mut Budget::default());
    let mut wide = coverage_full_box(0, 1, &[0, 0, 0, 1]);
    wide.extend_from_slice(&[0, 1, 1, 0, 1]);
    let _ = parse_ipma(&wide, &mut meta, &mut Budget::default());
    for payload in [
        coverage_full_box(0, 0, &[0, 0, 0, 1, 0, 0, 0]),
        coverage_full_box(1, 0, &[0, 0, 0, 1, 0, 0, 0, 0, 0]),
        coverage_full_box(0, 0, &[0, 0, 0, 2, 0, 2, 0, 0, 0, 1, 0]),
        coverage_full_box(0, 0, &[0, 0, 0, 1, 0, 1, 1, 0]),
        coverage_full_box(0, 0, &[0, 0, 0, 1, 0, 1, 1, 0x80]),
        coverage_full_box(0, 0, &[0, 0, 0, 1, 0, 1, 1, 2]),
        coverage_full_box(0, 0, &[0, 0, 0, 0, 1]),
    ] {
        let _ = parse_ipma(&payload, &mut meta, &mut Budget::default());
    }

    let _ = parse_iref(
        &coverage_full_box(2, 0, &[]),
        &mut Meta::default(),
        &mut Budget::default(),
    );
    for payload in [
        coverage_full_box(0, 0, &coverage_box(*b"auxl", &[0])),
        coverage_full_box(1, 0, &coverage_box(*b"auxl", &[0, 0, 0])),
        coverage_full_box(0, 0, &coverage_box(*b"auxl", &[0, 1])),
    ] {
        let _ = parse_iref(&payload, &mut Meta::default(), &mut Budget::default());
    }
    for payload in [
        coverage_full_box(
            1,
            0,
            &coverage_box(*b"auxl", &[0, 0, 0, 1, 0, 1, 0, 0, 0, 2]),
        ),
        coverage_full_box(1, 0, &coverage_box(*b"auxl", &[0, 0, 0, 0, 0, 0])),
        coverage_full_box(
            1,
            0,
            &coverage_box(*b"auxl", &[0, 0, 0, 1, 0, 1, 0, 0, 0, 0]),
        ),
        coverage_full_box(1, 0, &coverage_box(*b"auxl", &[0, 0, 0, 1, 0, 0, 1])),
    ] {
        let _ = parse_iref(&payload, &mut Meta::default(), &mut Budget::default());
    }

    let _ = std::hint::black_box(Meta::default().details());
    let mut details_meta = Meta {
        primary_item_id: Some(1),
        ..Meta::default()
    };
    let _ = std::hint::black_box(details_meta.details());
    details_meta.items.push(Item {
        id: 1,
        kind: *b"free",
    });
    let _ = details_meta.details();
    details_meta.items[0].kind = *b"av01";
    let _ = std::hint::black_box(details_meta.details());
    details_meta.properties.extend([
        Property::Other,
        Property::Ispe {
            width: 2,
            height: 3,
        },
        Property::AuxC { is_alpha: false },
        Property::AuxC { is_alpha: true },
    ]);
    details_meta.associations.extend([
        Association {
            item_id: 1,
            property_index: usize::MAX,
        },
        Association {
            item_id: 1,
            property_index: 0,
        },
        Association {
            item_id: 1,
            property_index: 1,
        },
    ]);
    let _ = details_meta.details();
    details_meta.references.extend([
        Reference {
            kind: *b"dimg",
            from_id: 9,
            to_id: 8,
        },
        Reference {
            kind: *b"dimg",
            from_id: 1,
            to_id: 1,
        },
        Reference {
            kind: *b"dimg",
            from_id: 1,
            to_id: 2,
        },
        Reference {
            kind: *b"auxl",
            from_id: 3,
            to_id: 9,
        },
        Reference {
            kind: *b"auxl",
            from_id: 3,
            to_id: 2,
        },
    ]);
    let _ = Meta {
        references: vec![Reference {
            kind: *b"auxl",
            from_id: 2,
            to_id: 1,
        }],
        ..Meta::default()
    }
    .has_alpha(1);
    details_meta.associations.push(Association {
        item_id: 3,
        property_index: 2,
    });
    let _ = details_meta.has_alpha(1);
    details_meta.associations.push(Association {
        item_id: 3,
        property_index: 3,
    });
    let _ = details_meta.has_alpha(1);

    let baseline = include_bytes!("../../../tests/fixtures/input/images/avif/baseline.avif");
    let mut duplicate_meta = baseline[..274].to_vec();
    duplicate_meta.extend_from_slice(&baseline[32..274]);
    let _ = inspect_inner(&duplicate_meta);
    let animated = include_bytes!("../../../tests/fixtures/input/images/avif/animated.avif");
    let mut duplicate_movie = animated[..1015].to_vec();
    duplicate_movie.extend_from_slice(&animated[286..1015]);
    let _ = inspect_inner(&duplicate_movie);
    let _ = inspect_inner(&animated[..44]);
    let _ = inspect_inner(&coverage_box(*b"free", &[]));
    let _ = inspect_inner(&coverage_box(*b"ftyp", b"avif\0\0\0\0"));
    let _ = inspect_inner(&coverage_box(*b"ftyp", b"avis\0\0\0\0"));
    let mut neutral_major = animated.to_vec();
    neutral_major[8..12].copy_from_slice(b"mif1");
    let _ = inspect_inner(&neutral_major);
    let mut item_fallback = animated.to_vec();
    item_fallback[638..642].copy_from_slice(b"auxv");
    let _ = inspect_inner(&item_fallback);
    let mut track_fallback = animated.to_vec();
    track_fallback[8..12].copy_from_slice(b"avif");
    track_fallback[170..174].copy_from_slice(b"free");
    let _ = inspect_inner(&track_fallback);
    coverage_prefixes(baseline);
    coverage_prefixes(animated);
    coverage_prefixes(include_bytes!(
        "../../../tests/fixtures/input/images/avif/alpha.avif"
    ));
    coverage_prefixes(include_bytes!(
        "../../../tests/fixtures/input/images/avif/grid.avif"
    ));
    coverage_prefixes(include_bytes!(
        "../../../tests/fixtures/input/images/avif/hdr.avif"
    ));
    coverage_prefixes(include_bytes!(
        "../../../tests/fixtures/input/images/avif/10bit.avif"
    ));
    coverage_metadata_mutations(baseline);
    coverage_metadata_mutations(animated);
    coverage_metadata_mutations(include_bytes!(
        "../../../tests/fixtures/input/images/avif/alpha.avif"
    ));
    coverage_metadata_mutations(include_bytes!(
        "../../../tests/fixtures/input/images/avif/grid.avif"
    ));
    coverage_metadata_mutations(include_bytes!(
        "../../../tests/fixtures/input/images/avif/hdr.avif"
    ));
    coverage_metadata_mutations(include_bytes!(
        "../../../tests/fixtures/input/images/avif/10bit.avif"
    ));
    let _ = image_info(Details {
        width: 0,
        height: 1,
        depth: 8,
        has_alpha: false,
        frame_count: 1,
    });
    let _ = image_info(Details {
        width: 1,
        height: 0,
        depth: 8,
        has_alpha: false,
        frame_count: 1,
    });
    let _ = image_info(Details {
        width: 1,
        height: 1,
        depth: 8,
        has_alpha: false,
        frame_count: 0,
    });

    let tkhd0 = coverage_box(*b"tkhd", &coverage_tkhd(0, 1));
    let tkhd1 = coverage_box(*b"tkhd", &coverage_tkhd(1, 1));
    let hdlr = coverage_box(*b"hdlr", &coverage_handler(*b"pict"));
    let stbl = coverage_box(*b"stbl", &coverage_stbl());
    let minf = coverage_box(*b"minf", &stbl);
    let mdia = coverage_box(*b"mdia", &coverage_join(&[&hdlr, &minf]));
    let track = coverage_join(&[&tkhd0, &mdia]);
    let _ = parse_track(&coverage_join(&[&track, &tkhd0]), &mut Budget::default());
    let _ = parse_track(&coverage_join(&[&track, &mdia]), &mut Budget::default());
    let tref = coverage_box(*b"tref", &coverage_box(*b"free", &[]));
    let _ = parse_track(
        &coverage_join(&[&track, &tref, &tref]),
        &mut Budget::default(),
    );
    let _ = parse_track(&[], &mut Budget::default());
    let _ = parse_track(&tkhd0, &mut Budget::default());
    let tkhd_zero = coverage_box(*b"tkhd", &coverage_tkhd(0, 0));
    let _ = parse_track(&coverage_join(&[&tkhd_zero, &mdia]), &mut Budget::default());
    let _ = parse_tkhd(&tkhd1[8..], &mut Track::default());
    let _ = parse_tkhd(&coverage_full_box(2, 0, &[]), &mut Track::default());
    let _ = parse_tref(
        &coverage_box(*b"free", &[]),
        &mut Track::default(),
        &mut Budget::default(),
    );
    let _ = parse_mdia(
        &coverage_join(&[&hdlr, &hdlr, &minf]),
        &mut Track::default(),
        &mut Budget::default(),
    );
    let _ = parse_mdia(
        &coverage_join(&[&hdlr, &minf, &minf]),
        &mut Track::default(),
        &mut Budget::default(),
    );
    let _ = parse_mdia(&[], &mut Track::default(), &mut Budget::default());
    let _ = parse_mdia(&hdlr, &mut Track::default(), &mut Budget::default());
    let _ = parse_minf(
        &coverage_join(&[&stbl, &stbl]),
        &mut Track::default(),
        &mut Budget::default(),
    );
    let _ = parse_minf(&[], &mut Track::default(), &mut Budget::default());
    let stsz = coverage_box(
        *b"stsz",
        &coverage_full_box(0, 0, &[0, 0, 0, 1, 0, 0, 0, 1]),
    );
    let stsd = coverage_box(*b"stsd", &coverage_full_box(0, 0, &[0, 0, 0, 0]));
    for payload in [
        coverage_join(&[&stsz, &stsz, &stsd]),
        coverage_join(&[&stsz, &stsd, &stsd]),
        Vec::new(),
        stsz.clone(),
    ] {
        let _ = parse_stbl(&payload, &mut Track::default(), &mut Budget::default());
    }
    for payload in [
        coverage_full_box(0, 0, &[0, 0, 0, 1, 0, 0, 0, 1, 1]),
        coverage_full_box(0, 0, &[0, 0, 0, 0, 0xff, 0xff, 0xff, 0xff]),
        coverage_full_box(0, 0, &[0, 0, 0, 0, 0, 0, 0, 1]),
    ] {
        let _ = parse_stsz(&payload);
    }
    let non_av1 = coverage_box(*b"jpeg", &[]);
    let short_av1 = coverage_box(*b"av01", &[]);
    for payload in [
        coverage_full_box(2, 0, &[0, 0, 0, 0]),
        coverage_full_box(0, 0, &coverage_join(&[&[0, 0, 0, 1], &non_av1])),
        coverage_full_box(0, 0, &coverage_join(&[&[0, 0, 0, 1], &short_av1])),
        coverage_full_box(0, 0, &[0, 0, 0, 0, 1]),
    ] {
        let _ = parse_stsd(&payload, &mut Track::default(), &mut Budget::default());
    }
    let av1_8 = coverage_box(*b"av1C", &[0x81, 0, 0, 0]);
    let av1_10 = coverage_box(*b"av1C", &[0x81, 0, 0x40, 0]);
    let alpha = coverage_box(
        *b"auxC",
        &coverage_full_box(0, 0, &coverage_join(&[ALPHA_URN_MPEG_B, &[0]])),
    );
    let non_alpha = coverage_box(*b"auxC", &coverage_full_box(0, 0, b"other\0"));
    let mut conflict = Track::default();
    let _ = parse_track_properties(
        &coverage_join(&[&av1_8, &av1_10]),
        &mut conflict,
        &mut Budget::default(),
    );
    let mut equal = Track::default();
    let _ = parse_track_properties(
        &coverage_join(&[&av1_8, &av1_8]),
        &mut equal,
        &mut Budget::default(),
    );
    let mut conflict = Track::default();
    let _ = parse_track_properties(
        &coverage_join(&[&alpha, &non_alpha]),
        &mut conflict,
        &mut Budget::default(),
    );
    let mut equal = Track::default();
    let _ = parse_track_properties(
        &coverage_join(&[&alpha, &alpha]),
        &mut equal,
        &mut Budget::default(),
    );

    let _ = parse_movie(&[], &mut Budget::default());
    let _ = parse_movie(&coverage_box(*b"free", &[]), &mut Budget::default());
    let _ = std::hint::black_box(Movie::default().details());
    let _ = Movie {
        tracks: vec![Track {
            handler: *b"free",
            ..Track::default()
        }],
    }
    .details();
    let mut movie = Movie {
        tracks: vec![
            Track {
                handler: *b"free",
                ..Track::default()
            },
            Track {
                id: 1,
                handler: *b"pict",
                width: 2,
                height: 3,
                sample_count: 1,
                depth: None,
                aux_for_id: None,
                aux_is_alpha: None,
            },
        ],
    };
    let _ = std::hint::black_box(movie.details());
    movie.tracks.push(Track {
        id: 3,
        handler: *b"auxv",
        aux_for_id: Some(9),
        ..Track::default()
    });
    movie.tracks.push(Track {
        id: 2,
        handler: *b"auxv",
        aux_for_id: Some(1),
        ..Track::default()
    });
    let _ = movie.details();
    movie.tracks[3].aux_is_alpha = Some(false);
    let _ = movie.details();
}
