// Modified Rust port copyright (c) 2026 Appunni M.
// Derived from libjpeg-turbo/IJG sources; see third_party/libjpeg-turbo/.

use super::huffman::HuffTable;
use crate::codecs::{CodecError, CodecResult, OptionCodecExt};
// ── Marker Constants ──────────────────────────────────────────────────────

pub(crate) const M_SOI: u16 = 0xFFD8;
pub(crate) const M_EOI: u16 = 0xFFD9;
pub(crate) const M_SOS: u16 = 0xFFDA;
pub(crate) const M_SOF0: u16 = 0xFFC0;
pub(crate) const M_SOF2: u16 = 0xFFC2;
pub(crate) const M_DHT: u16 = 0xFFC4;
pub(crate) const M_DQT: u16 = 0xFFDB;
pub(crate) const M_DRI: u16 = 0xFFDD;
pub(crate) const M_APP14: u16 = 0xFFEE;

// ── JPEG Structures ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub(super) struct FrameComponent {
    pub(super) id: u8,
    pub(super) h_samp: u8,
    pub(super) v_samp: u8,
    pub(super) quant_tbl: u8,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ScanComponent {
    pub(super) comp_index: usize,
    pub(super) dc_tbl: u8,
    pub(super) ac_tbl: u8,
}

#[derive(Debug, Clone)]
pub(super) struct ScanInfo {
    pub(super) components: Vec<ScanComponent>,
    pub(super) entropy_start: usize,
    pub(super) entropy_end: usize,
    pub(super) ss: u8,
    pub(super) se: u8,
    pub(super) ah: u8,
    pub(super) al: u8,
    pub(super) restart_interval: u16,
    pub(super) dc_huff_tables: Vec<Option<HuffTable>>,
    pub(super) ac_huff_tables: Vec<Option<HuffTable>>,
}

pub(super) struct JpegInfo {
    pub(super) width: u16,
    pub(super) height: u16,
    pub(super) num_components: u8,
    pub(super) components: Vec<FrameComponent>,
    pub(super) quant_tables: Vec<Option<[u16; 64]>>,
    pub(super) dc_huff_tables: Vec<Option<HuffTable>>,
    pub(super) ac_huff_tables: Vec<Option<HuffTable>>,
    pub(super) scan_components: Vec<ScanComponent>,
    pub(super) restart_interval: u16,
    pub(super) entropy_start: usize,
    pub(super) eoi_pos: usize,
    pub(super) max_h_samp: u8,
    pub(super) max_v_samp: u8,
    pub(super) progressive: bool,
    pub(super) scans: Vec<ScanInfo>,
    pub(super) adobe_transform: Option<u8>,
    pub(super) metadata: Vec<crate::types::OpaqueMetadata>,
}

// ── JPEG Parser ───────────────────────────────────────────────────────────

pub(super) fn read_u16(data: &[u8], pos: &mut usize) -> CodecResult<u16> {
    if pos.saturating_add(1) >= data.len() {
        return Err(CodecError::Malformed(
            "truncated JPEG 16-bit field".to_owned(),
        ));
    }
    let val = u16::from_be_bytes([data[*pos], data[pos.saturating_add(1)]]);
    *pos = pos.saturating_add(2);
    Ok(val)
}

pub(super) fn read_u8(data: &[u8], pos: &mut usize) -> CodecResult<u8> {
    if *pos >= data.len() {
        return Err(CodecError::Malformed(
            "truncated JPEG byte field".to_owned(),
        ));
    }
    let val = data[*pos];
    *pos = pos.saturating_add(1);
    Ok(val)
}

pub(super) fn find_next_marker(data: &[u8], pos: &mut usize) -> CodecResult<u16> {
    while *pos < data.len() {
        while *pos < data.len() && data[*pos] != 0xFF {
            *pos = pos.saturating_add(1);
        }
        if *pos >= data.len() {
            return Err(CodecError::Malformed(
                "JPEG marker stream ended unexpectedly".to_owned(),
            ));
        }
        if pos.saturating_add(1) >= data.len() {
            return Err(CodecError::Malformed(
                "truncated JPEG marker code".to_owned(),
            ));
        }
        let marker_byte = data[pos.saturating_add(1)];
        if marker_byte == 0x00 || marker_byte == 0xFF {
            *pos = pos.saturating_add(1);
            continue;
        }
        let marker = 0xFF00u16 | u16::from(marker_byte);
        *pos = pos.saturating_add(2);
        return Ok(marker);
    }
    Err(CodecError::Malformed(
        "JPEG marker stream ended unexpectedly".to_owned(),
    ))
}

pub(super) fn find_entropy_end(data: &[u8], mut pos: usize) -> usize {
    while pos.saturating_add(1) < data.len() {
        if data[pos] == 0xFF {
            let next = data[pos.saturating_add(1)];
            if next == 0x00 || (0xD0..=0xD7).contains(&next) {
                pos = pos.saturating_add(2);
            } else {
                return pos;
            }
        } else {
            pos = pos.saturating_add(1);
        }
    }
    data.len()
}

pub(super) fn find_eoi(data: &[u8], mut pos: usize) -> CodecResult<usize> {
    while pos.saturating_add(1) < data.len() {
        if data[pos] == 0xFF && data[pos.saturating_add(1)] == 0xD9 {
            return Ok(pos);
        }
        pos = pos.saturating_add(1);
    }
    Err(CodecError::Malformed(
        "JPEG entropy stream has no EOI marker".to_owned(),
    ))
}

pub(super) fn parse_sof0(
    data: &[u8],
    pos: &mut usize,
) -> CodecResult<(u16, u16, Vec<FrameComponent>, u8, u8)> {
    let _length = read_u16(data, pos)?;
    let precision = read_u8(data, pos)?;
    if precision != 8 {
        return Err(CodecError::Malformed(
            "unsupported JPEG sample precision".to_owned(),
        ));
    }
    let height = read_u16(data, pos)?;
    let width = read_u16(data, pos)?;
    if width == 0 || height == 0 {
        return Err(CodecError::Malformed(
            "JPEG frame dimensions must be nonzero".to_owned(),
        ));
    }
    let num_components = read_u8(data, pos)?;
    if num_components != 1 && num_components != 3 && num_components != 4 {
        return Err(CodecError::Malformed(
            "unsupported JPEG component count".to_owned(),
        ));
    }

    let mut components = Vec::with_capacity(num_components as usize);
    let mut max_h_samp = 0u8;
    let mut max_v_samp = 0u8;

    for _ in 0..num_components {
        let id = read_u8(data, pos)?;
        let sampling = read_u8(data, pos)?;
        let h_samp = sampling >> 4;
        let v_samp = sampling & 0x0F;
        let quant_tbl = read_u8(data, pos)?;
        if !(1..=4).contains(&h_samp) || !(1..=4).contains(&v_samp) {
            return Err(CodecError::Malformed(
                "invalid JPEG sampling factor".to_owned(),
            ));
        }
        if quant_tbl > 3 {
            return Err(CodecError::Malformed(
                "invalid JPEG quantization table selector".to_owned(),
            ));
        }
        max_h_samp = max_h_samp.max(h_samp);
        max_v_samp = max_v_samp.max(v_samp);
        components.push(FrameComponent {
            id,
            h_samp,
            v_samp,
            quant_tbl,
        });
    }

    Ok((width, height, components, max_h_samp, max_v_samp))
}

pub(super) fn parse_dqt(
    data: &[u8],
    pos: &mut usize,
    quant_tables: &mut Vec<Option<[u16; 64]>>,
) -> CodecResult<()> {
    let length = usize::from(read_u16(data, pos)?);
    let end = pos.saturating_add(length.saturating_sub(2));

    while *pos < end {
        let info = read_u8(data, pos)?;
        let precision = usize::from(info >> 4);
        let table_id = usize::from(info & 0x0F);
        if table_id >= 4 {
            return Err(CodecError::Malformed(
                "invalid JPEG quantization table id".to_owned(),
            ));
        }

        let mut table_zigzag = [0u16; 64];
        for entry in &mut table_zigzag {
            *entry = if precision == 0 {
                u16::from(read_u8(data, pos)?)
            } else {
                read_u16(data, pos)?
            };
        }
        while quant_tables.len() <= table_id {
            quant_tables.push(None);
        }
        quant_tables[table_id] = Some(table_zigzag);
    }
    Ok(())
}

pub(super) fn parse_dht(
    data: &[u8],
    pos: &mut usize,
    dc_tables: &mut Vec<Option<HuffTable>>,
    ac_tables: &mut Vec<Option<HuffTable>>,
) -> CodecResult<()> {
    let length = usize::from(read_u16(data, pos)?);
    let end = pos.saturating_add(length.saturating_sub(2));

    while *pos < end {
        let info = read_u8(data, pos)?;
        let table_class = info >> 4;
        let table_id = usize::from(info & 0x0F);
        if table_id >= 4 {
            return Err(CodecError::Malformed(
                "invalid JPEG Huffman table id".to_owned(),
            ));
        }

        let mut counts = [0u8; 16];
        let mut total_values = 0usize;
        for entry in &mut counts {
            *entry = read_u8(data, pos)?;
            total_values = total_values.saturating_add(usize::from(*entry));
        }

        let mut values = Vec::with_capacity(total_values);
        for _ in 0..total_values {
            values.push(read_u8(data, pos)?);
        }

        let table = HuffTable::build(&counts, &values);
        if table_class == 0 {
            while dc_tables.len() <= table_id {
                dc_tables.push(None);
            }
            dc_tables[table_id] = Some(table);
        } else {
            while ac_tables.len() <= table_id {
                ac_tables.push(None);
            }
            ac_tables[table_id] = Some(table);
        }
    }
    Ok(())
}

pub(super) fn parse_sos(
    data: &[u8],
    pos: &mut usize,
    components: &[FrameComponent],
) -> CodecResult<(Vec<ScanComponent>, usize, u8, u8, u8, u8)> {
    let _len = read_u16(data, pos)?;
    let num_scan_comps = read_u8(data, pos)?;
    if num_scan_comps == 0 {
        return Err(CodecError::Malformed(
            "JPEG scan has no components".to_owned(),
        ));
    }

    let mut scan_comps = Vec::with_capacity(num_scan_comps as usize);
    for _ in 0..num_scan_comps {
        let comp_id = read_u8(data, pos)?;
        let tbl_info = read_u8(data, pos)?;
        let dc_tbl = tbl_info >> 4;
        let ac_tbl = tbl_info & 0x0F;
        let comp_index = components
            .iter()
            .position(|c| c.id == comp_id)
            .malformed("JPEG scan references an unknown component")?;
        if dc_tbl > 3 || ac_tbl > 3 {
            return Err(CodecError::Malformed(
                "invalid JPEG scan Huffman table selector".to_owned(),
            ));
        }
        scan_comps.push(ScanComponent {
            comp_index,
            dc_tbl,
            ac_tbl,
        });
    }

    let ss = read_u8(data, pos)?;
    let se = read_u8(data, pos)?;
    let ah_al = read_u8(data, pos)?;
    let ah = ah_al >> 4;
    let al = ah_al & 0x0F;
    let entropy_start = *pos;

    Ok((scan_comps, entropy_start, ss, se, ah, al))
}

pub(super) fn parse_dri(data: &[u8], pos: &mut usize) -> CodecResult<u16> {
    let _len = read_u16(data, pos)?;
    let restart_interval = read_u16(data, pos)?;
    Ok(restart_interval)
}

pub(super) fn parse_jpeg(data: &[u8]) -> CodecResult<JpegInfo> {
    let mut pos = 0usize;

    let soi = read_u16(data, &mut pos)?;
    if soi != M_SOI {
        return Err(CodecError::Malformed("invalid JPEG SOI marker".to_owned()));
    }

    let mut width = 0u16;
    let mut height = 0u16;
    let mut components: Vec<FrameComponent> = Vec::new();
    let mut num_components = 0u8;
    let mut max_h_samp = 0u8;
    let mut max_v_samp = 0u8;
    let mut quant_tables: Vec<Option<[u16; 64]>> = Vec::new();
    let mut dc_huff_tables: Vec<Option<HuffTable>> = Vec::new();
    let mut ac_huff_tables: Vec<Option<HuffTable>> = Vec::new();
    let mut scan_components: Vec<ScanComponent> = Vec::new();
    let mut restart_interval: u16 = 0;
    let mut entropy_start = 0usize;
    let mut saw_sof = false;
    let mut saw_sos = false;
    let mut progressive = false;
    let mut scans: Vec<ScanInfo> = Vec::new();
    let mut adobe_transform = None;
    let mut metadata = Vec::new();

    let eoi_pos = loop {
        let marker_offset = pos as u64;
        let marker = find_next_marker(data, &mut pos)
            .map_err(|error| error.at(marker_offset, "jpeg_marker"))?;

        match marker {
            M_SOF0 | M_SOF2 => {
                if saw_sof {
                    return Err(CodecError::Malformed(
                        "duplicate JPEG frame header".to_owned(),
                    ));
                }
                progressive = marker == M_SOF2;
                let result = parse_sof0(data, &mut pos)
                    .map_err(|error| error.at(marker_offset, "jpeg_sof"))?;
                width = result.0;
                height = result.1;
                components = result.2;
                max_h_samp = result.3;
                max_v_samp = result.4;
                num_components = components.len().to_le_bytes()[0];
                saw_sof = true;
            }
            M_DQT => {
                parse_dqt(data, &mut pos, &mut quant_tables)
                    .map_err(|error| error.at(marker_offset, "jpeg_dqt"))?;
            }
            M_DHT => {
                parse_dht(data, &mut pos, &mut dc_huff_tables, &mut ac_huff_tables)
                    .map_err(|error| error.at(marker_offset, "jpeg_dht"))?;
            }
            M_SOS => {
                if !saw_sof {
                    return Err(CodecError::Malformed(
                        "JPEG scan precedes the frame header".to_owned(),
                    ));
                }
                let result = parse_sos(data, &mut pos, &components)
                    .map_err(|error| error.at(marker_offset, "jpeg_sos"))?;
                let comps = result.0;
                let scan_start = result.1;
                let ss = result.2;
                let se = result.3;
                let ah = result.4;
                let al = result.5;
                let scan_end = find_entropy_end(data, pos);

                let scan_info = ScanInfo {
                    components: comps.clone(),
                    entropy_start: scan_start,
                    entropy_end: scan_end,
                    ss,
                    se,
                    ah,
                    al,
                    restart_interval,
                    dc_huff_tables: dc_huff_tables.clone(),
                    ac_huff_tables: ac_huff_tables.clone(),
                };
                scans.push(scan_info);

                if !progressive {
                    // Baseline JPEG has exactly one entropy-coded scan in this
                    // parser: after SOS, we require EOI and break immediately.
                    scan_components = comps;
                    entropy_start = scan_start;
                    saw_sos = true;
                    break find_eoi(data, pos)
                        .map_err(|error| error.at(marker_offset, "jpeg_sos"))?;
                } else {
                    saw_sos = true;
                    if scan_components.is_empty() {
                        scan_components = comps;
                        entropy_start = scan_start;
                    }
                    pos = scan_end;
                }
            }
            M_DRI => {
                restart_interval = parse_dri(data, &mut pos)
                    .map_err(|error| error.at(marker_offset, "jpeg_dri"))?;
            }
            M_APP14 => {
                let length = usize::from(
                    read_u16(data, &mut pos)
                        .map_err(|error| error.at(marker_offset, "jpeg_app14"))?,
                );
                if length < 2 {
                    continue;
                }
                let payload_len = length.saturating_sub(2);
                if payload_len > data.len().saturating_sub(pos) {
                    return Err(CodecError::Malformed(
                        "truncated JPEG APP14 payload".to_owned(),
                    ));
                }
                let payload_end = pos.saturating_add(payload_len);
                metadata.push(crate::types::OpaqueMetadata {
                    kind: vec![0xee],
                    data: data[pos..payload_end].to_vec(),
                });
                if data.get(pos..pos.saturating_add(5)) == Some(b"Adobe") && length >= 14 {
                    // `payload_len >= 12` and the complete payload range was
                    // validated above, so the transform byte is present.
                    adobe_transform = Some(data[pos.saturating_add(11)]);
                }
                pos = payload_end;
            }
            0xFFE0..=0xFFEF | 0xFFFE => {
                let length = usize::from(
                    read_u16(data, &mut pos)
                        .map_err(|error| error.at(marker_offset, "jpeg_metadata"))?,
                );
                if length < 2 {
                    continue;
                }
                let payload_len = length.saturating_sub(2);
                if payload_len > data.len().saturating_sub(pos) {
                    return Err(CodecError::Malformed(
                        "truncated JPEG metadata marker payload".to_owned(),
                    )
                    .at(marker_offset, "jpeg_metadata"));
                }
                let payload_end = pos.saturating_add(payload_len);
                metadata.push(crate::types::OpaqueMetadata {
                    kind: vec![marker.to_le_bytes()[0]],
                    data: data[pos..payload_end].to_vec(),
                });
                pos = payload_end;
            }
            M_EOI => {
                break pos.saturating_sub(2);
            }
            0xFFD0..=0xFFD7 => {}
            0xFF01 => {
                return Err(CodecError::Malformed(
                    "unexpected JPEG TEM marker".to_owned(),
                ));
            }
            _ => {
                let length = usize::from(
                    read_u16(data, &mut pos)
                        .map_err(|error| error.at(marker_offset, "jpeg_segment"))?,
                );
                if length < 2 {
                    continue;
                }
                pos = pos.saturating_add(length.saturating_sub(2));
            }
        }
    };

    if !saw_sos {
        return Err(CodecError::Malformed("JPEG contains no scan".to_owned()));
    }

    Ok(JpegInfo {
        width,
        height,
        num_components,
        components,
        quant_tables,
        dc_huff_tables,
        ac_huff_tables,
        scan_components,
        restart_interval,
        entropy_start,
        eoi_pos,
        max_h_samp,
        max_v_samp,
        progressive,
        scans,
        adobe_transform,
        metadata,
    })
}

#[cfg(coverage)]
pub(crate) fn __coverage_exercise_private_branches() {
    let mut position = 0;
    assert!(parse_sof0(&[], &mut position).is_err());
    let mut position = 0;
    assert!(parse_sof0(&[0, 2], &mut position).is_err());

    let mut position = 0;
    assert!(parse_dqt(&[], &mut position, &mut Vec::new()).is_err());
    let mut position = 0;
    assert!(parse_dqt(&[0, 3], &mut position, &mut Vec::new()).is_err());

    let mut position = 0;
    assert!(parse_dht(&[], &mut position, &mut Vec::new(), &mut Vec::new()).is_err());
    let mut position = 0;
    assert!(parse_dht(&[0, 3], &mut position, &mut Vec::new(), &mut Vec::new()).is_err());

    let mut position = 0;
    assert!(parse_dri(&[], &mut position).is_err());
    let mut position = 0;
    assert!(parse_dri(&[0, 4], &mut position).is_err());

    let mut position = 0;
    assert!(find_next_marker(&[0xff], &mut position).is_err());

    for data in [&[0xff, 0x00][..], &[0xff, 0xff, 0xff, 0xd8]] {
        let mut position = 0;
        let _ = find_next_marker(data, &mut position);
    }

    for data in [
        &[][..],
        &[0, 0],
        &[0xff, 0xd8, 0xff, 0xdd],
        &[0xff, 0xd8, 0xff, 0xee],
        &[0xff, 0xd8, 0xff, 0xd0],
        &[0xff, 0xd8, 0xff, 0xe0],
        &[0xff, 0xd8, 0xff, 0xc8, 0x00, 0x04, 0xaa, 0xbb, 0xff, 0xd9],
        &[0xff, 0xd8, 0xff, 0xc8, 0x00, 0x01, 0xff, 0xd9],
        &[0xff, 0xd8, 0xff, 0xc8, 0xff],
    ] {
        assert!(parse_jpeg(data).is_err());
    }
}
