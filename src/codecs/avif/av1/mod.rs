//! Private, zero-copy AV1 syntax needed by the portable AVIF decoder.

mod bit_reader;
mod block;
mod entropy;
mod frame;
mod sequence;

use self::bit_reader::SegmentedData;
use self::frame::FrameState;
#[cfg(coverage)]
use super::samples::ByteSpan;
use super::samples::{EncodedPlane, EncodedSample, ExtractedAvif};
#[cfg(coverage)]
use super::samples::{SequencePayload, StillPayload};
use crate::codecs::{CodecError, CodecResult};
#[cfg(coverage)]
use std::num::NonZeroU32;

const MAX_OBUS_PER_SAMPLE: usize = 4_096;

type Av1Result<T> = CodecResult<T>;

fn malformed(stage: &'static str) -> CodecError {
    CodecError::Malformed(format!("invalid AV1 bitstream: {stage}"))
}

/// One complete still-image class that the portable decoder can materialize.
#[derive(Clone)]
pub(super) struct PortableStill {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) bit_depth: u32,
    pub(super) monochrome: bool,
    pub(super) color_primaries: u32,
    pub(super) transfer_characteristics: u32,
    pub(super) matrix_coefficients: u32,
    pub(super) color_range: bool,
    pub(super) subsampling_x: bool,
    pub(super) subsampling_y: bool,
    pub(super) planes: [block::ReconstructedPlane; 3],
    #[cfg(coverage)]
    pub(super) entropy_operations: Vec<crate::Av1EntropyOperationState>,
}

/// AV1 syntax accepted by the production parser, retaining a complete
/// portable still only when container and codec state prove that class.
pub(super) struct ValidatedAv1 {
    pub(super) portable_still: Option<PortableStill>,
}

struct ValidatedPlane {
    first_leaf: Option<block::FirstLeaf>,
    sequence: sequence::SequenceHeader,
}

// ✅ VERIFIED: AV1 specification sections 5.3.2-5.3.3; dav1d 1.5.3
// src/getbits.c:95-112 and src/obu.c:1169-1195.
fn read_uleb128(data: &SegmentedData<'_, '_>, offset: &mut usize) -> Av1Result<u32> {
    let mut value = 0_u64;
    for index in 0..8_u32 {
        let byte = data.byte(*offset)?;
        *offset = offset.saturating_add(1);
        value |= u64::from(byte & 0x7f) << index.saturating_mul(7);
        if byte & 0x80 == 0 {
            return u32::try_from(value).map_err(|_| malformed("ULEB128 value exceeds u32"));
        }
    }
    Err(malformed("ULEB128 value exceeds eight bytes"))
}

// ✅ VERIFIED: AV1 specification sections 5.3.1-5.3.3 and 6.2.2; dav1d
// 1.5.3 src/obu.c:1169-1209.
fn validate_sample(input: &[u8], sample: &EncodedSample, state: &mut FrameState) -> Av1Result<()> {
    let data = SegmentedData::new(input, &sample.spans)?;
    // The AVIF sample extractor constructs codec-configuration spans only
    // after validating them against the immutable input buffer.
    let config = &input[sample.config.start..sample.config.end];
    let mut offset = 0_usize;
    let mut obu_count = 0_usize;
    let mut frame_bearing = false;
    while offset < data.len() {
        obu_count = obu_count.saturating_add(1);
        if obu_count > MAX_OBUS_PER_SAMPLE {
            return Err(malformed("sample contains too many OBUs"));
        }
        // The loop condition proves the logical OBU header byte is present.
        let header = data.validated_byte(offset);
        offset = offset.saturating_add(1);
        if header & 0x80 != 0 || header & 1 != 0 {
            return Err(malformed("OBU header reserved bits are set"));
        }
        let obu_type = (header >> 3) & 0x0f;
        let has_extension = header & 4 != 0;
        let has_size_field = header & 2 != 0;
        let mut temporal_id = 0_u32;
        let mut spatial_id = 0_u32;
        if has_extension {
            let extension = data.byte(offset)?;
            offset = offset.saturating_add(1);
            if extension & 7 != 0 {
                return Err(malformed("OBU extension reserved bits are set"));
            }
            temporal_id = u32::from(extension >> 5);
            spatial_id = u32::from((extension >> 3) & 3);
        }
        if !has_size_field {
            return Err(malformed("OBU omits its payload-size field"));
        }
        let payload_size = read_uleb128(&data, &mut offset)? as usize;
        let payload_start = offset;
        let remaining = data.len().saturating_sub(payload_start);
        if payload_size > remaining {
            return Err(malformed("OBU payload exceeds its sample"));
        }
        let payload_end = payload_start.saturating_add(payload_size);
        match obu_type {
            1 => {
                let sequence = sequence::parse(&data, payload_start, payload_end)?;
                if !sequence.matches_config(config) {
                    return Err(malformed(
                        "sequence header disagrees with the AV1 codec configuration",
                    ));
                }
                state.accept_sequence(sequence)?;
            }
            2 => state.temporal_delimiter()?,
            3 => {
                state.frame_header_obu(
                    &data,
                    payload_start,
                    payload_end,
                    temporal_id,
                    spatial_id,
                    false,
                )?;
                frame_bearing = true;
            }
            4 => {
                state.tile_group_obu(&data, payload_start, payload_end)?;
                frame_bearing = true;
            }
            6 => {
                state.frame_obu(&data, payload_start, payload_end, temporal_id, spatial_id)?;
                frame_bearing = true;
            }
            7 => {
                state.frame_header_obu(
                    &data,
                    payload_start,
                    payload_end,
                    temporal_id,
                    spatial_id,
                    true,
                )?;
                frame_bearing = true;
            }
            _ => {}
        }
        offset = payload_end;
    }
    if !frame_bearing {
        return Err(malformed("sample contains no frame-bearing OBU"));
    }
    Ok(())
}

fn validate_plane(input: &[u8], plane: &EncodedPlane) -> Av1Result<ValidatedPlane> {
    let mut state = FrameState::new();
    for sample in &plane.samples {
        validate_sample(input, sample, &mut state)?;
    }
    let sequence = state.finish()?.clone();
    Ok(ValidatedPlane {
        first_leaf: state.first_leaf().cloned(),
        sequence,
    })
}

fn portable_still(leaf: block::FirstLeaf, sequence: sequence::SequenceHeader) -> PortableStill {
    PortableStill {
        width: leaf.width,
        height: leaf.height,
        bit_depth: sequence.bit_depth,
        monochrome: sequence.monochrome,
        color_primaries: sequence.color_primaries,
        transfer_characteristics: sequence.transfer_characteristics,
        matrix_coefficients: sequence.matrix_coefficients,
        color_range: sequence.color_range,
        subsampling_x: sequence.subsampling_x,
        subsampling_y: sequence.subsampling_y,
        planes: leaf.planes,
        #[cfg(coverage)]
        entropy_operations: leaf.entropy_operations,
    }
}

fn validate_still(extracted: &ExtractedAvif<'_>) -> Av1Result<Option<PortableStill>> {
    let mut portable = None;
    if let Some(still) = &extracted.still {
        let color = validate_plane(extracted.input, &still.color)?;
        if let Some(alpha) = &still.alpha {
            validate_plane(extracted.input, alpha)?;
        }
        portable = match (
            &extracted.sequence,
            &still.alpha,
            still.color.samples.as_slice(),
            color.first_leaf,
        ) {
            (None, None, [_], Some(leaf)) => Some(portable_still(leaf, color.sequence)),
            _ => None,
        };
    }
    Ok(portable)
}

pub(super) fn validate_first(extracted: &ExtractedAvif<'_>) -> Av1Result<ValidatedAv1> {
    let portable_still = validate_still(extracted)?;
    Ok(ValidatedAv1 { portable_still })
}

pub(super) fn validate(extracted: &ExtractedAvif<'_>) -> Av1Result<ValidatedAv1> {
    let portable_still = validate_still(extracted)?;
    if let Some(sequence) = &extracted.sequence {
        validate_plane(extracted.input, &sequence.color)?;
        if let Some(alpha) = &sequence.alpha {
            validate_plane(extracted.input, alpha)?;
        }
    }
    Ok(ValidatedAv1 { portable_still })
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_sample(sample: &[u8], config: [u8; 4]) -> (Vec<u8>, EncodedSample) {
    let mut input = sample.to_vec();
    let config_start = input.len();
    input.extend_from_slice(&config);
    let input_length = input.len();
    (
        input,
        EncodedSample {
            spans: vec![ByteSpan {
                start: 0,
                end: config_start,
            }],
            config: ByteSpan {
                start: config_start,
                end: input_length,
            },
            sync: true,
            duration: 1,
        },
    )
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_track_prefix(
    samples: &[&[u8]],
    target: usize,
    replacement: &[u8],
    config: [u8; 4],
) -> Av1Result<()> {
    let mut state = FrameState::new();
    for (index, sample) in samples.iter().enumerate().take(target.saturating_add(1)) {
        let bytes = if index == target { replacement } else { sample };
        let (input, encoded) = coverage_sample(bytes, config);
        validate_sample(&input, &encoded, &mut state)?;
    }
    Ok(())
}

#[cfg(coverage)]
#[coverage(off)]
fn coverage_sweep_track(samples: &[&[u8]], config: [u8; 4]) {
    for (target, sample) in samples.iter().enumerate() {
        assert!(coverage_track_prefix(samples, target, sample, config).is_ok());
        for end in 0..sample.len() {
            let _ = coverage_track_prefix(samples, target, &sample[..end], config);
        }
        for index in 0..sample.len() {
            for replacement in [0, 1, 0x55, 0x7f, 0x80, 0xaa, 0xff] {
                if sample[index] == replacement {
                    continue;
                }
                let mut mutated = sample.to_vec();
                mutated[index] = replacement;
                let _ = coverage_track_prefix(samples, target, &mutated, config);
            }
        }
    }
}

#[cfg(coverage)]
#[coverage(off)]
pub(crate) fn __coverage_exercise_private_branches() {
    bit_reader::__coverage_exercise_private_branches();
    block::__coverage_exercise_private_branches();
    entropy::__coverage_exercise_private_branches();
    frame::__coverage_exercise_private_branches();
    sequence::__coverage_exercise_private_branches();

    let valid = b"\x12\x00\x0a\x0a\x40\x00\x00\x02\xaf\xff\xbf\xff\x3e\xa0\x32\x0d\x10\x00\x93\x80\x00\x08\x00\x00\x01\x48\x1a\x7a\xa0";
    let valid_config = [0x81, 0x40, 0x7c, 0];
    let (input, sample) = coverage_sample(valid, valid_config);
    assert_eq!(
        validate_sample(&input, &sample, &mut FrameState::new()),
        Ok(())
    );
    let header = frame::__coverage_reduced_header_payload();
    let mut split = b"\x12\x00\x0a\x0a\x40\x00\x00\x02\xaf\xff\xbf\xff\x3e\xa0".to_vec();
    split.extend_from_slice(&[0x1a, u8::try_from(header.len()).unwrap()]);
    split.extend_from_slice(&header);
    split.extend_from_slice(&[0x22, 0]);
    let (input, sample) = coverage_sample(&split, valid_config);
    assert_eq!(
        validate_sample(&input, &sample, &mut FrameState::new()),
        Ok(())
    );
    let mut pending_then_delimiter = split[..split.len() - 2].to_vec();
    pending_then_delimiter.extend_from_slice(&[0x12, 0]);
    let (input, sample) = coverage_sample(&pending_then_delimiter, valid_config);
    assert!(validate_sample(&input, &sample, &mut FrameState::new()).is_err());
    let mut redundant = b"\x12\x00\x0a\x0a\x40\x00\x00\x02\xaf\xff\xbf\xff\x3e\xa0".to_vec();
    for obu_type in [0x1a, 0x3a] {
        redundant.extend_from_slice(&[obu_type, u8::try_from(header.len()).unwrap()]);
        redundant.extend_from_slice(&header);
    }
    redundant.extend_from_slice(&[0x22, 0]);
    let (input, sample) = coverage_sample(&redundant, valid_config);
    assert_eq!(
        validate_sample(&input, &sample, &mut FrameState::new()),
        Ok(())
    );
    let invalid_span = EncodedSample {
        spans: vec![ByteSpan { start: 0, end: 1 }],
        config: ByteSpan { start: 0, end: 0 },
        sync: true,
        duration: 1,
    };
    assert!(validate_sample(&[], &invalid_span, &mut FrameState::new()).is_err());
    for end in 0..valid.len() {
        let (input, sample) = coverage_sample(&valid[..end], valid_config);
        let _ = validate_sample(&input, &sample, &mut FrameState::new());
    }
    for index in 0..valid.len() {
        for replacement in 0..=u8::MAX {
            if valid[index] == replacement {
                continue;
            }
            let mut mutated = valid.to_vec();
            mutated[index] = replacement;
            let (input, sample) = coverage_sample(&mutated, valid_config);
            let _ = validate_sample(&input, &sample, &mut FrameState::new());
        }
    }

    let animated: &[&[u8]] = &[
        b"\x12\x00\x0a\x0e\x00\x00\x00\x03\xbc\xac\xa9\xb5\xf2\x20\x21\xa0\xd0\x80\x32\x13\x10\x00\x83\x80\x00\x00\x80\x00\x00\x00\xeb\xc5\xa6\x2e\x0c\x0d\xd1\x51\x40",
        b"\x12\x00\x32\x23\x28\x04\xe0\x40\x00\x00\x23\x43\x30\x00\x00\x40\x00\x04\x00\x00\x08\xe4\x66\x90\x91\x47\x7f\x6e\xcc\x05\x23\x9b\xc1\x1c\xc6\x74\xcb\x7e\xe0\x32\x23\x28\x02\xe0\x80\x00\x00\xa3\x44\xc0\x00\x00\x48\x00\x04\x00\x00\x26\x66\xc9\x49\xed\xf9\xfc\xed\x11\x20\x54\x85\xcf\x5f\x49\x98\x10\x5b\x20\x32\x23\x30\x03\xc2\x00\x00\x81\x46\x8c\x80\x00\x00\x90\x00\x08\x00\x1f\x3a\xcd\xf2\xb3\x29\xa3\x70\xb6\x44\xb1\xd9\x5a\x93\x1f\x3c\x56\x60\x14\xc4",
        b"\x12\x00\x1a\x01\xa8",
        b"\x12\x00\x32\x1a\x30\x06\x44\x09\x80\x01\x46\x8c\x80\x00\x00\x90\x00\x08\x00\x33\xa1\xc0\x60\x46\x86\x20\x7d\xcf\xf4\xfc",
        b"\x12\x00\x32\x15\x30\x08\x00\x11\x30\x01\x46\x8c\x80\x00\x00\x90\x00\x08\x00\xb3\x2e\xde\x2e\xcf\x20",
    ];
    coverage_sweep_track(animated, [0x81, 0x00, 0x0c, 0]);

    let twelve_bit_alpha: &[&[u8]] = &[
        valid,
        b"\x12\x00\x32\x10\x30\x03\x80\x80\x00\x00\x46\xa7\x80\x00\x09\x00\x08\x00\x9c\x50",
        b"\x12\x00\x0a\x0a\x40\x00\x00\x02\xaf\xff\xbf\xff\x3e\xa0\x32\x0a\x10\x00\xbe\x00\x00\x09\x00\x00\x0e\x36",
        b"\x12\x00\x0a\x0a\x40\x00\x00\x02\xaf\xff\xbf\xff\x3e\xa0\x32\x1c\x10\x00\xbe\x00\x00\x09\x18\x00\x3b\x95\xa6\xa8\x47\x2b\xdf\x67\x4b\xd6\x0e\x45\xbd\xbf\xf5\x1b\x6f\x23\x48\x62",
        b"\x12\x00\x0a\x0a\x40\x00\x00\x02\xaf\xff\xbf\xff\x3e\xa0\x32\x23\x10\x00\xa7\x80\x00\x09\x24\xe0\xff\xfc\xe9\x1e\xd8\x9f\x6e\x05\x5e\x6f\xc7\x36\x3a\x9d\x64\xd6\x35\x31\xf9\xc1\x4d\xfb\x26\x00\xd6\xbc\x5c",
    ];
    coverage_sweep_track(twelve_bit_alpha, [0x81, 0x40, 0x7c, 0]);

    let invalid_streams: &[&[u8]] = &[
        b"\x92\x00",
        b"\x13\x00",
        b"\x10",
        b"\x16",
        b"\x16\x01\x00",
        b"\x12\x80",
        b"\x12\x01",
        b"\x12\x80\x80\x80\x80\x80\x80\x80\x80",
        b"\x32\x00",
        b"\x0a\x09\x18\x19\xbf\xff\x68\x80\x86\x83\x42",
    ];
    for stream in invalid_streams {
        let (input, sample) = coverage_sample(stream, valid_config);
        let _ = validate_sample(&input, &sample, &mut FrameState::new());
    }
    let extension = b"\x16\x00\x00\x0a\x09\x18\x19\xbf\xff\x68\x80\x86\x83\x42\x32\x00";
    let (input, sample) = coverage_sample(extension, valid_config);
    let _ = validate_sample(&input, &sample, &mut FrameState::new());
    let reserved = b"\x4a\x00\x0a\x09\x18\x19\xbf\xff\x68\x80\x86\x83\x42\x32\x00";
    let (input, sample) = coverage_sample(reserved, valid_config);
    let _ = validate_sample(&input, &sample, &mut FrameState::new());
    let (input, sample) = coverage_sample(valid, [0x81, 0x20, 0, 0]);
    let _ = validate_sample(&input, &sample, &mut FrameState::new());

    let many_delimiters = b"\x12\x00".repeat(MAX_OBUS_PER_SAMPLE.saturating_add(1));
    let (input, sample) = coverage_sample(&many_delimiters, [0x81, 0x00, 0x0c, 0]);
    let _ = validate_sample(&input, &sample, &mut FrameState::new());

    let baseline_payload = b"\x18\x19\xbf\xff\x68\x80\x86\x83\x42";
    let animated_payload = b"\x00\x00\x00\x03\xbc\xac\xa9\xb5\xf2\x20\x21\xa0\xd0\x80";
    let baseline_spans = [ByteSpan {
        start: 0,
        end: baseline_payload.len(),
    }];
    let baseline_data = SegmentedData::new(baseline_payload, &baseline_spans).unwrap();
    let baseline_header = sequence::parse(&baseline_data, 0, baseline_payload.len()).unwrap();
    let animated_spans = [ByteSpan {
        start: 0,
        end: animated_payload.len(),
    }];
    let animated_data = SegmentedData::new(animated_payload, &animated_spans).unwrap();
    let animated_header = sequence::parse(&animated_data, 0, animated_payload.len()).unwrap();
    let mut state = FrameState::new();
    assert_eq!(state.accept_sequence(baseline_header.clone()), Ok(()));
    assert_eq!(state.accept_sequence(baseline_header), Ok(()));
    assert!(state.accept_sequence(animated_header.clone()).is_err());

    let (input, sample) = coverage_sample(valid, valid_config);
    let mut inconsistent_state = FrameState::new();
    assert_eq!(inconsistent_state.accept_sequence(animated_header), Ok(()));
    assert!(validate_sample(&input, &sample, &mut inconsistent_state).is_err());

    let _ = validate_plane(
        &[],
        &EncodedPlane {
            samples: Vec::new(),
        },
    );
    let invalid_plane = EncodedPlane {
        samples: vec![EncodedSample {
            spans: Vec::new(),
            config: ByteSpan { start: 0, end: 0 },
            sync: true,
            duration: 1,
        }],
    };
    assert!(validate_plane(&[], &invalid_plane).is_err());
    assert!(
        validate(&ExtractedAvif {
            input: &[],
            still: None,
            sequence: None,
            consumed: 0,
            retained_boxes: Vec::new(),
            metadata: Vec::new(),
            source_color: crate::types::SourceColor::new(),
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
        })
        .is_ok()
    );

    let invalid_plane = || EncodedPlane {
        samples: vec![EncodedSample {
            spans: Vec::new(),
            config: ByteSpan { start: 0, end: 0 },
            sync: true,
            duration: 1,
        }],
    };
    assert!(
        validate(&ExtractedAvif {
            input: &[],
            still: Some(StillPayload {
                color: invalid_plane(),
                alpha: None,
            }),
            sequence: None,
            consumed: 0,
            retained_boxes: Vec::new(),
            metadata: Vec::new(),
            source_color: crate::types::SourceColor::new(),
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
        })
        .is_err()
    );

    let valid_plane = || EncodedPlane {
        samples: vec![EncodedSample {
            spans: vec![ByteSpan {
                start: 0,
                end: valid.len(),
            }],
            config: ByteSpan {
                start: valid.len(),
                end: input.len(),
            },
            sync: true,
            duration: 1,
        }],
    };
    let valid_sample = || EncodedSample {
        spans: vec![ByteSpan {
            start: 0,
            end: valid.len(),
        }],
        config: ByteSpan {
            start: valid.len(),
            end: input.len(),
        },
        sync: true,
        duration: 1,
    };
    let validated_multi_sample = validate(&ExtractedAvif {
        input: &input,
        consumed: 0,
        retained_boxes: Vec::new(),
        metadata: Vec::new(),
        source_color: crate::types::SourceColor::new(),
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
                samples: vec![valid_sample(), valid_sample()],
            },
            alpha: None,
        }),
        sequence: None,
    });
    assert!(validated_multi_sample.is_ok_and(|validated| validated.portable_still.is_none()));
    assert!(
        validate(&ExtractedAvif {
            input: &input,
            still: Some(StillPayload {
                color: valid_plane(),
                alpha: Some(invalid_plane()),
            }),
            sequence: None,
            consumed: 0,
            retained_boxes: Vec::new(),
            metadata: Vec::new(),
            source_color: crate::types::SourceColor::new(),
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
        })
        .is_err()
    );
    assert!(
        validate(&ExtractedAvif {
            input: &[],
            still: None,
            sequence: Some(SequencePayload {
                color: invalid_plane(),
                alpha: None,
                timescale: NonZeroU32::new(1).unwrap(),
            }),
            consumed: 0,
            retained_boxes: Vec::new(),
            metadata: Vec::new(),
            source_color: crate::types::SourceColor::new(),
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
        })
        .is_err()
    );
    assert!(
        validate(&ExtractedAvif {
            input: &input,
            still: None,
            sequence: Some(SequencePayload {
                color: valid_plane(),
                alpha: Some(invalid_plane()),
                timescale: NonZeroU32::new(1).unwrap(),
            }),
            consumed: 0,
            retained_boxes: Vec::new(),
            metadata: Vec::new(),
            source_color: crate::types::SourceColor::new(),
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
        })
        .is_err()
    );
}

#[cfg(coverage)]
pub(crate) fn __coverage_entropy_reference_trace() -> CodecResult<Vec<crate::Av1EntropyTraceState>>
{
    entropy::reference_trace()
}

#[cfg(coverage)]
#[coverage(off)]
pub(crate) fn __coverage_reconstruction(
    input: &[u8],
) -> CodecResult<Option<crate::Av1ReconstructionTrace>> {
    let extracted = super::samples::validated(input)?;
    let validated = validate(&extracted)?;
    let Some(still) = validated.portable_still else {
        return Ok(None);
    };
    Ok(Some(crate::Av1ReconstructionTrace {
        width: still.width,
        height: still.height,
        bit_depth: still.bit_depth,
        monochrome: still.monochrome,
        color_primaries: still.color_primaries,
        transfer_characteristics: still.transfer_characteristics,
        matrix_coefficients: still.matrix_coefficients,
        color_range: still.color_range,
        subsampling_x: still.subsampling_x,
        subsampling_y: still.subsampling_y,
        planes: still.planes.map(|plane| plane.samples),
        entropy_operations: still.entropy_operations,
    }))
}

#[cfg(coverage)]
#[coverage(off)]
pub(super) fn __coverage_portable_still() -> PortableStill {
    PortableStill {
        width: 4,
        height: 4,
        bit_depth: 8,
        monochrome: false,
        color_primaries: 1,
        transfer_characteristics: 13,
        matrix_coefficients: 6,
        color_range: true,
        subsampling_x: false,
        subsampling_y: false,
        planes: std::array::from_fn(|_| block::ReconstructedPlane {
            samples: vec![128; 16],
        }),
        entropy_operations: Vec::new(),
    }
}

#[cfg(coverage)]
#[coverage(off)]
pub(crate) fn __coverage_sweep_first_leaf(input: &[u8]) {
    let extracted = super::samples::validated(input).expect("portable AVIF fixture must extract");
    let spans = extracted
        .still
        .as_ref()
        .and_then(|still| still.color.samples.first())
        .map(|sample| sample.spans.clone())
        .expect("portable AVIF fixture must contain one still sample");
    assert!(validate(&extracted).is_ok());
    drop(extracted);

    let validate_mutation = |mutated: &[u8]| {
        if let Ok(extracted) = super::samples::validated(mutated) {
            let _ = validate(&extracted);
        }
    };
    for span in spans {
        for offset in span.start..span.end {
            for replacement in 0..=u8::MAX {
                if input[offset] == replacement {
                    continue;
                }
                let mut mutated = input.to_vec();
                mutated[offset] = replacement;
                validate_mutation(&mutated);
            }
            for fill in [0, u8::MAX] {
                let mut mutated = input.to_vec();
                mutated[offset..span.end].fill(fill);
                validate_mutation(&mutated);
            }
        }
    }
}
