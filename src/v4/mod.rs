use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::ops::Range;
use std::vec;

use asammdf_derive::{
    basic_object, channel_conversion_object, channel_group_object, channel_object, comment_object,
    data_group_object, id_object, mdf_object, normal_object_v4,
};
use asammdf_derive::{IDObject, PermanentBlock};
use chrono::{DateTime, Local};
use libflate;
use tempfile::tempfile;

use self::parser::{
    at_block_basic, block_base, cc_block_basic, cg_block_basic, ch_block_basic, cn_block_basic,
    dg_block_basic, dl_block_basic, dz_block_basic, ev_block_basic, fh_block_basic, hd_block_basic,
    hl_block_basic, si_block_basic, sr_block_basic,
};
use super::SpecVer;
use super::UnfinalizedFlagsType;
use crate::misc::helper::{read_le_f64, read_le_i64, read_n_le_f64};
use crate::misc::transform_params;
use crate::{
    BlockId, ChannelType, ConversionType, DependencyType, IDObject, SignalType, SyncType,
    TimeFlagsType, TimeQualityType,
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
enum_u32_convert! {
#[derive(Clone, Copy, Debug)]
pub enum SRFlags {
    InvalidationBytes = 1,
}
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
enum_u32_convert! {
#[derive(Clone, Copy, Debug)]
pub enum RangeType {
    Point,
    BeginRange,
    EndRange,
}
}
enum_u32_convert! {
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
}

enum_u32_convert! {
#[derive(Clone, Copy, Debug)]
pub enum Event {
    Recording,
    RecordingInterrupt,
    AcquisitionInterrupt,
    StartRecordingTrigger,
    StopRecordingTrigger,
    Trigger,
    Marker,
}}
enum_u32_convert! {
#[derive(Clone, Copy, Debug)]
pub enum EventFlags {
    PostProcessing = 1,
}
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

enum_u32_convert! {
#[derive(Clone, Copy, Debug)]
pub enum Cause {
    Other,
    _Error,
    Tool,
    Script,
    User,
}
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

bitflags! {
    pub struct AttachmentFlags:u16 {
        const None = 0;
        const EmbeddedData = 1;
        const CompressedEmbbeddedData = 2;
        const MD5ChecksumValid = 4;
    }
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
    pub(crate) link_at_start: i64,
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
            link_at_start: 0,
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
            mut link_at_start,
            mut link_ev_start,
            mut link_comment,
        ) = (0, 0, 0, 0, 0, 0);
        // get all links
        for (i, val) in links.iter().enumerate() {
            match i {
                0 => link_dg_start = *val,
                1 => link_fh_start = *val,
                2 => link_ch_start = *val,
                3 => link_at_start = *val,
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
        hd_block.link_at_start = link_at_start;
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
            .checked_append(hd_id, &mut instance.arena)
            .unwrap();

        // parse dg blocks
        let mut link_dg = link_dg_start as u64;
        while link_dg > 0 {
            let (next_dg, block_id) = DGBlock::parse(byte_order, link_dg, hd_id, instance)?;
            instance.link_id_blocks.insert(link_dg, block_id);
            link_dg = next_dg;
        }
        // parse fh blocks
        let mut link_fh = link_fh_start as u64;
        while link_fh > 0 {
            link_fh = FHBlock::parse(byte_order, link_fh, hd_id, instance)?;
        }
        // parse ch blocks
        let mut link_ch = link_ch_start as u64;
        while link_ch > 0 {
            link_ch = CHBlock::parse(byte_order, link_ch, hd_id, instance)?;
        }
        // parse atb blocks
        let mut link_at = link_at_start as u64;
        while link_at > 0 {
            let (next_at, block_id) = ATBlock::parse(byte_order, link_at, hd_id, instance)?;
            instance.link_id_blocks.insert(link_at, block_id);
            link_at = next_at;
        }
        // parse ev blocks
        let mut link_ev = link_ev_start as u64;
        while link_ev > 0 {
            let (next_ev, block_id) = EVBlock::parse(byte_order, link_ev, hd_id, instance)?;
            instance.link_id_blocks.insert(link_ev, block_id);
            link_ev = next_ev;
        }

        buf_reader.seek(SeekFrom::Start(pos)).unwrap();

        Ok(())
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
    pub attachment_flags: AttachmentFlags,
    pub creator_index: u16,
    pub filename: String,
    pub mime_type: String,
    pub(crate) link_next_at: i64,
    pub(crate) link_filename: i64,
    pub(crate) link_mime_type: i64,
    pub(crate) link_comment: i64,
}

impl MDFObject for ATBlock {
    fn block_size(&self) -> u64 {
        self.block_size
    }

    fn name(&self) -> String {
        self.id.clone()
    }
}

impl ATBlock {
    pub fn new() -> Self {
        ATBlock {
            attachment_flags: AttachmentFlags::None,
            creator_index: 0,
            filename: Default::default(),
            mime_type: Default::default(),
            comment: Default::default(),
            id: "AT".to_string(),
            block_size: 0,
            links_count: 0,
            block_id: None,
            link_next_at: 0,
            link_filename: 0,
            link_mime_type: 0,
            link_comment: 0,
        }
    }
    // parse at block
    pub(crate) fn parse(
        byte_order: ByteOrder,
        link_id: u64,
        parent_id: BlockId,
        instance: &mut MDFFile,
    ) -> Result<(u64, BlockId), MDFErrorKind> {
        let mut buf_reader = instance.get_buf_reader()?;
        let pos = buf_reader.stream_position().unwrap();
        // seek to link_id position
        buf_reader.seek(SeekFrom::Start(link_id)).unwrap();
        // read the block basic info
        let (id, block_size, links, next_pos) = read_v4_basic_info(link_id, instance)?;

        // go to next pos, and construct at block(4bytes)
        buf_reader.seek(SeekFrom::Start(next_pos)).unwrap();
        let mut data = vec![0; 4];
        buf_reader.read_exact(&mut data).unwrap();
        let mut at_block = at_block_basic(&data, id, block_size, links.len() as u64)
            .unwrap()
            .1;

        // get links out
        let mut link_next_at = 0;
        let mut link_filename = 0;
        let mut link_mime_type = 0;
        let mut link_comment = 0;

        // loop links and assign links
        for (i, val) in links.iter().enumerate() {
            match i {
                0 => link_next_at = *val,
                1 => link_filename = *val,
                2 => link_mime_type = *val,
                3 => link_comment = *val,
                _ => {}
            }
        }
        // save links to ev_block
        at_block.link_next_at = link_next_at;
        at_block.link_filename = link_filename;
        at_block.link_mime_type = link_mime_type;
        at_block.link_comment = link_comment;

        // get comment from comment rdsdblock
        if link_comment > 0 {
            let comment_block = RDSDBlock::parse(byte_order, link_comment as u64, instance)?;
            at_block.comment = String::from_utf8_lossy(&comment_block.data).to_string();
        }
        // get name from name rdsdblock
        if link_filename > 0 {
            let filename_block = RDSDBlock::parse(byte_order, link_filename as u64, instance)?;
            at_block.filename = String::from_utf8_lossy(&filename_block.data).to_string();
        }
        // get mimetype from rdsdblock
        if link_mime_type > 0 {
            let mime_type_block = RDSDBlock::parse(byte_order, link_mime_type as u64, instance)?;
            at_block.mime_type = String::from_utf8_lossy(&mime_type_block.data).to_string();
        }

        // store to arena
        let at_id = instance.arena.new_node(Box::new(at_block));
        // add at block to parent
        parent_id
            .checked_append(at_id, &mut instance.arena)
            .unwrap();
        instance
            .get_mut_node_by_id::<EVBlock>(at_id)
            .unwrap()
            .block_id = Some(at_id);

        // restore stream position
        buf_reader.seek(SeekFrom::Start(pos)).unwrap();

        Ok((link_next_at as u64, at_id))
    }
}

#[normal_object_v4]
#[comment_object]
#[basic_object]
#[derive(Debug, PermanentBlock)]
pub struct CHBlock {
    pub hierarchy_type: Option<Hierarchy>,
    pub dependencies: Option<Vec<DependencyType>>,
    dependencies_type_data: Vec<i64>,
    pub name: String,
    pub(crate) link_next_ch: i64,
    pub(crate) link_ch_start: i64,
    pub(crate) link_name: i64,
    pub(crate) link_comment: i64,
}

impl MDFObject for CHBlock {
    fn block_size(&self) -> u64 {
        self.block_size
    }

    fn name(&self) -> String {
        self.name.clone()
    }
}

impl<'a> CHBlock {
    pub fn new() -> Self {
        CHBlock {
            hierarchy_type: None,
            dependencies: todo!(),
            name: Default::default(),
            id: "CH".to_string(),
            block_size: 0,
            links_count: 0,
            comment: Default::default(),
            dependencies_type_data: vec![],
            link_next_ch: 0,
            link_ch_start: 0,
            link_name: 0,
            link_comment: 0,
            block_id: None,
        }
    }

    fn chblocks(&self, mdf_file: &'a MDFFile) -> Option<Vec<&'a CHBlock>> {
        let id = self.block_id.clone();
        id.map_or(None, |node_id| {
            mdf_file.get_nodes_by_parent::<CHBlock>(node_id)
        })
    }

    // parse ch block
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

        // go to next pos, and construct ch block(8bytes)
        buf_reader.seek(SeekFrom::Start(next_pos)).unwrap();
        let mut data = vec![0; 8];
        buf_reader.read_exact(&mut data).unwrap();
        let (dependencies_count, mut ch_block) =
            ch_block_basic(&data, id, block_size, links.len() as u64)
                .unwrap()
                .1;

        // get links out
        let mut link_next_ch = 0;
        let mut link_ch_start = 0;
        let mut link_name = 0;
        let mut link_comment = 0;

        // loop links and assign links
        for (i, val) in links.iter().enumerate() {
            match i {
                0 => link_next_ch = *val,
                1 => link_ch_start = *val,
                2 => link_name = *val,
                3 => link_comment = *val,
                _ => {
                    if *val > 0 {
                        // add to refdata
                        ch_block.dependencies_type_data.push(*val);
                    }
                }
            }
        }
        // save links to ch_block
        ch_block.link_next_ch = link_next_ch;
        ch_block.link_ch_start = link_ch_start;
        ch_block.link_name = link_name;
        ch_block.link_comment = link_comment;

        // get comment from comment rdsdblock
        if link_comment > 0 {
            let comment_block = RDSDBlock::parse(byte_order, link_comment as u64, instance)?;
            ch_block.comment = String::from_utf8_lossy(&comment_block.data).to_string();
        }
        // get name from name rdsdblock
        if link_name > 0 {
            let name_block = RDSDBlock::parse(byte_order, link_name as u64, instance)?;
            ch_block.name = String::from_utf8_lossy(&name_block.data).to_string();
        }

        // parse ch_block's dependencies_tyhpe_data to dependencies
        let mut i = 0;
        let mut idx = 0;
        let mut dependencies = vec![];
        while i < dependencies_count && idx < ch_block.dependencies_type_data.len() {
            dependencies.push(DependencyType::new(
                ch_block.dependencies_type_data[idx],
                ch_block.dependencies_type_data[idx + 1],
                ch_block.dependencies_type_data[idx + 2],
            ));
            i += 1;
            idx += 3;
        }
        if !dependencies.is_empty() {
            ch_block.dependencies = Some(dependencies);
        }

        // store to arena
        let ch_id = instance.arena.new_node(Box::new(ch_block));
        // add cc block to parent
        parent_id
            .checked_append(ch_id, &mut instance.arena)
            .unwrap();
        instance
            .get_mut_node_by_id::<CHBlock>(ch_id)
            .unwrap()
            .block_id = Some(ch_id);

        // get child ch blocks
        let mut ch_link = link_ch_start as u64;
        while ch_link > 0 {
            ch_link = CHBlock::parse(byte_order, ch_link, ch_id, instance)?;
        }

        // restore stream position
        buf_reader.seek(SeekFrom::Start(pos)).unwrap();

        Ok(link_next_ch as u64)
    }
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

pub trait DataObject {
    /// To be honest, it's impossible to pass file handler or other things to children of SRBlock.
    /// we should collect all links of DTBlock and DZBlocks out and then create a temp file to store decompressed data.
    /// Never parse DTBlock and DZBlock data in itself. Because of above reason, we shouldn't parse DZBlock data out in DZBlock
    fn read_data(
        instance: &mut MDFFile,
        links: &Vec<i64>,
        parent_id: BlockId,
        link_data_idx: u64,
    ) -> Result<(Option<ZippedReader>, Vec<(u64, BlockId)>), MDFErrorKind> {
        let mut link_data = links[link_data_idx as usize] as u64;
        let mut block_ids = vec![];
        // links of DTBlock, and DZBlock
        let mut data_blocks: Vec<DataBlockLink> = vec![];
        while link_data > 0 {
            // read another basic block info out
            let (id, block_size, links, next_pos) = read_v4_basic_info(link_data, instance)?;
            // match id type
            match id.as_str() {
                "RD" => {
                    let rd_block = RDSDBlock::new(vec![], id);
                    // store to arena
                    let rd_id = instance.arena.new_node(Box::new(rd_block));
                    parent_id
                        .checked_append(rd_id, &mut instance.arena)
                        .unwrap();
                    instance
                        .get_mut_node_by_id::<EVBlock>(rd_id)
                        .unwrap()
                        .block_id = Some(rd_id);
                    block_ids.push((link_data, rd_id));
                    link_data = 0;
                }
                "HL" => {
                    let hl_id = HLBlock::parse(
                        ByteOrder::LittleEndian,
                        link_data,
                        parent_id,
                        instance,
                        &mut data_blocks,
                    )?;
                    block_ids.push((link_data, hl_id));
                    link_data = 0;
                }
                "DT" => {
                    let dt_id =
                        DTBlock::parse(ByteOrder::LittleEndian, link_data, parent_id, instance)?;
                    block_ids.push((link_data, dt_id));
                    link_data = 0;
                }
                "DL" => {
                    let (link_data_temp, dl_id) = DLBlock::parse(
                        ByteOrder::LittleEndian,
                        link_data,
                        parent_id,
                        instance,
                        &mut data_blocks,
                    )?;
                    link_data = link_data_temp;
                    block_ids.push((link_data, dl_id));
                    // get the DL block
                }
                "DZ" => {
                    // get the DZ block
                    let (dz_id, link_dz_data) =
                        DZBlock::parse(ByteOrder::LittleEndian, link_data, parent_id, instance)?;
                    block_ids.push((link_data, dz_id));
                    data_blocks.push((DataBlockType::DZ, link_dz_data, dz_id));
                    link_data = 0;
                }
                _ => {
                    link_data = 0;
                }
            }
        }
        let mut zip_reader = None;
        // if data blocks has elements, then create a temp file to store decompressed data
        if data_blocks.len() != 0 {
            // create a temp file, store handler inside self
            zip_reader = Some(ZippedReader::new());
            // get a mut ref to zip_reader
            let writer = zip_reader.as_mut().unwrap();
            // iterate data blocks and write data to tmp file
            for (data_block_type, data_block_pos, data_block_id) in data_blocks {
                match data_block_type {
                    DataBlockType::DT => {
                        writer.read_dt_block(data_block_id, data_block_pos, instance)?;
                    }
                    DataBlockType::DZ => {
                        writer.read_dz_block(data_block_id, data_block_pos, instance)?;
                    }
                }
            }
        }
        Ok((zip_reader, block_ids))
    }
}

#[normal_object_v4]
#[basic_object]
#[derive(Debug, PermanentBlock)]
pub struct SRBlock {
    pub flags: Option<SRFlags>,
    pub time_interval_len: f64,
    pub reduced_samples_number: u64,
    pub(crate) link_next_block: u64,
    pub(crate) link_data_block: u64,
    pub(crate) zip_reader: Option<ZippedReader>,
    pub(crate) blocks: Vec<(u64, BlockId)>,
}

pub(crate) fn clone_file_handler(file: Option<&mut File>) -> Option<File> {
    match file {
        Some(f) => Some(f.try_clone().unwrap()),
        None => None,
    }
}

impl MDFObject for SRBlock {
    fn block_size(&self) -> u64 {
        self.block_size
    }

    fn name(&self) -> String {
        self.id.clone()
    }
}

impl SRBlock {
    pub fn new() -> Self {
        SRBlock {
            flags: None,
            time_interval_len: 0.0,
            reduced_samples_number: 0,
            id: "SR".to_string(),
            block_size: 0,
            links_count: 0,
            block_id: None,
            link_next_block: 0,
            link_data_block: 0,
            zip_reader: None,
            blocks: vec![],
        }
    }
    // parse sr block
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

        // go to next pos, and construct ev block(32bytes)
        buf_reader.seek(SeekFrom::Start(next_pos)).unwrap();
        let mut data = vec![0; 32];
        buf_reader.read_exact(&mut data).unwrap();
        let mut sr_block = sr_block_basic(&data, id, block_size, links.len() as u64)
            .unwrap()
            .1;

        let mut link_next_block = 0;
        let mut link_data_block = 0;
        // get all links
        for (i, val) in links.iter().enumerate() {
            match i {
                0 => {
                    // get link to CG block
                    link_next_block = *val as u64;
                }
                1 => {
                    // get link to CG block
                    link_data_block = *val as u64;
                }
                _ => {}
            }
        }
        sr_block.link_next_block = link_next_block;
        sr_block.link_data_block = link_data_block;

        // save sr block to instance's arena
        let sr_id = instance.arena.new_node(Box::new(sr_block));
        // add sr block to parent
        parent_id
            .checked_append(sr_id, &mut instance.arena)
            .unwrap();
        let (zipped_reader, blocks_ids) = SRBlock::read_data(instance, &links, sr_id, 1)?;

        let sr = instance.get_mut_node_by_id::<SRBlock>(sr_id).unwrap();
        sr.block_id = Some(sr_id);
        sr.zip_reader = zipped_reader;
        sr.blocks = blocks_ids;

        // restore stream position
        buf_reader.seek(SeekFrom::Start(pos)).unwrap();

        Ok(link_next_block as u64)
    }
}

pub enum DataBlockType {
    DT,
    DZ,
}

/// (DataBlockType, position, block_id)
type DataBlockLink = (DataBlockType, u64, BlockId);

impl DataObject for SRBlock {}

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
    pub(crate) blocks: Vec<(u64, BlockId)>,
}

impl MDFObject for DGBlock {
    fn block_size(&self) -> u64 {
        self.block_size
    }

    fn name(&self) -> String {
        "Data group".to_string()
    }
}

impl DataObject for DGBlock {}

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

    pub fn read_dz_block(
        &mut self,
        dz_block: BlockId,
        position: u64,
        instance: &mut MDFFile,
    ) -> Result<(), MDFErrorKind> {
        let dz_block = instance.get_node_by_id::<DZBlock>(dz_block).unwrap();
        // first read data out
        let mut compressed_data = vec![0; (dz_block.length - 2) as usize];
        let zip_type = dz_block.zip_type;
        let group = dz_block.size / (dz_block.zip_parameter as i64);
        let group_size = dz_block.zip_parameter;
        let mut buf_reader = instance.get_buf_reader()?;
        let pos = buf_reader.stream_position().unwrap();
        buf_reader.seek(SeekFrom::Start(position + 2)).unwrap();
        buf_reader.read_exact(&mut compressed_data).unwrap();

        // get compressed data, then decompress it, write to a file
        match zip_type {
            Some(ZipType::Deflate) => {
                let mut decoder = libflate::deflate::Decoder::new(compressed_data.as_slice());
                let mut decompressed_data = vec![];
                decoder.read_to_end(&mut decompressed_data).unwrap();
                self.tmp_file.write_all(&mut decompressed_data).unwrap();
            }
            Some(ZipType::TransposeAndDeflate) => {
                let mut decoder = libflate::deflate::Decoder::new(compressed_data.as_slice());
                let mut decompressed_data = vec![];
                decoder.read_to_end(&mut decompressed_data).unwrap();
                // after get th edecompressed data, we need to transpose the data
                self.write_transpose_data(&decompressed_data, group as u32, group_size);
            }
            _ => {}
        }

        Ok(())
    }

    pub fn read_dt_block(
        &mut self,
        dt_block: BlockId,
        position: u64,
        instance: &mut MDFFile,
    ) -> Result<(), MDFErrorKind> {
        let dt_block = instance.get_node_by_id::<DTBlock>(dt_block).unwrap();
        let size = dt_block.size();

        let mut buf_reader = instance.get_buf_reader()?;
        let pos = buf_reader.stream_position().unwrap();
        buf_reader.seek(SeekFrom::Start(position)).unwrap();

        let mut buf_writer = self.get_buf_writer().unwrap();
        buf_writer.seek(SeekFrom::End(0)).unwrap();

        let count = 0;
        while size > count {
            let mut temp = vec![0; 65536.min(size - count) as usize];
            buf_reader.read_exact(&mut temp).unwrap();
            // write to buf writer
            buf_writer.write(&mut temp).unwrap();
        }
        buf_writer.flush();
        Ok(())
    }

    pub fn get_buf_writer(&mut self) -> Result<BufWriter<File>, MDFErrorKind> {
        let x = self
            .tmp_file
            .try_clone()
            .map_err(|x| MDFErrorKind::IOError(x))?;
        Ok(BufWriter::new(x))
    }

    fn write_transpose_data(&mut self, data: &Vec<u8>, group: u32, group_size: u32) {
        // first get a buf writer of the tmp file
        let mut buf_writer = self.get_buf_writer().unwrap();
        buf_writer.seek(SeekFrom::End(0)).unwrap();
        for i in 0..group {
            for j in 0..group_size {
                buf_writer.write(&[data[(j * group + i) as usize]]).unwrap();
            }
        }
        // if have some data left, write it all to the tmp file
        if data.len() as u64 > (group * group_size) as u64 {
            buf_writer
                .write(&data[(group * group_size) as usize..])
                .unwrap();
        }
        buf_writer.flush().unwrap();
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
            blocks: vec![],
        }
    }
    /// parse a DGBlock from file position at link_id, then add it to instance as child of parent_id
    /// return the postion of next block
    pub fn parse(
        byte_order: ByteOrder,
        link_id: u64,
        parent_id: BlockId,
        instance: &mut MDFFile,
    ) -> Result<(u64, BlockId), MDFErrorKind> {
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

        let (zipped_reader, blocks_ids) = DGBlock::read_data(instance, &links, dg_id, 2)?;

        let dg_ref = instance.get_mut_node_by_id::<DGBlock>(dg_id).unwrap();
        dg_ref.block_id = Some(dg_id);
        dg_ref.zip_reader = zipped_reader;

        // parse cg blocks
        let mut cg_link = link_cg_start;
        while cg_link > 0 {
            let (cg_next, block_id) = CGBlock::parse(byte_order, cg_link, dg_id, instance)?;
            instance.link_id_blocks.insert(cg_link, block_id);
            cg_link = cg_next;
        }

        // restore stream position
        buf_reader.seek(SeekFrom::Start(pos)).unwrap();
        Ok((link_next_block, dg_id))
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
        data_blocks: &mut Vec<DataBlockLink>,
    ) -> Result<BlockId, MDFErrorKind> {
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
            dl_link = DLBlock::parse(byte_order, dl_link, hl_id, instance, data_blocks)?.0;
        }

        // restore stream position
        buf_reader.seek(SeekFrom::Start(pos)).unwrap();

        Ok(hl_id)
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
        data_blocks: &mut Vec<DataBlockLink>,
    ) -> Result<(u64, BlockId), MDFErrorKind> {
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
                        data_blocks.push((DataBlockType::DT, *link as u64, dt_id));
                    }
                    "DZ" => {
                        // parse a DZ block
                        let (dz_id, data_link) =
                            DZBlock::parse(byte_order, *link as u64, dl_id, instance)?;
                        data_blocks.push((DataBlockType::DZ, data_link, dz_id));
                    }
                    _ => {
                        // do nothing
                    }
                }
            }
        }

        // restore stream position
        buf_reader.seek(SeekFrom::Start(pos)).unwrap();

        Ok((link_dl_next, dl_id))
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
    ) -> Result<(BlockId, u64), MDFErrorKind> {
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
        let compressed_data_pos = buf_reader.stream_position().unwrap();
        // let mut compressed_data = vec![0; dz_block.length as usize];
        // buf_reader.read_exact(&mut compressed_data).unwrap();

        // save dz block to instance's arena
        let dz_id = instance.arena.new_node(Box::new(dz_block));
        // add dt block to parent
        parent_id
            .checked_append(dz_id, &mut instance.arena)
            .unwrap();
        instance
            .get_mut_node_by_id::<DZBlock>(dz_id)
            .unwrap()
            .block_id = Some(dz_id);

        // restore stream position
        buf_reader.seek(SeekFrom::Start(pos)).unwrap();

        Ok((dz_id, compressed_data_pos))
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
    ) -> Result<(u64, BlockId), MDFErrorKind> {
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
        let mut cn_link = link_cn_start as u64;
        while cn_link > 0 {
            let (next_cn, block_id) = CNBlock::parse(byte_order, cn_link, cg_id, instance)?;
            // insert to instance's hashmap
            instance.link_id_blocks.insert(cn_link, block_id);
            cn_link = next_cn;
        }

        // parse sr blocks
        let mut sr_link = link_sr_start as u64;
        while sr_link > 0 {
            sr_link = SRBlock::parse(byte_order, sr_link, cg_id, instance)?;
        }

        // restore stream position
        buf_reader.seek(SeekFrom::Start(pos)).unwrap();
        Ok((link_next_cgblock as u64, cg_id))
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
    pub(crate) blocks: Vec<(i64, BlockId)>,
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
            blocks: vec![],
        }
    }
    /// parse CNBlocks
    pub(crate) fn parse(
        byte_order: ByteOrder,
        link_id: u64,
        parent_id: BlockId,
        instance: &mut MDFFile,
    ) -> Result<(u64, BlockId), MDFErrorKind> {
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
        // can be either atblock, hlblock, dlblock, or dzblock
        if link_sd_block > 0 {
            // todo:
            let key = link_sd_block as u64;
            if instance.link_id_blocks.contains_key(&key) {
                instance
                    .get_mut_node_by_id::<CNBlock>(cn_id)
                    .unwrap()
                    .sd_block = Some(instance.link_id_blocks.get(&key).unwrap().clone());
            } else {
                // if not inside dictionary, parse it here
                let mut data_blocks: Vec<DataBlockLink> = vec![];
                let mut block_ids = vec![];
                let (id, block_size, links, next_pos) =
                    read_v4_basic_info(link_sd_block as u64, instance)?;
                match id.as_str() {
                    "AT" => {
                        let (_, block_id) =
                            ATBlock::parse(byte_order, link_sd_block as u64, cn_id, instance)?;
                        block_ids.push((link_sd_block, block_id));
                    }
                    "HL" => {
                        let hl_id = HLBlock::parse(
                            ByteOrder::LittleEndian,
                            link_sd_block as u64,
                            cn_id,
                            instance,
                            &mut data_blocks,
                        )?;
                        block_ids.push((link_sd_block, hl_id));
                    }
                    "DL" => {
                        let (_, dl_id) = DLBlock::parse(
                            ByteOrder::LittleEndian,
                            link_sd_block as u64,
                            cn_id,
                            instance,
                            &mut data_blocks,
                        )?;
                        block_ids.push((link_sd_block, dl_id));
                    }
                    "DZ" => {
                        // get the DZ block
                        let (dz_id, link_dz_data) = DZBlock::parse(
                            ByteOrder::LittleEndian,
                            link_sd_block as u64,
                            cn_id,
                            instance,
                        )?;
                        block_ids.push((link_sd_block, dz_id));
                        data_blocks.push((DataBlockType::DZ, link_dz_data, dz_id));
                    }
                    _ => {}
                }
                // read datas out
                if block_ids.len() != 0 {
                    // set sdblock
                    instance
                        .get_mut_node_by_id::<CNBlock>(cn_id)
                        .unwrap()
                        .sd_block = Some(block_ids[0].1);
                    let mut zip_reader = None;
                    // if dz or dl block, read data out, and set zip reader
                    if data_blocks.len() != 0 {
                        // create a temp file, store handler inside self
                        zip_reader = Some(ZippedReader::new());
                        // get a mut ref to zip_reader
                        let writer = zip_reader.as_mut().unwrap();
                        // iterate data blocks and write data to tmp file
                        for (data_block_type, data_block_pos, data_block_id) in data_blocks {
                            match data_block_type {
                                DataBlockType::DT => {
                                    writer.read_dt_block(
                                        data_block_id,
                                        data_block_pos,
                                        instance,
                                    )?;
                                }
                                DataBlockType::DZ => {
                                    writer.read_dz_block(
                                        data_block_id,
                                        data_block_pos,
                                        instance,
                                    )?;
                                }
                            }
                        }
                    }
                    let cn_ref = instance.get_mut_node_by_id::<CNBlock>(cn_id).unwrap();
                    cn_ref.zip_reader = zip_reader;
                    cn_ref.blocks = block_ids;
                }
            }
        }

        // restore stream position
        buf_reader.seek(SeekFrom::Start(pos)).unwrap();
        Ok((link_next_cn as u64, cn_id))
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
    pub name: String,
    pub(crate) link_next_ev: u64,
    pub(crate) link_comment: u64,
    pub(crate) link_name: u64,
    pub(crate) ref_datas: Vec<u64>,
}

impl MDFObject for EVBlock {
    fn block_size(&self) -> u64 {
        self.block_size
    }

    fn name(&self) -> String {
        self.name.clone()
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
            link_comment: 0,
            link_name: 0,
            name: Default::default(),
        }
    }

    // parse ev block
    pub(crate) fn parse(
        byte_order: ByteOrder,
        link_id: u64,
        parent_id: BlockId,
        instance: &mut MDFFile,
    ) -> Result<(u64, BlockId), MDFErrorKind> {
        let mut buf_reader = instance.get_buf_reader()?;
        let pos = buf_reader.stream_position().unwrap();
        // seek to link_id position
        buf_reader.seek(SeekFrom::Start(link_id)).unwrap();
        // read the block basic info
        let (id, block_size, links, next_pos) = read_v4_basic_info(link_id, instance)?;

        // go to next pos, and construct ev block(32bytes)
        buf_reader.seek(SeekFrom::Start(next_pos)).unwrap();
        let mut data = vec![0; 32];
        buf_reader.read_exact(&mut data).unwrap();
        let mut ev_block = ev_block_basic(&data, id, block_size, links.len() as u64)
            .unwrap()
            .1;

        // get links out
        let mut link_next_ev = 0;
        let mut link_name = 0;
        let mut link_comment = 0;

        // loop links and assign links
        for (i, val) in links.iter().enumerate() {
            match i {
                0 => link_next_ev = *val,
                1 | 2 => {}
                3 => link_name = *val,
                4 => link_comment = *val,
                _ => {
                    if *val > 0 {
                        // add to refdata
                        ev_block.ref_datas.push(*val as u64);
                    }
                }
            }
        }
        // save links to ev_block
        ev_block.link_next_ev = link_next_ev as u64;
        ev_block.link_comment = link_comment as u64;
        ev_block.link_name = link_name as u64;

        // get comment from comment rdsdblock
        if link_comment > 0 {
            let comment_block = RDSDBlock::parse(byte_order, link_comment as u64, instance)?;
            ev_block.comment = String::from_utf8_lossy(&comment_block.data).to_string();
        }
        // get name from name rdsdblock
        if link_name > 0 {
            let name_block = RDSDBlock::parse(byte_order, link_name as u64, instance)?;
            ev_block.name = String::from_utf8_lossy(&name_block.data).to_string();
        }

        // store to arena
        let ev_id = instance.arena.new_node(Box::new(ev_block));
        // add cc block to parent
        parent_id
            .checked_append(ev_id, &mut instance.arena)
            .unwrap();
        instance
            .get_mut_node_by_id::<EVBlock>(ev_id)
            .unwrap()
            .block_id = Some(ev_id);

        // restore stream position
        buf_reader.seek(SeekFrom::Start(pos)).unwrap();

        Ok((link_next_ev as u64, ev_id))
    }
}
