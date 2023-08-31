use core::time;

use crate::{ByteOrder, ConversionType};

use super::{
    ATBlock, AttachmentFlags, CCBlock, CGBlock, CHBlock, CNBlock, ChannelFlags, ConversionFlags,
    DGBlock, DLBlock, DTBlock, DZBlock, EVBlock, FHBlock, HDBlock, HLBlock, IDBlock, SIBlock,
    SRBlock,
};
use chrono::Local;
use nom::bytes::complete::take;
use nom::number::complete::{le_f64, le_i16, le_i64, le_u16, le_u32, le_u64, le_u8};
use nom::sequence::tuple;
use nom::IResult;

/// return basic infos about MDF block v4(22bytes)
pub(crate) fn block_base(input: &[u8]) -> IResult<&[u8], (String, i64, u64)> {
    let (input, (ver1, ver2)) = tuple((le_u8, le_u8))(input)?;
    if ver1 == 35 && ver2 == 35 {
        let (input, (id_bytes, _, block_size, link_count)) =
            tuple((take(2u32), take(4u32), le_i64, le_u64))(input)?;
        Ok((
            input,
            (
                String::from_utf8_lossy(id_bytes).to_string(),
                block_size,
                link_count,
            ),
        ))
    } else {
        panic!("unsupported version");
    }
}

pub(crate) fn id_block(input: &[u8]) -> IResult<&[u8], IDBlock> {
    let (input, (fileid, formatid, programid, _, version, _, unfinalized_flags, custom_flags)) =
        tuple((
            take(8u32),
            take(8u32),
            take(8u32),
            take(4u32),
            le_u16,
            take(2 * 15u32),
            le_u16,
            le_u16,
        ))(input)?;

    let mut idblock = IDBlock::new(String::from_utf8(programid.to_vec()).unwrap());

    idblock.file_id = String::from_utf8(fileid.to_vec()).unwrap();
    idblock.format_id = String::from_utf8(formatid.to_vec()).unwrap();
    idblock.program_id = String::from_utf8(programid.to_vec()).unwrap();
    idblock.version = version;
    idblock.unfinalized_flags = (unfinalized_flags as u32)
        .try_into()
        .map_or(None, |x| Some(x));
    idblock.custom_flags = custom_flags;
    Ok((input, idblock))
}

/// HDBlock basic info, fix length(31bytes)
pub(crate) fn hd_block_basic(
    input: &[u8],
    id: String,
    block_size: i64,
    link_count: u64,
) -> IResult<&[u8], HDBlock> {
    let (
        input,
        (
            timestamp,
            utc_offset,
            dst_offset,
            flags,
            time_quality,
            start_indicator,
            start_angle,
            start_distance,
        ),
    ) = tuple((le_u64, le_i16, le_i16, le_u8, le_u8, le_u8, le_f64, le_f64))(input)?;
    // prepare basic info
    let flags = (flags as u32).try_into().map_or(None, |x| Some(x));
    let time_quality = (time_quality as u32).try_into().map_or(None, |x| Some(x));

    let mut hdblock = HDBlock::new(
        Local::now(),
        time_quality,
        Default::default(),
        Default::default(),
        Default::default(),
    );
    hdblock.id = id;
    hdblock.block_size = block_size as u64;
    hdblock.links_count = link_count;
    hdblock.timestamp = timestamp;
    hdblock.utc_offset = utc_offset;
    hdblock.dst_offset = dst_offset;
    hdblock.flags = flags;
    hdblock.start_angle = if start_indicator & 1 > 0 {
        start_angle
    } else {
        f64::NAN
    };
    hdblock.start_distance = if start_indicator & 2 > 0 {
        start_distance
    } else {
        f64::NAN
    };

    Ok((input, hdblock))
}

/// DGBlock basic info, fix length(1bytes)
pub(crate) fn dg_block_basic(
    input: &[u8],
    id: String,
    block_size: i64,
    link_count: u64,
) -> IResult<&[u8], DGBlock> {
    let (input, record_id_type) = le_u8(input)?;
    // prepare basic info
    let mut dg_block = DGBlock::new();
    dg_block.id = id;
    dg_block.block_size = block_size as u64;
    dg_block.links_count = link_count;
    dg_block.record_id_type = (record_id_type as u32).try_into().map_or(None, |x| Some(x));

    Ok((input, dg_block))
}

/// HLBlock basic info, fix length(3bytes)
pub(crate) fn hl_block_basic(
    input: &[u8],
    id: String,
    block_size: i64,
    link_count: u64,
) -> IResult<&[u8], HLBlock> {
    let (input, (data_block_flags, zip_type)) = tuple((le_u16, le_u8))(input)?;
    let data_block_flags = (data_block_flags as u32)
        .try_into()
        .map_or(None, |x| Some(x));
    let zip_type = (zip_type as u32).try_into().map_or(None, |x| Some(x));

    // prepare basic info
    let mut hl_block = HLBlock::new(zip_type, data_block_flags);
    hl_block.id = id;
    hl_block.block_size = block_size as u64;
    hl_block.links_count = link_count;

    Ok((input, hl_block))
}

/// DLBlock basic info, fix length(16bytes)
pub(crate) fn dl_block_basic(
    input: &[u8],
    id: String,
    block_size: i64,
    link_count: u64,
) -> IResult<&[u8], DLBlock> {
    let (input, (data_block_flags, _, count, equal_length)) =
        tuple((le_u8, take(3u32), le_u32, le_u64))(input)?;

    let data_block_flags = (data_block_flags as u32)
        .try_into()
        .map_or(None, |x| Some(x));

    // prepare basic info
    let mut dl_block = DLBlock::new(data_block_flags, count);
    dl_block.id = id;
    dl_block.block_size = block_size as u64;
    dl_block.links_count = link_count;
    dl_block.equal_length = equal_length;

    Ok((input, dl_block))
}

/// DZBlock basic info, fix length(24bytes)
pub(crate) fn dz_block_basic(
    input: &[u8],
    id: String,
    block_size: i64,
    link_count: u64,
) -> IResult<&[u8], DZBlock> {
    let (input, (block_type_bytes, zip_type, _, zip_parameter, size, length)) =
        tuple((take(2u32), le_u8, take(1u32), le_u32, le_i64, le_u64))(input)?;

    // conver zip_type
    let zip_type = (zip_type as u32).try_into().map_or(None, |x| Some(x));
    let block_type = String::from_utf8_lossy(block_type_bytes).to_string();

    // prepare basic info
    let mut dz_block = DZBlock::new(length, size, zip_type, zip_parameter, Some(block_type));
    dz_block.id = id;
    dz_block.block_size = block_size as u64;
    dz_block.links_count = link_count;

    Ok((input, dz_block))
}

/// SIBlock basic info, fix length(3bytes)
pub(crate) fn si_block_basic(
    input: &[u8],
    id: String,
    block_size: i64,
    link_count: u64,
) -> IResult<&[u8], SIBlock> {
    let (input, (source_type, bus_type, flags)) = tuple((le_u8, le_u8, le_u8))(input)?;
    // convert source_type,bus_type,flags to coresponding enum
    let source_type = (source_type as u32).try_into().map_or(None, |x| Some(x));
    let bus_type = (bus_type as u32).try_into().map_or(None, |x| Some(x));
    let flags = (flags as u32).try_into().map_or(None, |x| Some(x));

    // prepare basic info
    let mut si_block = SIBlock::new(
        source_type,
        bus_type,
        Default::default(),
        Default::default(),
        Default::default(),
        flags,
    );
    si_block.id = id;
    si_block.block_size = block_size as u64;
    si_block.links_count = link_count;

    Ok((input, si_block))
}

/// CGBlock basic info, fix length(32bytes)
pub(crate) fn cg_block_basic(
    input: &[u8],
    id: String,
    block_size: i64,
    link_count: u64,
) -> IResult<&[u8], CGBlock> {
    let (
        input,
        (record_id, record_count, channel_group_flags, path_separator, _, record_size, inval_size),
    ) = tuple((le_u64, le_u64, le_u16, le_u16, take(4u32), le_u32, le_u32))(input)?;

    // prepare basic info
    let mut cg_block = CGBlock::new(Default::default());
    cg_block.id = id;
    cg_block.block_size = block_size as u64;
    cg_block.links_count = link_count;
    cg_block.record_id = record_id;
    cg_block.record_count = record_count;
    // conver channel_groups_flags to coresponding enum
    let channel_group_flags = (channel_group_flags as u32)
        .try_into()
        .map_or(None, |x| Some(x));
    cg_block.flags = channel_group_flags;
    cg_block.path_separator = std::char::from_u32(path_separator as u32).unwrap();
    cg_block.record_size = record_size;
    cg_block.inval_size = inval_size;

    Ok((input, cg_block))
}

/// CNBlock basic info, fix length(76bytes)
pub(crate) fn cn_block_basic(
    input: &[u8],
    id: String,
    block_size: i64,
    link_count: u64,
) -> IResult<&[u8], CNBlock> {
    let (
        input,
        (
            channel_type,
            sync_type,
            signal_type,
            bit_offset,
            add_offset,
            bits_count,
            channel_flags,
            inval_bit_pos,
            precision,
            _,
            min_raw,
            max_raw,
            min,
            max,
            min_ex,
            max_ex,
        ),
    ) = tuple((
        le_u8,
        le_u8,
        le_u8,
        le_u8,
        le_u32,
        le_u32,
        le_u32,
        le_u32,
        le_u8,
        take(3u32),
        le_f64,
        le_f64,
        le_f64,
        le_f64,
        le_f64,
        le_f64,
    ))(input)?;
    let channel_type = (channel_type as u32).try_into().map_or(None, |x| Some(x));
    let sync_type = (sync_type as u32).try_into().map_or(None, |x| Some(x));
    let signal_type = (signal_type as u32).try_into().map_or(None, |x| Some(x));

    // prepare basic info
    let mut cn_block = CNBlock::new(
        signal_type,
        channel_type,
        Default::default(),
        Default::default(),
        0,
        0,
        bits_count,
        sync_type,
        ChannelFlags::from_bits_truncate(channel_flags),
        inval_bit_pos,
        precision,
    );
    cn_block.id = id;
    cn_block.block_size = block_size as u64;
    cn_block.links_count = link_count;
    cn_block.bit_offset = bit_offset as u16;
    cn_block.add_offset = add_offset;
    if (cn_block.flags & ChannelFlags::ValueRangeValid) > ChannelFlags::None {
        cn_block.min_raw = min_raw;
        cn_block.max_raw = max_raw;
    }
    if (cn_block.flags & ChannelFlags::LimitRangeValid) > ChannelFlags::None {
        cn_block.min = min;
        cn_block.max = max;
    }
    if (cn_block.flags & ChannelFlags::ExtendedLimitRangeValid) > ChannelFlags::None {
        cn_block.min_ex = min_ex;
        cn_block.max_ex = max_ex;
    }

    Ok((input, cn_block))
}

/// CCBlock basic info, fix length(24bytes)
pub(crate) fn cc_block_basic(
    input: &[u8],
    id: String,
    block_size: i64,
    link_count: u64,
) -> IResult<&[u8], CCBlock> {
    let (input, (conversion_type, precision, conversion_flags, ref_count, tab_size, min, max)) =
        tuple((le_u8, le_u8, le_u16, le_u16, le_u16, le_f64, le_f64))(input)?;
    let flags = ConversionFlags::from_bits_truncate(conversion_flags);
    let mut max_ = f64::NAN;
    let mut min_ = f64::NAN;
    if (flags & ConversionFlags::LimitRangeValid) > ConversionFlags::None {
        max_ = max;
        min_ = min;
    }
    let mut conv_type = None;
    match conversion_type {
        0 => {
            conv_type = None;
        }
        1 => {
            conv_type = Some(ConversionType::ParametricLinear);
        }
        2 => {
            conv_type = Some(ConversionType::Rational);
        }
        3 => {
            conv_type = Some(ConversionType::TextFormula);
        }
        4 => {
            conv_type = Some(ConversionType::TabInt);
        }
        5 => {
            conv_type = Some(ConversionType::Tab);
        }
        6 => {
            conv_type = Some(ConversionType::TabRange);
        }
        7 => {
            conv_type = Some(ConversionType::TextTable);
        }
        8 => {
            conv_type = Some(ConversionType::TextRange);
        }
        9 => {
            conv_type = Some(ConversionType::TextToValue);
        }
        10 => {
            conv_type = Some(ConversionType::TextToText);
        }
        _ => {
            conv_type = None;
        }
    }

    // prepare basic info
    let mut cc_block = CCBlock::new(conv_type, Default::default(), min_, max_);
    cc_block.id = id;
    cc_block.block_size = block_size as u64;
    cc_block.links_count = link_count;

    Ok((input, cc_block))
}

/// FHBlock basic info, fix length(16bytes)
pub(crate) fn fh_block_basic(
    input: &[u8],
    id: String,
    block_size: i64,
    link_count: u64,
) -> IResult<&[u8], FHBlock> {
    let (input, (timestamp, utc_offset, dst_offset, flags, _)) =
        tuple((le_u64, le_i16, le_i16, le_u8, take(3u32)))(input)?;
    // convert flags to enum TimeFlagsType
    let flags = (flags as u32).try_into().map_or(None, |x| Some(x));

    // prepare basic info
    let mut fh_block = FHBlock::new(
        Default::default(),
        Default::default(),
        timestamp,
        utc_offset,
        dst_offset,
        flags,
    );
    fh_block.id = id;
    fh_block.block_size = block_size as u64;
    fh_block.links_count = link_count;

    Ok((input, fh_block))
}

/// EVBlock basic info, fix length(32bytes)
pub(crate) fn ev_block_basic(
    input: &[u8],
    id: String,
    block_size: i64,
    link_count: u64,
) -> IResult<&[u8], EVBlock> {
    let (
        input,
        (
            evt_type,
            sync_type,
            range,
            cause_type,
            flags,
            _,
            creator_index,
            sync_basevalue,
            sync_factor,
        ),
    ) = tuple((
        le_u8,
        le_u8,
        le_u8,
        le_u8,
        le_u8,
        take(9u32),
        le_u16,
        le_i64,
        le_f64,
    ))(input)?;
    // convert flags to enum
    let evt_type = (evt_type as u32).try_into().map_or(None, |x| Some(x));
    let sync_type = (sync_type as u32).try_into().map_or(None, |x| Some(x));
    let cause_type = (cause_type as u32).try_into().map_or(None, |x| Some(x));
    let range = (range as u32).try_into().map_or(None, |x| Some(x));
    let flags = (flags as u32).try_into().map_or(None, |x| Some(x));

    // prepare basic info
    let mut ev_block = EVBlock::new();
    ev_block.id = id;
    ev_block.block_size = block_size as u64;
    ev_block.links_count = link_count;

    ev_block.evt_type = evt_type;
    ev_block.sync = sync_type;
    ev_block.range = range;
    ev_block.cause = cause_type;
    ev_block.flags = flags;
    ev_block.creator_index = creator_index;
    ev_block.sync_base_val = sync_basevalue;
    ev_block.sync_factor = sync_factor;

    Ok((input, ev_block))
}

/// ATBlock basic info, fix length(4bytes)
pub(crate) fn at_block_basic(
    input: &[u8],
    id: String,
    block_size: i64,
    link_count: u64,
) -> IResult<&[u8], ATBlock> {
    let (input, (attachment_flags, creator_index, _)) = tuple((le_u16, le_u16, le_u32))(input)?;
    // convert flags to enum
    let attachment_flags = AttachmentFlags::from_bits_truncate(attachment_flags);

    // prepare basic info
    let mut at_block = ATBlock::new();
    at_block.id = id;
    at_block.block_size = block_size as u64;
    at_block.links_count = link_count;
    at_block.attachment_flags = attachment_flags;
    at_block.creator_index = creator_index;

    Ok((input, at_block))
}

/// CHBlock basic info, fix length(8bytes)
pub(crate) fn ch_block_basic(
    input: &[u8],
    id: String,
    block_size: i64,
    link_count: u64,
) -> IResult<&[u8], (u32, CHBlock)> {
    let (input, (dependencies_count, hierarchy_type, _)) =
        tuple((le_u32, le_u8, take(3u32)))(input)?;
    // convert flags to enum
    let hierarchy_type = (hierarchy_type as u32).try_into().map_or(None, |x| Some(x));

    // prepare basic info
    let mut ch_block = CHBlock::new();
    ch_block.id = id;
    ch_block.block_size = block_size as u64;
    ch_block.links_count = link_count;
    ch_block.hierarchy_type = hierarchy_type;

    Ok((input, (dependencies_count, ch_block)))
}

/// SRBlock basic info, fix length(8bytes)
pub(crate) fn sr_block_basic(
    input: &[u8],
    id: String,
    block_size: i64,
    link_count: u64,
) -> IResult<&[u8], SRBlock> {
    let (input, (reduced_samples_number, time_interval_len, flags)) =
        tuple((le_u64, le_f64, le_u8))(input)?;
    // convert flags to enum
    let flags = (flags as u32).try_into().map_or(None, |x| Some(x));

    // prepare basic info
    let mut sr_block = SRBlock::new();
    sr_block.id = id;
    sr_block.block_size = block_size as u64;
    sr_block.links_count = link_count;
    sr_block.time_interval_len = time_interval_len;
    sr_block.reduced_samples_number = reduced_samples_number;
    sr_block.flags = flags;

    Ok((input, sr_block))
}
