use crate::ByteOrder;

use super::{HDBlock, IDBlock};
use chrono::{DateTime, Local};
use nom::bytes::complete::take;
use nom::number::complete::{le_i16, le_u16, le_u32, le_u64};
use nom::sequence::tuple;
use nom::IResult;

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

    idblock.file_id = String::from_utf8(fileid.to_vec()).unwrap();
    idblock.format_id = String::from_utf8(formatid.to_vec()).unwrap();
    idblock.program_id = String::from_utf8(programid.to_vec()).unwrap();

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
            id,
            block_size,
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
        take(2u32),
        le_u16,
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

    header_block.id = String::from_utf8(id.to_vec()).unwrap();
    header_block.block_size = block_size as _;
    header_block.link_first_file_group = link_first_file_group;
    header_block.link_file_comment_txt = link_first_comment_txt;
    header_block.link_program_block = link_program_block;
    header_block.date = String::from_utf8(date_bytes.to_vec()).unwrap();
    header_block.time = String::from_utf8(time_bytes.to_vec()).unwrap();
    header_block.author = String::from_utf8(author_bytes.to_vec()).unwrap();
    header_block.author = String::from_utf8(organization_bytes.to_vec()).unwrap();
    header_block.author = String::from_utf8(project_bytes.to_vec()).unwrap();
    header_block.author = String::from_utf8(subject_bytes.to_vec()).unwrap();
    Ok((input, header_block))
}

pub(crate) fn read_le_u64(input: &[u8]) -> IResult<&[u8], u64> {
    Ok(le_u64(input)?)
}

pub(crate) fn read_le_i16(input: &[u8]) -> IResult<&[u8], i16> {
    Ok(le_i16(input)?)
}

pub(crate) fn read_le_u16(input: &[u8]) -> IResult<&[u8], u16> {
    Ok(le_u16(input)?)
}

pub(crate) fn read_str(input: &[u8], count: u32) -> IResult<&[u8], String> {
    let (input, result) = take(count)(input)?;
    let result = String::from_utf8(result.to_vec()).unwrap();
    Ok((input, result))
}
