use std::{
    any::Any,
    collections::HashMap,
    fmt::Display,
    fs::File,
    io::{self, BufReader, Read, Seek, SeekFrom},
};

use chrono::{DateTime, Utc};
use indextree::{Arena, NodeId};
use misc::helper::{read_le_f32, read_le_f64, read_le_u16, read_le_u32, read_le_u64};
pub mod misc;
pub mod v3;
pub mod v4;
pub type BlockId = NodeId;

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

/// A struct store a annotation and corresponding timestamp
pub struct Annotation {
    pub timestamp: f64,
    pub text: String,
}

impl Annotation {
    fn new(timestamp: f64, text: String) -> Annotation {
        Annotation { timestamp, text }
    }
}

/// type alias for `Vec<Annotation>`
type AnnotationList = Vec<Annotation>;

enum_u32_convert! {
    #[derive(Clone, Copy, Debug, PartialEq)]
    /// Type of a channel
    pub enum ChannelType {
        /// Fixed length data channel. Channal value is contained in record itself.
        Data,
        /// **Variable length signal data** channel
        VariableData,
        ///
        Master,
        VirtualMaster,
        Sync,
        /// Contained since MDF 4.1.0.
        MaxLenData,
        /// Contained since MDF 4.1.0.
        VirtualData,
    }
}
enum_u32_convert! {
    #[derive(Clone, Copy, Debug)]
    pub enum ConversionType {
        ParametricLinear = 0,
        TabInt,
        Tab,
        Polynomial = 6,
        Exponential,
        Logarithmic,
        Rational,
        TextFormula,
        TextTable,
        TextRange,
        Date = 132,
        Time,
        TabRange,
        TextToValue,
        TextToText,
    }
}
/// File type of supported mdf file.
#[derive(Clone, Copy, Debug)]
pub enum FileType {
    Dat,
    Mdf,
    Mf3,
    Mf4,
}

/// Support version of MDF specification
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpecVer {
    /// support to v3.30
    V3,
    /// support to v4.10
    V4,
}

/// Value format to display data
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ValueFormat {
    /// Physical values
    Physical,
    /// Raw values (decimal)
    Raw,
    /// Raw values (hexadecimal)
    RawHex,
    /// Raw values (binary)
    RawBin,
}

enum_u32_convert! {
#[derive(Clone, Copy, Debug)]
pub enum RecordIDType {
    Before8Bit = 1,
    Before16Bit = 2,
    Before32Bit = 4,
    Before64Bit = 8,
    BeforeAndAfter8Bit = 255,
}
}

#[derive(Clone, Copy, Debug)]
pub enum SignalType {
    UIntLE,
    UIntBE,
    SIntLE,
    SIntBE,
    FloatLE,
    FloatBE,
    String,
    StringUTF8,
    StringUTF16LE,
    StringUTF16BE,
    ByteArray,
    MIMESample,
    MIMEStream,
    CANOPENData,
    CANOPENTime,
}

impl SignalType {
    pub fn from_u16(value: u16, byte_order: ByteOrder) -> Option<SignalType> {
        match value {
            0 => {
                if byte_order != ByteOrder::BigEndian {
                    Some(SignalType::UIntLE)
                } else {
                    Some(SignalType::UIntBE)
                }
            }
            1 => {
                if byte_order != ByteOrder::BigEndian {
                    Some(SignalType::SIntLE)
                } else {
                    Some(SignalType::SIntBE)
                }
            }
            2 | 3 => {
                if byte_order != ByteOrder::BigEndian {
                    Some(SignalType::FloatLE)
                } else {
                    Some(SignalType::FloatBE)
                }
            }
            7 => Some(SignalType::String),
            8 => Some(SignalType::ByteArray),
            9 => Some(SignalType::UIntBE),
            10 => Some(SignalType::SIntBE),
            11 | 12 => Some(SignalType::FloatBE),
            13 => Some(SignalType::UIntLE),
            14 => Some(SignalType::SIntLE),
            15 | 16 => Some(SignalType::FloatLE),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum SyncType {
    Time,
    Angle,
    Distance,
    Index,
}

enum_u32_convert! {
#[derive(Clone, Copy, Debug)]
pub enum TimeFlagsType {
    LocalTime = 1,
    OffsetsValid,
}
}
enum_u32_convert! {
#[derive(Clone, Copy, Debug)]
pub enum TimeQualityType {
    LocalPC,
    ExternalSource = 10,
    ExternalAbsolute = 16,
}
}
enum_u32_convert! {
#[derive(Clone, Copy, Debug)]
pub enum UnfinalizedFlagsType {
    UpdateCGBlockRequired = 1,
    UpdateSRBlockRequired,
}
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ByteOrder {
    BigEndian = 1,
    LittleEndian = 2,
}

pub struct RecordRawData {
    pub timestamp: f64,
    pub data: Option<Vec<u8>>,
}

pub trait MDFObject {
    fn block_size(&self) -> u64;
    fn name(&self) -> String;
    // reference tag?
}

/// write self to a file
pub trait Writer {}

pub trait IDObject: MDFObject {
    fn file_id(&self) -> String;
    fn spec_type(&self) -> SpecVer;
    fn unfinalized_flags_type(&self) -> Option<UnfinalizedFlagsType>;
    fn version(&self) -> u16;
    fn custom_flags(&self) -> u16;
}

/// Gerneric header block
pub trait HDObject<'a, DG> {
    /// get data group list
    fn data_groups(&self, mdf_file: &'a MDFFile) -> Option<Vec<&'a DG>>;
    /// recording time
    fn recording_time(&self) -> Option<DateTime<Utc>>;
}

pub trait CCObject<'a, CC> {
    fn to_physical(&self, value: f64, inversed: bool) -> f64;
    fn inc_ccblock(&self, mdf_file: &'a MDFFile) -> Option<Vec<&'a CC>>;
}

#[derive(Debug, Clone)]
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

pub trait DGObject {}

pub trait CGObject<'a, CN, DG, SR> {
    fn cnblocks(&self, mdf_file: &'a MDFFile) -> Option<Vec<&'a CN>>;
    fn dgblock(&self, mdf_file: &'a MDFFile) -> Option<&'a DG>;
    fn srblocks(&self, mdf_file: &'a MDFFile) -> Option<Vec<&'a SR>>;

    fn get_record_size(&self, mdf_file: &'a MDFFile) -> i64;
}

pub trait CNObject<'a, CC, CD, CE, CG> {
    fn read_position(&self, mdf_file: &MDFFile, record_index: i64) -> i64;
    fn ccblock(&self, mdf_file: &'a MDFFile) -> Option<&'a CC>;
    fn cdblock(&self, mdf_file: &'a MDFFile) -> Option<&'a CD>;
    fn ceblock(&self, mdf_file: &'a MDFFile) -> Option<&'a CE>;
    fn cgblock(&self, mdf_file: &'a MDFFile) -> Option<&'a CG>;
    fn max(&self, mdf_file: &'a MDFFile) -> f64;
    fn max_ex(&self, mdf_file: &'a MDFFile) -> f64;
    fn min(&self, mdf_file: &'a MDFFile) -> f64;
    fn min_ex(&self, mdf_file: &'a MDFFile) -> f64;
    fn unit(&self, mdf_file: &'a MDFFile) -> String;
}

pub trait SRObject {}

/// Block that can be stored inside internal arena,
/// When write to MDF file, record block or other data block will be buffered (in vector), and directly write to file.
pub trait PermanentBlock: MDFObject {}

#[derive(Debug)]
pub enum MDFErrorKind {
    IOError(io::Error),
    IDBlockError(String),
    CCBlockError(String),
    CEBlockError(String),
    VersionError(String),
}

/// MDFFile
///
/// Example:
///
/// ```
/// use asammdf::{MDFFile,SpecVer};
/// let mut file = MDFFile::new();
/// file.open("./mdf3.dat".to_string()).unwrap();
/// assert_eq!(file.spec_ver,Some(SpecVer::V3));
/// ```
///
/// TODO: use memory map file to parse file for better performance
#[derive(Debug)]
pub struct MDFFile {
    /// Note that only description node are stored inside arena
    arena: Arena<Box<dyn Any>>,
    /// BlockId of this file's IDBlock
    id: Option<BlockId>,
    /// BlockId of this file's HDBlock
    header: Option<BlockId>,
    /// Source file path
    pub source_file: String,
    /// file handler to the source file
    file_handler: Option<File>,
    /// specification version read from ID block
    pub spec_ver: Option<SpecVer>,
    /// a cache for link between id and blockid
    link_id_blocks: HashMap<u64, BlockId>,
}

impl MDFFile {
    /// create a new MDFFile
    pub fn new() -> MDFFile {
        MDFFile {
            arena: Arena::new(),
            id: None,
            header: None,
            source_file: Default::default(),
            file_handler: Default::default(),
            spec_ver: Default::default(),
            link_id_blocks: HashMap::new(),
        }
    }

    /// when use buffer reader, make sure to use SeekFrom::Start
    pub(crate) fn get_buf_reader(&mut self) -> Result<BufReader<File>, MDFErrorKind> {
        let x = self
            .file_handler
            .as_mut()
            .unwrap()
            .try_clone()
            .map_err(|x| MDFErrorKind::IOError(x))?;
        Ok(BufReader::new(x))
    }
    /// get BufReader of internal file handler at specific position
    pub(crate) fn get_buf_reader_at_loc(
        &mut self,
        loc: u64,
    ) -> Result<BufReader<File>, MDFErrorKind> {
        let x = self
            .file_handler
            .as_mut()
            .unwrap()
            .try_clone()
            .map_err(|x| MDFErrorKind::IOError(x))?;
        let mut reader = BufReader::new(x);
        reader.seek(SeekFrom::Start(loc)).unwrap();
        Ok(reader)
    }

    /// open a file, and than parse PermanentBlocks
    pub fn open(&mut self, file_path: String) -> Result<(), MDFErrorKind> {
        let mut file = File::open(&file_path).map_err(|x| MDFErrorKind::IOError(x))?;
        self.file_handler = Some(file.try_clone().map_err(|x| MDFErrorKind::IOError(x))?);
        let mut idblock_buf = [0; 64];
        // block size of idblock v3&v4 is the same(64)
        file.read(&mut idblock_buf)
            .map_err(|x| MDFErrorKind::IOError(x))?;
        // try to parse this buffer as
        let idblock = v3::IDBlock::parse(&idblock_buf)
            .ok_or_else(|| MDFErrorKind::IDBlockError("Faild to parse id block(v3)".to_string()))?;

        if idblock.file_id() != "MDF     " {
            return Err(MDFErrorKind::IDBlockError(
                "File id is not 'MDF     '".to_string(),
            ));
        }
        // if version greater than 400, should parse it as v4::IDBlock;
        if idblock.version() >= 400 {
            let idblock = v4::IDBlock::parse(&idblock_buf).ok_or_else(|| {
                MDFErrorKind::IDBlockError("Faild to parse id block(v4)".to_string())
            })?;
            // store ID block in arena, and set specification version to version 4
            self.id = Some(self.arena.new_node(Box::new(idblock)));
            self.spec_ver = Some(SpecVer::V4);
        } else {
            // store ID block in arena，and set specification versino to version 3
            self.id = Some(self.arena.new_node(Box::new(idblock)));
            self.spec_ver = Some(SpecVer::V3);
        }
        self.source_file = file_path.clone();
        // init other blocks that should be stored in arena
        self.init().unwrap();
        Ok(())
    }

    fn init(&mut self) -> Result<(), MDFErrorKind> {
        if self.is_v3() {
            // read byte order out of idblock
            let byte_order = self.arena[*self.id.as_ref().unwrap()]
                .get()
                .downcast_ref::<v3::IDBlock>()
                .unwrap()
                .byte_order
                .map_or(ByteOrder::LittleEndian, |x| x);
            // v3::hdblock
            v3::HDBlock::parse(byte_order, self)?;
        } else if self.is_v4() {
        } else {
            return Err(MDFErrorKind::VersionError(
                "MDF Specification Verion unsupported!".into(),
            ));
        }
        Ok(())
    }

    pub fn get_node<T: 'static + PermanentBlock>(&self) -> Option<&T> {
        self.id.and_then(|id| {
            id.descendants(&self.arena)
                .find(|x| self.arena[*x].get().downcast_ref::<T>().is_some())
                .map_or(None, |z| self.arena[z].get().downcast_ref::<T>())
        })
    }

    /// TODO: delete this method, `BlockId` should not be exposed to outside.
    pub fn get_node_id<T: 'static + PermanentBlock>(&self) -> Option<BlockId> {
        self.id.and_then(|id| {
            id.descendants(&self.arena)
                .find(|x| self.arena[*x].get().downcast_ref::<T>().is_some())
        })
    }

    pub fn get_all_nodes<T: 'static + PermanentBlock>(&self) -> Option<Vec<&T>> {
        self.id.and_then(|id| {
            Some(
                id.descendants(&self.arena)
                    .filter(|x| self.arena[*x].get().downcast_ref::<T>().is_some())
                    .map(|y| self.arena[y].get().downcast_ref::<T>().unwrap())
                    .collect(),
            )
        })
    }

    pub fn get_nodes_by_parent<T: 'static + PermanentBlock>(&self, id: BlockId) -> Option<Vec<&T>> {
        Some(
            id.children(&self.arena)
                .filter(|x| self.arena[*x].get().downcast_ref::<T>().is_some())
                .map(|y| self.arena[y].get().downcast_ref::<T>().unwrap())
                .collect(),
        )
    }

    pub fn get_mut_node_by_id<T: 'static + PermanentBlock>(
        &mut self,
        id: BlockId,
    ) -> Option<&mut T> {
        self.arena[id].get_mut().downcast_mut::<T>()
    }

    pub fn get_node_by_id<T: 'static + PermanentBlock>(&self, id: BlockId) -> Option<&T> {
        self.arena[id].get().downcast_ref::<T>()
    }

    /// TODO: delete this method, `BlockId` should not be exposed to outside.
    pub fn get_node_ids<T: 'static + PermanentBlock>(&mut self) -> Option<Vec<BlockId>> {
        self.id.and_then(|id| {
            Some(
                id.descendants(&self.arena)
                    .filter(|x| self.arena[*x].get().downcast_ref::<T>().is_some())
                    .collect(),
            )
        })
    }

    pub fn get_node_mut_by_name<T: 'static + PermanentBlock>(
        &mut self,
        name: &str,
    ) -> Option<&mut T> {
        self.id.and_then(|id| {
            id.descendants(&self.arena)
                .find(|x| {
                    self.arena[*x]
                        .get()
                        .downcast_ref::<T>()
                        .map_or(None, |y| if y.name() == name { Some(1) } else { None })
                        .is_some()
                })
                .map_or(None, |z| self.arena[z].get_mut().downcast_mut::<T>())
        })
    }

    /// TODO: delete this method, `BlockId` should not be exposed to outside.
    fn remove_node(&mut self, id: BlockId, recursive: bool) {
        if recursive {
            id.remove_subtree(&mut self.arena);
        } else {
            id.remove(&mut self.arena);
        }
    }

    /// TODO: delete this method, `BlockId` should not be exposed to outside.
    fn append_node(&mut self, parent_id: BlockId, child_id: BlockId) {
        parent_id.checked_append(child_id, &mut self.arena).unwrap();
    }

    pub fn get_channels(&self) -> Vec<BlockId> {
        todo!()
    }

    pub fn get_channel_groups(&self) -> Vec<BlockId> {
        todo!()
    }

    pub fn get_master_cnblock(&mut self, cgblock: BlockId) {}

    pub fn get_child_node_by_id<T: 'static + PermanentBlock>(&self, id: BlockId) -> Option<&T> {
        id.children(&self.arena)
            .find(|x| self.arena[*x].get().downcast_ref::<T>().is_some())
            .map_or(None, |z| self.arena[z].get().downcast_ref::<T>())
    }

    pub fn get_child_node_id_by_id<T: 'static + PermanentBlock>(
        &self,
        id: BlockId,
    ) -> Option<BlockId> {
        id.children(&self.arena)
            .find(|x| self.arena[*x].get().downcast_ref::<T>().is_some())
    }

    pub fn get_ancestor_node_by_id<T: 'static + PermanentBlock>(&self, id: BlockId) -> Option<&T> {
        id.ancestors(&self.arena)
            .find(|x| self.arena[*x].get().downcast_ref::<T>().is_some())
            .map_or(None, |z| self.arena[z].get().downcast_ref::<T>())
    }

    pub fn get_ancestor_node_id_by_id<T: 'static + PermanentBlock>(
        &self,
        id: BlockId,
    ) -> Option<BlockId> {
        id.ancestors(&self.arena)
            .find(|x| self.arena[*x].get().downcast_ref::<T>().is_some())
    }

    pub fn get_data_cnblock(
        &mut self,
        format: ValueFormat,
        cn_block: BlockId,
        record_index: i64,
    ) -> f64 {
        let id = cn_block;
        if self.is_v3() {
            let cn_block = self.get_node_by_id::<v3::CNBlock>(cn_block).unwrap();
            if cn_block.cgblock(&self).unwrap().record_size == 0 {
                f64::NAN
            } else {
                let record_size = cn_block.cgblock(self).unwrap().get_record_size(self);
                let read_position = cn_block.read_position(self, record_index);
                let data_record = cn_block
                    .cgblock(self)
                    .unwrap()
                    .dgblock(self)
                    .unwrap()
                    .link_data_records;
                let data =
                    self.read_data(data_record as u64, read_position as u64, record_size as u32);
                if data.len() == 0 {
                    f64::NAN
                } else {
                    self.get_format_value_by_array(format, id, data, 0, false)
                }
            }
        } else {
            f64::NAN
        }
    }

    /// get data from raw bytes of an record
    fn get_format_value_by_array(
        &mut self,
        format: ValueFormat,
        cn_block: BlockId,
        data: Vec<u8>,
        bit_no: i32,
        inversed: bool,
    ) -> f64 {
        let mut result = f64::NAN;
        if self.is_v3() {
            let cn_block = self.get_node_by_id::<v3::CNBlock>(cn_block).unwrap();
            let change_endianess = should_change_endianess(
                if cn_block.channel_type.unwrap() == ChannelType::VirtualData {
                    SignalType::UIntLE
                } else {
                    cn_block.signal_type.unwrap()
                },
            );

            let mut offset = (cn_block.add_offset + cn_block.bit_offset as u32 / 8) as i32;
            offset += bit_no;

            let bit_offset_remain = (cn_block.bit_offset % 8) as i32;
            // cal bit mask
            let mut bit_mask = u64::MAX;
            if bit_offset_remain > 0 || cn_block.bits_count % 8 > 0 {
                bit_mask = 1;
                let mut i = 1;
                while i < cn_block.bits_count {
                    bit_mask <<= 1;
                    bit_mask |= 1;
                    i += 1;
                }
            }

            if cn_block.bits_count <= 8 {
                // byte length
                let mut byte = data[offset as usize];
                byte = byte >> bit_offset_remain;
                byte &= bit_mask as u8;
                let signal_type = cn_block.signal_type;
                match signal_type {
                    Some(SignalType::SIntBE) | Some(SignalType::SIntLE) => {
                        result = (byte as i8) as f64;
                    }
                    Some(SignalType::UIntBE) | Some(SignalType::UIntLE) => {
                        result = byte as f64;
                    }
                    _ => {}
                }
            } else if cn_block.bits_count <= 16 {
                // 2 byte length
                let offset = offset as usize;
                let mut val = read_le_u16(&data[offset..(offset + 2)]).unwrap().1;
                if change_endianess {
                    val = val.to_be();
                }
                val = val >> bit_offset_remain;
                val &= bit_mask as u16;
                let signal_type = cn_block.signal_type;
                match signal_type {
                    Some(SignalType::SIntBE) | Some(SignalType::SIntLE) => {
                        result = (val as i16) as f64;
                    }
                    Some(SignalType::UIntBE) | Some(SignalType::UIntLE) => {
                        result = val as f64;
                    }
                    _ => {}
                }
            } else if cn_block.bits_count <= 32 {
                // 4 byte length
                let offset = offset as usize;
                let mut val = read_le_u32(&data[offset..(offset + 4)]).unwrap().1;
                if change_endianess {
                    val = val.to_be();
                }
                val = val >> bit_offset_remain;
                val &= bit_mask as u32;
                let signal_type = cn_block.signal_type;
                match signal_type {
                    Some(SignalType::SIntBE) | Some(SignalType::SIntLE) => {
                        result = (val as i32) as f64;
                    }
                    Some(SignalType::UIntBE) | Some(SignalType::UIntLE) => {
                        result = val as f64;
                    }
                    Some(SignalType::FloatBE) | Some(SignalType::FloatLE) => {
                        result = read_le_f32(&val.to_le_bytes()).unwrap().1 as f64;
                    }
                    _ => {}
                }
            } else {
                // 8 byte length
                let offset = offset as usize;
                let mut val = read_le_u64(&data[offset..(offset + 8)]).unwrap().1;
                if change_endianess {
                    val = val.to_be();
                }
                val = val >> bit_offset_remain;
                val &= bit_mask as u64;
                let signal_type = cn_block.signal_type;
                match signal_type {
                    Some(SignalType::SIntBE) | Some(SignalType::SIntLE) => {
                        result = (val as i64) as f64;
                    }
                    Some(SignalType::UIntBE) | Some(SignalType::UIntLE) => {
                        result = val as f64;
                    }
                    Some(SignalType::FloatBE) | Some(SignalType::FloatLE) => {
                        result = read_le_f64(&val.to_le_bytes()).unwrap().1 as f64;
                    }
                    _ => {}
                }
            }
        }
        // convert result to physical type
        if format == ValueFormat::Physical {
            result = self.get_phys_value_by_f64(cn_block, result, inversed);
        }
        result
    }

    /// get physical value of a data
    fn get_phys_value_by_f64(&mut self, cn_block: BlockId, data: f64, inversed: bool) -> f64 {
        let mut result = data;
        if self.is_v3() {
            let cn_block = self.get_node_by_id::<v3::CNBlock>(cn_block).unwrap();
            result = cn_block
                .ccblock(self)
                .map_or(data, |cc| cc.to_physical(result, inversed));
        }
        result
    }

    /// is MDFV3
    pub fn is_v3(&mut self) -> bool {
        self.spec_ver.is_some() && *self.spec_ver.as_ref().unwrap() == SpecVer::V3
    }

    /// is MDFV4
    pub fn is_v4(&mut self) -> bool {
        self.spec_ver.is_some() && *self.spec_ver.as_ref().unwrap() == SpecVer::V4
    }

    /// read data from internal file handler with provided start, offset and length
    fn read_data(&mut self, start: u64, offset: u64, length: u32) -> Vec<u8> {
        let loc = start + offset;
        let mut result = vec![0; length as usize];
        let mut buf_reader = self.get_buf_reader_at_loc(loc).unwrap();
        buf_reader.read_exact(&mut result).unwrap();
        result
    }
}

pub struct DataSample {
    pub x: f64,
    pub y: f64,
}

impl DataSample {
    pub fn new(x: f64, y: f64) -> DataSample {
        DataSample { x, y }
    }
}

impl Display for DataSample {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}:{}]", self.x, self.y)
    }
}

fn should_change_endianess(signal_type: SignalType) -> bool {
    match signal_type {
        // default is little endian
        SignalType::UIntLE
        | SignalType::SIntLE
        | SignalType::FloatLE
        | SignalType::StringUTF16LE => {
            return false;
        }
        // big endian should invert
        SignalType::UIntBE
        | SignalType::SIntBE
        | SignalType::FloatBE
        | SignalType::StringUTF16BE => {
            return true;
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::OpenOptions, io::Write};

    use super::*;
    #[test]
    fn mdf3_file_init() {
        let mut file = MDFFile::new();
        file.open("./mdf3.dat".to_string()).unwrap();

        // get out the id block
        assert!(file.id.is_some());
        let id = file.id.clone().unwrap();
        // test file is mdf3 block
        // assert!(file.arena[id].type_id() == TypeId::of::<v4::IDBlock>());

        let id_ref = file.get_node::<v3::IDBlock>().unwrap();
        assert_eq!(id_ref.name, "ID");
        // downcast id block
        let x = file.arena[id].get().downcast_ref::<v3::IDBlock>().unwrap();
        println!("{:?}", x);
        assert_eq!(x.file_id(), "MDF     ");
        assert_eq!(x.version(), 300);
        // write val to file
        let mut handle = OpenOptions::new()
            .write(true)
            .create(true)
            .open("./test_result.txt")
            .unwrap();
        let iter = file.get_node_ids::<v3::CNBlock>().unwrap().into_iter();
        let _x: Vec<f64> = iter
            .map(|node_id| {
                let temp = file.get_data_cnblock(ValueFormat::Physical, node_id, 0);
                let name = file.get_node_by_id::<v3::CNBlock>(node_id).unwrap().name();
                println!("({name},{temp})");
                write!(handle, "{},{}\n", name, temp).unwrap();
                temp
            })
            .collect();
    }

    #[test]
    fn mdf4_file_init() {
        let mut file = MDFFile::new();
        file.open("./mdf4.mf4".to_string()).unwrap();

        // get out the id block
        assert!(file.id.is_some());
        let id = file.id.take().unwrap();
        // test file is mdf3 block
        // assert!(file.arena[id].type_id() == TypeId::of::<v4::IDBlock>());

        // downcast id block
        let x = file.arena[id].get().downcast_ref::<v4::IDBlock>().unwrap();
        println!("{:?}", x);
        assert_eq!(x.file_id(), "MDF     ");
        assert_eq!(x.version(), 410);
    }

    #[test]
    #[should_panic]
    fn error_file_init() {
        let mut file = MDFFile::new();
        file.open("./error.dbc".to_string()).unwrap();
    }
}
