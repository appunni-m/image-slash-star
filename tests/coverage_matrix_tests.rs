//! Coverage matrix tests — driven by tests/fixtures/coverage_matrix.json
//! Each row in the matrix is one test assertion.
//! Decode: load asset → decode → compare pixel bytes with PIL reference bytes.
//! Encode: decode reference → encode with params → decode → compare pixel bytes.

use std::collections::{HashMap, HashSet, hash_map::Entry};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use bytemuck as _;
use image_slash_star as img;
#[cfg(feature = "jpeg")]
use wide as _;

#[path = "support/sha256.rs"]
mod sha256;
mod support;

use support::json::{self, FromJson, Object, Value};

static COVERAGE_MATRIX: OnceLock<Option<CoverageMatrix>> = OnceLock::new();

// Encode rows are partitioned into several integration-test functions for
// parallel execution. Keep their immutable, fixture-derived source sequences
// shared within this process so a repeated source asset is decoded once even
// when it crosses a partition boundary. Rows that mutate sequence metadata or
// pixels still clone the cached value below; the cache never becomes an
// assertion or coverage-only input.
type CachedEncodeSequence = Result<Arc<img::DecodedSequence>, String>;

static ENCODE_SEQUENCE_CACHE: OnceLock<
    Mutex<HashMap<PathBuf, Arc<OnceLock<CachedEncodeSequence>>>>,
> = OnceLock::new();

fn cached_encode_sequence(path: &Path, bytes: &[u8]) -> CachedEncodeSequence {
    let cache = ENCODE_SEQUENCE_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let entry = {
        let mut cache = cache
            .lock()
            .map_err(|_| "encode fixture cache mutex must not be poisoned".to_owned())?;
        cache
            .entry(path.to_owned())
            .or_insert_with(|| Arc::new(OnceLock::new()))
            .clone()
    };
    entry
        .get_or_init(|| {
            img::decode_sequence(bytes)
                .map(|decoded| Arc::new(decoded.content))
                .map_err(|error| error.to_string())
        })
        .clone()
}

fn matrix_verbose() -> bool {
    static MATRIX_VERBOSE: OnceLock<bool> = OnceLock::new();
    *MATRIX_VERBOSE.get_or_init(|| std::env::var_os("IMAGE_SLASH_STAR_VERBOSE_MATRIX").is_some())
}

macro_rules! matrix_success {
    ($($arg:tt)*) => {
        if matrix_verbose() {
            eprintln!($($arg)*);
        }
    };
}

// Each format owns an independent slice of the manifest. Keeping those slices
// as separate integration tests lets the Rust harness overlap codec work while
// preserving every active row and the same per-format contract assertions.

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
    gap: Option<String>,
    pure_rust_work_item: Option<String>,
    former_native_only: bool,
    asset: Option<String>,
    asset_path: Option<String>,
    asset_sha256: Option<String>,
    execution: Option<ExecutionRef>,
    assertion_origins: HashMap<String, String>,
    operations: HashMap<String, String>,
    error_contracts: HashMap<String, ErrorContractRef>,
    expect_error: Option<bool>,
    expect_sequence_error: bool,
    rust_expect_sequence_error: bool,
    rust_sequence_error_kind: Option<String>,
    rust_sequence_error_reason: Option<String>,
    oracle_detects_format: bool,
    oracle_status: Option<String>,
    oracle_error_type: Option<String>,
    oracle_error_message: Option<String>,
    oracle_error_kind: Option<String>,
    inspect_status: String,
    inspect_error_type: Option<String>,
    inspect_error_message: Option<String>,
    inspect_error_kind: Option<String>,
    inspect_container_format: Option<String>,
    inspect_cursor_hotspot: Option<Vec<u16>>,
    inspect_source_byte_order: Option<String>,
    inspect_source_byte_order_origin: Option<String>,
    ref_bit_depth: Option<u32>,
    ref_bit_depth_origin: Option<String>,
    verify_status: String,
    verify_error_type: Option<String>,
    verify_error_message: Option<String>,
    verify_error_kind: Option<String>,
    verification_scope: String,
    ref_mode: Option<String>,
    ref_size: Option<Vec<u32>>,
    ref_frame_count: Option<u32>,
    ref_is_animated: Option<bool>,
    inspect_palette: Option<PaletteParityRef>,
    decoded_palette: Option<PaletteParityRef>,
    decoded_source_byte_order: Option<String>,
    decoded_source_byte_order_origin: Option<String>,
    ref_path: Option<String>,
    ref_bytes: Option<usize>,
    ref_sha256: Option<String>,
    sequence_status: Option<String>,
    sequence_error_type: Option<String>,
    sequence_error_message: Option<String>,
    sequence_error_kind: Option<String>,
    sequence: Option<SequenceParityRef>,
}

#[derive(Debug)]
struct SequenceParityRef {
    canvas_size: Vec<u32>,
    canvas_origin: String,
    loop_count: Option<u32>,
    loop_origin: String,
    background: Option<BackgroundParityRef>,
    frames: Vec<FrameParityRef>,
}

#[derive(Debug)]
struct FrameParityRef {
    index: usize,
    source_rect: Vec<u32>,
    duration_num: u64,
    duration_den: u64,
    duration_origin: String,
    disposal: String,
    blend: String,
    interlaced: bool,
    is_default_image: bool,
    pixel_layout: String,
    source_origin: String,
    source_byte_order: Option<String>,
    source_byte_order_origin: Option<String>,
    pixel_assertion: String,
    pixel_origin: Option<String>,
    ref_path: Option<String>,
    ref_bytes: Option<usize>,
    ref_sha256: Option<String>,
    ref_mode: Option<String>,
    ref_size: Option<Vec<u32>>,
}

#[derive(Debug)]
struct BackgroundParityRef {
    palette_index: Option<u16>,
    rgba: Option<Vec<u16>>,
    origin: String,
}

#[derive(Debug)]
struct PaletteParityRef {
    state: String,
    origin: String,
    rgb_path: Option<String>,
    rgb_bytes: Option<usize>,
    rgb_sha256: Option<String>,
    alpha_path: Option<String>,
    alpha_bytes: Option<usize>,
    alpha_sha256: Option<String>,
}

#[derive(Debug)]
struct ExecutionRef {
    target: String,
    features: Vec<String>,
    suite: String,
}

#[derive(Debug)]
struct ErrorContractRef {
    pillow_type: Option<String>,
    pillow_message: Option<String>,
    rust_kind: String,
    rust_format: Option<String>,
    rust_message: String,
    origin: String,
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
    gap: Option<String>,
    pure_rust_work_item: Option<String>,
    former_native_only: bool,
    expect_error: bool,
    rust_expect_error: bool,
    rust_error_kind: Option<String>,
    rust_error_reason: Option<String>,
    oracle_status: Option<String>,
    oracle_error_type: Option<String>,
    oracle_error_message: Option<String>,
    oracle_error_kind: Option<String>,
    source_format: Option<String>,
    source_asset: Option<String>,
    source_mode: String,
    source_sha256: Option<String>,
    execution: Option<ExecutionRef>,
    assertion_origins: HashMap<String, String>,
    operations: HashMap<String, String>,
    error_contracts: HashMap<String, ErrorContractRef>,
    ref_bytes: Option<usize>,
    ref_sha256: Option<String>,
    ref_mode: Option<String>,
    ref_size: Option<Vec<u32>>,
    ref_path: Option<String>,
    encoded_ref_path: Option<String>,
    encoded_ref_bytes: Option<usize>,
    encoded_ref_sha256: Option<String>,
    sequence: Option<SequenceParityRef>,
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
    decoder_events: Vec<Value>,
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
            gap: object.take("gap")?,
            pure_rust_work_item: object.take("pure_rust_work_item")?,
            former_native_only: object.take_or_default("former_native_only")?,
            asset: object.take("asset")?,
            asset_path: object.take("asset_path")?,
            asset_sha256: object.take("asset_sha256")?,
            execution: object.take("execution")?,
            assertion_origins: object.take_or_default("assertion_origins")?,
            operations: object.take_or_default("operations")?,
            error_contracts: object.take_or_default("error_contracts")?,
            expect_error: object.take("expect_error")?,
            expect_sequence_error: object.take_or_default("expect_sequence_error")?,
            rust_expect_sequence_error: object.take_or_default("rust_expect_sequence_error")?,
            rust_sequence_error_kind: object.take("rust_sequence_error_kind")?,
            rust_sequence_error_reason: object.take("rust_sequence_error_reason")?,
            oracle_detects_format: object.take("oracle_detects_format")?,
            oracle_status: object.take("oracle_status")?,
            oracle_error_type: object.take("oracle_error_type")?,
            oracle_error_message: object.take("oracle_error_message")?,
            oracle_error_kind: object.take("oracle_error_kind")?,
            inspect_status: object.take("inspect_status")?,
            inspect_error_type: object.take("inspect_error_type")?,
            inspect_error_message: object.take("inspect_error_message")?,
            inspect_error_kind: object.take("inspect_error_kind")?,
            inspect_container_format: object.take("inspect_container_format")?,
            inspect_cursor_hotspot: object.take("inspect_cursor_hotspot")?,
            inspect_source_byte_order: object.take("inspect_source_byte_order")?,
            inspect_source_byte_order_origin: object.take("inspect_source_byte_order_origin")?,
            ref_bit_depth: object.take("ref_bit_depth")?,
            ref_bit_depth_origin: object.take("ref_bit_depth_origin")?,
            verify_status: object.take("verify_status")?,
            verify_error_type: object.take("verify_error_type")?,
            verify_error_message: object.take("verify_error_message")?,
            verify_error_kind: object.take("verify_error_kind")?,
            verification_scope: object.take("verification_scope")?,
            ref_mode: object.take("ref_mode")?,
            ref_size: object.take("ref_size")?,
            ref_frame_count: object.take("ref_frame_count")?,
            ref_is_animated: object.take("ref_is_animated")?,
            inspect_palette: object.take("inspect_palette")?,
            decoded_palette: object.take("decoded_palette")?,
            decoded_source_byte_order: object.take("decoded_source_byte_order")?,
            decoded_source_byte_order_origin: object.take("decoded_source_byte_order_origin")?,
            ref_path: object.take("ref_path")?,
            ref_bytes: object.take("ref_bytes")?,
            ref_sha256: object.take("ref_sha256")?,
            sequence_status: object.take("sequence_status")?,
            sequence_error_type: object.take("sequence_error_type")?,
            sequence_error_message: object.take("sequence_error_message")?,
            sequence_error_kind: object.take("sequence_error_kind")?,
            sequence: object.take("sequence")?,
        })
    }
}

json_object!(SequenceParityRef {
    canvas_size,
    canvas_origin,
    loop_count,
    loop_origin,
    background,
    frames,
});
json_object!(FrameParityRef {
    index,
    source_rect,
    duration_num,
    duration_den,
    duration_origin,
    disposal,
    blend,
    interlaced,
    is_default_image,
    pixel_layout,
    source_origin,
    source_byte_order,
    source_byte_order_origin,
    pixel_assertion,
    pixel_origin,
    ref_path,
    ref_bytes,
    ref_sha256,
    ref_mode,
    ref_size,
});
json_object!(BackgroundParityRef {
    palette_index,
    rgba,
    origin,
});
json_object!(PaletteParityRef {
    state,
    origin,
    rgb_path,
    rgb_bytes,
    rgb_sha256,
    alpha_path,
    alpha_bytes,
    alpha_sha256,
});
json_object!(ExecutionRef {
    target,
    features,
    suite,
});
json_object!(ErrorContractRef {
    pillow_type,
    pillow_message,
    rust_kind,
    rust_format,
    rust_message,
    origin,
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
            gap: object.take("gap")?,
            pure_rust_work_item: object.take("pure_rust_work_item")?,
            former_native_only: object.take_or_default("former_native_only")?,
            expect_error: object.take_or_default("expect_error")?,
            rust_expect_error: object.take_or_default("rust_expect_error")?,
            rust_error_kind: object.take("rust_error_kind")?,
            rust_error_reason: object.take("rust_error_reason")?,
            oracle_status: object.take("oracle_status")?,
            oracle_error_type: object.take("oracle_error_type")?,
            oracle_error_message: object.take("oracle_error_message")?,
            oracle_error_kind: object.take("oracle_error_kind")?,
            source_format: object.take("source_format")?,
            source_asset: object.take("source_asset")?,
            source_mode: object.take("source_mode")?,
            source_sha256: object.take("source_sha256")?,
            execution: object.take("execution")?,
            assertion_origins: object.take_or_default("assertion_origins")?,
            operations: object.take_or_default("operations")?,
            error_contracts: object.take_or_default("error_contracts")?,
            ref_bytes: object.take("ref_bytes")?,
            ref_sha256: object.take("ref_sha256")?,
            ref_mode: object.take("ref_mode")?,
            ref_size: object.take("ref_size")?,
            ref_path: object.take("ref_path")?,
            encoded_ref_path: object.take("encoded_ref_path")?,
            encoded_ref_bytes: object.take("encoded_ref_bytes")?,
            encoded_ref_sha256: object.take("encoded_ref_sha256")?,
            sequence: object.take("sequence")?,
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
    decoder_events,
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

fn legacy_encode_options(format: &str, params: &HashMap<String, Value>) -> Vec<(String, String)> {
    let keys: &[&str] = match format {
        "jpeg" => &[
            "quality",
            "progressive",
            "optimize",
            "subsampling",
            "restart_interval",
            "exif_hex",
        ],
        "png" => &[
            "compression",
            "optimize",
            "interlace",
            "interlaced",
            "gamma",
            "srgb",
            "physical",
            "text_chunks",
            "time",
        ],
        "gif" => &[
            "animated",
            "interlace",
            "interlaced",
            "disposal",
            "color_table",
            "transparency",
            "loop",
        ],
        "bmp" => &[],
        "tiff" => &["compression", "predictor"],
        "webp" => &[
            "quality",
            "lossless",
            "method",
            "icc_hex",
            "exif_hex",
            "xmp_hex",
            "kmax",
            "kmin",
            "minimize_size",
            "allow_mixed",
        ],
        "ico" => &["entry_type", "sizes"],
        "avif" => &[
            "quality",
            "codec",
            "subsampling",
            "range",
            "speed",
            "max_threads",
            "tile_rows",
            "tile_cols",
            "alpha_premultiplied",
            "autotiling",
            "icc_hex",
            "exif_hex",
            "exif_orientation",
            "xmp_hex",
            "sequence_time",
        ],
        _ => &[],
    };
    params
        .iter()
        .filter(|(key, _)| keys.contains(&key.as_str()))
        .map(|(key, value)| (key.clone(), option_text(value)))
        .collect()
}

fn advanced_encode_options(params: &HashMap<String, Value>) -> Vec<img::AvifAdvancedOption> {
    params
        .get("advanced")
        .and_then(Value::as_object)
        .map(|values| {
            values
                .iter()
                .map(|(key, value)| img::AvifAdvancedOption {
                    key: key.clone(),
                    value: option_text(value),
                })
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
    let mut has_adobe_app14 = false;
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
        if marker == 0xee && payload.starts_with(b"Adobe\0") {
            has_adobe_app14 = true;
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
    if params.get("color").and_then(Value::as_str) == Some("cmyk") {
        if sof_data[5] != 4 || sof_data.len() < 18 {
            return Err("JPEG CMYK output does not have four components".to_owned());
        }
        let expected_components = [(b'C', 0x11), (b'M', 0x11), (b'Y', 0x11), (b'K', 0x11)];
        for (index, (expected_id, expected_sampling)) in expected_components.into_iter().enumerate()
        {
            let component = 6usize.saturating_add(index.saturating_mul(3));
            if sof_data[component] != expected_id
                || sof_data[component.saturating_add(1)] != expected_sampling
            {
                return Err("JPEG CMYK component layout does not match Pillow".to_owned());
            }
        }
        if !has_adobe_app14 {
            return Err("JPEG CMYK output has no Adobe APP14 marker".to_owned());
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

fn assert_sha256(bytes: &[u8], expected: Option<&str>, label: &str) -> Result<(), String> {
    let expected = expected.ok_or_else(|| format!("{label} SHA-256 is missing"))?;
    let actual = sha256::digest_hex(bytes);
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "{label} SHA-256 mismatch: actual {actual}, expected {expected}"
        ))
    }
}

fn assert_execution_contract(expected: Option<&ExecutionRef>) -> Result<(), String> {
    let expected = expected.ok_or_else(|| "execution contract is missing".to_owned())?;
    let all_features = ["jpeg", "png", "gif", "bmp", "tiff", "webp", "ico", "avif"];
    if expected.target != "aarch64-apple-darwin"
        || expected.suite != "native_all_features"
        || expected
            .features
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            != all_features
        || !cfg!(all(
            target_arch = "aarch64",
            target_os = "macos",
            feature = "jpeg",
            feature = "png",
            feature = "gif",
            feature = "bmp",
            feature = "tiff",
            feature = "webp",
            feature = "ico",
            feature = "avif"
        ))
    {
        return Err(format!(
            "execution contract differs from this test lane: {expected:?}"
        ));
    }
    Ok(())
}

fn is_assertion_origin(origin: &str) -> bool {
    matches!(
        origin,
        "pillow_fixture"
            | "specification_reference"
            | "independent_implementation"
            | "defensive_model"
    )
}

fn assert_origins(origins: &HashMap<String, String>, required: &[&str]) -> Result<(), String> {
    for field in required {
        if !origins.contains_key(*field) {
            return Err(format!("assertion origin for {field} is missing"));
        }
    }
    for (field, origin) in origins {
        if !is_assertion_origin(origin) {
            return Err(format!("assertion origin for {field} is invalid: {origin}"));
        }
    }
    Ok(())
}

fn assert_operation_contract(
    operations: &HashMap<String, String>,
    required: &[&str],
) -> Result<(), String> {
    if operations.len() != required.len() {
        return Err(format!(
            "operation contract has {} entries, expected {}",
            operations.len(),
            required.len()
        ));
    }
    for operation in required {
        let status = operations
            .get(*operation)
            .ok_or_else(|| format!("{operation} expectation is missing"))?;
        if !matches!(status.as_str(), "ok" | "error" | "not_applicable") {
            return Err(format!("{operation} expectation {status:?} is invalid"));
        }
    }
    Ok(())
}

fn image_error_kind_name(kind: img::ImageErrorKind) -> &'static str {
    match kind {
        img::ImageErrorKind::UnknownFormat => "unknown_format",
        img::ImageErrorKind::FeatureDisabled => "feature_disabled",
        img::ImageErrorKind::Malformed => "malformed",
        img::ImageErrorKind::Unsupported => "unsupported",
        img::ImageErrorKind::Dimensions => "dimensions",
        img::ImageErrorKind::Parameter => "parameter",
        _ => "unknown",
    }
}

fn assert_error_contracts(
    operations: &HashMap<String, String>,
    contracts: &HashMap<String, ErrorContractRef>,
    format: &str,
) -> Result<(), String> {
    let expected_count = operations
        .values()
        .filter(|status| status.as_str() == "error")
        .count();
    if contracts.len() != expected_count {
        return Err(format!(
            "error contract has {} entries, expected {expected_count}",
            contracts.len()
        ));
    }
    for (operation, status) in operations {
        let contract = contracts.get(operation);
        if status == "error" && contract.is_none() {
            return Err(format!("{operation} error contract is missing"));
        }
        if status != "error" && contract.is_some() {
            return Err(format!("{operation} has a stale error contract"));
        }
    }
    for (operation, contract) in contracts {
        if !matches!(
            contract.rust_kind.as_str(),
            "unknown_format"
                | "feature_disabled"
                | "malformed"
                | "unsupported"
                | "dimensions"
                | "parameter"
        ) {
            return Err(format!(
                "{operation} Rust error kind is invalid: {:?}",
                contract.rust_kind
            ));
        }
        let expected_format = if contract.rust_kind == "unknown_format" {
            None
        } else {
            Some(
                format_from_name(format)
                    .ok_or_else(|| format!("unknown error-contract format {format:?}"))?
                    .as_str(),
            )
        };
        if contract.rust_format.as_deref() != expected_format {
            return Err(format!(
                "{operation} Rust error format is {:?}, expected {expected_format:?}",
                contract.rust_format
            ));
        }
        let expected_message =
            if contract.rust_kind == "unknown_format" || contract.rust_kind == "feature_disabled" {
                "none"
            } else {
                "non_empty"
            };
        if contract.rust_message != expected_message {
            return Err(format!(
                "{operation} Rust message policy is {:?}, expected {expected_message:?}",
                contract.rust_message
            ));
        }
        match contract.origin.as_str() {
            "pillow_fixture" => {
                let has_type = contract
                    .pillow_type
                    .as_deref()
                    .is_some_and(|value| !value.is_empty());
                let has_message = contract.pillow_message.is_some();
                if has_type != has_message {
                    return Err(format!(
                        "{operation} Pillow error type/message evidence is incomplete"
                    ));
                }
                if !has_type && contract.rust_kind != "unknown_format" {
                    return Err(format!(
                        "{operation} non-signature error lacks Pillow exception evidence"
                    ));
                }
            }
            "defensive_model" => {
                if contract.pillow_type.is_some() || contract.pillow_message.is_some() {
                    return Err(format!(
                        "{operation} defensive Rust contract must not claim a Pillow exception"
                    ));
                }
            }
            "specification_reference" | "independent_implementation" => {
                if contract.pillow_type.is_some() || contract.pillow_message.is_some() {
                    return Err(format!(
                        "{operation} non-Pillow contract must not claim a Pillow exception"
                    ));
                }
            }
            origin => {
                return Err(format!(
                    "{operation} error-contract origin is invalid: {origin:?}"
                ));
            }
        }
    }
    Ok(())
}

fn assert_result_error_contract<T>(
    result: &img::ImageResult<T>,
    contracts: &HashMap<String, ErrorContractRef>,
    operation: &str,
) -> Result<(), String> {
    let Some(contract) = contracts.get(operation) else {
        return if result.is_ok() {
            Ok(())
        } else {
            Err(format!("{operation} returned an undeclared error"))
        };
    };
    let error = match result {
        Err(error) => error,
        Ok(_) => return Err(format!("{operation} was required to return an error")),
    };
    let actual_kind = image_error_kind_name(error.kind());
    if actual_kind != contract.rust_kind {
        return Err(format!(
            "{operation} Rust error kind is {actual_kind:?}, expected {:?}",
            contract.rust_kind
        ));
    }
    let actual_format = error.format().map(img::ImageFormat::as_str);
    if actual_format != contract.rust_format.as_deref() {
        return Err(format!(
            "{operation} Rust error format is {actual_format:?}, expected {:?}",
            contract.rust_format
        ));
    }
    match contract.rust_message.as_str() {
        "none" if error.message().is_none() => Ok(()),
        "non_empty" if error.message().is_some_and(|message| !message.is_empty()) => Ok(()),
        policy => Err(format!(
            "{operation} Rust error message violates policy {policy:?}: {:?}",
            error.message()
        )),
    }
}

fn operation_status<'a>(
    operations: &'a HashMap<String, String>,
    operation: &str,
) -> Result<&'a str, String> {
    operations
        .get(operation)
        .map(String::as_str)
        .ok_or_else(|| format!("{operation} expectation is missing"))
}

fn result_has_status<T>(result: &img::ImageResult<T>, expected: &str) -> bool {
    matches!((result, expected), (Ok(_), "ok") | (Err(_), "error"))
}

fn load_exact_reference(
    manifest_dir: &Path,
    path: Option<&str>,
    expected_bytes: Option<usize>,
    expected_sha256: Option<&str>,
    label: &str,
) -> Result<Option<Vec<u8>>, String> {
    match (path, expected_bytes) {
        (None, None) => Ok(None),
        (Some(path), Some(expected_bytes)) => {
            let bytes = fs::read(manifest_dir.join(path))
                .map_err(|error| format!("{label} reference is unreadable: {error}"))?;
            if bytes.len() != expected_bytes {
                return Err(format!(
                    "{label} reference has {} bytes, expected {expected_bytes}",
                    bytes.len()
                ));
            }
            assert_sha256(&bytes, expected_sha256, label)?;
            Ok(Some(bytes))
        }
        _ => Err(format!("{label} reference path/length/hash is incomplete")),
    }
}

fn assert_palette_parity(
    manifest_dir: &Path,
    expected: Option<&PaletteParityRef>,
    mode: img::ImageMode,
    actual: Option<&img::ImagePalette>,
    label: &str,
) -> Result<(), String> {
    let expected = expected.ok_or_else(|| format!("{label} palette evidence is missing"))?;
    if !matches!(
        expected.origin.as_str(),
        "pillow_fixture"
            | "specification_reference"
            | "independent_implementation"
            | "defensive_model"
    ) {
        return Err(format!("{label} palette origin is invalid"));
    }
    match expected.state.as_str() {
        "absent" => {
            if mode == img::ImageMode::P8 || actual.is_some() {
                return Err(format!("{label} palette should be absent"));
            }
        }
        "implicit" => {
            if mode != img::ImageMode::P8 || actual.is_some() {
                return Err(format!(
                    "{label} should be indexed without an explicit table"
                ));
            }
        }
        "table" => {
            if mode != img::ImageMode::P8 {
                return Err(format!("{label} palette table requires indexed mode"));
            }
            let actual = actual.ok_or_else(|| format!("{label} palette table is missing"))?;
            let rgb = load_exact_reference(
                manifest_dir,
                expected.rgb_path.as_deref(),
                expected.rgb_bytes,
                expected.rgb_sha256.as_deref(),
                &format!("{label} RGB"),
            )?
            .ok_or_else(|| format!("{label} RGB reference is missing"))?;
            let alpha = load_exact_reference(
                manifest_dir,
                expected.alpha_path.as_deref(),
                expected.alpha_bytes,
                expected.alpha_sha256.as_deref(),
                &format!("{label} alpha"),
            )?
            .unwrap_or_default();
            if actual.rgb != rgb || actual.alpha != alpha {
                return Err(format!(
                    "{label} palette differs: RGB {}/{} bytes, alpha {}/{} bytes",
                    actual.rgb.len(),
                    rgb.len(),
                    actual.alpha.len(),
                    alpha.len()
                ));
            }
        }
        state => return Err(format!("{label} palette state {state:?} is invalid")),
    }
    if expected.state != "table"
        && (expected.rgb_path.is_some()
            || expected.rgb_bytes.is_some()
            || expected.rgb_sha256.is_some()
            || expected.alpha_path.is_some()
            || expected.alpha_bytes.is_some()
            || expected.alpha_sha256.is_some())
    {
        return Err(format!("{label} non-table palette retains byte references"));
    }
    Ok(())
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
        "Rgb32F" => Some(img::ImageMode::Rgb32F),
        "Rgba32F" => Some(img::ImageMode::Rgba32F),
        "F" | "F32" => Some(img::ImageMode::F32),
        "I" | "I32" => Some(img::ImageMode::I32),
        _ => None,
    }
}

fn zero_image_for_mode(
    width: u32,
    height: u32,
    mode: img::ImageMode,
) -> Result<img::DecodedImage, String> {
    let width_usize = usize::try_from(width).map_err(|_| "mode width is too large")?;
    let height_usize = usize::try_from(height).map_err(|_| "mode height is too large")?;
    let len = if mode == img::ImageMode::L1 {
        width_usize
            .div_ceil(8)
            .checked_mul(height_usize)
            .ok_or("mode byte length overflows")?
    } else {
        width_usize
            .checked_mul(height_usize)
            .and_then(|pixels| pixels.checked_mul(usize::from(mode.color_type().bytes_per_pixel())))
            .ok_or("mode byte length overflows")?
    };
    let image = img::DecodedImage::with_mode(width, height, vec![0; len], mode);
    if mode == img::ImageMode::P8 {
        Ok(image.with_palette(
            img::ImagePalette::new(vec![0, 0, 0], Vec::new()).map_err(|error| error.to_string())?,
        ))
    } else {
        Ok(image)
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
        let first_difference = expected
            .iter()
            .zip(actual)
            .position(|(expected, actual)| expected != actual)
            .unwrap_or(expected.len().min(actual.len()));
        let actual_byte = actual.get(first_difference).copied();
        let expected_byte = expected.get(first_difference).copied();
        return Err(format!(
            "encoded byte length mismatch: actual {}, expected {}; first difference at byte {}: actual {:02x?}, expected {:02x?}",
            actual.len(),
            expected.len(),
            first_difference,
            actual_byte,
            expected_byte,
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

fn expected_frame_disposal(value: &str) -> Result<img::FrameDisposal, String> {
    match value {
        "unspecified" => Ok(img::FrameDisposal::Unspecified),
        "keep" => Ok(img::FrameDisposal::Keep),
        "background" => Ok(img::FrameDisposal::Background),
        "previous" => Ok(img::FrameDisposal::Previous),
        value => value
            .strip_prefix("reserved:")
            .ok_or_else(|| format!("unknown frame disposal {value}"))?
            .parse::<u8>()
            .map(img::FrameDisposal::Reserved)
            .map_err(|error| format!("invalid reserved frame disposal {value}: {error}")),
    }
}

fn expected_frame_blend(value: &str) -> Result<img::FrameBlend, String> {
    match value {
        "unspecified" => Ok(img::FrameBlend::Unspecified),
        "source" => Ok(img::FrameBlend::Source),
        "over" => Ok(img::FrameBlend::Over),
        value => value
            .strip_prefix("reserved:")
            .ok_or_else(|| format!("unknown frame blend {value}"))?
            .parse::<u8>()
            .map(img::FrameBlend::Reserved)
            .map_err(|error| format!("invalid reserved frame blend {value}: {error}")),
    }
}

fn expected_source_byte_order(
    value: Option<&str>,
    origin: Option<&str>,
    label: &str,
) -> Result<Option<img::SourceByteOrder>, String> {
    match (value, origin) {
        (None, None) => Ok(None),
        (Some(value), Some(origin)) if is_assertion_origin(origin) => match value {
            "little" => Ok(Some(img::SourceByteOrder::Little)),
            "big" => Ok(Some(img::SourceByteOrder::Big)),
            _ => Err(format!("{label} has unknown source byte order {value}")),
        },
        _ => Err(format!(
            "{label} source byte order and evidence origin are incomplete"
        )),
    }
}

fn expected_background(
    expected: Option<&BackgroundParityRef>,
) -> Result<Option<img::AnimationBackground>, String> {
    let Some(expected) = expected else {
        return Ok(None);
    };
    if !is_assertion_origin(&expected.origin) {
        return Err(format!(
            "sequence background origin is invalid: {}",
            expected.origin
        ));
    }
    match (expected.palette_index, expected.rgba.as_deref()) {
        (Some(index), None) => u8::try_from(index)
            .map(img::AnimationBackground::PaletteIndex)
            .map(Some)
            .map_err(|_| "sequence palette background exceeds u8".to_owned()),
        (None, Some(channels)) => {
            let [red, green, blue, alpha] = <[u16; 4]>::try_from(channels)
                .map_err(|_| "sequence RGBA background must have four channels".to_owned())?;
            let channel = |value| {
                u8::try_from(value)
                    .map_err(|_| "sequence RGBA background channel exceeds u8".to_owned())
            };
            let rgba = [
                channel(red)?,
                channel(green)?,
                channel(blue)?,
                channel(alpha)?,
            ];
            Ok(Some(img::AnimationBackground::Rgba(rgba)))
        }
        _ => {
            Err("sequence background must contain exactly one of palette_index or rgba".to_owned())
        }
    }
}

fn assert_sequence_frame_pixels(
    manifest_dir: &Path,
    row_id: &str,
    expected: &FrameParityRef,
    actual: &img::DecodedFrame,
) -> Result<(), String> {
    if expected.pixel_assertion == "not_asserted_source_layout" {
        if expected.pixel_origin.is_some()
            || expected.ref_path.is_some()
            || expected.ref_bytes.is_some()
            || expected.ref_sha256.is_some()
            || expected.ref_mode.is_some()
            || expected.ref_size.is_some()
        {
            return Err(format!(
                "frame {} claims no pixel assertion but retains pixel evidence",
                expected.index
            ));
        }
        return Ok(());
    }
    if expected.pixel_assertion != "exact" {
        return Err(format!(
            "frame {} has unknown pixel assertion {}",
            expected.index, expected.pixel_assertion
        ));
    }
    let pixel_origin = expected
        .pixel_origin
        .as_deref()
        .ok_or_else(|| format!("frame {} exact pixels lack an origin", expected.index))?;
    if !is_assertion_origin(pixel_origin) {
        return Err(format!(
            "frame {} pixel origin is invalid: {pixel_origin}",
            expected.index
        ));
    }
    let ref_path = expected
        .ref_path
        .as_deref()
        .ok_or_else(|| format!("frame {} exact pixels lack a path", expected.index))?;
    let ref_bytes = expected
        .ref_bytes
        .ok_or_else(|| format!("frame {} exact pixels lack a byte length", expected.index))?;
    let ref_sha256 = expected
        .ref_sha256
        .as_deref()
        .ok_or_else(|| format!("frame {} exact pixels lack SHA-256", expected.index))?;
    let ref_mode = expected
        .ref_mode
        .as_ref()
        .ok_or_else(|| format!("frame {} exact pixels lack a mode", expected.index))?;
    let ref_size = expected
        .ref_size
        .as_ref()
        .ok_or_else(|| format!("frame {} exact pixels lack dimensions", expected.index))?;

    let bytes = fs::read(manifest_dir.join(ref_path))
        .map_err(|error| format!("frame {} reference unreadable: {error}", expected.index))?;
    if bytes.len() != ref_bytes {
        return Err(format!(
            "frame {} reference length mismatch: actual {}, declared {}",
            expected.index,
            bytes.len(),
            ref_bytes
        ));
    }
    assert_sha256(
        &bytes,
        Some(ref_sha256),
        &format!("frame {} pixels", expected.index),
    )?;
    let reference = PixelParityRef {
        id: format!("{row_id} frame {}", expected.index),
        bytes,
        width: ref_size.first().copied(),
        height: ref_size.get(1).copied(),
        mode: Some(ref_mode.clone()),
    };
    assert_pixel_parity(&reference, &actual.image)
        .map_err(|message| format!("frame {}: {message}", expected.index))
}

fn assert_sequence_reference_parity(
    manifest_dir: &Path,
    row_id: &str,
    expected: &SequenceParityRef,
    actual: &img::DecodedSequence,
) -> Result<(), String> {
    actual
        .validate()
        .map_err(|error| format!("decoded sequence validation failed: {error}"))?;
    if !is_assertion_origin(&expected.canvas_origin) {
        return Err(format!(
            "sequence canvas origin is invalid: {}",
            expected.canvas_origin
        ));
    }
    if expected.canvas_size.as_slice() != [actual.width, actual.height] {
        return Err(format!(
            "sequence canvas mismatch: actual {}x{}, expected {:?}",
            actual.width, actual.height, expected.canvas_size
        ));
    }
    if !is_assertion_origin(&expected.loop_origin) {
        return Err(format!(
            "sequence loop origin is invalid: {}",
            expected.loop_origin
        ));
    }
    if actual.loop_count != expected.loop_count {
        return Err(format!(
            "loop count mismatch: actual {:?}, expected {:?}",
            actual.loop_count, expected.loop_count
        ));
    }
    let expected_background = expected_background(expected.background.as_ref())?;
    if actual.background != expected_background {
        return Err(format!(
            "sequence background mismatch: actual {:?}, expected {:?}",
            actual.background, expected_background
        ));
    }
    if actual.frames.len() != expected.frames.len() {
        return Err(format!(
            "frame count mismatch: actual {}, expected {}",
            actual.frames.len(),
            expected.frames.len()
        ));
    }
    for (index, (actual_frame, expected_frame)) in
        actual.frames.iter().zip(&expected.frames).enumerate()
    {
        if expected_frame.index != index {
            return Err(format!(
                "frame evidence index mismatch: position {index}, declared {}",
                expected_frame.index
            ));
        }
        if !is_assertion_origin(&expected_frame.duration_origin)
            || !is_assertion_origin(&expected_frame.source_origin)
        {
            return Err(format!(
                "frame {} source origins are invalid: duration={}, source={}",
                expected_frame.index, expected_frame.duration_origin, expected_frame.source_origin
            ));
        }
        let expected_rect =
            <[u32; 4]>::try_from(expected_frame.source_rect.as_slice()).map_err(|_| {
                format!(
                    "frame {} source rectangle must have four values",
                    expected_frame.index
                )
            })?;
        let actual_rect = actual_frame.source.rect;
        if [
            actual_rect.left,
            actual_rect.top,
            actual_rect.width,
            actual_rect.height,
        ] != expected_rect
        {
            return Err(format!(
                "frame {} source rectangle mismatch: actual {:?}, expected {:?}",
                expected_frame.index, actual_rect, expected_rect
            ));
        }
        let expected_duration = img::FrameDuration {
            numerator: expected_frame.duration_num,
            denominator: expected_frame.duration_den,
        };
        if actual_frame.source.duration != expected_duration {
            return Err(format!(
                "frame {} exact duration mismatch: actual {:?}, expected {:?}",
                expected_frame.index, actual_frame.source.duration, expected_duration
            ));
        }
        let expected_disposal = expected_frame_disposal(&expected_frame.disposal)?;
        if actual_frame.source.disposal != expected_disposal {
            return Err(format!(
                "frame {} disposal mismatch: actual {:?}, expected {:?}",
                expected_frame.index, actual_frame.source.disposal, expected_disposal
            ));
        }
        let expected_blend = expected_frame_blend(&expected_frame.blend)?;
        if actual_frame.source.blend != expected_blend {
            return Err(format!(
                "frame {} blend mismatch: actual {:?}, expected {:?}",
                expected_frame.index, actual_frame.source.blend, expected_blend
            ));
        }
        if actual_frame.source.interlaced != expected_frame.interlaced
            || actual_frame.source.is_default_image != expected_frame.is_default_image
        {
            return Err(format!(
                "frame {} storage flags mismatch: interlaced {} versus {}, default-image {} versus {}",
                expected_frame.index,
                actual_frame.source.interlaced,
                expected_frame.interlaced,
                actual_frame.source.is_default_image,
                expected_frame.is_default_image
            ));
        }
        let expected_layout = match expected_frame.pixel_layout.as_str() {
            "source_rectangle" => img::FramePixelLayout::SourceRectangle,
            "rendered_canvas" => img::FramePixelLayout::RenderedCanvas,
            value => {
                return Err(format!(
                    "frame {} has unknown pixel layout {value}",
                    expected_frame.index
                ));
            }
        };
        if actual_frame.pixel_layout != expected_layout {
            return Err(format!(
                "frame {} pixel layout mismatch: actual {:?}, expected {:?}",
                expected_frame.index, actual_frame.pixel_layout, expected_layout
            ));
        }
        let expected_byte_order = expected_source_byte_order(
            expected_frame.source_byte_order.as_deref(),
            expected_frame.source_byte_order_origin.as_deref(),
            &format!("frame {}", expected_frame.index),
        )?;
        if actual_frame.image.source.byte_order() != expected_byte_order {
            return Err(format!(
                "frame {} source byte order mismatch: actual {:?}, expected {:?}",
                expected_frame.index,
                actual_frame.image.source.byte_order(),
                expected_byte_order
            ));
        }
        assert_sequence_frame_pixels(manifest_dir, row_id, expected_frame, actual_frame)?;
    }
    Ok(())
}

fn assert_sequence_parity(manifest_dir: &Path, row: &DecodeRow, data: &[u8]) -> Result<(), String> {
    let operation = operation_status(&row.operations, "decode_sequence")?;
    if row.rust_expect_sequence_error {
        if operation != "error" {
            return Err("Rust sequence-error row lacks an error operation contract".to_owned());
        }
        if row.ref_is_animated != Some(true)
            || row
                .ref_frame_count
                .is_some_and(|frame_count| frame_count <= 1)
        {
            return Err(format!(
                "Rust-only sequence error lacks multi-frame Pillow evidence: count {:?}, animated {:?}",
                row.ref_frame_count, row.ref_is_animated
            ));
        }
        if row
            .rust_sequence_error_reason
            .as_deref()
            .unwrap_or_default()
            .is_empty()
        {
            return Err("Rust-only sequence error lacks a contract reason".to_owned());
        }
        let expected_format = format_from_name(&row.format)
            .ok_or_else(|| format!("unsupported manifest format {}", row.format))?;
        let actual = img::decode_sequence(data);
        if result_matches_oracle(
            &actual,
            "error",
            row.rust_sequence_error_kind.as_deref(),
            expected_format,
        ) && assert_result_error_contract(&actual, &row.error_contracts, "decode_sequence")
            .is_ok()
        {
            return Ok(());
        }
        return Err(format!(
            "sequence decode silently collapsed a Pillow-proven multi-frame source: {actual:?}"
        ));
    }
    if row.expect_sequence_error {
        if operation != "error" {
            return Err("sequence-error row lacks an error operation contract".to_owned());
        }
        let expected_format = format_from_name(&row.format)
            .ok_or_else(|| format!("unsupported manifest format {}", row.format))?;
        let actual = img::decode_sequence(data);
        if row.sequence_status.as_deref() == Some("error")
            && result_matches_oracle(
                &actual,
                "error",
                row.sequence_error_kind.as_deref(),
                expected_format,
            )
            && assert_result_error_contract(&actual, &row.error_contracts, "decode_sequence")
                .is_ok()
        {
            return Ok(());
        }
        return Err(format!(
            "sequence error differs from Pillow ({actual:?} versus {:?} {:?}: {:?})",
            row.sequence_status, row.sequence_error_type, row.sequence_error_message
        ));
    }
    let Some(expected) = &row.sequence else {
        let expected_format = format_from_name(&row.format)
            .ok_or_else(|| format!("unsupported manifest format {}", row.format))?;
        let actual = img::decode_sequence(data);
        if !result_has_status(&actual, operation) {
            return Err(format!(
                "decode_sequence violates operation contract {operation}: {actual:?}"
            ));
        }
        assert_result_error_contract(&actual, &row.error_contracts, "decode_sequence")?;
        if let Ok(decoded) = actual {
            if decoded.format != expected_format {
                return Err(format!(
                    "sequence source format mismatch: actual {:?}, expected {expected_format:?}",
                    decoded.format
                ));
            }
            decoded
                .content
                .validate()
                .map_err(|error| format!("decoded sequence validation failed: {error}"))?;
        }
        return Ok(());
    };
    if operation != "ok" {
        return Err("successful sequence row lacks an ok operation contract".to_owned());
    }
    let expected_format = format_from_name(&row.format)
        .ok_or_else(|| format!("unsupported manifest format {}", row.format))?;
    let decoded =
        img::decode_sequence(data).map_err(|error| format!("sequence decode failed: {error}"))?;
    if decoded.format != expected_format {
        return Err(format!(
            "sequence source format mismatch: actual {:?}, expected {expected_format:?}",
            decoded.format
        ));
    }
    let actual = decoded.content;
    assert_sequence_reference_parity(manifest_dir, &row.id, expected, &actual)
}

// ── Decode Tests ─────────────────────────────────────────────────────────

fn format_from_name(format: &str) -> Option<img::ImageFormat> {
    img::ImageFormat::from_name(format).ok()
}

fn error_matches_kind(
    error: &img::ImageError,
    expected_kind: Option<&str>,
    expected_format: img::ImageFormat,
) -> bool {
    match (expected_kind, error) {
        (Some("unknown_format"), img::ImageError::UnknownFormat) => true,
        (Some("malformed"), img::ImageError::Malformed { format: actual, .. }) => {
            *actual == expected_format
        }
        (
            Some("unsupported"),
            img::ImageError::Unsupported {
                format: Some(actual),
                ..
            },
        ) => *actual == expected_format,
        (Some("dimensions"), img::ImageError::Dimensions { .. })
        | (Some("parameter"), img::ImageError::Parameter { .. }) => true,
        _ => false,
    }
}

fn result_matches_oracle<T>(
    result: &img::ImageResult<T>,
    status: &str,
    error_kind: Option<&str>,
    expected_format: img::ImageFormat,
) -> bool {
    match (status, result) {
        ("ok", Ok(_)) => true,
        ("error", Err(error)) => error_matches_kind(error, error_kind, expected_format),
        _ => false,
    }
}

#[test]
fn test_decode_matrix_jpeg() {
    run_decode_matrix(Some("jpeg"));
}

#[test]
fn test_decode_matrix_png() {
    run_decode_matrix(Some("png"));
}

#[test]
fn test_decode_matrix_gif() {
    run_decode_matrix(Some("gif"));
}

#[test]
fn test_decode_matrix_bmp() {
    run_decode_matrix(Some("bmp"));
}

#[test]
fn test_decode_matrix_tiff() {
    run_decode_matrix(Some("tiff"));
}

#[test]
fn test_decode_matrix_webp() {
    run_decode_matrix(Some("webp"));
}

#[test]
fn test_decode_matrix_ico() {
    run_decode_matrix(Some("ico"));
}

#[test]
fn test_decode_matrix_avif() {
    run_decode_matrix(Some("avif"));
}

#[test]
fn test_avif_planned_gaps_are_explicit_safe_rust_contracts() {
    if !cfg!(feature = "avif") {
        return;
    }

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let matrix = require_some(
        coverage_matrix(),
        "coverage_matrix.json is required; run scripts/generate_decode_refs.py to regenerate it",
    );
    let avif = require_some(
        matrix.formats.get("avif"),
        "the coverage matrix must contain the AVIF format",
    );
    let former_native_decode = avif
        .decode
        .iter()
        .filter(|row| row.former_native_only)
        .collect::<Vec<_>>();
    let planned = avif
        .decode
        .iter()
        .filter(|row| row.status == "planned")
        .collect::<Vec<_>>();
    assert_eq!(
        former_native_decode.len(),
        7,
        "the AVIF former-native decode census changed"
    );
    assert!(
        former_native_decode
            .iter()
            .all(|row| row.status == "planned"),
        "a former-native AVIF decode row cannot become active without pure-Rust evidence"
    );
    assert_eq!(
        former_native_decode.len(),
        planned.len(),
        "every former-native AVIF decode row must remain an explicit planned gap"
    );
    assert_eq!(planned.len(), 7, "the AVIF planned-gap ledger changed");
    assert_eq!(
        planned.len(),
        matrix.summary.decode_planned,
        "all current planned decode rows must be AVIF rows"
    );

    let assets_dir = manifest_dir
        .join("tests")
        .join("fixtures")
        .join("input")
        .join("images")
        .join("avif");
    for row in planned {
        assert_eq!(row.format, "avif", "planned row has the wrong format");
        assert!(
            row.former_native_only,
            "planned AVIF row {} must explicitly identify former native-only provenance",
            row.id
        );
        assert!(
            row.gap.as_deref().is_some_and(|gap| !gap.trim().is_empty()),
            "planned AVIF row {} needs a concrete safe-Rust gap reason",
            row.id
        );
        let expected_work_item = match row.id.as_str() {
            "portable_lossy_420_q99_eob_bin_control"
            | "portable_lossy_420_q99_eob_base_control" => "AVF-ENTROPY-001",
            "high_bitdepth" => "AVF-SAMPLE-001",
            "with_alpha" => "AVF-ALPHA-001",
            "partitioned_square_12x12_g96_direct_tokens"
            | "partitioned_square_12x12_midpoint_g96_ac"
            | "partitioned_square_12x12_top_left_luma_eob4"
            | "partitioned_square_12x12_top_left_luma_eob12_control"
            | "partitioned_square_12x12_luma_eob1"
            | "partitioned_square_12x12_luma_eob2_control"
            | "partitioned_square_12x12_luma_eob4_control"
            | "partitioned_square_12x12_luma_eob6_control"
            | "partitioned_square_12x12_luma_eob9_control"
            | "partitioned_square_12x12_luma_eob10_control"
            | "partitioned_square_12x12_luma_eob12_control"
            | "partitioned_square_12x12_luma_eob15_control"
            | "partitioned_square_16x16_g64"
            | "partitioned_square_16x16_g96_direct_tokens"
            | "partitioned_square_16x16_r64"
            | "partitioned_square_16x16_g127" => "AVF-STILL-001",
            "hdr" => "AVF-COLOR-001",
            "grid" => "AVF-COMPOSE-001",
            "animated" | "animated_error_resilient" | "error_animated_repeated_frame_id" => {
                "AVF-SEQUENCE-001"
            }
            "multitile" => "AVF-TILE-001",
            unexpected => panic!("unexpected planned AVIF decode row: {unexpected}"),
        };
        assert_eq!(
            row.pure_rust_work_item.as_deref(),
            Some(expected_work_item),
            "planned AVIF row {} must name its pure-Rust deliverable",
            row.id
        );
        assert!(
            matches!(row.oracle_status.as_deref(), Some("ok" | "error")),
            "planned AVIF row {} must retain a Pillow-visible oracle status",
            row.id
        );
        assert_eq!(
            row.operations.get("decode").map(String::as_str),
            row.oracle_status.as_deref(),
            "planned AVIF row {} must retain its Pillow operation status",
            row.id
        );
        assert!(
            row.ref_path.is_none() && row.ref_bytes.is_none() && row.ref_sha256.is_none(),
            "planned AVIF row {} must not claim pixel evidence",
            row.id
        );

        let asset = require_some(
            row.asset.as_deref(),
            "planned AVIF rows must name their fixture asset",
        );
        let data = fs::read(assets_dir.join(asset))
            .unwrap_or_else(|error| panic!("planned AVIF fixture {asset} is readable: {error}"));
        let decoded = img::decode(&data);
        assert!(
            matches!(
                &decoded,
                Err(img::ImageError::Unsupported {
                    format: Some(img::ImageFormat::Avif),
                    message,
                    reason: Some(img::UnsupportedReason::NotImplemented),
                    ..
                }) if !message.trim().is_empty()
            ),
            "planned AVIF row {} must remain a typed safe-Rust Unsupported gap: {decoded:?}",
            row.id
        );

        let sequence = img::decode_sequence(&data);
        if row.rust_expect_sequence_error {
            assert_eq!(
                row.rust_sequence_error_kind.as_deref(),
                Some("malformed"),
                "the Rust sequence-error contract must name malformed input"
            );
            assert!(
                matches!(
                    &sequence,
                    Err(img::ImageError::Malformed {
                        format: img::ImageFormat::Avif,
                        message,
                        ..
                    }) if !message.trim().is_empty()
                ),
                "planned AVIF row {} must enforce its typed sequence-validation error: {sequence:?}",
                row.id
            );
        } else {
            assert!(
                matches!(
                    &sequence,
                    Err(img::ImageError::Unsupported {
                        format: Some(img::ImageFormat::Avif),
                        message,
                        reason: Some(img::UnsupportedReason::NotImplemented),
                        ..
                    }) if !message.trim().is_empty()
                ),
                "planned AVIF row {} must remain a typed safe-Rust sequence gap: {sequence:?}",
                row.id
            );
        }
    }

    let planned_encodes = avif
        .encode
        .iter()
        .filter(|row| row.status == "planned")
        .collect::<Vec<_>>();
    let former_native_encode = avif
        .encode
        .iter()
        .filter(|row| row.former_native_only)
        .collect::<Vec<_>>();
    assert_eq!(
        former_native_encode.len(),
        32,
        "the AVIF former-native encode census changed"
    );
    assert!(
        former_native_encode
            .iter()
            .all(|row| row.status == "planned"),
        "a former-native AVIF encode row cannot become active without a pure-Rust encoder"
    );
    assert_eq!(
        former_native_encode.len(),
        planned_encodes.len(),
        "every former-native AVIF encode row must remain an explicit planned gap"
    );
    assert_eq!(
        former_native_decode.len() + former_native_encode.len(),
        39,
        "the complete former-native AVIF census changed"
    );
    assert_eq!(
        planned_encodes.len(),
        32,
        "the AVIF planned encoder ledger changed"
    );
    assert_eq!(
        planned_encodes.len(),
        matrix.summary.encode_not_wired,
        "all current planned encoder rows must be AVIF rows"
    );
    let image = img::DecodedImage::new(2, 3, vec![0; 18], img::ColorType::Rgb8);
    let sequence = img::DecodedSequence::from_image(image.clone());
    let options = img::EncodeOptions::for_format(img::ImageFormat::Avif);
    for row in planned_encodes {
        assert_eq!(
            row.format, "avif",
            "planned encoder row has the wrong format"
        );
        assert!(
            row.former_native_only,
            "planned AVIF encoder row {} must explicitly identify former native-only provenance",
            row.id
        );
        assert!(
            row.gap.as_deref().is_some_and(|gap| !gap.trim().is_empty()),
            "planned AVIF encoder row {} needs a concrete safe-Rust gap reason",
            row.id
        );
        assert_eq!(
            row.pure_rust_work_item.as_deref(),
            Some("AVF-ENCODE-001"),
            "planned AVIF encoder row {} must name the pure-Rust encoder deliverable",
            row.id
        );
        assert!(
            row.ref_path.is_none()
                && row.ref_bytes.is_none()
                && row.ref_sha256.is_none()
                && row.encoded_ref_path.is_none()
                && row.encoded_ref_bytes.is_none()
                && row.encoded_ref_sha256.is_none(),
            "planned AVIF encoder row {} must not claim output evidence",
            row.id
        );
        let operation = row.operations.get("encode").map(String::as_str);
        let encoded = if operation == Some("not_applicable") {
            img::encode_sequence(&sequence, img::ImageFormat::Avif, &options)
        } else {
            img::encode(&image, img::ImageFormat::Avif, &options)
        };
        assert!(
            matches!(
                &encoded,
                Err(img::ImageError::Unsupported {
                    format: Some(img::ImageFormat::Avif),
                    message,
                    reason: Some(img::UnsupportedReason::NotImplemented),
                    ..
                }) if !message.trim().is_empty()
            ),
            "planned AVIF encoder row {} must remain a typed safe-Rust gap: {encoded:?}",
            row.id
        );
    }
}

#[allow(clippy::arithmetic_side_effects)]
fn run_decode_matrix(format_filter: Option<&str>) {
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
        if format_filter.is_some_and(|filter| filter != fmt_name) {
            continue;
        }
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
            if let Err(message) = assert_execution_contract(row.execution.as_ref())
                .and_then(|()| assert_sha256(&data, row.asset_sha256.as_deref(), "decode asset"))
                .and_then(|()| {
                    assert_origins(
                        &row.assertion_origins,
                        &["detection", "inspection", "verification", "decode"],
                    )
                })
                .and_then(|()| {
                    assert_operation_contract(
                        &row.operations,
                        &["detect", "inspect", "verify", "decode", "decode_sequence"],
                    )
                })
                .and_then(|()| {
                    assert_error_contracts(&row.operations, &row.error_contracts, fmt_name)
                })
            {
                eprintln!("  FAIL [{}]: {message}", row.id);
                failed += 1;
                continue;
            }

            let expected_format = require_some(
                format_from_name(fmt_name),
                "manifest format must be supported",
            );
            let expected_verification_scope = match row.verification_scope.as_str() {
                "header_only" => img::VerificationScope::HeaderOnly,
                "structure" => img::VerificationScope::Structure,
                other => {
                    eprintln!("  FAIL [{}]: unknown verification scope {other:?}", row.id);
                    failed += 1;
                    continue;
                }
            };
            if expected_format.verification_scope() != expected_verification_scope {
                eprintln!(
                    "  FAIL [{}]: format verification capability differs from the manifest",
                    row.id
                );
                failed += 1;
                continue;
            }
            let expected_cursor_hotspot = match row.inspect_cursor_hotspot.as_deref() {
                None => None,
                Some([x, y]) => Some(img::CursorHotspot { x: *x, y: *y }),
                Some(other) => {
                    eprintln!(
                        "  FAIL [{}]: invalid cursor hotspot evidence {other:?}",
                        row.id
                    );
                    failed += 1;
                    continue;
                }
            };
            if (row.inspect_container_format.as_deref() == Some("CUR"))
                != expected_cursor_hotspot.is_some()
            {
                eprintln!(
                    "  FAIL [{}]: CUR identity and hotspot evidence disagree",
                    row.id
                );
                failed += 1;
                continue;
            }
            let expected_bit_depth = match (
                row.inspect_status.as_str(),
                row.ref_bit_depth,
                row.ref_bit_depth_origin.as_deref(),
            ) {
                (
                    "ok",
                    Some(bit_depth),
                    Some(
                        "pillow_fixture"
                        | "specification_reference"
                        | "independent_implementation"
                        | "defensive_model",
                    ),
                ) => Some(bit_depth),
                ("error", None, None) => None,
                _ => {
                    eprintln!(
                        "  FAIL [{}]: invalid inspect bit-depth evidence {:?} from {:?}",
                        row.id, row.ref_bit_depth, row.ref_bit_depth_origin
                    );
                    failed += 1;
                    continue;
                }
            };
            let expected_inspect_byte_order = match expected_source_byte_order(
                row.inspect_source_byte_order.as_deref(),
                row.inspect_source_byte_order_origin.as_deref(),
                "inspection",
            ) {
                Ok(value) => value,
                Err(message) => {
                    eprintln!("  FAIL [{}]: {message}", row.id);
                    failed += 1;
                    continue;
                }
            };
            let inspect_requires_byte_order =
                expected_format == img::ImageFormat::Tiff && row.inspect_status == "ok";
            if expected_inspect_byte_order.is_some() != inspect_requires_byte_order {
                eprintln!(
                    "  FAIL [{}]: inspect source byte-order presence disagrees with format/status",
                    row.id
                );
                failed += 1;
                continue;
            }
            let expected_decoded_byte_order = match expected_source_byte_order(
                row.decoded_source_byte_order.as_deref(),
                row.decoded_source_byte_order_origin.as_deref(),
                "decoded image",
            ) {
                Ok(value) => value,
                Err(message) => {
                    eprintln!("  FAIL [{}]: {message}", row.id);
                    failed += 1;
                    continue;
                }
            };
            let decode_requires_byte_order = expected_format == img::ImageFormat::Tiff
                && row.oracle_status.as_deref() == Some("ok");
            if expected_decoded_byte_order.is_some() != decode_requires_byte_order {
                eprintln!(
                    "  FAIL [{}]: decoded source byte-order presence disagrees with format/status",
                    row.id
                );
                failed += 1;
                continue;
            }
            let detected = img::detect_format(&data);
            let detect_status = require_ok(
                operation_status(&row.operations, "detect"),
                "detect operation status",
            );
            let detection_matches_oracle = if row.oracle_detects_format {
                detected == Ok(expected_format)
            } else {
                detected == Err(img::ImageError::UnknownFormat)
            };
            if !detection_matches_oracle
                || !result_has_status(&detected, detect_status)
                || assert_result_error_contract(&detected, &row.error_contracts, "detect").is_err()
            {
                eprintln!(
                    "  FAIL [{}]: detection result does not match Pillow ({detected:?})",
                    row.id
                );
                failed += 1;
                continue;
            }

            let inspected = img::inspect(&data);
            let inspect_operation_status = require_ok(
                operation_status(&row.operations, "inspect"),
                "inspect operation status",
            );
            if !result_matches_oracle(
                &inspected,
                &row.inspect_status,
                row.inspect_error_kind.as_deref(),
                expected_format,
            ) || !result_has_status(&inspected, inspect_operation_status)
                || assert_result_error_contract(&inspected, &row.error_contracts, "inspect")
                    .is_err()
            {
                eprintln!(
                    "  FAIL [{}]: inspect result does not match Pillow ({:?} versus {} {:?}: {:?})",
                    row.id,
                    inspected,
                    row.inspect_status,
                    row.inspect_error_type,
                    row.inspect_error_message
                );
                failed += 1;
                continue;
            }

            let decoded = img::decode(&data);
            let decode_operation_status = require_ok(
                operation_status(&row.operations, "decode"),
                "decode operation status",
            );
            let verify_result =
                img::EncodedImage::new(Arc::<[u8]>::from(data.clone())).and_then(|source| {
                    if source.verification_scope() != expected_verification_scope {
                        return Err(img::ImageError::Parameter {
                            format: Some(expected_format),
                            message:
                                "encoded source verification capability differs from the manifest"
                                    .to_owned(),
                            stage: None,
                            offset: None,
                            identity: None,
                        });
                    }
                    let result = source.verify();
                    assert!(
                        !source.is_decoded(),
                        "verify must not populate decode cache"
                    );
                    result
                });
            let verify_matches_oracle = result_matches_oracle(
                &verify_result,
                &row.verify_status,
                row.verify_error_kind.as_deref(),
                expected_format,
            );
            let verify_operation_status = require_ok(
                operation_status(&row.operations, "verify"),
                "verify operation status",
            );
            if !verify_matches_oracle
                || !result_has_status(&verify_result, verify_operation_status)
                || assert_result_error_contract(&verify_result, &row.error_contracts, "verify")
                    .is_err()
            {
                eprintln!(
                    "  FAIL [{}]: verify result does not match Pillow ({:?} versus {} {:?}: {:?})",
                    row.id,
                    verify_result,
                    row.verify_status,
                    row.verify_error_type,
                    row.verify_error_message
                );
                failed += 1;
                continue;
            }
            if !result_has_status(&decoded, decode_operation_status)
                || assert_result_error_contract(&decoded, &row.error_contracts, "decode").is_err()
            {
                eprintln!(
                    "  FAIL [{}]: decode result violates its operation contract",
                    row.id
                );
                failed += 1;
                continue;
            }
            if row.expect_error.unwrap_or(false) {
                if row.oracle_status.as_deref() != Some("error")
                    || row.oracle_error_type.as_deref().is_none_or(str::is_empty)
                    || row.oracle_error_kind.as_deref().is_none_or(str::is_empty)
                {
                    eprintln!(
                        "  FAIL [{}]: error fixture lacks Pillow oracle type/status ({:?}: {:?})",
                        row.id, row.oracle_error_type, row.oracle_error_message
                    );
                    failed += 1;
                    continue;
                }
                let sequence_result = img::decode_sequence(&data);
                let sequence_rejected = result_matches_oracle(
                    &sequence_result,
                    "error",
                    row.oracle_error_kind.as_deref(),
                    expected_format,
                ) && operation_status(&row.operations, "decode_sequence")
                    == Ok("error")
                    && assert_result_error_contract(
                        &sequence_result,
                        &row.error_contracts,
                        "decode_sequence",
                    )
                    .is_ok();
                let structured_error = result_matches_oracle(
                    &decoded,
                    "error",
                    row.oracle_error_kind.as_deref(),
                    expected_format,
                );
                let source_error_is_stable =
                    match img::EncodedImage::new(Arc::<[u8]>::from(data.clone())) {
                        Err(error) => {
                            row.inspect_status == "error"
                                && error_matches_kind(
                                    &error,
                                    row.inspect_error_kind.as_deref(),
                                    expected_format,
                                )
                                && img::inspect(&data) == Err(error)
                        }
                        Ok(source) => {
                            let clone = source.clone();
                            let verified = source.verify();
                            let first = source.decode();
                            let second = clone.decode();
                            let verify_is_expected = result_matches_oracle(
                                &verified,
                                &row.verify_status,
                                row.verify_error_kind.as_deref(),
                                expected_format,
                            );
                            verify_is_expected
                                && result_matches_oracle(
                                    &first,
                                    "error",
                                    row.oracle_error_kind.as_deref(),
                                    expected_format,
                                )
                                && first == second
                                && !source.is_decoded()
                        }
                    };
                if structured_error && sequence_rejected && source_error_is_stable {
                    matrix_success!("  OK   [{}] rejected as Pillow does", row.id);
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
                if let Err(message) = assert_palette_parity(
                    manifest_dir,
                    row.inspect_palette.as_ref(),
                    info.mode,
                    info.palette.as_ref(),
                    "inspect",
                )
                .and_then(|()| {
                    assert_palette_parity(
                        manifest_dir,
                        row.decoded_palette.as_ref(),
                        decoded.mode,
                        decoded.palette.as_ref(),
                        "decoded",
                    )
                }) {
                    eprintln!("  FAIL [{}]: {message}", row.id);
                    failed += 1;
                    continue;
                }
                if info.format != expected_format
                    || Some(info.width)
                        != row.ref_size.as_ref().and_then(|size| size.first()).copied()
                    || Some(info.height)
                        != row.ref_size.as_ref().and_then(|size| size.get(1)).copied()
                    || Some(info.mode) != expected_mode
                    || Some(u32::from(info.bit_depth)) != expected_bit_depth
                    || (matches!(fmt_name.as_str(), "png" | "bmp" | "tiff" | "ico")
                        && info.palette != decoded.palette)
                    || (fmt_name == "gif" && info.palette.is_some() != decoded.palette.is_some())
                    || info.is_indexed() != (decoded.mode == img::ImageMode::P8)
                    || info.has_palette_table() != info.palette.is_some()
                    || info.frame_count != row.ref_frame_count
                    || info.is_animated != expected_is_animated
                    || info.cursor_hotspot != expected_cursor_hotspot
                    || decoded.cursor_hotspot != expected_cursor_hotspot
                    || info.source.byte_order() != expected_inspect_byte_order
                    || decoded.source.byte_order() != expected_decoded_byte_order
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
            if let Err(message) =
                assert_sha256(&expected.bytes, row.ref_sha256.as_deref(), "decoded pixels")
            {
                eprintln!("  FAIL [{}]: {message}", row.id);
                failed += 1;
                continue;
            }

            match assert_pixel_parity(&expected, &decoded)
                .and_then(|()| assert_sequence_parity(manifest_dir, row, &data))
            {
                Ok(()) => {
                    matrix_success!(
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
fn test_encode_matrix_jpeg() {
    run_encode_matrix(Some("jpeg"), None);
}

#[test]
fn test_encode_matrix_png() {
    run_encode_matrix(Some("png"), None);
}

#[test]
fn test_encode_matrix_gif_basic() {
    // Active-row partitions keep expensive GIF encodes independent while
    // retaining repeated source assets in one worker. The ranges below are
    // contiguous in active-row order; the final range is intentionally open
    // ended via usize::MAX so appended active rows remain covered.
    run_encode_matrix(Some("gif"), Some((0, 4)));
}

#[test]
fn test_encode_matrix_gif_palette_animation() {
    run_encode_matrix(Some("gif"), Some((4, 9)));
}

#[test]
fn test_encode_matrix_gif_rgba_animation_a() {
    run_encode_matrix(Some("gif"), Some((9, 13)));
}

#[test]
fn test_encode_matrix_gif_rgba_animation_b1() {
    run_encode_matrix(Some("gif"), Some((13, 14)));
}

#[test]
fn test_encode_matrix_gif_rgba_animation_b2() {
    run_encode_matrix(Some("gif"), Some((14, 15)));
}

#[test]
fn test_encode_matrix_gif_rgba_animation_b3() {
    run_encode_matrix(Some("gif"), Some((15, 16)));
}

#[test]
fn test_encode_matrix_gif_rgba_animation_b4() {
    run_encode_matrix(Some("gif"), Some((16, 17)));
}

#[test]
fn test_encode_matrix_gif_rgba_animation_b5() {
    run_encode_matrix(Some("gif"), Some((17, 18)));
}

#[test]
fn test_encode_matrix_gif_still_options() {
    run_encode_matrix(Some("gif"), Some((18, 31)));
}

#[test]
fn test_encode_matrix_gif_color_quantization() {
    run_encode_matrix(Some("gif"), Some((31, usize::MAX)));
}

#[test]
fn test_encode_matrix_bmp() {
    run_encode_matrix(Some("bmp"), None);
}

#[test]
fn test_encode_matrix_tiff() {
    run_encode_matrix(Some("tiff"), None);
}

#[test]
fn test_encode_matrix_webp_animation() {
    run_encode_matrix(Some("webp"), Some((0, 28)));
}

#[test]
fn test_encode_matrix_webp_common_sources() {
    run_encode_matrix(Some("webp"), Some((28, 75)));
}

#[test]
fn test_encode_matrix_webp_remaining_sources() {
    run_encode_matrix(Some("webp"), Some((75, usize::MAX)));
}

#[test]
fn test_encode_matrix_ico() {
    run_encode_matrix(Some("ico"), None);
}

#[test]
fn test_encode_matrix_avif() {
    run_encode_matrix(Some("avif"), None);
}

#[allow(clippy::arithmetic_side_effects)]
fn run_encode_matrix(format_filter: Option<&str>, active_row_range: Option<(usize, usize)>) {
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
    let mut decoded_cache: HashMap<PathBuf, Arc<img::DecodedSequence>> = HashMap::new();

    for (fmt_name, fmt_data) in &matrix.formats {
        if format_filter.is_some_and(|filter| filter != fmt_name) {
            continue;
        }
        if fmt_data.encode.is_empty() {
            continue;
        }

        let mut active_row_index = 0usize;
        for row in &fmt_data.encode {
            if row.status == "planned" {
                skipped += 1;
                continue;
            }

            let row_index = active_row_index;
            active_row_index += 1;
            if active_row_range.is_some_and(|(start, end)| row_index < start || row_index >= end) {
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
            let source_bytes = require_some(
                asset_cache.get(&asset_path),
                "encode source must be cached before provenance validation",
            );
            if let Err(message) = assert_execution_contract(row.execution.as_ref())
                .and_then(|()| {
                    assert_sha256(
                        source_bytes,
                        row.source_sha256.as_deref(),
                        "encode source asset",
                    )
                })
                .and_then(|()| assert_origins(&row.assertion_origins, &["source", "encode"]))
                .and_then(|()| {
                    assert_operation_contract(&row.operations, &["encode", "encode_sequence"])
                })
                .and_then(|()| {
                    assert_error_contracts(&row.operations, &row.error_contracts, fmt_name)
                })
            {
                eprintln!("  FAIL [{}]: {message}", row.id);
                failed += 1;
                continue;
            }

            if let Entry::Vacant(entry) = decoded_cache.entry(asset_path.clone()) {
                let asset_data = require_some(
                    asset_cache.get(&asset_path),
                    "source asset must be cached before decode",
                );
                match cached_encode_sequence(&asset_path, asset_data) {
                    Ok(decoded) => {
                        entry.insert(decoded);
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
            let expected_source_mode = require_some(
                expected_image_mode(&row.source_mode),
                "encode source_mode must name a public ImageMode",
            );
            if cached_decoded.first_image().map(|image| image.mode) != Some(expected_source_mode) {
                eprintln!(
                    "  FAIL [{}]: Rust source mode {:?} differs from Pillow source mode {}",
                    row.id,
                    cached_decoded.first_image().map(|image| image.mode),
                    row.source_mode
                );
                failed += 1;
                continue;
            }
            let mut decoded_owned = row
                .params
                .keys()
                .any(|key| {
                    matches!(
                        key.as_str(),
                        "second_frame_mode"
                            | "oversized_palette"
                            | "palette_on_nonindexed"
                            | "sequence_canvas_padding"
                            | "sequence_frame_offset"
                            | "sequence_frame_mode"
                            | "sequence_duration_ms"
                            | "sequence_duration_fraction"
                            | "sequence_disposal"
                            | "sequence_blend"
                            | "sequence_interlaced"
                            | "sequence_default_image"
                            | "sequence_pixel_layout"
                            | "sequence_loop_count"
                            | "sequence_clear_loop"
                            | "sequence_background_rgba"
                            | "sequence_background_palette"
                            | "sequence_clear_background"
                    )
                })
                .then(|| cached_decoded.as_ref().clone());
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
            if let Some(decoded) = decoded_owned.as_mut()
                && row.params.get("oversized_palette").and_then(Value::as_bool) == Some(true)
            {
                let frame = require_some(
                    decoded.frames.get_mut(0),
                    "palette operation requires a first source frame",
                );
                frame.image.palette = Some(img::ImagePalette {
                    rgb: vec![0; 771],
                    alpha: Vec::new(),
                });
            }
            if let Some(decoded) = decoded_owned.as_mut()
                && row
                    .params
                    .get("palette_on_nonindexed")
                    .and_then(Value::as_bool)
                    == Some(true)
            {
                let frame = require_some(
                    decoded.frames.get_mut(0),
                    "palette operation requires a first source frame",
                );
                frame.image.palette = Some(img::ImagePalette {
                    rgb: vec![0; 768],
                    alpha: Vec::new(),
                });
            }
            if let Some(decoded) = decoded_owned.as_mut()
                && let Some(mode_name) = row
                    .params
                    .get("sequence_frame_mode")
                    .and_then(Value::as_str)
            {
                let frame = require_some(
                    decoded.frames.first_mut(),
                    "sequence frame-mode operation requires a first frame",
                );
                let mode = require_some(
                    expected_image_mode(mode_name),
                    "sequence_frame_mode must name a public ImageMode",
                );
                frame.image = require_ok(
                    zero_image_for_mode(frame.image.width, frame.image.height, mode),
                    "sequence frame-mode image must be constructible",
                );
            }
            if let Some(decoded) = decoded_owned.as_mut() {
                let geometry_modified = row.params.contains_key("sequence_canvas_padding")
                    || row.params.contains_key("sequence_frame_offset");
                if let Some(padding) = row
                    .params
                    .get("sequence_canvas_padding")
                    .and_then(Value::as_array)
                {
                    let horizontal = require_ok(
                        u32::try_from(require_some(
                            padding.first().and_then(Value::as_u64),
                            "sequence canvas horizontal padding must be unsigned",
                        )),
                        "sequence canvas horizontal padding must fit u32",
                    );
                    let vertical = require_ok(
                        u32::try_from(require_some(
                            padding.get(1).and_then(Value::as_u64),
                            "sequence canvas vertical padding must be unsigned",
                        )),
                        "sequence canvas vertical padding must fit u32",
                    );
                    decoded.width = require_some(
                        decoded.width.checked_add(horizontal),
                        "sequence canvas width must not overflow",
                    );
                    decoded.height = require_some(
                        decoded.height.checked_add(vertical),
                        "sequence canvas height must not overflow",
                    );
                }
                if geometry_modified {
                    for retained_frame in &mut decoded.frames {
                        retained_frame.pixel_layout = img::FramePixelLayout::SourceRectangle;
                        retained_frame.source.rect.width = retained_frame.image.width;
                        retained_frame.source.rect.height = retained_frame.image.height;
                    }
                }
                let frame = require_some(
                    decoded.frames.first_mut(),
                    "sequence transform requires a first frame",
                );
                if let Some(offset) = row
                    .params
                    .get("sequence_frame_offset")
                    .and_then(Value::as_array)
                {
                    frame.source.rect.left = require_ok(
                        u32::try_from(require_some(
                            offset.first().and_then(Value::as_u64),
                            "sequence frame left offset must be unsigned",
                        )),
                        "sequence frame left offset must fit u32",
                    );
                    frame.source.rect.top = require_ok(
                        u32::try_from(require_some(
                            offset.get(1).and_then(Value::as_u64),
                            "sequence frame top offset must be unsigned",
                        )),
                        "sequence frame top offset must fit u32",
                    );
                }
                if let Some(pixel_layout) = row
                    .params
                    .get("sequence_pixel_layout")
                    .and_then(Value::as_str)
                {
                    frame.pixel_layout = match pixel_layout {
                        "source_rectangle" => img::FramePixelLayout::SourceRectangle,
                        "rendered_canvas" => img::FramePixelLayout::RenderedCanvas,
                        _ => panic!("unknown manifest sequence pixel layout"),
                    };
                }
                if let Some(duration) = row
                    .params
                    .get("sequence_duration_ms")
                    .and_then(Value::as_u64)
                {
                    frame.source.duration = img::FrameDuration::from_milliseconds(require_ok(
                        u32::try_from(duration),
                        "sequence duration must fit u32",
                    ));
                }
                if let Some(duration) = row
                    .params
                    .get("sequence_duration_fraction")
                    .and_then(Value::as_array)
                {
                    frame.source.duration = img::FrameDuration {
                        numerator: require_some(
                            duration.first().and_then(Value::as_u64),
                            "sequence duration numerator must be unsigned",
                        ),
                        denominator: require_some(
                            duration.get(1).and_then(Value::as_u64),
                            "sequence duration denominator must be unsigned",
                        ),
                    };
                }
                if let Some(disposal) = row.params.get("sequence_disposal").and_then(Value::as_str)
                {
                    frame.source.disposal = match disposal {
                        "unspecified" => img::FrameDisposal::Unspecified,
                        "keep" => img::FrameDisposal::Keep,
                        "background" => img::FrameDisposal::Background,
                        "previous" => img::FrameDisposal::Previous,
                        value if value.starts_with("reserved:") => {
                            img::FrameDisposal::Reserved(require_ok(
                                value["reserved:".len()..].parse::<u8>(),
                                "reserved sequence disposal must contain a u8",
                            ))
                        }
                        _ => panic!("unknown manifest sequence disposal"),
                    };
                }
                if let Some(blend) = row.params.get("sequence_blend").and_then(Value::as_str) {
                    frame.source.blend = match blend {
                        "unspecified" => img::FrameBlend::Unspecified,
                        "source" => img::FrameBlend::Source,
                        "over" => img::FrameBlend::Over,
                        value if value.starts_with("reserved:") => {
                            img::FrameBlend::Reserved(require_ok(
                                value["reserved:".len()..].parse::<u8>(),
                                "reserved sequence blend must contain a u8",
                            ))
                        }
                        _ => panic!("unknown manifest sequence blend"),
                    };
                }
                if let Some(interlaced) = row
                    .params
                    .get("sequence_interlaced")
                    .and_then(Value::as_bool)
                {
                    frame.source.interlaced = interlaced;
                }
                if let Some(is_default_image) = row
                    .params
                    .get("sequence_default_image")
                    .and_then(Value::as_bool)
                {
                    frame.source.is_default_image = is_default_image;
                }
                if let Some(loop_count) = row
                    .params
                    .get("sequence_loop_count")
                    .and_then(Value::as_u64)
                {
                    decoded.loop_count = Some(require_ok(
                        u32::try_from(loop_count),
                        "sequence loop count must fit u32",
                    ));
                }
                if row
                    .params
                    .get("sequence_clear_loop")
                    .and_then(Value::as_bool)
                    == Some(true)
                {
                    decoded.loop_count = None;
                }
                if let Some(background) = row
                    .params
                    .get("sequence_background_rgba")
                    .and_then(Value::as_array)
                {
                    let channels = require_ok(
                        background
                            .iter()
                            .map(|value| {
                                u8::try_from(require_some(
                                    value.as_u64(),
                                    "sequence background channels must be unsigned",
                                ))
                            })
                            .collect::<Result<Vec<_>, _>>(),
                        "sequence background channels must fit u8",
                    );
                    let rgba = require_ok(
                        <[u8; 4]>::try_from(channels),
                        "sequence background must contain four u8 channels",
                    );
                    decoded.background = Some(img::AnimationBackground::Rgba(rgba));
                }
                if let Some(index) = row
                    .params
                    .get("sequence_background_palette")
                    .and_then(Value::as_u64)
                {
                    decoded.background = Some(img::AnimationBackground::PaletteIndex(require_ok(
                        u8::try_from(index),
                        "sequence palette background must fit u8",
                    )));
                }
                if row
                    .params
                    .get("sequence_clear_background")
                    .and_then(Value::as_bool)
                    == Some(true)
                {
                    decoded.background = None;
                }
            }
            let decoded = decoded_owned.as_ref().unwrap_or(cached_decoded.as_ref());

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
            let legacy_pairs = legacy_encode_options(fmt_name, &row.params);
            let mut opts = img::EncodeOptions::try_from_legacy_pairs(format, &legacy_pairs);
            if let Ok(img::EncodeOptions::Avif(options)) = &mut opts {
                options.advanced = advanced_encode_options(&row.params);
            }

            if let Some(mode_values) = row
                .params
                .get("rust_unsupported_modes")
                .and_then(Value::as_array)
            {
                let base = require_some(
                    decoded.first_image(),
                    "public-mode contract requires a decoded fixture frame",
                );
                let mut contract_failures = Vec::new();
                let fallback_options = img::EncodeOptions::for_format(format);
                let options = match &opts {
                    Ok(options) => options,
                    Err(error) => {
                        contract_failures
                            .push(format!("typed option construction failed: {error}"));
                        &fallback_options
                    }
                };
                for value in mode_values {
                    let name = require_some(
                        value.as_str(),
                        "public-mode contract entries must be strings",
                    );
                    let mode = require_some(
                        expected_image_mode(name),
                        "public-mode contract entry must name an ImageMode",
                    );
                    let image = require_ok(
                        zero_image_for_mode(base.width, base.height, mode),
                        "public-mode contract image must be constructible",
                    );
                    let result = img::encode(&image, format, options);
                    if !matches!(
                        result,
                        Err(img::ImageError::Unsupported {
                            format: Some(actual),
                            ..
                        }) if actual == format
                    ) {
                        contract_failures.push(format!("{name}: {result:?}"));
                    }
                }
                if row
                    .params
                    .get("rust_invalid_color_mode")
                    .and_then(Value::as_bool)
                    == Some(true)
                {
                    let mut invalid = require_ok(
                        zero_image_for_mode(base.width, base.height, img::ImageMode::L8),
                        "invalid-state contract image must be constructible",
                    );
                    invalid.color = img::ColorType::Rgb8;
                    let result = img::encode(&invalid, format, options);
                    if !matches!(result, Err(img::ImageError::Parameter { .. })) {
                        contract_failures.push(format!("inconsistent color/mode: {result:?}"));
                    }
                }
                let fixture_has_oracle_success = row.rust_expect_error
                    && row.rust_error_kind.as_deref() == Some("unsupported")
                    && row.oracle_status.as_deref() == Some("ok")
                    && row
                        .encoded_ref_path
                        .as_deref()
                        .is_some_and(|path| manifest_dir.join(path).is_file());
                let operation_contract_matches = operation_status(&row.operations, "encode")
                    == Ok("error")
                    && operation_status(&row.operations, "encode_sequence") == Ok("not_applicable");
                if contract_failures.is_empty()
                    && fixture_has_oracle_success
                    && operation_contract_matches
                {
                    matrix_success!(
                        "  OK   [{}] all public unsupported modes and invalid state returned structured errors",
                        row.id
                    );
                    passed += 1;
                } else {
                    eprintln!(
                        "  FAIL [{}]: public mode contract diverged: {}; fixture evidence={fixture_has_oracle_success}",
                        row.id,
                        contract_failures.join("; ")
                    );
                    failed += 1;
                }
                continue;
            }

            let direct_still = row
                .params
                .get("truncate_pixels")
                .is_some_and(|v| v.as_bool().unwrap_or(false))
                || row.params.contains_key("source_dimensions");
            let encoded = match opts.as_ref() {
                Err(error) => Err((*error).clone()),
                Ok(options)
                    if row
                        .params
                        .get("truncate_pixels")
                        .is_some_and(|v| v.as_bool().unwrap_or(false)) =>
                {
                    let mut malformed = require_some(
                        decoded.first_image(),
                        "encoded sequence must have a first frame",
                    )
                    .clone();
                    malformed.pixels.pop();
                    img::encode(&malformed, format, options)
                }
                Ok(options) => {
                    if let Some(dimensions) = row.params.get("source_dimensions") {
                        let dimensions = require_some(
                            dimensions.as_array(),
                            "source_dimensions must be a JSON array",
                        );
                        let mut malformed = require_some(
                            decoded.first_image(),
                            "encoded sequence must have a first frame",
                        )
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
                        img::encode(&malformed, format, options)
                    } else {
                        img::encode_sequence(decoded, format, options)
                    }
                }
            };
            let operation = if direct_still {
                "encode"
            } else {
                "encode_sequence"
            };
            let expected_operation_status = require_ok(
                operation_status(&row.operations, operation),
                "encode operation status",
            );
            if !result_has_status(&encoded, expected_operation_status) {
                eprintln!(
                    "  FAIL [{}]: {operation} result violates operation contract {expected_operation_status}",
                    row.id
                );
                failed += 1;
                continue;
            }
            if let Err(message) =
                assert_result_error_contract(&encoded, &row.error_contracts, operation)
            {
                eprintln!("  FAIL [{}]: {message}", row.id);
                failed += 1;
                continue;
            }
            if row.rust_expect_error {
                let fixture_has_oracle_success = row.oracle_status.as_deref() == Some("ok")
                    && row
                        .encoded_ref_path
                        .as_deref()
                        .is_some_and(|value| !value.is_empty())
                    && row
                        .ref_path
                        .as_deref()
                        .is_some_and(|value| !value.is_empty());
                if fixture_has_oracle_success
                    && result_matches_oracle(
                        &encoded,
                        "error",
                        row.rust_error_kind.as_deref(),
                        format,
                    )
                {
                    matrix_success!(
                        "  OK   [{}] rejected retained sequence semantics: {}",
                        row.id,
                        row.rust_error_reason.as_deref().unwrap_or("unspecified")
                    );
                    passed += 1;
                } else {
                    eprintln!(
                        "  FAIL [{}]: Rust contract error mismatch (encoded_ok={}, oracle_status={:?}, expected={:?})",
                        row.id,
                        encoded.is_ok(),
                        row.oracle_status,
                        row.rust_error_kind
                    );
                    failed += 1;
                }
                continue;
            }
            if row.expect_error {
                let fixture_has_oracle_error = row.oracle_status.as_deref() == Some("error")
                    && row
                        .oracle_error_type
                        .as_deref()
                        .is_some_and(|value| !value.is_empty())
                    && row
                        .oracle_error_kind
                        .as_deref()
                        .is_some_and(|value| !value.is_empty());
                if fixture_has_oracle_error
                    && result_matches_oracle(
                        &encoded,
                        "error",
                        row.oracle_error_kind.as_deref(),
                        format,
                    )
                {
                    matrix_success!("  OK   [{}] rejected as Pillow does", row.id);
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
                if operation_status(&row.operations, "encode") != Ok("ok") {
                    eprintln!(
                        "  FAIL [{}]: one-frame success lacks still encode capability",
                        row.id
                    );
                    failed += 1;
                    continue;
                }
                match img::encode(
                    require_some(
                        decoded.first_image(),
                        "encoded sequence must have a first frame",
                    ),
                    format,
                    require_ok(opts.as_ref(), "typed encode options"),
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
            } else if operation_status(&row.operations, "encode") != Ok("not_applicable") {
                eprintln!(
                    "  FAIL [{}]: multi-frame row must mark still encode not applicable",
                    row.id
                );
                failed += 1;
                continue;
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
            if let Err(message) = assert_sha256(
                &expected_encoded,
                row.encoded_ref_sha256.as_deref(),
                "encoded-byte reference",
            ) {
                eprintln!("  FAIL [{}]: {message}", row.id);
                failed += 1;
                continue;
            }
            if let Err(message) = assert_encoded_byte_parity(&expected_encoded, &encoded) {
                eprintln!("  FAIL [{}]: {message}", row.id);
                failed += 1;
                continue;
            }
            if let Some(expected_sequence) = &row.sequence {
                match img::decode_sequence(&encoded) {
                    Ok(decoded) => {
                        let parity = if decoded.format == format {
                            assert_sequence_reference_parity(
                                manifest_dir,
                                &row.id,
                                expected_sequence,
                                &decoded.content,
                            )
                        } else {
                            Err(format!(
                                "sequence format {:?} differs from {format:?}",
                                decoded.format
                            ))
                        };
                        if let Err(detail) = parity {
                            eprintln!("  FAIL [{}]: encoded sequence parity: {detail}", row.id);
                            failed += 1;
                            continue;
                        }
                    }
                    Err(error) => {
                        eprintln!(
                            "  FAIL [{}]: encoded sequence decode failed: {error}",
                            row.id
                        );
                        failed += 1;
                        continue;
                    }
                }
            }
            if row.params.get("encoded_only").and_then(Value::as_bool) == Some(true) {
                matrix_success!(
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
                        if let Err(message) = assert_sha256(
                            &expected.bytes,
                            row.ref_sha256.as_deref(),
                            "encode roundtrip pixels",
                        ) {
                            eprintln!("  FAIL [{}]: {message}", row.id);
                            failed += 1;
                            continue;
                        }
                        match assert_pixel_parity(&expected, &redecoded) {
                            Ok(()) => {
                                matrix_success!(
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
fn avif_reconstruction_fixture_is_planned(fixture: &str) -> bool {
    coverage_matrix()
        .and_then(|matrix| matrix.formats.get("avif"))
        .is_some_and(|format| {
            format
                .decode
                .iter()
                .any(|row| row.status == "planned" && row.asset.as_deref() == Some(fixture))
        })
}

#[cfg(coverage)]
#[test]
fn test_partitioned_square_444_fixtures_materialize() {
    let fixtures = [
        "partitioned_square_12x12_g96_direct_tokens.avif",
        "partitioned_square_12x12_midpoint_g96_ac.avif",
        "partitioned_square_12x12_top_left_luma_eob4.avif",
        "partitioned_square_12x12_top_left_luma_eob12_control.avif",
        "partitioned_square_12x12_luma_eob1.avif",
        "partitioned_square_12x12_luma_eob2_control.avif",
        "partitioned_square_12x12_luma_eob4_control.avif",
        "partitioned_square_12x12_luma_eob6_control.avif",
        "partitioned_square_12x12_luma_eob9_control.avif",
        "partitioned_square_12x12_luma_eob10_control.avif",
        "partitioned_square_12x12_luma_eob12_control.avif",
        "partitioned_square_12x12_luma_eob15_control.avif",
    ];
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/input/images/avif");
    for fixture in fixtures {
        let input = require_ok(fs::read(root.join(fixture)), "partitioned-square fixture");
        let actual = require_ok(
            img::__coverage_av1_reconstruction(&input),
            "partitioned-square reconstruction",
        );
        assert!(
            actual.is_some(),
            "partitioned 12x12 4:4:4 fixture must materialize: {fixture}"
        );
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
    assert_eq!(expected.cases.len(), 181);
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
        let actual = require_ok(
            img::__coverage_av1_reconstruction(&input),
            "production AV1 reconstruction validation must succeed",
        );
        let Some(actual) = actual else {
            assert!(
                avif_reconstruction_fixture_is_planned(&case.fixture),
                "unexpected AV1 reconstruction gap for {}",
                case.fixture
            );
            continue;
        };
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
            "portable_lossless_420_split_12x4_a.avif"
            | "portable_lossless_420_split_16x4_a.avif"
            | "portable_lossless_420_split_12x8_a.avif"
            | "portable_lossless_420_split_16x8_a.avif" => Some([37_392, 43_662, 48_034]),
            "portable_lossless_420_split_4x12_a.avif"
            | "portable_lossless_420_split_4x16_a.avif"
            | "portable_lossless_420_split_8x12_a.avif"
            | "portable_lossless_420_split_8x16_a.avif" => Some([46_608, 54_426, 53_236]),
            "coverage_adst_public_03.avif" => Some([46_608, 54_426, 50_176]),
            "coverage_adst_public_04.avif" => Some([37_392, 43_662, 35_645]),
            "coverage_adst_public_05.avif" => Some([46_608, 54_426, 46_810]),
            "coverage_adst_public_06.avif" => Some([37_392, 43_662, 61_804]),
            _ => None,
        };
        let deep_recursive_ranges = match case.fixture.as_str() {
            "coverage_r8x16_band_05.avif" => Some(vec![40_720, 57_892, 33_811, 60_156]),
            "coverage_r8x16_band_06.avif" => Some(vec![40_720, 57_892, 33_811, 60_156]),
            "coverage_adst_public_07.avif" => Some(vec![40_720, 57_892, 33_811, 44_974]),
            "coverage_adst_public_08.avif" => Some(vec![38_416, 43_816, 51_186, 53_848]),
            "coverage_adst_public_10.avif" => {
                Some(vec![38_416, 40_864, 47_838, 37_634, 57_584, 33_970])
            }
            _ => None,
        };
        let square_recursive_ranges = match case.fixture.as_str() {
            "partitioned_square_12x12_top_left_luma_eob4.avif" => {
                Some([34_880, 40_768, 47_278, 60_530, 38_697])
            }
            "partitioned_square_12x12_top_left_luma_eob12_control.avif" => {
                Some([34_880, 40_768, 53_060, 33_964, 42_488])
            }
            "partitioned_square_12x12_midpoint_g96_ac.avif" => {
                Some([34_880, 40_768, 39_772, 48_314, 52_050])
            }
            "partitioned_square_12x12_g96_direct_tokens.avif"
            | "partitioned_square_12x12_luma_eob1.avif"
            | "partitioned_square_12x12_luma_eob2_control.avif"
            | "partitioned_square_12x12_luma_eob4_control.avif"
            | "partitioned_square_12x12_luma_eob6_control.avif"
            | "partitioned_square_12x12_luma_eob9_control.avif"
            | "partitioned_square_12x12_luma_eob10_control.avif"
            | "partitioned_square_12x12_luma_eob12_control.avif"
            | "partitioned_square_12x12_luma_eob15_control.avif"
            | "partitioned_square_16x16_g64.avif"
            | "partitioned_square_16x16_g96_direct_tokens.avif"
            | "partitioned_square_16x16_r64.avif"
            | "partitioned_square_16x16_g127.avif" => {
                Some([34_880, 40_768, 50_626, 52_336, 54_330])
            }
            "partitioned_square_420_16x16_rgb_delta.avif"
            | "partitioned_square_420_16x16_g96.avif" => {
                Some([34_880, 40_768, 43_750, 52_892, 59_618])
            }
            "coverage_i444_palette2_square8_four_leaves.avif" => {
                Some([34_880, 40_768, 45_302, 45_386, 40_125])
            }
            "coverage_adst_public_09.avif" => Some([34_880, 40_768, 33_809, 44_126, 55_818]),
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
        } else if let Some(ranges) = deep_recursive_ranges {
            let horizontal = case.portable_color.width > case.portable_color.height;
            let expected_blocks = if ranges.len() == 4 {
                vec![
                    Av1PartitionBlock {
                        poc: 0,
                        x: 0,
                        y: 0,
                        level: 2,
                        context: 0,
                        partition: 3,
                        range: ranges[0],
                    },
                    Av1PartitionBlock {
                        poc: 0,
                        x: 0,
                        y: 0,
                        level: 3,
                        context: 0,
                        partition: 3,
                        range: ranges[1],
                    },
                    Av1PartitionBlock {
                        poc: 0,
                        x: 0,
                        y: 0,
                        level: 4,
                        context: 0,
                        partition: 0,
                        range: ranges[2],
                    },
                    Av1PartitionBlock {
                        poc: 0,
                        x: if horizontal { 2 } else { 0 },
                        y: if horizontal { 0 } else { 2 },
                        level: 4,
                        context: 0,
                        partition: 0,
                        range: ranges[3],
                    },
                ]
            } else {
                vec![
                    Av1PartitionBlock {
                        poc: 0,
                        x: 0,
                        y: 0,
                        level: 2,
                        context: 0,
                        partition: 3,
                        range: ranges[0],
                    },
                    Av1PartitionBlock {
                        poc: 0,
                        x: 0,
                        y: 0,
                        level: 3,
                        context: 0,
                        partition: 3,
                        range: ranges[1],
                    },
                    Av1PartitionBlock {
                        poc: 0,
                        x: 0,
                        y: 0,
                        level: 4,
                        context: 0,
                        partition: 0,
                        range: ranges[2],
                    },
                    Av1PartitionBlock {
                        poc: 0,
                        x: 2,
                        y: 0,
                        level: 4,
                        context: 0,
                        partition: 0,
                        range: ranges[3],
                    },
                    Av1PartitionBlock {
                        poc: 0,
                        x: 0,
                        y: 2,
                        level: 4,
                        context: 0,
                        partition: 0,
                        range: ranges[4],
                    },
                    Av1PartitionBlock {
                        poc: 0,
                        x: 2,
                        y: 2,
                        level: 4,
                        context: 0,
                        partition: 0,
                        range: ranges[5],
                    },
                ]
            };
            assert_eq!(
                case.partition_blocks, expected_blocks,
                "AV1 deep recursive partition topology case {case_index}"
            );
        } else if let Some(ranges) = square_recursive_ranges {
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
                    x: 2,
                    y: 0,
                    level: 4,
                    context: 0,
                    partition: 0,
                    range: ranges[2],
                },
                Av1PartitionBlock {
                    poc: 0,
                    x: 0,
                    y: 2,
                    level: 4,
                    context: 0,
                    partition: 0,
                    range: ranges[3],
                },
                Av1PartitionBlock {
                    poc: 0,
                    x: 2,
                    y: 2,
                    level: 4,
                    context: 0,
                    partition: 0,
                    range: ranges[4],
                },
            ];
            assert_eq!(
                case.partition_blocks, expected_blocks,
                "AV1 square partition topology case {case_index}"
            );
        } else {
            assert_eq!(
                case.partition_blocks.len(),
                1,
                "AV1 single-leaf partition topology case {case_index}"
            );
        }
        if case.fixture == "coverage_r16x64_grid_01.avif" {
            assert_eq!(
                case.partition_blocks,
                vec![Av1PartitionBlock {
                    poc: 0,
                    x: 0,
                    y: 0,
                    level: 1,
                    context: 0,
                    partition: 2,
                    range: 46_200,
                }],
                "AV1 Vertical16x64 witness partition topology"
            );
            let debug_lines = case
                .decoder_events
                .iter()
                .filter_map(|event| event.as_object()?.get("line")?.as_str());
            let debug_lines = debug_lines.collect::<Vec<_>>();
            assert!(
                debug_lines
                    .iter()
                    .any(|line| line.starts_with("Post-tx[2]:")),
                "AV1 Vertical16x64 witness must select TX16x16 depth two"
            );
            assert_eq!(
                debug_lines
                    .iter()
                    .filter(|line| line.starts_with("Post-y-cf-blk[tx=2,"))
                    .count(),
                1,
                "AV1 Vertical16x64 witness must decode its luma transform sentence"
            );
            assert_eq!(
                debug_lines
                    .iter()
                    .filter(|line| line.starts_with("Post-uv-cf-blk["))
                    .count(),
                2,
                "AV1 Vertical16x64 witness must decode both chroma planes"
            );
        }
        assert_eq!(case.decoded_planes.len(), 3);
        for (plane_index, (actual, expected)) in
            actual.planes.iter().zip(&case.decoded_planes).enumerate()
        {
            assert_eq!(expected.name, ["y", "u", "v"][plane_index]);
            let expected_dimensions = if plane_index == 0 {
                (case.portable_color.width, case.portable_color.height)
            } else {
                (
                    if case.portable_color.subsampling_x {
                        case.portable_color.width.div_ceil(2)
                    } else {
                        case.portable_color.width
                    },
                    if case.portable_color.subsampling_y {
                        case.portable_color.height.div_ceil(2)
                    } else {
                        case.portable_color.height
                    },
                )
            };
            assert_eq!((expected.width, expected.height), expected_dimensions);
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
        if case.fixture == "coverage_adst_public_04.avif" {
            assert_eq!(
                case.entropy_operations.len(),
                407,
                "AV1 _04 pinned entropy operation count"
            );
        }
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
            "portable_lossless_420_a.avif" => {
                "0fdfb2ec7d6741b65177c1343d0e510798f3177b75018fdbc8da541ea2d32a0b"
            }
            "portable_lossless_420_b.avif" => {
                "34a99c606d95db58868b24c3ce3ade1c502adcf213130c403486cbd50bc4fad5"
            }
            "portable_lossless_420_8x8_a.avif" => {
                "1f403e7f414473b888fcba438d60d269e54fc1d04c802dd32f96fa657932b2ac"
            }
            "portable_lossless_420_8x8_b.avif" => {
                "1217b329eae17189460716ba186b4d01617aa8648cd5c03aee2e8905cc20e008"
            }
            "portable_lossy_420_q99_gray_0.avif" => {
                "17b0761f87b081d5cf10757ccc89f12be355c70e2e29df288b65b30710dcbcd1"
            }
            "portable_lossy_420_q99_8x8_gray_0.avif" => {
                "5d89f056865052bcb89c910d2d62872e029fb273c3db03f8968a52a41593c1b5"
            }
            "portable_lossy_420_q99_gray_64.avif" => {
                "30c8d471cc44e88da2fec08638a4215ed2ce34c899f330115a604b80d19f2831"
            }
            "portable_lossy_420_q99_8x8_gray_64.avif" => {
                "557f22c418e6f4fcd4d4c1df7eb2b46180b67956794483587205e2e82163b395"
            }
            "portable_lossy_420_q99_gray_122_control.avif" => {
                "ad287d41398b2bc6aae343d24767bded9795b882f382b5abf480a6fc0bbddfdf"
            }
            "portable_lossy_420_q99_8x8_gray_122_control.avif" => {
                "9e96fe6320d50c09026df65c9676a19e57fe86b26652cf513c2cc03015711df0"
            }
            "portable_lossy_420_q99_gray_123_control.avif" => {
                "819d474948483b42b8e875e2bb3446526e0a5f1f090d012b993d6a12fcf0e4da"
            }
            "portable_lossy_420_q99_8x8_gray_123_control.avif" => {
                "d9bced69730dcb4567fcd0eac9073a83993278a18aebf3c03544b49d5660576d"
            }
            "portable_lossy_420_q99_gray_124_control.avif" => {
                "5acbd8048d53d1aa8fcbaacb57506e7eb6a1f570d93c899bd97f899f535f7ce9"
            }
            "portable_lossy_420_q99_8x8_gray_124_control.avif" => {
                "25c65b22ccf260aff6e521fbce082a40fd940968592a3c2e5272768c362481da"
            }
            "portable_lossy_420_q99_gray_125_control.avif" => {
                "e82feb502523b0e30e96c557012bbc79208f186e3fcb858916b2972db760aac1"
            }
            "portable_lossy_420_q99_8x8_gray_125_control.avif" => {
                "4d11382e9da0a7e9facadaf22c7d9036b341797376ecc5a77c2779e1884e1ec5"
            }
            "portable_lossy_420_q99_gray_126.avif" => {
                "0bc6b6903ab77a6d1706777bb507e076f01290f57cb975508aec1cd5cf589810"
            }
            "portable_lossy_420_q99_8x8_gray_126.avif" => {
                "9a5f0b79fce197304a6aa5a89af73862b128be0db6e93117a67d3ddd07e28edd"
            }
            "portable_lossy_420_q99_gray_127.avif" => {
                "a1fa26e9a041c510e9f8412accef2e5e0cda5eddd97fa6db80b30400b7964d42"
            }
            "portable_lossy_420_q99_8x8_gray_127.avif" => {
                "c24e73f000a4255a612416ecc4df81c9313e4c099877384712e4d8530dd7acbd"
            }
            "portable_lossy_420_q99_gray_129.avif" => {
                "b34e1e1e7cd63c9fb7069154ccd855d827a3dd3eca076232b4217745a2b6db57"
            }
            "portable_lossy_420_q99_8x8_gray_129.avif" => {
                "fca06fef259b9ebb452449c7feda724ccec06a4a76b2b4fb1e6420a0beac435e"
            }
            "portable_lossy_420_q99_gray_130.avif" => {
                "2c28ec0de076c8c2e7d6d8222ada07a0da8ec45ea53160a39b5dd64b79d7bcc8"
            }
            "portable_lossy_420_q99_8x8_gray_130.avif" => {
                "4371170b5239419060ed559afe13157740d69ef2aee0592cf4fc71c47dff58a5"
            }
            "portable_lossy_420_q99_gray_131_control.avif" => {
                "d8044c92ef2a961ebee78d49908caae12338872a8cb36675ef6dbfb0f244e2e9"
            }
            "portable_lossy_420_q99_8x8_gray_131_control.avif" => {
                "831ca0567d6d09bf16b7c76da27026347d9000d12ca92f486dd9c56b4226055e"
            }
            "portable_lossy_420_q99_gray_132_control.avif" => {
                "03a471cd2fdf8af4107b02673eec688e6c7bc946d184af0c514af6a206e51681"
            }
            "portable_lossy_420_q99_8x8_gray_132_control.avif" => {
                "603bfc293271617bfea86708fcd2820aa7246d3d73a47fd2c0184de328c68ab3"
            }
            "portable_lossy_420_q99_gray_133_control.avif" => {
                "7f0d7099d13d4903645f8fc327e2f0fe46fd9655a948fbc375024f82acc50fc2"
            }
            "portable_lossy_420_q99_8x8_gray_133_control.avif" => {
                "108f70bd32bd6aa8f4d1a6ee9450a6505f16158b350b293f7e37ca87724ae29a"
            }
            "portable_lossy_420_q99_gray_134_control.avif" => {
                "96a0187338028cdac12765e42d24b4cf369039db628878c674d273bdb0af4324"
            }
            "portable_lossy_420_q99_8x8_gray_134_control.avif" => {
                "d4ea4542b1b805cc3f636afb8bf16a483cc0fe47a40b4fba4c876ebb26432b2a"
            }
            "portable_lossy_420_q99_gray_192.avif" => {
                "af14d74c13f430d78f29de7246b5cbdf0937adbeb872ffe6dcf68282860d7cba"
            }
            "portable_lossy_420_q99_8x8_gray_192.avif" => {
                "6845b27f00c23448c01b082d69fdf01aae50f11e3f0b29b073dfe5e6b864c36b"
            }
            "portable_lossy_420_q99_gray_255.avif" => {
                "80a76a18acf8cb64fec3a659ffc4bab4a87cd9a6fde4dab2161a8751d136c9d2"
            }
            "portable_lossy_420_q99_8x8_gray_255.avif" => {
                "8f62c344eff1568474fb693b8c18526629db443b9653a84264189c97693605de"
            }
            "portable_lossy_420_q99_token_1048_control.avif"
            | "portable_lossy_420_q99_token_7764.avif" => {
                "17b0761f87b081d5cf10757ccc89f12be355c70e2e29df288b65b30710dcbcd1"
            }
            "portable_lossy_420_q99_token_2061.avif"
            | "portable_lossy_420_q99_token_2988.avif"
            | "portable_lossy_420_q99_token_7940.avif" => {
                "80a76a18acf8cb64fec3a659ffc4bab4a87cd9a6fde4dab2161a8751d136c9d2"
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
            "partitioned_square_16x16_g64.avif" => {
                "d7efc58f710522b0c6e2609ab53339cf9aa4c3c419b4023593bffd94fcb883fe"
            }
            "partitioned_square_12x12_g96_direct_tokens.avif" => {
                "8fd169458756409edfaf3380195c6ab881e3d7043d5c3b158a82feaaa82b993f"
            }
            "partitioned_square_12x12_top_left_luma_eob4.avif" => {
                "fcfe3605207a28cd1596ae0cb2b9b4ad1b8b356f7457cd2e60276b8d6530a691"
            }
            "partitioned_square_12x12_top_left_luma_eob12_control.avif" => {
                "16195f9646d15f2857da1864cbffdd3f12a965bbd287ca888b7dde113c2d7ec7"
            }
            "partitioned_square_12x12_midpoint_g96_ac.avif" => {
                "1d316f3236ecba0ebb2e4483622a7dbaa736686fc6ce609a44c3e7c7380a0ff4"
            }
            "partitioned_square_12x12_luma_eob1.avif" => {
                "d8ddfb34c1d4da25851a33b0515d025bd092a6bfd942eeda21683b9e564d6691"
            }
            "partitioned_square_12x12_luma_eob2_control.avif" => {
                "13878ffdf1168508a15759ff58c897370e8428fe522422d52149126a9cc42ef4"
            }
            "partitioned_square_12x12_luma_eob4_control.avif" => {
                "299dc7d8cf7b620bb3cc3a56ab17da5414d8377e0b79196fce64cae0e05ca7f3"
            }
            "partitioned_square_12x12_luma_eob6_control.avif" => {
                "84c006c2c0f8e322453101374baeb3c0f1e30653b7960fb1068cfc8f33c96e68"
            }
            "partitioned_square_12x12_luma_eob9_control.avif" => {
                "7b69d30ebe2894d11aa6d4f7c3385c8675a4cf8daf702d5b6cd709a6001ce506"
            }
            "partitioned_square_12x12_luma_eob10_control.avif" => {
                "edb3552022d80b01938371e9e0d78ea4544d2b1bab41cfe67253a89458774264"
            }
            "partitioned_square_12x12_luma_eob12_control.avif" => {
                "a98fa8dc8ff3ed903815016c02089c888bee48bfb8774903c8bf70d57aed2735"
            }
            "partitioned_square_12x12_luma_eob15_control.avif" => {
                "2d41c17b74e78417fd7ab3fdb5da3225f52c4035e39133275ee01496cc21a77a"
            }
            "partitioned_square_16x16_g96_direct_tokens.avif" => {
                "87cf9f38f5bc4a0a75c3284ff3b5826e0c0734066e863bcf416f2296623b890f"
            }
            "partitioned_square_16x16_r64.avif" => {
                "6492bb904bafc0a5c8acedff1fd7cd70965e3be844e8fd19d0e04a6bd63e2017"
            }
            "partitioned_square_16x16_g127.avif" => {
                "d1ce3617b6228d74d2b208847c20486f1a6301cf8b0708242c0019894eeb055e"
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
            "portable_lossless_420_leaf_4x8_a.avif" | "portable_lossless_420_leaf_8x4_a.avif" => {
                "116d1d3509d9d2a7558a2fad832f923fc1193f04b8e0e57946f49e57fa045475"
            }
            "portable_lossless_420_rect_12x4_gray_127.avif"
            | "portable_lossless_420_rect_4x12_gray_127.avif" => {
                "35fc07c937c1c3d13641f32cdc94ce1315ec420dd26e12b81a4651cfc1786ee3"
            }
            "portable_lossless_420_rect_16x4_gray_127.avif"
            | "portable_lossless_420_rect_4x16_gray_127.avif" => {
                "c24e73f000a4255a612416ecc4df81c9313e4c099877384712e4d8530dd7acbd"
            }
            "portable_lossless_420_rect_12x8_gray_127.avif"
            | "portable_lossless_420_rect_8x12_gray_127.avif" => {
                "cf8691a9b8c6c8e329b94f40345d822ef7d4f6e8e5c2343d74b12aa16e84838a"
            }
            "portable_lossless_420_rect_16x8_gray_127.avif"
            | "portable_lossless_420_rect_8x16_gray_127.avif" => {
                "7e18f1b2ca4e075b955848b4deafd56e47eeda83cc15b3ecdeb71d7ff58a5f57"
            }
            "portable_lossless_420_split_12x4_a.avif"
            | "portable_lossless_420_split_4x12_a.avif" => {
                "09fddd84398ad9a9d3ce8b981fea278a82e6b1fa62483fa0ef3c45cd484ae29e"
            }
            "portable_lossless_420_split_16x4_a.avif"
            | "portable_lossless_420_split_4x16_a.avif" => {
                "1f403e7f414473b888fcba438d60d269e54fc1d04c802dd32f96fa657932b2ac"
            }
            "portable_lossless_420_split_12x8_a.avif"
            | "portable_lossless_420_split_8x12_a.avif" => {
                "47c4a5d65d8ac82aa68f04754b38e5bf00438aeb64b2e48c2bb54a9268e6e4e7"
            }
            "portable_lossless_420_split_16x8_a.avif"
            | "portable_lossless_420_split_8x16_a.avif" => {
                "983aef668db1ea0d5801725fdf2b49d32232fc7f1d9ae578a03ffad6aebc4fc2"
            }
            "portable_lossless_420_square_12x12_a.avif" => {
                "cbc97cf0c2652e60e6e36611be9869444f603abf5f48b292a03d340f501320f8"
            }
            "portable_lossless_420_square_12x16_a.avif"
            | "portable_lossless_420_square_16x12_a.avif" => {
                "f6b42085d682a064da2a9956545f33ae7595b288f7589e8e498c62e6bc26e874"
            }
            "portable_lossless_420_square_16x16_a.avif" => {
                "8bdcc97ae19b09ec3d6b76a7d59f13d4aa3dd7a06d21db706f2a1d15caaa0431"
            }
            "partitioned_square_420_16x16_rgb_delta.avif" => {
                "33170bbddccc8cf1c2ce5dada1ab0dc1c510fc9b059ede87dff076f9df47e18d"
            }
            "partitioned_square_420_16x16_g96.avif" => {
                "1773a465660162ba2a563e2b05acb59d0ccd578de177210f9252a9abd2013bcf"
            }
            "coverage_r8x16_band_05.avif" => {
                "c11a94094afc690f85b60f373368af7995dca863a978e1835386df16567d5840"
            }
            "coverage_r8x16_band_06.avif" => {
                "70a7a0107bec2a81f759155aaf760088704eff6de4c628616a5173a3fb0df610"
            }
            "coverage_r16x32_grid_01.avif" => {
                "8a72d87e179a92b6fb293008f6fbfabc4df0ead6cd96311b1345f6f706c8eeac"
            }
            "coverage_r16x64_grid_01.avif" => {
                "f17df57e0946031d2b81ad5316e801aea9c27fe94422f360b1e328013b71ea15"
            }
            "coverage_adst_public_02.avif" => {
                "d872557591a66de992c9ecb7af416ac0c5d8dd364c0c26f1acc2ec530b75375f"
            }
            "coverage_adst_public_03.avif" => {
                "c4cbd418d7f72de0fd778268c0a4c40ac6c30b982987a3a4bfa84372c3c102e9"
            }
            "coverage_adst_public_04.avif" => {
                "8bf5648d07e20627c47a5909233a14efdeba2d9bb30ac51c2f1d0e9c3dc568f8"
            }
            "coverage_adst_public_05.avif" => {
                "ccf631ee65a05977a2020995f5dc442905ad0c21450f3e3e0df3bd0f0d2b8e11"
            }
            "coverage_adst_public_06.avif" => {
                "988aef43dcf1c4eeaa0cffee66f3ba32e9c127c0b07996830900b4a79ed07cd6"
            }
            "coverage_adst_public_07.avif" => {
                "a40858233036b25f36900bd39be40e6eda843493ac27b767448b891ac8437492"
            }
            "coverage_adst_public_08.avif" => {
                "8b308e80e0a1a904072657a1f8b3472b5b89e37dc01238c8dc6066689a9ebf6a"
            }
            "coverage_adst_public_09.avif" => {
                "e0e5a1ae7b7aef892258e7f7f2332f13f959b419ba0f9b14c8edcc9a298e487d"
            }
            "coverage_adst_public_10.avif" => {
                "93047df7e452ceca5c0cf243100db0b2e1508e7db35d86dc00ad34b70069db4e"
            }
            "coverage_i444_palette2_square8_four_leaves.avif" => {
                "ae90d60419a44e909e312e762e05d6f73d70d32c43366eb8885aabe4d2c7725b"
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
                "portable_lossless_420_a.avif"
                    | "partitioned_12x4_a.avif"
                    | "partitioned_4x12_a.avif"
                    | "partitioned_square_12x12_luma_eob1.avif"
                    | "partitioned_square_12x12_luma_eob2_control.avif"
                    | "partitioned_square_12x12_luma_eob4_control.avif"
                    | "partitioned_square_12x12_luma_eob6_control.avif"
                    | "partitioned_square_12x12_luma_eob9_control.avif"
                    | "partitioned_square_12x12_luma_eob10_control.avif"
                    | "partitioned_square_12x12_luma_eob12_control.avif"
                    | "partitioned_square_12x12_luma_eob15_control.avif"
                    | "partitioned_square_12x12_midpoint_g96_ac.avif"
                    | "partitioned_square_12x12_top_left_luma_eob12_control.avif"
                    | "partitioned_square_12x12_top_left_luma_eob4.avif"
                    | "partitioned_square_16x16_g64.avif"
                    | "portable_lossy_420_q99_gray_127.avif"
                    | "portable_lossless_420_split_12x4_a.avif"
                    | "portable_lossless_420_split_4x12_a.avif"
                    | "partitioned_square_420_16x16_rgb_delta.avif"
                    | "partitioned_square_420_16x16_g96.avif"
            )
        {
            img::__coverage_sweep_av1_first_leaf(&input);
        }
    }

    let masked_fixture = "portable_lossy_420_q99_token_2097724_masked_572.avif";
    let masked_input = require_ok(
        fs::read(
            fixture_root
                .join("input")
                .join("images")
                .join("avif")
                .join(masked_fixture),
        ),
        "masked-token AVIF fixture must be readable",
    );
    let masked_trace = require_ok(
        img::__coverage_av1_reconstruction(&masked_input),
        "masked-token AVIF reconstruction validation must succeed",
    )
    .expect("masked-token AVIF must retain its portable reconstruction");
    assert_eq!((masked_trace.width, masked_trace.height), (4, 4));
    assert_eq!(masked_trace.planes[0], vec![199; 16]);
    assert_eq!(masked_trace.planes[1], vec![128; 4]);
    assert_eq!(masked_trace.planes[2], vec![128; 4]);
    let masked_reference = require_ok(
        fs::read(
            fixture_root
                .join("outputs")
                .join("raws")
                .join("Decode.avif_portable_lossy_420_q99_token_2097724_masked_572_avif.bin"),
        ),
        "masked-token Pillow reference must be readable",
    );
    assert_eq!(masked_reference, vec![199; 48]);
    let masked_decoded = require_ok(
        img::decode(&masked_input),
        "masked-token AVIF must decode through the portable production path",
    );
    assert_eq!(masked_decoded.content.pixels, masked_reference);

    for fixture in [
        "animated.avif",
        "10bit.avif",
        "portable_lossy_420_q99_eob_bin_control.avif",
        "portable_lossy_420_q99_eob_base_control.avif",
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
            require_ok(
                img::__coverage_av1_reconstruction(&input),
                "non-portable AVIF reconstruction validation must succeed",
            )
            .is_none(),
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
