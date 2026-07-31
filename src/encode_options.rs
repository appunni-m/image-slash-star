//! Typed, format-specific encoder configuration.
//!
//! Every [`EncodeOptions`] value names exactly one target format. The common
//! [`crate::encode`] and [`crate::encode_sequence`] entry points reject an
//! option value whose target differs from their explicit [`ImageFormat`].

use crate::{ImageError, ImageFormat, ImageResult};
use std::collections::BTreeSet;

/// Format-qualified options accepted by the common encode APIs.
///
/// Use [`Self::for_format`] for format defaults or construct the matching
/// codec record explicitly. There is deliberately no target-free `Default`
/// implementation.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodeOptions {
    /// JPEG encoder settings.
    Jpeg(JpegEncodeOptions),
    /// PNG encoder settings.
    Png(PngEncodeOptions),
    /// GIF encoder settings.
    Gif(GifEncodeOptions),
    /// BMP encoder settings.
    Bmp(BmpEncodeOptions),
    /// TIFF encoder settings.
    Tiff(TiffEncodeOptions),
    /// WebP encoder settings.
    WebP(WebPEncodeOptions),
    /// ICO encoder settings.
    Ico(IcoEncodeOptions),
    /// AVIF encoder settings.
    Avif(AvifEncodeOptions),
}

impl EncodeOptions {
    /// Create the selected format's default option record.
    #[must_use]
    pub fn for_format(format: ImageFormat) -> Self {
        match format {
            ImageFormat::Jpeg => JpegEncodeOptions::default().into(),
            ImageFormat::Png => PngEncodeOptions::default().into(),
            ImageFormat::Gif => GifEncodeOptions::default().into(),
            ImageFormat::Bmp => BmpEncodeOptions::default().into(),
            ImageFormat::Tiff => TiffEncodeOptions::default().into(),
            ImageFormat::WebP => WebPEncodeOptions::default().into(),
            ImageFormat::Ico => IcoEncodeOptions::default().into(),
            ImageFormat::Avif => AvifEncodeOptions::default().into(),
        }
    }

    /// Return the target format named by this option value.
    #[must_use]
    pub const fn format(&self) -> ImageFormat {
        match self {
            Self::Jpeg(_) => ImageFormat::Jpeg,
            Self::Png(_) => ImageFormat::Png,
            Self::Gif(_) => ImageFormat::Gif,
            Self::Bmp(_) => ImageFormat::Bmp,
            Self::Tiff(_) => ImageFormat::Tiff,
            Self::WebP(_) => ImageFormat::WebP,
            Self::Ico(_) => ImageFormat::Ico,
            Self::Avif(_) => ImageFormat::Avif,
        }
    }

    /// Strictly translate the previous string-pair configuration boundary.
    ///
    /// This migration adapter accepts only keys that the selected codec
    /// historically consumed. Unknown or duplicate keys return a
    /// format-qualified [`ImageError::Parameter`]. Encoders do not inspect
    /// string keys; successful translation produces the same typed records
    /// callers can construct directly.
    ///
    /// AVIF's ordered advanced codec options are intentionally not string-map
    /// fields. Add them to [`AvifEncodeOptions::advanced`] after translation.
    ///
    /// # Errors
    ///
    /// Returns [`ImageError::Parameter`] for an unknown key, duplicate key, or
    /// value outside the selected option's syntax.
    pub fn try_from_legacy_pairs(
        format: ImageFormat,
        pairs: &[(String, String)],
    ) -> ImageResult<Self> {
        reject_duplicate_keys(format, pairs)?;
        match format {
            ImageFormat::Jpeg => parse_jpeg(pairs).map(Self::Jpeg),
            ImageFormat::Png => parse_png(pairs).map(Self::Png),
            ImageFormat::Gif => parse_gif(pairs).map(Self::Gif),
            ImageFormat::Bmp => parse_bmp(pairs).map(Self::Bmp),
            ImageFormat::Tiff => parse_tiff(pairs).map(Self::Tiff),
            ImageFormat::WebP => parse_webp(pairs).map(Self::WebP),
            ImageFormat::Ico => parse_ico(pairs).map(Self::Ico),
            ImageFormat::Avif => parse_avif(pairs).map(Self::Avif),
        }
    }
}

impl From<JpegEncodeOptions> for EncodeOptions {
    fn from(options: JpegEncodeOptions) -> Self {
        Self::Jpeg(options)
    }
}

impl From<PngEncodeOptions> for EncodeOptions {
    fn from(options: PngEncodeOptions) -> Self {
        Self::Png(options)
    }
}

impl From<GifEncodeOptions> for EncodeOptions {
    fn from(options: GifEncodeOptions) -> Self {
        Self::Gif(options)
    }
}

impl From<BmpEncodeOptions> for EncodeOptions {
    fn from(options: BmpEncodeOptions) -> Self {
        Self::Bmp(options)
    }
}

impl From<TiffEncodeOptions> for EncodeOptions {
    fn from(options: TiffEncodeOptions) -> Self {
        Self::Tiff(options)
    }
}

impl From<WebPEncodeOptions> for EncodeOptions {
    fn from(options: WebPEncodeOptions) -> Self {
        Self::WebP(options)
    }
}

impl From<IcoEncodeOptions> for EncodeOptions {
    fn from(options: IcoEncodeOptions) -> Self {
        Self::Ico(options)
    }
}

impl From<AvifEncodeOptions> for EncodeOptions {
    fn from(options: AvifEncodeOptions) -> Self {
        Self::Avif(options)
    }
}

/// JPEG chroma-subsampling layout.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JpegSubsampling {
    /// Preserve one chroma sample per luma sample.
    Cs444,
    /// Horizontally halve chroma resolution.
    Cs422,
    /// Horizontally and vertically halve chroma resolution.
    Cs420,
}

/// JPEG encoder settings.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct JpegEncodeOptions {
    /// Quality override in the Pillow `0..=100` scale.
    pub quality: Option<u8>,
    /// Whether to emit progressive scans.
    pub progressive: Option<bool>,
    /// Whether to optimize Huffman tables.
    pub optimize: Option<bool>,
    /// Chroma-subsampling override.
    pub subsampling: Option<JpegSubsampling>,
    /// Restart interval in MCU rows.
    pub restart_interval: Option<u32>,
    /// Exact EXIF APP1 payload bytes.
    pub exif: Option<Vec<u8>>,
}

/// PNG DEFLATE compression selection.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PngCompression {
    /// Store DEFLATE blocks without compression.
    None,
    /// Use the pinned Pillow-compatible default level.
    Default,
    /// Use maximum compression.
    Maximum,
    /// Use one explicit zlib level.
    Level(u8),
}

/// PNG encoder settings.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PngEncodeOptions {
    /// DEFLATE compression override.
    pub compression: Option<PngCompression>,
    /// Whether to use the pinned maximum-compression optimization path.
    pub optimize: Option<bool>,
    /// Pillow 12.2.0 compatibility input for its ignored interlace save flag.
    ///
    /// The current encoder always emits non-Adam7 PNG. This field makes that
    /// compatibility behavior explicit rather than silently accepting a
    /// cross-codec option.
    pub interlace: Option<bool>,
    legacy_ancillary: PngLegacyAncillary,
}

#[cfg(feature = "png")]
impl PngEncodeOptions {
    pub(crate) const fn legacy_gamma(&self) -> bool {
        self.legacy_ancillary.gamma
    }

    pub(crate) const fn legacy_srgb(&self) -> bool {
        self.legacy_ancillary.srgb
    }

    pub(crate) const fn legacy_physical(&self) -> bool {
        self.legacy_ancillary.physical
    }

    pub(crate) const fn legacy_text_chunks(&self) -> bool {
        self.legacy_ancillary.text_chunks
    }

    pub(crate) const fn legacy_time(&self) -> bool {
        self.legacy_ancillary.time
    }

    #[cfg(coverage)]
    pub(crate) fn __coverage_legacy_ancillary() -> Self {
        Self {
            compression: Some(PngCompression::None),
            legacy_ancillary: PngLegacyAncillary {
                gamma: true,
                srgb: true,
                physical: true,
                text_chunks: true,
                time: true,
            },
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct PngLegacyAncillary {
    gamma: bool,
    srgb: bool,
    physical: bool,
    text_chunks: bool,
    time: bool,
}

/// GIF color-table placement.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GifColorTable {
    /// Store one global table.
    Global,
    /// Store local tables with frames.
    Local,
}

/// Explicit GIF loop override.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GifLoop {
    /// Repeat indefinitely.
    Infinite,
    /// Use the encoded GIF repetition field.
    Finite(u16),
}

/// GIF encoder settings.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GifEncodeOptions {
    /// Override whether the output retains multiple frames.
    pub animated: Option<bool>,
    /// Override frame interlacing.
    pub interlace: Option<bool>,
    /// Override every output frame's disposal operation.
    pub disposal: Option<crate::FrameDisposal>,
    /// Select global or local frame tables.
    pub color_table: Option<GifColorTable>,
    /// Override whether a transparent palette entry is emitted.
    pub transparency: Option<bool>,
    /// Override the sequence loop field.
    pub loop_count: Option<GifLoop>,
}

/// BMP encoder settings.
///
/// The current deterministic BMP encoder has no configurable parameters.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BmpEncodeOptions {}

/// TIFF compression algorithm.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TiffCompression {
    /// Uncompressed strips.
    Raw,
    /// TIFF LZW.
    Lzw,
    /// Adobe Deflate.
    Deflate,
    /// PackBits run-length encoding.
    PackBits,
}

/// TIFF sample predictor.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TiffPredictor {
    /// No prediction.
    None,
    /// Horizontal differencing.
    Horizontal,
}

/// TIFF encoder settings.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TiffEncodeOptions {
    /// Strip compression override.
    pub compression: Option<TiffCompression>,
    /// Sample predictor override.
    pub predictor: Option<TiffPredictor>,
    #[cfg(coverage)]
    force_output_len_overflow: bool,
    #[cfg(coverage)]
    force_sequence_len_overflow: bool,
}

#[cfg(coverage)]
impl TiffEncodeOptions {
    pub(crate) const fn force_output_len_overflow(&self) -> bool {
        self.force_output_len_overflow
    }

    pub(crate) const fn force_sequence_len_overflow(&self) -> bool {
        self.force_sequence_len_overflow
    }

    pub(crate) fn set_force_output_len_overflow(&mut self) {
        self.force_output_len_overflow = true;
    }

    pub(crate) fn set_force_sequence_len_overflow(&mut self) {
        self.force_sequence_len_overflow = true;
    }
}

/// WebP encoder settings.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WebPEncodeOptions {
    /// Quality override in the Pillow `0..=100` scale.
    pub quality: Option<u8>,
    /// Select the lossless VP8L path.
    pub lossless: Option<bool>,
    /// Encoder effort in the `0..=6` range.
    pub method: Option<u8>,
    /// Exact ICC profile bytes.
    pub icc: Option<Vec<u8>>,
    /// Exact EXIF chunk bytes.
    pub exif: Option<Vec<u8>>,
    /// Exact XMP chunk bytes.
    pub xmp: Option<Vec<u8>>,
    legacy_sequence: WebPLegacySequenceOptions,
    #[cfg(coverage)]
    force_riff_size_overflow: bool,
}

#[cfg(feature = "webp")]
impl WebPEncodeOptions {
    pub(crate) const fn legacy_kmax(&self) -> Option<u32> {
        self.legacy_sequence.kmax
    }

    pub(crate) const fn has_unsupported_legacy_sequence_option(&self) -> bool {
        self.legacy_sequence.kmin.is_some()
            || self.legacy_sequence.minimize_size.is_some()
            || self.legacy_sequence.allow_mixed.is_some()
    }
}

#[cfg(coverage)]
impl WebPEncodeOptions {
    pub(crate) const fn force_riff_size_overflow(&self) -> bool {
        self.force_riff_size_overflow
    }

    pub(crate) fn set_force_riff_size_overflow(&mut self) {
        self.force_riff_size_overflow = true;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct WebPLegacySequenceOptions {
    kmax: Option<u32>,
    kmin: Option<u32>,
    minimize_size: Option<bool>,
    allow_mixed: Option<bool>,
}

/// ICO entry representation.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum IcoEntryType {
    /// Store a PNG-backed entry.
    #[default]
    Png,
    /// Store a DIB/BMP-backed entry.
    Bmp,
}

/// One requested ICO directory size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct IcoSize {
    /// Width in pixels.
    pub width: u16,
    /// Height in pixels.
    pub height: u16,
}

/// ICO encoder settings.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IcoEncodeOptions {
    /// Entry payload representation.
    pub entry_type: IcoEntryType,
    /// Requested directory sizes.
    ///
    /// The current no-processing encoder requires exactly one size equal to
    /// the source dimensions.
    pub sizes: Vec<IcoSize>,
}

/// Native AVIF encoder backend.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AvifCodec {
    /// Let libavif select its available backend.
    Auto,
    /// Require the AOM backend.
    Aom,
}

/// AVIF YUV sample layout.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AvifSubsampling {
    /// YUV 4:4:4.
    Cs444,
    /// YUV 4:2:2.
    Cs422,
    /// YUV 4:2:0.
    Cs420,
    /// Monochrome 4:0:0.
    Cs400,
}

/// AVIF YUV range.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AvifRange {
    /// Full-range samples.
    Full,
    /// Limited-range samples.
    Limited,
}

/// One ordered libavif codec-specific option.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AvifAdvancedOption {
    /// Backend option name.
    pub key: String,
    /// Backend option value.
    pub value: String,
}

/// AVIF encoder settings.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AvifEncodeOptions {
    /// Quality override in the Pillow `0..=100` scale.
    pub quality: Option<u8>,
    /// Native encoder backend selection.
    pub codec: Option<AvifCodec>,
    /// YUV subsampling override.
    pub subsampling: Option<AvifSubsampling>,
    /// YUV range override.
    pub range: Option<AvifRange>,
    /// Native encoder speed.
    pub speed: Option<i32>,
    /// Maximum native encoder thread count.
    pub max_threads: Option<i32>,
    /// Log2 tile-row count.
    pub tile_rows: Option<i32>,
    /// Log2 tile-column count.
    pub tile_cols: Option<i32>,
    /// Mark alpha as premultiplied in the AVIF container.
    pub alpha_premultiplied: Option<bool>,
    /// Let libavif choose tile layout automatically.
    pub autotiling: Option<bool>,
    /// Exact ICC profile bytes.
    pub icc: Option<Vec<u8>>,
    /// Exact EXIF bytes.
    pub exif: Option<Vec<u8>>,
    /// EXIF orientation value.
    pub exif_orientation: Option<i32>,
    /// Exact XMP bytes.
    pub xmp: Option<Vec<u8>>,
    /// Ordered codec-specific backend settings.
    pub advanced: Vec<AvifAdvancedOption>,
    /// Native sequence timescale override.
    pub sequence_time: Option<u64>,
}

fn parameter(format: ImageFormat, message: impl Into<String>) -> ImageError {
    ImageError::Parameter {
        format: Some(format),
        message: message.into(),
        stage: None,
    }
}

fn reject_duplicate_keys(format: ImageFormat, pairs: &[(String, String)]) -> ImageResult<()> {
    let mut seen = BTreeSet::new();
    for (key, _) in pairs {
        if !seen.insert(key.as_str()) {
            return Err(parameter(
                format,
                format!("duplicate legacy encode option `{key}`"),
            ));
        }
    }
    Ok(())
}

fn unknown(format: ImageFormat, key: &str) -> ImageError {
    parameter(format, format!("unknown legacy encode option `{key}`"))
}

fn parse_bool(format: ImageFormat, key: &str, value: &str) -> ImageResult<bool> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => Err(parameter(
            format,
            format!("invalid boolean value for `{key}`"),
        )),
    }
}

fn parse_number<T>(format: ImageFormat, key: &str, value: &str) -> ImageResult<T>
where
    T: std::str::FromStr,
{
    value
        .parse()
        .map_err(|_| parameter(format, format!("invalid numeric value for `{key}`")))
}

fn parse_hex(format: ImageFormat, key: &str, value: &str) -> ImageResult<Vec<u8>> {
    let bytes = value.as_bytes();
    if !bytes.len().is_multiple_of(2) {
        return Err(parameter(format, format!("invalid hex value for `{key}`")));
    }
    bytes
        .chunks_exact(2)
        .map(|pair| {
            let high = hex_nibble(format, key, pair[0])?;
            let low = hex_nibble(format, key, pair[1])?;
            Ok((high << 4) | low)
        })
        .collect()
}

fn hex_nibble(format: ImageFormat, key: &str, value: u8) -> ImageResult<u8> {
    match value {
        b'0'..=b'9' => Ok(value.saturating_sub(b'0')),
        b'a'..=b'f' => Ok(value.saturating_sub(b'a').saturating_add(10)),
        b'A'..=b'F' => Ok(value.saturating_sub(b'A').saturating_add(10)),
        _ => Err(parameter(format, format!("invalid hex value for `{key}`"))),
    }
}

fn parse_jpeg(pairs: &[(String, String)]) -> ImageResult<JpegEncodeOptions> {
    let format = ImageFormat::Jpeg;
    let mut options = JpegEncodeOptions::default();
    for (key, value) in pairs {
        match key.as_str() {
            "quality" => options.quality = Some(parse_number(format, key, value)?),
            "progressive" => options.progressive = Some(parse_bool(format, key, value)?),
            "optimize" => options.optimize = Some(parse_bool(format, key, value)?),
            "subsampling" => {
                options.subsampling = Some(match value.as_str() {
                    "444" | "4:4:4" => JpegSubsampling::Cs444,
                    "422" | "4:2:2" => JpegSubsampling::Cs422,
                    "420" | "4:2:0" => JpegSubsampling::Cs420,
                    _ => return Err(parameter(format, "invalid JPEG subsampling option")),
                });
            }
            "restart_interval" => {
                options.restart_interval = Some(parse_number(format, key, value)?);
            }
            "exif_hex" => options.exif = Some(parse_hex(format, key, value)?),
            _ => return Err(unknown(format, key)),
        }
    }
    Ok(options)
}

fn parse_png(pairs: &[(String, String)]) -> ImageResult<PngEncodeOptions> {
    let format = ImageFormat::Png;
    let mut options = PngEncodeOptions::default();
    for (key, value) in pairs {
        match key.as_str() {
            "compression" => {
                options.compression = Some(match value.as_str() {
                    "none" => PngCompression::None,
                    "default" => PngCompression::Default,
                    "max" => PngCompression::Maximum,
                    number => PngCompression::Level(parse_number(format, key, number)?),
                });
            }
            "optimize" => options.optimize = Some(parse_bool(format, key, value)?),
            "interlace" | "interlaced" => {
                options.interlace = Some(parse_bool(format, key, value)?);
            }
            "gamma" => options.legacy_ancillary.gamma = parse_bool(format, key, value)?,
            "srgb" => options.legacy_ancillary.srgb = parse_bool(format, key, value)?,
            "physical" => options.legacy_ancillary.physical = parse_bool(format, key, value)?,
            "text_chunks" => {
                options.legacy_ancillary.text_chunks = parse_bool(format, key, value)?;
            }
            "time" => options.legacy_ancillary.time = parse_bool(format, key, value)?,
            _ => return Err(unknown(format, key)),
        }
    }
    Ok(options)
}

fn parse_gif(pairs: &[(String, String)]) -> ImageResult<GifEncodeOptions> {
    let format = ImageFormat::Gif;
    let mut options = GifEncodeOptions::default();
    for (key, value) in pairs {
        match key.as_str() {
            "animated" => options.animated = Some(parse_bool(format, key, value)?),
            "interlace" | "interlaced" => {
                options.interlace = Some(parse_bool(format, key, value)?);
            }
            "disposal" => {
                options.disposal = Some(match value.as_str() {
                    "none" | "0" => crate::FrameDisposal::Unspecified,
                    "keep" | "1" => crate::FrameDisposal::Keep,
                    "background" | "2" => crate::FrameDisposal::Background,
                    "previous" | "3" => crate::FrameDisposal::Previous,
                    _ => return Err(parameter(format, "invalid GIF disposal option")),
                });
            }
            "color_table" => {
                options.color_table = Some(match value.as_str() {
                    "global" => GifColorTable::Global,
                    "local" => GifColorTable::Local,
                    _ => return Err(parameter(format, "invalid GIF color-table option")),
                });
            }
            "transparency" => {
                options.transparency = Some(parse_bool(format, key, value)?);
            }
            "loop" => {
                options.loop_count = match value.as_str() {
                    "false" => None,
                    "true" | "infinite" => Some(GifLoop::Infinite),
                    number => Some(GifLoop::Finite(parse_number(format, key, number)?)),
                };
            }
            _ => return Err(unknown(format, key)),
        }
    }
    Ok(options)
}

fn parse_bmp(pairs: &[(String, String)]) -> ImageResult<BmpEncodeOptions> {
    if let Some((key, _)) = pairs.first() {
        return Err(unknown(ImageFormat::Bmp, key));
    }
    Ok(BmpEncodeOptions::default())
}

fn parse_tiff(pairs: &[(String, String)]) -> ImageResult<TiffEncodeOptions> {
    let format = ImageFormat::Tiff;
    let mut options = TiffEncodeOptions::default();
    for (key, value) in pairs {
        match key.as_str() {
            "compression" => {
                options.compression = Some(match value.as_str() {
                    "none" | "raw" => TiffCompression::Raw,
                    "lzw" | "tiff_lzw" => TiffCompression::Lzw,
                    "deflate" | "tiff_adobe_deflate" => TiffCompression::Deflate,
                    "packbits" => TiffCompression::PackBits,
                    _ => return Err(parameter(format, "invalid TIFF compression option")),
                });
            }
            "predictor" => {
                options.predictor = Some(match value.as_str() {
                    "none" | "1" => TiffPredictor::None,
                    "horizontal" | "2" => TiffPredictor::Horizontal,
                    _ => return Err(parameter(format, "invalid TIFF predictor option")),
                });
            }
            _ => return Err(unknown(format, key)),
        }
    }
    Ok(options)
}

fn parse_webp(pairs: &[(String, String)]) -> ImageResult<WebPEncodeOptions> {
    let format = ImageFormat::WebP;
    let mut options = WebPEncodeOptions::default();
    for (key, value) in pairs {
        match key.as_str() {
            "quality" => options.quality = Some(parse_number(format, key, value)?),
            "lossless" => options.lossless = Some(parse_bool(format, key, value)?),
            "method" => options.method = Some(parse_number(format, key, value)?),
            "icc_hex" => options.icc = Some(parse_hex(format, key, value)?),
            "exif_hex" => options.exif = Some(parse_hex(format, key, value)?),
            "xmp_hex" => options.xmp = Some(parse_hex(format, key, value)?),
            "kmax" => {
                options.legacy_sequence.kmax = Some(parse_number(format, key, value)?);
            }
            "kmin" => {
                options.legacy_sequence.kmin = Some(parse_number(format, key, value)?);
            }
            "minimize_size" => {
                options.legacy_sequence.minimize_size = Some(parse_bool(format, key, value)?);
            }
            "allow_mixed" => {
                options.legacy_sequence.allow_mixed = Some(parse_bool(format, key, value)?);
            }
            _ => return Err(unknown(format, key)),
        }
    }
    Ok(options)
}

fn parse_ico(pairs: &[(String, String)]) -> ImageResult<IcoEncodeOptions> {
    let format = ImageFormat::Ico;
    let mut options = IcoEncodeOptions::default();
    for (key, value) in pairs {
        match key.as_str() {
            "entry_type" => {
                options.entry_type = match value.as_str() {
                    "png" => IcoEntryType::Png,
                    "bmp" => IcoEntryType::Bmp,
                    _ => return Err(parameter(format, "invalid ICO entry type")),
                };
            }
            "sizes" => {
                let numbers = value
                    .split(|character: char| !character.is_ascii_digit())
                    .filter(|part| !part.is_empty())
                    .map(|part| parse_number::<u16>(format, key, part))
                    .collect::<ImageResult<Vec<_>>>()?;
                let [width, height] = numbers.as_slice() else {
                    return Err(parameter(
                        format,
                        "ICO sizes must contain exactly one width-height pair",
                    ));
                };
                options.sizes = vec![IcoSize {
                    width: *width,
                    height: *height,
                }];
            }
            _ => return Err(unknown(format, key)),
        }
    }
    Ok(options)
}

fn parse_avif(pairs: &[(String, String)]) -> ImageResult<AvifEncodeOptions> {
    let format = ImageFormat::Avif;
    let mut options = AvifEncodeOptions::default();
    for (key, value) in pairs {
        match key.as_str() {
            "quality" => options.quality = Some(parse_number(format, key, value)?),
            "codec" => {
                options.codec = Some(match value.as_str() {
                    "auto" => AvifCodec::Auto,
                    "aom" => AvifCodec::Aom,
                    _ => return Err(parameter(format, "invalid AVIF codec option")),
                });
            }
            "subsampling" => {
                options.subsampling = Some(match value.as_str() {
                    "4:4:4" => AvifSubsampling::Cs444,
                    "4:2:2" => AvifSubsampling::Cs422,
                    "4:2:0" => AvifSubsampling::Cs420,
                    "4:0:0" => AvifSubsampling::Cs400,
                    _ => return Err(parameter(format, "invalid AVIF subsampling option")),
                });
            }
            "range" => {
                options.range = Some(match value.as_str() {
                    "full" => AvifRange::Full,
                    "limited" => AvifRange::Limited,
                    _ => return Err(parameter(format, "invalid AVIF range option")),
                });
            }
            "speed" => options.speed = Some(parse_number(format, key, value)?),
            "max_threads" => options.max_threads = Some(parse_number(format, key, value)?),
            "tile_rows" => options.tile_rows = Some(parse_number(format, key, value)?),
            "tile_cols" => options.tile_cols = Some(parse_number(format, key, value)?),
            "alpha_premultiplied" => {
                options.alpha_premultiplied = Some(parse_bool(format, key, value)?);
            }
            "autotiling" => options.autotiling = Some(parse_bool(format, key, value)?),
            "icc_hex" => options.icc = Some(parse_hex(format, key, value)?),
            "exif_hex" => options.exif = Some(parse_hex(format, key, value)?),
            "exif_orientation" => {
                options.exif_orientation = Some(parse_number(format, key, value)?);
            }
            "xmp_hex" => options.xmp = Some(parse_hex(format, key, value)?),
            "sequence_time" => {
                options.sequence_time = Some(parse_number(format, key, value)?);
            }
            _ => return Err(unknown(format, key)),
        }
    }
    Ok(options)
}
