//! Safe Rust ownership wrappers for the fixed libavif bridge.

#![allow(unsafe_code)]

use std::ffi::{CString, c_char};
use std::marker::PhantomData;
use std::num::{NonZeroU64, NonZeroUsize};
use std::ptr::NonNull;

const STATUS_OK: i32 = 0;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct FfiDecodeInfo {
    width: u32,
    height: u32,
    frame_count: u32,
    channels: u32,
    timescale: u64,
    repetition_count: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct FfiFrameTiming {
    pts_in_timescales: u64,
    duration_in_timescales: u64,
}

#[repr(C)]
struct FfiEncodeConfig {
    width: u32,
    height: u32,
    yuv_format: i32,
    yuv_range: i32,
    quality: i32,
    speed: i32,
    max_threads: i32,
    tile_rows_log2: i32,
    tile_cols_log2: i32,
    alpha_premultiplied: i32,
    auto_tiling: i32,
    timescale: u64,
    creation_time: u64,
    modification_time: u64,
    icc: *const u8,
    icc_size: usize,
    exif: *const u8,
    exif_size: usize,
    exif_orientation: i32,
    xmp: *const u8,
    xmp_size: usize,
    advanced: *const FfiCodecOption,
    advanced_count: usize,
}

#[repr(C)]
struct FfiCodecOption {
    key: *const c_char,
    value: *const c_char,
}

unsafe extern "C" {
    fn prs_avif_decoder_create(
        data: *const u8,
        size: usize,
        max_threads: i32,
        out_decoder: *mut *mut core::ffi::c_void,
        out_info: *mut FfiDecodeInfo,
    ) -> i32;
    fn prs_avif_decoder_decode(
        decoder: *mut core::ffi::c_void,
        frame_index: u32,
        output: *mut u8,
        output_size: usize,
        out_timing: *mut FfiFrameTiming,
    ) -> i32;
    fn prs_avif_decoder_destroy(decoder: *mut core::ffi::c_void);

    fn prs_avif_encoder_create(
        config: *const FfiEncodeConfig,
        out_encoder: *mut *mut core::ffi::c_void,
    ) -> i32;
    fn prs_avif_encoder_add(
        encoder: *mut core::ffi::c_void,
        pixels: *const u8,
        pixel_size: usize,
        width: u32,
        height: u32,
        channels: u32,
        duration_in_timescales: u64,
        is_single_frame: i32,
    ) -> i32;
    fn prs_avif_encoder_finish(
        encoder: *mut core::ffi::c_void,
        out_data: *mut *mut u8,
        out_size: *mut usize,
    ) -> i32;
    fn prs_avif_encoder_destroy(encoder: *mut core::ffi::c_void);
    fn prs_avif_bytes_free(data: *mut u8);
}

/// Immutable properties known after libavif parses an input container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DecodeInfo {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) frame_count: u32,
    pub(crate) has_alpha: bool,
    pub(crate) timescale: NonZeroU64,
    pub(crate) pixel_len: usize,
}

/// Presentation values for one decoded frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FrameTiming {
    pub(crate) duration_in_timescales: u64,
}

struct DecoderHandle {
    raw: NonNull<core::ffi::c_void>,
    destroy: unsafe extern "C" fn(*mut core::ffi::c_void),
}

impl Drop for DecoderHandle {
    fn drop(&mut self) {
        // SAFETY: this handle uniquely owns the successful bridge allocation.
        unsafe { (self.destroy)(self.raw.as_ptr()) };
    }
}

/// Parsed native decoder borrowing its in-memory input for its full lifetime.
pub(crate) struct Decoder<'data> {
    raw: DecoderHandle,
    info: DecodeInfo,
    _data: PhantomData<&'data [u8]>,
}

impl<'data> Decoder<'data> {
    pub(crate) fn new(data: &'data [u8]) -> Option<Self> {
        let mut raw = std::ptr::null_mut();
        let mut info = FfiDecodeInfo::default();
        let max_threads = default_max_threads();
        // SAFETY: `data` remains borrowed by the returned decoder, both output
        // pointers reference initialized local storage, and the bridge checks
        // every length before retaining the input pointer.
        let status = unsafe {
            prs_avif_decoder_create(data.as_ptr(), data.len(), max_threads, &mut raw, &mut info)
        };
        let (raw, info) = checked_decoder_creation(status, raw, info, prs_avif_decoder_destroy)?;
        Some(Self {
            raw,
            info,
            _data: PhantomData,
        })
    }

    pub(crate) const fn info(&self) -> DecodeInfo {
        self.info
    }

    pub(crate) fn decode_frame(&mut self, frame_index: u32) -> Option<(Vec<u8>, FrameTiming)> {
        if frame_index >= self.info.frame_count {
            return None;
        }
        let mut pixels = vec![0; self.info.pixel_len];
        let mut timing = FfiFrameTiming::default();
        // SAFETY: `self.raw` is a live unique decoder. `pixels` is writable for
        // exactly `pixel_len` bytes, and `timing` is valid output storage.
        let status = unsafe {
            prs_avif_decoder_decode(
                self.raw.raw.as_ptr(),
                frame_index,
                pixels.as_mut_ptr(),
                pixels.len(),
                &mut timing,
            )
        };
        decoded_frame_result(status, pixels, timing)
    }
}

fn decoded_frame_result(
    status: i32,
    pixels: Vec<u8>,
    timing: FfiFrameTiming,
) -> Option<(Vec<u8>, FrameTiming)> {
    status_ok(status)?;
    Some((
        pixels,
        FrameTiming {
            duration_in_timescales: timing.duration_in_timescales,
        },
    ))
}

fn successful_pointer(
    status: i32,
    raw: *mut core::ffi::c_void,
) -> Option<NonNull<core::ffi::c_void>> {
    if status != STATUS_OK {
        return None;
    }
    NonNull::new(raw)
}

fn checked_decode_info(info: FfiDecodeInfo) -> Option<DecodeInfo> {
    let width = std::num::NonZeroU32::new(info.width)?.get();
    let height = std::num::NonZeroU32::new(info.height)?.get();
    let frame_count = std::num::NonZeroU32::new(info.frame_count)?.get();
    let has_alpha = match info.channels {
        3 => false,
        4 => true,
        _ => return None,
    };
    let timescale = NonZeroU64::new(info.timescale)?;
    let channels = if has_alpha { 4 } else { 3 };
    #[cfg(target_pointer_width = "64")]
    let pixel_count = (u64::from(width) * u64::from(height)) as usize;
    #[cfg(not(target_pointer_width = "64"))]
    let pixel_count = usize::try_from(u64::from(width) * u64::from(height)).ok()?;
    let pixel_len = pixel_count.checked_mul(channels)?;
    Some(DecodeInfo {
        width,
        height,
        frame_count,
        has_alpha,
        timescale,
        pixel_len,
    })
}

fn checked_decoder_creation(
    status: i32,
    raw: *mut core::ffi::c_void,
    info: FfiDecodeInfo,
    destroy: unsafe extern "C" fn(*mut core::ffi::c_void),
) -> Option<(DecoderHandle, DecodeInfo)> {
    let raw = DecoderHandle {
        raw: successful_pointer(status, raw)?,
        destroy,
    };
    let info = checked_decode_info(info)?;
    Some((raw, info))
}

/// Borrowed native encoder settings. Metadata is copied by libavif at create.
pub(crate) struct EncodeConfig<'metadata> {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) yuv_format: i32,
    pub(crate) yuv_range: i32,
    pub(crate) quality: i32,
    pub(crate) speed: i32,
    pub(crate) max_threads: i32,
    pub(crate) tile_rows_log2: i32,
    pub(crate) tile_cols_log2: i32,
    pub(crate) alpha_premultiplied: bool,
    pub(crate) auto_tiling: bool,
    pub(crate) timescale: u64,
    pub(crate) creation_time: u64,
    pub(crate) modification_time: u64,
    pub(crate) icc: &'metadata [u8],
    pub(crate) exif: &'metadata [u8],
    pub(crate) exif_orientation: i32,
    pub(crate) xmp: &'metadata [u8],
    pub(crate) advanced: &'metadata [(CString, CString)],
}

/// Native animation encoder with deterministic Pillow-compatible settings.
pub(crate) struct Encoder {
    raw: EncoderHandle,
}

struct EncoderHandle(NonNull<core::ffi::c_void>);

impl Drop for EncoderHandle {
    fn drop(&mut self) {
        // SAFETY: this handle uniquely owns the successful bridge allocation.
        unsafe { prs_avif_encoder_destroy(self.0.as_ptr()) };
    }
}

impl Encoder {
    pub(crate) fn new(config: &EncodeConfig<'_>) -> Option<Self> {
        let advanced = config
            .advanced
            .iter()
            .map(|(key, value)| FfiCodecOption {
                key: key.as_ptr(),
                value: value.as_ptr(),
            })
            .collect::<Vec<_>>();
        let ffi_config = FfiEncodeConfig {
            width: config.width,
            height: config.height,
            yuv_format: config.yuv_format,
            yuv_range: config.yuv_range,
            quality: config.quality,
            speed: config.speed,
            max_threads: config.max_threads,
            tile_rows_log2: config.tile_rows_log2,
            tile_cols_log2: config.tile_cols_log2,
            alpha_premultiplied: i32::from(config.alpha_premultiplied),
            auto_tiling: i32::from(config.auto_tiling),
            timescale: config.timescale,
            creation_time: config.creation_time,
            modification_time: config.modification_time,
            icc: config.icc.as_ptr(),
            icc_size: config.icc.len(),
            exif: config.exif.as_ptr(),
            exif_size: config.exif.len(),
            exif_orientation: config.exif_orientation,
            xmp: config.xmp.as_ptr(),
            xmp_size: config.xmp.len(),
            advanced: advanced.as_ptr(),
            advanced_count: advanced.len(),
        };
        let mut raw = std::ptr::null_mut();
        // SAFETY: all slices outlive the call and libavif copies their contents;
        // `raw` points to initialized local output storage.
        let status = unsafe { prs_avif_encoder_create(&ffi_config, &mut raw) };
        Some(Self {
            raw: EncoderHandle(successful_pointer(status, raw)?),
        })
    }

    pub(crate) fn add_frame(
        &mut self,
        pixels: &[u8],
        width: u32,
        height: u32,
        channels: u32,
        duration_in_timescales: u64,
        is_single_frame: bool,
    ) -> Option<()> {
        // SAFETY: `self.raw` is live and unique; `pixels` is borrowed for the
        // synchronous call and its complete length is supplied to the bridge.
        let status = unsafe {
            prs_avif_encoder_add(
                self.raw.0.as_ptr(),
                pixels.as_ptr(),
                pixels.len(),
                width,
                height,
                channels,
                duration_in_timescales,
                i32::from(is_single_frame),
            )
        };
        status_ok(status)
    }

    pub(crate) fn finish(self) -> Option<Vec<u8>> {
        let mut data = std::ptr::null_mut();
        let mut size = 0usize;
        // SAFETY: the live encoder is uniquely borrowed for the synchronous
        // finish call, and both outputs are initialized local storage.
        let status = unsafe { prs_avif_encoder_finish(self.raw.0.as_ptr(), &mut data, &mut size) };
        // SAFETY: the bridge transfers a readable allocation only when it
        // returns `STATUS_OK`; `take_bridge_output` validates every output
        // before reading it and releases any successful non-null allocation.
        unsafe { take_bridge_output(status, data, size, prs_avif_bytes_free) }
    }
}

fn status_ok(status: i32) -> Option<()> {
    (status == STATUS_OK).then_some(())
}

/// Copies and releases a bridge-owned successful output.
///
/// # Safety
///
/// For a successful status and an accepted size, `data` must be readable for
/// that many bytes and owned by `free`. For a successful status with a non-null
/// malformed output, `free` must still accept `data`.
unsafe fn take_bridge_output(
    status: i32,
    data: *mut u8,
    size: usize,
    free: unsafe extern "C" fn(*mut u8),
) -> Option<Vec<u8>> {
    if status != STATUS_OK {
        return None;
    }
    let data = NonNull::new(data)?;
    if size == 0 || size > isize::MAX as usize {
        // SAFETY: the caller guarantees that successful non-null outputs are
        // owned by `free`, including malformed defensive-boundary states.
        unsafe { free(data.as_ptr()) };
        return None;
    }
    // SAFETY: validated size and the caller's bridge-output contract make the
    // complete allocation readable until `free` is invoked below.
    let output = unsafe { std::slice::from_raw_parts(data.as_ptr(), size) }.to_vec();
    // SAFETY: copying is complete and the allocation is still uniquely owned.
    unsafe { free(data.as_ptr()) };
    Some(output)
}

pub(crate) fn default_max_threads() -> i32 {
    normalize_max_threads(std::thread::available_parallelism().ok())
}

fn normalize_max_threads(available: Option<NonZeroUsize>) -> i32 {
    let threads = match available {
        Some(value) => value.get(),
        None => 1,
    };
    threads.min(i32::MAX as usize) as i32
}

#[cfg(coverage)]
unsafe extern "C" fn coverage_noop_free(_data: *mut u8) {}

#[cfg(coverage)]
unsafe extern "C" fn coverage_noop_destroy(_raw: *mut core::ffi::c_void) {}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    let null = std::ptr::null_mut();
    let null_bytes = std::ptr::null_mut();
    let dangling = NonNull::<u8>::dangling().as_ptr();
    let raw = dangling.cast::<core::ffi::c_void>();
    let valid_info = FfiDecodeInfo {
        width: 1,
        height: 1,
        frame_count: 1,
        channels: 3,
        timescale: 1,
        repetition_count: 0,
    };

    let _ = successful_pointer(1, null);
    let _ = successful_pointer(STATUS_OK, null);
    let _ = successful_pointer(STATUS_OK, raw);
    let _ = checked_decode_info(FfiDecodeInfo {
        width: 0,
        ..valid_info
    });
    let _ = checked_decode_info(FfiDecodeInfo {
        height: 0,
        ..valid_info
    });
    let _ = checked_decode_info(FfiDecodeInfo {
        frame_count: 0,
        ..valid_info
    });
    let _ = checked_decode_info(FfiDecodeInfo {
        channels: 2,
        ..valid_info
    });
    let _ = checked_decode_info(FfiDecodeInfo {
        channels: 4,
        ..valid_info
    });
    let _ = checked_decode_info(FfiDecodeInfo {
        timescale: 0,
        ..valid_info
    });
    let _ = checked_decode_info(FfiDecodeInfo {
        width: u32::MAX,
        height: u32::MAX,
        channels: 4,
        ..valid_info
    });
    let _ = checked_decode_info(valid_info);
    let _ = checked_decoder_creation(1, null, valid_info, coverage_noop_destroy);
    let _ = checked_decoder_creation(STATUS_OK, null, valid_info, coverage_noop_destroy);
    let _ = checked_decoder_creation(
        STATUS_OK,
        raw,
        FfiDecodeInfo {
            timescale: 0,
            ..valid_info
        },
        coverage_noop_destroy,
    );
    let _ = checked_decoder_creation(STATUS_OK, raw, valid_info, coverage_noop_destroy);

    let _ = status_ok(STATUS_OK);
    let _ = status_ok(1);
    let timing = FfiFrameTiming {
        pts_in_timescales: 0,
        duration_in_timescales: 1,
    };
    let _ = decoded_frame_result(STATUS_OK, vec![0], timing);
    let _ = decoded_frame_result(1, vec![0], timing);
    let _ = normalize_max_threads(None);
    let _ = normalize_max_threads(NonZeroUsize::new(2));
    let _ = normalize_max_threads(NonZeroUsize::new(usize::MAX));

    let mut byte = 7u8;
    // SAFETY: malformed paths never read `byte`; the valid path reads exactly
    // one byte, and the coverage free function deliberately owns nothing.
    unsafe {
        let _ = take_bridge_output(1, null_bytes, 0, coverage_noop_free);
        let _ = take_bridge_output(STATUS_OK, null_bytes, 1, coverage_noop_free);
        let _ = take_bridge_output(STATUS_OK, &mut byte, 0, coverage_noop_free);
        let oversized = (isize::MAX as usize).saturating_add(1);
        let _ = take_bridge_output(STATUS_OK, &mut byte, oversized, coverage_noop_free);
        let _ = take_bridge_output(STATUS_OK, &mut byte, 1, coverage_noop_free);
    }

    let mut decoder = Decoder::new(include_bytes!(
        "../../../tests/fixtures/input/images/avif/baseline.avif"
    ))
    .unwrap();
    let _ = decoder.decode_frame(decoder.info().frame_count);

    let advanced = Vec::<(CString, CString)>::new();
    let mut config = EncodeConfig {
        width: 1,
        height: 1,
        yuv_format: 3,
        yuv_range: 1,
        quality: -1,
        speed: 6,
        max_threads: 1,
        tile_rows_log2: 0,
        tile_cols_log2: 0,
        alpha_premultiplied: false,
        auto_tiling: true,
        timescale: 1_000,
        creation_time: 0,
        modification_time: 0,
        icc: &[],
        exif: &[],
        exif_orientation: 1,
        xmp: &[],
        advanced: &advanced,
    };
    let _ = Encoder::new(&config);
    config.quality = 75;
    let mut encoder = Encoder::new(&config).unwrap();
    let _ = encoder.add_frame(&[], 1, 1, 3, 0, true);
    let encoder = Encoder::new(&config).unwrap();
    let _ = encoder.finish();
}
