//! AV1 sequence-header syntax needed by the later frame decoder.

use super::bit_reader::{BitReader, SegmentedData};
use super::{Av1Result, malformed};

#[derive(Clone, PartialEq, Eq)]
pub(super) struct Timing {
    pub(super) num_units_in_tick: u32,
    pub(super) time_scale: u32,
    pub(super) equal_picture_interval: bool,
    pub(super) num_ticks_per_picture: Option<u32>,
    pub(super) num_units_in_decoding_tick: Option<u32>,
    pub(super) buffer_removal_delay_length: Option<u32>,
    pub(super) frame_presentation_delay_length: Option<u32>,
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct DecoderParameters {
    pub(super) decoder_buffer_delay: u32,
    pub(super) encoder_buffer_delay: u32,
    pub(super) low_delay_mode: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct OperatingPoint {
    pub(super) idc: u32,
    pub(super) level: u32,
    pub(super) tier: u32,
    pub(super) decoder_parameters: Option<DecoderParameters>,
    pub(super) display_model_present: bool,
    pub(super) initial_display_delay: u32,
}

/// Complete sequence state consumed by the future frame-header parser.
#[derive(Clone, PartialEq, Eq)]
pub(super) struct SequenceHeader {
    pub(super) profile: u32,
    pub(super) still_picture: bool,
    pub(super) reduced_still_picture_header: bool,
    pub(super) timing: Option<Timing>,
    pub(super) decoder_model_present: bool,
    pub(super) display_model_present: bool,
    pub(super) operating_points: Vec<OperatingPoint>,
    pub(super) width_bits: u32,
    pub(super) height_bits: u32,
    pub(super) max_width: u32,
    pub(super) max_height: u32,
    pub(super) frame_id_numbers_present: bool,
    pub(super) delta_frame_id_bits: u32,
    pub(super) frame_id_bits: u32,
    pub(super) use_128x128_superblock: bool,
    pub(super) enable_filter_intra: bool,
    pub(super) enable_intra_edge_filter: bool,
    pub(super) enable_interintra_compound: bool,
    pub(super) enable_masked_compound: bool,
    pub(super) enable_warped_motion: bool,
    pub(super) enable_dual_filter: bool,
    pub(super) enable_order_hint: bool,
    pub(super) enable_jnt_comp: bool,
    pub(super) enable_ref_frame_mvs: bool,
    pub(super) screen_content_tools: u32,
    pub(super) force_integer_mv: u32,
    pub(super) order_hint_bits: u32,
    pub(super) enable_superres: bool,
    pub(super) enable_cdef: bool,
    pub(super) enable_restoration: bool,
    pub(super) bit_depth: u32,
    pub(super) monochrome: bool,
    pub(super) color_primaries: u32,
    pub(super) transfer_characteristics: u32,
    pub(super) matrix_coefficients: u32,
    pub(super) color_range: bool,
    pub(super) subsampling_x: bool,
    pub(super) subsampling_y: bool,
    pub(super) chroma_sample_position: u32,
    pub(super) separate_uv_delta_q: bool,
    pub(super) film_grain_present: bool,
}

impl SequenceHeader {
    /// Compare repeated headers while excluding the operating-parameter values
    /// that AV1 section 7.5 explicitly permits to change.
    pub(super) fn consistent_with(&self, other: &Self) -> bool {
        let mut first = self.clone();
        let mut second = other.clone();
        for point in &mut first.operating_points {
            if let Some(parameters) = &mut point.decoder_parameters {
                *parameters = DecoderParameters {
                    decoder_buffer_delay: 0,
                    encoder_buffer_delay: 0,
                    low_delay_mode: false,
                };
            }
        }
        for point in &mut second.operating_points {
            if let Some(parameters) = &mut point.decoder_parameters {
                *parameters = DecoderParameters {
                    decoder_buffer_delay: 0,
                    encoder_buffer_delay: 0,
                    low_delay_mode: false,
                };
            }
        }
        first == second
    }

    /// Compare sequence declarations with the four-byte AV1 configuration
    /// record retained by the AVIF container.
    pub(super) fn matches_config(&self, bytes: &[u8]) -> bool {
        if bytes.len() < 4 || bytes[0] != 0x81 {
            return false;
        }
        let first_operating_point = self.operating_points.first();
        let profile = u32::from(bytes[1] >> 5);
        let level = u32::from(bytes[1] & 0x1f);
        let tier = u32::from(bytes[2] >> 7);
        let high_bitdepth = bytes[2] & 0x40 != 0;
        let twelve_bit = bytes[2] & 0x20 != 0;
        let bit_depth = if twelve_bit {
            12
        } else if high_bitdepth {
            10
        } else {
            8
        };
        profile == self.profile
            && first_operating_point.is_some_and(|point| point.level == level && point.tier == tier)
            && bit_depth == self.bit_depth
            && (bytes[2] & 0x10 != 0) == self.monochrome
            && (bytes[2] & 0x08 != 0) == self.subsampling_x
            && (bytes[2] & 0x04 != 0) == self.subsampling_y
            && u32::from(bytes[2] & 3) == self.chroma_sample_position
    }
}

// ✅ VERIFIED: AV1 specification sections 5.5 and 6.4; dav1d 1.5.3
// src/obu.c:72-299; libaom 3.13.2 av1/decoder/obu.c:104-275 and
// av1/decoder/decodeframe.c:4216-4298.
pub(super) fn parse(
    data: &SegmentedData<'_, '_>,
    start: usize,
    end: usize,
) -> Av1Result<SequenceHeader> {
    let mut bits = BitReader::new(data, start, end)?;
    parse_bits(&mut bits)
}

fn parse_bits(bits: &mut BitReader<'_, '_, '_>) -> Av1Result<SequenceHeader> {
    let profile = bits.bits(3)?;
    if profile > 2 {
        return Err(malformed("sequence profile exceeds 2"));
    }
    let still_picture = bits.bit()?;
    let reduced_still_picture_header = bits.bit()?;
    if reduced_still_picture_header && !still_picture {
        return Err(malformed(
            "reduced still-picture header is set on a non-still sequence",
        ));
    }

    let mut timing = None;
    let mut decoder_model_present = false;
    let mut decoder_delay_length = 0;
    let mut display_model_present = false;
    let mut operating_points = Vec::new();
    if reduced_still_picture_header {
        operating_points.push(OperatingPoint {
            idc: 0,
            level: bits.bits(5)?,
            tier: 0,
            decoder_parameters: None,
            display_model_present: false,
            initial_display_delay: 10,
        });
    } else {
        if bits.bit()? {
            let num_units_in_tick = bits.bits(32)?;
            let time_scale = bits.bits(32)?;
            if num_units_in_tick == 0 || time_scale == 0 {
                return Err(malformed("sequence timing contains a zero rate"));
            }
            let equal_picture_interval = bits.bit()?;
            let num_ticks_per_picture = if equal_picture_interval {
                let ticks_minus_one = bits.uvlc()?;
                Some(ticks_minus_one.saturating_add(1))
            } else {
                None
            };
            decoder_model_present = bits.bit()?;
            let (
                num_units_in_decoding_tick,
                buffer_removal_delay_length,
                frame_presentation_delay_length,
            ) = if decoder_model_present {
                decoder_delay_length = bits.bits(5)?.saturating_add(1);
                let decoding_tick = bits.bits(32)?;
                if decoding_tick == 0 {
                    return Err(malformed("decoder-model timing tick is zero"));
                }
                (
                    Some(decoding_tick),
                    Some(bits.bits(5)?.saturating_add(1)),
                    Some(bits.bits(5)?.saturating_add(1)),
                )
            } else {
                (None, None, None)
            };
            timing = Some(Timing {
                num_units_in_tick,
                time_scale,
                equal_picture_interval,
                num_ticks_per_picture,
                num_units_in_decoding_tick,
                buffer_removal_delay_length,
                frame_presentation_delay_length,
            });
        }
        display_model_present = bits.bit()?;
        let operating_point_count = bits.bits(5)?.saturating_add(1);
        operating_points.reserve(operating_point_count as usize);
        for _ in 0..operating_point_count {
            let idc = bits.bits(12)?;
            if idc != 0 && (idc & 0xff == 0 || idc & 0xf00 == 0) {
                return Err(malformed("operating-point IDC is inconsistent"));
            }
            let level = bits.bits(5)?;
            let tier = if level > 7 { bits.bits(1)? } else { 0 };
            let decoder_parameters = if decoder_model_present && bits.bit()? {
                Some(DecoderParameters {
                    decoder_buffer_delay: bits.bits(decoder_delay_length)?,
                    encoder_buffer_delay: bits.bits(decoder_delay_length)?,
                    low_delay_mode: bits.bit()?,
                })
            } else {
                None
            };
            let point_display_model_present = display_model_present && bits.bit()?;
            let initial_display_delay = if point_display_model_present {
                let delay = bits.bits(4)?.saturating_add(1);
                if delay > 10 {
                    return Err(malformed("initial display delay exceeds 10"));
                }
                delay
            } else {
                10
            };
            operating_points.push(OperatingPoint {
                idc,
                level,
                tier,
                decoder_parameters,
                display_model_present: point_display_model_present,
                initial_display_delay,
            });
        }
    }

    let width_bits = bits.bits(4)?.saturating_add(1);
    let height_bits = bits.bits(4)?.saturating_add(1);
    let max_width = bits.bits(width_bits)?.saturating_add(1);
    let max_height = bits.bits(height_bits)?.saturating_add(1);
    let frame_id_numbers_present = !reduced_still_picture_header && bits.bit()?;
    let (delta_frame_id_bits, frame_id_bits) = if frame_id_numbers_present {
        let delta = bits.bits(4)?.saturating_add(2);
        let frame = bits.bits(3)?.saturating_add(delta).saturating_add(1);
        if frame > 16 {
            return Err(malformed("frame ID width exceeds 16 bits"));
        }
        (delta, frame)
    } else {
        (0, 0)
    };

    let use_128x128_superblock = bits.bit()?;
    let enable_filter_intra = bits.bit()?;
    let enable_intra_edge_filter = bits.bit()?;
    let (
        enable_interintra_compound,
        enable_masked_compound,
        enable_warped_motion,
        enable_dual_filter,
        enable_order_hint,
        enable_jnt_comp,
        enable_ref_frame_mvs,
        screen_content_tools,
        force_integer_mv,
        order_hint_bits,
    ) = if reduced_still_picture_header {
        (false, false, false, false, false, false, false, 2, 2, 0)
    } else {
        let interintra = bits.bit()?;
        let masked = bits.bit()?;
        let warped = bits.bit()?;
        let dual = bits.bit()?;
        let order_hint = bits.bit()?;
        let jnt_comp = order_hint && bits.bit()?;
        let ref_frame_mvs = order_hint && bits.bit()?;
        let screen_tools = if bits.bit()? {
            2
        } else {
            u32::from(bits.bit()?)
        };
        let integer_mv = if screen_tools != 0 {
            if bits.bit()? {
                2
            } else {
                u32::from(bits.bit()?)
            }
        } else {
            2
        };
        let order_bits = if order_hint {
            bits.bits(3)?.saturating_add(1)
        } else {
            0
        };
        (
            interintra,
            masked,
            warped,
            dual,
            order_hint,
            jnt_comp,
            ref_frame_mvs,
            screen_tools,
            integer_mv,
            order_bits,
        )
    };

    let enable_superres = bits.bit()?;
    let enable_cdef = bits.bit()?;
    let enable_restoration = bits.bit()?;
    let high_bitdepth = bits.bit()?;
    let twelve_bit = profile == 2 && high_bitdepth && bits.bit()?;
    let bit_depth = if twelve_bit {
        12
    } else if high_bitdepth {
        10
    } else {
        8
    };
    let monochrome = profile != 1 && bits.bit()?;
    let color_description_present = bits.bit()?;
    let (color_primaries, transfer_characteristics, matrix_coefficients) =
        if color_description_present {
            (bits.bits(8)?, bits.bits(8)?, bits.bits(8)?)
        } else {
            (2, 2, 2)
        };
    let (color_range, subsampling_x, subsampling_y, chroma_sample_position) = if monochrome {
        (bits.bit()?, true, true, 0)
    } else if color_primaries == 1 && transfer_characteristics == 13 && matrix_coefficients == 0 {
        if profile != 1 && !(profile == 2 && bit_depth == 12) {
            return Err(malformed(
                "identity color matrix is invalid for this profile and bit depth",
            ));
        }
        (true, false, false, 0)
    } else {
        let range = bits.bit()?;
        let (x, y) = if profile == 0 {
            (true, true)
        } else if profile == 1 {
            (false, false)
        } else if bit_depth == 12 {
            let x = bits.bit()?;
            (x, x && bits.bit()?)
        } else {
            (true, false)
        };
        let position = if x && y { bits.bits(2)? } else { 0 };
        (range, x, y, position)
    };
    if matrix_coefficients == 0 && (subsampling_x | subsampling_y) {
        return Err(malformed(
            "identity color matrix cannot use chroma subsampling",
        ));
    }
    let separate_uv_delta_q = !monochrome && bits.bit()?;
    let film_grain_present = bits.bit()?;
    bits.trailing_bits()?;

    Ok(SequenceHeader {
        profile,
        still_picture,
        reduced_still_picture_header,
        timing,
        decoder_model_present,
        display_model_present,
        operating_points,
        width_bits,
        height_bits,
        max_width,
        max_height,
        frame_id_numbers_present,
        delta_frame_id_bits,
        frame_id_bits,
        use_128x128_superblock,
        enable_filter_intra,
        enable_intra_edge_filter,
        enable_interintra_compound,
        enable_masked_compound,
        enable_warped_motion,
        enable_dual_filter,
        enable_order_hint,
        enable_jnt_comp,
        enable_ref_frame_mvs,
        screen_content_tools,
        force_integer_mv,
        order_hint_bits,
        enable_superres,
        enable_cdef,
        enable_restoration,
        bit_depth,
        monochrome,
        color_primaries,
        transfer_characteristics,
        matrix_coefficients,
        color_range,
        subsampling_x,
        subsampling_y,
        chroma_sample_position,
        separate_uv_delta_q,
        film_grain_present,
    })
}
