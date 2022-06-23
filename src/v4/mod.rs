use asammdf_derive::{comment_object, id_object, mdf_object, normal_object};
use asammdf_derive::{IDObject, MDFObject, PermanentBlock};

use super::SpecVer;
use super::UnfinalizedFlagsType;
use crate::MDFObject;
use crate::PermanentBlock;
use crate::{IDObject, TimeFlagsType, TimeQualityType};

mod parser;

#[derive(Clone, Copy, Debug)]
/// Zip type of a compression block
pub enum Zip {
    Deflate,
    TransposeAndDeflate,
}

#[derive(Clone, Copy, Debug)]
pub enum SRFlags {
    InvalidationBytes = 1,
}

#[derive(Clone, Copy, Debug)]
pub enum Source {
    Other,
    ECU,
    Bus,
    IO,
    Tool,
    User,
}

#[derive(Clone, Copy, Debug)]
pub enum SourceFlags {
    SimulatedSource,
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
pub enum DataBlockFlags {
    EqualLength = 1,
}

#[derive(Clone, Copy, Debug)]
pub enum ConversionFlags {
    PrecisionValid = 1,
    LimitRangeValid = 2,
    StatusString = 4,
}

#[derive(Clone, Copy, Debug)]
pub enum ChannelGroupFlags {
    VariableLenSignalData = 1,
    BusEvent = 2,
    PlainBusEvent = 4,
}

#[derive(Clone, Copy, Debug)]
pub enum ChannelFlags {
    Invalid = 1,
    InvalBytesValid = 2,
    PrecisionValid = 4,
    ValueRangeValid = 8,
    LimitRangeValid = 16,
    ExtendedLimitRangeValid = 32,
    DiscreteValue = 64,
    Calibration = 128,
    Calculated = 256,
    Virtual = 512,
    BusEvent = 1024,
    Montonous = 2048,
    DefaultXAxis = 4096,
}

#[derive(Clone, Copy, Debug)]
pub enum Cause {
    Other,
    Error,
    Tool,
    Script,
    User,
}

#[derive(Clone, Copy, Debug)]
pub enum BusType {
    Other,
    CAN,
    LIN,
    MOST,
    FLEXRAY,
    KLINE,
    Ethernet,
    USB,
}

#[derive(Clone, Copy, Debug)]
pub enum AttachmentFlags {
    EmbeddedData = 1,
    CompressedEmbbeddedData = 2,
    MD5ChecksumValid = 4,
}

#[mdf_object]
#[id_object]
#[derive(Debug, MDFObject, IDObject, Clone, PermanentBlock)]
pub struct IDBlock {
    pub format_id: String,
    pub program_id: String,
}

impl IDBlock {
    /// Create a v4::IDBlock with version default to `410`, spec_type default to `SpecVer::V4`
    fn new(program_id: String) -> IDBlock {
        IDBlock {
            format_id: "4.10    ".to_string(),
            program_id,
            block_size: 64,
            name: "ID".to_string(),
            file_id: "MDF     ".to_string(),
            spec_type: SpecVer::V4,
            unfinalized_flags: None,
            version: 410,
            custom_flags: 0,
        }
    }

    /// parse a IDBlock from a 64byte u8 slice
    pub(crate) fn parse(input: &[u8]) -> Option<IDBlock> {
        parser::id_block(input).map_or(None, |x| Some(x.1))
    }
}

#[comment_object]
#[normal_object]
pub struct HDBlock {
    // TODO: store in arena,or inside this object?
    pub atb_locks: Vec<ATBlock>,
    pub dst_offset: i16,
    pub flags: TimeFlagsType,
    pub start_angle: f64,
    pub start_distance: f64,
    pub time_quality: TimeQualityType,
    pub utc_offset: i16,
}

#[comment_object]
#[normal_object]
pub struct ATBlock {
    pub attachment_flags: Option<AttachmentFlags>,
    pub creator_index: u16,
    pub filename: String,
    pub mime_type: String,
}

#[normal_object]
#[comment_object]
pub struct CHBlock {
    // chblocks?
    // dependency type?
    pub hierarchy_type: Hierarchy,
}
