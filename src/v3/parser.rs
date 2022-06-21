use crate::{ByteOrder, ChannelType, RecordIDType, SignalType};

use super::{CCBlock, CGBlock, CNBlock, DGBlock, HDBlock, IDBlock, TRBlock, TXBlock, TriggerEvent};
use chrono::{DateTime, Local};
use indextree::Descendants;
use nom::bytes::complete::take;
use nom::multi::count;
use nom::number::complete::{le_f64, le_i16, le_u16, le_u32, le_u64};
use nom::sequence::tuple;
use nom::IResult;

pub(crate) fn block_base_v3(input: &[u8]) -> IResult<&[u8], (&[u8], u16)> {
    let (input, (id, block_size)) = tuple((take(2u32), le_u16))(input)?;
    Ok((input, (id, block_size)))
}

pub(crate) fn id_block(input: &[u8]) -> IResult<&[u8], IDBlock> {
    let (
        input,
        (
            fileid,
            formatid,
            programid,
            byteorder,
            float_point_format,
            version,
            codepage,
            _,
            unfinalized_flags,
            custom_flags,
        ),
    ) = tuple((
        take(8u32),
        take(8u32),
        take(8u32),
        le_u16,
        le_u16,
        le_u16,
        le_u16,
        take(4 * 6u32),
        le_u16,
        le_u16,
    ))(input)?;

    let mut idblock = IDBlock::new(String::from_utf8(programid.to_vec()).unwrap(), codepage);

    idblock.file_id = String::from_utf8_lossy(fileid)
        .trim_matches('\0')
        .to_string();
    idblock.format_id = String::from_utf8_lossy(formatid)
        .trim_matches('\0')
        .to_string();
    idblock.program_id = String::from_utf8_lossy(programid)
        .trim_matches('\0')
        .to_string();

    idblock.byte_order = Some(if byteorder > 0 {
        ByteOrder::BigEndian
    } else {
        ByteOrder::LittleEndian
    });
    idblock.float_point_format = (float_point_format as u32)
        .try_into()
        .map_or(None, |x| Some(x));
    idblock.version = version;
    idblock.unfinalized_flags = (unfinalized_flags as u32)
        .try_into()
        .map_or(None, |x| Some(x));
    idblock.custom_flags = custom_flags;
    Ok((input, idblock))
}

/// basic info(164bytes)
pub(crate) fn header_block_basic(input: &[u8]) -> IResult<&[u8], HDBlock> {
    let (
        input,
        (
            (id, block_size),
            link_first_file_group,
            link_first_comment_txt,
            link_program_block,
            _,
            date_bytes,
            time_bytes,
            author_bytes,
            organization_bytes,
            project_bytes,
            subject_bytes,
        ),
    ) = tuple((
        block_base_v3,
        le_u32,
        le_u32,
        le_u32,
        le_u16,
        take(10u32),
        take(8u32),
        take(32u32),
        take(32u32),
        take(32u32),
        take(32u32),
    ))(input)?;

    let mut header_block = HDBlock::new(Local::now());

    header_block.id = String::from_utf8_lossy(id).trim_matches('\0').to_string();
    header_block.block_size = block_size as _;
    header_block.link_first_file_group = link_first_file_group;
    header_block.link_file_comment_txt = link_first_comment_txt;
    header_block.link_program_block = link_program_block;
    header_block.date = String::from_utf8_lossy(date_bytes)
        .trim_matches('\0')
        .to_string();
    header_block.time = String::from_utf8_lossy(time_bytes)
        .trim_matches('\0')
        .to_string();
    header_block.author = String::from_utf8_lossy(author_bytes)
        .trim_matches('\0')
        .to_string();
    header_block.organization = String::from_utf8_lossy(organization_bytes)
        .trim_matches('\0')
        .to_string();
    header_block.project = String::from_utf8_lossy(project_bytes)
        .trim_matches('\0')
        .to_string();
    header_block.subject = String::from_utf8_lossy(subject_bytes)
        .trim_matches('\0')
        .to_string();
    Ok((input, header_block))
}

/// trblock basic info(10bytes)
pub(crate) fn tr_block_basic(input: &[u8]) -> IResult<&[u8], (TRBlock, u16)> {
    let (input, ((id, block_size), link_comment, trigger_event_number)) =
        tuple((block_base_v3, le_u32, le_u16))(input)?;
    let mut tr_block = TRBlock::new(Vec::with_capacity(trigger_event_number as usize));
    tr_block.id = String::from_utf8_lossy(id).trim_matches('\0').to_string();
    tr_block.block_size = block_size as _;
    tr_block.link_comment = link_comment;

    Ok((input, (tr_block, trigger_event_number)))
}

pub(crate) fn trigger_evt(input: &[u8]) -> IResult<&[u8], TriggerEvent> {
    let (input, (time, pre_time, post_time)) = tuple((le_f64, le_f64, le_f64))(input)?;

    let mut trig_evt = TriggerEvent::new(time, pre_time, post_time);
    Ok((input, trig_evt))
}

/// ccblock basic info(46bytes)
pub(crate) fn cc_block_basic(input: &[u8], byte_order: ByteOrder) -> IResult<&[u8], CCBlock> {
    let (input, ((id, block_size), bounded, min, max, unit, conversion_type, tab_size)) =
        tuple((
            block_base_v3,
            le_u16,
            le_f64,
            le_f64,
            take(20u32),
            le_u16,
            le_u16,
        ))(input)?;
    let max = if bounded > 0 { max } else { f64::NAN };
    let min = if bounded < 0 { min } else { f64::NAN };
    let conversion_type = (conversion_type as u32)
        .try_into()
        .map_or(None, |x| Some(x));
    let unit = String::from_utf8_lossy(id).trim_matches('\0').to_string();
    let mut cc_block = CCBlock::new(conversion_type, unit, min, max);
    cc_block.id = String::from_utf8_lossy(id).trim_matches('\0').to_string();
    cc_block.block_size = block_size as _;

    Ok((input, cc_block))
}

/// cnblock basic info(218bytes)
pub(crate) fn cn_block_basic(input: &[u8], byte_order: ByteOrder) -> IResult<&[u8], CNBlock> {
    let (
        input,
        (
            (id, block_size),
            link_next_cn_block,
            link_cc_block,
            link_ce_block,
            link_cd_block,
            link_channel_comment,
            channel_type,
            name_bytes,
            description_bytes,
            bit_offset,
            bits_count,
            signal_type,
            bounded,
            min_raw,
            max_raw,
            rate,
        ),
    ) = tuple((
        block_base_v3,
        le_u32,
        le_u32,
        le_u32,
        le_u32,
        le_u32,
        le_u16,
        take(32u32),
        take(128u32),
        le_u16,
        le_u16,
        le_u16,
        le_u16,
        le_f64,
        le_f64,
        le_f64,
    ))(input)?;
    let signal_type = SignalType::from_u16(signal_type, byte_order).unwrap();
    let channel_type = (channel_type as u32)
        .try_into()
        .map_or(None, |x| Some(x))
        .unwrap();
    let name = String::from_utf8_lossy(name_bytes)
        .trim_matches('\0')
        .to_string();
    let description = String::from_utf8_lossy(description_bytes)
        .trim_matches('\0')
        .to_string();
    let mut cn_block = CNBlock::new(
        signal_type,
        channel_type,
        name,
        description,
        bit_offset as u32,
        0,
        bits_count as u32,
    );

    cn_block.id = String::from_utf8_lossy(id).trim_matches('\0').to_string();
    cn_block.block_size = block_size as _;
    cn_block.link_next_cnblock = link_next_cn_block;
    cn_block.link_ccblock = link_cc_block;
    cn_block.link_ceblock = link_ce_block;
    cn_block.link_cdblock = link_cd_block;
    cn_block.link_channel_comment = link_channel_comment;
    cn_block.rate = rate;
    // has bounded indicator
    if bounded > 0 {
        cn_block.min_raw = min_raw;
        cn_block.max_raw = max_raw;
    }
    Ok((input, cn_block))
}

/// cgblock basic info(26bytes)
pub(crate) fn cg_block_basic(input: &[u8]) -> IResult<&[u8], CGBlock> {
    let (
        input,
        (
            (id, block_size),
            link_next_cg_block,
            link_first_cn_block,
            link_channel_group_comment,
            record_id,
            _,
            record_size,
            record_count,
        ),
    ) = tuple((
        block_base_v3,
        le_u32,
        le_u32,
        le_u32,
        le_u16,
        le_u16,
        le_u16,
        le_u32,
    ))(input)?;
    let mut cg_block = CGBlock::new(Default::default());
    cg_block.id = String::from_utf8_lossy(id).trim_matches('\0').to_string();
    cg_block.block_size = block_size as _;

    cg_block.link_next_cgblock = link_next_cg_block;
    cg_block.link_first_cnblock = link_first_cn_block;
    cg_block.link_cg_comment = link_channel_group_comment;
    cg_block.record_id = record_id;
    cg_block.record_size = record_size as u32;
    cg_block.record_count = record_count as u64;

    Ok((input, cg_block))
}

/// Data group block basic(24 bytes)
pub(crate) fn dg_block_basic(input: &[u8]) -> IResult<&[u8], DGBlock> {
    let (
        input,
        (
            (id, block_size),
            link_next_dg_block,
            link_next_cg_block,
            link_tr_block,
            link_data_records,
            _,
            record_id_type,
        ),
    ) = tuple((
        block_base_v3,
        le_u32,
        le_u32,
        le_u32,
        le_u32,
        le_u16,
        le_u16,
    ))(input)?;

    let mut dg_block = DGBlock::new();
    dg_block.link_next_dgblock = link_next_dg_block;
    dg_block.link_next_cgblock = link_next_cg_block;
    dg_block.link_trblock = link_tr_block;
    dg_block.link_data_records = link_data_records;

    match record_id_type {
        1 => dg_block.record_id_type = Some(RecordIDType::Before8Bit),
        2 => dg_block.record_id_type = Some(RecordIDType::BeforeAndAfter8Bit),
        _ => dg_block.record_id_type = None,
    }
    Ok((input, dg_block))
}

pub(crate) fn read_le_u64(input: &[u8]) -> IResult<&[u8], u64> {
    Ok(le_u64(input)?)
}

pub(crate) fn read_le_u32(input: &[u8]) -> IResult<&[u8], u32> {
    Ok(le_u32(input)?)
}

pub(crate) fn read_le_i16(input: &[u8]) -> IResult<&[u8], i16> {
    Ok(le_i16(input)?)
}

pub(crate) fn read_le_u16(input: &[u8]) -> IResult<&[u8], u16> {
    Ok(le_u16(input)?)
}

pub(crate) fn read_n_le_f64(input: &[u8],number:usize) -> IResult<&[u8], Vec<f64>> {
    count(le_f64, number)(input)
}

pub(crate) fn read_str(input: &[u8], count: u32) -> IResult<&[u8], String> {
    let (input, result) = take(count)(input)?;
    let result = String::from_utf8_lossy(result)
        .trim_matches('\0')
        .to_string();
    Ok((input, result))
}

// TXBlock basis is 2 bytes
pub(crate) fn tx_block_basic(input: &[u8]) -> IResult<&[u8], TXBlock> {
    let (input, (id, block_size)) = (block_base_v3)(input)?;

    let mut tx_block = TXBlock::new(Default::default());
    tx_block.id = String::from_utf8_lossy(id).trim_matches('\0').to_string();
    tx_block.block_size = block_size as _;
    Ok((input, tx_block))
}
