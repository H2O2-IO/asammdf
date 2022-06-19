use asammdf_derive::{id_object, mdf_object};

use super::SpecVer;
use super::UnfinalizedFlagsType;
use crate::MDFObject;
use crate::PermanentBlock;
use crate::{v3::FloatPointFormat, ByteOrder};
mod parser;

/// Zip type of a compression block
pub enum Zip {
    Deflate,
    TransposeAndDeflate,
}

pub enum SRFlags {
    InvalidationBytes = 1,
}

pub enum Source {
    Other,
    ECU,
    Bus,
    IO,
    Tool,
    User,
}

pub enum SourceFlags {
    SimulatedSource,
}

pub enum RangeType {
    Point,
    BeginRange,
    EndRange,
}

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

pub enum Event {
    Recording,
    RecordingInterrupt,
    AcquisitionInterrupt,
    StartRecordingTrigger,
    StopRecordingTrigger,
    Trigger,
    Marker,
}

pub enum DataBlockFlags {
    EqualLength = 1,
}

pub enum ConversionFlags {
    PrecisionValid = 1,
    LimitRangeValid = 2,
    StatusString = 4,
}

pub enum ChannelGroupFlags {
    VariableLenSignalData = 1,
    BusEvent = 2,
    PlainBusEvent = 4,
}

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

pub enum Cause {
    Other,
    Error,
    Tool,
    Script,
    User,
}

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

pub enum AttachmentFlags {
    EmbeddedData = 1,
    CompressedEmbbeddedData = 2,
    MD5ChecksumValid = 4,
}

#[mdf_object]
#[id_object]
#[derive(Debug)]
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

impl PermanentBlock for IDBlock {}

impl MDFObject for IDBlock {
    fn block_size(&self) -> u64 {
        self.block_size
    }

    fn name(&self) -> String {
        self.name.clone()
    }
}
