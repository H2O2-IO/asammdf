use crate::ByteOrder;

use super::{DGBlock, DLBlock, DTBlock, HDBlock, HLBlock, IDBlock, DZBlock, SIBlock};
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
    let (input, (
        block_type_bytes,
        zip_type,
        _,
        zip_parameter,
        size,
        length,
    )) =
        tuple((
            take(2u32),
            le_u8,
            take(1u32),
            le_u32,
            le_i64,
            le_u64,
    ))(input)?;

    // conver zip_type
    let zip_type = (zip_type as u32).try_into().map_or(None, |x| Some(x));
    let block_type = String::from_utf8_lossy(block_type_bytes).to_string();

    // prepare basic info
    let mut dz_block = DZBlock::new(length,size,zip_type,zip_parameter,Some(block_type));
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
    let (input, (
        source_type,
        bus_type,
        flags,
    )) =
        tuple((
            le_u8,
            le_u8,
            le_u8,
        ))(input)?;
    // convert source_type,bus_type,flags to coresponding enum
    let source_type = (source_type as u32).try_into().map_or(None, |x| Some(x));
    let bus_type = (bus_type as u32).try_into().map_or(None, |x| Some(x));
    let flags = (flags as u32).try_into().map_or(None, |x| Some(x));

    // prepare basic info
    let mut si_block = SIBlock::new(source_type, bus_type, Default::default(),Default::default(),Default::default(),flags);
    si_block.id = id;
    si_block.block_size = block_size as u64;
    si_block.links_count = link_count;

    Ok((input, si_block))
}