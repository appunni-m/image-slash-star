// AS PER DESIGN — DO NOT REMOVE:
// Tests may use unwrap/expect. The deny lints are for production code only.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_in_result)]
#![allow(unused_crate_dependencies)]

//! Coverage matrix tests — driven by tests/fixtures/coverage_matrix.json
//! Each row in the matrix is one test assertion.
//! Decode: load asset → decode → compare pixel bytes with PIL reference bytes.
//! Encode: decode reference → encode with params → decode → compare pixel bytes.

use serde::Deserialize;
use std::collections::{HashMap, HashSet, hash_map::Entry};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use pillow_rs_image as img;

static COVERAGE_MATRIX: OnceLock<Option<CoverageMatrix>> = OnceLock::new();

fn coverage_matrix() -> Option<&'static CoverageMatrix> {
    COVERAGE_MATRIX
        .get_or_init(|| {
            let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
            let matrix_path = manifest_dir
                .join("tests")
                .join("fixtures")
                .join("coverage_matrix.json");

            if !matrix_path.exists() {
                return None;
            }

            Some(serde_json::from_str(&fs::read_to_string(&matrix_path).unwrap()).unwrap())
        })
        .as_ref()
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct CoverageMatrix {
    formats: HashMap<String, FormatData>,
    summary: Summary,
    #[serde(default)]
    operations: Vec<OperationRow>,
}

#[derive(Debug, Deserialize)]
struct OperationRow {
    id: String,
    source_format: String,
    source_asset: String,
    action: String,
    #[serde(default)]
    params: HashMap<String, serde_json::Value>,
    ref_path: String,
    ref_bytes: usize,
    ref_mode: String,
    ref_size: Vec<u32>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct FormatData {
    decode: Vec<DecodeRow>,
    encode: Vec<EncodeRow>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct DecodeRow {
    id: String,
    #[serde(rename = "type")]
    row_type: String,
    format: String,
    category: String,
    status: String,
    asset: Option<String>,
    asset_path: Option<String>,
    expect_error: Option<bool>,
    ref_mode: Option<String>,
    ref_size: Option<Vec<u32>>,
    ref_path: Option<String>,
    ref_bytes: Option<usize>,
    #[serde(default)]
    sequence: Option<SequenceParityRef>,
}

#[derive(Debug, Deserialize)]
struct SequenceParityRef {
    loop_count: Option<u32>,
    frames: Vec<FrameParityRef>,
}

#[derive(Debug, Deserialize)]
struct FrameParityRef {
    index: usize,
    ref_path: String,
    ref_bytes: usize,
    ref_mode: String,
    ref_size: Vec<u32>,
    duration_ms: u32,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct EncodeRow {
    id: String,
    #[serde(rename = "type")]
    row_type: String,
    format: String,
    params: HashMap<String, serde_json::Value>,
    description: Option<String>,
    status: String,
    #[serde(default)]
    expect_error: bool,
    #[serde(default)]
    source_format: Option<String>,
    #[serde(default)]
    source_asset: Option<String>,
    #[serde(default)]
    ref_bytes: Option<usize>,
    #[serde(default)]
    ref_mode: Option<String>,
    #[serde(default)]
    ref_size: Option<Vec<u32>>,
    #[serde(default)]
    ref_path: Option<String>,
    #[serde(default)]
    encoded_ref_path: Option<String>,
    #[serde(default)]
    encoded_ref_bytes: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Summary {
    total_rows: usize,
    decode_rows: usize,
    encode_rows: usize,
    formats: usize,
    assets_available: usize,
    decode_active: usize,
    decode_planned: usize,
    encode_not_wired: usize,
    #[serde(default)]
    operation_rows: usize,
}

#[derive(Debug)]
struct PixelParityRef {
    id: String,
    bytes: Vec<u8>,
    width: Option<u32>,
    height: Option<u32>,
    mode: Option<String>,
}

#[derive(Debug)]
struct PixelMismatch {
    byte_index: usize,
    pixel_index: usize,
    x: u32,
    y: u32,
    channel: usize,
    expected: u8,
    actual: u8,
}

fn option_text(value: &serde_json::Value) -> String {
    value
        .as_str()
        .map_or_else(|| value.to_string(), str::to_owned)
}

fn extra_encode_options(params: &HashMap<String, serde_json::Value>) -> HashMap<String, String> {
    params
        .iter()
        .map(|(key, value)| (key.clone(), option_text(value)))
        .collect()
}

fn read_le_u16(data: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        data.get(offset..offset.checked_add(2)?)?.try_into().ok()?,
    ))
}

fn read_le_u32(data: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        data.get(offset..offset.checked_add(4)?)?.try_into().ok()?,
    ))
}

fn read_be_u32(data: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_be_bytes(
        data.get(offset..offset.checked_add(4)?)?.try_into().ok()?,
    ))
}

fn assert_png_contract(
    params: &HashMap<String, serde_json::Value>,
    encoded: &[u8],
) -> Result<(), String> {
    if encoded.get(..8) != Some(b"\x89PNG\r\n\x1a\n") {
        return Err("encoded PNG has an invalid signature".to_owned());
    }
    let ihdr = encoded
        .get(8..33)
        .filter(|chunk| chunk.get(4..8) == Some(b"IHDR"))
        .ok_or("encoded PNG has no complete IHDR")?;
    let bit_depth = ihdr[16];
    let color_type = ihdr[17];
    let interlace = ihdr[20];

    if let Some(expected) = params.get("bit_depth").and_then(serde_json::Value::as_u64) {
        if u64::from(bit_depth) != expected {
            return Err(format!(
                "PNG depth mismatch: encoded {bit_depth}, requested {expected}"
            ));
        }
    }
    let color_request = params
        .get("color_type")
        .or_else(|| params.get("color"))
        .and_then(serde_json::Value::as_str);
    if let Some(request) = color_request {
        let expected = match request {
            "1" | "L" | "gray" => 0,
            "RGB" | "rgb" => 2,
            "P" => 3,
            "LA" | "gray_alpha" => 4,
            "RGBA" | "rgba" => 6,
            value => return Err(format!("unknown PNG color request {value}")),
        };
        if color_type != expected {
            return Err(format!(
                "PNG color mismatch: encoded type {color_type}, requested {request}"
            ));
        }
    }
    let requested_interlace = params
        .get("interlace")
        .or_else(|| params.get("interlaced"))
        .and_then(serde_json::Value::as_bool);
    if requested_interlace.is_some() && interlace != 0 {
        return Err(format!(
            "PNG interlace mismatch: Pillow ignores this option but encoded {interlace}"
        ));
    }

    let mut chunks = Vec::new();
    let mut offset = 8usize;
    while offset
        .checked_add(12)
        .is_some_and(|end| end <= encoded.len())
    {
        let length =
            usize::try_from(read_be_u32(encoded, offset).ok_or("truncated PNG chunk length")?)
                .map_err(|_| "PNG chunk is too large")?;
        let kind = encoded
            .get(offset + 4..offset + 8)
            .ok_or("truncated PNG chunk type")?;
        chunks.push(kind);
        offset = offset
            .checked_add(12)
            .and_then(|value| value.checked_add(length))
            .ok_or("PNG chunk length overflow")?;
    }
    for (option, kind) in [
        ("gamma", b"gAMA".as_slice()),
        ("srgb", b"sRGB".as_slice()),
        ("physical", b"pHYs".as_slice()),
        ("text_chunks", b"tEXt".as_slice()),
        ("time", b"tIME".as_slice()),
    ] {
        if params.get(option).and_then(serde_json::Value::as_bool) == Some(true)
            && !chunks.contains(&kind)
        {
            return Err(format!("PNG option {option} did not emit its chunk"));
        }
    }
    Ok(())
}

fn skip_gif_sub_blocks(encoded: &[u8], mut offset: usize) -> Option<usize> {
    loop {
        let length = usize::from(*encoded.get(offset)?);
        offset = offset.checked_add(1)?;
        if length == 0 {
            return Some(offset);
        }
        offset = offset.checked_add(length)?;
        encoded.get(..offset)?;
    }
}

fn assert_gif_contract(
    params: &HashMap<String, serde_json::Value>,
    encoded: &[u8],
) -> Result<(), String> {
    if !matches!(encoded.get(..6), Some(b"GIF87a" | b"GIF89a")) {
        return Err("encoded GIF has an invalid signature".to_owned());
    }
    let packed = *encoded.get(10).ok_or("truncated GIF logical screen")?;
    let has_global = packed & 0x80 != 0;
    let mut offset = 13usize;
    if has_global {
        offset = offset
            .checked_add(3usize << (usize::from(packed & 7) + 1))
            .ok_or("GIF color table overflow")?;
    }
    let mut frames = 0usize;
    let mut image_interlace = Vec::new();
    let mut image_local = Vec::new();
    let mut gce_disposals = Vec::new();
    let mut gce_transparency = Vec::new();
    let mut has_loop = false;
    loop {
        match *encoded.get(offset).ok_or("truncated GIF block stream")? {
            0x3b => break,
            0x2c => {
                let image_packed = *encoded.get(offset + 9).ok_or("truncated GIF image")?;
                frames += 1;
                image_local.push(image_packed & 0x80 != 0);
                image_interlace.push(image_packed & 0x40 != 0);
                offset += 10;
                if image_packed & 0x80 != 0 {
                    offset = offset
                        .checked_add(3usize << (usize::from(image_packed & 7) + 1))
                        .ok_or("GIF local color table overflow")?;
                }
                offset = offset.checked_add(1).ok_or("GIF image overflow")?;
                offset = skip_gif_sub_blocks(encoded, offset).ok_or("truncated GIF image data")?;
            }
            0x21 => {
                let label = *encoded.get(offset + 1).ok_or("truncated GIF extension")?;
                if label == 0xf9 {
                    if *encoded.get(offset + 2).ok_or("truncated GIF GCE")? != 4 {
                        return Err("invalid GIF GCE size".to_owned());
                    }
                    let gce_packed = *encoded.get(offset + 3).ok_or("truncated GIF GCE")?;
                    gce_disposals.push((gce_packed >> 2) & 7);
                    gce_transparency.push(gce_packed & 1 != 0);
                    offset = offset.checked_add(8).ok_or("GIF GCE overflow")?;
                } else {
                    if label == 0xff && encoded.get(offset + 3..offset + 14) == Some(b"NETSCAPE2.0")
                    {
                        has_loop = true;
                    }
                    offset = skip_gif_sub_blocks(encoded, offset + 2)
                        .ok_or("truncated GIF extension data")?;
                }
            }
            marker => return Err(format!("unknown GIF block marker 0x{marker:02x}")),
        }
    }

    // `frames` selects source frames passed to Pillow. Pillow may coalesce
    // visually identical consecutive frames, so the emitted descriptor count
    // is an output property covered by the exact byte reference, not a direct
    // restatement of the input selection count.
    if frames == 0 {
        return Err("encoded GIF has no image descriptor".to_owned());
    }
    if params.get("loop").and_then(serde_json::Value::as_bool) == Some(true) && !has_loop {
        return Err("GIF loop option did not emit NETSCAPE2.0".to_owned());
    }
    if let Some(expected) = params.get("interlace").and_then(serde_json::Value::as_bool)
        && image_interlace.iter().any(|&value| value != expected)
    {
        return Err(format!("GIF interlace setting does not match {expected}"));
    }
    if let Some(request) = params
        .get("color_table")
        .and_then(serde_json::Value::as_str)
    {
        let expected_local = request == "local";
        if !has_global || image_local.iter().any(|&value| value != expected_local) {
            return Err(format!("GIF color-table layout does not match {request}"));
        }
    }
    if let Some(request) = params.get("disposal").and_then(serde_json::Value::as_str) {
        let expected = match request {
            "none" => 0,
            "background" => 2,
            "previous" => 3,
            value => return Err(format!("unknown GIF disposal request {value}")),
        };
        if expected != 0 && !gce_disposals.contains(&expected) {
            return Err(format!("GIF disposal method does not match {request}"));
        }
    }
    if let Some(expected) = params
        .get("transparency")
        .and_then(serde_json::Value::as_bool)
    {
        if gce_transparency.iter().any(|&value| value != expected)
            || expected && gce_transparency.is_empty()
        {
            return Err(format!(
                "GIF transparency setting does not match {expected}"
            ));
        }
    }
    Ok(())
}

fn assert_bmp_contract(
    params: &HashMap<String, serde_json::Value>,
    encoded: &[u8],
) -> Result<(), String> {
    if encoded.get(..2) != Some(b"BM") {
        return Err("encoded BMP is missing BM signature".to_owned());
    }
    let header_size = read_le_u32(encoded, 14).ok_or("BMP header is truncated")?;
    let height = read_le_u32(encoded, 22).ok_or("BMP height is truncated")? as i32;
    let depth = read_le_u16(encoded, 28).ok_or("BMP depth is truncated")?;
    let compression = read_le_u32(encoded, 30).ok_or("BMP compression is truncated")?;

    if let Some(expected) = params.get("bit_depth").and_then(serde_json::Value::as_u64) {
        let expected = match expected {
            4 | 16 => 24,
            value => u16::try_from(value).map_err(|_| "invalid BMP bit_depth")?,
        };
        if depth != expected {
            return Err(format!(
                "BMP mode-derived depth mismatch: encoded {depth}, expected {expected}"
            ));
        }
    }
    if params.get("header").is_some() {
        if header_size != 40 {
            return Err(format!(
                "BMP header mismatch: Pillow always emits V3 but encoded {header_size}"
            ));
        }
    }
    if params.get("top_down").is_some() {
        if height.is_negative() {
            return Err(format!(
                "BMP row direction mismatch: Pillow always emits bottom-up height {height}"
            ));
        }
    }
    if params.get("compression").is_some() {
        if compression != 0 {
            return Err(format!(
                "BMP compression mismatch: Pillow always emits BI_RGB but encoded {compression}"
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum TiffEndian {
    Little,
    Big,
}

impl TiffEndian {
    fn read_u16(self, data: &[u8], offset: usize) -> Option<u16> {
        let bytes: [u8; 2] = data.get(offset..offset.checked_add(2)?)?.try_into().ok()?;
        Some(match self {
            Self::Little => u16::from_le_bytes(bytes),
            Self::Big => u16::from_be_bytes(bytes),
        })
    }

    fn read_u32(self, data: &[u8], offset: usize) -> Option<u32> {
        let bytes: [u8; 4] = data.get(offset..offset.checked_add(4)?)?.try_into().ok()?;
        Some(match self {
            Self::Little => u32::from_le_bytes(bytes),
            Self::Big => u32::from_be_bytes(bytes),
        })
    }
}

fn assert_tiff_contract(
    params: &HashMap<String, serde_json::Value>,
    encoded: &[u8],
) -> Result<(), String> {
    let endian = match encoded.get(..2) {
        Some(b"II") => TiffEndian::Little,
        Some(b"MM") => TiffEndian::Big,
        _ => return Err("encoded TIFF has an invalid byte-order marker".to_owned()),
    };
    if endian.read_u16(encoded, 2) != Some(42) {
        return Err("encoded TIFF has an invalid magic value".to_owned());
    }
    if let Some(request) = params.get("byte_order").and_then(serde_json::Value::as_str) {
        if !matches!(endian, TiffEndian::Little) {
            return Err(format!(
                "TIFF byte order mismatch: Pillow ignores {request} and emits little-endian"
            ));
        }
    }
    let ifd = usize::try_from(endian.read_u32(encoded, 4).ok_or("truncated TIFF header")?)
        .map_err(|_| "TIFF IFD offset is too large")?;
    let count = usize::from(endian.read_u16(encoded, ifd).ok_or("truncated TIFF IFD")?);
    let mut tags = HashMap::<u16, u32>::new();
    for index in 0..count {
        let offset = ifd
            .checked_add(2)
            .and_then(|value| value.checked_add(index.checked_mul(12)?))
            .ok_or("TIFF IFD overflow")?;
        let tag = endian
            .read_u16(encoded, offset)
            .ok_or("truncated TIFF entry")?;
        let field_type = endian
            .read_u16(encoded, offset + 2)
            .ok_or("truncated TIFF entry type")?;
        let item_count = endian
            .read_u32(encoded, offset + 4)
            .ok_or("truncated TIFF entry count")?;
        if item_count == 1 && matches!(field_type, 3 | 4) {
            let value = if field_type == 3 {
                u32::from(
                    endian
                        .read_u16(encoded, offset + 8)
                        .ok_or("truncated TIFF SHORT value")?,
                )
            } else {
                endian
                    .read_u32(encoded, offset + 8)
                    .ok_or("truncated TIFF LONG value")?
            };
            tags.insert(tag, value);
        }
    }
    if let Some(request) = params
        .get("compression")
        .and_then(serde_json::Value::as_str)
    {
        let expected = match request {
            "none" => 1,
            "lzw" => 5,
            "deflate" => 8,
            "packbits" => 32_773,
            value => return Err(format!("unknown TIFF compression request {value}")),
        };
        if tags.get(&259) != Some(&expected) {
            return Err(format!("TIFF compression tag does not match {request}"));
        }
    }
    if let Some(request) = params.get("predictor").and_then(serde_json::Value::as_str) {
        let expected = if request == "horizontal" { 2 } else { 1 };
        let actual = tags.get(&317).copied().unwrap_or(1);
        if actual != expected {
            return Err(format!("TIFF predictor tag does not match {request}"));
        }
    }
    if let Some(request) = params
        .get("organization")
        .and_then(serde_json::Value::as_str)
    {
        let tiled = tags.contains_key(&322) || tags.contains_key(&324);
        if tiled {
            return Err(format!(
                "TIFF organization mismatch: Pillow ignores {request} and emits strips"
            ));
        }
    }
    if let Some(request) = params.get("pages").and_then(serde_json::Value::as_u64) {
        let next_ifd_offset = ifd
            .checked_add(2 + count * 12)
            .ok_or("TIFF next-IFD offset overflow")?;
        if endian.read_u32(encoded, next_ifd_offset) != Some(0) {
            return Err(format!(
                "TIFF page-count mismatch: Pillow ignores pages={request} and emits one page"
            ));
        }
    }
    Ok(())
}

fn assert_jpeg_contract(
    params: &HashMap<String, serde_json::Value>,
    encoded: &[u8],
) -> Result<(), String> {
    if encoded.get(..2) != Some(&[0xff, 0xd8]) {
        return Err("encoded JPEG has no SOI marker".to_owned());
    }
    let mut offset = 2usize;
    let mut sof = None::<(u8, &[u8])>;
    let mut has_exif = false;
    let mut has_restart_interval = false;
    while offset < encoded.len() {
        while encoded.get(offset) == Some(&0xff) {
            offset += 1;
        }
        let marker = *encoded.get(offset).ok_or("truncated JPEG marker")?;
        offset += 1;
        if matches!(marker, 0xd8 | 0xd9 | 0x01 | 0xd0..=0xd7) {
            continue;
        }
        let length = usize::from(u16::from_be_bytes(
            encoded
                .get(offset..offset + 2)
                .ok_or("truncated JPEG segment length")?
                .try_into()
                .map_err(|_| "invalid JPEG segment length")?,
        ));
        if length < 2 {
            return Err("invalid JPEG segment length".to_owned());
        }
        let payload = encoded
            .get(offset + 2..offset + length)
            .ok_or("truncated JPEG segment")?;
        if marker == 0xe1 && payload.starts_with(b"Exif\0\0") {
            has_exif = true;
        }
        if marker == 0xdd {
            has_restart_interval = true;
        }
        if matches!(marker, 0xc0 | 0xc2) {
            sof = Some((marker, payload));
        }
        offset += length;
        if marker == 0xda {
            break;
        }
    }
    let (sof_marker, sof_data) = sof.ok_or("encoded JPEG has no supported SOF marker")?;
    if sof_data.len() < 8 {
        return Err("truncated JPEG SOF segment".to_owned());
    }
    if let Some(expected) = params
        .get("progressive")
        .and_then(serde_json::Value::as_bool)
        && (sof_marker == 0xc2) != expected
    {
        return Err(format!("JPEG progressive mode does not match {expected}"));
    }
    if let Some(expected) = params.get("grayscale").and_then(serde_json::Value::as_bool) {
        let components = sof_data[5];
        if (components == 1) != expected {
            return Err(format!("JPEG grayscale mode does not match {expected}"));
        }
    }
    if let Some(request) = params
        .get("subsampling")
        .and_then(serde_json::Value::as_str)
    {
        let expected = match request {
            "444" => 0x11,
            "422" => 0x21,
            "420" => 0x22,
            value => return Err(format!("unknown JPEG subsampling request {value}")),
        };
        if sof_data[7] != expected {
            return Err(format!("JPEG sampling factors do not match {request}"));
        }
    }
    if params.get("exif").and_then(serde_json::Value::as_bool) == Some(false) && has_exif {
        return Err("JPEG exif=false emitted EXIF metadata".to_owned());
    }
    if params.get("exif_hex").is_some() && !has_exif {
        return Err("JPEG EXIF metadata request did not emit APP1 EXIF".to_owned());
    }
    if params
        .get("restart_interval")
        .and_then(serde_json::Value::as_u64)
        == Some(0)
        && has_restart_interval
    {
        return Err("JPEG restart_interval=0 emitted DRI".to_owned());
    }
    Ok(())
}

fn assert_ico_contract(
    params: &HashMap<String, serde_json::Value>,
    encoded: &[u8],
) -> Result<(), String> {
    if encoded.get(..4) != Some(&[0, 0, 1, 0]) {
        return Err("encoded ICO has an invalid header".to_owned());
    }
    let count = usize::from(read_le_u16(encoded, 4).ok_or("truncated ICO header")?);
    if count == 0 {
        return (encoded.len() == 6)
            .then_some(())
            .ok_or_else(|| "zero-entry ICO has trailing data".to_owned());
    }
    if encoded.len() < 6 + count * 16 {
        return Err("encoded ICO has an invalid image directory".to_owned());
    }

    let expect_bmp = params.get("entry_type").and_then(serde_json::Value::as_str) == Some("bmp");
    for index in 0..count {
        let entry = 6 + index * 16;
        let directory_depth =
            read_le_u16(encoded, entry + 6).ok_or("truncated ICO directory entry")?;
        if !expect_bmp && directory_depth != 32 {
            return Err("ICO PNG directory entry is not 32-bit".to_owned());
        }

        let data_size = usize::try_from(
            read_le_u32(encoded, entry + 8).ok_or("truncated ICO directory entry")?,
        )
        .map_err(|_| "ICO data size is too large")?;
        let data_offset = usize::try_from(
            read_le_u32(encoded, entry + 12).ok_or("truncated ICO directory entry")?,
        )
        .map_err(|_| "ICO data offset is too large")?;
        if data_offset
            .checked_add(data_size)
            .is_none_or(|end| end > encoded.len())
        {
            return Err("ICO directory entry points outside the file".to_owned());
        }
        if expect_bmp && read_le_u32(encoded, data_offset) != Some(40) {
            return Err("ICO BMP entry request did not emit a BITMAPINFOHEADER".to_owned());
        }
        if expect_bmp && read_le_u16(encoded, data_offset + 14) != Some(directory_depth) {
            return Err("ICO BMP directory and payload bit depths disagree".to_owned());
        }
    }
    Ok(())
}

fn encoded_dimensions(format: &str, encoded: &[u8]) -> Option<(u32, u32)> {
    match format {
        "bmp" => Some((
            read_le_u32(encoded, 18)?,
            read_le_u32(encoded, 22)? & 0x7fff_ffff,
        )),
        "gif" => Some((
            u32::from(read_le_u16(encoded, 6)?),
            u32::from(read_le_u16(encoded, 8)?),
        )),
        "ico" => Some((
            if encoded.get(6).copied()? == 0 {
                256
            } else {
                u32::from(encoded[6])
            },
            if encoded.get(7).copied()? == 0 {
                256
            } else {
                u32::from(encoded[7])
            },
        )),
        "jpeg" => {
            let marker = encoded
                .windows(2)
                .position(|pair| matches!(pair, [0xff, 0xc0] | [0xff, 0xc2]))?;
            Some((
                u32::from(u16::from_be_bytes(
                    encoded.get(marker + 7..marker + 9)?.try_into().ok()?,
                )),
                u32::from(u16::from_be_bytes(
                    encoded.get(marker + 5..marker + 7)?.try_into().ok()?,
                )),
            ))
        }
        "png" => Some((read_be_u32(encoded, 16)?, read_be_u32(encoded, 20)?)),
        _ => None,
    }
}

fn assert_encoded_contract(
    format: &str,
    params: &HashMap<String, serde_json::Value>,
    encoded: &[u8],
) -> Result<(), String> {
    match format {
        "bmp" => assert_bmp_contract(params, encoded),
        "gif" => assert_gif_contract(params, encoded),
        "ico" => assert_ico_contract(params, encoded),
        "jpeg" => assert_jpeg_contract(params, encoded),
        "png" => assert_png_contract(params, encoded),
        "tiff" => assert_tiff_contract(params, encoded),
        _ => Ok(()),
    }?;
    if let Some(size) = params.get("size").and_then(serde_json::Value::as_array) {
        let expected = (
            size.first()
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| u32::try_from(value).ok())
                .ok_or("invalid requested width")?,
            size.get(1)
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| u32::try_from(value).ok())
                .ok_or("invalid requested height")?,
        );
        if let Some(actual) = encoded_dimensions(format, encoded)
            && actual != expected
        {
            return Err(format!(
                "{format} dimensions mismatch: encoded {actual:?}, requested {expected:?}"
            ));
        }
    }
    Ok(())
}

fn expected_raw_name(module: &str, format: &str, asset: &str) -> String {
    format!("{module}.{format}_{}.bin", asset.replace('.', "_"))
}

fn load_pixel_reference(
    manifest_dir: &Path,
    id: &str,
    ref_path: Option<&str>,
    module: &str,
    format: &str,
    asset: &str,
    ref_size: Option<&[u32]>,
    ref_mode: Option<&str>,
) -> Option<PixelParityRef> {
    let raw_path = ref_path.map_or_else(
        || {
            manifest_dir
                .join("tests")
                .join("fixtures")
                .join("outputs")
                .join("raws")
                .join(expected_raw_name(module, format, asset))
        },
        |path| manifest_dir.join(path),
    );

    let bytes = match fs::read(&raw_path) {
        Ok(bytes) => bytes,
        Err(err) => {
            eprintln!("  SKIP [{id}]: reference pixels not readable at {raw_path:?}: {err}");
            return None;
        }
    };

    Some(PixelParityRef {
        id: id.to_owned(),
        bytes,
        width: ref_size.and_then(|s| s.first().copied()),
        height: ref_size.and_then(|s| s.get(1).copied()),
        mode: ref_mode.map(str::to_owned),
    })
}

fn mode_bytes_per_pixel(mode: Option<&str>) -> Option<usize> {
    match mode {
        Some("1") | Some("P") | Some("L") | Some("L8") => Some(1),
        Some("I;16") | Some("I;16B") | Some("I;16L") | Some("L16") | Some("La8") => Some(2),
        Some("RGB") | Some("Rgb8") => Some(3),
        Some("RGBA") | Some("Rgba8") | Some("La16") | Some("I") | Some("I32") => Some(4),
        Some("Rgb16") => Some(6),
        Some("Rgba16") => Some(8),
        _ => None,
    }
}

fn expected_image_mode(mode: &str) -> Option<img::ImageMode> {
    match mode {
        "1" => Some(img::ImageMode::L1),
        "P" => Some(img::ImageMode::P8),
        "L" | "L8" => Some(img::ImageMode::L8),
        "LA" | "La8" => Some(img::ImageMode::La8),
        "RGB" | "Rgb8" => Some(img::ImageMode::Rgb8),
        "RGBA" | "Rgba8" => Some(img::ImageMode::Rgba8),
        "CMYK" | "Cmyk8" => Some(img::ImageMode::Cmyk8),
        "I;16" | "I;16B" | "I;16L" | "L16" => Some(img::ImageMode::L16),
        "La16" => Some(img::ImageMode::La16),
        "Rgb16" => Some(img::ImageMode::Rgb16),
        "Rgba16" => Some(img::ImageMode::Rgba16),
        "F" | "F32" => Some(img::ImageMode::F32),
        "I" | "I32" => Some(img::ImageMode::I32),
        _ => None,
    }
}

fn first_pixel_mismatches(
    expected: &[u8],
    actual: &[u8],
    width: u32,
    bytes_per_pixel: usize,
) -> Vec<PixelMismatch> {
    expected
        .chunks(64)
        .zip(actual.chunks(64))
        .enumerate()
        .flat_map(|(chunk_index, (expected_chunk, actual_chunk))| {
            expected_chunk
                .iter()
                .zip(actual_chunk)
                .enumerate()
                .filter_map(move |(offset, (&expected, &actual))| {
                    if expected == actual {
                        return None;
                    }
                    let byte_index = chunk_index * 64 + offset;
                    let pixel_index = byte_index / bytes_per_pixel;
                    let x = (pixel_index as u32) % width;
                    let y = (pixel_index as u32) / width;
                    Some(PixelMismatch {
                        byte_index,
                        pixel_index,
                        x,
                        y,
                        channel: byte_index % bytes_per_pixel,
                        expected,
                        actual,
                    })
                })
        })
        .take(8)
        .collect()
}

fn count_mismatched_bytes(expected: &[u8], actual: &[u8]) -> usize {
    expected
        .chunks(1024)
        .zip(actual.chunks(1024))
        .map(|(expected_chunk, actual_chunk)| {
            expected_chunk
                .iter()
                .zip(actual_chunk)
                .filter(|(expected, actual)| expected != actual)
                .count()
        })
        .sum()
}

fn assert_encoded_byte_parity(expected: &[u8], actual: &[u8]) -> Result<(), String> {
    if expected.len() != actual.len() {
        return Err(format!(
            "encoded byte length mismatch: actual {}, expected {}",
            actual.len(),
            expected.len()
        ));
    }
    if let Some(index) = expected
        .iter()
        .zip(actual)
        .position(|(expected, actual)| expected != actual)
    {
        return Err(format!(
            "encoded bytes first differ at byte {index}: actual {:02x}, expected {:02x}",
            actual[index], expected[index]
        ));
    }
    Ok(())
}

fn assert_pixel_parity(
    expected: &PixelParityRef,
    actual: &img::DecodedImage,
) -> Result<(), String> {
    if let Some(expected_mode) = expected.mode.as_deref().and_then(expected_image_mode)
        && actual.mode != expected_mode
    {
        return Err(format!(
            "mode mismatch: actual {:?}, expected {:?}",
            actual.mode, expected_mode
        ));
    }
    if let Some(width) = expected.width {
        if actual.width != width {
            return Err(format!(
                "width mismatch: actual {}, expected {}",
                actual.width, width
            ));
        }
    }
    if let Some(height) = expected.height {
        if actual.height != height {
            return Err(format!(
                "height mismatch: actual {}, expected {}",
                actual.height, height
            ));
        }
    }

    let actual_bytes = actual.as_bytes();
    if actual_bytes.len() != expected.bytes.len() {
        return Err(format!(
            "byte length mismatch: actual {}, expected {}",
            actual_bytes.len(),
            expected.bytes.len()
        ));
    }

    if actual_bytes == expected.bytes.as_slice() {
        return Ok(());
    }

    let bytes_per_pixel = mode_bytes_per_pixel(expected.mode.as_deref())
        .unwrap_or_else(|| usize::from(actual.color.bytes_per_pixel()));
    let width = expected.width.unwrap_or(actual.width).max(1);
    let mismatch_count = count_mismatched_bytes(&expected.bytes, actual_bytes);
    let examples = first_pixel_mismatches(&expected.bytes, actual_bytes, width, bytes_per_pixel)
        .into_iter()
        .map(|m| {
            format!(
                "byte {} pixel {} ({}, {}) channel {} expected {:02x} actual {:02x}",
                m.byte_index, m.pixel_index, m.x, m.y, m.channel, m.expected, m.actual
            )
        })
        .collect::<Vec<_>>()
        .join("; ");

    Err(format!(
        "{} mismatched byte(s) out of {} for mode {}; first: {}",
        mismatch_count,
        actual_bytes.len(),
        expected.mode.as_deref().unwrap_or("?"),
        examples
    ))
}

fn assert_dynamic_bridge_parity(
    expected: &PixelParityRef,
    decoded: &img::DecodedImage,
) -> Result<(), String> {
    if decoded.mode != decoded.color.into()
        || decoded.palette.is_some()
        || matches!(
            decoded.color,
            img::ColorType::Cmyk8 | img::ColorType::L32F | img::ColorType::L32I
        )
    {
        return Ok(());
    }

    let dynamic = img::DynamicImage::from_decoded(decoded)
        .ok_or("canonical decoded image could not enter the DynamicImage bridge")?;
    let bridged = dynamic.into_decoded();
    assert_pixel_parity(expected, &bridged)
        .map_err(|message| format!("DynamicImage bridge changed Pillow bytes: {message}"))
}

fn assert_sequence_parity(manifest_dir: &Path, row: &DecodeRow, data: &[u8]) -> Result<(), String> {
    let Some(expected) = &row.sequence else {
        return Ok(());
    };
    let actual = img::decode_sequence(data).ok_or("sequence decode returned None")?;
    if actual.loop_count != expected.loop_count {
        return Err(format!(
            "loop count mismatch: actual {:?}, expected {:?}",
            actual.loop_count, expected.loop_count
        ));
    }
    if actual.frames.len() != expected.frames.len() {
        return Err(format!(
            "frame count mismatch: actual {}, expected {}",
            actual.frames.len(),
            expected.frames.len()
        ));
    }
    for (actual_frame, expected_frame) in actual.frames.iter().zip(&expected.frames) {
        if actual_frame.duration_ms != expected_frame.duration_ms {
            return Err(format!(
                "frame {} duration mismatch: actual {}, expected {}",
                expected_frame.index, actual_frame.duration_ms, expected_frame.duration_ms
            ));
        }
        let bytes = fs::read(manifest_dir.join(&expected_frame.ref_path)).map_err(|error| {
            format!(
                "frame {} reference unreadable: {error}",
                expected_frame.index
            )
        })?;
        if bytes.len() != expected_frame.ref_bytes {
            return Err(format!(
                "frame {} reference length mismatch: actual {}, declared {}",
                expected_frame.index,
                bytes.len(),
                expected_frame.ref_bytes
            ));
        }
        let reference = PixelParityRef {
            id: format!("{} frame {}", row.id, expected_frame.index),
            bytes,
            width: expected_frame.ref_size.first().copied(),
            height: expected_frame.ref_size.get(1).copied(),
            mode: Some(expected_frame.ref_mode.clone()),
        };
        assert_pixel_parity(&reference, &actual_frame.image)
            .map_err(|message| format!("frame {}: {message}", expected_frame.index))?;
    }
    Ok(())
}

// ── Decode Tests ─────────────────────────────────────────────────────────

fn decode_direct(data: &[u8], format: &str) -> Option<img::DecodedImage> {
    match format {
        "jpeg" => img::codecs::jpeg::decode::decode(data),
        "png" => img::codecs::png::decode::decode(data),
        "gif" => img::codecs::gif::decode::decode(data),
        "bmp" => img::codecs::bmp::decode::decode(data),
        "tiff" => img::codecs::tiff::decode::decode(data),
        "webp" => img::codecs::webp::decode::decode(data),
        "ico" => img::codecs::ico::decode::decode(data),
        _ => None,
    }
}

fn encode_direct(
    image: &img::DecodedImage,
    format: img::ImageFormat,
    options: &img::encode_options::EncodeOptions,
) -> Option<Vec<u8>> {
    match format {
        img::ImageFormat::Jpeg | img::ImageFormat::Avif => None,
        img::ImageFormat::Png => img::codecs::png::encode::encode(image, options),
        img::ImageFormat::Gif => img::codecs::gif::encode::encode(image, options),
        img::ImageFormat::Bmp => img::codecs::bmp::encode::encode(image, options),
        img::ImageFormat::Tiff => img::codecs::tiff::encode::encode(image, options),
        img::ImageFormat::WebP => img::codecs::webp::encode::encode(image, options),
        img::ImageFormat::Ico => img::codecs::ico::encode::encode(image, options),
    }
}

#[test]
fn test_decode_matrix() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let matrix = coverage_matrix().expect(
        "coverage_matrix.json is required; run scripts/generate_decode_refs.py to regenerate it",
    );

    let assets_dir = manifest_dir
        .join("tests")
        .join("fixtures")
        .join("input")
        .join("images");
    let mut total = 0u32;
    let mut passed = 0u32;
    let mut failed = 0u32;
    let mut skipped = 0u32;

    for (fmt_name, fmt_data) in &matrix.formats {
        for row in &fmt_data.decode {
            if row.status == "planned" {
                skipped += 1;
                continue;
            }
            let asset_name = match &row.asset {
                Some(a) => a,
                None => {
                    total += 1;
                    failed += 1;
                    eprintln!("  FAIL [{}]: active row has no asset", row.id);
                    continue;
                }
            };
            let asset_path = assets_dir.join(fmt_name).join(asset_name);
            if !asset_path.exists() {
                total += 1;
                failed += 1;
                eprintln!("  FAIL [{}]: asset not found: {:?}", row.id, asset_path);
                continue;
            }

            total += 1;
            let data = match fs::read(&asset_path) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("  FAIL [{}]: read error {}", row.id, e);
                    failed += 1;
                    continue;
                }
            };

            let decoded = img::decode(&data);
            let direct = decode_direct(&data, fmt_name);
            if row.expect_error.unwrap_or(false) {
                let sequence_rejected = fmt_name != "webp"
                    || img::codecs::webp::decode::decode_sequence(&data).is_none();
                if decoded.is_none() && direct.is_none() && sequence_rejected {
                    eprintln!("  OK   [{}] rejected as Pillow does", row.id);
                    passed += 1;
                } else {
                    eprintln!(
                        "  FAIL [{}]: invalid input decoded successfully (auto={}, direct={}, sequence_rejected={})",
                        row.id,
                        decoded.is_some(),
                        direct.is_some(),
                        sequence_rejected
                    );
                    failed += 1;
                }
                continue;
            }

            let decoded = match decoded {
                Some(d) => d,
                None => {
                    eprintln!("  FAIL [{}]: decode returned None", row.id);
                    failed += 1;
                    continue;
                }
            };
            let direct = match direct {
                Some(image) if image == decoded => image,
                Some(_) => {
                    eprintln!(
                        "  FAIL [{}]: direct and auto-detected decoders differ",
                        row.id
                    );
                    failed += 1;
                    continue;
                }
                None => {
                    eprintln!("  FAIL [{}]: direct decoder returned None", row.id);
                    failed += 1;
                    continue;
                }
            };

            let Some(expected) = load_pixel_reference(
                manifest_dir,
                &row.id,
                row.ref_path.as_deref(),
                "Decode",
                fmt_name,
                asset_name,
                row.ref_size.as_deref(),
                row.ref_mode.as_deref(),
            ) else {
                eprintln!(
                    "  FAIL [{}]: active row has no readable pixel reference",
                    row.id
                );
                failed += 1;
                continue;
            };

            match assert_pixel_parity(&expected, &decoded)
                .and_then(|()| assert_pixel_parity(&expected, &direct))
                .and_then(|()| assert_dynamic_bridge_parity(&expected, &decoded))
                .and_then(|()| assert_sequence_parity(manifest_dir, row, &data))
            {
                Ok(()) => {
                    eprintln!(
                        "  OK   [{}] {} bytes pixel-parity (mode={})",
                        expected.id,
                        decoded.as_bytes().len(),
                        row.ref_mode.as_deref().unwrap_or("?")
                    );
                    passed += 1;
                }
                Err(message) => {
                    eprintln!("  FAIL [{}]: {message}", expected.id);
                    failed += 1;
                }
            }
        }
    }

    eprintln!("\ndecode matrix: {passed}/{total} passed, {failed} failed, {skipped} skipped");
    if failed > 0 {
        panic!("{failed} decode test(s) failed");
    }
}

// ── Encode Tests ─────────────────────────────────────────────────────────

#[test]
fn test_encode_matrix() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let matrix = coverage_matrix().expect(
        "coverage_matrix.json is required; run scripts/generate_decode_refs.py to regenerate it",
    );

    let mut total = 0u32;
    let mut passed = 0u32;
    let mut failed = 0u32;
    let mut skipped = 0u32;
    let assets_dir = manifest_dir
        .join("tests")
        .join("fixtures")
        .join("input")
        .join("images");
    let mut asset_cache: HashMap<PathBuf, Vec<u8>> = HashMap::new();
    let mut decoded_cache: HashMap<PathBuf, img::DecodedSequence> = HashMap::new();

    for (fmt_name, fmt_data) in &matrix.formats {
        if fmt_data.encode.is_empty() {
            continue;
        }

        for row in &fmt_data.encode {
            if row.status == "planned" {
                skipped += 1;
                continue;
            }

            total += 1;

            // Determine source: use row's source_asset if present, otherwise fall back
            // to the first active decode row for this format.
            let asset_path =
                if let (Some(src_fmt), Some(src_asset)) = (&row.source_format, &row.source_asset) {
                    let path = assets_dir.join(src_fmt).join(src_asset);
                    if path.exists() {
                        path
                    } else {
                        eprintln!("  FAIL [{}]: source asset not found: {:?}", row.id, path);
                        failed += 1;
                        continue;
                    }
                } else {
                    // Fallback: find a decode row in this format
                    let source_row = fmt_data
                        .decode
                        .iter()
                        .find(|r| r.status == "active" && r.asset.is_some());
                    match source_row {
                        Some(src) => {
                            let path = assets_dir.join(fmt_name).join(src.asset.as_ref().unwrap());
                            if path.exists() {
                                path
                            } else {
                                eprintln!("  FAIL [{}]: fallback source asset not found", row.id);
                                failed += 1;
                                continue;
                            }
                        }
                        None => {
                            eprintln!("  FAIL [{}]: active row has no source asset", row.id);
                            failed += 1;
                            continue;
                        }
                    }
                };

            if let Entry::Vacant(entry) = asset_cache.entry(asset_path.clone()) {
                entry.insert(fs::read(&asset_path).unwrap());
            }

            if let Entry::Vacant(entry) = decoded_cache.entry(asset_path.clone()) {
                let asset_data = asset_cache.get(&asset_path).unwrap();
                match img::decode_sequence(asset_data) {
                    Some(decoded) => {
                        entry.insert(decoded);
                    }
                    None => {
                        eprintln!("  FAIL [{}]: source decode failed", row.id);
                        failed += 1;
                        continue;
                    }
                }
            }
            let cached_decoded = decoded_cache.get(&asset_path).unwrap();
            let mut decoded_owned = row
                .params
                .contains_key("second_frame_mode")
                .then(|| cached_decoded.clone());
            if let Some(decoded) = decoded_owned.as_mut()
                && row
                    .params
                    .get("second_frame_mode")
                    .and_then(|value| value.as_str())
                    == Some("CMYK")
            {
                let frame = decoded.frames.get_mut(1).unwrap();
                let pixel_count = usize::try_from(frame.image.width).unwrap()
                    * usize::try_from(frame.image.height).unwrap();
                frame.image.pixels = vec![0; pixel_count * 4];
                frame.image.color = img::ColorType::Cmyk8;
                frame.image.mode = img::ImageMode::Cmyk8;
                frame.image.palette = None;
            }
            let decoded = decoded_owned.as_ref().unwrap_or(cached_decoded);

            // Build encode options from row params
            let opts = img::encode_options::EncodeOptions {
                quality: row
                    .params
                    .get("quality")
                    .and_then(|v| v.as_u64())
                    .and_then(|v| u8::try_from(v).ok()),
                compression: row
                    .params
                    .get("compression")
                    .and_then(|v| v.as_u64())
                    .and_then(|v| u8::try_from(v).ok()),
                lossless: row.params.get("lossless").and_then(|v| v.as_bool()),
                method: row
                    .params
                    .get("method")
                    .and_then(|v| v.as_u64())
                    .and_then(|v| u8::try_from(v).ok()),
                progressive: row.params.get("progressive").and_then(|v| v.as_bool()),
                optimize: row.params.get("optimize").and_then(|v| v.as_bool()),
                subsampling: row.params.get("subsampling").map(option_text),
                interlace: row
                    .params
                    .get("interlace")
                    .or_else(|| row.params.get("interlaced"))
                    .and_then(|v| v.as_bool()),
                extra: extra_encode_options(&row.params),
            };

            let format = match fmt_name.as_str() {
                "jpeg" => img::ImageFormat::Jpeg,
                "png" => img::ImageFormat::Png,
                "gif" => img::ImageFormat::Gif,
                "bmp" => img::ImageFormat::Bmp,
                "tiff" => img::ImageFormat::Tiff,
                "webp" => img::ImageFormat::WebP,
                "ico" => img::ImageFormat::Ico,
                _ => {
                    eprintln!(
                        "  FAIL [{}]: active format {fmt_name} has no encoder",
                        row.id
                    );
                    failed += 1;
                    continue;
                }
            };

            let encoded = if row
                .params
                .get("truncate_pixels")
                .is_some_and(|v| v.as_bool().unwrap_or(false))
            {
                let mut malformed = decoded.first().unwrap().clone();
                malformed.pixels.pop();
                encode_direct(&malformed, format, &opts)
                    .or_else(|| img::encode(&malformed, format, &opts))
            } else if let Some(dimensions) = row.params.get("source_dimensions") {
                let dimensions = dimensions.as_array().unwrap();
                let mut malformed = decoded.first().unwrap().clone();
                malformed.width = u32::try_from(dimensions[0].as_u64().unwrap()).unwrap();
                malformed.height = u32::try_from(dimensions[1].as_u64().unwrap()).unwrap();
                encode_direct(&malformed, format, &opts)
                    .or_else(|| img::encode(&malformed, format, &opts))
            } else {
                img::encode_sequence(decoded, format, &opts)
            };
            if row.expect_error {
                if encoded.is_none() {
                    eprintln!("  OK   [{}] rejected as Pillow does", row.id);
                    passed += 1;
                } else {
                    eprintln!("  FAIL [{}]: invalid input encoded successfully", row.id);
                    failed += 1;
                }
                continue;
            }
            let encoded = match encoded {
                Some(e) => e,
                None => {
                    eprintln!("  FAIL [{}]: encode returned None", row.id);
                    failed += 1;
                    continue;
                }
            };
            if decoded.frames.len() == 1 {
                match img::encode(decoded.first().unwrap(), format, &opts) {
                    Some(still) if still == encoded => {}
                    Some(_) => {
                        eprintln!(
                            "  FAIL [{}]: still and one-frame sequence encoders differ",
                            row.id
                        );
                        failed += 1;
                        continue;
                    }
                    None => {
                        eprintln!("  FAIL [{}]: still-image encoder returned None", row.id);
                        failed += 1;
                        continue;
                    }
                }
            }

            if let Err(message) = assert_encoded_contract(fmt_name, &row.params, &encoded) {
                eprintln!("  FAIL [{}]: {message}", row.id);
                failed += 1;
                continue;
            }

            let Some(encoded_ref_path) = row.encoded_ref_path.as_deref() else {
                eprintln!(
                    "  FAIL [{}]: active encode row has no encoded-byte reference",
                    row.id
                );
                failed += 1;
                continue;
            };
            let expected_encoded = match fs::read(manifest_dir.join(encoded_ref_path)) {
                Ok(bytes) => bytes,
                Err(error) => {
                    eprintln!(
                        "  FAIL [{}]: encoded-byte reference is unreadable: {error}",
                        row.id
                    );
                    failed += 1;
                    continue;
                }
            };
            if row.encoded_ref_bytes != Some(expected_encoded.len()) {
                eprintln!(
                    "  FAIL [{}]: encoded_ref_bytes metadata does not match the reference file",
                    row.id
                );
                failed += 1;
                continue;
            }
            if let Err(message) = assert_encoded_byte_parity(&expected_encoded, &encoded) {
                eprintln!("  FAIL [{}]: {message}", row.id);
                failed += 1;
                continue;
            }
            if row
                .params
                .get("encoded_only")
                .and_then(serde_json::Value::as_bool)
                == Some(true)
            {
                eprintln!(
                    "  OK   [{}] {}B, encoded-byte parity",
                    row.id,
                    encoded.len()
                );
                passed += 1;
                continue;
            }

            // Roundtrip: re-decode and compare pixels against the PIL reference.
            match img::decode(&encoded) {
                Some(redecoded) => {
                    if let Some(expected) = row.ref_path.as_deref().and_then(|ref_path| {
                        load_pixel_reference(
                            manifest_dir,
                            &row.id,
                            Some(ref_path),
                            "Encode",
                            fmt_name,
                            row.source_asset.as_deref().unwrap_or(""),
                            row.ref_size.as_deref(),
                            row.ref_mode.as_deref(),
                        )
                    }) {
                        match assert_pixel_parity(&expected, &redecoded)
                            .and_then(|()| assert_dynamic_bridge_parity(&expected, &redecoded))
                        {
                            Ok(()) => {
                                eprintln!(
                                    "  OK   [{}] {}B, re-decoded {}x{} pixel-parity (mode={})",
                                    row.id,
                                    encoded.len(),
                                    redecoded.width,
                                    redecoded.height,
                                    row.ref_mode.as_deref().unwrap_or("?")
                                );
                                passed += 1;
                            }
                            Err(message) => {
                                eprintln!("  FAIL [{}]: {message}", row.id);
                                failed += 1;
                            }
                        }
                    } else {
                        eprintln!(
                            "  FAIL [{}]: active encode row has no Pillow pixel reference",
                            row.id
                        );
                        failed += 1;
                    }
                }
                None => {
                    eprintln!("  FAIL [{}]: re-decode failed", row.id);
                    failed += 1;
                }
            }
        }
    }

    eprintln!("\nencode matrix: {passed}/{total} passed, {failed} failed, {skipped} skipped");
    if failed > 0 {
        panic!("{failed} encode test(s) failed");
    }
}

#[test]
fn test_operation_matrix() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let matrix = coverage_matrix().expect("coverage_matrix.json is required");
    let mut failed = Vec::new();
    let mut exercised_variants = HashSet::new();
    exercise_type_metadata();

    for row in &matrix.operations {
        let source_path = manifest_dir
            .join("tests/fixtures/input/images")
            .join(&row.source_format)
            .join(&row.source_asset);
        let source = fs::read(source_path).unwrap();
        let decoded = img::decode(&source).unwrap();
        let mut dynamic = img::DynamicImage::from_decoded(&decoded).unwrap();
        dynamic = match row
            .params
            .get("intermediate")
            .and_then(|value| value.as_str())
        {
            Some("L16") => img::DynamicImage::ImageLuma16(dynamic.to_luma16()),
            Some("LA16") => img::DynamicImage::ImageLumaA16(dynamic.to_luma_alpha16()),
            Some("RGB16") => img::DynamicImage::ImageRgb16(dynamic.to_rgb16()),
            Some("RGBA16") => img::DynamicImage::ImageRgba16(dynamic.to_rgba16()),
            Some("RGB32F") => img::DynamicImage::ImageRgb32F(dynamic.to_rgb32f()),
            Some("RGBA32F") => img::DynamicImage::ImageRgba32F(dynamic.to_rgba32f()),
            Some(value) => panic!("unknown intermediate mode {value}"),
            None => dynamic,
        };
        exercise_dynamic_buffer(&mut dynamic);
        if exercised_variants.insert(dynamic.color()) {
            exercise_dynamic_api(&dynamic);
        }
        let result = match row.action.as_str() {
            "convert" => dynamic,
            "fliph" => dynamic.fliph(),
            "flipv" => dynamic.flipv(),
            "rotate90" => dynamic.rotate90(),
            "rotate180" => dynamic.rotate180(),
            "rotate270" => dynamic.rotate270(),
            "crop" => dynamic.crop_imm(
                row.params["x"].as_u64().unwrap() as u32,
                row.params["y"].as_u64().unwrap() as u32,
                row.params["width"].as_u64().unwrap() as u32,
                row.params["height"].as_u64().unwrap() as u32,
            ),
            action => panic!("unknown operation {action}"),
        };
        let actual = match row.ref_mode.as_str() {
            "L" => result.to_luma8().into_raw(),
            "LA" => result.to_luma_alpha8().into_raw(),
            "RGB" => result.to_rgb8().into_raw(),
            "RGBA" => result.to_rgba8().into_raw(),
            mode => panic!("unsupported oracle operation mode {mode}"),
        };
        let expected = fs::read(manifest_dir.join(&row.ref_path)).unwrap();
        if actual != expected
            || actual.len() != row.ref_bytes
            || vec![result.width(), result.height()] != row.ref_size
        {
            let mismatch = actual
                .iter()
                .zip(&expected)
                .position(|(actual, expected)| actual != expected)
                .map(|index| {
                    format!(
                        " at byte {index}: actual {}, expected {}",
                        actual[index], expected[index]
                    )
                })
                .unwrap_or_default();
            failed.push(format!(
                "{}: actual {} bytes {}x{}, expected {} bytes {:?}{}",
                row.id,
                actual.len(),
                result.width(),
                result.height(),
                expected.len(),
                row.ref_size,
                mismatch,
            ));
        }
    }
    assert!(
        failed.is_empty(),
        "operation parity failures:\n{}",
        failed.join("\n")
    );
}

#[cfg(coverage)]
#[test]
fn test_internal_coverage_hooks() {
    img::__coverage_exercise_private_branches();
}

fn exercise_buffer<P>(buffer: &mut img::ImageBuffer<P, Vec<P::Subpixel>>)
where
    P: img::Pixel,
    P::Subpixel: std::fmt::Debug,
{
    use img::{GenericImage, GenericImageView, Primitive};

    let (width, height) = buffer.dimensions();
    assert_eq!(buffer.width(), width);
    assert_eq!(buffer.height(), height);
    assert_eq!(buffer.as_raw().len(), buffer.len());
    let mut clone = buffer.clone();
    clone.clone_from(buffer);

    let mut pixels = buffer.pixels();
    assert_eq!(pixels.size_hint().0, (width * height) as usize);
    assert_eq!(pixels.len(), (width * height) as usize);
    let _ = format!("{pixels:?}");
    let _ = pixels.clone().next();
    let _ = pixels.next_back();
    let mut rows = buffer.rows();
    let _ = rows.size_hint();
    let _ = rows.len();
    let _ = format!("{rows:?}");
    let _ = rows.clone().next();
    let _ = rows.next_back();
    let enumerate_pixels = buffer.enumerate_pixels();
    let _ = enumerate_pixels.size_hint();
    let _ = enumerate_pixels.len();
    let _ = format!("{enumerate_pixels:?}");
    let _ = enumerate_pixels.clone().count();
    let enumerate_rows = buffer.enumerate_rows();
    let _ = enumerate_rows.size_hint();
    let _ = enumerate_rows.len();
    let _ = format!("{enumerate_rows:?}");
    let _ = enumerate_rows.clone().count();

    let pixel = *buffer.get_pixel(0, 0);
    let one_pixel = img::ImageBuffer::from_pixel(1, 1, pixel);
    let mut one_pixel_iter = one_pixel.pixels();
    assert!(one_pixel_iter.next_back().is_some());
    assert!(one_pixel_iter.next_back().is_none());
    let mut one_row_iter = one_pixel.rows();
    assert!(one_row_iter.next_back().is_some());
    assert!(one_row_iter.next_back().is_none());
    let mut one_enumerate_rows = one_pixel.enumerate_rows();
    assert!(one_enumerate_rows.next().is_some());
    assert!(one_enumerate_rows.next().is_none());

    assert!(buffer.get_pixel_checked(0, 0).is_some());
    assert!(buffer.get_pixel_checked(width, 0).is_none());
    assert!(buffer.get_pixel_checked(0, height).is_none());
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = buffer.get_pixel(width, 0);
    }));
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = buffer.get_pixel(0, height);
    }));
    buffer.put_pixel(0, 0, pixel);
    buffer[(0, 0)] = pixel;
    assert!(buffer.get_pixel_mut_checked(0, 0).is_some());
    assert!(buffer.get_pixel_mut_checked(width, 0).is_none());
    assert!(buffer.get_pixel_mut_checked(0, height).is_none());
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = buffer.get_pixel_mut(width, 0);
    }));
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = buffer.get_pixel_mut(0, height);
    }));
    let _ = buffer[(0, 0)];
    {
        let mut pixels = buffer.pixels_mut();
        let _ = pixels.size_hint();
        let _ = pixels.len();
        let _ = format!("{pixels:?}");
        let _ = pixels.next_back();
    }
    {
        let mut rows = buffer.rows_mut();
        let _ = rows.size_hint();
        let _ = rows.len();
        let _ = format!("{rows:?}");
        let _ = rows.next();
        let _ = rows.next_back();
    }
    {
        let pixels = buffer.enumerate_pixels_mut();
        let _ = pixels.size_hint();
        let _ = pixels.len();
        let _ = format!("{pixels:?}");
        let _ = pixels.count();
    }
    {
        let rows = buffer.enumerate_rows_mut();
        let _ = rows.size_hint();
        let _ = rows.len();
        let _ = format!("{rows:?}");
        let _ = rows.count();
    }
    {
        let mut one_pixel_mut = img::ImageBuffer::from_pixel(1, 1, pixel);
        let mut pixels = one_pixel_mut.pixels_mut();
        assert!(pixels.next_back().is_some());
        assert!(pixels.next_back().is_none());
    }
    {
        let mut one_row_mut = img::ImageBuffer::from_pixel(1, 1, pixel);
        let mut rows = one_row_mut.rows_mut();
        assert!(rows.next_back().is_some());
        assert!(rows.next_back().is_none());
    }
    {
        let mut one_enumerate_rows_mut = img::ImageBuffer::from_pixel(1, 1, pixel);
        let mut rows = one_enumerate_rows_mut.enumerate_rows_mut();
        assert!(rows.next().is_some());
        assert!(rows.next().is_none());
    }

    assert!(GenericImageView::in_bounds(buffer, 0, 0));
    assert!(!GenericImageView::in_bounds(buffer, width, height));
    let mut view_pixels = GenericImageView::pixels(buffer);
    let _ = view_pixels.clone();
    while view_pixels.next().is_some() {}
    assert!(view_pixels.next().is_none());
    let _ = GenericImageView::buffer_like(buffer);
    let _ = GenericImageView::buffer_with_dimensions(buffer, 1, 1);
    GenericImage::copy_from(buffer, &clone, 0, 0).unwrap();
    let _ = GenericImage::get_pixel_mut(buffer, 0, 0);
    #[allow(deprecated)]
    GenericImage::blend_pixel(buffer, 0, 0, pixel);
    let mut too_small = img::ImageBuffer::<P, Vec<P::Subpixel>>::new(1, 1);
    assert!(GenericImage::copy_from(&mut too_small, &clone, 0, 0).is_err());

    let mut copy = img::ImageBuffer::from_pixel(3, 3, pixel);
    let rects = [
        (img::Rect::new(0, 0, 1, 1), 1, 1),
        (img::Rect::new(0, 1, 1, 1), 1, 0),
        (img::Rect::new(1, 0, 1, 1), 0, 1),
        (img::Rect::new(1, 1, 1, 1), 0, 0),
    ];
    for (source, x, y) in rects {
        assert!(GenericImage::copy_within(&mut copy, source, x, y));
    }
    assert!(!GenericImage::copy_within(
        &mut copy,
        img::Rect::new(3, 0, 1, 1),
        0,
        0,
    ));
    assert!(!GenericImage::copy_within(
        &mut copy,
        img::Rect::new(0, 3, 1, 1),
        0,
        0,
    ));
    assert!(!GenericImage::copy_within(
        &mut copy,
        img::Rect::new(0, 0, 3, 3),
        1,
        1,
    ));

    let generated = img::ImageBuffer::from_fn(2, 2, |_x, _y| pixel);
    assert_eq!(
        generated.into_vec().len(),
        4 * usize::from(P::CHANNEL_COUNT)
    );
    assert!(img::ImageBuffer::<P, Vec<P::Subpixel>>::from_vec(1, 1, vec![]).is_none());
    let default = img::ImageBuffer::<P, Vec<P::Subpixel>>::default();
    assert_eq!(default.dimensions(), (0, 0));
    assert_eq!(default.rows().count(), 0);
    let mut default_mut = default;
    assert_eq!(default_mut.rows_mut().count(), 0);
    let _: &mut [P::Subpixel] = &mut default_mut;

    let mut local = pixel;
    let _ = local.channels();
    let _ = local.channels_mut();
    let _ = local.alpha();
    #[allow(deprecated)]
    let _ = local.channels4();
    let _ = local.to_rgb();
    let _ = local.to_rgba();
    let _ = local.to_luma();
    let _ = local.to_luma_alpha();
    let _ = local.map(|value| value);
    local.apply(|value| value);
    let _ = local.map_with_alpha(|value| value, |alpha| alpha);
    local.apply_with_alpha(|value| value, |alpha| alpha);
    let _ = local.map_without_alpha(|value| value);
    local.apply_without_alpha(|value| value);
    let _ = local.map2(&pixel, |left, _right| left);
    local.apply2(&pixel, |left, _right| left);
    local.invert();
    local.blend(&pixel);
    let _ = P::Subpixel::DEFAULT_MIN_VALUE;
}

fn exercise_dynamic_buffer(image: &mut img::DynamicImage) {
    match image {
        img::DynamicImage::ImageLuma8(buffer) => exercise_buffer(buffer),
        img::DynamicImage::ImageLumaA8(buffer) => exercise_buffer(buffer),
        img::DynamicImage::ImageRgb8(buffer) => exercise_buffer(buffer),
        img::DynamicImage::ImageRgba8(buffer) => exercise_buffer(buffer),
        img::DynamicImage::ImageLuma16(buffer) => exercise_buffer(buffer),
        img::DynamicImage::ImageLumaA16(buffer) => exercise_buffer(buffer),
        img::DynamicImage::ImageRgb16(buffer) => exercise_buffer(buffer),
        img::DynamicImage::ImageRgba16(buffer) => exercise_buffer(buffer),
        img::DynamicImage::ImageRgb32F(buffer) => exercise_buffer(buffer),
        img::DynamicImage::ImageRgba32F(buffer) => exercise_buffer(buffer),
        _ => panic!("unsupported dynamic image variant"),
    }
}

fn exercise_dynamic_api(image: &img::DynamicImage) {
    use img::{GenericImage, GenericImageView};

    let (width, height) = (image.width(), image.height());
    let supported = [
        img::ColorType::L8,
        img::ColorType::La8,
        img::ColorType::Rgb8,
        img::ColorType::Rgba8,
        img::ColorType::L16,
        img::ColorType::La16,
        img::ColorType::Rgb16,
        img::ColorType::Rgba16,
        img::ColorType::Rgb32F,
        img::ColorType::Rgba32F,
    ];
    for color in supported {
        let created = img::DynamicImage::new(1, 1, color);
        assert_eq!(created.color(), color);
    }
    let mut same = image.clone();
    same.clone_from(image);
    let mut different = if image.color() == img::ColorType::L8 {
        img::DynamicImage::new_rgba8(1, 1)
    } else {
        img::DynamicImage::new_luma8(1, 1)
    };
    different.clone_from(image);

    let _ = image.to_rgb8();
    let _ = image.to_rgba8();
    let _ = image.to_luma8();
    let _ = image.to_luma_alpha8();
    let _ = image.to_rgb16();
    let _ = image.to_rgba16();
    let _ = image.to_luma16();
    let _ = image.to_luma_alpha16();
    let _ = image.to_rgb32f();
    let _ = image.to_rgba32f();
    let _: img::RgbaImage = image.to::<img::Rgba<u8>>();
    let _ = image.clone().into_rgb8();
    let _ = image.clone().into_rgba8();
    let _ = image.clone().into_luma8();
    let _ = image.clone().into_luma_alpha8();
    let _ = image.clone().into_rgb16();
    let _ = image.clone().into_rgba16();
    let _ = image.clone().into_luma16();
    let _ = image.clone().into_luma_alpha16();
    let _ = image.clone().into_rgb32f();
    let _ = image.clone().into_rgba32f();

    let mut mutable = image.clone();
    let _ = mutable.as_rgb8();
    let _ = mutable.as_mut_rgb8();
    let _ = mutable.as_rgba8();
    let _ = mutable.as_mut_rgba8();
    let _ = mutable.as_luma8();
    let _ = mutable.as_mut_luma8();
    let _ = mutable.as_luma_alpha8();
    let _ = mutable.as_mut_luma_alpha8();
    let _ = mutable.as_rgb16();
    let _ = mutable.as_mut_rgb16();
    let _ = mutable.as_rgba16();
    let _ = mutable.as_mut_rgba16();
    let _ = mutable.as_luma16();
    let _ = mutable.as_mut_luma16();
    let _ = mutable.as_luma_alpha16();
    let _ = mutable.as_mut_luma_alpha16();
    let _ = mutable.as_rgb32f();
    let _ = mutable.as_mut_rgb32f();
    let _ = mutable.as_rgba32f();
    let _ = mutable.as_mut_rgba32f();
    assert!(!image.as_bytes().is_empty());
    assert_eq!(image.color().has_alpha(), image.has_alpha());

    let decoded = image.clone().into_decoded();
    let roundtrip = img::DynamicImage::from_decoded(&decoded).unwrap();
    assert_eq!(roundtrip.as_bytes(), image.as_bytes());
    let _ = image.crop_imm(0, 0, 1, 1);
    assert_eq!(GenericImageView::dimensions(image), (width, height));
    let pixel = GenericImageView::get_pixel(image, 0, 0);
    let mut writable = image.clone();
    GenericImage::put_pixel(&mut writable, 0, 0, pixel);
    #[allow(deprecated)]
    GenericImage::blend_pixel(&mut writable, 0, 0, pixel);
    let _: img::RgbImage = image.clone().into();
    let _: img::RgbaImage = image.clone().into();
    let _: img::GrayImage = image.clone().into();
    let _: img::GrayAlphaImage = image.clone().into();
    let _: img::DynamicImage = image.to_rgb8().into();
    let _: img::DynamicImage = image.to_rgba8().into();
    let _: img::DynamicImage = image.to_luma8().into();
    let _: img::DynamicImage = image.to_luma_alpha8().into();

    let invalid_mode = img::DecodedImage {
        width: 1,
        height: 1,
        pixels: vec![0, 0, 0],
        color: img::ColorType::Rgb8,
        mode: img::ImageMode::L8,
        palette: None,
    };
    assert!(img::DynamicImage::from_decoded(&invalid_mode).is_none());
    let invalid_palette = img::DecodedImage {
        mode: img::ImageMode::Rgb8,
        palette: Some(img::ImagePalette::default()),
        ..invalid_mode
    };
    assert!(img::DynamicImage::from_decoded(&invalid_palette).is_none());
    for color in [
        img::ColorType::Cmyk8,
        img::ColorType::L32F,
        img::ColorType::L32I,
    ] {
        let decoded =
            img::DecodedImage::new(1, 1, vec![0; usize::from(color.bytes_per_pixel())], color);
        assert!(img::DynamicImage::from_decoded(&decoded).is_none());
    }
    let short = img::DecodedImage::new(1, 1, vec![], img::ColorType::Rgb8);
    assert!(img::DynamicImage::from_decoded(&short).is_none());
    let short_alpha = img::DecodedImage::new(1, 1, vec![], img::ColorType::La8);
    assert!(img::DynamicImage::from_decoded(&short_alpha).is_none());
}

fn exercise_primitive<T>(value: T)
where
    T: img::Primitive,
{
    let _ = value.to_f32();
    let _ = value.to_u64();
    let _ = T::from_f32(0.5);
    let _ = T::from_u64(1);
}

fn exercise_enlargeable<T>(value: T)
where
    T: img::Enlargeable,
{
    let larger = value.to_larger();
    let _ = T::clamp_from(larger);
}

fn exercise_type_metadata() {
    use img::{EncodableLayout, ExtendedColorType as E, GenericImage, Pixel};

    let _ = std::panic::catch_unwind(|| img::DynamicImage::new(1, 1, img::ColorType::Cmyk8));
    let _ = std::panic::catch_unwind(|| img::DynamicImage::new(1, 1, img::ColorType::L32F));
    let _ = std::panic::catch_unwind(|| img::DynamicImage::new(1, 1, img::ColorType::L32I));
    let mut dynamic = img::DynamicImage::new_rgba8(1, 1);
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        #[allow(deprecated)]
        let _ = GenericImage::get_pixel_mut(&mut dynamic, 0, 0);
    }));
    let immutable = img::RgbaImage::new(1, 1);
    let _ = std::panic::catch_unwind(|| immutable.get_pixel(1, 0));
    let mut mutable = img::RgbaImage::new(1, 1);
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = mutable.get_pixel_mut(0, 1);
    }));
    let _ = std::panic::catch_unwind(|| img::RgbaImage::new(u32::MAX, u32::MAX));

    let mut luma: img::Luma<u8> = [1].into();
    luma[0] = 2;
    let mut luma_alpha: img::LumaA<u8> = [1, 2].into();
    luma_alpha[1] = 3;
    let mut rgb: img::Rgb<u8> = [1, 2, 3].into();
    rgb[2] = 4;
    let mut rgba: img::Rgba<u8> = [1, 2, 3, 4].into();
    rgba[3] = 5;
    #[allow(deprecated)]
    {
        let _ = <img::Luma<u8> as Pixel>::from_channels(1, 2, 3, 4);
        let _ = <img::LumaA<u8> as Pixel>::from_channels(1, 2, 3, 4);
        let _ = <img::Rgb<u8> as Pixel>::from_channels(1, 2, 3, 4);
        let _ = <img::Rgba<u8> as Pixel>::from_channels(1, 2, 3, 4);
    }
    #[derive(Clone, Copy)]
    struct DefaultAlphaPixel([u8; 2]);
    impl Pixel for DefaultAlphaPixel {
        type Subpixel = u8;
        const CHANNEL_COUNT: u8 = 2;
        const COLOR_MODEL: &'static str = "YA";
        const HAS_ALPHA: bool = true;
        fn channels(&self) -> &[u8] {
            &self.0
        }
        fn channels_mut(&mut self) -> &mut [u8] {
            &mut self.0
        }
        #[allow(deprecated)]
        fn channels4(&self) -> (u8, u8, u8, u8) {
            (self.0[0], 255, 255, self.0[1])
        }
        #[allow(deprecated)]
        fn from_channels(a: u8, _b: u8, _c: u8, d: u8) -> Self {
            Self([a, d])
        }
        fn from_slice(_slice: &[u8]) -> &Self {
            panic!("not needed by coverage matrix")
        }
        fn from_slice_mut(_slice: &mut [u8]) -> &mut Self {
            panic!("not needed by coverage matrix")
        }
        fn to_rgb(&self) -> img::Rgb<u8> {
            img::Rgb([self.0[0]; 3])
        }
        fn to_rgba(&self) -> img::Rgba<u8> {
            img::Rgba([self.0[0], self.0[0], self.0[0], self.0[1]])
        }
        fn to_luma(&self) -> img::Luma<u8> {
            img::Luma([self.0[0]])
        }
        fn to_luma_alpha(&self) -> img::LumaA<u8> {
            img::LumaA(self.0)
        }
        fn map<F>(&self, mut f: F) -> Self
        where
            F: FnMut(u8) -> u8,
        {
            Self([f(self.0[0]), f(self.0[1])])
        }
        fn apply<F>(&mut self, mut f: F)
        where
            F: FnMut(u8) -> u8,
        {
            self.0 = [f(self.0[0]), f(self.0[1])];
        }
        fn map_with_alpha<F, G>(&self, mut f: F, mut g: G) -> Self
        where
            F: FnMut(u8) -> u8,
            G: FnMut(u8) -> u8,
        {
            Self([f(self.0[0]), g(self.0[1])])
        }
        fn apply_with_alpha<F, G>(&mut self, mut f: F, mut g: G)
        where
            F: FnMut(u8) -> u8,
            G: FnMut(u8) -> u8,
        {
            self.0 = [f(self.0[0]), g(self.0[1])];
        }
        fn map2<F>(&self, other: &Self, mut f: F) -> Self
        where
            F: FnMut(u8, u8) -> u8,
        {
            Self([f(self.0[0], other.0[0]), f(self.0[1], other.0[1])])
        }
        fn apply2<F>(&mut self, other: &Self, mut f: F)
        where
            F: FnMut(u8, u8) -> u8,
        {
            self.0 = [f(self.0[0], other.0[0]), f(self.0[1], other.0[1])];
        }
        fn invert(&mut self) {
            self.0 = [255 - self.0[0], 255 - self.0[1]];
        }
        fn blend(&mut self, other: &Self) {
            *self = *other;
        }
    }
    assert_eq!(DefaultAlphaPixel([1, 7]).alpha(), 7);

    let mut gray_bg = img::LumaA([10u8, 100]);
    gray_bg.blend(&img::LumaA([20, 255]));
    gray_bg.blend(&img::LumaA([20, 0]));
    gray_bg.blend(&img::LumaA([20, 128]));
    let mut gray_zero = img::LumaA([0.0f32, -1.0]);
    gray_zero.blend(&img::LumaA([0.0, 0.5]));
    let mut rgba_bg = img::Rgba([10u8, 20, 30, 100]);
    rgba_bg.blend(&img::Rgba([20, 30, 40, 255]));
    rgba_bg.blend(&img::Rgba([20, 30, 40, 0]));
    rgba_bg.blend(&img::Rgba([20, 30, 40, 128]));
    let mut rgba_zero = img::Rgba([0.0f32, 0.0, 0.0, -1.0]);
    rgba_zero.blend(&img::Rgba([0.0, 0.0, 0.0, 0.5]));
    let mut flat_luma = img::Luma([1u8]);
    flat_luma.blend(&img::Luma([2]));
    let mut flat_rgb = img::Rgb([1u8, 2, 3]);
    flat_rgb.blend(&img::Rgb([4, 5, 6]));

    let colors = [
        img::ColorType::L8,
        img::ColorType::La8,
        img::ColorType::Rgb8,
        img::ColorType::Rgba8,
        img::ColorType::Cmyk8,
        img::ColorType::L16,
        img::ColorType::La16,
        img::ColorType::Rgb16,
        img::ColorType::Rgba16,
        img::ColorType::Rgb32F,
        img::ColorType::Rgba32F,
        img::ColorType::L32F,
        img::ColorType::L32I,
    ];
    for color in colors {
        assert_eq!(
            color.bits_per_pixel(),
            u16::from(color.bytes_per_pixel()) * 8
        );
        let _ = color.has_alpha();
        let _ = color.has_color();
        let _ = color.channel_count();
        let _: E = color.into();
    }
    let extended = [
        E::A8,
        E::L1,
        E::La1,
        E::Rgb1,
        E::Rgba1,
        E::L2,
        E::La2,
        E::Rgb2,
        E::Rgba2,
        E::L4,
        E::La4,
        E::Rgb4,
        E::Rgba4,
        E::Rgb5x1,
        E::L8,
        E::La8,
        E::Rgb8,
        E::Rgba8,
        E::L16,
        E::La16,
        E::Rgb16,
        E::Rgba16,
        E::Bgr8,
        E::Bgra8,
        E::Rgb32F,
        E::Rgba32F,
        E::L32F,
        E::L32I,
        E::Cmyk8,
        E::Cmyk16,
        E::Unknown(7),
    ];
    for color in extended {
        assert!(color.channel_count() > 0);
        assert!(color.bits_per_pixel() > 0);
        let _ = color.color_type();
    }

    for mode in [
        img::ImageMode::La16,
        img::ImageMode::Rgb16,
        img::ImageMode::Rgba16,
        img::ImageMode::Rgb32F,
        img::ImageMode::Rgba32F,
        img::ImageMode::I32,
    ] {
        let _ = mode.color_type();
    }
    for (rgb, alpha) in [
        (vec![], vec![]),
        (vec![0], vec![]),
        (vec![0; 257 * 3], vec![]),
        (vec![0, 0, 0], vec![0, 0]),
    ] {
        assert!(img::ImagePalette::new(rgb, alpha).is_err());
    }
    let palette = img::ImagePalette::new(vec![0, 0, 0], vec![255]).unwrap();
    let invalid_images = [
        img::DecodedImage::new(0, 1, vec![], img::ColorType::L8),
        img::DecodedImage {
            width: 1,
            height: 1,
            pixels: vec![0],
            color: img::ColorType::Rgb8,
            mode: img::ImageMode::L8,
            palette: None,
        },
        img::DecodedImage::with_mode(1, 1, vec![1], img::ImageMode::P8)
            .with_palette(palette.clone()),
        img::DecodedImage::new(1, 1, vec![0], img::ColorType::L8).with_palette(palette),
    ];
    for image in invalid_images {
        assert!(image.validate().is_err());
    }
    let valid = img::DecodedImage::new(1, 1, vec![0, 0, 0], img::ColorType::Rgb8);
    let empty = img::DecodedSequence {
        width: 1,
        height: 1,
        frames: vec![],
        loop_count: None,
        background: None,
    };
    assert!(empty.validate().is_err());
    let outside = img::DecodedSequence {
        width: 1,
        height: 1,
        frames: vec![img::DecodedFrame {
            image: valid.clone(),
            left: 1,
            top: 0,
            duration_ms: 0,
            disposal: img::FrameDisposal::Unspecified,
            interlaced: false,
        }],
        loop_count: None,
        background: None,
    };
    assert!(outside.validate().is_err());
    assert_eq!(
        img::detect_format(b"\0\0\0\x18ftypavif\0\0\0\0"),
        Some(img::ImageFormat::Avif)
    );
    assert!(img::decode(b"\0\0\0\x18ftypavif\0\0\0\0").is_none());
    assert!(img::encode(&valid, img::ImageFormat::Avif, &Default::default()).is_none());
    assert!(img::encode_default(&valid, img::ImageFormat::Avif).is_none());
    let zero_png = img::DecodedImage::new(0, 1, vec![], img::ColorType::L8);
    assert!(img::codecs::png::encode::encode(&zero_png, &Default::default()).is_none());
    let unsupported_tiff = img::DecodedImage::new(1, 1, vec![0; 4], img::ColorType::La16);
    assert!(img::codecs::tiff::encode::encode(&unsupported_tiff, &Default::default()).is_none());
    let unsupported_webp = img::DecodedImage::new(1, 1, vec![0, 0], img::ColorType::La8);
    assert!(img::codecs::webp::encode::encode(&unsupported_webp, &Default::default()).is_none());
    let mut bad_webp_metadata = img::encode_options::EncodeOptions::default();
    bad_webp_metadata
        .extra
        .insert("xmp_hex".to_owned(), "f".to_owned());
    assert!(img::codecs::webp::encode::encode(&valid, &bad_webp_metadata).is_none());
    let mut lossless_webp = img::encode_options::EncodeOptions::default();
    lossless_webp.lossless = Some(true);
    assert!(img::codecs::webp::encode::encode(&unsupported_webp, &lossless_webp).is_none());

    exercise_primitive(1u8);
    exercise_primitive(1u16);
    exercise_primitive(1u32);
    exercise_primitive(1u64);
    exercise_primitive(1u128);
    exercise_primitive(1usize);
    exercise_primitive(0.5f32);
    exercise_primitive(0.5f64);
    exercise_enlargeable(1u8);
    exercise_enlargeable(1u16);
    exercise_enlargeable(1u32);
    exercise_enlargeable(1u64);
    exercise_enlargeable(1usize);
    exercise_enlargeable(0.5f32);
    let _ = <u8 as img::FromPrimitive<f32>>::from_primitive(0.5);
    let _ = <u16 as img::FromPrimitive<f32>>::from_primitive(0.5);
    let _ = <u8 as img::FromPrimitive<u16>>::from_primitive(257);
    let _ = <f32 as img::FromPrimitive<u16>>::from_primitive(1);
    let _ = <f32 as img::FromPrimitive<u8>>::from_primitive(1);
    let _ = <u16 as img::FromPrimitive<u8>>::from_primitive(1);
    assert_eq!([1u8, 2].as_slice().as_bytes(), &[1, 2]);
    let _ = [1u16, 2].as_slice().as_bytes();
    let _ = [0.25f32, 0.5].as_slice().as_bytes();

    let paths = [
        "a.jpg", "a.png", "a.gif", "a.bmp", "a.webp", "a.tif", "a.ico", "a.avif",
    ];
    for path in paths {
        assert!(img::ImageFormat::from_path(path).is_ok());
    }
    assert!(img::ImageFormat::from_path("a.unknown").is_err());
    let errors = [
        img::ImageError::Dimensions,
        img::ImageError::Unsupported("x".to_owned()),
        img::ImageError::Parameter("x".to_owned()),
        img::ImageError::IoError("x".to_owned()),
    ];
    for error in errors {
        assert!(!error.to_string().is_empty());
    }
    let _ = img::Rect::new(0, 0, 1, 1);
    let _ = img::encode_options::EncodeOptions::none();
}

// ── Manifest Coverage ────────────────────────────────────────────────────

#[test]
fn test_coverage_matrix() {
    let matrix = coverage_matrix().expect(
        "coverage_matrix.json is required; run scripts/generate_decode_refs.py to regenerate it",
    );

    let s = &matrix.summary;
    eprintln!(
        "Coverage: {}/{} decode active, {} planned, {} encode not wired, {} operations, {} assets",
        s.decode_active,
        s.decode_rows,
        s.decode_planned,
        s.encode_not_wired,
        s.operation_rows,
        s.assets_available
    );

    assert!(s.total_rows > 0, "Matrix must have rows");
    assert_eq!(
        s.total_rows,
        s.decode_rows + s.encode_rows + s.operation_rows
    );
}
