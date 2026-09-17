//! Still-image muxing, following pinned libavif 1.4.1 `src/write.c`.
//!
//! The compressor owns sample/header agreement, including dimensions and the
//! omission of opaque alpha. This module validates the prepared descriptor and
//! writes its container; it does not reinterpret or repair compressed samples.
//! All optional values represent absent metadata, alpha, transforms or a
//! sizing-only output stream. A failed substring lookup means no reuse exists.

use crate::codecs::error::check_cancelled;
use crate::codecs::{CodecError, CodecResult};
use crate::{CancellationToken, CodecOperation, EncodePolicy, ImageFormat};

const ALPHA_URN: &[u8] = b"urn:mpeg:mpegB:cicp:systems:auxiliary:alpha\0";
const CHUNK: usize = 1024;

#[derive(Clone, Copy)]
pub(super) struct Sample<'a> {
    pub bytes: &'a [u8],
    pub configuration: [u8; 4],
}

#[derive(Clone, Copy)]
pub(super) struct Metadata<'a> {
    pub dimensions: [u32; 2],
    pub cicp: [u16; 3],
    pub full_range: bool,
    pub premultiplied: bool,
    pub icc: &'a [u8],
    /// Complete prepared EXIF item payload, including its four-byte TIFF offset.
    pub exif: &'a [u8],
    pub xmp: &'a [u8],
    pub rotation: Option<u8>,
    pub mirror: Option<u8>,
}

pub(super) struct StillImage<'a> {
    pub color: Sample<'a>,
    pub alpha: Option<Sample<'a>>,
    pub metadata: Metadata<'a>,
}

fn invalid(message: &str) -> CodecError {
    CodecError::Parameter(message.to_owned())
}

fn size_error() -> CodecError {
    CodecError::Unsupported("AVIF still container exceeds 32-bit box or item offsets".to_owned())
}

fn add(a: usize, b: usize) -> CodecResult<usize> {
    a.checked_add(b).ok_or_else(size_error)
}

fn narrow(value: usize) -> CodecResult<u32> {
    checked_length(u64::try_from(value).map_err(|_| size_error())?)
}

pub(super) fn checked_length(value: u64) -> CodecResult<u32> {
    u32::try_from(value).map_err(|_| size_error())
}

fn sample_depth(sample: Sample<'_>) -> CodecResult<u8> {
    let [marker, profile_level, flags, delay] = sample.configuration;
    let profile = profile_level >> 5;
    let level = profile_level & 31;
    let high = flags & 64 != 0;
    let twelve = flags & 32 != 0;
    let mono = flags & 16 != 0;
    let x = flags & 8 != 0;
    let y = flags & 4 != 0;
    let position = flags & 3;
    // The native still writer emits version 1 and no initial presentation delay.
    // Profile 0 is 420, profile 1 is nonmonochrome 444, profile 2 at 8/10
    // bits is 422. Monochrome signals both subsampling flags and no position.
    if sample.bytes.is_empty()
        || marker != 0x81
        || profile > 2
        || (24..31).contains(&level)
        || level <= 7 && flags & 128 != 0
        || delay != 0
        || twelve && (!high || profile != 2)
        || y && !x
        || position == 3
        || position != 0 && (!x || !y || mono)
        || mono && (!x || !y || profile == 1)
        || !mono
            && match profile {
                0 => !x || !y,
                1 => x || y,
                _ => !twelve && (!x || y),
            }
    {
        return Err(invalid("invalid prepared AVIF sample configuration"));
    }
    narrow(sample.bytes.len())?;
    Ok(if twelve {
        12
    } else if high {
        10
    } else {
        8
    })
}

impl StillImage<'_> {
    fn validate(&self) -> CodecResult<u8> {
        let depth = sample_depth(self.color)?;
        let metadata = self.metadata;
        if metadata.dimensions.contains(&0) {
            return Err(CodecError::Dimensions(
                "zero AVIF still dimensions".to_owned(),
            ));
        }
        if metadata.rotation.is_some_and(|angle| angle > 3)
            || metadata.mirror.is_some_and(|axis| axis > 1)
        {
            return Err(invalid("invalid prepared AVIF transform"));
        }
        if let Some(alpha) = self.alpha
            && (sample_depth(alpha)? != depth || alpha.configuration[2] & 16 == 0)
        {
            return Err(invalid(
                "prepared AVIF alpha depth or channel count disagrees",
            ));
        }
        for bytes in [metadata.icc, metadata.exif, metadata.xmp] {
            narrow(bytes.len())?;
        }
        if !metadata.exif.is_empty() {
            let offset_bytes: [u8; 4] = metadata
                .exif
                .get(..4)
                .ok_or_else(|| invalid("missing prepared EXIF offset"))?
                .try_into()
                .map_err(|_| invalid("invalid prepared EXIF offset"))?;
            let offset = usize::try_from(u32::from_be_bytes(offset_bytes))
                .map_err(|_| invalid("prepared EXIF offset exceeds the payload"))?;
            let tiff_header = metadata
                .exif
                .get(4..)
                .and_then(|payload| payload.get(offset..))
                .and_then(|tail| tail.get(..4));
            if !matches!(tiff_header, Some(b"II\x2a\0" | b"MM\0\x2a")) {
                return Err(invalid(
                    "prepared EXIF offset does not identify a TIFF header",
                ));
            }
        }
        Ok(depth)
    }
}

/// A counting pass and an emitting pass use exactly the same box traversal.
/// No output allocation occurs until the complete deduplicated length passes
/// the caller's policy. The allocation covers the result, not prepared inputs.
struct Stream<'a> {
    bytes: Option<Vec<u8>>,
    position: usize,
    token: Option<&'a CancellationToken>,
}

impl Stream<'_> {
    fn write(&mut self, bytes: &[u8]) -> CodecResult<()> {
        self.position = add(self.position, bytes.len())?;
        narrow(self.position)?;
        check_cancelled(self.token)?;
        if let Some(output) = &mut self.bytes {
            if self.position > output.capacity() {
                return Err(invalid("AVIF emission exceeds its checked sizing pass"));
            }
            for chunk in bytes.chunks(CHUNK) {
                check_cancelled(self.token)?;
                output.extend_from_slice(chunk);
            }
        }
        Ok(())
    }

    fn u16(&mut self, value: u16) -> CodecResult<()> {
        self.write(&value.to_be_bytes())
    }
    fn u32(&mut self, value: u32) -> CodecResult<()> {
        self.write(&value.to_be_bytes())
    }

    fn start(&mut self, kind: &[u8; 4]) -> CodecResult<usize> {
        let marker = self.position;
        self.u32(0)?;
        self.write(kind)?;
        Ok(marker)
    }

    fn full(&mut self, kind: &[u8; 4], version: u8) -> CodecResult<usize> {
        let marker = self.start(kind)?;
        self.write(&[version, 0, 0, 0])?;
        Ok(marker)
    }

    fn finish(&mut self, marker: usize) -> CodecResult<()> {
        let size = self.position.checked_sub(marker).ok_or_else(size_error)?;
        let size = narrow(size)?;
        if let Some(output) = &mut self.bytes {
            output
                .get_mut(marker..add(marker, 4)?)
                .ok_or_else(|| invalid("invalid AVIF box marker"))?
                .copy_from_slice(&size.to_be_bytes());
        }
        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Property<'a> {
    Spatial([u32; 2]),
    Pixels(u8, u8),
    Configuration([u8; 4]),
    Icc(&'a [u8]),
    Color([u16; 3], bool),
    Alpha,
    Rotation(u8),
    Mirror(u8),
}

impl Property<'_> {
    fn essential(self) -> bool {
        matches!(
            self,
            Self::Configuration(_) | Self::Rotation(_) | Self::Mirror(_)
        )
    }

    fn emit(self, s: &mut Stream<'_>) -> CodecResult<()> {
        let marker = match self {
            Self::Spatial([width, height]) => {
                let marker = s.full(b"ispe", 0)?;
                s.u32(width)?;
                s.u32(height)?;
                marker
            }
            Self::Pixels(depth, channels) => {
                let marker = s.full(b"pixi", 0)?;
                s.write(&[channels])?;
                for _ in 0..channels {
                    s.write(&[depth])?;
                }
                marker
            }
            Self::Configuration(config) => {
                let marker = s.start(b"av1C")?;
                s.write(&config)?;
                marker
            }
            Self::Icc(bytes) => {
                let marker = s.start(b"colr")?;
                s.write(b"prof")?;
                s.write(bytes)?;
                marker
            }
            Self::Color(cicp, full) => {
                let marker = s.start(b"colr")?;
                s.write(b"nclx")?;
                for value in cicp {
                    s.u16(value)?;
                }
                s.write(&[if full { 128 } else { 0 }])?;
                marker
            }
            Self::Alpha => {
                let marker = s.full(b"auxC", 0)?;
                s.write(ALPHA_URN)?;
                marker
            }
            Self::Rotation(angle) => {
                let marker = s.start(b"irot")?;
                s.write(&[angle])?;
                marker
            }
            Self::Mirror(axis) => {
                let marker = s.start(b"imir")?;
                s.write(&[axis])?;
                marker
            }
        };
        s.finish(marker)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Color,
    Alpha,
    Exif,
    Xmp,
}

#[derive(Clone, Copy)]
struct Item<'a> {
    kind: Kind,
    payload: &'a [u8],
    offset: usize,
    associations: [u8; 8],
    association_count: usize,
}

impl<'a> Item<'a> {
    const fn new(kind: Kind, payload: &'a [u8]) -> Self {
        Self {
            kind,
            payload,
            offset: 0,
            associations: [0; 8],
            association_count: 0,
        }
    }

    fn id(index: usize) -> CodecResult<u16> {
        u16::try_from(add(index, 1)?).map_err(|_| size_error())
    }
}

struct Plan<'a> {
    items: [Item<'a>; 4],
    item_count: usize,
    properties: [Option<Property<'a>>; 16],
    property_count: usize,
    chunks: [&'a [u8]; 4],
    chunk_count: usize,
}

impl<'a> Plan<'a> {
    fn property(&mut self, item: usize, value: Property<'a>) -> CodecResult<()> {
        // Only color can supply ICC, so equality never repeatedly scans ICC.
        let existing = self.properties[..self.property_count]
            .iter()
            .position(|p| *p == Some(value));
        let index = if let Some(index) = existing {
            index
        } else {
            let index = self.property_count;
            *self
                .properties
                .get_mut(index)
                .ok_or_else(|| invalid("too many AVIF properties"))? = Some(value);
            self.property_count = add(index, 1)?;
            index
        };
        let item = self
            .items
            .get_mut(item)
            .ok_or_else(|| invalid("invalid AVIF property item"))?;
        let slot = item
            .associations
            .get_mut(item.association_count)
            .ok_or_else(|| invalid("too many AVIF property associations"))?;
        *slot = u8::try_from(add(index, 1)?).map_err(|_| size_error())?
            | if value.essential() { 128 } else { 0 };
        item.association_count = add(item.association_count, 1)?;
        Ok(())
    }

    fn append_item(&mut self, kind: Kind, payload: &'a [u8]) -> CodecResult<()> {
        *self
            .items
            .get_mut(self.item_count)
            .ok_or_else(|| invalid("too many AVIF items"))? = Item::new(kind, payload);
        self.item_count = add(self.item_count, 1)?;
        Ok(())
    }

    fn new(
        image: &StillImage<'a>,
        depth: u8,
        token: Option<&CancellationToken>,
    ) -> CodecResult<Self> {
        let mut plan = Self {
            items: [Item::new(Kind::Color, &[]); 4],
            item_count: 0,
            properties: [None; 16],
            property_count: 0,
            chunks: [&[]; 4],
            chunk_count: 0,
        };
        plan.append_item(Kind::Color, image.color.bytes)?;
        if let Some(alpha) = image.alpha {
            plan.append_item(Kind::Alpha, alpha.bytes)?;
        }
        let meta = image.metadata;
        for (kind, bytes) in [(Kind::Exif, meta.exif), (Kind::Xmp, meta.xmp)] {
            if !bytes.is_empty() {
                plan.append_item(kind, bytes)?;
            }
        }
        for (index, sample) in [Some(image.color), image.alpha].into_iter().enumerate() {
            let Some(sample) = sample else { continue };
            plan.property(index, Property::Spatial(meta.dimensions))?;
            plan.property(
                index,
                Property::Pixels(
                    depth,
                    if sample.configuration[2] & 16 == 0 {
                        3
                    } else {
                        1
                    },
                ),
            )?;
            plan.property(index, Property::Configuration(sample.configuration))?;
            if index == 0 {
                if !meta.icc.is_empty() {
                    plan.property(index, Property::Icc(meta.icc))?;
                }
                plan.property(index, Property::Color(meta.cicp, meta.full_range))?;
            } else {
                plan.property(index, Property::Alpha)?;
            }
            if let Some(angle) = meta.rotation {
                plan.property(index, Property::Rotation(angle))?;
            }
            if let Some(axis) = meta.mirror {
                plan.property(index, Property::Mirror(axis))?;
            }
        }
        let mut media_length = 0;
        for kind in [Kind::Exif, Kind::Xmp, Kind::Alpha, Kind::Color] {
            for item in &mut plan.items[..plan.item_count] {
                if item.kind != kind {
                    continue;
                }
                check_cancelled(token)?;
                if let Some(offset) = find_payload(
                    &plan.chunks[..plan.chunk_count],
                    media_length,
                    item.payload,
                    token,
                )? {
                    item.offset = offset;
                } else {
                    item.offset = media_length;
                    media_length = add(media_length, item.payload.len())?;
                    narrow(media_length)?;
                    *plan
                        .chunks
                        .get_mut(plan.chunk_count)
                        .ok_or_else(|| invalid("too many AVIF media chunks"))? = item.payload;
                    plan.chunk_count = add(plan.chunk_count, 1)?;
                }
            }
        }
        Ok(plan)
    }
}

/// Match the native earliest-substring search over the whole physical mdat,
/// including matches spanning two appended chunks. The virtual view avoids
/// allocating a second copy of potentially large prepared media payloads.
fn find_payload(
    chunks: &[&[u8]],
    length: usize,
    needle: &[u8],
    token: Option<&CancellationToken>,
) -> CodecResult<Option<usize>> {
    let Some(last) = length.checked_sub(needle.len()) else {
        return Ok(None);
    };
    for offset in 0..=last {
        if offset % CHUNK == 0 {
            check_cancelled(token)?;
        }
        let mut skip = offset;
        let mut matched = 0;
        'compare: for chunk in chunks {
            if skip >= chunk.len() {
                skip = skip.saturating_sub(chunk.len());
                continue;
            }
            for segment in chunk[skip..].chunks(CHUNK) {
                let count = segment.len().min(needle.len().saturating_sub(matched));
                if count == 0 {
                    break 'compare;
                }
                if matched != 0 || count == CHUNK {
                    check_cancelled(token)?;
                }
                let end = add(matched, count)?;
                if segment[..count] != needle[matched..end] {
                    break 'compare;
                }
                matched = end;
            }
            skip = 0;
        }
        if matched == needle.len() {
            return Ok(Some(offset));
        }
    }
    Ok(None)
}

fn reference(s: &mut Stream<'_>, kind: &[u8; 4], from: u16, to: u16) -> CodecResult<()> {
    let marker = s.start(kind)?;
    s.u16(from)?;
    s.u16(1)?;
    s.u16(to)?;
    s.finish(marker)
}

fn header(
    s: &mut Stream<'_>,
    image: &StillImage<'_>,
    plan: &Plan<'_>,
    depth: u8,
    media_start: usize,
) -> CodecResult<()> {
    let ftyp = s.start(b"ftyp")?;
    s.write(b"avif")?;
    s.u32(0)?;
    s.write(b"avifmif1miaf")?;
    let format = image.color.configuration[2] & 28;
    if depth <= 10 {
        if format == 12 {
            s.write(b"MA1B")?;
        } else if format == 0 {
            s.write(b"MA1A")?;
        }
    }
    s.finish(ftyp)?;
    let meta = s.full(b"meta", 0)?;
    let hdlr = s.full(b"hdlr", 0)?;
    s.u32(0)?;
    s.write(b"pict")?;
    s.write(&[0; 13])?;
    s.finish(hdlr)?;
    let pitm = s.full(b"pitm", 0)?;
    s.u16(1)?;
    s.finish(pitm)?;
    let iloc = s.full(b"iloc", 0)?;
    s.u16(0x4400)?;
    s.u16(u16::try_from(plan.item_count).map_err(|_| size_error())?)?;
    for (index, item) in plan.items[..plan.item_count].iter().enumerate() {
        s.u16(Item::id(index)?)?;
        s.u16(0)?;
        s.u16(1)?;
        s.u32(narrow(add(media_start, item.offset)?)?)?;
        s.u32(narrow(item.payload.len())?)?;
    }
    s.finish(iloc)?;
    let iinf = s.full(b"iinf", 0)?;
    s.u16(u16::try_from(plan.item_count).map_err(|_| size_error())?)?;
    for (index, item) in plan.items[..plan.item_count].iter().enumerate() {
        let infe = s.full(b"infe", 2)?;
        s.u16(Item::id(index)?)?;
        s.u16(0)?;
        s.write(match item.kind {
            Kind::Color => b"av01Color\0",
            Kind::Alpha => b"av01Alpha\0",
            Kind::Exif => b"ExifExif\0",
            Kind::Xmp => b"mimeXMP\0application/rdf+xml\0",
        })?;
        s.finish(infe)?;
    }
    s.finish(iinf)?;
    if plan.item_count > 1 {
        let iref = s.full(b"iref", 0)?;
        if image.alpha.is_some() && image.metadata.premultiplied {
            reference(s, b"prem", 1, 2)?;
        }
        for (index, item) in plan.items[..plan.item_count].iter().enumerate().skip(1) {
            reference(
                s,
                if item.kind == Kind::Alpha {
                    b"auxl"
                } else {
                    b"cdsc"
                },
                Item::id(index)?,
                1,
            )?;
        }
        s.finish(iref)?;
    }
    let iprp = s.start(b"iprp")?;
    let ipco = s.start(b"ipco")?;
    for property in &plan.properties[..plan.property_count] {
        property
            .ok_or_else(|| invalid("missing AVIF property"))?
            .emit(s)?;
    }
    s.finish(ipco)?;
    let ipma = s.full(b"ipma", 0)?;
    s.u32(if image.alpha.is_some() { 2 } else { 1 })?;
    for (index, item) in plan.items[..plan.item_count].iter().enumerate() {
        if item.association_count == 0 {
            continue;
        }
        s.u16(Item::id(index)?)?;
        s.write(&[u8::try_from(item.association_count).map_err(|_| size_error())?])?;
        s.write(&item.associations[..item.association_count])?;
    }
    s.finish(ipma)?;
    s.finish(iprp)?;
    s.finish(meta)
}

fn media(s: &mut Stream<'_>, plan: &Plan<'_>) -> CodecResult<()> {
    let marker = s.start(b"mdat")?;
    for chunk in &plan.chunks[..plan.chunk_count] {
        s.write(chunk)?;
    }
    s.finish(marker)
}

pub(super) fn write_still(
    image: &StillImage<'_>,
    policy: EncodePolicy,
    operation: CodecOperation,
    token: Option<&CancellationToken>,
) -> CodecResult<Vec<u8>> {
    check_cancelled(token)?;
    let depth = image.validate()?;
    let plan = Plan::new(image, depth, token)?;
    let mut sizing = Stream {
        bytes: None,
        position: 0,
        token,
    };
    header(&mut sizing, image, &plan, depth, 0)?;
    let media_start = add(sizing.position, 8)?;
    media(&mut sizing, &plan)?;
    policy
        .check_output_len(sizing.position, ImageFormat::Avif, operation)
        .map_err(CodecError::from_image_error)?;
    check_cancelled(token)?;
    let mut output = Vec::new();
    output.try_reserve_exact(sizing.position).map_err(|_| {
        CodecError::Dimensions("unable to allocate AVIF still container".to_owned())
    })?;
    let mut emitting = Stream {
        bytes: Some(output),
        position: 0,
        token,
    };
    header(&mut emitting, image, &plan, depth, media_start)?;
    media(&mut emitting, &plan)?;
    if emitting.position != sizing.position {
        return Err(invalid("AVIF sizing and emission disagree"));
    }
    emitting
        .bytes
        .ok_or_else(|| invalid("missing AVIF output stream"))
}
