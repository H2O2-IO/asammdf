use crate::{ByteOrder, ChannelType, DependencyType, RecordIDType, SignalType};

use super::{
    CCBlock, CDBlock, CEBlock, CGBlock, CNBlock, DGBlock, DateType, DimType, HDBlock, IDBlock,
    SRBlock, TRBlock, TXBlock, TimeType, TriggerEvent, VectorCANType,
};
use chrono::Local;
use nom::bytes::complete::take;
use nom::number::complete::{le_f64, le_u16, le_u32, le_u8};
use nom::sequence::tuple;
use nom::IResult;

/// Basic parser that parse id & blocksize of MDFv3 Block Objects
pub(crate) fn block_base_v3(input: &[u8]) -> IResult<&[u8], (&[u8], u16)> {
    let (input, (id, block_size)) = tuple((take(2u32), le_u16))(input)?;
    Ok((input, (id, block_size)))
}

/// IDBlock parser, fix length(46bytes)
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

/// HDBlcok basic parser, basic info with fix length(164bytes)
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

/// TRBlock basic parser, basic info with fix length(10bytes)
pub(crate) fn tr_block_basic(input: &[u8]) -> IResult<&[u8], (TRBlock, u16)> {
    let (input, ((id, block_size), link_comment, trigger_event_number)) =
        tuple((block_base_v3, le_u32, le_u16))(input)?;
    let mut tr_block = TRBlock::new(Vec::with_capacity(trigger_event_number as usize));
    tr_block.id = String::from_utf8_lossy(id).trim_matches('\0').to_string();
    tr_block.block_size = block_size as _;
    tr_block.link_comment = link_comment;

    Ok((input, (tr_block, trigger_event_number)))
}

/// SRBlock basic parser, basic info with fix length(24bytes)
pub(crate) fn sr_block_basic(input: &[u8]) -> IResult<&[u8], SRBlock> {
    let (
        input,
        (
            (id, block_size),
            link_next_sr,
            link_data_block,
            reduced_samples_number,
            time_interval_len,
        ),
    ) = tuple((block_base_v3, le_u32, le_u32, le_u32, le_f64))(input)?;
    let mut sr_block = SRBlock::new();
    sr_block.id = String::from_utf8_lossy(id).trim_matches('\0').to_string();
    sr_block.block_size = block_size as _;
    sr_block.link_next_sr = link_next_sr;
    sr_block.link_data_block = link_data_block;
    sr_block.reduced_samples_number = reduced_samples_number as u64;
    sr_block.time_interval_len = time_interval_len;

    Ok((input, sr_block))
}

/// TriggerEvent(belong to TRBlock) parser, fix length(24bytes)
pub(crate) fn trigger_evt(input: &[u8]) -> IResult<&[u8], TriggerEvent> {
    let (input, (time, pre_time, post_time)) = tuple((le_f64, le_f64, le_f64))(input)?;

    let trig_evt = TriggerEvent::new(time, pre_time, post_time);
    Ok((input, trig_evt))
}

/// DateType(belong to CCBlock) parser, fix length((7bytes)
pub(crate) fn parse_datetype(input: &[u8]) -> IResult<&[u8], DateType> {
    let (input, (ms, minute, hour, day, month, year)) =
        tuple((le_u16, le_u8, le_u8, le_u8, le_u8, le_u8))(input)?;
    let date = DateType::new(ms, minute, hour, day, month, year);
    Ok((input, date))
}

/// TimeType(belong to CCBlock) parser, fix length((5bytes)
pub(crate) fn parse_timetype(input: &[u8]) -> IResult<&[u8], TimeType> {
    let (input, (ms, day)) = tuple((le_u32, le_u8))(input)?;
    Ok((input, TimeType::new(ms, day)))
}

/// CCBlock basic parser, basic info with fix length(46bytes)
pub(crate) fn cc_block_basic(input: &[u8], _byte_order: ByteOrder) -> IResult<&[u8], CCBlock> {
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
    let (max, min) = if bounded > 0 {
        (max, min)
    } else {
        (f64::NAN, f64::NAN)
    };
    let conversion_type = (conversion_type as u32)
        .try_into()
        .map_or(None, |x| Some(x));
    let unit = String::from_utf8_lossy(unit).trim_matches('\0').to_string();
    let mut cc_block = CCBlock::new(conversion_type, unit, min, max);
    cc_block.id = String::from_utf8_lossy(id).trim_matches('\0').to_string();
    cc_block.block_size = block_size as _;
    cc_block.tab_size = tab_size;
    Ok((input, cc_block))
}

/// CEBlock basic parser, basic info with fix length(6bytes)
pub(crate) fn ce_block_basic(input: &[u8]) -> IResult<&[u8], CEBlock> {
    let (input, ((id, block_size), ext_type)) = tuple((block_base_v3, le_u16))(input)?;
    let mut ce_block = CEBlock::new();
    ce_block.id = String::from_utf8_lossy(id).trim_matches('\0').to_string();
    ce_block.block_size = block_size as _;
    ce_block.extension_type = (ext_type as u32).try_into().map_or(None, |x| Some(x));

    Ok((input, ce_block))
}

/// DimType(belong to CEBlock) parser, fix length(118 bytes)
pub(crate) fn parse_dim_type(input: &[u8]) -> IResult<&[u8], DimType> {
    let (input, (module, address, description, ecuid)) =
        tuple((le_u16, le_u32, take(80u32), take(32u32)))(input)?;
    let description = String::from_utf8_lossy(description)
        .trim_matches('\0')
        .to_string();
    let ecuid = String::from_utf8_lossy(ecuid)
        .trim_matches('\0')
        .to_string();
    Ok((input, DimType::new(module, address, description, ecuid)))
}

/// VectorCANType(belong to CEBlock) parser, fix length(80bytes)
pub(crate) fn vector_can_type(input: &[u8]) -> IResult<&[u8], VectorCANType> {
    let (input, (id, index, message, sender)) =
        tuple((le_u32, le_u32, take(36u32), take(36u32)))(input)?;
    let message = String::from_utf8_lossy(message)
        .trim_matches('\0')
        .to_string();
    let sender = String::from_utf8_lossy(sender)
        .trim_matches('\0')
        .to_string();
    Ok((input, VectorCANType::new(id, index, message, sender)))
}

/// CDBlock basic parser, basic info with fix length(8bytes)
pub(crate) fn cdblock_basic(input: &[u8]) -> IResult<&[u8], (u16, CDBlock)> {
    let (input, ((id, block_size), dependency_type, dependency_number)) =
        tuple((block_base_v3, le_u16, le_u16))(input)?;

    let mut cd_block = CDBlock::new();
    cd_block.id = String::from_utf8_lossy(id).trim_matches('\0').to_string();
    cd_block.block_size = block_size as _;
    cd_block.dependency_type = dependency_type;

    Ok((input, (dependency_number, cd_block)))
}

/// DependencyType(belong to CDBlock) parser, fix length(12bytes)
pub(crate) fn dependency_type(input: &[u8]) -> IResult<&[u8], DependencyType> {
    let (input, (link_dg, link_cg, link_cn)) = tuple((le_u32, le_u32, le_u32))(input)?;

    Ok((
        input,
        DependencyType::new(link_dg as i64, link_cg as i64, link_cn as i64),
    ))
}

/// CNBlock basic parser, basic info with fix length(218bytes)
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
    let channel_type = if channel_type == 0 {
        ChannelType::Data
    } else if channel_type == 1 {
        ChannelType::Master
    } else {
        panic!()
    };
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

/// CGBlock basic parser, basic info with fix length(26bytes)
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
    let mut cg_block = CGBlock::new(String::default());
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

/// DGBlock basic parser, basic info with fix length(24bytes)
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
    dg_block.id = String::from_utf8_lossy(id).trim_matches('\0').to_string();
    dg_block.block_size = block_size as _;
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

/// TXBlock basic parser, basic info with fix length(2bytes)
pub(crate) fn tx_block_basic(input: &[u8]) -> IResult<&[u8], TXBlock> {
    let (input, (id, block_size)) = (block_base_v3)(input)?;

    let mut tx_block = TXBlock::new(Default::default());
    tx_block.id = String::from_utf8_lossy(id).trim_matches('\0').to_string();
    tx_block.block_size = block_size as _;
    Ok((input, tx_block))
}
