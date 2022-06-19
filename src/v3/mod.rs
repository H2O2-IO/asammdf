use asammdf_derive::{
    channel_group_object, channel_object, comment_object, data_group_object, header_object,
    id_object, mdf_object, normal_object,
};
use asammdf_derive::{IDObject, MDFObject, PermanentBlock};
use chrono::Local;

use super::SpecVer;
use super::UnfinalizedFlagsType;
use crate::IDObject;
use crate::MDFObject;
use crate::PermanentBlock;
use crate::{ByteOrder, ChannelType, RecordIDType, SignalType, SyncType, TimeQualityType};
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
pub struct HDBlock {
    pub author: String,
    pub date: String,
    pub organization: String,
    pub program_specific_data: String,
    pub project: String,
    pub subject: String,
    pub time: String,
    pub time_quality: Option<TimeQualityType>,
    pub timer_id: String,
    pub timestamp: i64,
    pub utc_offset: i16,

    link_first_file_group: u32,
    link_file_comment_txt: u32,
    link_program_block: u32,
}

impl HDBlock {
    fn new(date_time: DateTime<Local>, time_quality: TimeQualityType) -> HDBlock {
        HDBlock {
            author: Default::default(),
            date: date_time.format("%d:%m:%Y").to_string(),
            organization: Default::default(),
            program_specific_data: Default::default(),
            project: Default::default(),
            subject: Default::default(),
            time: date_time.format("%H:%M:%S").to_string(),
            time_quality: Some(time_quality),
            timer_id: "Local PC Reference Time".to_string(),
            timestamp: date_time.timestamp(),
            utc_offset: (Local::now().offset().local_minus_utc() as f64 / 3600.0f64).round() as i16,
            block_size: 208,
            id: "HD".to_string(),
            comment: Default::default(),
            link_file_comment_txt: 0,
            link_first_file_group: 0,
            link_program_block: 0,
        }
    }

    pub(crate) fn parse(byte_order: ByteOrder) {}
}

#[normal_object]
#[comment_object]
#[data_group_object]
pub struct DGBlock {
    link_data_records: u32,
    link_next_cgblock: u32,
    link_next_dgblock: u32,
    link_trblock: u32,
    pub tr_block: Option<TRBlock>,
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
}

#[normal_object]
#[channel_group_object]
#[comment_object]
pub struct CGBlock {
    pub record_id: u16,
    link_cg_comment: u32,
    link_first_cnblock: u32,
    link_first_srblock: u32,
    link_next_cgblock: u32,
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
}

#[normal_object]
#[channel_object]
#[comment_object]
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
            max: 0.0,
            max_ex: 0.0,
            max_raw: 0.0,
            min: 0.0,
            min_ex: 0.0,
            min_raw: 0.0,
            bits_count,
            signal_type: Some(signal_type),
            sync_type: None,
            unit: Default::default(),
            comment: Default::default(),
        }
    }
}

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
pub struct CDBlock {
    pub dependency_type: u16,
    pub name: String,
}

impl CDBlock {
    pub fn new() -> CDBlock {
        CDBlock {
            dependency_type: 0,
            name: "Dependency".to_string(),
            id: "CD".to_string(),
            block_size: 8,
        }
    }
}

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

#[normal_object]
#[comment_object]
pub struct TRBlock {
    pub trigger_events: Vec<TriggerEvent>,
    pub link_comment: u32,
}

impl TRBlock {
    pub fn new(trigger_events: Vec<TriggerEvent>) -> TRBlock {
        TRBlock {
            trigger_events: trigger_events,
            link_comment: 0,
            id: "TR".to_string(),
            block_size: 10,
            comment: Default::default(),
        }
    }
}

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
#[normal_object]
pub struct TXBlock {
    pub text: String,
}

impl TXBlock {
    pub fn new(text: String) -> TXBlock {
        TXBlock {
            text: text.clone(),
            id: "TX".to_string(),
            block_size: 4 + (text.len() as u64) + 1,
        }
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
