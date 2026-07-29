//! Coverage matrix tests — driven by tests/fixtures/coverage_matrix.json
//! Each row in the matrix is one test assertion.
//! Decode: load asset → decode → compare pixel bytes with PIL reference bytes.
//! Encode: decode reference → encode with params → decode → compare pixel bytes.

use std::collections::{HashMap, HashSet, hash_map::Entry};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use bytemuck as _;
use image_slash_star as img;

mod support;

use support::json::{self, FromJson, Object, Value};

static COVERAGE_MATRIX: OnceLock<Option<CoverageMatrix>> = OnceLock::new();

#[track_caller]
fn require_some<T>(value: Option<T>, context: &str) -> T {
    match value {
        Some(value) => value,
        None => panic!("{context}"),
    }
}

#[track_caller]
fn require_ok<T, E: std::fmt::Debug>(value: Result<T, E>, context: &str) -> T {
    match value {
        Ok(value) => value,
        Err(error) => panic!("{context}: {error:?}"),
    }
}

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

            let contents = require_ok(
                fs::read_to_string(&matrix_path),
                "coverage matrix must be readable",
            );
            Some(require_ok(
                json::from_str(&contents),
                "coverage matrix must be valid JSON",
            ))
        })
        .as_ref()
}

#[derive(Debug)]
#[allow(dead_code)]
struct CoverageMatrix {
    formats: HashMap<String, FormatData>,
    summary: Summary,
}

#[derive(Debug)]
#[allow(dead_code)]
struct FormatData {
    decode: Vec<DecodeRow>,
    encode: Vec<EncodeRow>,
}

#[derive(Debug)]
#[allow(dead_code)]
struct DecodeRow {
    id: String,
    row_type: String,
    format: String,
    category: String,
    status: String,
    asset: Option<String>,
    asset_path: Option<String>,
    expect_error: Option<bool>,
    oracle_status: Option<String>,
    oracle_error_type: Option<String>,
    oracle_error_message: Option<String>,
    verify_status: String,
    verify_error_type: Option<String>,
    verify_error_message: Option<String>,
    ref_mode: Option<String>,
    ref_size: Option<Vec<u32>>,
    ref_frame_count: Option<u32>,
    ref_is_animated: Option<bool>,
    ref_path: Option<String>,
    ref_bytes: Option<usize>,
    sequence: Option<SequenceParityRef>,
}

#[derive(Debug)]
struct SequenceParityRef {
    loop_count: Option<u32>,
    frames: Vec<FrameParityRef>,
}

#[derive(Debug)]
struct FrameParityRef {
    index: usize,
    ref_path: String,
    ref_bytes: usize,
    ref_mode: String,
    ref_size: Vec<u32>,
    duration_ms: u32,
}

#[derive(Debug)]
#[allow(dead_code)]
struct EncodeRow {
    id: String,
    row_type: String,
    format: String,
    params: HashMap<String, Value>,
    description: Option<String>,
    status: String,
    expect_error: bool,
    oracle_status: Option<String>,
    oracle_error_type: Option<String>,
    oracle_error_message: Option<String>,
    source_format: Option<String>,
    source_asset: Option<String>,
    ref_bytes: Option<usize>,
    ref_mode: Option<String>,
    ref_size: Option<Vec<u32>>,
    ref_path: Option<String>,
    encoded_ref_path: Option<String>,
    encoded_ref_bytes: Option<usize>,
}

#[derive(Debug)]
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
}

#[cfg(coverage)]
#[derive(Debug)]
struct Av1EntropyDocument {
    format_version: u32,
    oracle: Av1EntropyOracle,
    input_hex: String,
    partition_422_inputs: HashMap<String, String>,
    records: Vec<Av1EntropyRecord>,
}

#[cfg(coverage)]
#[derive(Debug)]
struct Av1EntropyOracle {
    implementation: String,
    version: String,
    commit: String,
    source_files: Vec<String>,
}

#[cfg(coverage)]
#[derive(Debug, PartialEq, Eq)]
struct Av1EntropyRecord {
    case: String,
    step: u32,
    value: i32,
    byte_position: usize,
    difference: u64,
    range: u32,
    count: i32,
    cdf: Vec<u16>,
}

#[cfg(coverage)]
#[derive(Debug)]
struct Av1ReconstructionDocument {
    format_version: u32,
    oracle: Av1ReconstructionOracle,
    scope: String,
    cases: Vec<Av1ReconstructionCase>,
}

#[cfg(coverage)]
#[derive(Debug)]
struct Av1ReconstructionOracle {
    implementation: String,
    version: String,
    commit: String,
    pillow_avif: String,
    pillow_codecs: String,
    pillow_libyuv: u32,
}

#[cfg(coverage)]
#[derive(Debug)]
struct Av1ReconstructionCase {
    fixture: String,
    portable_color: Av1PortableColor,
    pillow: Av1PillowOutput,
    partition_blocks: Vec<Av1PartitionBlock>,
    decoded_planes: Vec<Av1ReconstructionPlane>,
    entropy_operations: Vec<Av1ReconstructionEntropyOperation>,
}

#[cfg(coverage)]
#[derive(Debug, PartialEq, Eq)]
struct Av1PartitionBlock {
    poc: i32,
    x: i32,
    y: i32,
    level: u32,
    context: u32,
    partition: u32,
    range: u32,
}

#[cfg(coverage)]
#[derive(Debug)]
struct Av1PortableColor {
    width: u32,
    height: u32,
    bit_depth: u32,
    monochrome: bool,
    color_primaries: u32,
    transfer_characteristics: u32,
    matrix_coefficients: u32,
    color_range: bool,
    subsampling_x: bool,
    subsampling_y: bool,
}

#[cfg(coverage)]
#[derive(Debug)]
struct Av1PillowOutput {
    mode: String,
    size: [u32; 2],
    bytes: usize,
    sha256: String,
    row_bytes: Vec<String>,
}

#[cfg(coverage)]
#[derive(Debug)]
struct Av1ReconstructionPlane {
    name: String,
    width: u32,
    height: u32,
    row_bytes: Vec<String>,
}

#[cfg(coverage)]
#[derive(Debug, PartialEq, Eq)]
struct Av1ReconstructionEntropyOperation {
    operation: String,
    parameter: i32,
    step: u32,
    value: i32,
    byte_position: usize,
    difference: u64,
    range: u32,
    count: i32,
    cdf: Vec<u16>,
}

macro_rules! json_object {
    ($type:ty { $($field:ident),+ $(,)? }) => {
        impl FromJson for $type {
            fn from_json(value: Value) -> Result<Self, support::json::Error> {
                let mut object = Object::new(value)?;
                Ok(Self {
                    $($field: object.take(stringify!($field))?,)+
                })
            }
        }
    };
}

json_object!(CoverageMatrix { formats, summary });
json_object!(FormatData { decode, encode });

impl FromJson for DecodeRow {
    fn from_json(value: Value) -> Result<Self, support::json::Error> {
        let mut object = Object::new(value)?;
        Ok(Self {
            id: object.take("id")?,
            row_type: object.take("type")?,
            format: object.take("format")?,
            category: object.take("category")?,
            status: object.take("status")?,
            asset: object.take("asset")?,
            asset_path: object.take("asset_path")?,
            expect_error: object.take("expect_error")?,
            oracle_status: object.take("oracle_status")?,
            oracle_error_type: object.take("oracle_error_type")?,
            oracle_error_message: object.take("oracle_error_message")?,
            verify_status: object.take("verify_status")?,
            verify_error_type: object.take("verify_error_type")?,
            verify_error_message: object.take("verify_error_message")?,
            ref_mode: object.take("ref_mode")?,
            ref_size: object.take("ref_size")?,
            ref_frame_count: object.take("ref_frame_count")?,
            ref_is_animated: object.take("ref_is_animated")?,
            ref_path: object.take("ref_path")?,
            ref_bytes: object.take("ref_bytes")?,
            sequence: object.take("sequence")?,
        })
    }
}

json_object!(SequenceParityRef { loop_count, frames });
json_object!(FrameParityRef {
    index,
    ref_path,
    ref_bytes,
    ref_mode,
    ref_size,
    duration_ms,
});

impl FromJson for EncodeRow {
    fn from_json(value: Value) -> Result<Self, support::json::Error> {
        let mut object = Object::new(value)?;
        Ok(Self {
            id: object.take("id")?,
            row_type: object.take("type")?,
            format: object.take("format")?,
            params: object.take("params")?,
            description: object.take("description")?,
            status: object.take("status")?,
            expect_error: object.take_or_default("expect_error")?,
            oracle_status: object.take("oracle_status")?,
            oracle_error_type: object.take("oracle_error_type")?,
            oracle_error_message: object.take("oracle_error_message")?,
            source_format: object.take("source_format")?,
            source_asset: object.take("source_asset")?,
            ref_bytes: object.take("ref_bytes")?,
            ref_mode: object.take("ref_mode")?,
            ref_size: object.take("ref_size")?,
            ref_path: object.take("ref_path")?,
            encoded_ref_path: object.take("encoded_ref_path")?,
            encoded_ref_bytes: object.take("encoded_ref_bytes")?,
        })
    }
}

json_object!(Summary {
    total_rows,
    decode_rows,
    encode_rows,
    formats,
    assets_available,
    decode_active,
    decode_planned,
    encode_not_wired,
});

#[cfg(coverage)]
json_object!(Av1EntropyDocument {
    format_version,
    oracle,
    input_hex,
    partition_422_inputs,
    records,
});
#[cfg(coverage)]
json_object!(Av1EntropyOracle {
    implementation,
    version,
    commit,
    source_files,
});
#[cfg(coverage)]
json_object!(Av1EntropyRecord {
    case,
    step,
    value,
    byte_position,
    difference,
    range,
    count,
    cdf,
});
#[cfg(coverage)]
json_object!(Av1ReconstructionDocument {
    format_version,
    oracle,
    scope,
    cases,
});
#[cfg(coverage)]
json_object!(Av1ReconstructionOracle {
    implementation,
    version,
    commit,
    pillow_avif,
    pillow_codecs,
    pillow_libyuv,
});
#[cfg(coverage)]
json_object!(Av1ReconstructionCase {
    fixture,
    portable_color,
    pillow,
    partition_blocks,
    decoded_planes,
    entropy_operations,
});
#[cfg(coverage)]
json_object!(Av1PartitionBlock {
    poc,
    x,
    y,
    level,
    context,
    partition,
    range,
});
#[cfg(coverage)]
json_object!(Av1PortableColor {
    width,
    height,
    bit_depth,
    monochrome,
    color_primaries,
    transfer_characteristics,
    matrix_coefficients,
    color_range,
    subsampling_x,
    subsampling_y,
});
#[cfg(coverage)]
json_object!(Av1PillowOutput {
    mode,
    size,
    bytes,
    sha256,
    row_bytes,
});
#[cfg(coverage)]
json_object!(Av1ReconstructionPlane {
    name,
    width,
    height,
    row_bytes,
});
#[cfg(coverage)]
json_object!(Av1ReconstructionEntropyOperation {
    operation,
    parameter,
    step,
    value,
    byte_position,
    difference,
    range,
    count,
    cdf,
});

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

fn option_text(value: &Value) -> String {
    value
        .as_str()
        .map_or_else(|| value.to_string(), str::to_owned)
}

fn extra_encode_options(params: &HashMap<String, Value>) -> HashMap<String, String> {
    params
        .iter()
        .filter(|(key, _)| key.as_str() != "advanced")
        .map(|(key, value)| (key.clone(), option_text(value)))
        .collect()
}

fn advanced_encode_options(params: &HashMap<String, Value>) -> Vec<(String, String)> {
    params
        .get("advanced")
        .and_then(Value::as_object)
        .map(|values| {
            values
                .iter()
                .map(|(key, value)| (key.clone(), option_text(value)))
                .collect()
        })
        .unwrap_or_default()
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

fn assert_png_contract(params: &HashMap<String, Value>, encoded: &[u8]) -> Result<(), String> {
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

    if let Some(expected) = params.get("bit_depth").and_then(Value::as_u64)
        && u64::from(bit_depth) != expected
    {
        return Err(format!(
            "PNG depth mismatch: encoded {bit_depth}, requested {expected}"
        ));
    }
    let color_request = params
        .get("color_type")
        .or_else(|| params.get("color"))
        .and_then(Value::as_str);
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
        .and_then(Value::as_bool);
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
        let kind_start = offset.checked_add(4).ok_or("PNG chunk offset overflow")?;
        let kind_end = offset.checked_add(8).ok_or("PNG chunk offset overflow")?;
        let kind = encoded
            .get(kind_start..kind_end)
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
        if params.get(option).and_then(Value::as_bool) == Some(true) && !chunks.contains(&kind) {
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

fn assert_gif_contract(params: &HashMap<String, Value>, encoded: &[u8]) -> Result<(), String> {
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
                let packed_offset = offset.checked_add(9).ok_or("GIF image overflow")?;
                let image_packed = *encoded.get(packed_offset).ok_or("truncated GIF image")?;
                frames = frames.wrapping_add(1);
                image_local.push(image_packed & 0x80 != 0);
                image_interlace.push(image_packed & 0x40 != 0);
                offset = offset.checked_add(10).ok_or("GIF image overflow")?;
                if image_packed & 0x80 != 0 {
                    offset = offset
                        .checked_add(3usize << (usize::from(image_packed & 7) + 1))
                        .ok_or("GIF local color table overflow")?;
                }
                offset = offset.checked_add(1).ok_or("GIF image overflow")?;
                offset = skip_gif_sub_blocks(encoded, offset).ok_or("truncated GIF image data")?;
            }
            0x21 => {
                let label_offset = offset.checked_add(1).ok_or("GIF extension overflow")?;
                let label = *encoded.get(label_offset).ok_or("truncated GIF extension")?;
                if label == 0xf9 {
                    let size_offset = offset.checked_add(2).ok_or("GIF GCE overflow")?;
                    if *encoded.get(size_offset).ok_or("truncated GIF GCE")? != 4 {
                        return Err("invalid GIF GCE size".to_owned());
                    }
                    let packed_offset = offset.checked_add(3).ok_or("GIF GCE overflow")?;
                    let gce_packed = *encoded.get(packed_offset).ok_or("truncated GIF GCE")?;
                    gce_disposals.push((gce_packed >> 2) & 7);
                    gce_transparency.push(gce_packed & 1 != 0);
                    offset = offset.checked_add(8).ok_or("GIF GCE overflow")?;
                } else {
                    let application_start =
                        offset.checked_add(3).ok_or("GIF extension overflow")?;
                    let application_end = offset.checked_add(14).ok_or("GIF extension overflow")?;
                    if label == 0xff
                        && encoded.get(application_start..application_end) == Some(b"NETSCAPE2.0")
                    {
                        has_loop = true;
                    }
                    let blocks_offset = offset.checked_add(2).ok_or("GIF extension overflow")?;
                    offset = skip_gif_sub_blocks(encoded, blocks_offset)
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
    if params.get("loop").and_then(Value::as_bool) == Some(true) && !has_loop {
        return Err("GIF loop option did not emit NETSCAPE2.0".to_owned());
    }
    if let Some(expected) = params.get("interlace").and_then(Value::as_bool)
        && image_interlace.iter().any(|&value| value != expected)
    {
        return Err(format!("GIF interlace setting does not match {expected}"));
    }
    if let Some(request) = params.get("color_table").and_then(Value::as_str) {
        let expected_local = request == "local";
        if !has_global || image_local.iter().any(|&value| value != expected_local) {
            return Err(format!("GIF color-table layout does not match {request}"));
        }
    }
    if let Some(request) = params.get("disposal").and_then(Value::as_str) {
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
    if let Some(expected) = params.get("transparency").and_then(Value::as_bool)
        && (gce_transparency.iter().any(|&value| value != expected)
            || expected && gce_transparency.is_empty())
    {
        return Err(format!(
            "GIF transparency setting does not match {expected}"
        ));
    }
    Ok(())
}

fn assert_bmp_contract(params: &HashMap<String, Value>, encoded: &[u8]) -> Result<(), String> {
    if encoded.get(..2) != Some(b"BM") {
        return Err("encoded BMP is missing BM signature".to_owned());
    }
    let header_size = read_le_u32(encoded, 14).ok_or("BMP header is truncated")?;
    let height = read_le_u32(encoded, 22).ok_or("BMP height is truncated")? as i32;
    let depth = read_le_u16(encoded, 28).ok_or("BMP depth is truncated")?;
    let compression = read_le_u32(encoded, 30).ok_or("BMP compression is truncated")?;

    if let Some(expected) = params.get("bit_depth").and_then(Value::as_u64) {
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
    if params.get("header").is_some() && header_size != 40 {
        return Err(format!(
            "BMP header mismatch: Pillow always emits V3 but encoded {header_size}"
        ));
    }
    if params.get("top_down").is_some() && height.is_negative() {
        return Err(format!(
            "BMP row direction mismatch: Pillow always emits bottom-up height {height}"
        ));
    }
    if params.get("compression").is_some() && compression != 0 {
        return Err(format!(
            "BMP compression mismatch: Pillow always emits BI_RGB but encoded {compression}"
        ));
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

fn assert_tiff_contract(params: &HashMap<String, Value>, encoded: &[u8]) -> Result<(), String> {
    let endian = match encoded.get(..2) {
        Some(b"II") => TiffEndian::Little,
        Some(b"MM") => TiffEndian::Big,
        _ => return Err("encoded TIFF has an invalid byte-order marker".to_owned()),
    };
    if endian.read_u16(encoded, 2) != Some(42) {
        return Err("encoded TIFF has an invalid magic value".to_owned());
    }
    if let Some(request) = params.get("byte_order").and_then(Value::as_str)
        && !matches!(endian, TiffEndian::Little)
    {
        return Err(format!(
            "TIFF byte order mismatch: Pillow ignores {request} and emits little-endian"
        ));
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
            .read_u16(encoded, offset.checked_add(2).ok_or("TIFF entry overflow")?)
            .ok_or("truncated TIFF entry type")?;
        let item_count = endian
            .read_u32(encoded, offset.checked_add(4).ok_or("TIFF entry overflow")?)
            .ok_or("truncated TIFF entry count")?;
        if item_count == 1 && matches!(field_type, 3 | 4) {
            let value = if field_type == 3 {
                u32::from(
                    endian
                        .read_u16(encoded, offset.checked_add(8).ok_or("TIFF entry overflow")?)
                        .ok_or("truncated TIFF SHORT value")?,
                )
            } else {
                endian
                    .read_u32(encoded, offset.checked_add(8).ok_or("TIFF entry overflow")?)
                    .ok_or("truncated TIFF LONG value")?
            };
            tags.insert(tag, value);
        }
    }
    if let Some(request) = params.get("compression").and_then(Value::as_str) {
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
    if let Some(request) = params.get("predictor").and_then(Value::as_str) {
        let expected = if request == "horizontal" { 2 } else { 1 };
        let actual = tags.get(&317).copied().unwrap_or(1);
        if actual != expected {
            return Err(format!("TIFF predictor tag does not match {request}"));
        }
    }
    if let Some(request) = params.get("organization").and_then(Value::as_str) {
        let tiled = tags.contains_key(&322) || tags.contains_key(&324);
        if tiled {
            return Err(format!(
                "TIFF organization mismatch: Pillow ignores {request} and emits strips"
            ));
        }
    }
    if let Some(request) = params.get("pages").and_then(Value::as_u64) {
        let next_ifd_offset = ifd
            .checked_add(
                count
                    .checked_mul(12)
                    .and_then(|size| size.checked_add(2))
                    .ok_or("TIFF next-IFD size overflow")?,
            )
            .ok_or("TIFF next-IFD offset overflow")?;
        if endian.read_u32(encoded, next_ifd_offset) != Some(0) {
            return Err(format!(
                "TIFF page-count mismatch: Pillow ignores pages={request} and emits one page"
            ));
        }
    }
    Ok(())
}

fn assert_jpeg_contract(params: &HashMap<String, Value>, encoded: &[u8]) -> Result<(), String> {
    if encoded.get(..2) != Some(&[0xff, 0xd8]) {
        return Err("encoded JPEG has no SOI marker".to_owned());
    }
    let mut offset = 2usize;
    let mut sof = None::<(u8, &[u8])>;
    let mut has_exif = false;
    let mut has_restart_interval = false;
    while offset < encoded.len() {
        while encoded.get(offset) == Some(&0xff) {
            offset = offset.checked_add(1).ok_or("JPEG marker overflow")?;
        }
        let marker = *encoded.get(offset).ok_or("truncated JPEG marker")?;
        offset = offset.checked_add(1).ok_or("JPEG marker overflow")?;
        if matches!(marker, 0xd8 | 0xd9 | 0x01 | 0xd0..=0xd7) {
            continue;
        }
        let length = usize::from(u16::from_be_bytes(
            encoded
                .get(
                    offset
                        ..offset
                            .checked_add(2)
                            .ok_or("JPEG segment length overflow")?,
                )
                .ok_or("truncated JPEG segment length")?
                .try_into()
                .map_err(|_| "invalid JPEG segment length")?,
        ));
        if length < 2 {
            return Err("invalid JPEG segment length".to_owned());
        }
        let payload_start = offset
            .checked_add(2)
            .ok_or("JPEG segment payload overflow")?;
        let payload_end = offset
            .checked_add(length)
            .ok_or("JPEG segment payload overflow")?;
        let payload = encoded
            .get(payload_start..payload_end)
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
        offset = offset
            .checked_add(length)
            .ok_or("JPEG segment offset overflow")?;
        if marker == 0xda {
            break;
        }
    }
    let (sof_marker, sof_data) = sof.ok_or("encoded JPEG has no supported SOF marker")?;
    if sof_data.len() < 8 {
        return Err("truncated JPEG SOF segment".to_owned());
    }
    if let Some(expected) = params.get("progressive").and_then(Value::as_bool)
        && (sof_marker == 0xc2) != expected
    {
        return Err(format!("JPEG progressive mode does not match {expected}"));
    }
    if let Some(expected) = params.get("grayscale").and_then(Value::as_bool) {
        let components = sof_data[5];
        if (components == 1) != expected {
            return Err(format!("JPEG grayscale mode does not match {expected}"));
        }
    }
    if let Some(request) = params.get("subsampling").and_then(Value::as_str) {
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
    if params.get("exif").and_then(Value::as_bool) == Some(false) && has_exif {
        return Err("JPEG exif=false emitted EXIF metadata".to_owned());
    }
    if params.get("exif_hex").is_some() && !has_exif {
        return Err("JPEG EXIF metadata request did not emit APP1 EXIF".to_owned());
    }
    if params.get("restart_interval").and_then(Value::as_u64) == Some(0) && has_restart_interval {
        return Err("JPEG restart_interval=0 emitted DRI".to_owned());
    }
    Ok(())
}

fn assert_ico_contract(params: &HashMap<String, Value>, encoded: &[u8]) -> Result<(), String> {
    if encoded.get(..4) != Some(&[0, 0, 1, 0]) {
        return Err("encoded ICO has an invalid header".to_owned());
    }
    let count = usize::from(read_le_u16(encoded, 4).ok_or("truncated ICO header")?);
    if count == 0 {
        return (encoded.len() == 6)
            .then_some(())
            .ok_or_else(|| "zero-entry ICO has trailing data".to_owned());
    }
    let directory_end = count
        .checked_mul(16)
        .and_then(|size| size.checked_add(6))
        .ok_or("ICO directory size overflow")?;
    if encoded.len() < directory_end {
        return Err("encoded ICO has an invalid image directory".to_owned());
    }

    let expect_bmp = params.get("entry_type").and_then(Value::as_str) == Some("bmp");
    for index in 0..count {
        let entry = index
            .checked_mul(16)
            .and_then(|value| value.checked_add(6))
            .ok_or("ICO directory offset overflow")?;
        let depth_offset = entry
            .checked_add(6)
            .ok_or("ICO directory offset overflow")?;
        let directory_depth =
            read_le_u16(encoded, depth_offset).ok_or("truncated ICO directory entry")?;
        if !expect_bmp && directory_depth != 32 {
            return Err("ICO PNG directory entry is not 32-bit".to_owned());
        }

        let data_size = usize::try_from(
            read_le_u32(
                encoded,
                entry
                    .checked_add(8)
                    .ok_or("ICO directory offset overflow")?,
            )
            .ok_or("truncated ICO directory entry")?,
        )
        .map_err(|_| "ICO data size is too large")?;
        let data_offset = usize::try_from(
            read_le_u32(
                encoded,
                entry
                    .checked_add(12)
                    .ok_or("ICO directory offset overflow")?,
            )
            .ok_or("truncated ICO directory entry")?,
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
        if expect_bmp
            && read_le_u16(
                encoded,
                data_offset
                    .checked_add(14)
                    .ok_or("ICO payload offset overflow")?,
            ) != Some(directory_depth)
        {
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
                    encoded
                        .get(marker.checked_add(7)?..marker.checked_add(9)?)?
                        .try_into()
                        .ok()?,
                )),
                u32::from(u16::from_be_bytes(
                    encoded
                        .get(marker.checked_add(5)?..marker.checked_add(7)?)?
                        .try_into()
                        .ok()?,
                )),
            ))
        }
        "png" => Some((read_be_u32(encoded, 16)?, read_be_u32(encoded, 20)?)),
        _ => None,
    }
}

fn assert_encoded_contract(
    format: &str,
    params: &HashMap<String, Value>,
    encoded: &[u8],
) -> Result<(), String> {
    match format {
        "bmp" => assert_bmp_contract(params, encoded),
        "gif" => assert_gif_contract(params, encoded),
        "ico" => assert_ico_contract(params, encoded),
        "jpeg" => assert_jpeg_contract(params, encoded),
        "png" => assert_png_contract(params, encoded),
        "tiff" => assert_tiff_contract(params, encoded),
        "avif" => {
            if encoded.get(4..8) != Some(b"ftyp")
                || !matches!(
                    encoded.get(8..12),
                    Some(b"avif" | b"avis" | b"mif1" | b"msf1")
                )
            {
                Err("encoded AVIF has no accepted ftyp major brand".to_owned())
            } else {
                Ok(())
            }
        }
        _ => Ok(()),
    }?;
    if let Some(size) = params.get("size").and_then(Value::as_array) {
        let expected = (
            size.first()
                .and_then(Value::as_u64)
                .and_then(|value| u32::try_from(value).ok())
                .ok_or("invalid requested width")?,
            size.get(1)
                .and_then(Value::as_u64)
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
    reference: (Option<&[u32]>, Option<&str>),
) -> Option<PixelParityRef> {
    let (ref_size, ref_mode) = reference;
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
                    let byte_index = chunk_index.wrapping_mul(64).wrapping_add(offset);
                    let pixel_index = byte_index.checked_div(bytes_per_pixel)?;
                    let pixel_index_u32 = u32::try_from(pixel_index).ok()?;
                    let x = pixel_index_u32.checked_rem(width)?;
                    let y = pixel_index_u32.checked_div(width)?;
                    Some(PixelMismatch {
                        byte_index,
                        pixel_index,
                        x,
                        y,
                        channel: byte_index.checked_rem(bytes_per_pixel)?,
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
    if let Some(width) = expected.width
        && actual.width != width
    {
        return Err(format!(
            "width mismatch: actual {}, expected {}",
            actual.width, width
        ));
    }
    if let Some(height) = expected.height
        && actual.height != height
    {
        return Err(format!(
            "height mismatch: actual {}, expected {}",
            actual.height, height
        ));
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

fn assert_sequence_parity(manifest_dir: &Path, row: &DecodeRow, data: &[u8]) -> Result<(), String> {
    let Some(expected) = &row.sequence else {
        return Ok(());
    };
    let actual = img::decode_sequence(data)
        .map_err(|error| format!("sequence decode failed: {error}"))?
        .content;
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

fn format_from_name(format: &str) -> Option<img::ImageFormat> {
    img::ImageFormat::from_name(format).ok()
}

#[test]
fn test_decode_matrix() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let matrix = require_some(
        coverage_matrix(),
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
    let mut source_lifecycle_formats = HashSet::new();

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
            let verify_result =
                img::EncodedImage::new(Arc::<[u8]>::from(data.clone())).and_then(|source| {
                    let result = source.verify();
                    assert!(
                        !source.is_decoded(),
                        "verify must not populate decode cache"
                    );
                    result
                });
            let verify_matches_oracle = match row.verify_status.as_str() {
                "ok" => verify_result.is_ok(),
                "error" => {
                    row.verify_error_type
                        .as_deref()
                        .is_some_and(|kind| !kind.is_empty())
                        && row.verify_error_message.as_deref().is_some()
                        && verify_result.is_err()
                }
                _ => false,
            };
            if !verify_matches_oracle {
                eprintln!(
                    "  FAIL [{}]: verify result does not match Pillow ({:?} versus {})",
                    row.id, verify_result, row.verify_status
                );
                failed += 1;
                continue;
            }
            if row.expect_error.unwrap_or(false) {
                if row.oracle_status.as_deref() != Some("error")
                    || row.oracle_error_type.as_deref().is_none_or(str::is_empty)
                {
                    eprintln!(
                        "  FAIL [{}]: error fixture lacks Pillow oracle type/status ({:?}: {:?})",
                        row.id, row.oracle_error_type, row.oracle_error_message
                    );
                    failed += 1;
                    continue;
                }
                if matches!(
                    fmt_name.as_str(),
                    "png" | "jpeg" | "gif" | "bmp" | "webp" | "tiff" | "ico" | "avif"
                ) {
                    let _ = img::inspect(&data);
                }
                let sequence_rejected = match fmt_name.as_str() {
                    "gif" | "webp" | "avif" => match img::decode_sequence(&data) {
                        Err(img::ImageError::UnknownFormat) => {
                            img::detect_format(&data) == Err(img::ImageError::UnknownFormat)
                        }
                        Err(img::ImageError::Malformed { format, .. }) => {
                            format_from_name(fmt_name) == Some(format)
                        }
                        _ => false,
                    },
                    _ => true,
                };
                let expected_format = require_some(
                    format_from_name(fmt_name),
                    "manifest format must be supported",
                );
                let structured_error = match img::detect_format(&data) {
                    Err(img::ImageError::UnknownFormat) => {
                        matches!(decoded, Err(img::ImageError::UnknownFormat))
                    }
                    Ok(format) => {
                        format == expected_format
                            && matches!(
                                decoded,
                                Err(img::ImageError::Malformed {
                                    format: error_format,
                                    ..
                                }) if error_format == expected_format
                            )
                    }
                    Err(_) => false,
                };
                let source_error_is_stable =
                    match img::EncodedImage::new(Arc::<[u8]>::from(data.clone())) {
                        Err(error) => img::inspect(&data) == Err(error),
                        Ok(source) => {
                            let clone = source.clone();
                            let verified = source.verify();
                            let first = source.decode();
                            let second = clone.decode();
                            let verify_is_expected = match row.verify_status.as_str() {
                                "ok" => verified.is_ok(),
                                "error" => verified.is_err(),
                                _ => false,
                            };
                            verify_is_expected
                                && first.is_err()
                                && first == second
                                && !source.is_decoded()
                        }
                    };
                if structured_error && sequence_rejected && source_error_is_stable {
                    eprintln!("  OK   [{}] rejected as Pillow does", row.id);
                    passed += 1;
                } else {
                    eprintln!(
                        "  FAIL [{}]: invalid input lifecycle mismatch (auto={}, sequence_rejected={}, source_error_is_stable={})",
                        row.id,
                        decoded.is_ok(),
                        sequence_rejected,
                        source_error_is_stable
                    );
                    failed += 1;
                }
                continue;
            }

            let decoded = match decoded {
                Ok(decoded) => decoded,
                Err(error) => {
                    eprintln!("  FAIL [{}]: decode failed: {error}", row.id);
                    failed += 1;
                    continue;
                }
            };
            let expected_format = require_some(
                format_from_name(fmt_name),
                "manifest format must be supported",
            );
            if source_lifecycle_formats.insert(fmt_name.as_str()) {
                let source = match img::EncodedImage::new(Arc::<[u8]>::from(data.clone())) {
                    Ok(source) => source,
                    Err(error) => {
                        eprintln!(
                            "  FAIL [{}]: encoded source inspection failed: {error}",
                            row.id
                        );
                        failed += 1;
                        continue;
                    }
                };
                let source_clone = source.clone();
                if source.format() != expected_format
                    || source.info().format != expected_format
                    || source.bytes() != data
                    || source.is_decoded()
                {
                    eprintln!(
                        "  FAIL [{}]: encoded source changed inspected state",
                        row.id
                    );
                    failed += 1;
                    continue;
                }
                if let Err(error) = source.verify() {
                    eprintln!("  FAIL [{}]: encoded source verify failed: {error}", row.id);
                    failed += 1;
                    continue;
                }
                if source.is_decoded() {
                    eprintln!(
                        "  FAIL [{}]: verify populated ordinary decode cache",
                        row.id
                    );
                    failed += 1;
                    continue;
                }
                let concurrent_addresses = require_ok(
                    std::thread::scope(|scope| {
                        (0..4)
                            .map(|_| {
                                let source = source.clone();
                                scope.spawn(move || {
                                    source
                                        .decode()
                                        .map(|decoded| std::ptr::from_ref(decoded) as usize)
                                })
                            })
                            .collect::<Vec<_>>()
                            .into_iter()
                            .map(|handle| match handle.join() {
                                Ok(result) => result,
                                Err(_) => panic!("concurrent decode worker panicked"),
                            })
                            .collect::<Result<Vec<_>, _>>()
                    }),
                    "concurrent source decode must succeed",
                );
                let cached = require_ok(source.decode(), "cached source decode must succeed");
                let cached_from_clone =
                    require_ok(source_clone.decode(), "clone decode must succeed");
                if !std::ptr::eq(cached, cached_from_clone)
                    || !source.is_decoded()
                    || cached != &decoded
                    || concurrent_addresses
                        .iter()
                        .any(|address| *address != std::ptr::from_ref(cached) as usize)
                {
                    eprintln!("  FAIL [{}]: shared lazy decode cache diverged", row.id);
                    failed += 1;
                    continue;
                }
            }
            if decoded.format != expected_format {
                eprintln!(
                    "  FAIL [{}]: detected {:?}, expected {:?}",
                    row.id, decoded.format, expected_format
                );
                failed += 1;
                continue;
            }
            let borrowed = decoded.as_ref();
            if borrowed.format != expected_format || borrowed.content != &decoded.content {
                eprintln!(
                    "  FAIL [{}]: decoded envelope borrow changed content",
                    row.id
                );
                failed += 1;
                continue;
            }
            let decoded = decoded.into_inner();
            if matches!(
                fmt_name.as_str(),
                "png" | "jpeg" | "gif" | "bmp" | "webp" | "tiff" | "ico" | "avif"
            ) {
                let info = match img::inspect(&data) {
                    Ok(info) => info,
                    Err(error) => {
                        eprintln!("  FAIL [{}]: metadata inspection failed: {error}", row.id);
                        failed += 1;
                        continue;
                    }
                };
                let expected_mode = row.ref_mode.as_deref().and_then(expected_image_mode);
                let expected_is_animated = row
                    .ref_is_animated
                    .unwrap_or(row.ref_frame_count.is_some_and(|count| count > 1));
                if info.format != expected_format
                    || Some(info.width)
                        != row.ref_size.as_ref().and_then(|size| size.first()).copied()
                    || Some(info.height)
                        != row.ref_size.as_ref().and_then(|size| size.get(1)).copied()
                    || Some(info.mode) != expected_mode
                    || (matches!(fmt_name.as_str(), "png" | "bmp" | "tiff" | "ico")
                        && info.palette != decoded.palette)
                    || (fmt_name == "gif" && info.palette.is_some() != decoded.palette.is_some())
                    || info.has_palette() != (decoded.mode == img::ImageMode::P8)
                    || info.frame_count != row.ref_frame_count
                    || info.is_animated != expected_is_animated
                {
                    eprintln!(
                        "  FAIL [{}]: metadata {:?} differs from Pillow mode/size/frame and decoded palette",
                        row.id, info
                    );
                    failed += 1;
                    continue;
                }
            } else if !matches!(
                img::inspect(&data),
                Err(img::ImageError::Unsupported {
                    format: Some(format),
                    ..
                }) if format == expected_format
            ) {
                eprintln!(
                    "  FAIL [{}]: unmigrated metadata parser did not report structured unsupported",
                    row.id
                );
                failed += 1;
                continue;
            }
            let Some(expected) = load_pixel_reference(
                manifest_dir,
                &row.id,
                row.ref_path.as_deref(),
                "Decode",
                fmt_name,
                asset_name,
                (row.ref_size.as_deref(), row.ref_mode.as_deref()),
            ) else {
                eprintln!(
                    "  FAIL [{}]: active row has no readable pixel reference",
                    row.id
                );
                failed += 1;
                continue;
            };

            match assert_pixel_parity(&expected, &decoded)
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
    let matrix = require_some(
        coverage_matrix(),
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
                            let asset = require_some(
                                src.asset.as_ref(),
                                "active fallback decode row must have an asset",
                            );
                            let path = assets_dir.join(fmt_name).join(asset);
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
                entry.insert(require_ok(
                    fs::read(&asset_path),
                    "encode source asset must be readable",
                ));
            }

            if let Entry::Vacant(entry) = decoded_cache.entry(asset_path.clone()) {
                let asset_data = require_some(
                    asset_cache.get(&asset_path),
                    "source asset must be cached before decode",
                );
                match img::decode_sequence(asset_data) {
                    Ok(decoded) => {
                        entry.insert(decoded.content);
                    }
                    Err(error) => {
                        eprintln!("  FAIL [{}]: source decode failed: {error}", row.id);
                        failed += 1;
                        continue;
                    }
                }
            }
            let cached_decoded = require_some(
                decoded_cache.get(&asset_path),
                "decoded source must be cached before encode",
            );
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
                let frame = require_some(
                    decoded.frames.get_mut(1),
                    "second-frame operation requires a second source frame",
                );
                let width = require_ok(
                    usize::try_from(frame.image.width),
                    "frame width must fit usize",
                );
                let height = require_ok(
                    usize::try_from(frame.image.height),
                    "frame height must fit usize",
                );
                let pixel_count = width.saturating_mul(height);
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
                advanced: advanced_encode_options(&row.params),
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
                "avif" => img::ImageFormat::Avif,
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
                let mut malformed =
                    require_some(decoded.first(), "encoded sequence must have a first frame")
                        .clone();
                malformed.pixels.pop();
                img::encode(&malformed, format, &opts)
            } else if let Some(dimensions) = row.params.get("source_dimensions") {
                let dimensions = require_some(
                    dimensions.as_array(),
                    "source_dimensions must be a JSON array",
                );
                let mut malformed =
                    require_some(decoded.first(), "encoded sequence must have a first frame")
                        .clone();
                malformed.width = require_ok(
                    u32::try_from(require_some(
                        dimensions[0].as_u64(),
                        "source width must be an unsigned integer",
                    )),
                    "source width must fit u32",
                );
                malformed.height = require_ok(
                    u32::try_from(require_some(
                        dimensions[1].as_u64(),
                        "source height must be an unsigned integer",
                    )),
                    "source height must fit u32",
                );
                img::encode(&malformed, format, &opts)
            } else {
                img::encode_sequence(decoded, format, &opts)
            };
            if row.expect_error {
                let fixture_has_oracle_error = row.oracle_status.as_deref() == Some("error")
                    && row
                        .oracle_error_type
                        .as_deref()
                        .is_some_and(|value| !value.is_empty());
                if encoded.is_err() && fixture_has_oracle_error {
                    eprintln!("  OK   [{}] rejected as Pillow does", row.id);
                    passed += 1;
                } else {
                    eprintln!(
                        "  FAIL [{}]: fixture error mismatch (encoded_ok={}, oracle={:?}: {:?})",
                        row.id,
                        encoded.is_ok(),
                        row.oracle_error_type,
                        row.oracle_error_message
                    );
                    failed += 1;
                }
                continue;
            }
            let encoded = match encoded {
                Ok(encoded) => encoded,
                Err(error) => {
                    eprintln!("  FAIL [{}]: encode failed: {error}", row.id);
                    failed += 1;
                    continue;
                }
            };
            if decoded.frames.len() == 1 {
                match img::encode(
                    require_some(decoded.first(), "encoded sequence must have a first frame"),
                    format,
                    &opts,
                ) {
                    Ok(still) if still == encoded => {}
                    Ok(_) => {
                        eprintln!(
                            "  FAIL [{}]: still and one-frame sequence encoders differ",
                            row.id
                        );
                        failed += 1;
                        continue;
                    }
                    Err(error) => {
                        eprintln!("  FAIL [{}]: still-image encoder failed: {error}", row.id);
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
            if row.params.get("encoded_only").and_then(Value::as_bool) == Some(true) {
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
                Ok(redecoded) => {
                    let redecoded = redecoded.content;
                    if let Some(expected) = row.ref_path.as_deref().and_then(|ref_path| {
                        load_pixel_reference(
                            manifest_dir,
                            &row.id,
                            Some(ref_path),
                            "Encode",
                            fmt_name,
                            row.source_asset.as_deref().unwrap_or(""),
                            (row.ref_size.as_deref(), row.ref_mode.as_deref()),
                        )
                    }) {
                        match assert_pixel_parity(&expected, &redecoded) {
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
                Err(error) => {
                    eprintln!("  FAIL [{}]: re-decode failed: {error}", row.id);
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

#[cfg(coverage)]
#[test]
fn test_internal_coverage_hooks() {
    img::__coverage_exercise_private_branches();
}

#[cfg(coverage)]
#[test]
fn test_av1_entropy_trace_matches_pinned_dav1d_fixture() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("outputs")
        .join("av1_entropy.json");
    let contents = require_ok(
        fs::read_to_string(path),
        "AV1 entropy fixture must be readable",
    );
    let expected: Av1EntropyDocument = require_ok(
        json::from_str(&contents),
        "AV1 entropy fixture must be valid JSON",
    );
    assert_eq!(expected.format_version, 2);
    assert_eq!(expected.oracle.implementation, "dav1d");
    assert_eq!(expected.oracle.version, "1.5.3");
    assert_eq!(
        expected.oracle.commit,
        "b546257f770768b2c88258c533da38b91a06f737"
    );
    assert_eq!(
        expected.oracle.source_files,
        [
            "src/msac.c",
            "src/msac.h",
            "src/decode.c",
            "src/cdf.c",
            "src/tables.c"
        ]
    );
    assert_eq!(
        expected.input_hex,
        "00ff817e55aa13ec42bd99660180fe24db10ef738c31ce5aa50ff069963cc37f"
    );
    assert_eq!(
        expected
            .partition_422_inputs
            .get("still")
            .map(String::as_str),
        Some("00e234fe35f6ba4026a9e0b77e80")
    );
    assert_eq!(
        expected
            .partition_422_inputs
            .get("frame_2")
            .map(String::as_str),
        Some("0a057797a7a05837feb11c8887")
    );
    assert_eq!(
        expected
            .partition_422_inputs
            .get("frame_3")
            .map(String::as_str),
        Some(
            "f83f9ffd73c02fa55948fac5e5748785cac600815da53a6efaf37c24180bfc69\
             2c41073b722ecfffb02a3b55452247bb8c3c03b219e9df68caf0156ec0e79d21\
             ff54f6ce3093636f599789ba72"
        )
    );

    let actual = require_ok(
        img::__coverage_av1_entropy_reference_trace(),
        "Rust AV1 entropy trace must be constructible",
    )
    .into_iter()
    .map(|state| Av1EntropyRecord {
        case: state.case.to_owned(),
        step: state.step,
        value: state.value,
        byte_position: state.byte_position,
        difference: state.difference,
        range: state.range,
        count: state.count,
        cdf: state.cdf,
    })
    .collect::<Vec<_>>();
    assert_eq!(actual.len(), expected.records.len());
    for (index, (actual, expected)) in actual.iter().zip(&expected.records).enumerate() {
        assert_eq!(actual, expected, "AV1 entropy record {index}");
    }
}

#[cfg(coverage)]
#[test]
fn test_av1_reconstruction_matches_pinned_dav1d_fixture() {
    let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures");
    let document_path = fixture_root.join("outputs").join("av1_reconstruction.json");
    let contents = require_ok(
        fs::read_to_string(document_path),
        "AV1 reconstruction fixture must be readable",
    );
    let expected: Av1ReconstructionDocument = require_ok(
        json::from_str(&contents),
        "AV1 reconstruction fixture must be valid JSON",
    );
    assert_eq!(expected.format_version, 3);
    assert_eq!(expected.oracle.implementation, "dav1d");
    assert_eq!(expected.oracle.version, "1.5.3");
    assert_eq!(
        expected.oracle.commit,
        "b546257f770768b2c88258c533da38b91a06f737"
    );
    assert_eq!(expected.oracle.pillow_avif, "1.4.1");
    assert_eq!(
        expected.oracle.pillow_codecs,
        "dav1d [dec]:1.5.3-0-gb546257, aom [enc]:3.13.2"
    );
    assert_eq!(
        expected.scope,
        "private AV1 first-block reconstruction and closed-class AVIF materialization; \
         not a public image-processing API"
    );
    assert_eq!(expected.oracle.pillow_libyuv, 1922);
    assert_eq!(expected.cases.len(), 86);
    for (accepted, extension) in [
        ("partitioned_12x4_a.avif", "partitioned_16x4_a.avif"),
        (
            "partitioned_12x4_gray_32.avif",
            "partitioned_16x4_gray_32.avif",
        ),
        ("partitioned_12x4_green.avif", "partitioned_16x4_green.avif"),
        ("partitioned_4x12_a.avif", "partitioned_4x16_a.avif"),
        (
            "partitioned_4x12_gray_32.avif",
            "partitioned_4x16_gray_32.avif",
        ),
        ("partitioned_4x12_green.avif", "partitioned_4x16_green.avif"),
        ("partitioned_16x4_a.avif", "partitioned_12x8_a.avif"),
        (
            "partitioned_16x4_gray_32.avif",
            "partitioned_12x8_gray_32.avif",
        ),
        ("partitioned_16x4_green.avif", "partitioned_12x8_green.avif"),
        ("partitioned_16x4_a.avif", "partitioned_16x8_a.avif"),
        (
            "partitioned_16x4_gray_32.avif",
            "partitioned_16x8_gray_32.avif",
        ),
        ("partitioned_16x4_green.avif", "partitioned_16x8_green.avif"),
        ("partitioned_4x16_a.avif", "partitioned_8x12_a.avif"),
        (
            "partitioned_4x16_gray_32.avif",
            "partitioned_8x12_gray_32.avif",
        ),
        ("partitioned_4x16_green.avif", "partitioned_8x12_green.avif"),
        ("partitioned_4x16_a.avif", "partitioned_8x16_a.avif"),
        (
            "partitioned_4x16_gray_32.avif",
            "partitioned_8x16_gray_32.avif",
        ),
        ("partitioned_4x16_green.avif", "partitioned_8x16_green.avif"),
    ] {
        let accepted = expected
            .cases
            .iter()
            .find(|case| case.fixture == accepted)
            .expect("accepted recursive AVIF oracle case");
        let extension = expected
            .cases
            .iter()
            .find(|case| case.fixture == extension)
            .expect("extended recursive AVIF oracle case");
        assert_eq!(extension.partition_blocks, accepted.partition_blocks);
        assert_eq!(extension.entropy_operations, accepted.entropy_operations);
    }

    for (case_index, case) in expected.cases.iter().enumerate() {
        let input = require_ok(
            fs::read(
                fixture_root
                    .join("input")
                    .join("images")
                    .join("avif")
                    .join(&case.fixture),
            ),
            "portable AVIF fixture must be readable",
        );
        let actual = img::__coverage_av1_reconstruction(&input).unwrap_or_else(|| {
            panic!(
                "production AV1 path must retain the reconstructed first leaf for {}",
                case.fixture
            )
        });
        assert_eq!(
            (
                actual.width,
                actual.height,
                actual.bit_depth,
                actual.monochrome,
                actual.color_primaries,
                actual.transfer_characteristics,
                actual.matrix_coefficients,
                actual.color_range,
                actual.subsampling_x,
                actual.subsampling_y,
            ),
            (
                case.portable_color.width,
                case.portable_color.height,
                case.portable_color.bit_depth,
                case.portable_color.monochrome,
                case.portable_color.color_primaries,
                case.portable_color.transfer_characteristics,
                case.portable_color.matrix_coefficients,
                case.portable_color.color_range,
                case.portable_color.subsampling_x,
                case.portable_color.subsampling_y,
            ),
            "AV1 portable color state case {case_index}"
        );
        let recursive_ranges = match case.fixture.as_str() {
            "partitioned_12x4_a.avif" => Some([37_392, 43_662, 53_296]),
            "partitioned_12x4_gray_32.avif" => Some([37_392, 43_662, 58_282]),
            "partitioned_12x4_green.avif" => Some([37_392, 43_662, 56_842]),
            "partitioned_16x4_a.avif" => Some([37_392, 43_662, 53_296]),
            "partitioned_16x4_gray_32.avif" => Some([37_392, 43_662, 58_282]),
            "partitioned_16x4_green.avif" => Some([37_392, 43_662, 56_842]),
            "partitioned_12x8_a.avif" => Some([37_392, 43_662, 53_296]),
            "partitioned_12x8_gray_32.avif" => Some([37_392, 43_662, 58_282]),
            "partitioned_12x8_green.avif" => Some([37_392, 43_662, 56_842]),
            "partitioned_16x8_a.avif" => Some([37_392, 43_662, 53_296]),
            "partitioned_16x8_gray_32.avif" => Some([37_392, 43_662, 58_282]),
            "partitioned_16x8_green.avif" => Some([37_392, 43_662, 56_842]),
            "partitioned_4x12_a.avif" => Some([46_608, 54_426, 36_309]),
            "partitioned_4x12_gray_32.avif" => Some([46_608, 54_426, 36_767]),
            "partitioned_4x12_green.avif" => Some([46_608, 54_426, 61_946]),
            "partitioned_4x16_a.avif" => Some([46_608, 54_426, 36_309]),
            "partitioned_4x16_gray_32.avif" => Some([46_608, 54_426, 36_767]),
            "partitioned_4x16_green.avif" => Some([46_608, 54_426, 61_946]),
            "partitioned_8x12_a.avif" => Some([46_608, 54_426, 36_309]),
            "partitioned_8x12_gray_32.avif" => Some([46_608, 54_426, 36_767]),
            "partitioned_8x12_green.avif" => Some([46_608, 54_426, 61_946]),
            "partitioned_8x16_a.avif" => Some([46_608, 54_426, 36_309]),
            "partitioned_8x16_gray_32.avif" => Some([46_608, 54_426, 36_767]),
            "partitioned_8x16_green.avif" => Some([46_608, 54_426, 61_946]),
            _ => None,
        };
        if let Some(ranges) = recursive_ranges {
            let horizontal = case.portable_color.width > case.portable_color.height;
            let expected_blocks = [
                Av1PartitionBlock {
                    poc: 0,
                    x: 0,
                    y: 0,
                    level: 3,
                    context: 0,
                    partition: 3,
                    range: ranges[0],
                },
                Av1PartitionBlock {
                    poc: 0,
                    x: 0,
                    y: 0,
                    level: 4,
                    context: 0,
                    partition: 0,
                    range: ranges[1],
                },
                Av1PartitionBlock {
                    poc: 0,
                    x: if horizontal { 2 } else { 0 },
                    y: if horizontal { 0 } else { 2 },
                    level: 4,
                    context: 0,
                    partition: 0,
                    range: ranges[2],
                },
            ];
            assert_eq!(
                case.partition_blocks, expected_blocks,
                "AV1 recursive partition topology case {case_index}"
            );
        } else {
            assert_eq!(
                case.partition_blocks.len(),
                1,
                "AV1 single-leaf partition topology case {case_index}"
            );
        }
        assert_eq!(case.decoded_planes.len(), 3);
        for (plane_index, (actual, expected)) in
            actual.planes.iter().zip(&case.decoded_planes).enumerate()
        {
            assert_eq!(expected.name, ["y", "u", "v"][plane_index]);
            assert_eq!(
                (expected.width, expected.height),
                (case.portable_color.width, case.portable_color.height)
            );
            assert_eq!(
                expected.row_bytes.len(),
                usize::try_from(expected.height).expect("AV1 plane height")
            );
            let expected_bytes = expected
                .row_bytes
                .iter()
                .flat_map(|row| {
                    assert_eq!(
                        row.len(),
                        usize::try_from(expected.width)
                            .expect("AV1 plane width")
                            .saturating_mul(2)
                    );
                    row.as_bytes().chunks_exact(2).map(|pair| {
                        let pair = std::str::from_utf8(pair).expect("hex pair must be UTF-8");
                        u8::from_str_radix(pair, 16).expect("hex pair must be valid")
                    })
                })
                .collect::<Vec<_>>();
            let actual_bytes = actual
                .iter()
                .map(|sample| u8::try_from(*sample).expect("eight-bit AV1 sample"))
                .collect::<Vec<_>>();
            assert_eq!(
                actual_bytes, expected_bytes,
                "AV1 reconstruction case {case_index} plane {plane_index}"
            );
        }
        let actual_operations = actual
            .entropy_operations
            .into_iter()
            .map(|operation| Av1ReconstructionEntropyOperation {
                operation: operation.operation.to_owned(),
                parameter: operation.parameter,
                step: operation.step,
                value: operation.value,
                byte_position: operation.byte_position,
                difference: operation.difference,
                range: operation.range,
                count: operation.count,
                cdf: operation.cdf,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            actual_operations.len(),
            case.entropy_operations.len(),
            "AV1 reconstruction entropy operation count case {case_index}"
        );
        for (operation_index, (actual, expected)) in actual_operations
            .iter()
            .zip(&case.entropy_operations)
            .enumerate()
        {
            assert_eq!(
                actual, expected,
                "AV1 reconstruction entropy case {case_index} operation {operation_index}"
            );
        }

        assert_eq!(case.pillow.mode, "RGB");
        assert_eq!(
            case.pillow.size,
            [case.portable_color.width, case.portable_color.height]
        );
        assert_eq!(
            case.pillow.bytes,
            usize::try_from(
                case.pillow.size[0]
                    .saturating_mul(case.pillow.size[1])
                    .saturating_mul(3),
            )
            .expect("Pillow RGB byte count")
        );
        let expected_pillow_sha256 = match case.fixture.as_str() {
            "portable_lossless_a.avif" => {
                "0fdfb2ec7d6741b65177c1343d0e510798f3177b75018fdbc8da541ea2d32a0b"
            }
            "portable_lossless_b.avif" => {
                "34a99c606d95db58868b24c3ce3ade1c502adcf213130c403486cbd50bc4fad5"
            }
            "portable_lossless_gray_32.avif" => {
                "b4a53f2b248b5701814756a08eb3435e49117eda791610ff85dd22e8a6a86df3"
            }
            "portable_lossless_gray_127.avif" => {
                "a1fa26e9a041c510e9f8412accef2e5e0cda5eddd97fa6db80b30400b7964d42"
            }
            "portable_probe_gray_128.avif" => {
                "2ac4dd6f486e2f061ebe8ce8b651dbdf25d71b88184d0bf308608cdcaae05309"
            }
            "portable_probe_gray_129.avif" => {
                "b34e1e1e7cd63c9fb7069154ccd855d827a3dd3eca076232b4217745a2b6db57"
            }
            "portable_lossless_8x8_a.avif" => {
                "1f403e7f414473b888fcba438d60d269e54fc1d04c802dd32f96fa657932b2ac"
            }
            "portable_lossless_8x8_gray_127.avif" => {
                "c24e73f000a4255a612416ecc4df81c9313e4c099877384712e4d8530dd7acbd"
            }
            "portable_probe_8x8_gray_128.avif" => {
                "fa7b78cc215df21d7ce54d8c3c6637c326dab95c10fbc12263101365973f4268"
            }
            "portable_probe_8x8_gray_129.avif" => {
                "fca06fef259b9ebb452449c7feda724ccec06a4a76b2b4fb1e6420a0beac435e"
            }
            "portable_lossless_4x8_a.avif" | "portable_lossless_8x4_a.avif" => {
                "116d1d3509d9d2a7558a2fad832f923fc1193f04b8e0e57946f49e57fa045475"
            }
            "portable_lossless_4x8_gray_127.avif" | "portable_lossless_8x4_gray_127.avif" => {
                "faa8c27b41b2603cd12911cd93ee3953ff1f98c9fba83fdeef738cc8406c4b3f"
            }
            "portable_probe_4x8_gray_128.avif" | "portable_probe_8x4_gray_128.avif" => {
                "1b34669db94decae583e183ee2ffeb07cf504b9f52fae0056c5cf343325157e4"
            }
            "portable_probe_4x8_gray_129.avif" | "portable_probe_8x4_gray_129.avif" => {
                "780832a7ab39814257a857d37a67ab541a1152afbcf6a1883a16ad32c264ff4e"
            }
            "portable_lossless_12x12_a.avif" => {
                "cbc97cf0c2652e60e6e36611be9869444f603abf5f48b292a03d340f501320f8"
            }
            "portable_lossless_12x12_gray_127.avif" => {
                "cb4987527501d0915664b8e624e5f51ebbf5f48b52917058615c1f3b96764076"
            }
            "portable_probe_12x12_gray_128.avif" => {
                "cc0fcf371bdd305ff6099895e60aac93968bf0358724de1678979a37a9bd7a17"
            }
            "portable_probe_12x12_gray_129.avif" => {
                "143efd9552ea35a74333bbfc58d10ae5a0eccfe76d2283c05b2b4a9391c346cd"
            }
            "portable_lossless_16x16_a.avif" => {
                "8bdcc97ae19b09ec3d6b76a7d59f13d4aa3dd7a06d21db706f2a1d15caaa0431"
            }
            "portable_lossless_16x16_gray_127.avif" => {
                "cbab715ff6cfaa81c9b09e014dc1406ceff24034caa265de65f9f948c5434807"
            }
            "portable_probe_16x16_gray_128.avif" => {
                "7f3e5e4e65eca4390e9242558012bc9bdad133d7ac9f6aed53fa156a2288f73b"
            }
            "portable_probe_16x16_gray_129.avif" => {
                "15dc2c3b0ea25a84b4994b9a73dbcf65eef174bad152c689cc1945843b543657"
            }
            "portable_lossless_12x16_a.avif" | "portable_lossless_16x12_a.avif" => {
                "f6b42085d682a064da2a9956545f33ae7595b288f7589e8e498c62e6bc26e874"
            }
            "portable_lossless_12x16_gray_127.avif" | "portable_lossless_16x12_gray_127.avif" => {
                "1b9924ee11c55d5fd4d944003b8b272c1f4ce12ea8e800c33563bed483fa406d"
            }
            "portable_probe_12x16_gray_128.avif" | "portable_probe_16x12_gray_128.avif" => {
                "af1857bf5516aa3e2e39b6842559746fa7b45daa8dc4cc6675ad86e0cfe425b9"
            }
            "portable_probe_12x16_gray_129.avif" | "portable_probe_16x12_gray_129.avif" => {
                "5269c00892aff8abcc6a4da60b82b890936aef6b1aa24c6b713c5a80a831c0b9"
            }
            "partitioned_12x4_gray_127.avif" | "partitioned_4x12_gray_127.avif" => {
                "35fc07c937c1c3d13641f32cdc94ce1315ec420dd26e12b81a4651cfc1786ee3"
            }
            "portable_rect_12x4_gray_128.avif" | "portable_rect_4x12_gray_128.avif" => {
                "7053108d4e37b600ae17d35890c69102ee6484d79a3a5cd622afca6f5606c543"
            }
            "portable_rect_12x4_gray_129.avif" | "portable_rect_4x12_gray_129.avif" => {
                "c60b05f1911c0ccc80c5af2cd922c7cf1836279d44a17682c918cdaa5c7747e6"
            }
            "portable_rect_12x8_gray_127.avif" | "portable_rect_8x12_gray_127.avif" => {
                "cf8691a9b8c6c8e329b94f40345d822ef7d4f6e8e5c2343d74b12aa16e84838a"
            }
            "portable_rect_12x8_gray_128.avif" | "portable_rect_8x12_gray_128.avif" => {
                "88f2f6050a4ef8c9fd8bd69d3e51689155f6aa570f0ac0da6d3c0ee794bf3867"
            }
            "portable_rect_12x8_gray_129.avif" | "portable_rect_8x12_gray_129.avif" => {
                "fe124f63ee1300955e9b2ffbed15cf383e9f4ae7c5cf60a09b074e4b0d73947f"
            }
            "portable_rect_16x4_gray_127.avif" | "portable_rect_4x16_gray_127.avif" => {
                "c24e73f000a4255a612416ecc4df81c9313e4c099877384712e4d8530dd7acbd"
            }
            "portable_rect_16x4_gray_128.avif" | "portable_rect_4x16_gray_128.avif" => {
                "fa7b78cc215df21d7ce54d8c3c6637c326dab95c10fbc12263101365973f4268"
            }
            "portable_rect_16x4_gray_129.avif" | "portable_rect_4x16_gray_129.avif" => {
                "fca06fef259b9ebb452449c7feda724ccec06a4a76b2b4fb1e6420a0beac435e"
            }
            "portable_rect_16x8_gray_127.avif" | "portable_rect_8x16_gray_127.avif" => {
                "7e18f1b2ca4e075b955848b4deafd56e47eeda83cc15b3ecdeb71d7ff58a5f57"
            }
            "portable_rect_16x8_gray_128.avif" | "portable_rect_8x16_gray_128.avif" => {
                "f83545d43c6939ec393b6b8310959b6174fd764b08a12fc22d908408a7e6a43e"
            }
            "portable_rect_16x8_gray_129.avif" | "portable_rect_8x16_gray_129.avif" => {
                "7d965db8cbcf57e71b10b16973c9c2439222485594191da31460986a000f497c"
            }
            "portable_rect_12x4_a_speed0.avif" | "portable_rect_4x12_a_speed0.avif" => {
                "09fddd84398ad9a9d3ce8b981fea278a82e6b1fa62483fa0ef3c45cd484ae29e"
            }
            "portable_rect_12x4_gray_32_speed0.avif" | "portable_rect_4x12_gray_32_speed0.avif" => {
                "31178565d9d883446d9e273ee881220f43cb4c5de74e237f590f845e25659f38"
            }
            "partitioned_12x4_a.avif" | "partitioned_4x12_a.avif" => {
                "09fddd84398ad9a9d3ce8b981fea278a82e6b1fa62483fa0ef3c45cd484ae29e"
            }
            "partitioned_12x4_gray_32.avif" | "partitioned_4x12_gray_32.avif" => {
                "31178565d9d883446d9e273ee881220f43cb4c5de74e237f590f845e25659f38"
            }
            "partitioned_12x4_green.avif" | "partitioned_4x12_green.avif" => {
                "7f5e545c140df34ec243d4449ab8c4c0e476f532d3f6472ce956e7060b271e1c"
            }
            "partitioned_16x4_a.avif" | "partitioned_4x16_a.avif" => {
                "1f403e7f414473b888fcba438d60d269e54fc1d04c802dd32f96fa657932b2ac"
            }
            "partitioned_16x4_gray_32.avif" | "partitioned_4x16_gray_32.avif" => {
                "1d3659ada1bf4b80ae974a7b544090591793cb954ac3f9ad13d3af3f09c21967"
            }
            "partitioned_16x4_green.avif" | "partitioned_4x16_green.avif" => {
                "32e7c45e59200de4c1012eac0ef31f3fa35d02b40d563f4602644bca9266f7fc"
            }
            "partitioned_12x8_a.avif" | "partitioned_8x12_a.avif" => {
                "47c4a5d65d8ac82aa68f04754b38e5bf00438aeb64b2e48c2bb54a9268e6e4e7"
            }
            "partitioned_12x8_gray_32.avif" | "partitioned_8x12_gray_32.avif" => {
                "a80ec409692fd6c32b82fa895a118a06751d63671cd6da6ed14ef5bb59f41541"
            }
            "partitioned_12x8_green.avif" | "partitioned_8x12_green.avif" => {
                "c1046797ae8db85c1b32d232085bdc2251d6e94567771f20ce9f86b6a2cc5cbc"
            }
            "partitioned_16x8_a.avif" | "partitioned_8x16_a.avif" => {
                "983aef668db1ea0d5801725fdf2b49d32232fc7f1d9ae578a03ffad6aebc4fc2"
            }
            "partitioned_16x8_gray_32.avif" | "partitioned_8x16_gray_32.avif" => {
                "f89d41f00d89e8b0bf8cb8cff89f9f23e9fa1e5113473dda8d16098575db7388"
            }
            "partitioned_16x8_green.avif" | "partitioned_8x16_green.avif" => {
                "ff87dfd10bc6c01f8e9dac23bb518192e6579a383b2ff1bbd8b8c80a58e677b4"
            }
            fixture => panic!("unexpected portable AVIF fixture: {fixture}"),
        };
        assert_eq!(case.pillow.sha256, expected_pillow_sha256);
        let expected_rgb = case
            .pillow
            .row_bytes
            .iter()
            .flat_map(|row| {
                assert_eq!(
                    row.len(),
                    usize::try_from(case.pillow.size[0])
                        .expect("Pillow RGB width")
                        .saturating_mul(6)
                );
                row.as_bytes().chunks_exact(2).map(|pair| {
                    let pair = std::str::from_utf8(pair).expect("hex pair must be UTF-8");
                    u8::from_str_radix(pair, 16).expect("hex pair must be valid")
                })
            })
            .collect::<Vec<_>>();
        let decoded = require_ok(
            img::decode(&input),
            "closed-class AVIF must decode through the production path",
        );
        assert_eq!(decoded.format, img::ImageFormat::Avif);
        assert_eq!(
            (
                decoded.content.width,
                decoded.content.height,
                decoded.content.mode,
                decoded.content.color,
            ),
            (
                case.portable_color.width,
                case.portable_color.height,
                img::ImageMode::Rgb8,
                img::ColorType::Rgb8,
            )
        );
        assert_eq!(
            decoded.content.pixels, expected_rgb,
            "portable AVIF RGB case {case_index}"
        );
        if case_index == 0
            || matches!(
                case.fixture.as_str(),
                "partitioned_12x4_a.avif" | "partitioned_4x12_a.avif"
            )
        {
            img::__coverage_sweep_av1_first_leaf(&input);
        }
    }

    for fixture in [
        "baseline.avif",
        "alpha.avif",
        "animated.avif",
        "10bit.avif",
        "multitile.avif",
    ] {
        let input = require_ok(
            fs::read(
                fixture_root
                    .join("input")
                    .join("images")
                    .join("avif")
                    .join(fixture),
            ),
            "non-portable AVIF fixture must be readable",
        );
        assert!(
            img::__coverage_av1_reconstruction(&input).is_none(),
            "{fixture} must not be classified as the closed portable still"
        );
    }
}

// ── Manifest Coverage ────────────────────────────────────────────────────

#[test]
fn test_coverage_matrix() {
    let matrix = require_some(
        coverage_matrix(),
        "coverage_matrix.json is required; run scripts/generate_decode_refs.py to regenerate it",
    );

    let s = &matrix.summary;
    eprintln!(
        "Coverage: {}/{} decode active, {} planned, {} encode not wired, {} assets",
        s.decode_active, s.decode_rows, s.decode_planned, s.encode_not_wired, s.assets_available
    );

    assert!(s.total_rows > 0, "Matrix must have rows");
    assert_eq!(s.total_rows, s.decode_rows + s.encode_rows);
}
