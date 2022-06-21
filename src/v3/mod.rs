use std::collections::HashMap;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::ops::Range;
use std::result;
use std::sync::mpsc::channel;

use asammdf_derive::{
    channel_group_object, channel_object, comment_object, data_group_object, header_object,
    id_object, mdf_object, normal_object,
};
use asammdf_derive::{IDObject, MDFObject, PermanentBlock};
use chrono::Local;
use indextree::Arena;

use self::parser::{
    cg_block_basic, cn_block_basic, dg_block_basic, header_block_basic, read_le_i16, read_le_u16,
    read_le_u32, read_le_u64, read_str, tr_block_basic, trigger_evt, tx_block_basic, cc_block_basic, read_n_le_f64,
};

use super::SpecVer;
use super::UnfinalizedFlagsType;
use crate::PermanentBlock;
use crate::{BlockId, IDObject, MDFErrorKind, MDFFile};
use crate::{ByteOrder, ChannelType, RecordIDType, SignalType, SyncType, TimeQualityType};
use crate::{ConversionType, MDFObject};
use crate::{DateTime, Utc};
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
    #[derive(Debug,Clone,Copy)]
    pub enum FloatPointFormat {
        IEEE754,
        GFloat,
        DFloat,
    }
}

pub enum ExtensionType {
    DIM = 2,
    VectorCAN = 19,
}

#[mdf_object]
#[id_object]
#[derive(Debug, MDFObject, IDObject, Clone, PermanentBlock)]
pub struct IDBlock {
    pub byte_order: Option<ByteOrder>,
    pub code_page: u16,
    pub format_id: String,
    pub float_point_format: Option<FloatPointFormat>,
    pub program_id: String,
}

impl IDBlock {
    /// Create a v3::IDBlock with version default to `330`, spec_type default to `SpecVer::V3`
    /// and float_point_format default to `FloatPointFormat::IEEE754`
    fn new(program_id: String, code_page: u16) -> IDBlock {
        IDBlock {
            byte_order: None,
            code_page,
            format_id: "3.30    ".to_string(),
            float_point_format: Some(FloatPointFormat::IEEE754),
            program_id,
            block_size: 64,
            name: "ID".to_string(),
            file_id: "MDF     ".to_string(),
            spec_type: Some(SpecVer::V3),
            unfinalized_flags: None,
            version: 330,
            custom_flags: 0,
        }
    }

    /// parse a IDBlock from a 64byte u8 slice
    pub(crate) fn parse(input: &[u8]) -> Option<IDBlock> {
        parser::id_block(input).map_or(None, |x| Some(x.1))
    }
}

#[comment_object]
#[header_object]
#[normal_object]
#[derive(Debug, Clone, PermanentBlock)]
pub struct HDBlock {
    pub author: String,
    pub date: String,
    pub organization: String,
    pub program_specific_data: String,
    pub project: String,
    pub subject: String,
    pub time: String,
    pub time_quality: Option<TimeQualityType>,
    pub timer_id: Option<String>,
    pub timestamp: Option<u64>,
    pub utc_offset: Option<i16>,

    link_first_file_group: u32,
    link_file_comment_txt: u32,
    link_program_block: u32,
}

impl MDFObject for HDBlock {
    fn block_size(&self) -> u64 {
        self.block_size
    }

    fn name(&self) -> String {
        self.id.clone()
    }
}

impl HDBlock {
    fn new(date_time: DateTime<Local>) -> HDBlock {
        HDBlock {
            author: Default::default(),
            date: date_time.format("%d:%m:%Y").to_string(),
            organization: Default::default(),
            program_specific_data: Default::default(),
            project: Default::default(),
            subject: Default::default(),
            time: date_time.format("%H:%M:%S").to_string(),
            time_quality: None, //Some(time_quality),
            timer_id: None,     //"Local PC Reference Time".to_string(),
            timestamp: None,    //date_time.timestamp(),
            utc_offset: None, //(Local::now().offset().local_minus_utc() as f64 / 3600.0f64).round() as i16,
            block_size: 208,
            id: "HD".to_string(),
            comment: Default::default(),
            link_file_comment_txt: 0,
            link_first_file_group: 0,
            link_program_block: 0,
        }
    }

    pub(crate) fn parse(byte_order: ByteOrder, instance: &mut MDFFile) -> Result<(), MDFErrorKind> {
        // get file handler out
        // when id block parsed, position of file handler inside MDFFile is 64.
        let mut buf_reader = instance.get_buf_reader()?;
        // get the position of current stream
        let pos = buf_reader.stream_position().unwrap();
        // basic info bytes count is 164 bytes
        let mut basic_info = [0; 164];
        buf_reader.read_exact(&mut basic_info).unwrap();
        let mut hd_block = header_block_basic(&basic_info).unwrap().1;
        // optionals bytes
        if buf_reader.stream_position().unwrap() - pos < hd_block.block_size {
            // 8 bytes timestamp
            let mut buf = [0; 8];
            buf_reader.read_exact(&mut buf).unwrap();
            hd_block.timestamp = Some(read_le_u64(&buf).unwrap().1);
        }
        if buf_reader.stream_position().unwrap() - pos < hd_block.block_size {
            // 2 bytes utcoffset
            let mut buf = [0; 2];
            buf_reader.read_exact(&mut buf).unwrap();
            hd_block.utc_offset = Some(read_le_i16(&buf).unwrap().1);
        }
        if buf_reader.stream_position().unwrap() - pos < hd_block.block_size {
            // 2 bytes time quality type
            let mut buf = [0; 2];
            buf_reader.read_exact(&mut buf).unwrap();
            let time_quality = read_le_u16(&buf).unwrap().1;
            hd_block.time_quality = (time_quality as u32).try_into().map_or(None, |x| Some(x));
        }
        if buf_reader.stream_position().unwrap() - pos < hd_block.block_size {
            // 32 bytes timer id bytes
            let mut buf = [0; 32];
            buf_reader.read_exact(&mut buf).unwrap();
            hd_block.timer_id = Some(read_str(&buf, 32).unwrap().1);
        }
        let mut group_id = hd_block.link_first_file_group as u64;
        let comment_link = hd_block.link_file_comment_txt as u64;
        let program_block_link = hd_block.link_program_block as u64;
        // read comment block
        let comment_block = TXBlock::parse(instance, comment_link);
        hd_block.comment = comment_block
            .as_ref()
            .map_or(Default::default(), |x| x.text.clone());
        let comment_id = comment_block.map(|x| instance.arena.new_node(Box::new(x)));
        // read program specific data
        println!("{program_block_link}");
        let program_block = TXBlock::parse(instance, program_block_link);
        hd_block.program_specific_data = program_block
            .as_ref()
            .map_or(Default::default(), |x| x.text.clone());
        let program_block_id = program_block.map(|x| instance.arena.new_node(Box::new(x)));

        println!("{:?}", hd_block);
        let hd_id = instance.arena.new_node(Box::new(hd_block));
        // add hd_block as id_block child
        let id_id = instance.id.as_ref().unwrap().clone();
        id_id.append(hd_id, &mut instance.arena);
        // add comment and program specific data as child
        comment_id.map(|node_id| {
            hd_id.append(node_id, &mut instance.arena);
        });
        program_block_id.map(|node_id| {
            hd_id.append(node_id, &mut instance.arena);
        });
        // process date group block
        while group_id > 0 {
            group_id = DGBlock::parse(byte_order, hd_id, group_id, instance).unwrap();
        }
        // debug
        println!(
            "header children number is : {}",
            hd_id.descendants(&mut instance.arena).count()
        );
        Ok(())
    }
}

#[normal_object]
#[comment_object]
#[data_group_object]
#[derive(Debug, Clone, PermanentBlock)]
pub struct DGBlock {
    link_data_records: u32,
    link_next_cgblock: u32,
    link_next_dgblock: u32,
    link_trblock: u32,
    pub tr_block: Option<BlockId>,
}

impl MDFObject for DGBlock {
    fn block_size(&self) -> u64 {
        self.block_size
    }

    fn name(&self) -> String {
        "Data group".to_string()
    }
}

impl DGBlock {
    pub fn new() -> DGBlock {
        DGBlock {
            link_data_records: 0,
            link_next_cgblock: 0,
            link_next_dgblock: 0,
            link_trblock: 0,
            id: "DG".to_string(),
            block_size: 28,
            comment: Default::default(),
            record_id_type: None,
            tr_block: None,
        }
    }

    pub(crate) fn parse(
        byte_order: ByteOrder,
        parent_id: BlockId,
        link_id: u64,
        instance: &mut MDFFile,
    ) -> Result<u64, MDFErrorKind> {
        let mut buf_reader = instance.get_buf_reader()?;
        let pos = buf_reader.stream_position().unwrap();
        buf_reader.seek(SeekFrom::Start(link_id)).unwrap();
        let mut buf = [0; 24];
        buf_reader.read_exact(&mut buf).unwrap();
        let dg_block = dg_block_basic(&buf).unwrap().1;
        let mut cg_link = dg_block.link_next_cgblock as u64;
        let tr_link = dg_block.link_trblock as u64;
        // return the next dg link
        let result = dg_block.link_next_dgblock;
        // save this data group to arena, and add as hdblock's children
        let dg_id = instance.arena.new_node(Box::new(dg_block));
        instance.link_id_blocks.insert(link_id, dg_id);
        parent_id.append(dg_id, &mut instance.arena);
        // then parse cg blocks
        while cg_link > 0 {
            cg_link = CGBlock::parse(byte_order, dg_id, cg_link, instance).unwrap();
        }
        // then parse trblock
        if tr_link > 0 {
            let tr_block_id = TRBlock::parse(byte_order, dg_id, tr_link, instance).unwrap();
            let x = instance.get_mut_node_by_id::<DGBlock>(dg_id).unwrap();
            x.tr_block = Some(tr_block_id);
        }
        buf_reader.seek(SeekFrom::Start(pos)).unwrap();
        Ok(result as u64)
    }
}

#[normal_object]
#[channel_group_object]
#[comment_object]
#[derive(Debug, Clone, PermanentBlock)]
pub struct CGBlock {
    pub record_id: u16,
    link_cg_comment: u32,
    link_first_cnblock: u32,
    link_first_srblock: u32,
    link_next_cgblock: u32,
}

impl MDFObject for CGBlock {
    fn block_size(&self) -> u64 {
        self.block_size
    }

    fn name(&self) -> String {
        "Channel Group".to_string()
    }
}

impl CGBlock {
    pub fn new(comment: String) -> CGBlock {
        CGBlock {
            record_id: 0,
            link_cg_comment: 0,
            link_first_cnblock: 0,
            link_first_srblock: 0,
            link_next_cgblock: 0,
            record_count: 0,
            record_size: 0,
            comment: comment,
            id: "CG".to_string(),
            block_size: 30,
        }
    }

    pub(crate) fn parse(
        byte_order: ByteOrder,
        parent_id: BlockId,
        link_id: u64,
        instance: &mut MDFFile,
    ) -> Result<u64, MDFErrorKind> {
        let mut buf_reader = instance.get_buf_reader()?;
        let pos = buf_reader.stream_position().unwrap();
        buf_reader.seek(SeekFrom::Start(link_id)).unwrap();
        let mut buf = [0; 26];
        buf_reader.read_exact(&mut buf).unwrap();
        let mut cg_block = cg_block_basic(&buf).unwrap().1;
        // optional param link_first_srblock
        if buf_reader.stream_position().unwrap() - link_id < cg_block.block_size {
            let mut buf = [0; 4];
            buf_reader.read_exact(&mut buf).unwrap();
            cg_block.link_first_srblock = read_le_u32(&buf).unwrap().1;
        }
        let result = cg_block.link_next_cgblock;

        // parse comment block
        let comment_block = TXBlock::parse(instance, cg_block.link_cg_comment as u64);
        cg_block.comment = comment_block
            .as_ref()
            .map_or(Default::default(), |x| x.text.clone());
        let comment_id = comment_block.map(|x| instance.arena.new_node(Box::new(x)));
        // save block to arena
        let cg_id = instance.arena.new_node(Box::new(cg_block));
        comment_id.map(|node_id| cg_id.append(node_id, &mut instance.arena));
        instance.link_id_blocks.insert(link_id, cg_id);
        parent_id.append(cg_id, &mut instance.arena);

        // parse channel blocks

        // parse srblocks

        buf_reader.seek(SeekFrom::Start(pos)).unwrap();
        Ok(result as u64)
    }
}

#[normal_object]
#[channel_object]
#[comment_object]
#[derive(Debug, Clone, PermanentBlock)]
pub struct CNBlock {
    pub description: String,
    pub display_name: String,
    pub long_name: String,
    pub rate: f64,
    pub name: String,
    link_ccblock: u32,
    link_cdblock: u32,
    link_ceblock: u32,
    link_channel_comment: u32,
    link_mcd_unique_name: u32,
    link_next_cnblock: u32,
    link_signal_display_identifier: u32,
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
    pub fn new(
        signal_type: SignalType,
        channel_type: ChannelType,
        name: String,
        description: String,
        bit_offset: u32,
        add_offset: u32,
        bits_count: u32,
    ) -> CNBlock {
        if channel_type != ChannelType::Master && channel_type != ChannelType::Data {
            panic!("MDFv3 only supported channel type: master, data");
        }
        CNBlock {
            description,
            display_name: Default::default(),
            long_name: Default::default(),
            rate: 0.0,
            name,
            link_ccblock: 0,
            link_cdblock: 0,
            link_ceblock: 0,
            link_channel_comment: 0,
            link_mcd_unique_name: 0,
            link_next_cnblock: 0,
            link_signal_display_identifier: 0,
            id: "CN".to_string(),
            block_size: 228,
            add_offset,
            bitmask_cache: 0,
            bit_offset: bit_offset as u16,
            channel_type: Some(channel_type),
            max_raw: 0.0,
            min_raw: 0.0,
            bits_count,
            signal_type: Some(signal_type),
            sync_type: None,
            unit: Default::default(),
            comment: Default::default(),
        }
    }

    pub(crate) fn parse(
        byte_order: ByteOrder,
        parent_id: BlockId,
        link_id: u64,
        instance: &mut MDFFile,
    ) -> Result<u64, MDFErrorKind> {
        let mut buf_reader = instance.get_buf_reader()?;
        let pos = buf_reader.stream_position().unwrap();
        buf_reader.seek(SeekFrom::Start(link_id)).unwrap();
        let mut buf = [0; 26];
        buf_reader.read_exact(&mut buf).unwrap();
        let mut cn_block = cn_block_basic(&buf, byte_order).unwrap().1;

        if buf_reader.stream_position().unwrap() - link_id < cn_block.block_size {
            let mut buf = [0; 4];
            buf_reader.read_exact(&mut buf).unwrap();
            cn_block.link_mcd_unique_name = read_le_u32(&buf).unwrap().1;
        }
        if buf_reader.stream_position().unwrap() - link_id < cn_block.block_size {
            let mut buf = [0; 4];
            buf_reader.read_exact(&mut buf).unwrap();
            cn_block.link_signal_display_identifier = read_le_u32(&buf).unwrap().1;
        }
        if buf_reader.stream_position().unwrap() - link_id < cn_block.block_size {
            let mut buf = [0; 2];
            buf_reader.read_exact(&mut buf).unwrap();
            cn_block.add_offset = read_le_u16(&buf).unwrap().1 as u32;
        }
        let result = cn_block.link_next_cnblock as u64;
        // parse comment
        let comment_block = TXBlock::parse(instance, cn_block.link_channel_comment as u64);
        cn_block.comment = comment_block
            .as_ref()
            .map_or(Default::default(), |x| x.text.clone());
        let comment_id = comment_block.map(|x| instance.arena.new_node(Box::new(x)));
        // parse longname
        let longname_block = TXBlock::parse(instance, cn_block.link_mcd_unique_name as u64);
        cn_block.long_name = longname_block
            .as_ref()
            .map_or(Default::default(), |x| x.text.clone());
        let longnam_id = longname_block.map(|x| instance.arena.new_node(Box::new(x)));
        // parse display name
        let displayname_block =
            TXBlock::parse(instance, cn_block.link_signal_display_identifier as u64);
        cn_block.long_name = displayname_block
            .as_ref()
            .map_or(Default::default(), |x| x.text.clone());
        let displayname_id = displayname_block.map(|x| instance.arena.new_node(Box::new(x)));

        // parse ccblock
        if cn_block.link_ccblock > 0 {}
        // parse ceblock
        if cn_block.link_ceblock > 0 {}
        // parse cdblock
        if cn_block.link_cdblock > 0 {}

        buf_reader.seek(SeekFrom::Start(pos)).unwrap();
        Ok(result)
    }
}

#[normal_object]
#[comment_object]
#[derive(Debug, Clone, PermanentBlock)]
pub struct CCBlock {
    pub conversion_type: Option<ConversionType>,
    pub date: Option<DateType>,
    pub default_text: String,
    pub foumula: String,
    pub inv_ccblock: Option<BlockId>,
    pub tab_pairs: Option<HashMap<f64, f64>>,
    pub text_table_pairs: Option<HashMap<f64, String>>,
    pub text_range_pairs: Option<HashMap<String, Range<f64>>>,
    pub max: f64,
    pub min: f64,
    pub params: Option<Vec<f64>>,
    pub tab_size: u16,
    pub time: Option<TimeType>,
    pub unit: String,
}

impl MDFObject for CCBlock {
    fn block_size(&self) -> u64 {
        self.block_size
    }

    fn name(&self) -> String {
        self.id.to_string()
    }
}

impl CCBlock {
    pub fn new(conversion_type: Option<ConversionType>, unit: String, min: f64, max: f64) -> Self {
        CCBlock {
            conversion_type: conversion_type,
            date: None,
            default_text: Default::default(),
            foumula: Default::default(),
            inv_ccblock: None,
            tab_pairs: None,
            text_table_pairs: None,
            text_range_pairs: None,
            max,
            min,
            params: None,
            tab_size: 0,
            time: None,
            unit: Default::default(),
            id: "CC".to_string(),
            block_size: 46,
            comment: Default::default(),
        }
    }

    pub(crate) fn parse(
        byte_order: ByteOrder,
        parent_id: BlockId,
        link_id: u64,
        instance: &mut MDFFile,
    ) -> Result<BlockId, MDFErrorKind> {
        let mut buf_reader = instance.get_buf_reader()?;
        let pos = buf_reader.stream_position().unwrap();
        buf_reader.seek(SeekFrom::Start(link_id)).unwrap();
        let mut buf = [0; 26];
        buf_reader.read_exact(&mut buf).unwrap();
        let mut cc_block = cc_block_basic(&buf, byte_order).unwrap().1;
        
        buf_reader.seek(SeekFrom::Start(pos)).unwrap();
        Ok(result)
    }

    fn parse_params(&mut self,instance: &mut MDFFile) {
        let conv_type = match self.conversion_type {
            Some(conv_type) => {
                conv_type
            },
            _ => return
        };
        let mut buf_reader = instance.get_buf_reader().unwrap();
        match conv_type {
            ConversionType::ParametricLinear |
            ConversionType::Polynomial |
            ConversionType::Exponential |
            ConversionType::Logarithmic |
            ConversionType::Rational => {
                if self.tab_size > 0 {
                    let mut buf = vec![0;(self.tab_size as usize)*8];
                    buf_reader.read_exact(&mut buf).unwrap();
                    let params = read_n_le_f64(&buf, self.tab_size as usize);
                    
                }
            },
            ConversionType::TabInt |
            ConversionType::Tab => {

            },
            ConversionType::TextFormula => {

            },
            ConversionType::TextTable => {

            },
            ConversionType::TextRange => {

            },
            ConversionType::Date => {

            },
            ConversionType::Time => {

            },
            _ => {
                panic!("Doesn't supported thid kind of conversion type in: {}",self.name());
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct DateType {
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub month: u8,
    pub ms: u16,
    pub year: u8,
}

impl DateType {
    pub fn new(ms: u16, minute: u8, hour: u8, day: u8, month: u8, year: u8) -> DateType {
        DateType {
            day,
            hour,
            minute,
            month,
            ms,
            year,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TimeType {
    pub days: u8,
    pub ms: u32,
}

impl TimeType {
    pub fn new(ms: u32, days: u8) -> TimeType {
        TimeType { days, ms }
    }
}

#[normal_object]
pub struct CEBlock {
    pub dim: Option<DimType>,
    pub extension_type: Option<ExtensionType>,
    pub vector_can: Option<VectorCANType>,
}

pub struct DimType {
    pub address: u32,
    pub description: String,
    pub ecu_id: String,
    pub module: u16,
}

impl DimType {
    pub fn new(module: u16, address: u32, description: String, ecu_id: String) -> DimType {
        DimType {
            address,
            description,
            ecu_id,
            module,
        }
    }
}

#[normal_object]
#[derive(Debug, Clone, PermanentBlock)]
pub struct CDBlock {
    pub dependency_type: u16,
}

impl MDFObject for CDBlock {
    fn block_size(&self) -> u64 {
        self.block_size
    }

    fn name(&self) -> String {
        "Dependency".to_string()
    }
}

impl CDBlock {
    pub fn new() -> CDBlock {
        CDBlock {
            dependency_type: 0,
            id: "CD".to_string(),
            block_size: 8,
        }
    }
}

#[derive(Debug, Clone)]
pub struct VectorCANType {
    pub id: u32,
    pub index: u32,
    pub message: String,
    pub sender: String,
}

impl VectorCANType {
    pub fn new(id: u32, index: u32, message: String, sender: String) -> VectorCANType {
        VectorCANType {
            id,
            index,
            message,
            sender,
        }
    }
}
/// Trigger Block
#[normal_object]
#[comment_object]
#[derive(Debug, Clone, PermanentBlock)]
pub struct TRBlock {
    pub trigger_events: Vec<TriggerEvent>,
    pub link_comment: u32,
}

impl MDFObject for TRBlock {
    fn block_size(&self) -> u64 {
        self.block_size
    }

    fn name(&self) -> String {
        "Trigger".to_string()
    }
}

impl TRBlock {
    pub fn new(trigger_events: Vec<TriggerEvent>) -> TRBlock {
        TRBlock {
            block_size: 10 + (trigger_events.len() as u64) * 3 * 8,
            trigger_events: trigger_events,
            link_comment: 0,
            id: "TR".to_string(),
            comment: Default::default(),
        }
    }

    pub(crate) fn parse(
        byte_order: ByteOrder,
        parent_id: BlockId,
        link_id: u64,
        instance: &mut MDFFile,
    ) -> Result<BlockId, MDFErrorKind> {
        let mut buf_reader = instance.get_buf_reader()?;
        let pos = buf_reader.stream_position().unwrap();
        buf_reader.seek(SeekFrom::Start(link_id)).unwrap();
        let mut buf = [0; 10];
        buf_reader.read_exact(&mut buf).unwrap();
        let (mut tr_block, trig_evt_num) = tr_block_basic(&buf).unwrap().1;
        // parse trigger events
        for _ in [0..trig_evt_num] {
            let mut buf = [0; 3 * 8];
            buf_reader.read_exact(&mut buf).unwrap();
            tr_block.trigger_events.push(trigger_evt(&buf).unwrap().1);
        }
        // parse comment block
        let comment_block = TXBlock::parse(instance, tr_block.link_comment as u64);
        tr_block.comment = comment_block
            .as_ref()
            .map_or(Default::default(), |x| x.text.clone());
        let comment_id = comment_block.map(|x| instance.arena.new_node(Box::new(x)));
        // save tr block to arena
        let tr_id = instance.arena.new_node(Box::new(tr_block));
        comment_id.map(|node_id| tr_id.append(node_id, &mut instance.arena));
        parent_id.append(tr_id, &mut instance.arena);

        buf_reader.seek(SeekFrom::Start(pos)).unwrap();

        Ok(tr_id)
    }
}

#[derive(Debug, Clone)]
pub struct TriggerEvent {
    pub post_time: f64,
    pub pre_time: f64,
    pub time: f64,
}

impl TriggerEvent {
    pub fn new(time: f64, pre_time: f64, post_time: f64) -> TriggerEvent {
        TriggerEvent {
            post_time,
            pre_time,
            time,
        }
    }
}

/// Text Block
///
/// take care that block_size of this type may not equal to len(self.text) + 4 + 1,
/// leading and trailing `\0` in self.text will be trimed.
/// when writing to disk, block_size will be calculatd directly from self.text (self.text + 4 + 1).
#[normal_object]
#[derive(Debug, Clone, PermanentBlock)]
pub struct TXBlock {
    /// a string with a eol(`\0`) char, block size include eol char.
    pub text: String,
}

impl MDFObject for TXBlock {
    fn block_size(&self) -> u64 {
        self.block_size
    }

    fn name(&self) -> String {
        self.id.clone()
    }
}

impl TXBlock {
    pub fn new(text: String) -> TXBlock {
        TXBlock {
            text: text.clone(),
            id: "TX".to_string(),
            block_size: 4 + (text.len() as u64) + 1,
        }
    }
    /// giving a link to text block, parsing to `TXBlock`
    pub(crate) fn parse(instance: &mut MDFFile, link_txt: u64) -> Option<TXBlock> {
        let mut buf_reader = instance.get_buf_reader().unwrap();
        if link_txt == 0 {
            return None;
        }
        // push prev stream pos
        let prev_pos = buf_reader.stream_position().unwrap();
        // set stream pos to link_comment
        buf_reader.seek(SeekFrom::Start(link_txt)).unwrap();
        let mut buf = [0; 4];
        buf_reader.read_exact(&mut buf).unwrap();
        let mut tx_block = tx_block_basic(&buf).unwrap().1;
        println!("{}", tx_block.block_size);
        // get variable length text, len = tx_block.block_size - 4
        let len = tx_block.block_size - 4;
        if len > 0 {
            let mut buf: Vec<u8> = vec![0; len as usize];
            buf_reader.read_exact(buf.as_mut_slice()).unwrap();
            tx_block.text = read_str(
                &buf.as_slice()[0..(len as usize)],
                (tx_block.block_size - 4) as u32,
            )
            .unwrap()
            .1;
        }
        // pop prev stream pos
        buf_reader.seek(SeekFrom::Start(prev_pos)).unwrap();
        Some(tx_block)
    }
}

pub struct DependencyType {
    pub link_channel_group: i64,
    pub link_channel: i64,
    pub link_data_grup: i64,
}

impl DependencyType {
    pub fn new(link_dg: i64, link_cg: i64, link_cn: i64) -> DependencyType {
        DependencyType {
            link_channel_group: link_cg,
            link_channel: link_cn,
            link_data_grup: link_dg,
        }
    }
}
