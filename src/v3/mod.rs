use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::ops::Range;

use asammdf_derive::{
    basic_object, channel_group_object, channel_object, comment_object, data_group_object,
    id_object, mdf_object, normal_object_v3,
};

use asammdf_derive::{IDObject, MDFObject, PermanentBlock};
use chrono::{DateTime, Local, TimeZone, Utc};

use self::parser::{
    cc_block_basic, cdblock_basic, ce_block_basic, cg_block_basic, cn_block_basic, dependency_type,
    dg_block_basic, header_block_basic, parse_datetype, parse_dim_type, parse_timetype,
    sr_block_basic, tr_block_basic, trigger_evt, tx_block_basic, vector_can_type,
};
use crate::misc::helper::{
    read_le_i16, read_le_u16, read_le_u32, read_le_u64, read_n_le_f64, read_str,
};
use crate::misc::{transform_params, PARAM_LINEAR_ARRAY_I, RATIONAL_PARAM_ARRAY_I};
use crate::{
    BlockId, ByteOrder, CCObject, CGObject, CNObject, ChannelType, ConversionType, DGObject,
    DependencyType, HDObject, IDObject, MDFErrorKind, MDFFile, MDFObject, PermanentBlock,
    RecordIDType, SRObject, SignalType, SpecVer, SyncType, TimeQualityType, UnfinalizedFlagsType,
};

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

enum_u32_convert! {
    #[derive(Debug, Clone)]
    pub enum ExtensionType {
        DIM = 2,
        VectorCAN = 19,
    }
}
#[mdf_object]
#[id_object]
#[basic_object]
#[derive(Debug, MDFObject, IDObject, Clone, PermanentBlock)]
pub struct IDBlock {
    pub byte_order: Option<ByteOrder>,
    pub code_page: u16,
    format_id: String,
    pub float_point_format: Option<FloatPointFormat>,
    pub program_id: String,
}

impl IDBlock {
    /// Create a v3::IDBlock with version default to `330`, spec_type default to `SpecVer::V3`
    /// and float_point_format default to `FloatPointFormat::IEEE754`
    pub fn new(program_id: String, code_page: u16) -> IDBlock {
        IDBlock {
            byte_order: None,
            code_page,
            format_id: "3.30    ".to_string(),
            float_point_format: Some(FloatPointFormat::IEEE754),
            program_id,
            block_size: 64,
            name: "ID".to_string(),
            file_id: "MDF     ".to_string(),
            spec_type: SpecVer::V3,
            unfinalized_flags: None,
            version: 330,
            custom_flags: 0,
            block_id: None,
        }
    }
    /// get `format_id` of IDBlock
    pub fn format_id(&self) -> String {
        self.format_id.to_string()
    }

    /// parse a IDBlock from a 64byte u8 slice
    pub(crate) fn parse(input: &[u8]) -> Option<IDBlock> {
        parser::id_block(input).map_or(None, |x| Some(x.1))
    }
}

#[comment_object]
#[normal_object_v3]
#[basic_object]
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

    pub(crate) link_first_file_group: u32,
    pub(crate) link_file_comment_txt: u32,
    pub(crate) link_program_block: u32,
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
            block_id: None,
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
        // read program specific data
        println!("{program_block_link}");
        let program_block = TXBlock::parse(instance, program_block_link);
        hd_block.program_specific_data = program_block
            .as_ref()
            .map_or(Default::default(), |x| x.text.clone());

        let hd_id = instance.arena.new_node(Box::new(hd_block));
        // add hd_block as id_block child
        let id_id = instance.id.as_ref().unwrap().clone();
        instance
            .get_mut_node_by_id::<IDBlock>(id_id)
            .unwrap()
            .block_id = Some(id_id);
        id_id.checked_append(hd_id, &mut instance.arena).unwrap();
        // process date group block
        while group_id > 0 {
            group_id = DGBlock::parse(byte_order, hd_id, group_id, instance).unwrap();
        }
        Ok(())
    }
}

/// trait alias for specific `HDObject` trait
trait HDObjectV3<'a>: HDObject<'a, DGBlock> {}

impl<'a> HDObjectV3<'a> for HDBlock {}

impl<'a> HDObject<'a, DGBlock> for HDBlock {
    /// get data groups contained inside this file
    fn data_groups(&self, mdf_file: &'a MDFFile) -> Option<Vec<&'a DGBlock>> {
        let id = self.block_id.clone();
        id.map_or(None, |node_id| {
            mdf_file.get_nodes_by_parent::<DGBlock>(node_id)
        })
    }
    /// get the recording time of the file
    fn recording_time(&self) -> Option<DateTime<Utc>> {
        self.timestamp.map_or_else(
            || {
                let time_str = self.date.clone() + " " + &self.time;
                Utc.datetime_from_str(&time_str, "%d:%m:%Y %H:%M:%S")
                    .map_or(None, |time| Some(time))
            },
            |nanos| Some(Utc.timestamp_nanos(nanos as i64)),
        )
    }
}

#[normal_object_v3]
#[comment_object]
#[data_group_object]
#[basic_object]
#[derive(Debug, Clone, PermanentBlock)]
pub struct DGBlock {
    pub(crate) link_data_records: u32,
    pub(crate) link_next_cgblock: u32,
    pub(crate) link_next_dgblock: u32,
    pub(crate) link_trblock: u32,
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

impl DGObject for DGBlock {}

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
            block_id: None,
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
        instance
            .get_mut_node_by_id::<DGBlock>(dg_id)
            .unwrap()
            .block_id = Some(dg_id);
        instance.link_id_blocks.insert(link_id, dg_id);
        parent_id
            .checked_append(dg_id, &mut instance.arena)
            .unwrap();
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

#[normal_object_v3]
#[channel_group_object]
#[comment_object]
#[basic_object]
#[derive(Debug, Clone, PermanentBlock)]
pub struct CGBlock {
    pub record_id: u16,
    pub(crate) link_cg_comment: u32,
    pub(crate) link_first_cnblock: u32,
    pub(crate) link_first_srblock: u32,
    pub(crate) link_next_cgblock: u32,
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
            block_id: None,
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
        let cn_link = cg_block.link_first_cnblock;
        let sr_link = cg_block.link_first_srblock;
        // parse comment block
        let comment_block = TXBlock::parse(instance, cg_block.link_cg_comment as u64);
        cg_block.comment = comment_block
            .as_ref()
            .map_or(Default::default(), |x| x.text.clone());
        // save block to arena
        let cg_id = instance.arena.new_node(Box::new(cg_block));
        instance
            .get_mut_node_by_id::<CGBlock>(cg_id)
            .unwrap()
            .block_id = Some(cg_id);
        instance.link_id_blocks.insert(link_id, cg_id);
        parent_id
            .checked_append(cg_id, &mut instance.arena)
            .unwrap();
        // parse channel blocks
        let mut cn_link = cn_link as u64;
        while cn_link > 0 {
            cn_link = CNBlock::parse(byte_order, cg_id, cn_link, instance)?;
        }
        // parse srblocks
        let mut sr_link = sr_link as u64;
        while sr_link > 0 {
            sr_link = SRBlock::parse(byte_order, cg_id, sr_link, instance)?;
        }
        buf_reader.seek(SeekFrom::Start(pos)).unwrap();
        Ok(result as u64)
    }
}

/// trait alias for specific `CGObject` trait
trait CGObjectV3<'a>: CGObject<'a, CNBlock, DGBlock, SRBlock> {}

impl<'a> CGObjectV3<'a> for CGBlock {}

impl<'a> CGObject<'a, CNBlock, DGBlock, SRBlock> for CGBlock {
    /// get child channel blocks of this channel group block
    fn cnblocks(&self, mdf_file: &'a MDFFile) -> Option<Vec<&'a CNBlock>> {
        let id = self.block_id.clone();
        id.map_or(None, |node_id| {
            mdf_file.get_nodes_by_parent::<CNBlock>(node_id)
        })
    }
    // get parent data group block
    fn dgblock(&self, mdf_file: &'a MDFFile) -> Option<&'a DGBlock> {
        let id = self.block_id.clone();
        id.map_or(None, |node_id| {
            mdf_file.get_ancestor_node_by_id::<DGBlock>(node_id)
        })
    }
    /// get child srblocks of this channel group block
    fn srblocks(&self, mdf_file: &'a MDFFile) -> Option<Vec<&'a SRBlock>> {
        let id = self.block_id.clone();
        id.map_or(None, |node_id| {
            mdf_file.get_nodes_by_parent::<SRBlock>(node_id)
        })
    }
    /// get total record size of this channel group
    fn get_record_size(&self, mdf_file: &'a MDFFile) -> i64 {
        let mut size = self.record_size;
        let record_id_type = self.dgblock(mdf_file).unwrap().record_id_type;
        match record_id_type {
            Some(RecordIDType::Before8Bit) => {
                size += 1;
            }
            Some(RecordIDType::Before16Bit) => {
                size += 2;
            }
            Some(RecordIDType::Before32Bit) => {
                size += 4;
            }
            Some(RecordIDType::Before64Bit) => {
                size += 8;
            }
            Some(RecordIDType::BeforeAndAfter8Bit) => {
                size += 2;
            }
            _ => {}
        }
        size as i64
    }
}

#[normal_object_v3]
#[channel_object]
#[comment_object]
#[basic_object]
#[derive(Debug, Clone, PermanentBlock)]
pub struct CNBlock {
    pub description: String,
    pub display_name: String,
    pub long_name: String,
    pub rate: f64,
    pub name: String,
    pub(crate) link_ccblock: u32,
    pub(crate) link_cdblock: u32,
    pub(crate) link_ceblock: u32,
    pub(crate) link_channel_comment: u32,
    pub(crate) link_mcd_unique_name: u32,
    pub(crate) link_next_cnblock: u32,
    pub(crate) link_signal_display_identifier: u32,
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
            bit_offset: bit_offset as u16,
            channel_type: Some(channel_type),
            max_raw: f64::NAN,
            min_raw: f64::NAN,
            bits_count,
            signal_type: Some(signal_type),
            sync_type: None,
            unit: Default::default(),
            comment: Default::default(),
            block_id: None,
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
        let mut buf = [0; 218];
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
        // parse longname
        let longname_block = TXBlock::parse(instance, cn_block.link_mcd_unique_name as u64);
        cn_block.long_name = longname_block
            .as_ref()
            .map_or(Default::default(), |x| x.text.clone());
        // parse display name
        let displayname_block =
            TXBlock::parse(instance, cn_block.link_signal_display_identifier as u64);
        cn_block.display_name = displayname_block
            .as_ref()
            .map_or(Default::default(), |x| x.text.clone());
        // store link before store to arena for convenience
        let link_ccblock = cn_block.link_ccblock;
        let link_ceblock = cn_block.link_ceblock;
        let link_cdblock = cn_block.link_cdblock;
        let cn_id = instance.arena.new_node(Box::new(cn_block));
        // parse ccblock
        if link_ccblock > 0 {
            CCBlock::parse(byte_order, cn_id, link_ccblock as u64, instance)?;
        }
        // parse ceblock
        if link_ceblock > 0 {
            CEBlock::parse(byte_order, cn_id, link_ceblock as u64, instance)?;
        }
        // parse cdblock
        if link_cdblock > 0 {
            CDBlock::parse(byte_order, cn_id, link_cdblock as u64, instance)?;
        }
        // save id to self, append self to parent and restore file stream position
        instance
            .get_mut_node_by_id::<CNBlock>(cn_id)
            .unwrap()
            .block_id = Some(cn_id);
        parent_id
            .checked_append(cn_id, &mut instance.arena)
            .unwrap();
        buf_reader.seek(SeekFrom::Start(pos)).unwrap();
        Ok(result)
    }
}

/// trait alias for specific `CNObject` trait
trait CNObjectV3<'a>: CNObject<'a, CCBlock, CDBlock, CEBlock, CGBlock> {}

impl<'a> CNObjectV3<'a> for CNBlock {}

impl<'a> CNObject<'a, CCBlock, CDBlock, CEBlock, CGBlock> for CNBlock {
    fn read_position(&self, mdf_file: &MDFFile, record_index: i64) -> i64 {
        let id = self.block_id.clone();
        let cg_block = self.cgblock(&mdf_file).unwrap();
        let mut size = record_index * (cg_block.record_size as i64);
        id.map_or(-1, |node_id| {
            let dg_block = mdf_file
                .get_ancestor_node_by_id::<DGBlock>(node_id)
                .unwrap();
            let record_type = dg_block.record_id_type;
            match record_type {
                Some(RecordIDType::Before8Bit) | Some(RecordIDType::BeforeAndAfter8Bit) => {
                    size += 1;
                }
                _ => {}
            }
            size
        })
    }

    fn ccblock(&self, mdf_file: &'a MDFFile) -> Option<&'a CCBlock> {
        let id = self.block_id.clone();
        id.map_or(None, |node_id| {
            mdf_file.get_child_node_by_id::<CCBlock>(node_id)
        })
    }

    fn cdblock(&self, mdf_file: &'a MDFFile) -> Option<&'a CDBlock> {
        let id = self.block_id.clone();
        id.map_or(None, |node_id| {
            mdf_file.get_child_node_by_id::<CDBlock>(node_id)
        })
    }

    fn ceblock(&self, mdf_file: &'a MDFFile) -> Option<&'a CEBlock> {
        let id = self.block_id.clone();
        id.map_or(None, |node_id| {
            mdf_file.get_child_node_by_id::<CEBlock>(node_id)
        })
    }

    fn cgblock(&self, mdf_file: &'a MDFFile) -> Option<&'a CGBlock> {
        let id = self.block_id.clone();
        id.map_or(None, |node_id| {
            mdf_file.get_ancestor_node_by_id::<CGBlock>(node_id)
        })
    }

    fn max(&self, mdf_file: &'a MDFFile) -> f64 {
        self.ccblock(mdf_file).map_or(f64::NAN, |x| x.max)
    }

    fn max_ex(&self, mdf_file: &'a MDFFile) -> f64 {
        self.max(mdf_file)
    }

    fn min(&self, mdf_file: &'a MDFFile) -> f64 {
        self.ccblock(mdf_file).map_or(f64::NAN, |x| x.min)
    }

    fn min_ex(&self, mdf_file: &'a MDFFile) -> f64 {
        self.min(mdf_file)
    }

    fn unit(&self, mdf_file: &'a MDFFile) -> String {
        self.ccblock(mdf_file)
            .map_or(Default::default(), |x| x.unit.clone())
    }
}

#[normal_object_v3]
#[derive(Debug, Clone, PermanentBlock)]
pub struct SRBlock {
    /// Length of time interval(/s)
    pub time_interval_len: f64,
    /// Number of reduced samples
    pub reduced_samples_number: u64,
    pub(crate) link_data_block: u32,
    pub(crate) link_next_sr: u32,
}

impl MDFObject for SRBlock {
    fn block_size(&self) -> u64 {
        self.block_size
    }

    fn name(&self) -> String {
        self.id.to_string()
    }
}

impl SRBlock {
    pub fn new() -> SRBlock {
        SRBlock {
            time_interval_len: 0.0,
            reduced_samples_number: 0,
            id: "SR".to_string(),
            block_size: 24,
            link_data_block: 0,
            link_next_sr: 0,
        }
    }

    pub(crate) fn parse(
        _byte_order: ByteOrder,
        parent_id: BlockId,
        link_id: u64,
        instance: &mut MDFFile,
    ) -> Result<u64, MDFErrorKind> {
        let mut buf_reader = instance.get_buf_reader()?;
        let pos = buf_reader.stream_position().unwrap();
        buf_reader.seek(SeekFrom::Start(link_id)).unwrap();
        let mut buf = [0; 24];
        buf_reader.read_exact(&mut buf).unwrap();
        let sr_block = sr_block_basic(&buf).unwrap().1;
        let result = sr_block.link_next_sr as u64;
        let sr_id = instance.arena.new_node(Box::new(sr_block));
        parent_id
            .checked_append(sr_id, &mut instance.arena)
            .unwrap();
        buf_reader.seek(SeekFrom::Start(pos)).unwrap();
        Ok(result)
    }
}

impl SRObject for SRBlock {}

#[normal_object_v3]
#[comment_object]
#[derive(Debug, Clone, PermanentBlock)]
pub struct CCBlock {
    pub conversion_type: Option<ConversionType>,
    pub date: Option<DateType>,
    pub default_text: String,
    pub foumula: String,
    pub inv_ccblock: Option<BlockId>,
    pub tab_pairs: Option<Vec<(f64, f64)>>,
    pub text_table_pairs: Option<Vec<(f64, String)>>,
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
            unit: unit,
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
        let mut buf = [0; 46];
        buf_reader.read_exact(&mut buf).unwrap();
        let mut cc_block = cc_block_basic(&buf, byte_order).unwrap().1;
        let pos_pos = buf_reader.stream_position().unwrap();
        cc_block.parse_params(pos_pos, instance)?;
        let cc_id = instance.arena.new_node(Box::new(cc_block));
        parent_id
            .checked_append(cc_id, &mut instance.arena)
            .unwrap();
        buf_reader.seek(SeekFrom::Start(pos)).unwrap();
        Ok(cc_id)
    }

    fn parse_params(&mut self, position: u64, instance: &mut MDFFile) -> Result<(), MDFErrorKind> {
        let mut buf_reader = instance.get_buf_reader().unwrap();
        buf_reader.seek(SeekFrom::Start(position)).unwrap();
        match self.conversion_type {
            Some(conv_type) => {
                match conv_type {
                    ConversionType::ParametricLinear
                    | ConversionType::Polynomial
                    | ConversionType::Exponential
                    | ConversionType::Logarithmic
                    | ConversionType::Rational => {
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
                        for _ in 0..self.tab_size {
                            let mut buf = [0; 16];
                            buf_reader.read_exact(&mut buf).unwrap();
                            let key_var = read_n_le_f64(&buf, 2).unwrap().1;
                            tab_pairs.push((key_var[0], key_var[1]));
                        }
                        self.tab_pairs = Some(tab_pairs);
                        // println!("tab_pairs:{:?}",self.tab_pairs);
                    }
                    ConversionType::TextFormula => {
                        let mut buf = vec![0; self.tab_size as usize];
                        buf_reader.read_exact(&mut buf).unwrap();
                        self.foumula = read_str(&buf, self.tab_size as u32).unwrap().1;
                    }
                    ConversionType::TextTable => {
                        let mut text_table_pairs = Vec::new();
                        for _ in 0..self.tab_size {
                            let mut buf = [0; 8];
                            buf_reader.read_exact(&mut buf).unwrap();
                            let key = read_n_le_f64(&buf, 1).unwrap().1;
                            let mut buf = [0; 32];
                            buf_reader.read_exact(&mut buf).unwrap();
                            let value = read_str(&buf, 32).unwrap().1;
                            text_table_pairs.push((key[0], value));
                        }
                        self.text_table_pairs = Some(text_table_pairs);
                    }
                    ConversionType::TextRange => {
                        let mut text_range_pairs = HashMap::new();

                        let mut buf = [0; 16];
                        buf_reader.read_exact(&mut buf).unwrap();
                        let mut buf = [0; 4];
                        buf_reader.read_exact(&mut buf).unwrap();
                        let link_default_text = read_le_u32(&buf).unwrap().1;
                        let default_text_block = TXBlock::parse(instance, link_default_text as u64);
                        self.comment = default_text_block
                            .as_ref()
                            .map_or(Default::default(), |x| x.text.clone());

                        for _ in 0..(self.tab_size - 1) {
                            let mut buf = vec![0; 2 * 8];
                            buf_reader.read_exact(&mut buf).unwrap();
                            let min_max = read_n_le_f64(&buf, 2).unwrap().1;
                            let range = Range {
                                start: min_max[0],
                                end: min_max[2],
                            };
                            let mut buf = [0; 4];
                            buf_reader.read_exact(&mut buf).unwrap();
                            let link_text = read_le_u32(&buf).unwrap().1;

                            let text_block = TXBlock::parse(instance, link_text as u64);
                            let text = text_block
                                .as_ref()
                                .map_or(Default::default(), |x| x.text.clone());

                            text_range_pairs.insert(text, range);
                        }
                        self.text_range_pairs = Some(text_range_pairs);
                    }
                    ConversionType::Date => {
                        let mut buf = [0; 7];
                        buf_reader.read_exact(&mut buf).unwrap();
                        self.date = Some(parse_datetype(&buf).unwrap().1);
                    }
                    ConversionType::Time => {
                        let mut buf = [0; 5];
                        buf_reader.read_exact(&mut buf).unwrap();
                        self.time = Some(parse_timetype(&buf).unwrap().1);
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

/// trait alias for specific `CCObject` trait
trait CCObjectV3<'a>: CCObject<'a, CCBlock> {}

impl<'a> CCObjectV3<'a> for CCBlock {}

impl<'a> CCObject<'a, CCBlock> for CCBlock {
    fn to_physical(&self, value: f64, inversed: bool) -> f64 {
        let mut value = value;
        let conv_type = self.conversion_type;
        match conv_type {
            Some(ConversionType::ParametricLinear) => {
                if self.params.is_some()
                    && !itertools::equal(self.params.as_ref().unwrap(), &PARAM_LINEAR_ARRAY_I)
                {
                    value = self.params.as_ref().unwrap()[1] * (value)
                        + self.params.as_ref().unwrap()[0];
                }
            }
            Some(ConversionType::TabInt) => {
                let mut value_val = value;
                if self.tab_pairs.is_some() && self.tab_pairs.as_ref().unwrap().len() > 0 {
                    let tab_pairs = self.tab_pairs.as_ref().unwrap();
                    let len = tab_pairs.len();
                    let mut value_key;

                    (value_key, value_val) = tab_pairs[0];

                    for i in 1..len {
                        if value < value_key {
                            return value_val;
                        }
                        let (key, val) = tab_pairs[i];
                        if value < key {
                            return (value - value_key) / (key - value_key) * (val - value_key)
                                + value_val;
                        }
                        value_key = key;
                        value_val = val;
                    }
                }
                value = value_val;
            }
            Some(ConversionType::Tab) => {
                if self.tab_pairs.is_some() {
                    let tab_pairs = self.tab_pairs.as_ref().unwrap();
                    tab_pairs
                        .iter()
                        .find(|(key, _)| *key == value)
                        .map(|(_, val)| {
                            value = *val;
                        });
                }
            }
            Some(ConversionType::TextFormula)
            | Some(ConversionType::TextTable)
            | Some(ConversionType::TextRange) => {}
            Some(ConversionType::Polynomial) => {
                if self.params.is_some() {
                    let params = self.params.as_ref().unwrap();
                    let v1 = value - params[4] - params[5];
                    let v2 = params[2] * v1 - params[0];
                    value = (params[1] - params[3] * v1) / v2;
                }
            }
            Some(ConversionType::Exponential) => {
                if self.params.is_some() {
                    let params = self.params.as_ref().unwrap();
                    if params[0] == 0.0 {
                        value = ((params[2] / (value - params[6]) - params[5]) / params[3]).exp()
                            / params[4]
                    } else if params[3] == 0.0 {
                        value = (((value - params[6]) * params[5] - params[2]) / params[0]).exp()
                            / params[1];
                    }
                }
            }
            Some(ConversionType::Logarithmic) => {
                if self.params.is_some() {
                    let params = self.params.as_ref().unwrap();
                    if params[0] == 0.0 {
                        value = ((params[2] / (value - params[6]) - params[5]) / params[3]).exp()
                            / params[4]
                    } else if params[4] == 0.0 {
                        value = (((value - params[6]) * params[5] - params[2]) / params[0]).exp()
                            / params[1];
                    }
                }
            }
            Some(ConversionType::Rational) => {
                if self.params.is_some()
                    && !itertools::equal(self.params.as_ref().unwrap(), &RATIONAL_PARAM_ARRAY_I)
                {
                    let params = self.params.as_ref().unwrap().clone();
                    let params = if inversed {
                        transform_params(params, self.conversion_type)
                    } else {
                        params
                    };

                    let v1 = params[3] * value - params[0];
                    let v2 = params[4] * value - params[1];
                    let v3 = params[5] * value - params[2];

                    if v1 == 0.0 || v1.is_nan() || v1.is_infinite() {
                        value = -v3 / v2;
                    }
                }
            }
            _ => {}
        }

        value
    }

    fn inc_ccblock(&self, _mdf_file: &'a MDFFile) -> Option<Vec<&'a CCBlock>> {
        None
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

/// Some extension blocks
#[normal_object_v3]
#[derive(Debug, Clone, PermanentBlock)]
pub struct CEBlock {
    pub dim: Option<DimType>,
    pub extension_type: Option<ExtensionType>,
    pub vector_can: Option<VectorCANType>,
}

impl MDFObject for CEBlock {
    fn block_size(&self) -> u64 {
        self.block_size
    }

    fn name(&self) -> String {
        self.id.clone()
    }
}

impl CEBlock {
    pub fn new_with_dim(dim: DimType) -> CEBlock {
        CEBlock {
            dim: Some(dim),
            extension_type: Some(ExtensionType::DIM),
            vector_can: None,
            id: "CE".to_string(),
            block_size: 6,
        }
    }
    pub fn new_with_vector_can(vector_can: VectorCANType) -> CEBlock {
        CEBlock {
            dim: None,
            extension_type: Some(ExtensionType::VectorCAN),
            vector_can: Some(vector_can),
            id: "CE".to_string(),
            block_size: 6,
        }
    }
    pub fn new() -> CEBlock {
        CEBlock {
            dim: None,
            extension_type: None,
            vector_can: None,
            id: "CE".to_string(),
            block_size: 6,
        }
    }

    pub(crate) fn parse(
        _byte_order: ByteOrder,
        parent_id: BlockId,
        link_id: u64,
        instance: &mut MDFFile,
    ) -> Result<BlockId, MDFErrorKind> {
        let mut buf_reader = instance.get_buf_reader()?;
        let pos = buf_reader.stream_position().unwrap();
        buf_reader.seek(SeekFrom::Start(link_id)).unwrap();
        let mut buf = [0; 6];
        buf_reader.read_exact(&mut buf).unwrap();
        let mut ce_block = ce_block_basic(&buf).unwrap().1;

        match ce_block.extension_type {
            Some(ExtensionType::DIM) => {
                let mut buf = [0; 118];
                buf_reader.read_exact(&mut buf).unwrap();
                ce_block.dim = Some(parse_dim_type(&buf).unwrap().1);
            }
            Some(ExtensionType::VectorCAN) => {
                let mut buf = [0; 80];
                buf_reader.read_exact(&mut buf).unwrap();
                ce_block.vector_can = Some(vector_can_type(&buf).unwrap().1);
            }
            None => {}
        }

        let ce_id = instance.arena.new_node(Box::new(ce_block));
        parent_id
            .checked_append(ce_id, &mut instance.arena)
            .unwrap();
        buf_reader.seek(SeekFrom::Start(pos)).unwrap();

        Ok(ce_id)
    }
}

#[derive(Debug, Clone)]
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

/// Dependency block
#[normal_object_v3]
#[derive(Debug, Clone, PermanentBlock)]
pub struct CDBlock {
    pub dependency_type: u16,
    pub dependencies: Option<Vec<DependencyType>>,
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
            dependencies: None,
        }
    }

    pub(crate) fn parse(
        _byte_order: ByteOrder,
        parent_id: BlockId,
        link_id: u64,
        instance: &mut MDFFile,
    ) -> Result<BlockId, MDFErrorKind> {
        let mut buf_reader = instance.get_buf_reader()?;
        let pos = buf_reader.stream_position().unwrap();
        buf_reader.seek(SeekFrom::Start(link_id)).unwrap();
        let mut buf = [0; 8];
        buf_reader.read_exact(&mut buf).unwrap();
        let (dependency_number, cd_block) = cdblock_basic(&buf).unwrap().1;
        let mut dependencies = Vec::new();
        for _ in 0..dependency_number {
            let mut buf = [0; 12];
            buf_reader.read_exact(&mut buf).unwrap();
            dependencies.push(dependency_type(&buf).unwrap().1);
        }
        let cd_id = instance.arena.new_node(Box::new(cd_block));
        parent_id
            .checked_append(cd_id, &mut instance.arena)
            .unwrap();
        buf_reader.seek(SeekFrom::Start(pos)).unwrap();
        Ok(cd_id)
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
#[normal_object_v3]
#[comment_object]
#[derive(Debug, Clone, PermanentBlock)]
pub struct TRBlock {
    pub trigger_events: Vec<TriggerEvent>,
    pub(crate) link_comment: u32,
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
        _byte_order: ByteOrder,
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
        // save tr block to arena
        let tr_id = instance.arena.new_node(Box::new(tr_block));
        parent_id
            .checked_append(tr_id, &mut instance.arena)
            .unwrap();
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
///
/// For convenience when writing, currently, we only store TXBlock as a String, not as child of parent block.
#[normal_object_v3]
#[derive(Debug, Clone, PermanentBlock)]
pub struct TXBlock {
    /// a string with a eol(`\0`) char, block size include this eol char.
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
