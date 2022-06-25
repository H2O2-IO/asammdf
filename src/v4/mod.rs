use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Cursor, Read, Seek, SeekFrom};
use std::ops::Range;
use std::sync::Arc;
use std::vec;

use asammdf_derive::{
    basic_object, channel_conversion_object, channel_group_object, channel_object, comment_object,
    data_group_object, id_object, mdf_object, normal_object_v4,
};
use asammdf_derive::{IDObject, MDFObject, PermanentBlock};
use chrono::{DateTime, Local};
use itertools::Zip;
use tempfile::tempfile;

use self::parser::{
    block_base, cc_block_basic, cg_block_basic, cn_block_basic, dg_block_basic, dl_block_basic,
    dz_block_basic, fh_block_basic, hd_block_basic, hl_block_basic, si_block_basic,
};
use super::SpecVer;
use super::UnfinalizedFlagsType;
use crate::misc::helper::{read_le_f64, read_le_i64, read_n_le_f64};
use crate::misc::transform_params;
use crate::{
    BlockId, ChannelType, ConversionType, IDObject, SignalType, SyncType, TimeFlagsType,
    TimeQualityType,
};
use crate::{ByteOrder, MDFErrorKind, MDFFile, MDFObject, RecordIDType};
use crate::{CNObject, PermanentBlock};
use bitflags::bitflags;

mod parser;

macro_rules! enum_u32_convert {
    ($(#[$meta:meta])* $vis:vis enum $name:ident {
        $($(#[$vmeta:meta])* $vname:ident $(= $val:expr)?,)*
    }) => {
        $(#[$meta])*
        $vis enum $name {
            $($(#[$vmeta])* $vname $(= $val)?,)*
        }

        impl std::convert::TryFrom<u32> for $name {
            type Error = ();

            fn try_from(v: u32) -> Result<Self, Self::Error> {
                match v {
                    $(x if x == $name::$vname as u32 => Ok($name::$vname),)*
                    _ => Err(()),
                }
            }
        }
    }
}
enum_u32_convert! {
#[derive(Clone, Copy, Debug)]
/// Zip type of a compression block
pub enum ZipType {
    Deflate,
    TransposeAndDeflate,
}
}

#[derive(Clone, Copy, Debug)]
pub enum SRFlags {
    InvalidationBytes = 1,
}

enum_u32_convert! {
#[derive(Clone, Copy, Debug)]
pub enum Source {
    ECU,
    Bus,
    IO,
    Tool,
    User,
}
}
enum_u32_convert! {
#[derive(Clone, Copy, Debug)]
pub enum SourceFlags {
    SimulatedSource,
}
}

#[derive(Clone, Copy, Debug)]
pub enum RangeType {
    Point,
    BeginRange,
    EndRange,
}

#[derive(Clone, Copy, Debug)]
/// hierarchy type
pub enum Hierarchy {
    Group,
    Function,
    Structure,
    MapList,
    InMeasurement,
    OutMeasurement,
    LocMeasurement,
    DefCharacteristic,
    RefCharacteristic,
}

#[derive(Clone, Copy, Debug)]
pub enum Event {
    Recording,
    RecordingInterrupt,
    AcquisitionInterrupt,
    StartRecordingTrigger,
    StopRecordingTrigger,
    Trigger,
    Marker,
}

#[derive(Clone, Copy, Debug)]
pub enum EventFlags {
    PostProcessing = 1,
}

enum_u32_convert! {
#[derive(Clone, Copy, Debug)]
pub enum DataBlockFlags {
    EqualLength = 1,
}
}

bitflags! {
pub struct ConversionFlags:u16 {
    const None = 0;
    const PrecisionValid = 1;
    const LimitRangeValid = 2;
    const StatusString = 4;
}
}

enum_u32_convert! {
#[derive(Clone, Copy, Debug)]
pub enum ChannelGroupFlags {
    VariableLenSignalData = 1,
    BusEvent = 2,
    PlainBusEvent = 4,
}
}

bitflags! {
pub struct ChannelFlags:u32 {
    const None = 0;
    const Invalid = 1;
    const InvalBytesValid = 2;
    const PrecisionValid = 4;
    const ValueRangeValid = 8;
    const LimitRangeValid = 16;
    const ExtendedLimitRangeValid = 32;
    const DiscreteValue = 64;
    const Calibration = 128;
    const Calculated = 256;
    const Virtual = 512;
    const BusEvent = 1024;
    const Montonous = 2048;
    const DefaultXAxis = 4096;
}
}

#[derive(Clone, Copy, Debug)]
pub enum Cause {
    Other,
    Error,
    Tool,
    Script,
    User,
}

enum_u32_convert! {
#[derive(Clone, Copy, Debug)]
pub enum BusType {
    CAN,
    LIN,
    MOST,
    FLEXRAY,
    KLINE,
    Ethernet,
    USB,
}
}

#[derive(Clone, Copy, Debug)]
pub enum AttachmentFlags {
    EmbeddedData = 1,
    CompressedEmbbeddedData = 2,
    MD5ChecksumValid = 4,
}

#[mdf_object]
#[basic_object]
#[id_object]
#[derive(Debug, IDObject, Clone, PermanentBlock)]
pub struct IDBlock {
    pub format_id: String,
    pub program_id: String,
}

impl MDFObject for IDBlock {
    fn block_size(&self) -> u64 {
        self.block_size
    }

    fn name(&self) -> String {
        "ID".to_string()
    }
}

impl IDBlock {
    /// Create a v4::IDBlock with version default to `410`, spec_type default to `SpecVer::V4`
    fn new(program_id: String) -> IDBlock {
        IDBlock {
            format_id: "4.10    ".to_string(),
            program_id,
            block_size: 64,
            file_id: "MDF     ".to_string(),
            spec_type: SpecVer::V4,
            unfinalized_flags: None,
            version: 410,
            custom_flags: 0,
            name: "ID".to_string(),
            block_id: None,
        }
    }

    /// parse a IDBlock from a 64byte u8 slice
    pub(crate) fn parse(input: &[u8]) -> Option<IDBlock> {
        parser::id_block(input).map_or(None, |x| Some(x.1))
    }
}

#[comment_object]
#[normal_object_v4]
#[basic_object]
#[derive(Debug, PermanentBlock)]
pub struct HDBlock {
    pub dst_offset: i16,
    pub flags: Option<TimeFlagsType>,
    pub start_angle: f64,
    pub start_distance: f64,
    pub time_quality: Option<TimeQualityType>,
    pub utc_offset: i16,
    timestamp: u64,
    pub(crate) link_dg_start: i64,
    pub(crate) link_fh_start: i64,
    pub(crate) link_ch_start: i64,
    pub(crate) link_atb_start: i64,
    pub(crate) link_ev_start: i64,
    pub(crate) link_comment: i64,
}

impl MDFObject for HDBlock {
    fn block_size(&self) -> u64 {
        self.block_size
    }

    fn name(&self) -> String {
        "Header".to_string()
    }
}

impl HDBlock {
    pub fn new(
        date_time: DateTime<Local>,
        time_quality_type: Option<TimeQualityType>,
        fn_username: String,
        fn_text: String,
        comment: String,
    ) -> HDBlock {
        HDBlock {
            dst_offset: 0,
            flags: Some(TimeFlagsType::OffsetsValid),
            start_angle: f64::NAN,
            start_distance: f64::NAN,
            time_quality: time_quality_type,
            utc_offset: (Local::now().offset().local_minus_utc() as f64 / 3600.0f64).round() as i16,
            comment,
            id: "HD".to_string(),
            block_size: 104,
            links_count: 6,
            block_id: None,
            timestamp: date_time.timestamp() as u64,
            link_dg_start: 0,
            link_fh_start: 0,
            link_ch_start: 0,
            link_atb_start: 0,
            link_ev_start: 0,
            link_comment: 0,
        }
    }

    pub(crate) fn parse(byte_order: ByteOrder, instance: &mut MDFFile) -> Result<(), MDFErrorKind> {
        // currently, file position inside instance is 64
        let mut buf_reader = instance.get_buf_reader()?;
        let pos = buf_reader.stream_position().unwrap();
        // read the block basic info
        let (id, block_size, links, next_pos) = read_v4_basic_info(64, instance)?;
        let (
            mut link_dg_start,
            mut link_fh_start,
            mut link_ch_start,
            mut link_atb_start,
            mut link_ev_start,
            mut link_comment,
        ) = (0, 0, 0, 0, 0, 0);
        // get all links
        for (i, val) in links.iter().enumerate() {
            match i {
                0 => link_dg_start = *val,
                1 => link_fh_start = *val,
                2 => link_ch_start = *val,
                3 => link_atb_start = *val,
                4 => link_ev_start = *val,
                5 => link_comment = *val,
                _ => {}
            }
        }
        // parse basic info and get HDBlock instance
        buf_reader.seek(SeekFrom::Start(next_pos)).unwrap();
        let mut buf = [0; 31];
        buf_reader.read_exact(&mut buf).unwrap();
        let mut hd_block = hd_block_basic(&buf, id, block_size, links.len() as u64)
            .unwrap()
            .1;
        // set links info of hd_block
        hd_block.link_dg_start = link_dg_start;
        hd_block.link_fh_start = link_fh_start;
        hd_block.link_ch_start = link_ch_start;
        hd_block.link_atb_start = link_atb_start;
        hd_block.link_ev_start = link_ev_start;
        hd_block.link_comment = link_comment;

        // parse comment block
        let rdsd_block = RDSDBlock::parse(byte_order, link_comment as u64, instance)?;
        // set rdsd_block's data as hd_block's comment
        hd_block.comment = String::from_utf8_lossy(&rdsd_block.data).to_string();

        // store hd_block to instance's arena
        let hd_id = instance.arena.new_node(Box::new(hd_block));
        // set hd_id to instance
        instance.header = Some(hd_id);
        // set hd_block as id_block's child
        instance
            .id
            .unwrap()
            .checked_append(hd_id, &mut instance.arena);

        // parse dg blocks
        let mut link_dg = link_dg_start;
        while link_dg > 0 {
            break;
        }
        // parse fh blocks
        let mut link_fh = link_fh_start;
        while link_fh > 0 {
            break;
        }
        // parse ch blocks
        let mut link_ch = link_ch_start;
        while link_ch > 0 {
            break;
        }
        // parse atb blocks
        let mut link_atb = link_atb_start;
        while link_atb > 0 {
            break;
        }
        // parse ev blocks
        let mut link_ev = link_ev_start;
        while link_ev > 0 {
            break;
        }

        buf_reader.seek(SeekFrom::Start(pos)).unwrap();
        todo!()
    }
}

/// read basic block info of a MDF v4 block
fn read_v4_basic_info(
    position: u64,
    mdf_file: &mut MDFFile,
) -> Result<(String, i64, Vec<i64>, u64), MDFErrorKind> {
    let mut buf_reader = mdf_file.get_buf_reader()?;
    let pos = buf_reader.stream_position().unwrap();
    buf_reader.seek(SeekFrom::Start(position)).unwrap();

    let mut links_list = Vec::new();
    let mut block = [0; 24];
    buf_reader.read_exact(&mut block).unwrap();

    let (id, block_size, links_count) = block_base(&block).unwrap().1;
    // push links to links_list
    for _ in 0..links_count {
        let mut link = [0; 8];
        buf_reader.read_exact(&mut link).unwrap();
        links_list.push(read_le_i64(&link).unwrap().1);
    }
    buf_reader.seek(SeekFrom::Start(pos)).unwrap();
    Ok((id, block_size, links_list, position + 24 + 8 * links_count))
}

#[comment_object]
#[normal_object_v4]
#[basic_object]
#[derive(Debug, PermanentBlock)]
pub struct ATBlock {
    pub attachment_flags: Option<AttachmentFlags>,
    pub creator_index: u16,
    pub filename: String,
    pub mime_type: String,
}

impl MDFObject for ATBlock {
    fn block_size(&self) -> u64 {
        self.block_size
    }

    fn name(&self) -> String {
        self.id.clone()
    }
}

#[normal_object_v4]
#[comment_object]
pub struct CHBlock {
    // chblocks?
    // dependency type?u
    pub hierarchy_type: Hierarchy,
}

/// MDFv4 Block
#[normal_object_v4]
#[basic_object]
#[derive(Debug, PermanentBlock)]
pub struct RDSDBlock {
    pub data: Vec<u8>,
}

impl MDFObject for RDSDBlock {
    fn block_size(&self) -> u64 {
        self.block_size
    }

    fn name(&self) -> String {
        self.id.clone()
    }
}

impl RDSDBlock {
    pub fn new(data: Vec<u8>, id: String) -> Self {
        RDSDBlock {
            block_size: 24 + data.len() as u64,
            data,
            id: id,
            links_count: 0,
            block_id: None,
        }
    }

    pub fn parse(
        byte_order: ByteOrder,
        link_id: u64,
        instance: &mut MDFFile,
    ) -> Result<RDSDBlock, MDFErrorKind> {
        let mut buf_reader = instance.get_buf_reader()?;
        let pos = buf_reader.stream_position().unwrap();
        // seek to link_id position
        buf_reader.seek(SeekFrom::Start(link_id)).unwrap();
        // read the block basic info
        let (id, block_size, links, next_pos) = read_v4_basic_info(link_id, instance)?;
        let rdsd_block;
        if block_size - 24 > 0 {
            // go to next pos
            buf_reader.seek(SeekFrom::Start(next_pos)).unwrap();
            let mut data = vec![0; block_size as usize - 24];
            buf_reader.read_exact(&mut data).unwrap();
            rdsd_block = RDSDBlock::new(data, id);
        } else {
            rdsd_block = RDSDBlock::new(vec![], id);
        }
        // restore stream position
        buf_reader.seek(SeekFrom::Start(pos)).unwrap();
        Ok(rdsd_block)
    }
}

#[normal_object_v4]
#[comment_object]
#[data_group_object]
#[basic_object]
#[derive(Debug, PermanentBlock)]
pub struct DGBlock {
    pub(crate) link_cg_start: u64,
    pub(crate) link_comment: u64,
    pub(crate) link_next_block: u64,
    pub(crate) link_data_block: u64,
    pub(crate) zip_reader: Option<ZippedReader>,
}

impl MDFObject for DGBlock {
    fn block_size(&self) -> u64 {
        self.block_size
    }

    fn name(&self) -> String {
        "Data group".to_string()
    }
}
#[derive(Debug)]
pub struct ZippedReader {
    pub tmp_file: File,
}

impl ZippedReader {
    pub fn new() -> Self {
        ZippedReader {
            tmp_file: tempfile().unwrap(),
        }
    }
    pub fn get_buf_reader(&self) -> Result<BufReader<File>, MDFErrorKind> {
        let x = self
            .tmp_file
            .try_clone()
            .map_err(|x| MDFErrorKind::IOError(x))?;
        Ok(BufReader::new(x))
    }

    pub fn get_position(&mut self) -> u64 {
        self.tmp_file.stream_position().unwrap()
    }
}

struct CRCHelper {
    pub high: i32,
    pub low: i32,
}

impl CRCHelper {
    pub fn new() -> Self {
        CRCHelper { high: 0, low: 1 }
    }
    pub fn get_crc_val(&self) -> i32 {
        self.high * 65535 + self.low
    }
    pub fn build_crc(&mut self, data: &[u8], offset: i32, length: i32) {
        for i in 0..length {
            self.low = (self.low + data[(offset + i) as usize] as i32) % 65521;
            self.high = (self.high + self.low) % 65521;
        }
    }
}

impl DGBlock {
    pub fn new() -> Self {
        DGBlock {
            block_size: 64,
            id: "DG".to_string(),
            links_count: 4,
            block_id: None,
            comment: Default::default(),
            record_id_type: None,
            link_cg_start: 0,
            link_comment: 0,
            link_data_block: 0,
            link_next_block: 0,
            zip_reader: None,
        }
    }
    /// parse a DGBlock from file position at link_id, then add it to instance as child of parent_id
    /// return the postion of next block
    pub fn parse(
        byte_order: ByteOrder,
        link_id: u64,
        parent_id: BlockId,
        instance: &mut MDFFile,
    ) -> Result<u64, MDFErrorKind> {
        let mut buf_reader = instance.get_buf_reader()?;
        let pos = buf_reader.stream_position().unwrap();
        // seek to link_id position
        buf_reader.seek(SeekFrom::Start(link_id)).unwrap();
        // read the block basic info
        let (id, block_size, links, next_pos) = read_v4_basic_info(link_id, instance)?;
        // preread data link block
        let mut link_data_block = 0;
        // get links out
        let mut link_cg_start = 0;
        let mut link_comment = 0;
        let mut link_next_block = 0;
        // get all links
        for (i, val) in links.iter().enumerate() {
            match i {
                0 => {
                    // get link to CG block
                    link_next_block = *val as u64;
                }
                1 => {
                    // get link to CG block
                    link_cg_start = *val as u64;
                }
                2 => {
                    // get link to CG block
                    link_data_block = *val as u64;
                }
                3 => {
                    // get the comment block
                    link_comment = *val as u64;
                }
                _ => {}
            }
        }
        while link_data_block > 0 {
            // read another basic block info out
            let (id, block_size, links, next_pos) = read_v4_basic_info(link_data_block, instance)?;
            // match id type
            match id.as_str() {
                "RD" => {
                    let rd_block = RDSDBlock::new(vec![], id);
                    link_data_block = 0;
                }
                "HL" => {
                    link_data_block = 0;
                    // get the HL block
                }
                "DT" => {
                    link_data_block = 0;
                    // get the DT block
                }
                "DL" => {
                    // get the DL block
                }
                "DZ" => {
                    link_data_block = 0;
                    // get the DZ block
                }
                _ => {
                    link_data_block = 0;
                }
            }
        }

        // go to next pos, and construct dg block
        buf_reader.seek(SeekFrom::Start(next_pos)).unwrap();
        let mut data = vec![0; 1];
        buf_reader.read_exact(&mut data).unwrap();
        let mut dg_block = dg_block_basic(&data, id, block_size, links.len() as u64)
            .unwrap()
            .1;
        dg_block.link_cg_start = link_cg_start;
        dg_block.link_comment = link_comment;
        dg_block.link_data_block = link_data_block;
        dg_block.link_next_block = link_next_block;
        // parse comment block
        if link_comment > 0 {
            let comment_block = RDSDBlock::parse(byte_order, link_comment, instance)?;
            dg_block.comment = String::from_utf8_lossy(&comment_block.data).to_string();
        }

        // save dg block to instance's arena
        let dg_id = instance.arena.new_node(Box::new(dg_block));
        // add dg block to parent
        parent_id
            .checked_append(dg_id, &mut instance.arena)
            .unwrap();

        instance
            .get_mut_node_by_id::<DGBlock>(dg_id)
            .unwrap()
            .block_id = Some(dg_id);
        // parse cg blocks
        let mut cg_link = link_cg_start;
        while cg_link > 0 {
            break;
        }

        // restore stream position
        buf_reader.seek(SeekFrom::Start(pos)).unwrap();
        Ok(link_next_block)
    }
}

#[normal_object_v4]
#[basic_object]
#[derive(Debug, PermanentBlock)]
pub struct HLBlock {
    pub zip_type: Option<ZipType>,
    pub flags: Option<DataBlockFlags>,
    pub(crate) link_dl_start: u64,
}

impl MDFObject for HLBlock {
    fn block_size(&self) -> u64 {
        self.block_size
    }

    fn name(&self) -> String {
        self.id.clone()
    }
}

impl<'a> HLBlock {
    pub fn new(zip_type: Option<ZipType>, flags: Option<DataBlockFlags>) -> Self {
        HLBlock {
            zip_type,
            flags,
            id: "HL".to_string(),
            block_size: 40,
            links_count: 1,
            block_id: None,
            link_dl_start: 0,
        }
    }

    pub fn parse(
        byte_order: ByteOrder,
        link_id: u64,
        parent_id: BlockId,
        instance: &mut MDFFile,
    ) -> Result<(), MDFErrorKind> {
        let mut buf_reader = instance.get_buf_reader()?;
        let pos = buf_reader.stream_position().unwrap();
        // seek to link_id position
        buf_reader.seek(SeekFrom::Start(link_id)).unwrap();
        // read the block basic info
        let (id, block_size, links, next_pos) = read_v4_basic_info(link_id, instance)?;
        let mut link_dl_start = 0;
        // get all links
        for (i, val) in links.iter().enumerate() {
            match i {
                0 => {
                    // get link to CG block
                    link_dl_start = *val as u64;
                }
                _ => {}
            }
        }
        // go to next pos, and construct dg block
        buf_reader.seek(SeekFrom::Start(next_pos)).unwrap();

        let mut data = vec![0; 3];
        buf_reader.read_exact(&mut data).unwrap();
        let mut hl_block = hl_block_basic(&data, id, block_size, links.len() as u64)
            .unwrap()
            .1;

        hl_block.link_dl_start = link_dl_start;

        // save hl block to instance's arena
        let hl_id = instance.arena.new_node(Box::new(hl_block));
        // add hl block to parent
        parent_id
            .checked_append(hl_id, &mut instance.arena)
            .unwrap();

        instance
            .get_mut_node_by_id::<HLBlock>(hl_id)
            .unwrap()
            .block_id = Some(hl_id);
        // parse dl blocks
        let mut dl_link = link_dl_start;
        while dl_link > 0 {
            break;
        }

        // restore stream position
        buf_reader.seek(SeekFrom::Start(pos)).unwrap();

        Ok(())
    }

    pub fn dlblocks(&self, mdf_file: &'a MDFFile) -> Option<Vec<&'a DLBlock>> {
        todo!()
    }
}

#[normal_object_v4]
#[basic_object]
#[derive(Debug, PermanentBlock)]
pub struct DLBlock {
    pub count: u32,
    pub equal_length: u64,
    pub flags: Option<DataBlockFlags>,
    pub(crate) link_dl_data: Vec<i64>,
    pub(crate) link_dl_next: u64,
}

impl MDFObject for DLBlock {
    fn block_size(&self) -> u64 {
        self.block_size
    }

    fn name(&self) -> String {
        self.id.clone()
    }
}

impl DLBlock {
    pub fn new(flags: Option<DataBlockFlags>, count: u32) -> DLBlock {
        DLBlock {
            count,
            equal_length: 0,
            flags,
            link_dl_data: vec![],
            link_dl_next: 0,
            id: "DL".to_string(),
            block_size: 48 + 8 * count as u64,
            links_count: 1 + count as u64,
            block_id: None,
        }
    }

    /// get blocks with type DTBlock and DZBlocks
    pub fn data_blocks<T>(&self, mdf_file: &MDFFile) -> Option<Vec<&T>> {
        todo!()
    }

    pub fn parse(
        byte_order: ByteOrder,
        link_id: u64,
        parent_id: BlockId,
        instance: &mut MDFFile,
    ) -> Result<u64, MDFErrorKind> {
        let mut buf_reader = instance.get_buf_reader()?;
        let pos = buf_reader.stream_position().unwrap();
        // seek to link_id position
        buf_reader.seek(SeekFrom::Start(link_id)).unwrap();
        // read the block basic info
        let (id, block_size, links, next_pos) = read_v4_basic_info(link_id, instance)?;
        let mut link_dl_next = 0;
        // get all links
        for (i, val) in links.iter().enumerate() {
            match i {
                0 => {
                    // get link to CG block
                    link_dl_next = *val as u64;
                }
                _ => {}
            }
        }

        // go to next pos, and construct dl block
        buf_reader.seek(SeekFrom::Start(next_pos)).unwrap();

        let mut data = vec![0; 1];
        buf_reader.read_exact(&mut data).unwrap();
        let mut dl_block = dl_block_basic(&data, id, block_size, links.len() as u64)
            .unwrap()
            .1;
        dl_block.link_dl_next = link_dl_next;

        // save dl block to instance's arena
        let dl_id = instance.arena.new_node(Box::new(dl_block));
        // add dl block to parent
        parent_id
            .checked_append(dl_id, &mut instance.arena)
            .unwrap();

        instance
            .get_mut_node_by_id::<DLBlock>(dl_id)
            .unwrap()
            .block_id = Some(dl_id);

        // parse DT and DZ blocks
        for (i, link) in links.iter().skip(1).enumerate() {
            // if link > 0, is a valid link
            if *link > 0 {
                // read basic info out
                let (id, block_size, links, next_pos) = read_v4_basic_info(*link as u64, instance)?;
                match id.as_str() {
                    "DT" => {
                        // parse a DT Block
                        let dt_id = DTBlock::parse(byte_order, *link as u64, dl_id, instance)?;
                    }
                    "DZ" => {
                        // parse a DZ block
                        let (dz_id, data) =
                            DZBlock::parse(byte_order, *link as u64, dl_id, instance)?;
                    }
                    _ => {
                        // do nothing
                    }
                }
            }
        }

        // restore stream position
        buf_reader.seek(SeekFrom::Start(pos)).unwrap();

        Ok(link_dl_next)
    }
}

fn read_one_dt_block(block_id: BlockId, mdf_file: &mut MDFFile, position: u64) {}

#[normal_object_v4]
#[basic_object]
#[derive(Debug, PermanentBlock)]
pub struct DTBlock {
    pub base_position: i64,
}

impl MDFObject for DTBlock {
    fn block_size(&self) -> u64 {
        self.block_size
    }

    fn name(&self) -> String {
        self.id.to_string()
    }
}

impl DTBlock {
    pub fn size(&self) -> u64 {
        self.block_size() - 24
    }

    pub fn new(size: i64) -> Self {
        DTBlock {
            base_position: 0,
            id: "DT".to_string(),
            block_size: 24 + size as u64,
            links_count: 0,
            block_id: None,
        }
    }

    pub fn parse(
        byte_order: ByteOrder,
        link_id: u64,
        parent_id: BlockId,
        instance: &mut MDFFile,
    ) -> Result<(BlockId), MDFErrorKind> {
        let mut buf_reader = instance.get_buf_reader()?;
        let pos = buf_reader.stream_position().unwrap();
        // seek to link_id position
        buf_reader.seek(SeekFrom::Start(link_id)).unwrap();
        // read the block basic info
        let (id, block_size, links, next_pos) = read_v4_basic_info(link_id, instance)?;
        let mut dt_block = DTBlock::new(0);
        dt_block.id = id;
        dt_block.block_size = block_size as u64;
        dt_block.links_count = links.len() as u64;
        dt_block.base_position = next_pos as i64;

        // save dt block to instance's arena
        let dt_id = instance.arena.new_node(Box::new(dt_block));
        // add dt block to parent
        parent_id
            .checked_append(dt_id, &mut instance.arena)
            .unwrap();
        instance
            .get_mut_node_by_id::<DTBlock>(dt_id)
            .unwrap()
            .block_id = Some(dt_id);

        // restore stream position
        buf_reader.seek(SeekFrom::Start(pos)).unwrap();

        Ok(dt_id)
    }
}

/// Zipped data blocks
#[normal_object_v4]
#[basic_object]
#[derive(Debug, PermanentBlock)]
pub struct DZBlock {
    /// compressed size of data block
    pub length: u64,
    /// uncompressed size of data block
    pub size: i64,
    pub zip_parameter: u32,
    pub zip_type: Option<ZipType>,
    block_type: String,
}

impl MDFObject for DZBlock {
    fn block_size(&self) -> u64 {
        self.block_size
    }

    fn name(&self) -> String {
        self.id.to_string()
    }
}

impl DZBlock {
    pub fn new(
        compressed_size: u64,
        uncompressed_size: i64,
        zip_type: Option<ZipType>,
        zip_param: u32,
        block_type: Option<String>,
    ) -> Self {
        DZBlock {
            length: compressed_size,
            size: uncompressed_size,
            zip_parameter: zip_param,
            zip_type: zip_type,
            block_type: block_type.map_or("DT".to_string(), |x| x),
            id: "DZ".to_string(),
            block_size: 48 + compressed_size,
            links_count: 0,
            block_id: None,
        }
    }

    pub fn block_type(&self) -> String {
        self.block_type.clone()
    }

    pub fn parse(
        byte_order: ByteOrder,
        link_id: u64,
        parent_id: BlockId,
        instance: &mut MDFFile,
    ) -> Result<(BlockId, Vec<u8>), MDFErrorKind> {
        let mut buf_reader = instance.get_buf_reader()?;
        let pos = buf_reader.stream_position().unwrap();
        // seek to link_id position
        buf_reader.seek(SeekFrom::Start(link_id)).unwrap();
        // read the block basic info
        let (id, block_size, links, next_pos) = read_v4_basic_info(link_id, instance)?;

        // go to next pos, and construct dz block
        buf_reader.seek(SeekFrom::Start(next_pos)).unwrap();
        let mut data = vec![0; 24];
        buf_reader.read_exact(&mut data).unwrap();
        let mut dz_block = dz_block_basic(&data, id, block_size, links.len() as u64)
            .unwrap()
            .1;
        // whether if zip type supported
        if dz_block.zip_type.is_none() {
            return Err(MDFErrorKind::UnsupportedZipType);
        }
        // read compressed data out
        let mut compressed_data = vec![0; dz_block.length as usize];
        buf_reader.read_exact(&mut compressed_data).unwrap();

        // save dz block to instance's arena
        let dz_id = instance.arena.new_node(Box::new(dz_block));
        // add dt block to parent
        parent_id
            .checked_append(dz_id, &mut instance.arena)
            .unwrap();
        instance
            .get_mut_node_by_id::<DGBlock>(dz_id)
            .unwrap()
            .block_id = Some(dz_id);

        // restore stream position
        buf_reader.seek(SeekFrom::Start(pos)).unwrap();

        Ok((dz_id, compressed_data))
    }
}

#[normal_object_v4]
#[channel_group_object]
#[comment_object]
#[basic_object]
#[derive(Debug, PermanentBlock)]
pub struct CGBlock {
    pub(crate) link_next_cgblock: u64,
    pub(crate) link_cn_start: u64,
    pub(crate) link_acquisition_name: u64,
    pub(crate) link_si_block: u64,
    pub(crate) link_sr_start: u64,
    pub(crate) link_comment: u64,
    pub acquisition_name: String,
    pub flags: Option<ChannelGroupFlags>,
    pub inval_size: u32,
    pub path_separator: char,
    pub record_id: u64,
}

impl MDFObject for CGBlock {
    fn block_size(&self) -> u64 {
        self.block_size
    }

    fn name(&self) -> String {
        "Channel group".to_string()
    }
}

impl CGBlock {
    pub fn new(comment: String) -> Self {
        CGBlock {
            link_next_cgblock: 0,
            acquisition_name: Default::default(),
            flags: None,
            inval_size: 0,
            path_separator: Default::default(),
            record_id: 0,
            id: "CG".to_string(),
            block_size: 104,
            links_count: 6,
            record_count: 0,
            record_size: 0,
            comment,
            block_id: None,
            link_cn_start: 0,
            link_acquisition_name: 0,
            link_si_block: 0,
            link_sr_start: 0,
            link_comment: 0,
        }
    }

    pub fn parse(
        byte_order: ByteOrder,
        link_id: u64,
        parent_id: BlockId,
        instance: &mut MDFFile,
    ) -> Result<u64, MDFErrorKind> {
        let mut buf_reader = instance.get_buf_reader()?;
        let pos = buf_reader.stream_position().unwrap();
        // seek to link_id position
        buf_reader.seek(SeekFrom::Start(link_id)).unwrap();
        // read the block basic info
        let (id, block_size, links, next_pos) = read_v4_basic_info(link_id, instance)?;

        // go to next pos, and construct cg block(size 32bytes)
        buf_reader.seek(SeekFrom::Start(next_pos)).unwrap();
        let mut data = vec![0; 32];
        buf_reader.read_exact(&mut data).unwrap();
        let mut cg_block = cg_block_basic(&data, id, block_size, links.len() as u64)
            .unwrap()
            .1;
        // get links out
        let mut link_next_cgblock = 0;
        let mut link_cn_start = 0;
        let mut link_acquisition_name = 0;
        let mut link_si_block = 0;
        let mut link_sr_start = 0;
        let mut link_comment = 0;
        // loop links and assign links
        for (i, val) in links.iter().enumerate() {
            match i {
                0 => link_next_cgblock = *val,
                1 => link_cn_start = *val,
                2 => link_acquisition_name = *val,
                3 => link_si_block = *val,
                4 => link_sr_start = *val,
                5 => link_comment = *val,
                _ => {}
            }
        }
        // save links to cg_block
        cg_block.link_next_cgblock = link_next_cgblock as u64;
        cg_block.link_cn_start = link_cn_start as u64;
        cg_block.link_acquisition_name = link_acquisition_name as u64;
        cg_block.link_si_block = link_si_block as u64;
        cg_block.link_sr_start = link_sr_start as u64;
        cg_block.link_comment = link_comment as u64;

        // parse comment block
        if link_comment > 0 {
            let comment_block = RDSDBlock::parse(byte_order, link_comment as u64, instance)?;
            cg_block.comment = String::from_utf8_lossy(&comment_block.data).to_string();
        }
        // parse acquisition name block
        if link_acquisition_name > 0 {
            let acquisition_name_block =
                RDSDBlock::parse(byte_order, link_acquisition_name as u64, instance)?;
            cg_block.acquisition_name =
                String::from_utf8_lossy(&acquisition_name_block.data).to_string();
        }

        // save cg block to instance's arena
        let cg_id = instance.arena.new_node(Box::new(cg_block));
        // add cg block to parent
        parent_id
            .checked_append(cg_id, &mut instance.arena)
            .unwrap();
        // save cg_id to self
        instance
            .get_mut_node_by_id::<CGBlock>(cg_id)
            .unwrap()
            .block_id = Some(cg_id);

        // parse acquisition source
        if link_si_block > 0 {
            SIBlock::parse(byte_order, link_si_block as u64, cg_id, instance)?;
        }

        // parse cn blocks
        let mut cn_link = link_cn_start;
        while cn_link > 0 {
            break;
        }

        // parse sr blocks
        let mut sr_link = link_sr_start;
        while sr_link > 0 {
            break;
        }

        // restore stream position
        buf_reader.seek(SeekFrom::Start(pos)).unwrap();
        Ok(link_next_cgblock as u64)
    }
}

#[normal_object_v4]
#[comment_object]
#[basic_object]
#[derive(Debug, PermanentBlock)]
pub struct SIBlock {
    pub bus_type: Option<BusType>,
    pub flags: Option<SourceFlags>,
    pub name: String,
    pub path: String,
    pub source_type: Option<Source>,
    pub(crate) link_name: u64,
    pub(crate) link_path: u64,
    pub(crate) link_comment: u64,
}

impl MDFObject for SIBlock {
    fn block_size(&self) -> u64 {
        self.block_size
    }

    fn name(&self) -> String {
        self.name.clone()
    }
}

impl SIBlock {
    pub fn new(
        source_type: Option<Source>,
        bus_type: Option<BusType>,
        name: String,
        path: String,
        comment: String,
        source_flags: Option<SourceFlags>,
    ) -> Self {
        SIBlock {
            bus_type,
            flags: source_flags,
            name,
            path,
            source_type,
            id: "SI".to_string(),
            block_size: 56,
            links_count: 3,
            block_id: None,
            comment,
            link_name: 0,
            link_path: 0,
            link_comment: 0,
        }
    }

    pub fn parse(
        byte_order: ByteOrder,
        link_id: u64,
        parent_id: BlockId,
        instance: &mut MDFFile,
    ) -> Result<(), MDFErrorKind> {
        let mut buf_reader = instance.get_buf_reader()?;
        let pos = buf_reader.stream_position().unwrap();
        // seek to link_id position
        buf_reader.seek(SeekFrom::Start(link_id)).unwrap();
        // read the block basic info
        let (id, block_size, links, next_pos) = read_v4_basic_info(link_id, instance)?;

        // go to next pos, and construct si block
        buf_reader.seek(SeekFrom::Start(next_pos)).unwrap();
        let mut data = vec![0; 3];
        buf_reader.read_exact(&mut data).unwrap();
        let mut si_block = si_block_basic(&data, id, block_size, links.len() as u64)
            .unwrap()
            .1;

        let mut link_name = 0;
        let mut link_comment = 0;
        let mut link_path = 0;
        // iterate over links to store name, path and comment link
        for (i, val) in links.iter().enumerate() {
            match i {
                0 => link_name = *val,
                1 => link_path = *val,
                2 => link_comment = *val,
                _ => unreachable!(),
            }
        }
        si_block.link_name = link_name as u64;
        si_block.link_comment = link_comment as u64;
        si_block.link_path = link_path as u64;

        // get name from name rdsdblock
        if link_name > 0 {
            let name_block = RDSDBlock::parse(byte_order, link_name as u64, instance)?;
            si_block.name = String::from_utf8_lossy(&name_block.data).to_string();
        }
        // get path from path rdsdblock
        if link_path > 0 {
            let path_block = RDSDBlock::parse(byte_order, link_path as u64, instance)?;
            si_block.path = String::from_utf8_lossy(&path_block.data).to_string();
        }
        // get comment from comment rdsdblock
        if link_comment > 0 {
            let comment_block = RDSDBlock::parse(byte_order, link_comment as u64, instance)?;
            si_block.comment = String::from_utf8_lossy(&comment_block.data).to_string();
        }
        // save dz block to instance's arena
        let si_id = instance.arena.new_node(Box::new(si_block));
        // add dt block to parent
        parent_id
            .checked_append(si_id, &mut instance.arena)
            .unwrap();
        instance
            .get_mut_node_by_id::<SIBlock>(si_id)
            .unwrap()
            .block_id = Some(si_id);

        // restore stream position
        buf_reader.seek(SeekFrom::Start(pos)).unwrap();

        Ok(())
    }
}

#[normal_object_v4]
#[channel_object]
#[comment_object]
#[basic_object]
#[derive(Debug, PermanentBlock)]
pub struct CNBlock {
    pub name: String,
    pub flags: ChannelFlags,
    pub inval_bit_pos: u32,
    pub max: f64,
    pub max_ex: f64,
    pub min: f64,
    pub min_ex: f64,
    pub precision: u8,
    pub zip_reader: Option<ZippedReader>,
    pub sd_block: Option<BlockId>,
    pub(crate) link_ids: Vec<u64>,
    pub(crate) link_cn_next: u64,
    pub(crate) link_name: u64,
    pub(crate) link_si_block: u64,
    pub(crate) link_cc_block: u64,
    pub(crate) link_sd_block: u64,
    pub(crate) link_unit: u64,
    pub(crate) link_comment: u64,
}

impl MDFObject for CNBlock {
    fn block_size(&self) -> u64 {
        self.block_size
    }

    fn name(&self) -> String {
        self.name.clone()
    }
}

impl CNBlock {
    pub fn attachments(&self) -> Option<Vec<&ATBlock>> {
        todo!()
    }
    pub fn cm_block(&self) -> Option<&CMBlock> {
        todo!()
    }
    pub fn sd_block(&self) -> Option<&SDBlock> {
        todo!()
    }
    pub fn si_block(&self) -> Option<&SIBlock> {
        todo!()
    }

    pub fn new(
        signal_type: Option<SignalType>,
        channel_type: Option<ChannelType>,
        name: String,
        comment: String,
        offset: u32,
        add_start: u32,
        bits_count: u32,
        sync_type: Option<SyncType>,
        channel_flags: ChannelFlags,
        inval_bit_pos: u32,
        precision: u8,
    ) -> Self {
        CNBlock {
            name,
            flags: channel_flags,
            inval_bit_pos,
            max: f64::NAN,
            max_ex: f64::NAN,
            min: f64::NAN,
            min_ex: f64::NAN,
            link_ids: vec![],
            id: "CN".to_string(),
            block_size: 160,
            links_count: 8,
            add_offset: (add_start + offset) / 8,
            bit_offset: (offset % 8) as u16,
            channel_type,
            max_raw: f64::NAN,
            min_raw: f64::NAN,
            bits_count,
            signal_type,
            sync_type,
            unit: "".to_string(),
            comment,
            block_id: None,
            precision,
            link_cn_next: 0,
            link_name: 0,
            link_si_block: 0,
            link_cc_block: 0,
            link_sd_block: 0,
            link_unit: 0,
            link_comment: 0,
            zip_reader: None,
            sd_block: None,
        }
    }
    /// parse CNBlocks
    pub(crate) fn parse(
        byte_order: ByteOrder,
        link_id: u64,
        parent_id: BlockId,
        instance: &mut MDFFile,
    ) -> Result<u64, MDFErrorKind> {
        let mut buf_reader = instance.get_buf_reader()?;
        let pos = buf_reader.stream_position().unwrap();
        // seek to link_id position
        buf_reader.seek(SeekFrom::Start(link_id)).unwrap();
        // read the block basic info
        let (id, block_size, links, next_pos) = read_v4_basic_info(link_id, instance)?;

        // go to next pos, and construct cn block(76bytes)
        buf_reader.seek(SeekFrom::Start(next_pos)).unwrap();
        let mut data = vec![0; 76];
        buf_reader.read_exact(&mut data).unwrap();
        let mut cn_block = cn_block_basic(&data, id, block_size, links.len() as u64)
            .unwrap()
            .1;

        // get links out
        let mut link_next_cn = 0;
        let mut link_name = 0;
        let mut link_si_block = 0;
        let mut link_cc_block = 0;
        let mut link_sd_block = 0;
        let mut link_unit = 0;
        let mut link_comment = 0;
        let mut link_ids = vec![];
        // loop links and assign links
        for (i, val) in links.iter().enumerate() {
            match i {
                0 => link_next_cn = *val,
                1 => {}
                2 => link_name = *val,
                3 => link_si_block = *val,
                4 => link_cc_block = *val,
                5 => link_sd_block = *val,
                6 => link_unit = *val,
                7 => link_comment = *val,
                _ => {
                    if *val > 0 {
                        link_ids.push(*val as u64);
                    }
                }
            }
        }

        // save links to cg_block
        cn_block.link_ids = link_ids;
        cn_block.link_cn_next = link_next_cn as u64;
        cn_block.link_name = link_name as u64;
        cn_block.link_si_block = link_si_block as u64;
        cn_block.link_cc_block = link_cc_block as u64;
        cn_block.link_sd_block = link_sd_block as u64;
        cn_block.link_unit = link_unit as u64;
        cn_block.link_comment = link_comment as u64;

        // get name from name rdsdblock
        if link_name > 0 {
            let name_block = RDSDBlock::parse(byte_order, link_name as u64, instance)?;
            cn_block.name = String::from_utf8_lossy(&name_block.data).to_string();
        }
        // get unit from unit rdsdblock
        if link_unit > 0 {
            let unit_block = RDSDBlock::parse(byte_order, link_unit as u64, instance)?;
            cn_block.unit = String::from_utf8_lossy(&unit_block.data).to_string();
        }
        // get comment from comment rdsdblock
        if link_comment > 0 {
            let comment_block = RDSDBlock::parse(byte_order, link_comment as u64, instance)?;
            cn_block.comment = String::from_utf8_lossy(&comment_block.data).to_string();
        }

        // save cn_block to instance's arena
        let cn_id = instance.arena.new_node(Box::new(cn_block));
        // add cn block to parent
        parent_id
            .checked_append(cn_id, &mut instance.arena)
            .unwrap();
        instance
            .get_mut_node_by_id::<CNBlock>(cn_id)
            .unwrap()
            .block_id = Some(cn_id);

        // parse si block
        if link_si_block > 0 {
            SIBlock::parse(byte_order, link_si_block as u64, cn_id, instance)?;
        }
        // parse cc block, and replace min,max with cc_block's min&max
        let cc_ref = instance.get_node_by_id::<CNBlock>(cn_id).unwrap();
        let cc_min = cc_ref.min;
        let cc_max = cc_ref.max;
        if link_cc_block > 0 {
            let (_, min, max) =
                CCBlock::parse(byte_order, link_cc_block as u64, cn_id, instance, true)?;
            if cc_min.is_nan() && !min.is_nan() {
                instance.get_mut_node_by_id::<CNBlock>(cn_id).unwrap().min = min;
            }
            if cc_max.is_nan() && !max.is_nan() {
                instance.get_mut_node_by_id::<CNBlock>(cn_id).unwrap().max = max;
            }
        }

        // link sd block
        if link_sd_block > 0 {
            // todo:
        }

        // restore stream position
        buf_reader.seek(SeekFrom::Start(pos)).unwrap();
        Ok(link_next_cn as u64)
    }
}

#[normal_object_v4]
#[basic_object]
#[derive(Debug, PermanentBlock)]
pub struct CMBlock {}

impl MDFObject for CMBlock {
    fn block_size(&self) -> u64 {
        self.block_size
    }

    fn name(&self) -> String {
        self.id.clone()
    }
}

#[normal_object_v4]
#[basic_object]
#[derive(Debug, PermanentBlock)]
pub struct SDBlock {}

impl MDFObject for SDBlock {
    fn block_size(&self) -> u64 {
        self.block_size
    }

    fn name(&self) -> String {
        self.id.clone()
    }
}

#[normal_object_v4]
#[basic_object]
#[comment_object]
#[channel_conversion_object]
#[derive(Debug, PermanentBlock)]
pub struct CCBlock {
    pub name: String,
    pub flags: ConversionFlags,
    pub precision: u8,
    pub refcount: u16,
    pub tab_pairs: Option<Vec<(f64, f64)>>,
    pub text_table_pairs: Option<Vec<(f64, String)>>,
    pub text_range_pairs: Option<HashMap<String, Range<f64>>>,
    pub(crate) link_md_comment: u64,
    pub(crate) link_md_unit: u64,
    pub(crate) link_tx_name: u64,
    pub(crate) link_cc_inverse: u64,
}

impl MDFObject for CCBlock {
    fn block_size(&self) -> u64 {
        self.block_size
    }

    fn name(&self) -> String {
        self.name.clone()
    }
}

impl CCBlock {
    pub fn new(conversion_type: Option<ConversionType>, unit: String, min: f64, max: f64) -> Self {
        let mut flags = ConversionFlags::None;
        let mut max_ = f64::NAN;
        let mut min_ = f64::NAN;
        if !min.is_nan() && !max.is_nan() {
            flags |= ConversionFlags::LimitRangeValid;
            max_ = max;
            min_ = min;
        }
        CCBlock {
            name: Default::default(),
            flags: flags,
            precision: 0,
            refcount: 0,
            id: "CC".to_string(),
            block_size: 80,
            links_count: 4,
            block_id: None,
            conversion_type,
            default_text: Default::default(),
            formula: Default::default(),
            inv_ccblock: None,
            max: max_,
            min: min_,
            params: None,
            tab_size: 0,
            unit,
            comment: Default::default(),
            link_md_comment: 0,
            link_md_unit: 0,
            link_tx_name: 0,
            link_cc_inverse: 0,
            tab_pairs: None,
            text_table_pairs: None,
            text_range_pairs: None,
        }
    }
    // parse cc block
    pub(crate) fn parse(
        byte_order: ByteOrder,
        link_id: u64,
        parent_id: BlockId,
        instance: &mut MDFFile,
        add_as_child: bool,
    ) -> Result<(BlockId, f64, f64), MDFErrorKind> {
        let mut buf_reader = instance.get_buf_reader()?;
        let pos = buf_reader.stream_position().unwrap();
        // seek to link_id position
        buf_reader.seek(SeekFrom::Start(link_id)).unwrap();
        // read the block basic info
        let (id, block_size, links, next_pos) = read_v4_basic_info(link_id, instance)?;

        // go to next pos, and construct cc block(24bytes)
        buf_reader.seek(SeekFrom::Start(next_pos)).unwrap();
        let mut data = vec![0; 24];
        buf_reader.read_exact(&mut data).unwrap();
        let mut cc_block = cc_block_basic(&data, id, block_size, links.len() as u64)
            .unwrap()
            .1;

        // get links out
        let mut link_text_name = 0;
        let mut link_md_unit = 0;
        let mut link_md_comment = 0;
        let mut link_cc_inverse = 0;
        // loop links and assign links
        for (i, val) in links.iter().enumerate() {
            match i {
                0 => link_text_name = *val,
                1 => link_md_unit = *val,
                2 => link_md_comment = *val,
                3 => link_cc_inverse = *val,
                _ => {}
            }
        }
        let min = cc_block.min;
        let max = cc_block.max;
        // save links to cg_block
        cc_block.link_tx_name = link_text_name as u64;
        cc_block.link_md_unit = link_md_unit as u64;
        cc_block.link_md_comment = link_md_comment as u64;
        cc_block.link_cc_inverse = link_cc_inverse as u64;

        // get name from name rdsdblock
        if link_text_name > 0 {
            let name_block = RDSDBlock::parse(byte_order, link_text_name as u64, instance)?;
            cc_block.name = String::from_utf8_lossy(&name_block.data).to_string();
        }
        // get unit from unit rdsdblock
        if link_md_unit > 0 {
            let unit_block = RDSDBlock::parse(byte_order, link_md_unit as u64, instance)?;
            cc_block.unit = String::from_utf8_lossy(&unit_block.data).to_string();
        }
        // get comment from comment rdsdblock
        if link_md_comment > 0 {
            let comment_block = RDSDBlock::parse(byte_order, link_md_comment as u64, instance)?;
            cc_block.comment = String::from_utf8_lossy(&comment_block.data).to_string();
        }
        let pos_pos = buf_reader.stream_position().unwrap();
        cc_block.parse_params(pos_pos, instance, &links)?;

        // save cc_block to instance's arena
        let cc_id = instance.arena.new_node(Box::new(cc_block));

        if add_as_child {
            // add cc block to parent
            parent_id
                .checked_append(cc_id, &mut instance.arena)
                .unwrap();
        }
        instance
            .get_mut_node_by_id::<CCBlock>(cc_id)
            .unwrap()
            .block_id = Some(cc_id);

        // parse inverse cc block
        if link_cc_inverse > 0 {
            let (inv_cc_id, _, _) =
                CCBlock::parse(byte_order, link_cc_inverse as u64, cc_id, instance, false)?;
            instance
                .get_mut_node_by_id::<CCBlock>(cc_id)
                .unwrap()
                .inv_ccblock = Some(inv_cc_id);
        }

        // restore stream position
        buf_reader.seek(SeekFrom::Start(pos)).unwrap();

        Ok((cc_id, min, max))
    }

    fn parse_params(
        &mut self,
        position: u64,
        instance: &mut MDFFile,
        links: &Vec<i64>,
    ) -> Result<(), MDFErrorKind> {
        let mut buf_reader = instance.get_buf_reader().unwrap();
        buf_reader.seek(SeekFrom::Start(position)).unwrap();
        match self.conversion_type {
            Some(conv_type) => {
                match conv_type {
                    ConversionType::ParametricLinear | ConversionType::Rational => {
                        if self.tab_size > 0 {
                            let mut buf = vec![0; (self.tab_size as usize) * 8];
                            buf_reader.read_exact(&mut buf).unwrap();
                            let mut params = read_n_le_f64(&buf, self.tab_size as usize).unwrap().1;
                            params = transform_params(params, Some(conv_type));
                            self.params = Some(params);
                        }
                    }
                    ConversionType::TabInt | ConversionType::Tab => {
                        let mut tab_pairs = Vec::new();
                        for _ in 0..(self.tab_size / 2) {
                            let mut buf = [0; 16];
                            buf_reader.read_exact(&mut buf).unwrap();
                            let key_var = read_n_le_f64(&buf, 2).unwrap().1;
                            tab_pairs.push((key_var[0], key_var[1]));
                        }
                        self.tab_pairs = Some(tab_pairs);
                    }
                    ConversionType::TextFormula => {
                        if links.len() > 4 {
                            let link_formula = links[4];
                            if link_formula > 0 {
                                let formula_block = RDSDBlock::parse(
                                    ByteOrder::LittleEndian,
                                    link_formula as u64,
                                    instance,
                                )?;
                                self.formula =
                                    String::from_utf8_lossy(&formula_block.data).to_string();
                            }
                        }
                    }
                    ConversionType::TextTable => {
                        let mut text_table_pairs = Vec::new();
                        let mut vec_f64 = Vec::with_capacity(self.tab_size as usize);
                        for _ in 0..self.tab_size {
                            let mut buf = [0; 8];
                            buf_reader.read_exact(&mut buf).unwrap();
                            vec_f64.push(read_le_f64(&buf).unwrap().1);
                        }
                        let mut vec_str = Vec::with_capacity(links.len() - 4);
                        for i in 0..(links.len() - 4) {
                            let link_address = links[4 + i];
                            let val_block = RDSDBlock::parse(
                                ByteOrder::LittleEndian,
                                link_address as u64,
                                instance,
                            )?;
                            vec_str.push(String::from_utf8_lossy(&val_block.data).to_string());
                        }
                        for i in 0..self.tab_size {
                            text_table_pairs
                                .push((vec_f64[i as usize], vec_str[i as usize].clone()));
                        }
                        // set default text
                        self.default_text = vec_str[vec_str.len() - 1].clone();
                        self.text_table_pairs = Some(text_table_pairs);
                    }
                    ConversionType::TextRange => {
                        let mut text_range_pairs = HashMap::new();
                        let mut n = 0;
                        let mut idx = 0;
                        let mut vec_range = Vec::new();
                        while n < self.tab_size {
                            let mut buf = vec![0; 2 * 8];
                            buf_reader.read_exact(&mut buf).unwrap();
                            let min_max = read_n_le_f64(&buf, 2).unwrap().1;
                            let range = Range {
                                start: min_max[0],
                                end: min_max[2],
                            };
                            vec_range.push(range);
                            n += 2;
                            idx += 1;
                        }
                        let mut vec_str = Vec::with_capacity(links.len() - 4);
                        for i in 0..(links.len() - 4) {
                            let link_address = links[4 + i];
                            let val_block = RDSDBlock::parse(
                                ByteOrder::LittleEndian,
                                link_address as u64,
                                instance,
                            )?;
                            vec_str.push(String::from_utf8_lossy(&val_block.data).to_string());
                        }
                        for i in 0..(links.len() - 4 - 1) {
                            text_range_pairs
                                .insert(vec_str[i as usize].clone(), vec_range[i as usize].clone());
                        }
                        self.text_range_pairs = Some(text_range_pairs);
                        // set default text
                        self.default_text = vec_str[vec_str.len() - 1].clone();
                    }
                    _ => {
                        return Err(MDFErrorKind::CCBlockError(
                            "Doesn't supported thid kind of conversion type!".into(),
                        ));
                    }
                }
            }
            None => {}
        }
        Ok(())
    }
}

#[normal_object_v4]
#[comment_object]
#[basic_object]
#[derive(Debug, Clone, PermanentBlock)]
pub struct FHBlock {
    pub dst_offset: i16,
    pub flags: Option<TimeFlagsType>,
    pub timestamp: u64,
    pub utc_offset: i16,
    pub(crate) link_fh_next: u64,
    pub(crate) link_comment: u64,
}

impl MDFObject for FHBlock {
    fn block_size(&self) -> u64 {
        self.block_size
    }

    fn name(&self) -> String {
        self.id.clone()
    }
}

impl FHBlock {
    pub fn new(
        username: String,
        text: String,
        timestamp: u64,
        utc_offset: i16,
        dst_offset: i16,
        flags: Option<TimeFlagsType>,
    ) -> Self {
        let comment = format!("<FHcomment>\r\n<TX>{0}</TX>\r\n<tool_id>asammdf rust crate</tool_id>\r\n<tool_vendor>H2O2.IO</tool_vendor>\r\n<tool_version>{1}</tool_version>\r\n<user_name>{2}</user_name>\r\n<common_properties>\r\n<e name=\"asammdf rust crate\">Version {1}</e>\r\n</common_properties>\r\n</FHcomment>", text, env!("CARGO_PKG_VERSION"), username);
        Self {
            dst_offset,
            flags: Some(TimeFlagsType::LocalTime),
            timestamp,
            utc_offset,
            link_fh_next: 0,
            id: "FH".to_string(),
            block_size: 56,
            links_count: 2,
            comment,
            block_id: None,
            link_comment: 0,
        }
    }

    // parse fh block
    pub(crate) fn parse(
        byte_order: ByteOrder,
        link_id: u64,
        parent_id: BlockId,
        instance: &mut MDFFile,
        add_as_child: bool,
    ) -> Result<u64, MDFErrorKind> {
        let mut buf_reader = instance.get_buf_reader()?;
        let pos = buf_reader.stream_position().unwrap();
        // seek to link_id position
        buf_reader.seek(SeekFrom::Start(link_id)).unwrap();
        // read the block basic info
        let (id, block_size, links, next_pos) = read_v4_basic_info(link_id, instance)?;

        // go to next pos, and construct fh block(16bytes)
        buf_reader.seek(SeekFrom::Start(next_pos)).unwrap();
        let mut data = vec![0; 16];
        buf_reader.read_exact(&mut data).unwrap();
        let mut fh_block = fh_block_basic(&data, id, block_size, links.len() as u64)
            .unwrap()
            .1;

        // get links out
        let mut link_comment = 0;
        let mut link_fh_next = 0;
        // loop links and assign links
        for (i, val) in links.iter().enumerate() {
            match i {
                0 => link_fh_next = *val,
                1 => link_comment = *val,
                _ => {}
            }
        }
        // save links to fh_block
        fh_block.link_comment = link_comment as u64;
        fh_block.link_fh_next = link_fh_next as u64;

        // get comment from comment rdsdblock
        if link_comment > 0 {
            let comment_block = RDSDBlock::parse(byte_order, link_comment as u64, instance)?;
            fh_block.comment = String::from_utf8_lossy(&comment_block.data).to_string();
        }

        // store to arena
        let fh_id = instance.arena.new_node(Box::new(fh_block));
        // add cc block to parent
        parent_id
            .checked_append(fh_id, &mut instance.arena)
            .unwrap();
        instance
            .get_mut_node_by_id::<FHBlock>(fh_id)
            .unwrap()
            .block_id = Some(fh_id);

        // restore stream position
        buf_reader.seek(SeekFrom::Start(pos)).unwrap();

        Ok(link_fh_next as u64)
    }
}

#[normal_object_v4]
#[comment_object]
#[basic_object]
#[derive(Debug, Clone, PermanentBlock)]
pub struct EVBlock {
    pub cause: Option<Cause>,
    pub creator_index: u16,
    pub flags: Option<EventFlags>,
    pub range: Option<RangeType>,
    pub sync: Option<SyncType>,
    pub sync_base_val: i64,
    pub sync_factor: f64,
    pub evt_type: Option<Event>,
    pub(crate) link_next_ev: u64,
    pub(crate) ref_datas: Vec<u64>,
}

impl MDFObject for EVBlock {
    fn block_size(&self) -> u64 {
        self.block_size
    }

    fn name(&self) -> String {
        self.id.clone()
    }
}

impl EVBlock {
    pub fn new() -> Self {
        EVBlock {
            cause: None,
            creator_index: 0,
            flags: None,
            range: None,
            sync: None,
            sync_base_val: 0,
            sync_factor: 0.0,
            evt_type: None,
            link_next_ev: 0,
            id: "EV".to_string(),
            block_size: 0,
            links_count: 0,
            comment: Default::default(),
            block_id: None,
            ref_datas: vec![],
        }
    }

    // parse ev block
    pub(crate) fn parse(
        byte_order: ByteOrder,
        link_id: u64,
        parent_id: BlockId,
        instance: &mut MDFFile,
        add_as_child: bool,
    ) -> Result<u64, MDFErrorKind> {
        let mut buf_reader = instance.get_buf_reader()?;
        let pos = buf_reader.stream_position().unwrap();
        // seek to link_id position
        buf_reader.seek(SeekFrom::Start(link_id)).unwrap();
        // read the block basic info
        let (id, block_size, links, next_pos) = read_v4_basic_info(link_id, instance)?;

        // go to next pos, and construct fh block(16bytes)
        buf_reader.seek(SeekFrom::Start(next_pos)).unwrap();
        let mut data = vec![0; 16];
        buf_reader.read_exact(&mut data).unwrap();
        let mut fh_block = fh_block_basic(&data, id, block_size, links.len() as u64)
            .unwrap()
            .1;

        // get links out
        let mut link_comment = 0;
        let mut link_fh_next = 0;
        // loop links and assign links
        for (i, val) in links.iter().enumerate() {
            match i {
                0 => link_fh_next = *val,
                1 => link_comment = *val,
                _ => {}
            }
        }
        // save links to fh_block
        fh_block.link_comment = link_comment as u64;
        fh_block.link_fh_next = link_fh_next as u64;

        // get comment from comment rdsdblock
        if link_comment > 0 {
            let comment_block = RDSDBlock::parse(byte_order, link_comment as u64, instance)?;
            fh_block.comment = String::from_utf8_lossy(&comment_block.data).to_string();
        }

        // store to arena
        let fh_id = instance.arena.new_node(Box::new(fh_block));
        // add cc block to parent
        parent_id
            .checked_append(fh_id, &mut instance.arena)
            .unwrap();
        instance
            .get_mut_node_by_id::<FHBlock>(fh_id)
            .unwrap()
            .block_id = Some(fh_id);

        // restore stream position
        buf_reader.seek(SeekFrom::Start(pos)).unwrap();

        Ok(link_fh_next as u64)
    }
}
