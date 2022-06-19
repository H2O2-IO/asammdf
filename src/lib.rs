use std::{
    any::Any,
    error::Error,
    fmt::Display,
    fs::File,
    io::{self, BufReader, Read},
};

use indextree::{Arena, NodeEdge, NodeId};

pub mod v3;
pub mod v4;

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

#[derive(Clone, Copy, Debug)]
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
#[derive(Clone, Copy, Debug)]
pub enum ConversionType {
    ParametricLinear,
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

/// File type of supported mdf file.
#[derive(Clone, Copy, Debug)]
pub enum FileType {
    Dat,
    Mdf,
    Mf3,
    Mf4,
}

/// Support version of MDF specification
#[derive(Clone, Copy, Debug)]
pub enum SpecVer {
    /// support to v3.30
    V3,
    /// support to v4.10
    V4,
}
#[derive(Clone, Copy, Debug)]
pub enum RecordIDType {
    None,
    Before8Bit,
    Before16Bit,
    Before32Bit = 4,
    Before64Bit = 8,
    BeforeAndAfter8Bit = 255,
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
#[derive(Clone, Copy, Debug)]
pub enum SyncType {
    Time,
    Angle,
    Distance,
    Index,
}
#[derive(Clone, Copy, Debug)]
pub enum TimeFlagsType {
    LocalTime = 1,
    OffsetsValid,
}
#[derive(Clone, Copy, Debug)]
pub enum TimeQualityType {
    LocalPC,
    ExternalSource = 10,
    ExternalAbsolute = 16,
}

enum_u32_convert! {
#[derive(Clone, Copy, Debug)]
pub enum UnfinalizedFlagsType {
    UpdateCGBlockRequired = 1,
    UpdateSRBlockRequired,
}
}

#[derive(Clone, Copy, Debug)]
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

pub trait CommentObject: MDFObject {
    fn comment(&self) -> String;
}

pub trait IDObject: MDFObject {
    fn file_id(&self) -> String;
    fn spec_type(&self) -> SpecVer;
    fn unfinalized_flags_type(&self) -> UnfinalizedFlagsType;
    fn version(&self) -> u16;
    fn custom_flags(&self) -> u16;
}

pub trait CCObject {
    fn conversion_type(&self) -> Option<ConversionType>;
    fn default_text(&self) -> Option<String>;
    fn set_default_text(&self, default_text: String);
    fn formula(&self) -> Option<String>;
    fn set_formula(&self, formula: String);
    fn max(&self) -> Option<f64>;
    fn set_max(&self, max: f64);
    fn min(&self) -> Option<f64>;
    fn set_min(&self, min: f64);
}

/// Block that can be stored inside internal arena
trait PermanentBlock: MDFObject {}

pub struct MDFFile {
    /// Note that only description node are stored inside arena
    arena: Arena<Box<dyn Any>>,
    /// NodeId of this file's IDBlock
    id: Option<NodeId>,
    /// NodeId of this file's HDBlock
    header: Option<NodeId>,
    /// Source file path
    pub source_file: String,
    /// file handler to the source file
    file_handler: Option<File>,
    spec_ver: Option<SpecVer>,
}

#[derive(Debug)]
pub enum MDFErrorKind {
    IOError(io::Error),
    IDBlockError(String),
}

impl MDFFile {
    pub fn new() -> MDFFile {
        MDFFile {
            arena: Arena::new(),
            id: None,
            header: None,
            source_file: Default::default(),
            file_handler: Default::default(),
            spec_ver: Default::default(),
        }
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

        if idblock.file_id != "MDF     " {
            return Err(MDFErrorKind::IDBlockError(
                "File id is not 'MDF     '".to_string(),
            ));
        }
        // if version greater than 400, should parse it as v4::IDBlock;
        if idblock.version >= 400 {
            let idblock = v4::IDBlock::parse(&idblock_buf).ok_or_else(|| {
                MDFErrorKind::IDBlockError("Faild to parse id block(v4)".to_string())
            })?;
            self.id = Some(self.arena.new_node(Box::new(idblock)));
            self.spec_ver = Some(SpecVer::V4);
        } else {
            self.id = Some(self.arena.new_node(Box::new(idblock)));
            self.spec_ver = Some(SpecVer::V3);
        }
        self.source_file = file_path.clone();

        self.init();

        Ok(())
    }

    fn init(&mut self) {}

    fn get_node<T: 'static + PermanentBlock>(&self) -> Option<&T> {
        self.id.and_then(|id| {
            id.descendants(&self.arena)
                .find(|x| self.arena[*x].get().downcast_ref::<T>().is_some())
                .map_or(None, |z| self.arena[z].get().downcast_ref::<T>())
        })
    }

    fn get_node_id<T: 'static + PermanentBlock>(&self) -> Option<NodeId> {
        self.id.and_then(|id| {
            id.descendants(&self.arena)
                .find(|x| self.arena[*x].get().downcast_ref::<T>().is_some())
        })
    }

    fn get_nodes<T: 'static + PermanentBlock>(&self) -> Option<Vec<&T>> {
        self.id.and_then(|id| {
            Some(
                id.descendants(&self.arena)
                    .filter(|x| self.arena[*x].get().downcast_ref::<T>().is_some())
                    .map(|y| self.arena[y].get().downcast_ref::<T>().unwrap())
                    .collect(),
            )
        })
    }

    fn get_mut_node<T: 'static + PermanentBlock>(&mut self, id: NodeId) -> Option<&mut T> {
        self.arena[id].get_mut().downcast_mut::<T>()
    }

    /// NodeId is Copyable,
    fn get_node_ids<T: 'static + PermanentBlock>(&mut self) -> Option<Vec<NodeId>> {
        self.id.and_then(|id| {
            Some(
                id.descendants(&self.arena)
                    .filter(|x| self.arena[*x].get().downcast_ref::<T>().is_some())
                    .collect(),
            )
        })
    }

    fn get_node_mut_by_name<T: 'static + PermanentBlock>(&mut self, name: &str) -> Option<&mut T> {
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

    /// remove node for arena by arena node id
    /// TODO: wrap NodeId inside a new type
    fn remove_node(&mut self, id: NodeId, recursive: bool) {
        if recursive {
            id.remove_subtree(&mut self.arena);
        } else {
            id.remove(&mut self.arena);
        }
    }

    fn append_node(&mut self, parent_id: NodeId, child_id: NodeId) {
        parent_id.append(child_id, &mut self.arena);
    }
}

#[cfg(test)]
mod tests {
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
        assert_eq!(id_ref.name,"ID");
        // downcast id block
        let x = file.arena[id].get().downcast_ref::<v3::IDBlock>().unwrap();
        println!("{:?}", x);
        assert_eq!(x.file_id, "MDF     ");
        assert_eq!(x.version, 300);

        let id_ref_mut = file.get_mut_node::<v3::IDBlock>(id).unwrap();
        (*id_ref_mut).version = 400;
        assert_eq!(id_ref_mut.version,400);
        let id_ref_mut2 = file.get_mut_node::<v3::IDBlock>(id).unwrap();
        (*id_ref_mut2).version = 300;
        assert_eq!(id_ref_mut2.version,300);
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
        assert_eq!(x.file_id, "MDF     ");
        assert_eq!(x.version, 410);
    }
}
