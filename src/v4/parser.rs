use crate::ByteOrder;

use super::IDBlock;
use nom::bytes::complete::take;
use nom::number::complete::le_u16;
use nom::sequence::tuple;
use nom::IResult;

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
