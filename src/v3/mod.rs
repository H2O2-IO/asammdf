use super::SpecVer;
use super::UnfinalizedFlagsType;
use crate::ByteOrder;
use crate::MDFObject;
use crate::PermanentBlock;
use asammdf_derive::{id_object, mdf_object};

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
#[derive(Debug)]
pub struct IDBlock {
    pub byte_order: Option<ByteOrder>,
    pub code_page: u16,
    pub format_id: String,
    pub float_point_format: Option<FloatPointFormat>,
    pub program_id: String,
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
